use std::ops::AddAssign;

use crate::element::Element;
use crate::platform::SnapshotIter;

/// A snapshot in time of a dataset's statistics.
///
/// The various fields are not saved atomically, but ought to be close.
#[derive(Clone, Debug, Default)]
pub(crate) struct Snapshot {
    pub(crate) name:      String,
    pub(crate) nunlinked: u64,
    pub(crate) nunlinks:  u64,
    pub(crate) nread:     u64,
    pub(crate) reads:     u64,
    pub(crate) nwritten:  u64,
    pub(crate) writes:    u64,
}

impl Snapshot {
    pub(crate) fn compute(&self, prev: Option<&Self>, etime: f64) -> Element {
        if let Some(prev) = prev {
            Element {
                name:       self.name.clone(),
                ops_r:      (self.reads - prev.reads) as f64 / etime,
                r_s:        (self.nread - prev.nread) as f64 / etime,
                ops_w:      (self.writes - prev.writes) as f64 / etime,
                w_s:        (self.nwritten - prev.nwritten) as f64 / etime,
                ops_unlink: (self.nunlinked - prev.nunlinked) as f64 / etime,
            }
        } else {
            Element {
                name:       self.name.clone(),
                ops_r:      self.reads as f64 / etime,
                r_s:        self.nread as f64 / etime,
                ops_w:      self.writes as f64 / etime,
                w_s:        self.nwritten as f64 / etime,
                ops_unlink: self.nunlinked as f64 / etime,
            }
        }
    }

    /// Iterate through ZFS datasets, returning stats for each.
    ///
    /// Iterates through every dataset beneath each of the given pools, or
    /// through all datasets if no pool is supplied.
    pub(crate) fn iter(
        pool: Option<&str>,
    ) -> Result<SnapshotIter, Box<dyn std::error::Error>> {
        SnapshotIter::new(pool)
    }
}

impl AddAssign<&Self> for Snapshot {
    fn add_assign(&mut self, other: &Self) {
        assert!(
            other.name.starts_with(&self.name),
            "Why would you want to combine two unrelated datasets?"
        );
        self.nunlinked += other.nunlinked;
        self.nunlinks += other.nunlinks;
        self.nread += other.nread;
        self.reads += other.reads;
        self.nwritten += other.nwritten;
        self.writes += other.writes;
    }
}
