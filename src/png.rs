//! A minimal PNG codec: enough to read a silhouette drawn in any ordinary paint program and
//! to write the pictures this program produces. Includes its own DEFLATE (RFC 1951) inflater
//! (stored, fixed and dynamic Huffman blocks) and a small compressor (LZ77 + fixed Huffman).
//!
//! Reading: bit depths 1-16, all five colour types, all five row filters; not interlaced.
//! Writing: 8-bit greyscale, RGB or palette images.

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

const LEN_BASE: [u16; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LEN_EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DIST_BASE: [u16; 30] =
    [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
const DIST_EXTRA: [u8; 30] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

pub fn crc32(chunks: &[&[u8]]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for data in chunks {
        for &b in *data {
            crc ^= b as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
            }
        }
    }
    !crc
}

pub fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5000) {
        for &x in chunk {
            a += x as u32;
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

// ---------------------------------------------------------------- inflate

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    bit: u32,
}

impl Bits<'_> {
    fn bit(&mut self) -> Result<u32, String> {
        let byte = *self.data.get(self.pos).ok_or("deflate stream ends early")?;
        let v = (byte >> self.bit) & 1;
        self.bit += 1;
        if self.bit == 8 {
            self.bit = 0;
            self.pos += 1;
        }
        Ok(v as u32)
    }
    fn bits(&mut self, n: u32) -> Result<u32, String> {
        let mut v = 0;
        for i in 0..n {
            v |= self.bit()? << i;
        }
        Ok(v)
    }
}

/// Canonical Huffman code, decoded one bit at a time (simple rather than fast).
struct Huffman {
    count: [u16; 16],
    symbol: Vec<u16>,
}

impl Huffman {
    fn new(lengths: &[u8]) -> Result<Huffman, String> {
        let mut count = [0u16; 16];
        for &l in lengths {
            count[l as usize] += 1;
        }
        count[0] = 0;
        // Reject over-subscribed codes (incomplete ones are legal for single-symbol tables).
        let mut left = 1i32;
        for c in &count[1..] {
            left = (left << 1) - *c as i32;
            if left < 0 {
                return Err("invalid Huffman code lengths".into());
            }
        }
        let mut offs = [0u16; 16];
        for l in 1..15 {
            offs[l + 1] = offs[l] + count[l];
        }
        let mut symbol = vec![0u16; lengths.len()];
        for (s, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbol[offs[l as usize] as usize] = s as u16;
                offs[l as usize] += 1;
            }
        }
        Ok(Huffman { count, symbol })
    }

    fn decode(&self, br: &mut Bits) -> Result<usize, String> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..16 {
            code |= br.bit()? as i32;
            let count = self.count[len] as i32;
            if code - count < first {
                return Ok(self.symbol[(index + code - first) as usize] as usize);
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        Err("invalid Huffman code in deflate stream".into())
    }
}

fn fixed_tables() -> (Huffman, Huffman) {
    let mut l = [8u8; 288];
    l[144..256].fill(9);
    l[256..280].fill(7);
    (Huffman::new(&l).unwrap(), Huffman::new(&[5u8; 30]).unwrap())
}

/// Decompresses a raw DEFLATE stream.
pub fn inflate(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut br = Bits { data, pos: 0, bit: 0 };
    let mut out: Vec<u8> = Vec::new();
    loop {
        let last = br.bit()?;
        match br.bits(2)? {
            0 => {
                if br.bit != 0 {
                    br.bit = 0;
                    br.pos += 1;
                }
                let hdr = data.get(br.pos..br.pos + 4).ok_or("truncated stored block")?;
                let len = u16::from_le_bytes([hdr[0], hdr[1]]);
                if len != !u16::from_le_bytes([hdr[2], hdr[3]]) {
                    return Err("stored block length check failed".into());
                }
                br.pos += 4;
                out.extend_from_slice(data.get(br.pos..br.pos + len as usize).ok_or("truncated stored block")?);
                br.pos += len as usize;
            }
            t @ (1 | 2) => {
                let (lit, dist) = if t == 1 { fixed_tables() } else { dynamic_tables(&mut br)? };
                loop {
                    let sym = lit.decode(&mut br)?;
                    match sym {
                        0..=255 => out.push(sym as u8),
                        256 => break,
                        257..=285 => {
                            let len = LEN_BASE[sym - 257] as usize + br.bits(LEN_EXTRA[sym - 257] as u32)? as usize;
                            let ds = dist.decode(&mut br)?;
                            if ds >= 30 {
                                return Err("invalid distance code".into());
                            }
                            let d = DIST_BASE[ds] as usize + br.bits(DIST_EXTRA[ds] as u32)? as usize;
                            if d > out.len() {
                                return Err("distance reaches before the start of the data".into());
                            }
                            let start = out.len() - d;
                            for k in 0..len {
                                out.push(out[start + k]);
                            }
                        }
                        _ => return Err("invalid literal/length code".into()),
                    }
                }
            }
            _ => return Err("invalid deflate block type".into()),
        }
        if last == 1 {
            return Ok(out);
        }
    }
}

fn dynamic_tables(br: &mut Bits) -> Result<(Huffman, Huffman), String> {
    const ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
    let hlit = br.bits(5)? as usize + 257;
    let hdist = br.bits(5)? as usize + 1;
    let hclen = br.bits(4)? as usize + 4;
    if hlit > 286 || hdist > 30 {
        return Err("too many Huffman codes".into());
    }
    let mut cl = [0u8; 19];
    for &o in ORDER.iter().take(hclen) {
        cl[o] = br.bits(3)? as u8;
    }
    let clh = Huffman::new(&cl)?;
    let mut lengths = vec![0u8; hlit + hdist];
    let mut i = 0;
    while i < hlit + hdist {
        let sym = clh.decode(br)?;
        let (value, repeat) = match sym {
            0..=15 => (sym as u8, 1),
            16 => {
                if i == 0 {
                    return Err("repeat code with nothing to repeat".into());
                }
                (lengths[i - 1], 3 + br.bits(2)? as usize)
            }
            17 => (0, 3 + br.bits(3)? as usize),
            _ => (0, 11 + br.bits(7)? as usize),
        };
        if i + repeat > hlit + hdist {
            return Err("code lengths overflow".into());
        }
        lengths[i..i + repeat].fill(value);
        i += repeat;
    }
    if lengths[256] == 0 {
        return Err("no end-of-block code".into());
    }
    Ok((Huffman::new(&lengths[..hlit])?, Huffman::new(&lengths[hlit..])?))
}

// ---------------------------------------------------------------- deflate

struct BitWriter {
    out: Vec<u8>,
    acc: u64,
    n: u32,
}

impl BitWriter {
    /// Appends `n` bits, least significant first.
    fn put(&mut self, v: u32, n: u32) {
        self.acc |= (v as u64) << self.n;
        self.n += n;
        while self.n >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }
    /// Appends a Huffman code (which is defined most significant bit first).
    fn code(&mut self, code: u32, n: u32) {
        self.put(code.reverse_bits() >> (32 - n), n);
    }
    fn literal_or_length(&mut self, sym: u32) {
        match sym {
            0..=143 => self.code(0x30 + sym, 8),
            144..=255 => self.code(0x190 + sym - 144, 9),
            256..=279 => self.code(sym - 256, 7),
            _ => self.code(0xc0 + sym - 280, 8),
        }
    }
}

/// Compresses with LZ77 (hash chains, 32 KiB window) and the fixed Huffman code.
pub fn deflate(data: &[u8]) -> Vec<u8> {
    const WINDOW: usize = 32768;
    const CHAIN: usize = 24;
    let mut w = BitWriter { out: Vec::with_capacity(data.len() / 2 + 16), acc: 0, n: 0 };
    w.put(1, 1); // final block
    w.put(1, 2); // fixed Huffman
    let hash = |i: usize| ((data[i] as usize) << 10 ^ (data[i + 1] as usize) << 5 ^ data[i + 2] as usize) & 0x7fff;
    let mut head = vec![usize::MAX; 0x8000];
    let mut prev = vec![usize::MAX; data.len()];
    let mut i = 0;
    while i < data.len() {
        let (mut best_len, mut best_dist) = (0usize, 0usize);
        if i + 3 <= data.len() {
            let max = (data.len() - i).min(258);
            let mut cand = head[hash(i)];
            let mut steps = 0;
            while cand != usize::MAX && i - cand <= WINDOW && steps < CHAIN {
                let mut l = 0;
                while l < max && data[cand + l] == data[i + l] {
                    l += 1;
                }
                if l > best_len {
                    best_len = l;
                    best_dist = i - cand;
                    if l == max {
                        break;
                    }
                }
                cand = prev[cand];
                steps += 1;
            }
        }
        let advance = if best_len >= 3 {
            let lc = LEN_BASE.iter().rposition(|&b| b as usize <= best_len).unwrap();
            w.literal_or_length(257 + lc as u32);
            w.put((best_len - LEN_BASE[lc] as usize) as u32, LEN_EXTRA[lc] as u32);
            let dc = DIST_BASE.iter().rposition(|&b| b as usize <= best_dist).unwrap();
            w.code(dc as u32, 5);
            w.put((best_dist - DIST_BASE[dc] as usize) as u32, DIST_EXTRA[dc] as u32);
            best_len
        } else {
            w.literal_or_length(data[i] as u32);
            1
        };
        for k in i..i + advance {
            if k + 3 <= data.len() {
                let h = hash(k);
                prev[k] = head[h];
                head[h] = k;
            }
        }
        i += advance;
    }
    w.literal_or_length(256);
    w.put(0, 7); // flush the last partial byte
    w.out
}

fn zlib_compress(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    out.extend(deflate(data));
    out.extend(adler32(data).to_be_bytes());
    out
}

fn zlib_decompress(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < 6 || data[0] & 0x0f != 8 || (u16::from_be_bytes([data[0], data[1]]) % 31) != 0 || data[1] & 0x20 != 0 {
        return Err("not a zlib stream".into());
    }
    let out = inflate(&data[2..])?;
    // The checksum is the last four bytes of the stream (anything after the final block).
    let tail = &data[data.len() - 4..];
    if u32::from_be_bytes([tail[0], tail[1], tail[2], tail[3]]) != adler32(&out) {
        return Err("zlib checksum mismatch".into());
    }
    Ok(out)
}

// ---------------------------------------------------------------- PNG

/// A decoded picture: 8-bit RGBA, row 0 at the top.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub w: usize,
    pub h: usize,
    pub rgba: Vec<u8>,
}

impl Image {
    /// "Body-ness" of every pixel in [0, 1]: dark and opaque is body, light or transparent is
    /// background.
    pub fn silhouette(&self) -> Vec<f32> {
        self.rgba
            .chunks_exact(4)
            .map(|p| {
                let lum = (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32) / 255.0;
                (p[3] as f32 / 255.0) * (1.0 - lum)
            })
            .collect()
    }
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    out.extend((body.len() as u32).to_be_bytes());
    out.extend(kind);
    out.extend(body);
    out.extend(crc32(&[kind, body]).to_be_bytes());
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let (a, b, c) = (a as i32, b as i32, c as i32);
    let p = a + b - c;
    let (pa, pb, pc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());
    if pa <= pb && pa <= pc {
        a as u8
    } else if pb <= pc {
        b as u8
    } else {
        c as u8
    }
}

/// Colour layouts this encoder writes.
pub enum Pixels<'a> {
    Gray(&'a [u8]),
    Rgb(&'a [u8]),
    /// One palette index per pixel, plus the palette (at most 256 entries).
    Indexed(&'a [u8], &'a [[u8; 3]]),
}

/// Encodes an 8-bit image. Each row uses whichever of the None / Sub / Up filters gives the
/// smallest sum of absolute residuals.
pub fn encode(w: usize, h: usize, pixels: Pixels) -> Vec<u8> {
    let (data, bpp, color_type) = match pixels {
        Pixels::Gray(d) => (d, 1, 0u8),
        Pixels::Rgb(d) => (d, 3, 2),
        Pixels::Indexed(d, _) => (d, 1, 3),
    };
    assert_eq!(data.len(), w * h * bpp, "pixel buffer does not match the image size");
    let stride = w * bpp;
    let mut raw = Vec::with_capacity((stride + 1) * h);
    let mut candidates = [vec![0u8; stride], vec![0u8; stride], vec![0u8; stride]];
    for y in 0..h {
        let row = &data[y * stride..(y + 1) * stride];
        for x in 0..stride {
            let left = if x >= bpp { row[x - bpp] } else { 0 };
            let up = if y > 0 { data[(y - 1) * stride + x] } else { 0 };
            candidates[0][x] = row[x];
            candidates[1][x] = row[x].wrapping_sub(left);
            candidates[2][x] = row[x].wrapping_sub(up);
        }
        let cost = |v: &Vec<u8>| v.iter().map(|&b| (b as i8).unsigned_abs() as u64).sum::<u64>();
        let best = (0..3).min_by_key(|&k| cost(&candidates[k])).unwrap();
        raw.push(best as u8);
        raw.extend_from_slice(&candidates[best]);
    }
    let mut out = SIGNATURE.to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend((w as u32).to_be_bytes());
    ihdr.extend((h as u32).to_be_bytes());
    ihdr.extend([8, color_type, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    if let Pixels::Indexed(_, palette) = pixels {
        assert!(palette.len() <= 256);
        let flat: Vec<u8> = palette.iter().flatten().copied().collect();
        chunk(&mut out, b"PLTE", &flat);
    }
    chunk(&mut out, b"IDAT", &zlib_compress(&raw));
    chunk(&mut out, b"IEND", &[]);
    out
}

/// Decodes a PNG file into RGBA.
pub fn decode(file: &[u8]) -> Result<Image, String> {
    if file.len() < 8 || file[..8] != SIGNATURE {
        return Err("not a PNG file (bad signature)".into());
    }
    let mut pos = 8;
    let (mut w, mut h, mut depth, mut ctype) = (0usize, 0usize, 0u8, 0u8);
    let mut palette: Vec<u8> = Vec::new();
    let mut trns: Vec<u8> = Vec::new();
    let mut idat: Vec<u8> = Vec::new();
    let mut seen_end = false;
    while pos + 12 <= file.len() {
        let len = u32::from_be_bytes([file[pos], file[pos + 1], file[pos + 2], file[pos + 3]]) as usize;
        let kind = &file[pos + 4..pos + 8];
        let body = file.get(pos + 8..pos + 8 + len).ok_or("truncated PNG chunk")?;
        let crc = file.get(pos + 8 + len..pos + 12 + len).ok_or("truncated PNG chunk")?;
        if u32::from_be_bytes([crc[0], crc[1], crc[2], crc[3]]) != crc32(&[kind, body]) {
            return Err(format!("PNG chunk {} is corrupt (CRC mismatch)", String::from_utf8_lossy(kind)));
        }
        match kind {
            b"IHDR" => {
                if len != 13 {
                    return Err("bad IHDR".into());
                }
                w = u32::from_be_bytes([body[0], body[1], body[2], body[3]]) as usize;
                h = u32::from_be_bytes([body[4], body[5], body[6], body[7]]) as usize;
                depth = body[8];
                ctype = body[9];
                if body[12] != 0 {
                    return Err("interlaced PNG files are not supported; save the picture without interlacing".into());
                }
            }
            b"PLTE" => palette = body.to_vec(),
            b"tRNS" => trns = body.to_vec(),
            b"IDAT" => idat.extend_from_slice(body),
            b"IEND" => {
                seen_end = true;
                break;
            }
            _ => {}
        }
        pos += 12 + len;
    }
    if !seen_end || w == 0 || h == 0 {
        return Err("PNG file is incomplete".into());
    }
    if w.checked_mul(h).is_none_or(|p| p > 64_000_000) {
        return Err("PNG picture is too large (limit 64 megapixels)".into());
    }
    let channels = match ctype {
        0 | 3 => 1,
        2 => 3,
        4 => 2,
        6 => 4,
        _ => return Err("unknown PNG colour type".into()),
    };
    let depth_ok = match ctype {
        0 => [1, 2, 4, 8, 16].contains(&depth),
        3 => [1, 2, 4, 8].contains(&depth),
        _ => depth == 8 || depth == 16,
    };
    if !depth_ok {
        return Err("invalid PNG bit depth".into());
    }
    let bits = channels * depth as usize;
    let bpp = bits.div_ceil(8);
    let stride = (w * bits).div_ceil(8);
    let raw = zlib_decompress(&idat)?;
    if raw.len() != (stride + 1) * h {
        return Err("PNG pixel data has the wrong size".into());
    }
    // Undo the row filters in place.
    let mut px = vec![0u8; stride * h];
    for y in 0..h {
        let filter = raw[y * (stride + 1)];
        let src = &raw[y * (stride + 1) + 1..(y + 1) * (stride + 1)];
        for x in 0..stride {
            let a = if x >= bpp { px[y * stride + x - bpp] } else { 0 };
            let b = if y > 0 { px[(y - 1) * stride + x] } else { 0 };
            let c = if x >= bpp && y > 0 { px[(y - 1) * stride + x - bpp] } else { 0 };
            let pred = match filter {
                0 => 0,
                1 => a,
                2 => b,
                3 => ((a as u16 + b as u16) / 2) as u8,
                4 => paeth(a, b, c),
                _ => return Err("invalid PNG row filter".into()),
            };
            px[y * stride + x] = src[x].wrapping_add(pred);
        }
    }
    // Expand to RGBA.
    let sample = |row: &[u8], i: usize| -> u8 {
        match depth {
            8 => row[i],
            16 => row[2 * i],
            _ => {
                let d = depth as usize;
                let shift = 8 - d - (i * d) % 8;
                (row[i * d / 8] >> shift) & ((1u8 << d) - 1)
            }
        }
    };
    let maxv = if depth >= 8 { 255u32 } else { (1u32 << depth) - 1 };
    let mut rgba = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        let row = &px[y * stride..(y + 1) * stride];
        for x in 0..w {
            let s = |c: usize| sample(row, x * channels + c);
            let scale = |v: u8| (v as u32 * 255 / maxv) as u8;
            let p = match ctype {
                0 => {
                    let g = scale(s(0));
                    [g, g, g, 255]
                }
                2 => [s(0), s(1), s(2), 255],
                3 => {
                    let i = s(0) as usize;
                    let c = palette.get(3 * i..3 * i + 3).ok_or("palette index out of range")?;
                    [c[0], c[1], c[2], trns.get(i).copied().unwrap_or(255)]
                }
                4 => [s(0), s(0), s(0), s(1)],
                _ => [s(0), s(1), s(2), s(3)],
            };
            rgba.extend(p);
        }
    }
    Ok(Image { w, h, rgba })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(n: usize, seed: u64, modulus: u8) -> Vec<u8> {
        let mut s = seed;
        (0..n)
            .map(|_| {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                ((s >> 33) as u8) % modulus
            })
            .collect()
    }

    #[test]
    fn checksums_match_known_values() {
        assert_eq!(crc32(&[b"123456789"]), 0xcbf4_3926);
        assert_eq!(crc32(&[b"1234", b"56789"]), 0xcbf4_3926);
        assert_eq!(adler32(b"Wikipedia"), 0x11e6_0398);
    }

    #[test]
    fn inflate_known_streams() {
        // Fixed-Huffman stream for "hello hello hello hello\n" (made with zlib, raw deflate).
        let fixed = [0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0xc8, 0x40, 0x27, 0xb9, 0x00];
        assert_eq!(inflate(&fixed).unwrap(), b"hello hello hello hello\n");
        // Dynamic-Huffman stream (zlib level 9) of "0 1 4 9 16 ..." = i*i mod 97 for i < 120.
        let dynamic: [u8; 147] = [
            0xa5, 0x90, 0x89, 0x0d, 0x44, 0x21, 0x08, 0x05, 0x5b, 0x99, 0x12, 0x3e, 0xa0, 0xa8, 0xfd, 0x37, 0xb6, 0xa3, 0x2d, 0x6c, 0x62, 0x8c,
            0xc7, 0x3b, 0xf9, 0x08, 0x06, 0x87, 0x68, 0x72, 0x52, 0xcd, 0x38, 0xf4, 0x60, 0x07, 0x45, 0x0e, 0xc6, 0x62, 0x25, 0x49, 0x05, 0x9d,
            0x1c, 0x11, 0xc5, 0xfa, 0x88, 0x64, 0x16, 0x47, 0xb4, 0x5c, 0x05, 0x3c, 0x0f, 0xe6, 0xc7, 0xa6, 0x27, 0xb9, 0xd8, 0x9b, 0x39, 0xc8,
            0x64, 0xab, 0xa6, 0xd4, 0x24, 0x82, 0xdd, 0xb4, 0x8c, 0x4d, 0x25, 0x21, 0x92, 0x53, 0xec, 0xc9, 0x3a, 0x2c, 0xf7, 0x7a, 0xeb, 0x5d,
            0x7d, 0xf4, 0xab, 0x2f, 0x48, 0xa8, 0x04, 0x69, 0x92, 0xe3, 0x09, 0x29, 0xa7, 0xa8, 0xd2, 0x1a, 0x68, 0xa3, 0x99, 0x96, 0xfb, 0x9a,
            0x1b, 0xe1, 0x06, 0x89, 0x17, 0xaa, 0x6f, 0x40, 0x63, 0x1a, 0xd6, 0xc8, 0x06, 0xef, 0x57, 0x22, 0x6f, 0x1d, 0x4b, 0x59, 0xad, 0x6e,
            0x49, 0xab, 0x5a, 0xb8, 0x5e, 0x79, 0x47, 0x70, 0x9c, 0x45, 0xf0, 0xfd, 0x37, 0x93, 0x1f,
        ];
        let expect: String = (0..120).map(|i| format!("{} ", i * i % 97)).collect();
        assert_eq!(inflate(&dynamic).unwrap(), expect.as_bytes());
        // Stored block.
        let stored = [0x01, 0x03, 0x00, 0xfc, 0xff, b'a', b'b', b'c'];
        assert_eq!(inflate(&stored).unwrap(), b"abc");
    }

    #[test]
    fn deflate_round_trip() {
        let mut cases: Vec<Vec<u8>> = vec![vec![], vec![7], b"abcabcabcabcabcabc".to_vec(), vec![0; 100_000]];
        cases.push(noise(50_000, 1, 255)); // incompressible
        cases.push(noise(50_000, 2, 3)); // few symbols, short matches
        let mut text = Vec::new();
        for i in 0..4000 {
            text.extend(format!("line {} of the test corpus; ", i % 37).bytes());
        }
        cases.push(text);
        for (k, c) in cases.iter().enumerate() {
            let z = deflate(c);
            assert_eq!(&inflate(&z).unwrap(), c, "case {k}");
        }
        assert!(deflate(&vec![0; 100_000]).len() < 700);
    }

    #[test]
    fn inflate_rejects_garbage_without_panicking() {
        let good = deflate(&noise(2000, 5, 16));
        for cut in [0, 1, 5, good.len() / 2, good.len() - 1] {
            assert!(inflate(&good[..cut]).is_err());
        }
        let mut seed = 77u64;
        for _ in 0..2000 {
            let n = 1 + (noise(1, seed, 60)[0] as usize);
            seed += 1;
            let _ = inflate(&noise(n, seed, 255)); // must return, not panic or hang
        }
    }

    #[test]
    fn png_round_trip_all_writers() {
        let (w, h) = (37, 23);
        let gray = noise(w * h, 3, 255);
        let img = decode(&encode(w, h, Pixels::Gray(&gray))).unwrap();
        assert_eq!((img.w, img.h), (w, h));
        for i in 0..w * h {
            assert_eq!(img.rgba[4 * i..4 * i + 4], [gray[i], gray[i], gray[i], 255]);
        }
        let rgb = noise(w * h * 3, 4, 255);
        let img = decode(&encode(w, h, Pixels::Rgb(&rgb))).unwrap();
        for i in 0..w * h {
            assert_eq!(img.rgba[4 * i..4 * i + 3], rgb[3 * i..3 * i + 3]);
        }
        let palette: Vec<[u8; 3]> = (0..16).map(|i| [i * 16, 255 - i * 16, i]).collect();
        let idx = noise(w * h, 5, 16);
        let img = decode(&encode(w, h, Pixels::Indexed(&idx, &palette))).unwrap();
        for i in 0..w * h {
            assert_eq!(img.rgba[4 * i..4 * i + 3], palette[idx[i] as usize]);
        }
    }

    #[test]
    fn smooth_images_compress() {
        // A smooth gradient: the Sub filter turns it into long runs.
        let (w, h) = (256, 64);
        let data: Vec<u8> = (0..w * h).map(|i| (i % w) as u8).collect();
        assert!(encode(w, h, Pixels::Gray(&data)).len() < w * h / 20);
    }

    #[test]
    fn decode_rejects_corruption() {
        let mut png = encode(8, 8, Pixels::Gray(&[9; 64]));
        assert!(decode(&png[..20]).is_err());
        let n = png.len();
        png[n - 20] ^= 0x40; // inside IDAT
        assert!(decode(&png).unwrap_err().contains("CRC"));
        assert!(decode(b"GIF89a....").is_err());
    }

    #[test]
    fn silhouette_from_dark_and_transparent_pixels() {
        let img = Image { w: 4, h: 1, rgba: vec![0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 0, 128, 128, 128, 255] };
        let s = img.silhouette();
        assert!(s[0] > 0.99 && s[1] < 0.01 && s[2] < 0.01 && (s[3] - 0.5).abs() < 0.01);
    }
}
