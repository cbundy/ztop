# ztop Docker
I forked this with the sole intention of wrapping it in a docker container so it's trivial to run on TrueNAS without needing to install any packages.

The image contains two binaries: the interactive TUI (`ztop`) and a Prometheus metrics exporter (`ztop-exporter`). The default entrypoint is `ztop-exporter`.

## TUI (interactive)

Run the terminal UI interactively. ZFS stats are read from the host via the
`/proc/spl/kstat/zfs` filesystem (Linux) or `kstat.zfs` sysctl (FreeBSD).

```bash
# All datasets, refresh every second
docker run -it --entrypoint /bin/ztop ghcr.io/cbundy/ztop

# Specific pool only
docker run -it --entrypoint /bin/ztop ghcr.io/cbundy/ztop tank

# All ztop flags work as normal, e.g. sort by read bytes, include children
docker run -it --entrypoint /bin/ztop ghcr.io/cbundy/ztop -s "kB/s r" -c
```

See [TUI keybindings](#tui-keybindings) below for interactive controls.

## Prometheus exporter

The exporter scrapes ZFS dataset I/O stats on a background thread and serves
them in Prometheus text format on `GET /metrics`.

```bash
# Default: port 9901, refresh every 15 seconds, all pools
docker run -d --network host ghcr.io/cbundy/ztop

# Custom port and interval
docker run -d --network host ghcr.io/cbundy/ztop --port 9100 --interval 30

# Specific pool, include child dataset rollups
docker run -d --network host ghcr.io/cbundy/ztop tank --children

# Verify
curl http://localhost:9901/metrics
```

`--network host` is the simplest way to give the container access to ZFS kernel
stats. Alternatively, bind-mount the relevant paths:

```bash
# Linux
docker run -d -p 9901:9901 \
  -v /proc/spl/kstat/zfs:/proc/spl/kstat/zfs:ro \
  ghcr.io/cbundy/ztop
```

### Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `zfs_dataset_read_ops_per_second` | Gauge | Read IOPS |
| `zfs_dataset_read_bytes_per_second` | Gauge | Read throughput (bytes/s) |
| `zfs_dataset_write_ops_per_second` | Gauge | Write IOPS |
| `zfs_dataset_write_bytes_per_second` | Gauge | Write throughput (bytes/s) |
| `zfs_dataset_unlink_ops_per_second` | Gauge | Unlink/delete ops per second |

All metrics carry a `dataset` label with the full dataset name (e.g. `tank/data`).

### Prometheus scrape config

```yaml
scrape_configs:
  - job_name: zfs
    static_configs:
      - targets: ['localhost:9901']
```

## TUI keybindings

| Key | Action |
|-----|--------|
| `+` / `-` | Cycle sort column forward / backward |
| `r` | Toggle reverse sort |
| `a` | Toggle auto-hide idle datasets |
| `c` | Toggle child dataset rollup |
| `d` / `D` | Increase / decrease display depth |
| `f` | Set name filter (regex) |
| `F` | Clear filter |
| `<` / `>` | Halve / double refresh interval |
| `q` | Quit |

# ztop

Display ZFS datasets' I/O in real time

[![Build Status](https://api.cirrus-ci.com/github/asomers/ztop.svg)](https://cirrus-ci.com/github/asomers/ztop)
[![Crates.io](https://img.shields.io/crates/v/ztop.svg)](https://crates.io/crates/ztop)

# Overview

`ztop` is like `top`, but for ZFS datasets.  It displays the real-time activity
for datasets.  The built-in `zpool iostat` can display real-time I/O statistics
for pools, but until now there was no similar tool for datasets.

# Platform support

`ztop` works on FreeBSD 12 and later, and Linux.

# Screenshot

![Screenshot 1](https://raw.githubusercontent.com/asomers/ztop/master/doc/demo.gif)

# Minimum Supported Rust Version (MSRV)

ztop does not guarantee any specific MSRV.  Rather, it guarantees compatibility
with the oldest rustc shipped in the package collection of each supported
operating system.

* https://www.freshports.org/lang/rust/

# License

`ztop` is primarily distributed under the terms of the BSD 2-clause license.

See LICENSE for details.

# Sponsorship

ztop is sponsored by Axcient, inc.
