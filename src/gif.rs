//! Animated GIF (GIF89a) writer with its own LZW coder, plus a reader for the subset the
//! writer produces (full-frame images sharing one global palette), used to test it.

use std::collections::HashMap;

const CLEAR: u16 = 256;
const END: u16 = 257;

struct CodeWriter {
    out: Vec<u8>,
    acc: u32,
    n: u32,
}

impl CodeWriter {
    fn put(&mut self, code: u16, width: u32) {
        self.acc |= (code as u32) << self.n;
        self.n += width;
        while self.n >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }
}

/// GIF-flavoured LZW with 8-bit symbols: variable code width 9..12, clear code when the
/// dictionary is full.
pub fn lzw_encode(pixels: &[u8]) -> Vec<u8> {
    let mut w = CodeWriter { out: Vec::with_capacity(pixels.len() / 2), acc: 0, n: 0 };
    let mut dict: HashMap<u32, u16> = HashMap::with_capacity(8192);
    let (mut next, mut width) = (258u16, 9u32);
    w.put(CLEAR, width);
    let mut iter = pixels.iter();
    let Some(&first) = iter.next() else {
        w.put(END, width);
        if w.n > 0 {
            w.out.push(w.acc as u8);
        }
        return w.out;
    };
    let mut prefix = first as u16;
    for &px in iter {
        let key = (prefix as u32) << 8 | px as u32;
        if let Some(&code) = dict.get(&key) {
            prefix = code;
            continue;
        }
        w.put(prefix, width);
        if next < 4096 {
            dict.insert(key, next);
            next += 1;
            // The decoder runs one entry behind; widen once the newest code no longer fits.
            if next > (1 << width) && width < 12 {
                width += 1;
            }
        } else {
            w.put(CLEAR, width);
            dict.clear();
            next = 258;
            width = 9;
        }
        prefix = px as u16;
    }
    w.put(prefix, width);
    w.put(END, width);
    if w.n > 0 {
        w.out.push(w.acc as u8);
    }
    w.out
}

/// Inverse of [`lzw_encode`]; stops at the end code or after `limit` pixels.
pub fn lzw_decode(data: &[u8], limit: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(limit);
    // Dictionary as (prefix code, last byte, first byte).
    let mut table: Vec<(u16, u8, u8)> = Vec::with_capacity(4096);
    let reset = |t: &mut Vec<(u16, u8, u8)>| {
        t.clear();
        for i in 0..258u16 {
            t.push((u16::MAX, i as u8, i as u8));
        }
    };
    reset(&mut table);
    let (mut width, mut acc, mut nbits, mut pos) = (9u32, 0u32, 0u32, 0usize);
    let mut prev: Option<u16> = None;
    let mut stack = Vec::new();
    loop {
        while nbits < width {
            acc |= (*data.get(pos).ok_or("LZW data ends early")? as u32) << nbits;
            pos += 1;
            nbits += 8;
        }
        let code = (acc & ((1 << width) - 1)) as u16;
        acc >>= width;
        nbits -= width;
        if code == CLEAR {
            reset(&mut table);
            width = 9;
            prev = None;
            continue;
        }
        if code == END {
            return Ok(out);
        }
        // A code equal to the next free slot is the string being defined right now: the
        // previous string followed by its own first byte.
        let fresh = code as usize == table.len();
        if code as usize > table.len() || (fresh && (prev.is_none() || table.len() >= 4096)) {
            return Err("invalid LZW code".into());
        }
        if fresh {
            let p = prev.unwrap();
            table.push((p, table[p as usize].2, table[p as usize].2));
        }
        let mut c = code;
        loop {
            let (p, last, _) = table[c as usize];
            stack.push(last);
            if p == u16::MAX {
                break;
            }
            c = p;
        }
        let first = *stack.last().unwrap();
        while let Some(b) = stack.pop() {
            out.push(b);
        }
        if let (Some(p), false) = (prev, fresh) {
            if table.len() < 4096 {
                table.push((p, first, table[p as usize].2));
            }
        }
        if table.len() == (1 << width) && width < 12 {
            width += 1;
        }
        prev = Some(code);
        if out.len() > limit {
            return Err("LZW data longer than the image".into());
        }
    }
}

pub struct Writer {
    out: Vec<u8>,
    w: usize,
    h: usize,
}

impl Writer {
    /// Starts an endlessly looping animation. The palette is padded to 256 entries.
    pub fn new(w: usize, h: usize, palette: &[[u8; 3]]) -> Writer {
        assert!(w > 0 && h > 0 && w < 65536 && h < 65536 && palette.len() <= 256);
        let mut out = b"GIF89a".to_vec();
        out.extend((w as u16).to_le_bytes());
        out.extend((h as u16).to_le_bytes());
        out.extend([0xf7, 0, 0]); // global palette, 8 bits per channel, 256 entries
        for i in 0..256 {
            out.extend(palette.get(i).copied().unwrap_or([0, 0, 0]));
        }
        out.extend([0x21, 0xff, 11]);
        out.extend(b"NETSCAPE2.0");
        out.extend([3, 1, 0, 0, 0]); // loop count 0 = forever
        Writer { out, w, h }
    }

    /// Appends a frame of palette indices shown for `delay_cs` hundredths of a second.
    pub fn frame(&mut self, pixels: &[u8], delay_cs: u16) {
        assert_eq!(pixels.len(), self.w * self.h);
        self.out.extend([0x21, 0xf9, 4, 0x04]); // graphic control: keep frame in place
        self.out.extend(delay_cs.to_le_bytes());
        self.out.extend([0, 0]);
        self.out.extend([0x2c, 0, 0, 0, 0]);
        self.out.extend((self.w as u16).to_le_bytes());
        self.out.extend((self.h as u16).to_le_bytes());
        self.out.extend([0, 8]); // no local palette; LZW minimum code size 8
        for block in lzw_encode(pixels).chunks(255) {
            self.out.push(block.len() as u8);
            self.out.extend(block);
        }
        self.out.push(0);
    }

    pub fn finish(mut self) -> Vec<u8> {
        self.out.push(0x3b);
        self.out
    }
}

#[derive(Debug, PartialEq)]
pub struct Animation {
    pub w: usize,
    pub h: usize,
    pub palette: Vec<[u8; 3]>,
    pub frames: Vec<Vec<u8>>,
    pub delays_cs: Vec<u16>,
}

/// Reads back a file made by [`Writer`].
pub fn decode(data: &[u8]) -> Result<Animation, String> {
    if data.len() < 13 + 768 || &data[..6] != b"GIF89a" || data[10] != 0xf7 {
        return Err("not a GIF written by this program".into());
    }
    let w = u16::from_le_bytes([data[6], data[7]]) as usize;
    let h = u16::from_le_bytes([data[8], data[9]]) as usize;
    let palette = data[13..13 + 768].chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
    let mut anim = Animation { w, h, palette, frames: Vec::new(), delays_cs: Vec::new() };
    let mut pos = 13 + 768;
    let sub_blocks = |pos: &mut usize| -> Result<Vec<u8>, String> {
        let mut v = Vec::new();
        loop {
            let n = *data.get(*pos).ok_or("truncated GIF")? as usize;
            *pos += 1;
            if n == 0 {
                return Ok(v);
            }
            v.extend_from_slice(data.get(*pos..*pos + n).ok_or("truncated GIF")?);
            *pos += n;
        }
    };
    loop {
        match *data.get(pos).ok_or("truncated GIF")? {
            0x3b => return Ok(anim),
            0x21 => {
                let label = *data.get(pos + 1).ok_or("truncated GIF")?;
                pos += 2;
                let body = sub_blocks(&mut pos)?;
                if label == 0xf9 && body.len() == 4 {
                    anim.delays_cs.push(u16::from_le_bytes([body[1], body[2]]));
                }
            }
            0x2c => {
                let d = data.get(pos + 1..pos + 11).ok_or("truncated GIF")?;
                if d[..4] != [0, 0, 0, 0] || u16::from_le_bytes([d[4], d[5]]) as usize != w || u16::from_le_bytes([d[6], d[7]]) as usize != h || d[8] != 0 || d[9] != 8 {
                    return Err("unsupported GIF frame layout".into());
                }
                pos += 11;
                let lzw = sub_blocks(&mut pos)?;
                let px = lzw_decode(&lzw, w * h)?;
                if px.len() != w * h {
                    return Err("GIF frame has the wrong number of pixels".into());
                }
                anim.frames.push(px);
            }
            _ => return Err("unknown GIF block".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(n: usize, seed: u64, modulus: u32) -> Vec<u8> {
        let mut s = seed;
        (0..n)
            .map(|_| {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                ((s >> 33) as u32 % modulus) as u8
            })
            .collect()
    }

    #[test]
    fn lzw_round_trip() {
        let mut cases: Vec<Vec<u8>> = vec![vec![], vec![5], vec![9; 3], vec![0; 70_000], (0..=255).collect()];
        cases.push(noise(100_000, 1, 256)); // fills the dictionary several times
        cases.push(noise(100_000, 2, 2));
        cases.push(noise(30_000, 3, 7));
        cases.push(b"abababababababababababababab".to_vec()); // exercises the KwKwK case
        for (k, c) in cases.iter().enumerate() {
            let z = lzw_encode(c);
            assert_eq!(&lzw_decode(&z, c.len()).unwrap(), c, "case {k}");
        }
        assert!(lzw_encode(&vec![0; 70_000]).len() < 1000);
    }

    #[test]
    fn animation_round_trip() {
        let (w, h) = (31, 17);
        let palette: Vec<[u8; 3]> = (0..100u8).map(|i| [i, 2 * i, 255 - i]).collect();
        let frames: Vec<Vec<u8>> = (0..5).map(|k| noise(w * h, 10 + k, 100)).collect();
        let mut g = Writer::new(w, h, &palette);
        for (k, f) in frames.iter().enumerate() {
            g.frame(f, 4 + k as u16);
        }
        let bytes = g.finish();
        let a = decode(&bytes).unwrap();
        assert_eq!((a.w, a.h), (w, h));
        assert_eq!(a.frames, frames);
        assert_eq!(a.delays_cs, vec![4, 5, 6, 7, 8]);
        assert_eq!(&a.palette[..100], &palette[..]);
        assert!(decode(&bytes[..bytes.len() / 2]).is_err());
    }

    #[test]
    fn decoder_survives_garbage() {
        let mut seed = 5u64;
        for _ in 0..2000 {
            seed += 1;
            let n = 1 + noise(1, seed, 80)[0] as usize;
            let _ = lzw_decode(&noise(n, seed * 3, 256), 10_000);
        }
    }
}
