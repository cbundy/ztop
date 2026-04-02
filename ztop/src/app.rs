// vim: tw=80
use std::error::Error;

use regex::Regex;
use ztop_core::{DataSource, Element};

#[derive(Default)]
pub struct App {
    auto:        bool,
    data:        DataSource,
    depth:       Option<usize>,
    filter:      Option<Regex>,
    reverse:     bool,
    should_quit: bool,
    /// 0-based index of the column to sort by, if any
    sort_idx:    Option<usize>,
}

impl App {
    pub fn new(
        auto: bool,
        children: bool,
        pools: Vec<String>,
        depth: Option<usize>,
        filter: Option<Regex>,
        reverse: bool,
        sort_idx: Option<usize>,
    ) -> Self {
        let mut data = DataSource::new(children, pools);
        data.refresh().unwrap();
        App {
            auto,
            data,
            depth,
            filter,
            reverse,
            sort_idx,
            ..Default::default()
        }
    }

    pub fn clear_filter(&mut self) {
        self.filter = None;
    }

    /// Return the elements that should be displayed, in order
    #[rustfmt::skip]
    pub fn elements(&mut self) -> Vec<Element> {
        let auto = self.auto;
        let depth = self.depth;
        let filter = &self.filter;
        let mut v = self.data.iter()
            .filter(move |elem| {
                if let Some(limit) = depth {
                    let edepth = elem.name.split('/').count() - 1;
                    edepth <= limit
                } else {
                    true
                }
            }).filter(|elem|
                 filter.as_ref()
                 .map(|f| f.is_match(&elem.name))
                 .unwrap_or(true)
            ).filter(|elem| !auto ||
                     (elem.r_s + elem.w_s + elem.ops_unlink > 1.0)
            ).collect::<Vec<_>>();
        match (self.reverse, self.sort_idx) {
            (true, Some(0)) => v.sort_by(|x, y| x.ops_r.total_cmp(&y.ops_r)),
            (false,  Some(0)) => v.sort_by(|x, y| y.ops_r.total_cmp(&x.ops_r)),
            (true, Some(1)) => v.sort_by(|x, y| x.r_s.total_cmp(&y.r_s)),
            (false,  Some(1)) => v.sort_by(|x, y| y.r_s.total_cmp(&x.r_s)),
            (true, Some(2)) => v.sort_by(|x, y| x.ops_w.total_cmp(&y.ops_w)),
            (false,  Some(2)) => v.sort_by(|x, y| y.ops_w.total_cmp(&x.ops_w)),
            (true, Some(3)) => v.sort_by(|x, y| x.w_s.total_cmp(&y.w_s)),
            (false,  Some(3)) => v.sort_by(|x, y| y.w_s.total_cmp(&x.w_s)),
            (true, Some(4)) => v.sort_by(|x, y|
                x.ops_unlink.total_cmp(&y.ops_unlink)),
            (false,  Some(4)) => v.sort_by(|x, y|
                y.ops_unlink.total_cmp(&x.ops_unlink)),
            (false, Some(5)) => v.sort_by(|x, y| x.name.cmp(&y.name)),
            (true,  Some(5)) => v.sort_by(|x, y| y.name.cmp(&x.name)),
            _ => ()
        }
        v
    }

    pub fn on_a(&mut self) {
        self.auto ^= true;
    }

    pub fn on_c(&mut self) -> Result<(), Box<dyn Error>> {
        self.data.toggle_children()
    }

    pub fn on_d(&mut self, more_depth: bool) {
        self.depth = if more_depth {
            match self.depth {
                None => Some(1),
                Some(x) => Some(x + 1),
            }
        } else {
            match self.depth {
                None => Some(0),
                Some(x) => Some(x.saturating_sub(1)),
            }
        }
    }

    pub fn on_minus(&mut self) {
        self.sort_idx = match self.sort_idx {
            Some(0) => None,
            Some(old) => Some(old - 1),
            None => Some(5),
        }
    }

    pub fn on_plus(&mut self) {
        self.sort_idx = match self.sort_idx {
            Some(old) if old >= 5 => None,
            Some(old) => Some(old + 1),
            None => Some(0),
        }
    }

    pub fn on_q(&mut self) {
        self.should_quit = true;
    }

    pub fn on_r(&mut self) {
        self.reverse ^= true;
    }

    pub fn on_tick(&mut self) {
        self.data.refresh().unwrap();
    }

    pub fn set_filter(&mut self, filter: Regex) {
        self.filter = Some(filter);
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn sort_idx(&self) -> Option<usize> {
        self.sort_idx
    }
}
