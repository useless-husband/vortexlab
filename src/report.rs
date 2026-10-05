//! Builds the static HTML report from the CSV files written by `validate` and `corners`.

use std::path::Path;

pub fn build(_results: &Path, _out: &Path) -> Result<(), String> {
    Err("not implemented yet".into())
}
