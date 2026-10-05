//! The PNG decoder against files written by an independent encoder (Python's zlib; see
//! tools/make_png_fixtures.py), and the encoders against the decoders on real renderings.

#![allow(clippy::needless_range_loop)]

use vortexlab::{gif, png, render};

fn load(name: &str) -> png::Image {
    let path = format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"));
    png::decode(&std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap_or_else(|e| panic!("{name}: {e}"))
}

const W: usize = 33;
const H: usize = 21;

#[test]
fn decodes_rgb_and_rgba() {
    let img = load("rgb8.png");
    assert_eq!((img.w, img.h), (W, H));
    for y in 0..H {
        for x in 0..W {
            let p = &img.rgba[4 * (y * W + x)..][..4];
            let e = |c: usize| ((x * 7 + y * 13 + c * 29) % 256) as u8;
            assert_eq!(p, [e(0), e(1), e(2), 255], "rgb8 at {x},{y}");
        }
    }
    let img = load("rgba8.png");
    for y in 0..H {
        for x in 0..W {
            let p = &img.rgba[4 * (y * W + x)..][..4];
            let e = |c: usize| ((x * 5 + y * 11 + c * 31) % 256) as u8;
            assert_eq!(p, [e(0), e(1), e(2), e(3)], "rgba8 at {x},{y}");
        }
    }
}

#[test]
fn decodes_grey_variants() {
    let img = load("graya8.png");
    for y in 0..H {
        for x in 0..W {
            let (g, a) = (((x * 3 + y * 17) % 256) as u8, ((x * 3 + y * 17 + 101) % 256) as u8);
            assert_eq!(&img.rgba[4 * (y * W + x)..][..4], [g, g, g, a]);
        }
    }
    let img = load("gray16.png");
    for y in 0..H {
        for x in 0..W {
            let g = (((x * 1021 + y * 4099) % 65536) >> 8) as u8; // high byte of the 16-bit sample
            assert_eq!(&img.rgba[4 * (y * W + x)..][..4], [g, g, g, 255]);
        }
    }
    let img = load("gray1.png");
    assert_eq!((img.w, img.h), (19, H));
    for y in 0..H {
        for x in 0..19 {
            let g = if (x * x + y) % 3 == 0 { 255 } else { 0 };
            assert_eq!(&img.rgba[4 * (y * 19 + x)..][..4], [g, g, g, 255], "gray1 at {x},{y}");
        }
    }
}

#[test]
fn decodes_palette_with_transparency() {
    let img = load("palette4.png");
    for y in 0..H {
        for x in 0..W {
            let i = (x + 2 * y) % 16;
            let alpha = if i < 10 { ((i * 17) % 256) as u8 } else { 255 };
            assert_eq!(&img.rgba[4 * (y * W + x)..][..4], [(i * 16) as u8, (255 - i * 16) as u8, ((i * 37) % 256) as u8, alpha]);
        }
    }
}

#[test]
fn decodes_dynamic_huffman_with_long_matches() {
    let img = load("smooth.png");
    assert_eq!((img.w, img.h), (200, 120));
    for y in 0..120 {
        for x in 0..200 {
            assert_eq!(img.rgba[4 * (y * 200 + x)], ((x * x / 97 + y * 3) % 256) as u8);
        }
    }
    // Re-encode with this crate's encoder and decode again.
    let gray: Vec<u8> = img.rgba.chunks(4).map(|p| p[0]).collect();
    let again = png::decode(&png::encode(200, 120, png::Pixels::Gray(&gray))).unwrap();
    assert_eq!(again, img);
}

#[test]
fn silhouette_survives_a_png_round_trip() {
    // Draw a disc, save it as PNG, read it back, and check the body mask.
    let (w, h) = (64, 48);
    let gray: Vec<u8> = (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) as f64 - 30.0, (i / w) as f64 - 20.0);
            if x * x + y * y < 15.0 * 15.0 {
                0
            } else {
                255
            }
        })
        .collect();
    let img = png::decode(&png::encode(w, h, png::Pixels::Gray(&gray))).unwrap();
    let s = img.silhouette();
    let area = s.iter().filter(|&&v| v > 0.5).count() as f64;
    assert!((area / (std::f64::consts::PI * 225.0) - 1.0).abs() < 0.02);
    assert!(s[20 * w + 30] > 0.99 && s[0] < 0.01);
}

#[test]
fn vorticity_palette_animation_round_trip() {
    // Frames with the real palette and smooth content, like the pictures the program writes.
    let (w, h) = (120, 50);
    let palette = render::palette();
    let frames: Vec<Vec<u8>> = (0..6)
        .map(|k| (0..w * h).map(|i| render::level((((i % w) as f64 * 0.11 + k as f64).sin()) * (((i / w) as f64) * 0.2).cos())).collect())
        .collect();
    let mut g = gif::Writer::new(w, h, &palette);
    for f in &frames {
        g.frame(f, 7);
    }
    let bytes = g.finish();
    let back = gif::decode(&bytes).unwrap();
    assert_eq!(back.frames, frames);
    assert_eq!(&back.palette[..palette.len()], &palette[..]);
    // The same frame as an indexed PNG.
    let img = png::decode(&png::encode(w, h, png::Pixels::Indexed(&frames[0], &palette))).unwrap();
    for (i, &idx) in frames[0].iter().enumerate() {
        assert_eq!(img.rgba[4 * i..4 * i + 3], palette[idx as usize]);
    }
}
