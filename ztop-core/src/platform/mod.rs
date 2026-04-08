use cfg_if::cfg_if;
use nix::time::ClockId;

cfg_if! {
    if #[cfg(target_os = "freebsd")] {
        mod freebsd;
        pub(crate) use freebsd::SnapshotIter;
        pub(crate) const CLOCK_UPTIME: ClockId = ClockId::CLOCK_UPTIME;
    } else if #[cfg(target_os = "linux")] {
        mod linux;
        pub(crate) use linux::SnapshotIter;
        pub(crate) const CLOCK_UPTIME: ClockId = ClockId::CLOCK_BOOTTIME;
    }
}
