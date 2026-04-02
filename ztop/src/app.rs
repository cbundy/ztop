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
    pub fn elements(&mut self) -> Vec<Element> {
        filter_and_sort(
            self.data.iter(),
            self.auto,
            self.depth,
            &self.filter,
            self.reverse,
            self.sort_idx,
        )
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

/// Apply depth, name, and auto filters then sort the given elements.
///
/// Extracted from `App::elements` so it can be tested independently of the
/// ZFS kernel interface.
#[rustfmt::skip]
fn filter_and_sort(
    source:   impl Iterator<Item = Element>,
    auto:     bool,
    depth:    Option<usize>,
    filter:   &Option<Regex>,
    reverse:  bool,
    sort_idx: Option<usize>,
) -> Vec<Element> {
    let mut v = source
        .filter(move |elem| {
            if let Some(limit) = depth {
                let edepth = elem.name.split('/').count() - 1;
                edepth <= limit
            } else {
                true
            }
        })
        .filter(|elem| {
            filter.as_ref()
                .map(|f| f.is_match(&elem.name))
                .unwrap_or(true)
        })
        .filter(|elem| !auto || (elem.r_s + elem.w_s + elem.ops_unlink > 1.0))
        .collect::<Vec<_>>();

    match (reverse, sort_idx) {
        (true,  Some(0)) => v.sort_by(|x, y| x.ops_r.total_cmp(&y.ops_r)),
        (false, Some(0)) => v.sort_by(|x, y| y.ops_r.total_cmp(&x.ops_r)),
        (true,  Some(1)) => v.sort_by(|x, y| x.r_s.total_cmp(&y.r_s)),
        (false, Some(1)) => v.sort_by(|x, y| y.r_s.total_cmp(&x.r_s)),
        (true,  Some(2)) => v.sort_by(|x, y| x.ops_w.total_cmp(&y.ops_w)),
        (false, Some(2)) => v.sort_by(|x, y| y.ops_w.total_cmp(&x.ops_w)),
        (true,  Some(3)) => v.sort_by(|x, y| x.w_s.total_cmp(&y.w_s)),
        (false, Some(3)) => v.sort_by(|x, y| y.w_s.total_cmp(&x.w_s)),
        (true,  Some(4)) => v.sort_by(|x, y| x.ops_unlink.total_cmp(&y.ops_unlink)),
        (false, Some(4)) => v.sort_by(|x, y| y.ops_unlink.total_cmp(&x.ops_unlink)),
        (false, Some(5)) => v.sort_by(|x, y| x.name.cmp(&y.name)),
        (true,  Some(5)) => v.sort_by(|x, y| y.name.cmp(&x.name)),
        _ => ()
    }
    v
}

#[cfg(test)]
mod t {
    use super::*;

    fn elem(name: &str, ops_r: f64, r_s: f64, ops_w: f64, w_s: f64, ops_unlink: f64) -> Element {
        Element { name: name.to_string(), ops_r, r_s, ops_w, w_s, ops_unlink }
    }

    fn names(v: &[Element]) -> Vec<&str> {
        v.iter().map(|e| e.name.as_str()).collect()
    }

    // ---- App state-machine tests (use App::default(), no ZFS needed) ----

    #[test]
    fn quit_starts_false_becomes_true() {
        let mut app = App::default();
        assert!(!app.should_quit());
        app.on_q();
        assert!(app.should_quit());
    }

    #[test]
    fn sort_cycles_forward_through_all_columns() {
        let mut app = App::default();
        assert_eq!(app.sort_idx(), None);
        for expected in [Some(0), Some(1), Some(2), Some(3), Some(4), Some(5), None] {
            app.on_plus();
            assert_eq!(app.sort_idx(), expected);
        }
    }

    #[test]
    fn sort_cycles_backward_through_all_columns() {
        let mut app = App::default();
        assert_eq!(app.sort_idx(), None);
        for expected in [Some(5), Some(4), Some(3), Some(2), Some(1), Some(0), None] {
            app.on_minus();
            assert_eq!(app.sort_idx(), expected);
        }
    }

    #[test]
    fn reverse_toggles() {
        let mut app = App::default();
        assert!(!app.reverse);
        app.on_r();
        assert!(app.reverse);
        app.on_r();
        assert!(!app.reverse);
    }

    #[test]
    fn auto_toggles() {
        let mut app = App::default();
        assert!(!app.auto);
        app.on_a();
        assert!(app.auto);
        app.on_a();
        assert!(!app.auto);
    }

    #[test]
    fn depth_increase_and_decrease() {
        let mut app = App::default();
        assert_eq!(app.depth, None);
        app.on_d(true);
        assert_eq!(app.depth, Some(1));
        app.on_d(true);
        assert_eq!(app.depth, Some(2));
        app.on_d(false);
        assert_eq!(app.depth, Some(1));
        app.on_d(false);
        assert_eq!(app.depth, Some(0));
        // saturating: stays at 0
        app.on_d(false);
        assert_eq!(app.depth, Some(0));
    }

    #[test]
    fn depth_decrease_from_none_gives_zero() {
        let mut app = App::default();
        app.on_d(false);
        assert_eq!(app.depth, Some(0));
    }

    #[test]
    fn filter_set_and_clear() {
        let mut app = App::default();
        assert!(app.filter.is_none());
        app.set_filter(Regex::new("zroot").unwrap());
        assert!(app.filter.is_some());
        app.clear_filter();
        assert!(app.filter.is_none());
    }

    #[test]
    fn elements_empty_without_zfs_data() {
        let mut app = App::default();
        assert!(app.elements().is_empty());
    }

    // ---- filter_and_sort unit tests ----

    fn sample() -> Vec<Element> {
        vec![
            elem("tank",       10.0, 1000.0, 5.0,  500.0, 1.0),
            elem("tank/logs",   2.0,  200.0, 8.0,  800.0, 3.0),
            elem("tank/data",  20.0, 2000.0, 1.0,  100.0, 0.5),
            elem("zroot",       0.0,    0.0, 0.0,    0.0, 0.0),
            elem("zroot/ROOT",  1.0,  100.0, 1.0,  100.0, 0.0),
        ]
    }

    #[test]
    fn no_filters_no_sort_preserves_order() {
        let v = filter_and_sort(sample().into_iter(), false, None, &None, false, None);
        assert_eq!(names(&v), ["tank", "tank/logs", "tank/data", "zroot", "zroot/ROOT"]);
    }

    #[test]
    fn depth_zero_keeps_only_pool_roots() {
        let v = filter_and_sort(sample().into_iter(), false, Some(0), &None, false, None);
        assert_eq!(names(&v), ["tank", "zroot"]);
    }

    #[test]
    fn depth_one_includes_one_level_deep() {
        let v = filter_and_sort(sample().into_iter(), false, Some(1), &None, false, None);
        assert_eq!(names(&v), ["tank", "tank/logs", "tank/data", "zroot", "zroot/ROOT"]);
    }

    #[test]
    fn name_filter_matches_prefix() {
        let filter = Some(Regex::new("^tank").unwrap());
        let v = filter_and_sort(sample().into_iter(), false, None, &filter, false, None);
        assert_eq!(names(&v), ["tank", "tank/logs", "tank/data"]);
    }

    #[test]
    fn auto_filter_hides_idle_datasets() {
        // Only elements where r_s + w_s + ops_unlink > 1.0 survive.
        // "zroot" has all zeros — hidden. "zroot/ROOT" has r_s=100+w_s=100 — shown.
        let v = filter_and_sort(sample().into_iter(), true, None, &None, false, None);
        assert!(!names(&v).contains(&"zroot"), "idle zroot should be filtered");
        assert!(names(&v).contains(&"tank"));
    }

    #[test]
    fn sort_by_read_ops_descending() {
        let v = filter_and_sort(sample().into_iter(), false, None, &None, false, Some(0));
        // ops_r: tank/data=20, tank=10, tank/logs=2, zroot/ROOT=1, zroot=0
        assert_eq!(v[0].name, "tank/data");
        assert_eq!(v[1].name, "tank");
    }

    #[test]
    fn sort_by_read_ops_ascending_when_reversed() {
        let v = filter_and_sort(sample().into_iter(), false, None, &None, true, Some(0));
        assert_eq!(v[0].name, "zroot");
        assert_eq!(v.last().unwrap().name, "tank/data");
    }

    #[test]
    fn sort_by_write_bytes_descending() {
        let v = filter_and_sort(sample().into_iter(), false, None, &None, false, Some(3));
        // w_s: tank/logs=800, tank=500, tank/data=100, zroot/ROOT=100, zroot=0
        assert_eq!(v[0].name, "tank/logs");
    }

    #[test]
    fn sort_by_name_alphabetical() {
        let v = filter_and_sort(sample().into_iter(), false, None, &None, false, Some(5));
        assert_eq!(names(&v), ["tank", "tank/data", "tank/logs", "zroot", "zroot/ROOT"]);
    }

    #[test]
    fn sort_by_name_reverse_alphabetical() {
        let v = filter_and_sort(sample().into_iter(), false, None, &None, true, Some(5));
        assert_eq!(names(&v), ["zroot/ROOT", "zroot", "tank/logs", "tank/data", "tank"]);
    }
}
