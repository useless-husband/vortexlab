//! Time-series analysis: FFT, dominant frequency, peaks, basic statistics.

use std::f64::consts::PI;

/// In-place radix-2 Cooley-Tukey FFT (forward, e^{-i...}); length must be a power of two.
pub fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    assert!(n.is_power_of_two() && im.len() == n, "FFT length must be a power of two");
    // Bit-reversal permutation.
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * PI / len as f64;
        let (wr, wi) = (ang.cos(), ang.sin());
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0, 0.0);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let (tr, ti) = (re[b] * cr - im[b] * ci, re[b] * ci + im[b] * cr);
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                let ncr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = ncr;
            }
        }
        len <<= 1;
    }
}

/// O(n^2) discrete Fourier transform, any length; the reference the FFT is tested against.
pub fn dft_naive(re: &[f64], im: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let n = re.len();
    let mut or = vec![0.0; n];
    let mut oi = vec![0.0; n];
    for k in 0..n {
        for t in 0..n {
            let ang = -2.0 * PI * (k * t % n) as f64 / n as f64;
            let (c, s) = (ang.cos(), ang.sin());
            or[k] += re[t] * c - im[t] * s;
            oi[k] += re[t] * s + im[t] * c;
        }
    }
    (or, oi)
}

pub fn mean(x: &[f64]) -> f64 {
    x.iter().sum::<f64>() / x.len() as f64
}

/// Root-mean-square of the fluctuation about the mean.
pub fn rms(x: &[f64]) -> f64 {
    let m = mean(x);
    (x.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / x.len() as f64).sqrt()
}

/// One-sided amplitude spectrum of a real signal sampled every `dt`: Hann window, mean
/// removed, zero-padded to at least `pad` times its length. Returns (frequency, amplitude)
/// with the amplitude scaled so that a pure sine of amplitude A peaks at about A.
pub fn spectrum(x: &[f64], dt: f64, pad: usize) -> Vec<(f64, f64)> {
    let n = x.len();
    let m = mean(x);
    let size = (n * pad.max(1)).next_power_of_two();
    let mut re = vec![0.0; size];
    let mut im = vec![0.0; size];
    for i in 0..n {
        let w = 0.5 - 0.5 * (2.0 * PI * i as f64 / n as f64).cos();
        re[i] = (x[i] - m) * w;
    }
    fft(&mut re, &mut im);
    // Hann coherent gain is 1/2, and half the energy sits in the negative frequencies.
    let scale = 4.0 / n as f64;
    (0..size / 2).map(|k| (k as f64 / (size as f64 * dt), scale * (re[k] * re[k] + im[k] * im[k]).sqrt())).collect()
}

/// Frequency and amplitude of the strongest spectral line (parabolic interpolation of the
/// log-amplitude around the highest bin of the padded spectrum).
pub fn dominant_frequency(x: &[f64], dt: f64) -> (f64, f64) {
    let s = spectrum(x, dt, 8);
    let mut k = 1;
    for i in 1..s.len() - 1 {
        if s[i].1 > s[k].1 {
            k = i;
        }
    }
    if k == 0 || k + 1 >= s.len() || s[k].1 <= 0.0 || s[k - 1].1 <= 0.0 || s[k + 1].1 <= 0.0 {
        return (s[k].0, s[k].1);
    }
    let (a, b, c) = (s[k - 1].1.ln(), s[k].1.ln(), s[k + 1].1.ln());
    let shift = 0.5 * (a - c) / (a - 2.0 * b + c);
    let df = s[1].0 - s[0].0;
    (s[k].0 + shift * df, (b - 0.25 * (a - c) * shift).exp())
}

/// Local maxima above the signal mean, as (fractional sample index, value), each refined by
/// a parabola through the three samples around it.
pub fn peaks(x: &[f64]) -> Vec<(f64, f64)> {
    let m = mean(x);
    let mut out = Vec::new();
    for i in 1..x.len().saturating_sub(1) {
        if x[i] > m && x[i] > x[i - 1] && x[i] >= x[i + 1] {
            let (a, b, c) = (x[i - 1], x[i], x[i + 1]);
            let denom = a - 2.0 * b + c;
            let shift = if denom != 0.0 { 0.5 * (a - c) / denom } else { 0.0 };
            out.push((i as f64 + shift, b - 0.25 * (a - c) * shift));
        }
    }
    out
}

/// Frequency from the mean spacing of upward crossings of the signal mean: an estimate that
/// shares no code with the FFT, used as a cross-check. `None` with fewer than two crossings.
pub fn crossing_frequency(x: &[f64], dt: f64) -> Option<f64> {
    let m = mean(x);
    let mut times = Vec::new();
    for i in 1..x.len() {
        let (a, b) = (x[i - 1] - m, x[i] - m);
        if a < 0.0 && b >= 0.0 {
            times.push((i - 1) as f64 + a / (a - b));
        }
    }
    if times.len() < 2 {
        return None;
    }
    Some((times.len() - 1) as f64 / ((times[times.len() - 1] - times[0]) * dt))
}

/// Linear interpolation of a sampled signal at a fractional index.
pub fn sample_at(x: &[f64], pos: f64) -> f64 {
    let i = (pos.floor() as usize).min(x.len() - 2);
    let f = pos - i as f64;
    x[i] * (1.0 - f) + x[i + 1] * f
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(seed: &mut u64) -> f64 {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (*seed >> 11) as f64 / (1u64 << 53) as f64 - 0.5
    }

    #[test]
    fn fft_matches_naive_dft() {
        let mut seed = 2024u64;
        for n in [1usize, 2, 4, 8, 64, 256] {
            let re: Vec<f64> = (0..n).map(|_| lcg(&mut seed)).collect();
            let im: Vec<f64> = (0..n).map(|_| lcg(&mut seed)).collect();
            let (er, ei) = dft_naive(&re, &im);
            let (mut fr, mut fi) = (re.clone(), im.clone());
            fft(&mut fr, &mut fi);
            for k in 0..n {
                assert!((fr[k] - er[k]).abs() < 1e-10 && (fi[k] - ei[k]).abs() < 1e-10, "n={n} k={k}");
            }
        }
    }

    #[test]
    fn fft_parseval_and_impulse() {
        let n = 128;
        let mut re = vec![0.0; n];
        let mut im = vec![0.0; n];
        re[3] = 1.0;
        fft(&mut re, &mut im);
        for k in 0..n {
            assert!((re[k] * re[k] + im[k] * im[k] - 1.0).abs() < 1e-12);
        }
    }

    #[test]
    fn dominant_frequency_between_bins() {
        // 13.37 periods in the window: far from any FFT bin of the unpadded signal.
        let n = 3000;
        let dt = 0.01;
        let f0 = 13.37 / (n as f64 * dt);
        let x: Vec<f64> = (0..n).map(|i| 2.5 + 0.8 * (2.0 * PI * f0 * i as f64 * dt + 0.4).sin()).collect();
        let (f, amp) = dominant_frequency(&x, dt);
        assert!((f / f0 - 1.0).abs() < 2e-4, "f = {f}, expected {f0}");
        assert!((amp - 0.8).abs() < 0.01, "amplitude {amp}");
        let fc = crossing_frequency(&x, dt).unwrap();
        assert!((fc / f0 - 1.0).abs() < 1e-3);
    }

    #[test]
    fn peaks_and_statistics() {
        let n = 2000;
        let x: Vec<f64> = (0..n).map(|i| 1.0 + 0.5 * (2.0 * PI * i as f64 / 400.0).sin()).collect();
        let p = peaks(&x);
        assert_eq!(p.len(), 5);
        for (k, (pos, val)) in p.iter().enumerate() {
            assert!((pos - (100.0 + 400.0 * k as f64)).abs() < 1e-6);
            assert!((val - 1.5).abs() < 1e-9);
        }
        assert!((mean(&x) - 1.0).abs() < 1e-12);
        assert!((rms(&x) - 0.5 / 2f64.sqrt()).abs() < 1e-12);
        assert!((sample_at(&x, 100.5) - 0.5 * (x[100] + x[101])).abs() < 1e-15);
    }
}
