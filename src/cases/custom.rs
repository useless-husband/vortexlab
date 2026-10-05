//! "Bring your own shape": place a silhouette read from a picture into the towing tank.

use crate::cases::tunnel::Tunnel;
use crate::geometry::{bounding_box, Bitmap, Rotated, Shape, Translated};
use crate::lattice::Collision;
use crate::png::Image;

/// Builds the tunnel and the body for a picture.
///
/// * `cells`: lattice cells across the longer side of the shape's bounding box (before
///   rotation); this sets the resolution.
/// * `angle_deg`: rotation of the shape, counter-clockwise (the flow comes from the left).
///
/// The reference length D of the run is the frontal height of the (rotated) shape, the
/// channel is 10 D high (10 % blockage), with 5 D of open tunnel ahead of the nose and 14 D
/// behind the tail.
pub fn place(
    image: &Image,
    cells: usize,
    angle_deg: f64,
    re: f64,
    u: f64,
    collision: Collision,
    threads: usize,
) -> Result<(Tunnel, Box<dyn Shape>), String> {
    let mut bitmap = Bitmap { w: image.w, h: image.h, level: image.silhouette(), x0: 0.0, y0: 0.0, scale: 1.0 };
    let (xa, xb, ya, yb) =
        bitmap.bounds().ok_or("the picture contains no dark shape (the body must be dark on a light or transparent background)")?;
    let longest = (xb - xa).max(yb - ya);
    if longest < 4.0 {
        return Err("the shape is only a few pixels across; draw it larger".into());
    }
    // Scale so the longer side spans `cells`, and centre the bounding box on the origin.
    let s = cells as f64 / longest;
    bitmap.scale = s;
    bitmap.x0 = -0.5 * (xa + xb) * s;
    bitmap.y0 = -0.5 * (ya + yb) * s;
    let rotated = Rotated { inner: Box::new(bitmap), cx: 0.0, cy: 0.0, angle: angle_deg.to_radians() };
    let (x0, x1, y0, y1) = bounding_box(&rotated, cells as f64, 0.25).ok_or("the shape vanished at this resolution; use more cells")?;
    let d = (y1 - y0).round() as usize;
    if d < 8 {
        return Err(format!(
            "the shape is only {d} cells thick across the flow at this resolution; use more cells (--cells) or turn it (--angle)"
        ));
    }
    let length = (x1 - x0) / d as f64;
    let tunnel = Tunnel {
        d,
        re,
        u,
        upstream: 5.0 + 0.5 * length,
        downstream: 14.0 + 0.5 * length,
        height: 10.0,
        length,
        collision,
        threads,
        open_sides: true,
    };
    let (nx, ny) = tunnel.grid();
    if nx * ny > 6_000_000 {
        return Err(format!("the grid would be {nx} x {ny} cells, too large; use fewer cells (--cells)"));
    }
    let (cx, cy) = tunnel.centre();
    let body = Translated { inner: Box::new(rotated), dx: cx - 0.5 * (x0 + x1), dy: cy - 0.5 * (y0 + y1) };
    Ok((tunnel, Box::new(body)))
}

/// Anti-aliased rendering of a shape into an 8-bit greyscale picture (dark body on white),
/// `px` pixels per unit of the shape's coordinates, covering [-w/2, w/2] x [-h/2, h/2].
pub fn rasterize(shape: &dyn Shape, w: usize, h: usize, px: f64) -> Vec<u8> {
    const SUB: usize = 4;
    let mut out = vec![255u8; w * h];
    for j in 0..h {
        for i in 0..w {
            let mut hits = 0;
            for sj in 0..SUB {
                for si in 0..SUB {
                    let x = (i as f64 + (si as f64 + 0.5) / SUB as f64 - 0.5 * w as f64) / px;
                    let y = (0.5 * h as f64 - j as f64 - (sj as f64 + 0.5) / SUB as f64) / px;
                    hits += shape.inside(x, y) as usize;
                }
            }
            out[j * w + i] = (255 - 255 * hits / (SUB * SUB)) as u8;
        }
    }
    out
}

/// Outline of a symmetric NACA four-digit aerofoil of unit chord (nose at x = -0.5), with
/// thickness `t` as a fraction of the chord, as a polygon.
pub fn naca_symmetric(t: f64, points: usize) -> Vec<[f64; 2]> {
    let half = |x: f64| 5.0 * t * (0.2969 * x.sqrt() - 0.1260 * x - 0.3516 * x * x + 0.2843 * x.powi(3) - 0.1036 * x.powi(4));
    let xs: Vec<f64> = (0..=points).map(|k| 0.5 * (1.0 - (std::f64::consts::PI * k as f64 / points as f64).cos())).collect();
    let mut pts: Vec<[f64; 2]> = xs.iter().map(|&x| [x - 0.5, half(x)]).collect();
    pts.extend(xs.iter().rev().skip(1).take(points - 1).map(|&x| [x - 0.5, -half(x)]));
    pts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{section, Corner, Polygon};

    fn picture(shape: &dyn Shape, w: usize, h: usize, px: f64) -> Image {
        let gray = rasterize(shape, w, h, px);
        Image { w, h, rgba: gray.iter().flat_map(|&g| [g, g, g, 255]).collect() }
    }

    #[test]
    fn square_picture_becomes_a_square_body() {
        // A 100-pixel square in a 160 x 120 picture, off-centre.
        let sq = Translated { inner: section(Corner::Sharp, 0.0, 0.0, 100.0, 0.0), dx: 12.0, dy: -3.0 };
        let img = picture(&sq, 160, 120, 1.0);
        let (tunnel, body) = place(&img, 40, 0.0, 100.0, 0.05, Collision::Bgk, 1).unwrap();
        assert_eq!(tunnel.d, 40);
        assert!((tunnel.length - 1.0).abs() < 0.03);
        let (cx, cy) = tunnel.centre();
        // The body is a 40-cell square centred on the tunnel centre, whatever its place in the picture.
        assert!(body.inside(cx + 19.0, cy + 19.0) && body.inside(cx - 19.0, cy - 19.0));
        assert!(!body.inside(cx + 21.0, cy) && !body.inside(cx, cy - 21.0));
    }

    #[test]
    fn rotation_changes_the_frontal_height() {
        // A 4:1 plate: flat it is 10 cells thick, at 90 degrees it is 40.
        let plate = Polygon { pts: vec![[-80.0, -20.0], [80.0, -20.0], [80.0, 20.0], [-80.0, 20.0]] };
        let img = picture(&plate, 200, 60, 1.0);
        let (flat, _) = place(&img, 40, 0.0, 100.0, 0.05, Collision::Bgk, 1).unwrap();
        assert_eq!(flat.d, 10);
        assert!((flat.length - 4.0).abs() < 0.15);
        let (up, _) = place(&img, 40, 90.0, 100.0, 0.05, Collision::Bgk, 1).unwrap();
        assert_eq!(up.d, 40);
        assert!((up.length - 0.25).abs() < 0.03);
    }

    #[test]
    fn unusable_pictures_are_rejected_with_a_reason() {
        let blank = Image { w: 50, h: 50, rgba: vec![255; 50 * 50 * 4] };
        assert!(place(&blank, 40, 0.0, 100.0, 0.05, Collision::Bgk, 1).err().unwrap().contains("no dark shape"));
        let sliver = Polygon { pts: vec![[-90.0, -2.0], [90.0, -2.0], [90.0, 2.0], [-90.0, 2.0]] };
        let img = picture(&sliver, 200, 40, 1.0);
        assert!(place(&img, 40, 0.0, 100.0, 0.05, Collision::Bgk, 1).err().unwrap().contains("cells thick"));
    }

    #[test]
    fn naca_outline_is_closed_and_has_the_right_thickness() {
        let pts = naca_symmetric(0.12, 60);
        assert_eq!(pts.len(), 120);
        let max_y = pts.iter().map(|p| p[1]).fold(0.0, f64::max);
        assert!((2.0 * max_y - 0.12).abs() < 1e-3);
        let poly = Polygon { pts };
        assert!(poly.inside(-0.2, 0.0) && !poly.inside(-0.2, 0.07) && !poly.inside(0.51, 0.0));
    }
}
