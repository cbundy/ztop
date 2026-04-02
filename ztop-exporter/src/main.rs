use std::{
    io::Write,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use clap::Parser;
use tiny_http::{Response, Server};
use ztop_core::DataSource;

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

fn render_metrics(ds: &mut DataSource) -> Vec<u8> {
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

    for elem in ds.iter() {
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
            let body = render_metrics(&mut ds.lock().unwrap());
            let response = Response::from_data(body);
            let _ = request.respond(response);
        } else {
            let _ = request.respond(
                Response::from_string("Not Found\n").with_status_code(404),
            );
        }
    }
}
