/// One thing to display in the table
#[derive(Clone, Debug)]
pub struct Element {
    pub name:       String,
    /// Read IOPs
    pub ops_r:      f64,
    /// Read B/s
    pub r_s:        f64,
    /// Files unlinked per second
    pub ops_unlink: f64,
    /// Write IOPs
    pub ops_w:      f64,
    /// Write B/s
    pub w_s:        f64,
}
