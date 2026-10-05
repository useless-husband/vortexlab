//! Static SVG charts for the report. No scripts: the report must work from `file://` in a
//! browser with JavaScript restricted. Colours come from CSS classes defined by the page
//! (`s1`..`s5` for series, `grid`, `axis`, ...), so the charts follow light and dark themes.

use std::fmt::Write;

#[derive(Clone, Debug)]
pub struct Series {
    pub name: String,
    /// Colour slot 1..=5 (fixed per entity across the report).
    pub slot: usize,
    pub pts: Vec<(f64, f64)>,
    pub line: bool,
    pub markers: bool,
    pub dashed: bool,
}

impl Series {
    pub fn line(name: &str, slot: usize, pts: Vec<(f64, f64)>) -> Series {
        Series { name: name.into(), slot, pts, line: true, markers: false, dashed: false }
    }
    pub fn points(name: &str, slot: usize, pts: Vec<(f64, f64)>) -> Series {
        Series { name: name.into(), slot, pts, line: false, markers: true, dashed: false }
    }
    pub fn both(name: &str, slot: usize, pts: Vec<(f64, f64)>) -> Series {
        Series { name: name.into(), slot, pts, line: true, markers: true, dashed: false }
    }
    pub fn dashed(mut self) -> Series {
        self.dashed = true;
        self
    }
}

#[derive(Clone, Debug, Default)]
pub struct Plot {
    pub w: f64,
    pub h: f64,
    pub x_label: String,
    pub y_label: String,
    pub log_x: bool,
    pub log_y: bool,
    pub x_range: Option<(f64, f64)>,
    pub y_range: Option<(f64, f64)>,
    pub series: Vec<Series>,
    /// Horizontal reference bands: (y low, y high, label).
    pub bands: Vec<(f64, f64, String)>,
    /// Accessible description.
    pub desc: String,
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Short human-readable number for tick labels and tooltips.
pub fn num(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let a = v.abs();
    if !(1e-3..1e5).contains(&a) {
        let e = a.log10().floor();
        let m = v / 10f64.powf(e);
        return if (m.abs() - 1.0).abs() < 1e-9 { format!("{}1e{}", if v < 0.0 { "-" } else { "" }, e) } else { format!("{:.1}e{}", m, e) };
    }
    let s = format!("{:.4}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

/// "Nice" tick positions covering [lo, hi].
pub fn ticks(lo: f64, hi: f64, target: usize) -> Vec<f64> {
    let span = (hi - lo).max(1e-300);
    let raw = span / target.max(1) as f64;
    let mag = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 2.5, 5.0, 10.0].iter().map(|m| m * mag).find(|s| span / s <= target as f64 + 0.5).unwrap_or(10.0 * mag);
    let mut t = (lo / step).ceil() * step;
    let mut out = Vec::new();
    while t <= hi + 1e-9 * span {
        out.push(if t.abs() < 1e-12 * span.max(1.0) { 0.0 } else { t });
        t += step;
    }
    out
}

fn log_ticks(lo: f64, hi: f64) -> Vec<f64> {
    let (a, b) = (lo.log10().floor() as i32, hi.log10().ceil() as i32);
    let mut out = Vec::new();
    for e in a..=b {
        for m in if b - a <= 2 { vec![1.0, 2.0, 5.0] } else { vec![1.0] } {
            let v = m * 10f64.powi(e);
            if v >= lo * 0.999 && v <= hi * 1.001 {
                out.push(v);
            }
        }
    }
    out
}

impl Plot {
    pub fn new(x_label: &str, y_label: &str) -> Plot {
        Plot { w: 560.0, h: 340.0, x_label: x_label.into(), y_label: y_label.into(), ..Plot::default() }
    }

    pub fn render(&self) -> String {
        let (ml, mr, mt, mb) = (58.0, 16.0, if self.series.len() > 1 { 30.0 } else { 12.0 }, 44.0);
        let (pw, ph) = (self.w - ml - mr, self.h - mt - mb);
        let all = || self.series.iter().flat_map(|s| s.pts.iter().copied());
        let bounds = |f: &dyn Fn((f64, f64)) -> f64, log: bool, fixed: Option<(f64, f64)>, extra: &[f64]| -> (f64, f64) {
            if let Some(r) = fixed {
                return r;
            }
            let vals: Vec<f64> = all().map(f).chain(extra.iter().copied()).filter(|v| v.is_finite() && (!log || *v > 0.0)).collect();
            let (lo, hi) = vals.iter().fold((f64::MAX, f64::MIN), |a, &v| (a.0.min(v), a.1.max(v)));
            if lo > hi {
                return (0.0, 1.0);
            }
            if log {
                (lo / 1.3, hi * 1.3)
            } else {
                let pad = if hi > lo { 0.06 * (hi - lo) } else { 0.5 };
                (lo - pad, hi + pad)
            }
        };
        let band_ys: Vec<f64> = self.bands.iter().flat_map(|b| [b.0, b.1]).collect();
        let (x0, x1) = bounds(&|p| p.0, self.log_x, self.x_range, &[]);
        let (y0, y1) = bounds(&|p| p.1, self.log_y, self.y_range, &band_ys);
        let tx = |v: f64, lo: f64, hi: f64, log: bool| if log { (v.ln() - lo.ln()) / (hi.ln() - lo.ln()) } else { (v - lo) / (hi - lo) };
        let sx = |x: f64| ml + pw * tx(x, x0, x1, self.log_x);
        let sy = |y: f64| mt + ph * (1.0 - tx(y, y0, y1, self.log_y));

        let mut o = String::new();
        let _ = write!(o, r#"<svg viewBox="0 0 {} {}" role="img" class="chart" xmlns="http://www.w3.org/2000/svg"><desc>{}</desc>"#, self.w, self.h, esc(&self.desc));
        // Grid and tick labels.
        let xt = if self.log_x { log_ticks(x0, x1) } else { ticks(x0, x1, 6) };
        let yt = if self.log_y { log_ticks(y0, y1) } else { ticks(y0, y1, 5) };
        for &t in &yt {
            let y = sy(t);
            let _ = write!(o, r#"<line class="grid" x1="{ml}" x2="{:.1}" y1="{y:.1}" y2="{y:.1}"/><text class="tick" x="{:.1}" y="{:.1}" text-anchor="end">{}</text>"#, ml + pw, ml - 6.0, y + 3.5, num(t));
        }
        for &t in &xt {
            let x = sx(t);
            let _ = write!(o, r#"<line class="grid" x1="{x:.1}" x2="{x:.1}" y1="{mt}" y2="{:.1}"/><text class="tick" x="{x:.1}" y="{:.1}" text-anchor="middle">{}</text>"#, mt + ph, mt + ph + 15.0, num(t));
        }
        // Reference bands.
        for (lo, hi, label) in &self.bands {
            let (ya, yb) = (sy(*hi), sy(*lo));
            let _ = write!(o, r#"<rect class="band" x="{ml}" y="{ya:.1}" width="{pw}" height="{:.1}"><title>{}: {} to {}</title></rect>"#, (yb - ya).max(1.5), esc(label), num(*lo), num(*hi));
            let _ = write!(o, r#"<text class="bandlbl" x="{:.1}" y="{:.1}" text-anchor="end">{}</text>"#, ml + pw - 4.0, ya - 4.0, esc(label));
        }
        let _ = write!(o, r#"<line class="axis" x1="{ml}" x2="{:.1}" y1="{:.1}" y2="{:.1}"/>"#, ml + pw, mt + ph, mt + ph);
        let _ = write!(o, r#"<text class="lbl" x="{:.1}" y="{:.1}" text-anchor="middle">{}</text>"#, ml + pw / 2.0, self.h - 6.0, esc(&self.x_label));
        let _ = write!(o, r#"<text class="lbl" transform="translate(13 {:.1}) rotate(-90)" text-anchor="middle">{}</text>"#, mt + ph / 2.0, esc(&self.y_label));
        // Data.
        for s in &self.series {
            let ok = |p: &(f64, f64)| p.0.is_finite() && p.1.is_finite() && (!self.log_x || p.0 > 0.0) && (!self.log_y || p.1 > 0.0);
            if s.line {
                let d: Vec<String> = s.pts.iter().filter(|p| ok(p)).map(|p| format!("{:.1},{:.1}", sx(p.0), sy(p.1))).collect();
                let _ = write!(o, r#"<polyline class="ln s{}{}" points="{}"/>"#, s.slot, if s.dashed { " dash" } else { "" }, d.join(" "));
            }
            if s.markers {
                for p in s.pts.iter().filter(|p| ok(p)) {
                    let _ = write!(o, r#"<circle class="mk f{}" cx="{:.1}" cy="{:.1}" r="4"><title>{}: {}, {}</title></circle>"#, s.slot, sx(p.0), sy(p.1), esc(&s.name), num(p.0), num(p.1));
                }
            }
        }
        // Legend (only when there is more than one series).
        if self.series.len() > 1 {
            let mut x = ml;
            for s in &self.series {
                if s.line {
                    let _ = write!(o, r#"<line class="ln s{}{}" x1="{x:.1}" x2="{:.1}" y1="12" y2="12"/>"#, s.slot, if s.dashed { " dash" } else { "" }, x + 18.0);
                }
                if s.markers {
                    let _ = write!(o, r#"<circle class="mk f{}" cx="{:.1}" cy="12" r="4"/>"#, s.slot, x + 9.0);
                }
                let _ = write!(o, r#"<text class="leg" x="{:.1}" y="16">{}</text>"#, x + 24.0, esc(&s.name));
                x += 36.0 + 6.4 * s.name.chars().count() as f64;
            }
        }
        o.push_str("</svg>");
        o
    }
}

/// Horizontal bar chart: one bar per item, value written at the bar end. `reference` draws a
/// vertical marker (e.g. the plain-square value) for comparison.
pub fn bars(items: &[(String, f64)], slot: usize, decimals: usize, reference: Option<f64>, desc: &str) -> String {
    let (w, row, ml, mr) = (360.0, 26.0, 118.0, 56.0);
    let h = row * items.len() as f64 + 8.0;
    let max = items.iter().map(|i| i.1).fold(0.0, f64::max).max(1e-300);
    let pw = w - ml - mr;
    let mut o = String::new();
    let _ = write!(o, r#"<svg viewBox="0 0 {w} {h}" role="img" class="chart bars" xmlns="http://www.w3.org/2000/svg"><desc>{}</desc>"#, esc(desc));
    for (k, (name, v)) in items.iter().enumerate() {
        let y = 4.0 + row * k as f64;
        let len = (pw * v / max).max(1.0);
        let _ = write!(o, r#"<text class="leg" x="{:.1}" y="{:.1}" text-anchor="end">{}</text>"#, ml - 8.0, y + 15.0, esc(name));
        // Square at the baseline, rounded at the data end.
        let r = 4.0f64.min(len / 2.0);
        let _ = write!(
            o,
            r#"<path class="f{slot}" d="M{ml},{y:.1}h{:.1}a{r},{r} 0 0 1 {r},{r}v{:.1}a{r},{r} 0 0 1 -{r},{r}h-{:.1}z"><title>{}: {:.*}</title></path>"#,
            len - r,
            row - 6.0 - 2.0 * r,
            len - r,
            esc(name),
            decimals,
            v
        );
        let _ = write!(o, r#"<text class="val" x="{:.1}" y="{:.1}">{:.*}</text>"#, ml + len + 6.0, y + 15.0, decimals, v);
    }
    if let Some(r) = reference {
        let x = ml + pw * r / max;
        let _ = write!(o, r#"<line class="refline" x1="{x:.1}" x2="{x:.1}" y1="0" y2="{h}"/>"#);
    }
    let _ = write!(o, r#"<line class="axis" x1="{ml}" x2="{ml}" y1="0" y2="{h}"/></svg>"#);
    o
}

/// Colour-bar legend for a palette: `palette` colours spread from `lo` to `hi`.
pub fn colorbar(palette: &[[u8; 3]], lo: f64, hi: f64, label: &str) -> String {
    let (w, h, ml, bw) = (420.0, 46.0, 10.0, 400.0);
    let n = 50usize.min(palette.len());
    let mut o = String::new();
    let _ = write!(o, r#"<svg viewBox="0 0 {w} {h}" role="img" class="chart colorbar" xmlns="http://www.w3.org/2000/svg"><desc>{}</desc>"#, esc(label));
    for k in 0..n {
        let c = palette[k * (palette.len() - 1) / (n - 1)];
        let _ = write!(o, r##"<rect x="{:.2}" y="2" width="{:.2}" height="14" fill="#{:02x}{:02x}{:02x}"/>"##, ml + bw * k as f64 / n as f64, bw / n as f64 + 0.3, c[0], c[1], c[2]);
    }
    for (k, v) in [lo, 0.5 * lo, 0.0, 0.5 * hi, hi].iter().enumerate() {
        let x = ml + bw * k as f64 / 4.0;
        let anchor = ["start", "middle", "middle", "middle", "end"][k];
        let sign = if *v > 0.0 { "+" } else { "" };
        let edge = if k == 0 { "≤ " } else if k == 4 { "≥ " } else { "" };
        let _ = write!(o, r#"<text class="tick" x="{x:.1}" y="29" text-anchor="{anchor}">{edge}{sign}{}</text>"#, num(*v));
    }
    let _ = write!(o, r#"<text class="lbl" x="{:.1}" y="43" text-anchor="middle">{}</text></svg>"#, ml + bw / 2.0, esc(label));
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nice_ticks() {
        assert_eq!(ticks(0.0, 1.0, 5), vec![0.0, 0.2, 0.4, 0.6000000000000001, 0.8, 1.0]);
        let t = ticks(-0.43, 1.07, 5);
        assert!(t.contains(&0.0) && t.len() >= 3 && t.len() <= 7);
        assert_eq!(log_ticks(1.0, 1000.0), vec![1.0, 10.0, 100.0, 1000.0]);
    }

    #[test]
    fn number_formatting() {
        assert_eq!(num(0.0), "0");
        assert_eq!(num(0.25), "0.25");
        assert_eq!(num(3.0), "3");
        assert_eq!(num(1e-6), "1e-6");
        assert_eq!(num(-120.5), "-120.5");
    }

    #[test]
    fn plots_are_well_formed() {
        let mut p = Plot::new("x <cells>", "error & more");
        p.log_x = true;
        p.log_y = true;
        p.series.push(Series::both("a<b", 1, vec![(8.0, 1e-2), (16.0, 2.5e-3), (32.0, 6e-4)]));
        p.series.push(Series::line("slope 2", 2, vec![(8.0, 1e-2), (32.0, 6.25e-4)]).dashed());
        p.bands.push((1e-3, 2e-3, "ref".into()));
        let s = p.render();
        assert!(s.starts_with("<svg") && s.ends_with("</svg>"));
        assert!(s.contains("a&lt;b") && !s.contains("a<b"));
        assert_eq!(s.matches("<circle").count(), 3 + 1); // three points + one legend marker
        assert!(!s.contains("NaN") && !s.contains("inf"));
        let b = bars(&[("square".into(), 1.5), ("rounded".into(), 1.2)], 1, 3, Some(1.5), "drag");
        assert_eq!(b.matches("<path").count(), 2);
        assert!(b.contains("1.500") && !b.contains("NaN"));
        let c = colorbar(&crate::render::palette()[..crate::render::LEVELS], -3.0, 3.0, "vorticity");
        assert_eq!(c.matches("<rect").count(), 50);
    }
}
