//! Body shapes.
//!
//! A shape only has to answer "is this point inside?". Everything the solver needs follows from
//! that: which lattice nodes are solid, and, for every lattice link that leaves a fluid node
//! and ends in a solid node, the fraction `q` of the link that lies in the fluid (found by
//! bisection in [`cut_fraction`]). `q` is what lets the interpolated bounce-back rule see a
//! circle as a circle instead of a staircase.
//!
//! Coordinates are lattice units with nodes at integer positions.

/// Anything with an inside and an outside.
pub trait Shape: Send + Sync {
    fn inside(&self, x: f64, y: f64) -> bool;
}

pub struct Circle {
    pub cx: f64,
    pub cy: f64,
    pub r: f64,
}

impl Shape for Circle {
    fn inside(&self, x: f64, y: f64) -> bool {
        let (dx, dy) = (x - self.cx, y - self.cy);
        dx * dx + dy * dy < self.r * self.r
    }
}

/// Simple polygon (even-odd rule).
pub struct Polygon {
    pub pts: Vec<[f64; 2]>,
}

impl Shape for Polygon {
    fn inside(&self, x: f64, y: f64) -> bool {
        let n = self.pts.len();
        let mut inside = false;
        let mut j = n - 1;
        for i in 0..n {
            let (xi, yi) = (self.pts[i][0], self.pts[i][1]);
            let (xj, yj) = (self.pts[j][0], self.pts[j][1]);
            if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
                inside = !inside;
            }
            j = i;
        }
        inside
    }
}

/// Axis-aligned square of half-width `half` whose corners are rounded with radius `r`.
pub struct RoundedSquare {
    pub cx: f64,
    pub cy: f64,
    pub half: f64,
    pub r: f64,
}

impl Shape for RoundedSquare {
    fn inside(&self, x: f64, y: f64) -> bool {
        let core = self.half - self.r;
        let dx = ((x - self.cx).abs() - core).max(0.0);
        let dy = ((y - self.cy).abs() - core).max(0.0);
        (x - self.cx).abs() < self.half && (y - self.cy).abs() < self.half && dx * dx + dy * dy < self.r * self.r
    }
}

/// Another shape rotated by `angle` (radians, counter-clockwise) about (`cx`, `cy`).
pub struct Rotated {
    pub inner: Box<dyn Shape>,
    pub cx: f64,
    pub cy: f64,
    pub angle: f64,
}

impl Shape for Rotated {
    fn inside(&self, x: f64, y: f64) -> bool {
        let (s, c) = self.angle.sin_cos();
        let (dx, dy) = (x - self.cx, y - self.cy);
        self.inner.inside(self.cx + c * dx + s * dy, self.cy - s * dx + c * dy)
    }
}

/// Another shape moved by (dx, dy).
pub struct Translated {
    pub inner: Box<dyn Shape>,
    pub dx: f64,
    pub dy: f64,
}

impl Shape for Translated {
    fn inside(&self, x: f64, y: f64) -> bool {
        self.inner.inside(x - self.dx, y - self.dy)
    }
}

/// Bounding box (x_min, x_max, y_min, y_max) of the part of a shape inside the square
/// |x|, |y| <= `reach`, found by sampling every `step`; `None` if nothing is inside.
pub fn bounding_box(shape: &dyn Shape, reach: f64, step: f64) -> Option<(f64, f64, f64, f64)> {
    let n = (2.0 * reach / step).ceil() as usize;
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for j in 0..=n {
        let y = -reach + j as f64 * step;
        for i in 0..=n {
            let x = -reach + i as f64 * step;
            if shape.inside(x, y) {
                x0 = x0.min(x);
                x1 = x1.max(x);
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        }
    }
    (x0 <= x1).then_some((x0 - 0.5 * step, x1 + 0.5 * step, y0 - 0.5 * step, y1 + 0.5 * step))
}

/// A grey-level picture used as a shape: `level` is 1 inside the body and 0 outside, sampled
/// at pixel centres; the boundary is the 0.5 contour of the bilinearly interpolated picture,
/// so the outline is smooth even though the input is made of pixels.
pub struct Bitmap {
    pub w: usize,
    pub h: usize,
    /// Row-major, row 0 at the top (image convention).
    pub level: Vec<f32>,
    /// Lattice position of the picture's bottom-left corner.
    pub x0: f64,
    pub y0: f64,
    /// Lattice units per pixel.
    pub scale: f64,
}

impl Bitmap {
    fn at(&self, i: isize, j: isize) -> f64 {
        if i < 0 || j < 0 || i >= self.w as isize || j >= self.h as isize {
            0.0
        } else {
            self.level[j as usize * self.w + i as usize] as f64
        }
    }

    /// Interpolated level at a lattice position.
    pub fn sample(&self, x: f64, y: f64) -> f64 {
        // Pixel-space coordinates with pixel centres at integers; image row 0 is at the top.
        let px = (x - self.x0) / self.scale - 0.5;
        let py = (self.h as f64) - (y - self.y0) / self.scale - 0.5;
        let (i, j) = (px.floor(), py.floor());
        let (fx, fy) = (px - i, py - j);
        let (i, j) = (i as isize, j as isize);
        (1.0 - fx) * (1.0 - fy) * self.at(i, j)
            + fx * (1.0 - fy) * self.at(i + 1, j)
            + (1.0 - fx) * fy * self.at(i, j + 1)
            + fx * fy * self.at(i + 1, j + 1)
    }

    /// Bounding box (x_min, x_max, y_min, y_max) of the body in lattice units, or `None` if the
    /// picture contains no body pixels.
    pub fn bounds(&self) -> Option<(f64, f64, f64, f64)> {
        let (mut i0, mut i1, mut j0, mut j1) = (usize::MAX, 0usize, usize::MAX, 0usize);
        for j in 0..self.h {
            for i in 0..self.w {
                if self.level[j * self.w + i] > 0.5 {
                    i0 = i0.min(i);
                    i1 = i1.max(i);
                    j0 = j0.min(j);
                    j1 = j1.max(j);
                }
            }
        }
        if i0 == usize::MAX {
            return None;
        }
        Some((
            self.x0 + i0 as f64 * self.scale,
            self.x0 + (i1 + 1) as f64 * self.scale,
            self.y0 + (self.h - 1 - j1) as f64 * self.scale,
            self.y0 + (self.h - j0) as f64 * self.scale,
        ))
    }
}

impl Shape for Bitmap {
    fn inside(&self, x: f64, y: f64) -> bool {
        self.sample(x, y) > 0.5
    }
}

/// Corner treatment of a square section of side `d`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Corner {
    /// Sharp 90-degree corners.
    Sharp,
    /// Corners cut at 45 degrees; each cut removes a leg of length `b` from both sides.
    Chamfered(f64),
    /// Quarter-circle corners of radius `b`.
    Rounded(f64),
    /// One square notch of side `b` removed from each corner.
    Recessed(f64),
    /// Two-step staircase removed from each corner, each step `b` by `b` (total setback 2b
    /// along each face): the "double-notched" plan.
    DoubleRecessed(f64),
}

impl Corner {
    pub fn name(self) -> &'static str {
        match self {
            Corner::Sharp => "square",
            Corner::Chamfered(_) => "chamfered",
            Corner::Rounded(_) => "rounded",
            Corner::Recessed(_) => "recessed",
            Corner::DoubleRecessed(_) => "double-recessed",
        }
    }
}

/// Outline of the top-right corner as offsets from the corner point, walking counter-clockwise
/// (from the right face up and round to the top face). Sizes are in units of the side length.
fn corner_profile(corner: Corner) -> Vec<[f64; 2]> {
    match corner {
        Corner::Sharp | Corner::Rounded(_) => vec![[0.0, 0.0]],
        Corner::Chamfered(b) => vec![[0.0, -b], [-b, 0.0]],
        Corner::Recessed(b) => vec![[0.0, -b], [-b, -b], [-b, 0.0]],
        Corner::DoubleRecessed(b) => {
            vec![[0.0, -2.0 * b], [-b, -2.0 * b], [-b, -b], [-2.0 * b, -b], [-2.0 * b, 0.0]]
        }
    }
}

/// Vertices (counter-clockwise) of a square section of side `d` centred on (cx, cy) with the
/// given corner treatment. For `Rounded` this is the sharp square; use [`section`] instead.
pub fn section_polygon(corner: Corner, cx: f64, cy: f64, d: f64) -> Vec<[f64; 2]> {
    let h = 0.5 * d;
    let prof = corner_profile(corner);
    let mut pts = Vec::new();
    // Rotate the top-right profile by 0, 90, 180, 270 degrees.
    for k in 0..4 {
        for p in &prof {
            let (x, y) = (h + p[0] * d, h + p[1] * d);
            let (rx, ry) = match k {
                0 => (x, y),
                1 => (-y, x),
                2 => (-x, -y),
                _ => (y, -x),
            };
            pts.push([cx + rx, cy + ry]);
        }
    }
    pts
}

/// A square section of side `d` centred on (cx, cy), optionally rotated by `angle` radians.
pub fn section(corner: Corner, cx: f64, cy: f64, d: f64, angle: f64) -> Box<dyn Shape> {
    let base: Box<dyn Shape> = match corner {
        Corner::Rounded(b) => Box::new(RoundedSquare { cx, cy, half: 0.5 * d, r: b * d }),
        _ => Box::new(Polygon { pts: section_polygon(corner, cx, cy, d) }),
    };
    if angle == 0.0 {
        base
    } else {
        Box::new(Rotated { inner: base, cx, cy, angle })
    }
}

/// For a link that starts at a fluid point (x, y) and ends, one step (dx, dy) later, inside
/// the shape: the fraction of the link that lies in the fluid, in (0, 1].
///
/// Bisection to ~1e-12 of a link; a boundary that the link crosses more than once (features
/// thinner than a cell) resolves to one of the crossings.
pub fn cut_fraction(shape: &dyn Shape, x: f64, y: f64, dx: f64, dy: f64) -> f64 {
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    for _ in 0..40 {
        let mid = 0.5 * (lo + hi);
        if shape.inside(x + mid * dx, y + mid * dy) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    (0.5 * (lo + hi)).clamp(1e-9, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(shape: &dyn Shape, x0: f64, x1: f64, n: usize) -> f64 {
        let h = (x1 - x0) / n as f64;
        let mut count = 0usize;
        for j in 0..n {
            for i in 0..n {
                if shape.inside(x0 + (i as f64 + 0.5) * h, x0 + (j as f64 + 0.5) * h) {
                    count += 1;
                }
            }
        }
        count as f64 * h * h
    }

    #[test]
    fn cut_fraction_on_a_circle() {
        let c = Circle { cx: 0.0, cy: 0.0, r: 10.3 };
        // Axis link from (-11, 0) to (-10, 0): wall at x = -10.3, so 70 % of the link is fluid.
        assert!((cut_fraction(&c, -11.0, 0.0, 1.0, 0.0) - 0.7).abs() < 1e-9);
        // Diagonal link from (8, 8) to (7, 7): wall at 10.3/sqrt(2) = 7.2832.
        let q = cut_fraction(&c, 8.0, 8.0, -1.0, -1.0);
        assert!((q - (8.0 - 10.3 / 2f64.sqrt())).abs() < 1e-9);
    }

    #[test]
    fn section_areas_match_geometry() {
        let d = 1.0;
        let cases: [(Corner, f64); 5] = [
            (Corner::Sharp, 1.0),
            (Corner::Chamfered(0.1), 1.0 - 4.0 * 0.005),
            (Corner::Rounded(0.1), 1.0 - (4.0 - std::f64::consts::PI) * 0.01),
            (Corner::Recessed(0.1), 1.0 - 4.0 * 0.01),
            (Corner::DoubleRecessed(0.05), 1.0 - 4.0 * 3.0 * 0.0025),
        ];
        for (corner, expect) in cases {
            let s = section(corner, 0.0, 0.0, d, 0.0);
            let a = area(s.as_ref(), -0.6, 0.6, 1200);
            assert!((a - expect).abs() < 2e-3, "{corner:?}: area {a} vs {expect}");
            // All variants keep the full frontal width: the face centres are still inside.
            assert!(s.inside(0.499, 0.0) && s.inside(0.0, 0.499) && !s.inside(0.501, 0.0));
        }
    }

    #[test]
    fn rotation_by_45_degrees() {
        let s = section(Corner::Sharp, 0.0, 0.0, 1.0, std::f64::consts::FRAC_PI_4);
        assert!(s.inside(0.69, 0.0) && !s.inside(0.72, 0.0)); // half-diagonal = 0.7071
        assert!(!s.inside(0.45, 0.45));
    }

    #[test]
    fn translation_and_bounding_box() {
        let s = Translated { inner: section(Corner::Sharp, 0.0, 0.0, 4.0, 0.0), dx: 1.0, dy: -0.5 };
        assert!(s.inside(2.9, 1.4) && !s.inside(3.1, 0.0) && !s.inside(0.0, 1.6));
        let (x0, x1, y0, y1) = bounding_box(&s, 6.0, 0.05).unwrap();
        assert!((x0 + 1.0).abs() < 0.06 && (x1 - 3.0).abs() < 0.06 && (y0 + 2.5).abs() < 0.06 && (y1 - 1.5).abs() < 0.06);
        assert!(bounding_box(&Circle { cx: 50.0, cy: 0.0, r: 1.0 }, 6.0, 0.1).is_none());
    }

    #[test]
    fn bitmap_outline_is_interpolated() {
        // 4x4 picture, right half solid; one pixel = 2 lattice units.
        let mut level = vec![0.0f32; 16];
        for j in 0..4 {
            level[j * 4 + 2] = 1.0;
            level[j * 4 + 3] = 1.0;
        }
        let b = Bitmap { w: 4, h: 4, level, x0: 10.0, y0: 20.0, scale: 2.0 };
        // The 0.5 contour sits on the pixel edge between columns 1 and 2: x = 10 + 2*2 = 14.
        assert!(!b.inside(13.9, 24.0) && b.inside(14.1, 24.0));
        let q = cut_fraction(&b, 13.0, 24.0, 2.0, 0.0);
        assert!((q - 0.5).abs() < 1e-6);
        assert_eq!(b.bounds(), Some((14.0, 18.0, 20.0, 28.0)));
    }
}
