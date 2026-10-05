//! Published reference values, read from the CSV files in `data/reference/` (compiled into
//! the binary so tests and the report always use the cited numbers).

const GHIA: &str = include_str!("../data/reference/ghia1982.csv");
const CAVITY_EXTREMA: &str = include_str!("../data/reference/cavity_extrema.csv");
const SCHAEFER_TUREK: &str = include_str!("../data/reference/schaefer_turek1996.csv");
const CORNERS: &str = include_str!("../data/reference/square_cylinder_low_re.csv");

/// Data rows of a CSV text: comment lines (#) and blank lines skipped, fields trimmed.
pub fn rows(text: &str) -> Vec<Vec<String>> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.split(',').map(|f| f.trim().to_string()).collect())
        .collect()
}

/// Ghia, Ghia & Shin (1982) centreline profile: (coordinate, velocity / lid velocity).
/// `table` is "u" (vertical centreline) or "v" (horizontal centreline); `re` is 100 or 1000.
pub fn ghia(table: &str, re: u32) -> Vec<(f64, f64)> {
    let col = match re {
        100 => 3,
        1000 => 4,
        _ => panic!("Ghia data stored for Re = 100 and 1000 only"),
    };
    let mut v: Vec<(f64, f64)> =
        rows(GHIA).iter().filter(|r| r[0] == table).map(|r| (r[2].parse().unwrap(), r[col].parse().unwrap())).collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    v
}

/// Centreline extremum (value, position) from `source` = "ghia", "botella-peyret" or "marchi".
pub fn cavity_extremum(source: &str, re: u32, quantity: &str) -> Option<(f64, f64)> {
    rows(CAVITY_EXTREMA)
        .iter()
        .find(|r| r[0] == source && r[1] == re.to_string() && r[2] == quantity)
        .map(|r| (r[3].parse().unwrap(), r[4].parse().unwrap()))
}

/// Schafer & Turek (1996) reference interval for `case` "2D-1"/"2D-2" and a quantity.
pub fn st_interval(case: &str, quantity: &str) -> Option<(f64, f64)> {
    st_row("interval", case, quantity)
}

/// Later, more precise value for the same benchmark (FeatFlow pages), where one exists.
pub fn st_later(case: &str, quantity: &str) -> Option<f64> {
    st_row("later", case, quantity).map(|r| r.0)
}

fn st_row(kind: &str, case: &str, quantity: &str) -> Option<(f64, f64)> {
    rows(SCHAEFER_TUREK)
        .iter()
        .find(|r| r[0] == kind && r[1] == case && r[2] == quantity)
        .map(|r| (r[3].parse().unwrap(), r[4].parse().unwrap()))
}

/// Published low-Reynolds-number results for square-section cylinders (for context only).
/// Rows: source key, shape, Re, blockage, Cd, St, Cl_rms (empty where not reported).
pub fn square_cylinder_literature() -> Vec<Vec<String>> {
    rows(CORNERS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_files_parse() {
        for re in [100, 1000] {
            for t in ["u", "v"] {
                let g = ghia(t, re);
                assert_eq!(g.len(), 17);
                assert_eq!(g[0].0, 0.0);
                assert_eq!(g[16].0, 1.0);
            }
        }
        assert_eq!(ghia("u", 1000)[16].1, 1.0);
        assert!(ghia("u", 1000).iter().any(|p| p.1 == -0.38289));
        assert!(ghia("v", 1000).iter().any(|p| p.1 == -0.51550));
        assert_eq!(st_interval("2D-1", "cd"), Some((5.57, 5.59)));
        assert_eq!(st_interval("2D-2", "st"), Some((0.295, 0.305)));
        assert_eq!(st_later("2D-2", "cl_max"), Some(0.986571));
        assert_eq!(cavity_extremum("botella-peyret", 1000, "u_min").unwrap().0, -0.3885698);
        for r in square_cylinder_literature() {
            assert_eq!(r.len(), 7, "{r:?}");
        }
    }
}
