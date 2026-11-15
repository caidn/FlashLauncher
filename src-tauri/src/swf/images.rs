//! Pulls bitmaps out of a SWF so they can be offered as fallback art.
//! Handles every bitmap tag: DefineBits (+JPEGTables), DefineBitsJPEG2/3/4
//! and DefineBitsLossless/Lossless2. Everything comes out as PNG.

use super::{reader::ByteReader, Swf};
use flate2::read::ZlibDecoder;
use image::{ImageFormat, ImageReader, RgbaImage};
use std::io::{Cursor, Read};

const DEFINE_BITS: u16 = 6;
const JPEG_TABLES: u16 = 8;
const DEFINE_BITS_LOSSLESS: u16 = 20;
const DEFINE_BITS_JPEG2: u16 = 21;
const DEFINE_BITS_JPEG3: u16 = 35;
const DEFINE_BITS_LOSSLESS2: u16 = 36;
const DEFINE_BITS_JPEG4: u16 = 90;

/// Anything smaller is an icon, a particle or a UI scrap, not useful art.
const MIN_SIDE: u32 = 48;
const MAX_SIDE: u32 = 4096;

#[derive(Debug, Clone)]
pub struct EmbeddedImage {
    pub width: u32,
    pub height: u32,
    /// 0..1, how much the image is worth showing. See `interest`.
    pub interest: f64,
    pub png: Vec<u8>,
}

enum Source<'a> {
    /// Encoded image bytes (JPEG/PNG/GIF) plus optional zlib alpha plane.
    Encoded {
        bytes: Vec<u8>,
        alpha: Option<&'a [u8]>,
    },
    Lossless {
        version: u8,
        data: &'a [u8],
    },
}

struct Candidate<'a> {
    width: u32,
    height: u32,
    source: Source<'a>,
}

/// Returns up to `limit` of the largest images in the file, biggest first.
pub fn extract_images(swf: &Swf, limit: usize) -> Vec<EmbeddedImage> {
    let tags = swf.tags();
    let tables = tags.iter().find(|t| t.code == JPEG_TABLES).map(|t| t.data);

    // Pass 1: find every bitmap and its size without fully decoding it.
    let mut found: Vec<Candidate> = Vec::new();
    for t in &tags {
        let mut r = ByteReader::new(t.data);
        let cand = match t.code {
            DEFINE_BITS => (|| {
                r.u16()?; // character id
                let bytes = merge_jpeg_tables(tables?, r.rest());
                encoded(bytes, None)
            })(),
            DEFINE_BITS_JPEG2 => (|| {
                r.u16()?; // character id
                encoded(fix_jpeg(r.rest()), None)
            })(),
            DEFINE_BITS_JPEG3 | DEFINE_BITS_JPEG4 => (|| {
                r.u16()?; // character id
                let alpha_offset = r.u32()? as usize;
                if t.code == DEFINE_BITS_JPEG4 {
                    r.u16()?; // deblocking param
                }
                let rest = r.rest();
                let img = rest.get(..alpha_offset)?;
                let alpha = rest.get(alpha_offset..).filter(|a| !a.is_empty());
                encoded(fix_jpeg(img), alpha)
            })(),
            DEFINE_BITS_LOSSLESS | DEFINE_BITS_LOSSLESS2 => (|| {
                r.u16()?; // character id
                let _format = r.u8()?;
                let width = r.u16()? as u32;
                let height = r.u16()? as u32;
                let version = if t.code == DEFINE_BITS_LOSSLESS { 1 } else { 2 };
                Some(Candidate {
                    width,
                    height,
                    source: Source::Lossless {
                        version,
                        data: t.data,
                    },
                })
            })(),
            _ => None,
        };
        if let Some(c) = cand {
            if c.width >= MIN_SIDE && c.height >= MIN_SIDE && c.width <= MAX_SIDE && c.height <= MAX_SIDE {
                found.push(c);
            }
        }
    }

    found.sort_by_key(|c| std::cmp::Reverse(c.width as u64 * c.height as u64));

    // Pass 2: decode only the ones we keep.
    let mut out = Vec::new();
    for c in found {
        if out.len() >= limit {
            break;
        }
        let rgba = match &c.source {
            Source::Encoded { bytes, alpha } => decode_encoded(bytes, *alpha),
            Source::Lossless { version, data } => decode_lossless(*version, data),
        };
        let Some(rgba) = rgba else { continue };
        if is_blank(&rgba) {
            continue;
        }
        let mut png = Vec::new();
        if rgba
            .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .is_ok()
        {
            out.push(EmbeddedImage {
                width: rgba.width(),
                height: rgba.height(),
                interest: interest(&rgba),
                png,
            });
        }
    }
    out
}

fn encoded<'a>(bytes: Vec<u8>, alpha: Option<&'a [u8]>) -> Option<Candidate<'a>> {
    let (width, height) = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()?;
    Some(Candidate {
        width,
        height,
        source: Source::Encoded { bytes, alpha },
    })
}

/// Flash-era JPEGs often carry a bogus EOI+SOI pair (FF D9 FF D8) at the
/// start or where two streams were glued together. Decoders choke on it.
fn fix_jpeg(data: &[u8]) -> Vec<u8> {
    if data.starts_with(&[0x89, b'P', b'N', b'G']) || data.starts_with(b"GIF8") {
        return data.to_vec();
    }
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        if data[i..].starts_with(&[0xff, 0xd9, 0xff, 0xd8]) {
            i += 4;
            continue;
        }
        out.push(data[i]);
        i += 1;
    }
    // Starting with the bogus pair leaves no SOI at all; put one back.
    if !out.starts_with(&[0xff, 0xd8]) {
        out.splice(0..0, [0xff, 0xd8]);
    }
    out
}

/// DefineBits stores only scan data; the tables live in a shared JPEGTables tag.
fn merge_jpeg_tables(tables: &[u8], data: &[u8]) -> Vec<u8> {
    let mut joined = tables.to_vec();
    joined.extend_from_slice(data);
    fix_jpeg(&joined)
}

fn decode_encoded(bytes: &[u8], alpha: Option<&[u8]>) -> Option<RgbaImage> {
    let img = image::load_from_memory(bytes).ok()?;
    let mut rgba = img.to_rgba8();
    if let Some(a) = alpha {
        let n = (rgba.width() * rgba.height()) as usize;
        if let Some(plane) = inflate_exact(a, n) {
            for (px, alpha) in rgba.pixels_mut().zip(plane) {
                px.0[3] = alpha;
            }
        }
    }
    Some(rgba)
}

fn decode_lossless(version: u8, data: &[u8]) -> Option<RgbaImage> {
    let mut r = ByteReader::new(data);
    r.u16()?; // id
    let format = r.u8()?;
    let width = r.u16()? as usize;
    let height = r.u16()? as usize;
    let mut img = RgbaImage::new(width as u32, height as u32);

    match format {
        3 => {
            let colors = r.u8()? as usize + 1;
            let entry = if version == 1 { 3 } else { 4 };
            let row = (width + 3) & !3;
            let raw = inflate_exact(r.rest(), colors * entry + row * height)?;
            let (table, pixels) = raw.split_at(colors * entry);
            for y in 0..height {
                for x in 0..width {
                    let idx = pixels[y * row + x] as usize;
                    let px = if idx < colors {
                        let c = &table[idx * entry..idx * entry + entry];
                        if version == 1 {
                            [c[0], c[1], c[2], 255]
                        } else {
                            unpremultiply([c[0], c[1], c[2], c[3]])
                        }
                    } else {
                        [0, 0, 0, 0]
                    };
                    img.put_pixel(x as u32, y as u32, image::Rgba(px));
                }
            }
        }
        4 if version == 1 => {
            let row = (width * 2 + 3) & !3;
            let raw = inflate_exact(r.rest(), row * height)?;
            for y in 0..height {
                for x in 0..width {
                    let o = y * row + x * 2;
                    let v = u16::from_be_bytes([raw[o], raw[o + 1]]);
                    let c5 = |s: u16| (((v >> s) & 0x1f) as u32 * 255 / 31) as u8;
                    img.put_pixel(x as u32, y as u32, image::Rgba([c5(10), c5(5), c5(0), 255]));
                }
            }
        }
        5 => {
            let raw = inflate_exact(r.rest(), width * height * 4)?;
            for (i, px) in raw.chunks_exact(4).enumerate() {
                let (x, y) = ((i % width) as u32, (i / width) as u32);
                let rgba = if version == 1 {
                    [px[1], px[2], px[3], 255]
                } else {
                    unpremultiply([px[1], px[2], px[3], px[0]])
                };
                img.put_pixel(x, y, image::Rgba(rgba));
            }
        }
        _ => return None,
    }
    Some(img)
}

fn unpremultiply([r, g, b, a]: [u8; 4]) -> [u8; 4] {
    if a == 0 || a == 255 {
        return [r, g, b, a];
    }
    let f = |c: u8| ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
    [f(r), f(g), f(b), a]
}

/// Decompress exactly `n` bytes, or None if the stream is shorter.
fn inflate_exact(data: &[u8], n: usize) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(n);
    let _ = ZlibDecoder::new(data).take(n as u64).read_to_end(&mut out);
    (out.len() == n).then_some(out)
}

/// Rough "would a person want this as box art" score, 0..1. Colorful,
/// varied images score high. Mostly-black or gray ones (loading screens,
/// preloader backdrops, masks) score low.
pub fn interest(img: &RgbaImage) -> f64 {
    let total = (img.width() * img.height()) as usize;
    let step = (total / 4096).max(1);
    let (mut opaque, mut dark, mut sat_sum) = (0usize, 0usize, 0.0f64);
    let mut buckets = std::collections::HashSet::new();
    for p in img.pixels().step_by(step) {
        let [r, g, b, a] = p.0;
        if a < 32 {
            continue;
        }
        opaque += 1;
        let lum = 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
        if lum < 40.0 {
            dark += 1;
        }
        let max = r.max(g).max(b) as f64;
        let min = r.min(g).min(b) as f64;
        sat_sum += (max - min) / 255.0;
        buckets.insert((r >> 4, g >> 4, b >> 4));
    }
    if opaque == 0 {
        return 0.0;
    }
    let sat = sat_sum / opaque as f64;
    let dark_frac = dark as f64 / opaque as f64;
    let variety = (buckets.len() as f64 / 48.0).min(1.0);
    (0.2 + 0.8 * (sat * 2.5).min(1.0)) * (1.0 - 0.75 * dark_frac) * (0.4 + 0.6 * variety)
}

/// Fully transparent or single-color images aren't worth showing.
fn is_blank(img: &RgbaImage) -> bool {
    let mut pixels = img.pixels().step_by(7);
    let Some(first) = pixels.next() else {
        return true;
    };
    if img.pixels().all(|p| p.0[3] == 0) {
        return true;
    }
    pixels.all(|p| p == first)
}

#[cfg(test)]
mod tests {
    use super::super::tests::{body, fws};
    use super::*;
    use flate2::{write::ZlibEncoder, Compression};
    use std::io::Write;

    fn zlib(data: &[u8]) -> Vec<u8> {
        let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }

    fn gradient(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, y| image::Rgba([(x * 3) as u8, (y * 5) as u8, 128, 255]))
    }

    fn encode(img: &RgbaImage, fmt: ImageFormat) -> Vec<u8> {
        let mut v = Vec::new();
        if fmt == ImageFormat::Jpeg {
            image::DynamicImage::ImageRgba8(img.clone())
                .to_rgb8()
                .write_to(&mut Cursor::new(&mut v), fmt)
                .unwrap();
        } else {
            img.write_to(&mut Cursor::new(&mut v), fmt).unwrap();
        }
        v
    }

    #[test]
    fn extracts_jpeg2_with_bogus_header_and_lossless2() {
        let jpeg = encode(&gradient(80, 60), ImageFormat::Jpeg);
        let mut j2 = 1u16.to_le_bytes().to_vec();
        j2.extend_from_slice(&[0xff, 0xd9, 0xff, 0xd8]);
        j2.extend_from_slice(&jpeg);

        // 64x64 ARGB lossless2, a two-color pattern so it isn't "blank"
        let (w, h) = (64u16, 64u16);
        let mut argb = Vec::new();
        for i in 0..(w as usize * h as usize) {
            if i % 2 == 0 {
                argb.extend_from_slice(&[255, 200, 10, 10]);
            } else {
                argb.extend_from_slice(&[255, 10, 10, 200]);
            }
        }
        let mut l2 = 2u16.to_le_bytes().to_vec();
        l2.push(5);
        l2.extend_from_slice(&w.to_le_bytes());
        l2.extend_from_slice(&h.to_le_bytes());
        l2.extend_from_slice(&zlib(&argb));

        // a tiny image that should be skipped
        let small = encode(&gradient(16, 16), ImageFormat::Png);
        let mut s = 3u16.to_le_bytes().to_vec();
        s.extend_from_slice(&small);

        let b = body(100, 100, (24, 0), 1, &[(21, j2), (36, l2), (21, s)]);
        let swf = Swf::parse(&fws(8, &b)).unwrap();
        let imgs = extract_images(&swf, 10);
        assert_eq!(imgs.len(), 2);
        assert_eq!((imgs[0].width, imgs[0].height), (80, 60));
        assert_eq!((imgs[1].width, imgs[1].height), (64, 64));
        let decoded = image::load_from_memory(&imgs[1].png).unwrap().to_rgba8();
        assert_eq!(decoded.get_pixel(0, 0).0, [200, 10, 10, 255]);
    }

    #[test]
    fn colormapped_lossless_and_jpeg3_alpha() {
        // 50x50 paletted, padded rows (50 -> 52)
        let (w, h) = (50usize, 50usize);
        let mut raw = vec![255, 0, 0, 0, 255, 0]; // 2-color RGB palette
        for y in 0..h {
            for x in 0..52 {
                raw.push(((x + y) % 2) as u8);
            }
        }
        let mut l1 = 7u16.to_le_bytes().to_vec();
        l1.push(3);
        l1.extend_from_slice(&(w as u16).to_le_bytes());
        l1.extend_from_slice(&(h as u16).to_le_bytes());
        l1.push(1); // palette size - 1
        l1.extend_from_slice(&zlib(&raw));

        let jpeg = encode(&gradient(60, 60), ImageFormat::Jpeg);
        let mut j3 = 8u16.to_le_bytes().to_vec();
        j3.extend_from_slice(&(jpeg.len() as u32).to_le_bytes());
        j3.extend_from_slice(&jpeg);
        j3.extend_from_slice(&zlib(&vec![128u8; 60 * 60]));

        let b = body(100, 100, (24, 0), 1, &[(20, l1), (35, j3)]);
        let swf = Swf::parse(&fws(8, &b)).unwrap();
        let imgs = extract_images(&swf, 10);
        assert_eq!(imgs.len(), 2);
        let j = image::load_from_memory(&imgs[0].png).unwrap().to_rgba8();
        assert_eq!(j.get_pixel(5, 5).0[3], 128);
        let p = image::load_from_memory(&imgs[1].png).unwrap().to_rgba8();
        assert_eq!(p.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(p.get_pixel(1, 0).0, [0, 255, 0, 255]);
    }

    #[test]
    fn interest_prefers_colorful_over_dark() {
        let dark = RgbaImage::from_fn(100, 100, |x, _| image::Rgba([(x % 3) as u8 * 20, 20, 20, 255]));
        let colorful = gradient(100, 100);
        assert!(interest(&colorful) > interest(&dark) * 3.0);
        let clear = RgbaImage::new(10, 10);
        assert_eq!(interest(&clear), 0.0);
    }

    #[test]
    fn define_bits_with_shared_tables() {
        let jpeg = encode(&gradient(64, 64), ImageFormat::Jpeg);
        // Split a real JPEG at the SOS marker: tables = header part + EOI,
        // image = SOI + scan data. Same shape Flash wrote.
        let sos = jpeg.windows(2).position(|w| w == [0xff, 0xda]).unwrap();
        let mut tables = jpeg[..sos].to_vec();
        tables.extend_from_slice(&[0xff, 0xd9]);
        let mut img = vec![0xff, 0xd8];
        img.extend_from_slice(&jpeg[sos..]);
        let mut db = 4u16.to_le_bytes().to_vec();
        db.extend_from_slice(&img);

        let b = body(100, 100, (24, 0), 1, &[(8, tables), (6, db)]);
        let swf = Swf::parse(&fws(6, &b)).unwrap();
        let imgs = extract_images(&swf, 10);
        assert_eq!(imgs.len(), 1);
        assert_eq!((imgs[0].width, imgs[0].height), (64, 64));
    }
}
