//! Minimal SWF reader: enough to describe a file on import and pull
//! embedded images out of it. Not a player. Ruffle does the playing.
//!
//! Format reference: the SWF File Format Specification v19.

mod images;
mod reader;

pub use images::{extract_images, interest};

use flate2::read::ZlibDecoder;
use reader::{BitReader, ByteReader};
use serde::Serialize;
use std::io::Read;

/// Refuse anything claiming to decompress past this. Real games top out
/// well under 100 MB; this just stops a hostile header from eating RAM.
const MAX_UNCOMPRESSED: u32 = 512 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Compression {
    /// "FWS"
    None,
    /// "CWS", zlib, Flash 6+
    Zlib,
    /// "ZWS", LZMA, Flash 13+
    Lzma,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwfInfo {
    pub compression: Compression,
    pub version: u8,
    pub uncompressed_len: u32,
    pub width: f32,
    pub height: f32,
    pub frame_rate: f32,
    pub frame_count: u16,
    /// 1, 2 or 3. 1 vs 2 is a best guess from the SWF version; 3 is certain.
    pub as_version: u8,
    pub uses_network: bool,
    pub background: Option<String>,
    /// dc:title from the Metadata tag, when the author bothered.
    pub meta_title: Option<String>,
}

#[derive(Debug)]
pub struct Swf {
    pub info: SwfInfo,
    /// Decompressed tag stream, starting right after the header.
    body: Vec<u8>,
}

#[derive(Debug, PartialEq)]
pub enum SwfError {
    TooShort,
    BadSignature,
    TooLarge(u32),
    Decompress(String),
    Truncated,
}

impl std::fmt::Display for SwfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SwfError::TooShort => write!(f, "file is too short to be a SWF"),
            SwfError::BadSignature => write!(f, "not a SWF file (bad signature)"),
            SwfError::TooLarge(n) => write!(f, "SWF claims to be {n} bytes uncompressed, which is too big"),
            SwfError::Decompress(e) => write!(f, "couldn't decompress SWF: {e}"),
            SwfError::Truncated => write!(f, "SWF header is truncated"),
        }
    }
}

impl std::error::Error for SwfError {}

pub(crate) struct Tag<'a> {
    pub code: u16,
    pub data: &'a [u8],
}

mod tag {
    pub const END: u16 = 0;
    pub const SET_BACKGROUND_COLOR: u16 = 9;
    pub const FILE_ATTRIBUTES: u16 = 69;
    pub const DO_ABC_OLD: u16 = 72;
    pub const METADATA: u16 = 77;
    pub const DO_ABC: u16 = 82;
}

impl Swf {
    pub fn parse(bytes: &[u8]) -> Result<Swf, SwfError> {
        if bytes.len() < 8 {
            return Err(SwfError::TooShort);
        }
        let compression = match &bytes[0..3] {
            b"FWS" => Compression::None,
            b"CWS" => Compression::Zlib,
            b"ZWS" => Compression::Lzma,
            _ => return Err(SwfError::BadSignature),
        };
        let version = bytes[3];
        let uncompressed_len = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        if uncompressed_len > MAX_UNCOMPRESSED {
            return Err(SwfError::TooLarge(uncompressed_len));
        }
        // Everything after the 8-byte header, decompressed.
        let body_len = uncompressed_len.saturating_sub(8) as usize;
        let body = match compression {
            Compression::None => bytes[8..].to_vec(),
            Compression::Zlib => inflate(&bytes[8..], body_len)?,
            Compression::Lzma => unlzma(bytes, body_len)?,
        };

        let mut info = SwfInfo {
            compression,
            version,
            uncompressed_len,
            width: 0.0,
            height: 0.0,
            frame_rate: 0.0,
            frame_count: 0,
            as_version: 1,
            uses_network: false,
            background: None,
            meta_title: None,
        };

        // Stage RECT (bit packed, in twips), then frame rate (8.8) and count.
        let mut bits = BitReader::new(&body);
        let nbits = bits.read_ubits(5).ok_or(SwfError::Truncated)?;
        let x_min = bits.read_sbits(nbits).ok_or(SwfError::Truncated)?;
        let x_max = bits.read_sbits(nbits).ok_or(SwfError::Truncated)?;
        let y_min = bits.read_sbits(nbits).ok_or(SwfError::Truncated)?;
        let y_max = bits.read_sbits(nbits).ok_or(SwfError::Truncated)?;
        info.width = (x_max - x_min) as f32 / 20.0;
        info.height = (y_max - y_min) as f32 / 20.0;

        let mut r = ByteReader::new(&body[bits.byte_pos()..]);
        let rate_frac = r.u8().ok_or(SwfError::Truncated)?;
        let rate_int = r.u8().ok_or(SwfError::Truncated)?;
        info.frame_rate = rate_int as f32 + rate_frac as f32 / 256.0;
        info.frame_count = r.u16().ok_or(SwfError::Truncated)?;
        let tags_start = bits.byte_pos() + 4;

        let mut swf = Swf { info, body };
        swf.body.drain(..tags_start.min(swf.body.len()));
        swf.scan_tags();
        Ok(swf)
    }

    /// Walks the top-level tags. Stops quietly at the first malformed tag,
    /// since lots of archived SWFs have junk tacked on the end.
    pub(crate) fn tags(&self) -> Vec<Tag<'_>> {
        let mut out = Vec::new();
        let mut r = ByteReader::new(&self.body);
        while let Some(header) = r.u16() {
            let code = header >> 6;
            let mut len = (header & 0x3f) as usize;
            if len == 0x3f {
                match r.u32() {
                    Some(l) => len = l as usize,
                    None => break,
                }
            }
            let Some(data) = r.bytes(len) else { break };
            if code == tag::END {
                break;
            }
            out.push(Tag { code, data });
        }
        out
    }

    fn scan_tags(&mut self) {
        let mut has_abc = false;
        let mut as3_flag = false;
        let mut background = None;
        let mut meta_title = None;
        let mut uses_network = false;

        for t in self.tags() {
            match t.code {
                tag::FILE_ATTRIBUTES => {
                    if let Some(&flags) = t.data.first() {
                        as3_flag = flags & 0x08 != 0;
                        uses_network = flags & 0x01 != 0;
                    }
                }
                tag::DO_ABC | tag::DO_ABC_OLD => has_abc = true,
                tag::SET_BACKGROUND_COLOR if t.data.len() >= 3 => {
                    background = Some(format!("#{:02x}{:02x}{:02x}", t.data[0], t.data[1], t.data[2]));
                }
                tag::METADATA => {
                    meta_title = metadata_title(t.data);
                }
                _ => {}
            }
        }

        self.info.as_version = if as3_flag || has_abc {
            3
        } else if self.info.version >= 7 {
            // AS2 arrived with Flash 7, so older files are AS1. SWF 7/8 can
            // still hold AS1 code, but AS2 is by far the common case there.
            2
        } else {
            1
        };
        self.info.background = background;
        self.info.meta_title = meta_title;
        self.info.uses_network = uses_network;
    }
}

fn inflate(data: &[u8], expected: usize) -> Result<Vec<u8>, SwfError> {
    let mut out = Vec::with_capacity(expected.min(64 * 1024 * 1024));
    // Read up to the declared size; some files have trailing garbage that
    // makes the zlib checksum fail, but the content is fine.
    let mut dec = ZlibDecoder::new(data).take(expected as u64);
    match dec.read_to_end(&mut out) {
        Ok(_) => Ok(out),
        Err(e) if !out.is_empty() => {
            // Keep a partial body if we got the header at least.
            if out.len() >= 32 {
                Ok(out)
            } else {
                Err(SwfError::Decompress(e.to_string()))
            }
        }
        Err(e) => Err(SwfError::Decompress(e.to_string())),
    }
}

/// ZWS layout: "ZWS" ver u32:uncompressedLen u32:compressedLen props[5] data...
/// lzma-rs wants the classic .lzma header (props + u64 size), so rebuild one.
fn unlzma(bytes: &[u8], body_len: usize) -> Result<Vec<u8>, SwfError> {
    if bytes.len() < 17 {
        return Err(SwfError::Truncated);
    }
    let props = &bytes[12..17];
    let data = &bytes[17..];
    let mut stream = Vec::with_capacity(13 + data.len());
    stream.extend_from_slice(props);
    stream.extend_from_slice(&(body_len as u64).to_le_bytes());
    stream.extend_from_slice(data);

    let mut out = Vec::with_capacity(body_len.min(64 * 1024 * 1024));
    let opts = lzma_rs::decompress::Options {
        unpacked_size: lzma_rs::decompress::UnpackedSize::ReadHeaderButUseProvided(Some(body_len as u64)),
        ..Default::default()
    };
    lzma_rs::lzma_decompress_with_options(&mut std::io::Cursor::new(stream), &mut out, &opts)
        .map_err(|e| SwfError::Decompress(e.to_string()))?;
    Ok(out)
}

fn metadata_title(data: &[u8]) -> Option<String> {
    let xml = String::from_utf8_lossy(data.strip_suffix(&[0]).unwrap_or(data));
    let start = xml.find("<dc:title")?;
    let open_end = xml[start..].find('>')? + start + 1;
    let close = xml[open_end..].find("</dc:title>")? + open_end;
    let mut inner = xml[open_end..close].trim().to_string();
    // Titles are sometimes wrapped in rdf:Alt / rdf:li.
    while let (Some(a), Some(b)) = (inner.find('<'), inner.rfind('>')) {
        if let Some(gt) = inner[a..].find('>') {
            let tag_end = a + gt + 1;
            if let Some(close_start) = inner[..=b].rfind("</") {
                if close_start >= tag_end {
                    inner = inner[tag_end..close_start].trim().to_string();
                    continue;
                }
            }
        }
        break;
    }
    let text = inner
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'");
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use flate2::{write::ZlibEncoder, Compression as Level};
    use std::io::Write;

    /// Builds an uncompressed SWF body: RECT, rate, count, then tags.
    pub fn body(
        width_px: i32,
        height_px: i32,
        fps: (u8, u8),
        frames: u16,
        tags: &[(u16, Vec<u8>)],
    ) -> Vec<u8> {
        let mut bits = BitWriter::default();
        let nbits = 16;
        bits.write(nbits, 5);
        bits.write(0, nbits);
        bits.write((width_px * 20) as u32, nbits);
        bits.write(0, nbits);
        bits.write((height_px * 20) as u32, nbits);
        let mut b = bits.finish();
        b.push(fps.1); // fraction
        b.push(fps.0); // integer
        b.extend_from_slice(&frames.to_le_bytes());
        for (code, data) in tags {
            push_tag(&mut b, *code, data);
        }
        push_tag(&mut b, 0, &[]);
        b
    }

    pub fn push_tag(b: &mut Vec<u8>, code: u16, data: &[u8]) {
        if data.len() < 0x3f {
            b.extend_from_slice(&((code << 6) | data.len() as u16).to_le_bytes());
        } else {
            b.extend_from_slice(&((code << 6) | 0x3f).to_le_bytes());
            b.extend_from_slice(&(data.len() as u32).to_le_bytes());
        }
        b.extend_from_slice(data);
    }

    pub fn fws(version: u8, body: &[u8]) -> Vec<u8> {
        let mut f = b"FWS".to_vec();
        f.push(version);
        f.extend_from_slice(&((body.len() + 8) as u32).to_le_bytes());
        f.extend_from_slice(body);
        f
    }

    fn cws(version: u8, body: &[u8]) -> Vec<u8> {
        let mut f = b"CWS".to_vec();
        f.push(version);
        f.extend_from_slice(&((body.len() + 8) as u32).to_le_bytes());
        let mut enc = ZlibEncoder::new(Vec::new(), Level::default());
        enc.write_all(body).unwrap();
        f.extend_from_slice(&enc.finish().unwrap());
        f
    }

    fn zws(version: u8, body: &[u8]) -> Vec<u8> {
        let mut lz = Vec::new();
        let opts = lzma_rs::compress::Options {
            unpacked_size: lzma_rs::compress::UnpackedSize::WriteToHeader(Some(body.len() as u64)),
        };
        lzma_rs::lzma_compress_with_options(&mut std::io::Cursor::new(body), &mut lz, &opts).unwrap();
        let props = &lz[0..5];
        let data = &lz[13..];
        let mut f = b"ZWS".to_vec();
        f.push(version);
        f.extend_from_slice(&((body.len() + 8) as u32).to_le_bytes());
        f.extend_from_slice(&(data.len() as u32).to_le_bytes());
        f.extend_from_slice(props);
        f.extend_from_slice(data);
        f
    }

    #[derive(Default)]
    struct BitWriter {
        out: Vec<u8>,
        cur: u8,
        n: u32,
    }
    impl BitWriter {
        fn write(&mut self, value: u32, bits: u32) {
            for i in (0..bits).rev() {
                self.cur = (self.cur << 1) | ((value >> i) & 1) as u8;
                self.n += 1;
                if self.n == 8 {
                    self.out.push(self.cur);
                    self.cur = 0;
                    self.n = 0;
                }
            }
        }
        fn finish(mut self) -> Vec<u8> {
            if self.n > 0 {
                self.out.push(self.cur << (8 - self.n));
            }
            self.out
        }
    }

    #[test]
    fn parses_uncompressed_header() {
        let b = body(550, 400, (24, 0), 12, &[(9, vec![0x33, 0x66, 0x99])]);
        let swf = Swf::parse(&fws(6, &b)).unwrap();
        assert_eq!(swf.info.compression, Compression::None);
        assert_eq!(swf.info.version, 6);
        assert_eq!(swf.info.width, 550.0);
        assert_eq!(swf.info.height, 400.0);
        assert_eq!(swf.info.frame_rate, 24.0);
        assert_eq!(swf.info.frame_count, 12);
        assert_eq!(swf.info.as_version, 1);
        assert_eq!(swf.info.background.as_deref(), Some("#336699"));
    }

    #[test]
    fn fractional_frame_rate() {
        let b = body(100, 100, (29, 0x80), 1, &[]);
        let swf = Swf::parse(&fws(8, &b)).unwrap();
        assert_eq!(swf.info.frame_rate, 29.5);
    }

    #[test]
    fn parses_zlib() {
        let b = body(640, 480, (30, 0), 1, &[(12, vec![0])]);
        let swf = Swf::parse(&cws(8, &b)).unwrap();
        assert_eq!(swf.info.compression, Compression::Zlib);
        assert_eq!(swf.info.width, 640.0);
        assert_eq!(swf.info.as_version, 2);
    }

    #[test]
    fn parses_lzma() {
        let b = body(
            800,
            600,
            (60, 0),
            3,
            &[(69, vec![0x08, 0, 0, 0]), (82, vec![1, 2, 3])],
        );
        let swf = Swf::parse(&zws(13, &b)).unwrap();
        assert_eq!(swf.info.compression, Compression::Lzma);
        assert_eq!(swf.info.height, 600.0);
        assert_eq!(swf.info.frame_count, 3);
        assert_eq!(swf.info.as_version, 3);
    }

    #[test]
    fn as3_from_file_attributes_and_network_flag() {
        let b = body(10, 10, (12, 0), 1, &[(69, vec![0x09, 0, 0, 0])]);
        let swf = Swf::parse(&fws(10, &b)).unwrap();
        assert_eq!(swf.info.as_version, 3);
        assert!(swf.info.uses_network);
    }

    #[test]
    fn long_tag_headers() {
        let big = vec![0u8; 300];
        let b = body(10, 10, (12, 0), 1, &[(12, big), (9, vec![1, 2, 3])]);
        let swf = Swf::parse(&fws(7, &b)).unwrap();
        assert_eq!(swf.info.background.as_deref(), Some("#010203"));
    }

    #[test]
    fn metadata_title_is_read() {
        let xml = r#"<rdf:RDF><rdf:Description><dc:title><rdf:Alt><rdf:li xml:lang="x-default">Paper &amp; Planes</rdf:li></rdf:Alt></dc:title></rdf:Description></rdf:RDF>"#;
        let mut data = xml.as_bytes().to_vec();
        data.push(0);
        let b = body(10, 10, (12, 0), 1, &[(77, data)]);
        let swf = Swf::parse(&fws(9, &b)).unwrap();
        assert_eq!(swf.info.meta_title.as_deref(), Some("Paper & Planes"));
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(Swf::parse(b"GIF89a....").unwrap_err(), SwfError::BadSignature);
        assert_eq!(Swf::parse(b"FWS").unwrap_err(), SwfError::TooShort);
        let mut huge = b"CWS\x0a".to_vec();
        huge.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(Swf::parse(&huge).unwrap_err(), SwfError::TooLarge(_)));
    }

    #[test]
    fn tolerates_truncated_tag_stream() {
        let mut b = body(10, 10, (12, 0), 1, &[(9, vec![1, 2, 3])]);
        b.truncate(b.len() - 2); // chop the End tag
        b.extend_from_slice(&[0xff, 0xff, 0x01]); // junk long-tag header
        let swf = Swf::parse(&fws(7, &b)).unwrap();
        assert_eq!(swf.info.background.as_deref(), Some("#010203"));
    }
}

/// Runs the parser over a folder of real SWFs and prints what it finds.
/// `SWF_SAMPLES=/path/to/swfs cargo test real_files -- --ignored --nocapture`
#[cfg(test)]
mod real_files {
    use super::*;

    #[test]
    #[ignore]
    fn real_files() {
        let Ok(dir) = std::env::var("SWF_SAMPLES") else {
            return;
        };
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        entries.sort();
        for p in entries
            .iter()
            .filter(|p| p.extension().is_some_and(|e| e == "swf"))
        {
            let bytes = std::fs::read(p).unwrap();
            match Swf::parse(&bytes) {
                Ok(swf) => {
                    let imgs = extract_images(&swf, 6);
                    let i = &swf.info;
                    println!(
                        "{:40} v{:<2} {:?} {}x{} {}fps {}f AS{} imgs={:?}",
                        p.file_name().unwrap().to_string_lossy(),
                        i.version,
                        i.compression,
                        i.width,
                        i.height,
                        i.frame_rate,
                        i.frame_count,
                        i.as_version,
                        imgs.iter().map(|m| (m.width, m.height)).collect::<Vec<_>>()
                    );
                }
                Err(e) => println!("{:40} ERROR {e}", p.file_name().unwrap().to_string_lossy()),
            }
        }
    }
}
