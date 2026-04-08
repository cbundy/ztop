use std::{
    collections::{btree_map, BTreeMap},
    error::Error,
    mem,
};

use nix::time::{clock_gettime, ClockId};

use crate::element::Element;
use crate::platform::CLOCK_UPTIME;
use crate::snapshot::Snapshot;

#[derive(Default)]
pub struct DataSource {
    children: bool,
    prev:     BTreeMap<String, Snapshot>,
    prev_ts:  Option<nix::sys::time::TimeSpec>,
    cur:      BTreeMap<String, Snapshot>,
    cur_ts:   Option<nix::sys::time::TimeSpec>,
    pools:    Vec<String>,
}

impl DataSource {
    pub fn new(children: bool, pools: Vec<String>) -> Self {
        DataSource {
            children,
            pools,
            ..Default::default()
        }
    }

    /// Iterate through all the datasets, returning current stats
    pub fn iter(&mut self) -> impl Iterator<Item = Element> + '_ {
        let etime = if let Some(prev_ts) = self.prev_ts.as_ref() {
            let delta = *self.cur_ts.as_ref().unwrap() - *prev_ts;
            delta.tv_sec() as f64 + delta.tv_nsec() as f64 * 1e-9
        } else {
            let boottime = clock_gettime(CLOCK_UPTIME).unwrap();
            boottime.tv_sec() as f64 + boottime.tv_nsec() as f64 * 1e-9
        };
        DataSourceIter {
            inner_iter: self.cur.iter(),
            ds: self,
            etime,
        }
    }

    /// Iterate over all of the names of parent datasets of the argument
    pub(crate) fn with_parents(s: &str) -> impl Iterator<Item = &str> {
        s.char_indices().filter_map(move |(idx, c)| {
            if c == '/' {
                Some(s.split_at(idx).0)
            } else if idx == s.len() - 1 {
                Some(s)
            } else {
                None
            }
        })
    }

    pub fn refresh(&mut self) -> Result<(), Box<dyn Error>> {
        let now = clock_gettime(ClockId::CLOCK_MONOTONIC)?;
        self.prev = mem::take(&mut self.cur);
        self.prev_ts = self.cur_ts.replace(now);
        if self.pools.is_empty() {
            for rss in Snapshot::iter(None).unwrap() {
                let ss = rss?;
                Self::upsert(&mut self.cur, ss, self.children);
            }
        } else {
            for pool in self.pools.iter() {
                for rss in Snapshot::iter(Some(pool)).unwrap() {
                    let ss = rss?;
                    Self::upsert(&mut self.cur, ss, self.children);
                }
            }
        }
        Ok(())
    }

    pub fn toggle_children(&mut self) -> Result<(), Box<dyn Error>> {
        self.children ^= true;
        // Wipe out previous statistics.  The next refresh will report stats
        // since boot.
        self.refresh()?;
        mem::take(&mut self.prev);
        self.prev_ts = None;
        Ok(())
    }

    /// Insert a snapshot into `cur`, and/or update it and its parents
    fn upsert(
        cur: &mut BTreeMap<String, Snapshot>,
        ss: Snapshot,
        children: bool,
    ) {
        if children {
            for dsname in Self::with_parents(&ss.name) {
                match cur.entry(dsname.to_string()) {
                    btree_map::Entry::Vacant(ve) => {
                        if ss.name == dsname {
                            ve.insert(ss.clone());
                        } else {
                            let mut parent_ss = ss.clone();
                            parent_ss.name = dsname.to_string();
                            ve.insert(parent_ss);
                        }
                    }
                    btree_map::Entry::Occupied(mut oe) => {
                        *oe.get_mut() += &ss;
                    }
                }
            }
        } else {
            match cur.entry(ss.name.clone()) {
                btree_map::Entry::Vacant(ve) => {
                    ve.insert(ss);
                }
                btree_map::Entry::Occupied(mut oe) => {
                    *oe.get_mut() += &ss;
                }
            }
        };
    }
}

struct DataSourceIter<'a> {
    inner_iter: btree_map::Iter<'a, String, Snapshot>,
    ds:         &'a DataSource,
    etime:      f64,
}

impl Iterator for DataSourceIter<'_> {
    type Item = Element;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner_iter
            .next()
            .map(|(_, ss)| ss.compute(self.ds.prev.get(&ss.name), self.etime))
    }
}

#[cfg(test)]
mod t {
    mod with_parents {
        use super::super::*;

        /// The empty string is not a valid dataset, but make sure nothing bad
        /// happens anyway
        #[test]
        fn empty() {
            let ds = "";
            let mut actual = DataSource::with_parents(ds);
            assert!(actual.next().is_none());
        }

        #[test]
        fn pool() {
            let ds = "zroot";
            let expected = ["zroot"];
            let actual = DataSource::with_parents(ds).collect::<Vec<_>>();
            assert_eq!(&expected[..], &actual[..]);
        }

        #[test]
        fn one_level() {
            let ds = "zroot/ROOT";
            let expected = ["zroot", "zroot/ROOT"];
            let actual = DataSource::with_parents(ds).collect::<Vec<_>>();
            assert_eq!(&expected[..], &actual[..]);
        }

        #[test]
        fn two_levels() {
            let ds = "zroot/ROOT/13.0-RELEASE";
            let expected = ["zroot", "zroot/ROOT", "zroot/ROOT/13.0-RELEASE"];
            let actual = DataSource::with_parents(ds).collect::<Vec<_>>();
            assert_eq!(&expected[..], &actual[..]);
        }
    }
}
