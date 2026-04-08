use std::{
    io::Write,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use clap::Parser;
use tiny_http::{Response, Server};
use ztop_core::{DataSource, Element};

/// Export ZFS dataset I/O stats as Prometheus metrics
#[derive(Debug, Parser)]
struct Cli {
    /// Port to listen on
    #[clap(short, long, default_value = "9901")]
    port: u16,

    /// Include child datasets' stats with their parents'
    #[clap(short, long)]
    children: bool,

    /// Refresh interval in seconds
    #[clap(short, long, default_value = "15")]
    interval: u64,

    /// Only export stats for these pools (default: all pools)
    pools: Vec<String>,
}

fn render_metrics(elements: impl Iterator<Item = Element>) -> Vec<u8> {
    let mut buf = Vec::new();

    macro_rules! header {
        ($name:expr, $help:expr) => {
            writeln!(buf, "# HELP {} {}", $name, $help).unwrap();
            writeln!(buf, "# TYPE {} gauge", $name).unwrap();
        };
    }

    header!(
        "zfs_dataset_read_ops_per_second",
        "Read operations per second"
    );
    header!(
        "zfs_dataset_read_bytes_per_second",
        "Read bytes per second"
    );
    header!(
        "zfs_dataset_write_ops_per_second",
        "Write operations per second"
    );
    header!(
        "zfs_dataset_write_bytes_per_second",
        "Write bytes per second"
    );
    header!(
        "zfs_dataset_unlink_ops_per_second",
        "Unlink operations per second"
    );

    for elem in elements {
        let name = &elem.name;
        writeln!(
            buf,
            "zfs_dataset_read_ops_per_second{{dataset=\"{}\"}} {}",
            name, elem.ops_r
        )
        .unwrap();
        writeln!(
            buf,
            "zfs_dataset_read_bytes_per_second{{dataset=\"{}\"}} {}",
            name, elem.r_s
        )
        .unwrap();
        writeln!(
            buf,
            "zfs_dataset_write_ops_per_second{{dataset=\"{}\"}} {}",
            name, elem.ops_w
        )
        .unwrap();
        writeln!(
            buf,
            "zfs_dataset_write_bytes_per_second{{dataset=\"{}\"}} {}",
            name, elem.w_s
        )
        .unwrap();
        writeln!(
            buf,
            "zfs_dataset_unlink_ops_per_second{{dataset=\"{}\"}} {}",
            name, elem.ops_unlink
        )
        .unwrap();
    }

    buf
}

fn main() {
    let cli = Cli::parse();

    let mut ds = DataSource::new(cli.children, cli.pools);
    ds.refresh().unwrap();

    let ds = Arc::new(Mutex::new(ds));
    let interval = Duration::from_secs(cli.interval);

    // Background refresh thread
    {
        let ds = Arc::clone(&ds);
        thread::spawn(move || loop {
            thread::sleep(interval);
            ds.lock().unwrap().refresh().unwrap();
        });
    }

    let addr = format!("0.0.0.0:{}", cli.port);
    let server = Server::http(&addr).expect("failed to start HTTP server");
    eprintln!("Listening on http://{addr}/metrics");

    for request in server.incoming_requests() {
        if request.url() == "/metrics" {
            let body = render_metrics(ds.lock().unwrap().iter());
            let response = Response::from_data(body);
            let _ = request.respond(response);
        } else {
            let _ = request.respond(
                Response::from_string("Not Found\n").with_status_code(404),
            );
        }
    }
}

#[cfg(test)]
mod t {
    use std::io::{Read, Write as IoWrite};
    use std::net::TcpStream;

    use super::*;

    fn elem(name: &str, ops_r: f64, r_s: f64, ops_w: f64, w_s: f64, ops_unlink: f64) -> Element {
        Element {
            name: name.to_string(),
            ops_r,
            r_s,
            ops_w,
            w_s,
            ops_unlink,
        }
    }

    // ---- render_metrics unit tests ----

    #[test]
    fn render_empty_has_all_headers() {
        let output = String::from_utf8(render_metrics(std::iter::empty())).unwrap();
        for metric in &[
            "zfs_dataset_read_ops_per_second",
            "zfs_dataset_read_bytes_per_second",
            "zfs_dataset_write_ops_per_second",
            "zfs_dataset_write_bytes_per_second",
            "zfs_dataset_unlink_ops_per_second",
        ] {
            assert!(
                output.contains(&format!("# HELP {metric}")),
                "missing HELP for {metric}"
            );
            assert!(
                output.contains(&format!("# TYPE {metric} gauge")),
                "missing TYPE for {metric}"
            );
        }
        // No data lines when there are no elements
        assert!(!output.contains("dataset="), "unexpected data lines in empty output");
    }

    #[test]
    fn render_single_element() {
        let elements = vec![elem("tank/data", 10.0, 2048.0, 5.0, 4096.0, 1.0)];
        let output = String::from_utf8(render_metrics(elements.into_iter())).unwrap();

        assert!(output.contains(r#"zfs_dataset_read_ops_per_second{dataset="tank/data"} 10"#));
        assert!(output.contains(r#"zfs_dataset_read_bytes_per_second{dataset="tank/data"} 2048"#));
        assert!(output.contains(r#"zfs_dataset_write_ops_per_second{dataset="tank/data"} 5"#));
        assert!(output.contains(r#"zfs_dataset_write_bytes_per_second{dataset="tank/data"} 4096"#));
        assert!(output.contains(r#"zfs_dataset_unlink_ops_per_second{dataset="tank/data"} 1"#));
    }

    #[test]
    fn render_multiple_elements_all_present() {
        let elements = vec![
            elem("zroot", 1.0, 0.0, 1.0, 0.0, 0.0),
            elem("zroot/ROOT", 2.0, 0.0, 2.0, 0.0, 0.0),
            elem("tank/data", 3.0, 0.0, 3.0, 0.0, 0.0),
        ];
        let output = String::from_utf8(render_metrics(elements.into_iter())).unwrap();

        assert!(output.contains(r#"dataset="zroot""#));
        assert!(output.contains(r#"dataset="zroot/ROOT""#));
        assert!(output.contains(r#"dataset="tank/data""#));
    }

    #[test]
    fn render_zero_values() {
        let elements = vec![elem("tank", 0.0, 0.0, 0.0, 0.0, 0.0)];
        let output = String::from_utf8(render_metrics(elements.into_iter())).unwrap();
        assert!(output.contains(r#"zfs_dataset_read_ops_per_second{dataset="tank"} 0"#));
    }

    // ---- HTTP integration tests ----

    fn http_get(port: u16, path: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(stream, "GET {path} HTTP/1.0\r\nHost: localhost\r\n\r\n").unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    }

    #[test]
    fn http_metrics_returns_200_with_prometheus_body() {
        let server = Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();

        let handle = std::thread::spawn(move || {
            let req = server.recv().unwrap();
            let body = render_metrics(std::iter::once(elem("zroot", 1.0, 512.0, 2.0, 1024.0, 0.5)));
            let _ = req.respond(Response::from_data(body));
        });

        let response = http_get(port, "/metrics");
        handle.join().unwrap();

        assert!(response.contains("200 OK"), "expected 200, got: {response}");
        assert!(response.contains("# HELP zfs_dataset_read_ops_per_second"));
        assert!(response.contains(r#"dataset="zroot""#));
    }

    #[test]
    fn http_unknown_path_returns_404() {
        let server = Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();

        let handle = std::thread::spawn(move || {
            let req = server.recv().unwrap();
            let _ = req.respond(Response::from_string("Not Found\n").with_status_code(404));
        });

        let response = http_get(port, "/unknown");
        handle.join().unwrap();

        assert!(response.contains("404"), "expected 404, got: {response}");
    }
}
