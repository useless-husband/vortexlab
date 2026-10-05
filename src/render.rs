//! Turning fields into pictures: the colour map and the vorticity renderer.
//!
//! Vorticity is signed, so it gets a diverging map: blue for clockwise (negative), red for
//! counter-clockwise (positive), neutral light grey at zero. Both arms are built in the Oklab
//! colour space with the *same* lightness at equal |value|, so equal strengths look equally
//! strong whichever the sign, and the map still reads correctly in greyscale as "how strong".

use crate::sim::Sim;

/// Number of colour levels in the vorticity palette (odd: the middle one is zero).
pub const LEVELS: usize = 201;
/// Palette index used for solid cells.
pub const BODY: u8 = LEVELS as u8;
const BODY_COLOR: [u8; 3] = [0x2b, 0x2b, 0x29];

fn to_linear(c: u8) -> f64 {
    let v = c as f64 / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn from_linear(v: f64) -> f64 {
    if v <= 0.0031308 {
        12.92 * v
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// sRGB -> Oklab (Bjorn Ottosson's 2020 definition).
pub fn srgb_to_oklab(c: [u8; 3]) -> [f64; 3] {
    let (r, g, b) = (to_linear(c[0]), to_linear(c[1]), to_linear(c[2]));
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    [
        0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    ]
}

/// Oklab -> sRGB components (may fall outside [0, 1] for colours sRGB cannot show).
fn oklab_to_srgb_f(c: [f64; 3]) -> [f64; 3] {
    let l = (c[0] + 0.3963377774 * c[1] + 0.2158037573 * c[2]).powi(3);
    let m = (c[0] - 0.1055613458 * c[1] - 0.0638541728 * c[2]).powi(3);
    let s = (c[0] - 0.0894841775 * c[1] - 1.2914855480 * c[2]).powi(3);
    [
        from_linear(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
        from_linear(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
        from_linear(-0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s),
    ]
}

/// Oklab -> 8-bit sRGB, reducing the chroma until the colour is displayable.
pub fn oklab_to_srgb(mut c: [f64; 3]) -> [u8; 3] {
    for _ in 0..200 {
        let rgb = oklab_to_srgb_f(c);
        if rgb.iter().all(|v| (-0.0005..=1.0005).contains(v)) {
            return [0, 1, 2].map(|i| (rgb[i].clamp(0.0, 1.0) * 255.0).round() as u8);
        }
        c[1] *= 0.97;
        c[2] *= 0.97;
    }
    let g = (from_linear(c[0].powi(3)).clamp(0.0, 1.0) * 255.0).round() as u8;
    [g, g, g]
}

fn lerp3(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
}

/// Colour for a signed value in [-1, 1].
pub fn diverging(v: f64) -> [u8; 3] {
    // Blue arm anchors, from zero outwards (neutral grey, then one blue hue darkening).
    const ANCHORS: [(f64, [u8; 3]); 4] = [(0.0, [0xf0, 0xef, 0xec]), (0.35, [0x9e, 0xc5, 0xf4]), (0.7, [0x2a, 0x78, 0xd6]), (1.0, [0x0d, 0x36, 0x6b])];
    const RED: [u8; 3] = [0xd0, 0x3b, 0x3b];
    let t = v.abs().min(1.0);
    let k = ANCHORS.iter().rposition(|a| a.0 <= t).unwrap().min(ANCHORS.len() - 2);
    let (a, b) = (ANCHORS[k], ANCHORS[k + 1]);
    let blue = lerp3(srgb_to_oklab(a.1), srgb_to_oklab(b.1), (t - a.0) / (b.0 - a.0));
    if v <= 0.0 {
        return oklab_to_srgb(blue);
    }
    // Red arm: same lightness and chroma as the blue arm, hue of the red pole. The chroma
    // of the neutral midpoint is kept as is so both arms meet in exactly the same grey.
    let mid = srgb_to_oklab(ANCHORS[0].1);
    let red = srgb_to_oklab(RED);
    let hue = red[2].atan2(red[1]);
    let chroma = ((blue[1] - mid[1]).powi(2) + (blue[2] - mid[2]).powi(2)).sqrt();
    oklab_to_srgb([blue[0], mid[1] + chroma * hue.cos(), mid[2] + chroma * hue.sin()])
}

/// The palette used by all vorticity pictures: `LEVELS` map colours, then the body colour.
pub fn palette() -> Vec<[u8; 3]> {
    let half = (LEVELS - 1) as f64 / 2.0;
    let mut p: Vec<[u8; 3]> = (0..LEVELS).map(|i| diverging((i as f64 - half) / half)).collect();
    p.push(BODY_COLOR);
    p
}

/// Palette index for a value normalised to [-1, 1].
pub fn level(v: f64) -> u8 {
    let half = (LEVELS - 1) as f64 / 2.0;
    (v.clamp(-1.0, 1.0) * half + half).round() as u8
}

/// Which part of the grid to draw and how large.
#[derive(Clone, Copy, Debug)]
pub struct View {
    /// Cell range [x0, x1) x [y0, y1).
    pub x0: usize,
    pub x1: usize,
    pub y0: usize,
    pub y1: usize,
    /// Output pixels per cell (below 1 shrinks the picture).
    pub zoom: f64,
}

impl View {
    pub fn size(&self) -> (usize, usize) {
        ((((self.x1 - self.x0) as f64) * self.zoom).round().max(1.0) as usize, (((self.y1 - self.y0) as f64) * self.zoom).round().max(1.0) as usize)
    }
}

/// Vorticity as palette indices (row 0 at the top). `full_scale` is the vorticity (lattice
/// units, 1/step) drawn in the darkest colour; stronger values are clamped to it.
pub fn vorticity_frame(sim: &Sim, view: View, full_scale: f64) -> Vec<u8> {
    let field = sim.vorticity();
    let (w, h) = view.size();
    let mut out = vec![0u8; w * h];
    let nx = sim.nx;
    for py in 0..h {
        // Pixel centre in lattice coordinates; image rows run downwards.
        let y = view.y1 as f64 - (py as f64 + 0.5) / view.zoom - 0.5;
        for px in 0..w {
            let x = view.x0 as f64 + (px as f64 + 0.5) / view.zoom - 0.5;
            let (xn, yn) = ((x.round() as usize).min(sim.nx - 1), (y.round().max(0.0) as usize).min(sim.ny - 1));
            out[py * w + px] = if !sim.is_fluid(xn, yn) {
                BODY
            } else {
                // Bilinear sample of the node-centred field.
                let (i, j) = ((x.floor().max(0.0) as usize).min(sim.nx - 2), (y.floor().max(0.0) as usize).min(sim.ny - 2));
                let (fx, fy) = ((x - i as f64).clamp(0.0, 1.0), (y - j as f64).clamp(0.0, 1.0));
                let f = |a: usize, b: usize| field[b * nx + a] as f64;
                let v = (1.0 - fx) * (1.0 - fy) * f(i, j) + fx * (1.0 - fy) * f(i + 1, j) + (1.0 - fx) * fy * f(i, j + 1) + fx * fy * f(i + 1, j + 1);
                level(v / full_scale)
            };
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oklab_round_trip() {
        for c in [[0u8, 0, 0], [255, 255, 255], [0x2a, 0x78, 0xd6], [0xd0, 0x3b, 0x3b], [12, 200, 99]] {
            let back = oklab_to_srgb(srgb_to_oklab(c));
            for i in 0..3 {
                assert!((back[i] as i32 - c[i] as i32).abs() <= 1, "{c:?} -> {back:?}");
            }
        }
        // White has lightness 1 and no chroma.
        let w = srgb_to_oklab([255, 255, 255]);
        assert!((w[0] - 1.0).abs() < 1e-3 && w[1].abs() < 1e-3 && w[2].abs() < 1e-3);
    }

    #[test]
    fn diverging_map_is_perceptually_ordered() {
        let l = |v: f64| srgb_to_oklab(diverging(v))[0];
        // Lightness falls monotonically away from zero on both arms...
        for k in 0..100 {
            let (a, b) = (k as f64 / 100.0, (k + 1) as f64 / 100.0);
            assert!(l(b) < l(a) + 2e-3, "red arm not monotone at {a}");
            assert!(l(-b) < l(-a) + 2e-3, "blue arm not monotone at {a}");
            // ...and is the same for +v and -v (8-bit rounding and gamut clipping allowed for).
            assert!((l(b) - l(-b)).abs() < 0.03, "arms differ in lightness at {b}");
        }
        assert_eq!(diverging(0.0), [0xf0, 0xef, 0xec]);
        // The poles are clearly blue and clearly red.
        let (neg, pos) = (diverging(-0.7), diverging(0.7));
        assert!(neg[2] > neg[0] + 60 && pos[0] > pos[2] + 60);
        assert!(l(1.0) < 0.45 && l(0.0) > 0.9);
    }

    #[test]
    fn palette_layout() {
        let p = palette();
        assert_eq!(p.len(), LEVELS + 1);
        assert_eq!(p[level(0.0) as usize], [0xf0, 0xef, 0xec]);
        assert_eq!(level(-5.0), 0);
        assert_eq!(level(5.0) as usize, LEVELS - 1);
        assert_eq!(p[BODY as usize], BODY_COLOR);
    }
}
