//! Builds the static HTML report from the CSV files written by `validate`, `corners` and
//! `bench`. Plain HTML + inline SVG + image files next to it: no scripts, no network.

use crate::reference;
use crate::render;
use crate::suite::Table;
use crate::svg::{self, esc, Plot, Series};
use std::fmt::Write;
use std::path::Path;

const CSS: &str = r#"
:root{color-scheme:light;--page:#f9f9f7;--surface:#fcfcfb;--ink:#0b0b0b;--ink2:#52514e;--muted:#898781;--grid:#e1e0d9;--axis:#c3c2b7;--rule:#dddcd4;
--s1:#2a78d6;--s2:#eb6834;--s3:#1baf7a;--s4:#eda100;--s5:#e87ba4;--good:#0ca30c;--bad:#d03b3b;--band:rgba(27,175,122,.16);--code:#f0efec}
@media (prefers-color-scheme:dark){:root:not([data-theme="light"]){color-scheme:dark;--page:#0d0d0d;--surface:#1a1a19;--ink:#fff;--ink2:#c3c2b7;--muted:#898781;--grid:#2c2c2a;--axis:#383835;--rule:#383835;
--s1:#3987e5;--s2:#d95926;--s3:#199e70;--s4:#c98500;--s5:#d55181;--band:rgba(25,158,112,.22);--code:#262624}}
:root[data-theme="dark"]{color-scheme:dark;--page:#0d0d0d;--surface:#1a1a19;--ink:#fff;--ink2:#c3c2b7;--muted:#898781;--grid:#2c2c2a;--axis:#383835;--rule:#383835;
--s1:#3987e5;--s2:#d95926;--s3:#199e70;--s4:#c98500;--s5:#d55181;--band:rgba(25,158,112,.22);--code:#262624}
*{box-sizing:border-box}
body{margin:0;background:var(--page);color:var(--ink);font:16px/1.55 system-ui,-apple-system,"Segoe UI","PingFang TC","Noto Sans TC",sans-serif}
main{max-width:1040px;margin:0 auto;padding:28px 16px 64px}
h1{font-size:28px;line-height:1.2;margin:0 0 6px}h2{font-size:21px;margin:44px 0 6px;padding-top:14px;border-top:1px solid var(--rule)}h3{font-size:16px;margin:24px 0 6px}
p,li{max-width:76ch}p.zh,span.zh{color:var(--ink2)}.lead{color:var(--ink2);margin:0 0 14px}
code{background:var(--code);padding:1px 4px;border-radius:3px;font:13px/1.4 ui-monospace,Menlo,monospace}
pre{background:var(--code);padding:10px 12px;border-radius:4px;overflow-x:auto;font:13px/1.45 ui-monospace,Menlo,monospace}
.tw{overflow-x:auto;margin:10px 0}table{border-collapse:collapse;font-size:14px;font-variant-numeric:tabular-nums}
th,td{padding:5px 10px;border-bottom:1px solid var(--rule);text-align:right;white-space:nowrap}th{color:var(--ink2);font-weight:600}
td:first-child,th:first-child{text-align:left}tr.ref td{color:var(--ink2);background:var(--surface)}
.ok::before{content:"\2713\00a0";color:var(--good);font-weight:700}.no::before{content:"\2717\00a0";color:var(--bad);font-weight:700}
.figs{display:flex;flex-wrap:wrap;gap:14px;margin:12px 0}.fig{background:var(--surface);border:1px solid var(--rule);border-radius:6px;padding:10px;flex:1 1 320px;min-width:0;max-width:600px}
.fig h4{margin:0 0 4px;font-size:14px}.fig p{margin:4px 0 0;font-size:13px;color:var(--ink2)}.fig img{display:block;width:100%;height:auto;image-rendering:auto;border-radius:3px}
svg.chart{display:block;width:100%;height:auto}svg.colorbar{max-width:420px}
.grid{stroke:var(--grid);stroke-width:1}.axis{stroke:var(--axis);stroke-width:1}.tick{fill:var(--muted);font-size:11px}.lbl{fill:var(--ink2);font-size:12px}.leg,.val{fill:var(--ink);font-size:12px}
.band{fill:var(--band)}.bandlbl{fill:var(--ink2);font-size:11px}.refline{stroke:var(--ink2);stroke-width:1;stroke-dasharray:3 3}
.ln{fill:none;stroke-width:2;stroke-linejoin:round}.dash{stroke-dasharray:5 4}.mk{stroke:var(--surface);stroke-width:2}
.s1{stroke:var(--s1)}.s2{stroke:var(--s2)}.s3{stroke:var(--s3)}.s4{stroke:var(--s4)}.s5{stroke:var(--s5)}
.f1{fill:var(--s1)}.f2{fill:var(--s2)}.f3{fill:var(--s3)}.f4{fill:var(--s4)}.f5{fill:var(--s5)}
.note{border-left:3px solid var(--axis);padding:2px 12px;color:var(--ink2);margin:12px 0}
"#;

fn table(head: &[&str], rows: &[(Vec<String>, bool)]) -> String {
    let mut o = String::from("<div class=\"tw\"><table><thead><tr>");
    for h in head {
        let _ = write!(o, "<th>{h}</th>");
    }
    o.push_str("</tr></thead><tbody>");
    for (r, is_ref) in rows {
        o.push_str(if *is_ref { "<tr class=\"ref\">" } else { "<tr>" });
        for c in r {
            let _ = write!(o, "<td>{c}</td>");
        }
        o.push_str("</tr>");
    }
    o.push_str("</tbody></table></div>");
    o
}

fn fig(title: &str, body: &str, caption: &str) -> String {
    format!(
        "<div class=\"fig\"><h4>{title}</h4>{body}{}</div>",
        if caption.is_empty() { String::new() } else { format!("<p>{caption}</p>") }
    )
}

/// A value with a check or cross against an interval, and how far outside it is.
fn judged(v: f64, decimals: usize, iv: Option<(f64, f64)>) -> String {
    match iv {
        None => format!("{v:.decimals$}"),
        Some((lo, hi)) if v >= lo && v <= hi => format!("<span class=\"ok\" title=\"inside the interval\">{v:.decimals$}</span>"),
        Some((lo, hi)) => {
            let edge = if v < lo { lo } else { hi };
            format!("<span class=\"no\" title=\"outside the interval\">{v:.decimals$}</span> ({:+.2}%)", 100.0 * (v - edge) / edge.abs())
        }
    }
}

fn pct(v: f64, r: f64) -> String {
    format!("{:+.2}%", 100.0 * (v - r) / r.abs())
}

fn poiseuille(dir: &Path) -> String {
    let Some(t) = Table::read(&dir.join("poiseuille.csv")) else { return String::from("<p>Not run.</p>") };
    let mut schemes: Vec<String> = Vec::new();
    for r in 0..t.rows.len() {
        if !schemes.contains(&t.text(r, "scheme").to_string()) {
            schemes.push(t.text(r, "scheme").to_string());
        }
    }
    let mut rows = Vec::new();
    let mut plot = Plot::new("fluid rows across the channel", "relative L2 error of the velocity profile");
    (plot.log_x, plot.log_y) = (true, true);
    plot.desc = "Poiseuille flow error against resolution for four scheme variants; all fall with slope 2.".into();
    let mut slot = 1;
    for s in &schemes {
        let idx: Vec<usize> = (0..t.rows.len()).filter(|&r| t.text(r, "scheme") == s).collect();
        let pts: Vec<(f64, f64)> = idx.iter().map(|&r| (t.num(r, "rows"), t.num(r, "l2"))).collect();
        let exact = pts.iter().all(|p| p.1 < 1e-9);
        for &r in &idx {
            rows.push((
                vec![
                    esc(s),
                    t.text(r, "rows").into(),
                    format!("{:.3e}", t.num(r, "l2")),
                    if exact { "exact".into() } else { t.text(r, "order").into() },
                    format!("{:.10}", t.num(r, "balance")),
                ],
                false,
            ));
        }
        if !exact {
            plot.series.push(Series::both(s, slot, pts));
            slot += 1;
        }
    }
    if let Some(first) = plot.series.first().map(|s| s.pts.clone()) {
        let (a, b) = (first[0], first[first.len() - 1]);
        plot.series.push(Series::line("slope 2", 5, vec![(a.0, a.1 * 2.0), (b.0, a.1 * 2.0 * (a.0 / b.0).powi(2))]).dashed());
    }
    format!(
        "<div class=\"figs\">{}</div>{}",
        fig("Grid convergence", &plot.render(), "Error against the exact parabola. The dashed guide has slope 2 (second order)."),
        table(&["scheme", "rows", "L2 error", "observed order", "wall drag / driving force"], &rows)
    )
}

fn cavity(dir: &Path) -> String {
    let Some(t) = Table::read(&dir.join("cavity.csv")) else { return String::from("<p>Not run.</p>") };
    let mut o = String::new();
    // Profiles of the finest run at each Reynolds number.
    let mut figs = String::new();
    for re in [100u32, 1000] {
        let runs: Vec<usize> = (0..t.rows.len()).filter(|&r| t.num(r, "re") == re as f64).collect();
        let Some(&best) = runs.iter().max_by_key(|&&r| t.num(r, "n") as usize) else { continue };
        let n = t.num(best, "n") as usize;
        let Some(p) = Table::read(&dir.join(format!("cavity_profile_re{re}_n{n}.csv"))) else { continue };
        for (line, xl, yl) in [("u", "y", "u / U_lid on the vertical centreline"), ("v", "x", "v / U_lid on the horizontal centreline")] {
            let pts: Vec<(f64, f64)> =
                (0..p.rows.len()).filter(|&r| p.text(r, "line") == line).map(|r| (p.num(r, "s"), p.num(r, "value"))).collect();
            let mut plot = Plot::new(xl, yl);
            plot.desc = format!("Cavity centreline profile at Re {re} compared with the 17 tabulated points of Ghia et al.");
            plot.series.push(Series::line(&format!("vortexlab, {n} x {n}"), 1, pts));
            plot.series.push(Series::points("Ghia, Ghia & Shin (1982)", 2, reference::ghia(line, re)));
            figs += &fig(&format!("Re = {re}: {line} profile"), &plot.render(), "");
        }
    }
    let _ = write!(o, "<div class=\"figs\">{figs}</div>");
    let mut rows = Vec::new();
    for r in 0..t.rows.len() {
        rows.push((
            vec![
                t.text(r, "re").into(),
                format!("{0} x {0}", t.text(r, "n")),
                format!("{:.5}", t.num(r, "max_dev_u")),
                format!("{:.5}", t.num(r, "max_dev_v")),
                format!("{:.5}", t.num(r, "rms_dev")),
                format!("{} (Ghia {})", t.text(r, "worst_point"), t.text(r, "worst_ref")),
                t.text(r, "steps").into(),
                t.text(r, "converged").into(),
            ],
            false,
        ));
    }
    o += "<h3>Deviation from Ghia's table</h3><p>Largest and root-mean-square difference over the 2 x 15 interior tabulated points, in units of the lid speed (Ghia's table has five decimals).</p>";
    o += &table(&["Re", "grid", "max |Δu|", "max |Δv|", "RMS", "worst point", "steps", "steady"], &rows);
    // Extrema against the spectral reference.
    let mut rows = Vec::new();
    for re in [100u32, 1000] {
        for r in (0..t.rows.len()).filter(|&r| t.num(r, "re") == re as f64) {
            let mut cells = vec![re.to_string(), format!("vortexlab {0} x {0}", t.text(r, "n"))];
            for q in ["u_min", "v_max", "v_min"] {
                let bp = reference::cavity_extremum("botella-peyret", re, q).unwrap().0;
                cells.push(format!("{:.5} ({})", t.num(r, q), pct(t.num(r, q), bp)));
            }
            rows.push((cells, false));
        }
        for (src, label) in [("ghia", "Ghia et al. 1982, 129 x 129"), ("botella-peyret", "Botella &amp; Peyret 1998, spectral")] {
            let mut cells = vec![re.to_string(), label.to_string()];
            for q in ["u_min", "v_max", "v_min"] {
                let v = reference::cavity_extremum(src, re, q).unwrap().0;
                let bp = reference::cavity_extremum("botella-peyret", re, q).unwrap().0;
                cells.push(if src == "ghia" { format!("{v:.5} ({})", pct(v, bp)) } else { format!("{v:.7}") });
            }
            rows.push((cells, true));
        }
    }
    o += "<h3>Profile extrema against the spectral benchmark</h3><p>Percentages are relative to Botella &amp; Peyret. Ghia's own table is 1-2 % away from the spectral values at Re = 1000, so agreement with Ghia much better than that is not meaningful.</p>";
    o += &table(&["Re", "source", "u min", "v max", "v min"], &rows);
    o
}

struct Quantity {
    key: &'static str,
    label: &'static str,
    decimals: usize,
}

fn cylinder(dir: &Path, file: &str, case: &str, quantities: &[Quantity], base_u: f64) -> String {
    let Some(t) = Table::read(&dir.join(file)) else { return String::from("<p>Not run.</p>") };
    let mut head: Vec<String> = vec!["cells per D".into(), "lattice U".into()];
    head.extend(quantities.iter().map(|q| q.label.to_string()));
    let mut rows = Vec::new();
    let mut order: Vec<usize> = (0..t.rows.len()).collect();
    order.sort_by(|&a, &b| {
        (t.num(a, "u_mean") != base_u, t.num(a, "n") as usize).cmp(&(t.num(b, "u_mean") != base_u, t.num(b, "n") as usize))
    });
    for r in order {
        let mut cells = vec![t.text(r, "n").to_string(), t.text(r, "u_mean").to_string()];
        for q in quantities {
            cells.push(judged(t.num(r, q.key), q.decimals, reference::st_interval(case, q.key)));
        }
        rows.push((cells, false));
    }
    let mut iv = vec!["Schäfer &amp; Turek 1996 interval".to_string(), String::new()];
    let mut later = vec!["later reference (FeatFlow)".to_string(), String::new()];
    for q in quantities {
        iv.push(reference::st_interval(case, q.key).map(|(a, b)| format!("{a:.0$} – {b:.0$}", q.decimals)).unwrap_or_else(|| "–".into()));
        later.push(
            reference::st_later(case, q.key)
                .map(|v| judged(v, q.decimals + 1, reference::st_interval(case, q.key)))
                .unwrap_or_else(|| "–".into()),
        );
    }
    rows.push((iv, true));
    rows.push((later, true));
    let head_refs: Vec<&str> = head.iter().map(String::as_str).collect();
    let mut figs = String::new();
    for q in quantities {
        let Some((lo, hi)) = reference::st_interval(case, q.key) else { continue };
        let pts: Vec<(f64, f64)> =
            (0..t.rows.len()).filter(|&r| t.num(r, "u_mean") == base_u).map(|r| (t.num(r, "n"), t.num(r, q.key))).collect();
        let mut plot = Plot::new("cells per cylinder diameter", q.label);
        (plot.w, plot.h, plot.log_x) = (330.0, 230.0, true);
        plot.desc = format!("{} against resolution with the published reference interval as a band.", q.label);
        plot.bands.push((lo, hi, "1996 interval".into()));
        plot.series.push(Series::both(q.label, 1, pts));
        if let Some(v) = reference::st_later(case, q.key) {
            let x: Vec<f64> = plot.series[0].pts.iter().map(|p| p.0).collect();
            plot.series.push(Series::line("later reference", 2, vec![(x[0], v), (x[x.len() - 1], v)]).dashed());
        }
        figs += &fig(q.label, &plot.render(), "");
    }
    format!("{}<div class=\"figs\">{figs}</div>", table(&head_refs, &rows))
}

fn unsteady_series(dir: &Path) -> String {
    let Some(t) = Table::read(&dir.join("cylinder_2d2.csv")) else { return String::new() };
    let Some(best) = (0..t.rows.len()).max_by_key(|&r| t.num(r, "n") as usize) else { return String::new() };
    let (n, u) = (t.text(best, "n"), t.text(best, "u_mean"));
    let Some(s) = Table::read(&dir.join(format!("cylinder_2d2_series_n{n}_u{u}.csv"))) else { return String::new() };
    let col = |name: &str| -> Vec<(f64, f64)> { (0..s.rows.len()).map(|r| (0.1 * s.num(r, "t"), s.num(r, name))).collect() };
    let t_end = 0.1 * s.num(s.rows.len() - 1, "t");
    let window = |v: Vec<(f64, f64)>| -> Vec<(f64, f64)> { v.into_iter().filter(|p| p.0 >= t_end - 1.0).collect() };
    let mut figs = String::new();
    for (name, label) in [("cl", "lift coefficient c_L"), ("cd", "drag coefficient c_D"), ("dp", "pressure difference ΔP (Pa)")] {
        let mut plot = Plot::new("time (s)", label);
        (plot.w, plot.h) = (330.0, 230.0);
        plot.desc = format!("{label} over the last second of the run: a regular periodic signal.");
        plot.series.push(Series::line(label, 1, window(col(name))));
        figs += &fig(label, &plot.render(), "");
    }
    // Lift spectrum of the stored window.
    let cl: Vec<f64> = (0..s.rows.len()).map(|r| s.num(r, "cl")).collect();
    let dt = s.num(1, "t") - s.num(0, "t");
    let spec: Vec<(f64, f64)> = crate::signal::spectrum(&cl, dt, 8).into_iter().filter(|p| p.0 > 0.02 && p.0 < 1.2 && p.1 > 1e-6).collect();
    let mut plot = Plot::new("Strouhal number  f D / U", "lift amplitude spectrum");
    (plot.w, plot.h, plot.log_y) = (330.0, 230.0, true);
    plot.desc = "Amplitude spectrum of the lift coefficient: one sharp line at the shedding frequency and its odd harmonics.".into();
    plot.series.push(Series::line("lift spectrum", 1, spec));
    figs += &fig("lift spectrum", &plot.render(), "Hann window, 8x zero padding; the Strouhal number is the interpolated peak.");
    format!("<h3>Force history, {n} cells per diameter</h3><div class=\"figs\">{figs}</div>")
}

const SHAPE_LABEL: [(&str, &str, &str); 5] = [
    ("square", "sharp square", "直角方柱"),
    ("chamfered", "chamfered", "切角"),
    ("rounded", "rounded", "圓角"),
    ("recessed", "single recess", "單層內凹"),
    ("double-recessed", "double recess (saw-tooth)", "雙層內凹（鋸齒）"),
];

fn label(shape: &str) -> &'static str {
    SHAPE_LABEL.iter().find(|s| s.0 == shape).map(|s| s.1).unwrap_or("?")
}

fn corner_tables(dir: &Path) -> Vec<(String, Table)> {
    let mut v: Vec<(String, Table)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if let Some(tag) = name.strip_prefix("corners_").and_then(|n| n.strip_suffix(".csv")) {
                if !tag.contains("_series") && !tag.ends_with("series") {
                    if let Some(t) = Table::read(&e.path()).filter(|t| t.header.iter().any(|h| h == "cl_rms")) {
                        v.push((tag.to_string(), t));
                    }
                }
            }
        }
    }
    v.sort_by(|a, b| (a.0 != "main", &a.0).cmp(&(b.0 != "main", &b.0)));
    v
}

fn corner_rows(t: &Table) -> Vec<(Vec<String>, bool)> {
    let base = (0..t.rows.len()).find(|&r| t.text(r, "shape") == "square");
    let mut rows = Vec::new();
    for (shape, _, _) in SHAPE_LABEL {
        let Some(r) = (0..t.rows.len()).find(|&r| t.text(r, "shape") == shape) else { continue };
        let rel = |k: &str| base.filter(|&b| b != r).map(|b| format!(" ({})", pct(t.num(r, k), t.num(b, k)))).unwrap_or_default();
        rows.push((
            vec![
                label(shape).to_string(),
                format!("{:.3}{}", t.num(r, "cd_mean"), rel("cd_mean")),
                format!("{:.3}{}", t.num(r, "cl_rms"), rel("cl_rms")),
                format!("{:.4}{}", t.num(r, "st"), rel("st")),
                format!("{:.4}", t.num(r, "st_crossings")),
                format!("{:.4}", t.num(r, "cd_rms")),
            ],
            false,
        ));
    }
    rows
}

fn corners(dir: &Path, out: &Path) -> Result<String, String> {
    let tables = corner_tables(dir);
    let Some((tag, t)) = tables.first() else { return Ok(String::from("<p>Not run.</p>")) };
    let mut o = String::new();
    let _ = write!(
        o,
        "<p>Main run: Reynolds number {}, {} cells across the section, blockage {:.1} %, corner size {} D, stream velocity {} (Mach {:.2}), {} convective time units, statistics over the second half.</p>",
        t.text(0, "re"),
        t.text(0, "d"),
        100.0 * t.num(0, "blockage"),
        t.text(0, "b"),
        t.text(0, "u"),
        crate::units::mach(t.num(0, "u")),
        t.text(0, "units")
    );
    o += &table(&["section", "mean drag C_D", "RMS lift C_L'", "Strouhal St", "St (zero crossings)", "RMS of C_D"], &corner_rows(t));
    o += "<p>Percentages are changes relative to the sharp square in the same run set.</p>";
    // Small multiples: one bar chart per measure.
    let mut figs = String::new();
    for (k, (key, title, dec)) in
        [("cd_mean", "Mean drag coefficient", 3), ("cl_rms", "RMS lift coefficient", 3), ("st", "Strouhal number", 4)].iter().enumerate()
    {
        let items: Vec<(String, f64)> = SHAPE_LABEL
            .iter()
            .filter_map(|s| (0..t.rows.len()).find(|&r| t.text(r, "shape") == s.0).map(|r| (s.1.to_string(), t.num(r, key))))
            .collect();
        let base = items.first().map(|i| i.1);
        figs += &fig(title, &svg::bars(&items, k + 1, *dec, base, title), "Dashed line: the sharp square.");
    }
    let _ = write!(o, "<div class=\"figs\">{figs}</div>");
    // Pictures.
    let scale = crate::cases::tunnel::VORTICITY_FULL_SCALE;
    let _ = write!(
        o,
        "<h3>Wakes</h3><p>Vorticity ω D / U, clockwise blue, counter-clockwise red; the colour scale is clamped at ±{scale}, so the thin shear layers on the body are saturated. Each animation is one shedding period, looped.</p>{}",
        svg::colorbar(&render::palette()[..render::LEVELS], -scale, scale, "vorticity ω D / U")
    );
    let mut figs = String::new();
    for (shape, en, zh) in SHAPE_LABEL {
        let (gif, png) = (format!("corners_{tag}_{shape}.gif"), format!("corners_{tag}_{shape}.png"));
        let file = if dir.join(&gif).exists() { gif } else { png };
        if !dir.join(&file).exists() {
            continue;
        }
        std::fs::copy(dir.join(&file), out.join(&file)).map_err(|e| format!("copy {file}: {e}"))?;
        let r = (0..t.rows.len()).find(|&r| t.text(r, "shape") == shape);
        let cap = r
            .map(|r| format!("C_D {:.3}, C_L' {:.3}, St {:.4}", t.num(r, "cd_mean"), t.num(r, "cl_rms"), t.num(r, "st")))
            .unwrap_or_default();
        figs += &fig(
            &format!("{en} <span class=\"zh\">{zh}</span>"),
            &format!("<img src=\"{file}\" alt=\"Vorticity in the wake of the {en} section\" loading=\"lazy\">"),
            &cap,
        );
    }
    let _ = write!(o, "<div class=\"figs\">{figs}</div>");
    // Lift histories of the main set.
    let mut plot = Plot::new("time (D / U)", "lift coefficient C_L");
    (plot.w, plot.h) = (980.0, 300.0);
    plot.desc = "Lift coefficient histories of the five sections over the last 30 time units.".into();
    for (k, (shape, en, _)) in SHAPE_LABEL.iter().enumerate() {
        if let Some(s) = Table::read(&dir.join(format!("corners_{tag}_{shape}_series.csv"))) {
            let t_end = s.num(s.rows.len() - 1, "t");
            let pts: Vec<(f64, f64)> = (0..s.rows.len()).map(|r| (s.num(r, "t"), s.num(r, "cl"))).filter(|p| p.0 >= t_end - 30.0).collect();
            plot.series.push(Series::line(en, k + 1, pts));
        }
    }
    if !plot.series.is_empty() {
        o += &format!("<div class=\"figs\"><div class=\"fig\" style=\"max-width:none;flex-basis:100%\"><h4>Lift history, last 30 time units</h4>{}</div></div>", plot.render());
    }
    // Baseline against the literature, and the other run sets.
    let mut rows = Vec::new();
    for (tag, t) in &tables {
        if let Some(r) = (0..t.rows.len()).find(|&r| t.text(r, "shape") == "square") {
            rows.push((
                vec![
                    format!("vortexlab, set “{}”", esc(tag)),
                    t.text(r, "re").into(),
                    format!("{:.1} %", 100.0 * t.num(r, "blockage")),
                    format!("{} cells / D, U = {}", t.text(r, "d"), t.text(r, "u")),
                    format!("{:.3}", t.num(r, "cd_mean")),
                    format!("{:.4}", t.num(r, "st")),
                    format!("{:.3}", t.num(r, "cl_rms")),
                ],
                false,
            ));
        }
    }
    for r in reference::square_cylinder_literature() {
        rows.push((
            vec![
                format!("Sohankar et al. 1998 ({})", esc(&r[0]["sohankar1998-".len()..])),
                r[2].clone(),
                format!("{:.1} %", 100.0 * r[3].parse::<f64>().unwrap_or(0.0)),
                "finite volume".into(),
                r[4].clone(),
                r[5].clone(),
                r[6].clone(),
            ],
            true,
        ));
    }
    o += "<h3>The plain square against published 2-D results</h3>";
    o += &table(&["source", "Re", "blockage", "resolution", "C_D", "St", "C_L'"], &rows);
    if tables.len() > 1 {
        o += "<h3>Other run sets (sensitivity)</h3>";
        for (tag, t) in &tables[1..] {
            let _ = write!(
                o,
                "<p>Set “{}”: Re {}, {} cells / D, blockage {:.1} %, corner size {} D, U = {}.</p>",
                esc(tag),
                t.text(0, "re"),
                t.text(0, "d"),
                100.0 * t.num(0, "blockage"),
                t.text(0, "b"),
                t.text(0, "u")
            );
            o +=
                &table(&["section", "mean drag C_D", "RMS lift C_L'", "Strouhal St", "St (zero crossings)", "RMS of C_D"], &corner_rows(t));
        }
    }
    Ok(o)
}

fn performance(dir: &Path) -> String {
    let mut rows = Vec::new();
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| n.starts_with("bench_") && n.ends_with(".csv"))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    for name in names {
        let Some(t) = Table::read(&dir.join(&name)) else { continue };
        let single = t.num(0, "mlups");
        for r in 0..t.rows.len() {
            rows.push((
                vec![
                    t.text(r, "grid").into(),
                    t.text(r, "fluid_nodes").into(),
                    t.text(r, "threads").into(),
                    format!("{:.0}", t.num(r, "mlups")),
                    format!("{:.2}x", t.num(r, "mlups") / single),
                    t.text(r, "state_hash").into(),
                ],
                false,
            ));
        }
    }
    if rows.is_empty() {
        return String::from("<p>Not run.</p>");
    }
    table(&["grid", "fluid nodes", "threads", "MLUPS", "speed-up", "state hash after the run"], &rows)
}

/// Writes `index.html` (and copies the pictures it shows) into `out`.
pub fn build(results: &Path, out: &Path) -> Result<(), String> {
    if !results.is_dir() {
        return Err(format!("no results folder at {}; run `vortexlab validate` first", results.display()));
    }
    let q1 = [
        Quantity { key: "cd", label: "drag c_D", decimals: 4 },
        Quantity { key: "cl", label: "lift c_L", decimals: 5 },
        Quantity { key: "la", label: "recirculation length L_a (m)", decimals: 4 },
        Quantity { key: "dp", label: "pressure difference ΔP (Pa)", decimals: 4 },
    ];
    let q2 = [
        Quantity { key: "cd_max", label: "maximum drag c_D", decimals: 4 },
        Quantity { key: "cd_min", label: "minimum drag c_D", decimals: 4 },
        Quantity { key: "cl_max", label: "maximum lift c_L", decimals: 4 },
        Quantity { key: "cl_min", label: "minimum lift c_L", decimals: 4 },
        Quantity { key: "st", label: "Strouhal number", decimals: 4 },
        Quantity { key: "dp", label: "ΔP half a period after the lift maximum (Pa)", decimals: 4 },
    ];
    let mut h = String::new();
    let _ = write!(
        h,
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>vortexlab validation report</title><style>{CSS}</style></head><body><main>"
    );
    h += include_str!("report_intro.html");
    h += "<h2 id=\"poiseuille\">1. Channel flow against the exact solution <span class=\"zh\">管道流對照解析解</span></h2>";
    h += include_str!("report_poiseuille.html");
    h += &poiseuille(results);
    h += "<h2 id=\"cavity\">2. Lid-driven cavity against Ghia, Ghia &amp; Shin (1982) <span class=\"zh\">頂蓋驅動方腔</span></h2>";
    h += include_str!("report_cavity.html");
    h += &cavity(results);
    h += "<h2 id=\"cylinder\">3. Cylinder in a channel: the Schäfer–Turek benchmark <span class=\"zh\">圓柱繞流標準考題</span></h2>";
    h += include_str!("report_cylinder.html");
    h += "<h3>Case 2D-1: steady flow, Re = 20</h3>";
    h += &cylinder(results, "cylinder_2d1.csv", "2D-1", &q1, 0.04);
    h += "<h3>Case 2D-2: periodic vortex shedding, Re = 100</h3>";
    h += &cylinder(results, "cylinder_2d2.csv", "2D-2", &q2, 0.05);
    h += &unsteady_series(results);
    h += include_str!("report_cylinder_notes.html");
    h += "<h2 id=\"corners\">4. Experiment: corner modifications of a square tower section <span class=\"zh\">台北 101 的鋸齒角為什麼有用</span></h2>";
    h += include_str!("report_corners.html");
    h += &corners(results, out)?;
    h += include_str!("report_corners_notes.html");
    h += "<h2 id=\"performance\">5. Speed <span class=\"zh\">速度</span></h2>";
    h += include_str!("report_performance.html");
    h += &performance(results);
    h += include_str!("report_outro.html");
    h += "</main></body></html>\n";
    std::fs::write(out.join("index.html"), h).map_err(|e| format!("cannot write the report: {e}"))
}
