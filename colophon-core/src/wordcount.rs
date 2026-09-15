//! Word counting over user-provided EPUBs (spec.md "Words in book") and
//! KOReader's partial-MD5 file identity (spec.md "User-provided book
//! files (EPUB)"), which verifies that a picked file really is the book
//! it is attached to.
//!
//! The MD5 core is the RFC 1321 algorithm, hand-rolled rather than pulled
//! as a dependency: this crate uses it only to identify user-provided
//! files (never for security), and the golden vectors in the tests pin
//! it against the published digests. `partial_md5` translates the
//! on-device plugin source (`research/koreader-plugin-src` upstream,
//! `frontend/util.lua:1111`) literally: MD5 over 1024-byte samples taken
//! at file offsets `1024 * 4^i` for `i = -1..10`, concatenated in order,
//! an empty sample ending the walk.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{Context, Result};

// --- MD5 (RFC 1321) -----------------------------------------------------

/// The per-round shift amounts.
const S: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9,
    14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15,
    21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

/// `floor(2^32 * abs(sin(i + 1)))`, the RFC's K table, computed rather
/// than transcribed (f64's 52-bit mantissa represents these exactly
/// enough for the floor; the golden vectors confirm it).
fn k_table() -> [u32; 64] {
    let mut k = [0u32; 64];
    for (i, slot) in k.iter_mut().enumerate() {
        let v = ((i as f64 + 1.0).sin().abs() * 2f64.powf(32.0)).floor();
        *slot = v as u32;
    }
    k
}

struct Md5 {
    state: [u32; 4],
    /// Total bytes absorbed, before padding.
    len: u64,
    buf: [u8; 64],
    buflen: usize,
}

impl Default for Md5 {
    fn default() -> Self {
        Self {
            state: [0; 4],
            len: 0,
            buf: [0; 64],
            buflen: 0,
        }
    }
}

impl Md5 {
    fn new() -> Self {
        Self {
            state: [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476],
            ..Default::default()
        }
    }

    fn absorb(&mut self, mut data: &[u8]) {
        self.len = self.len.wrapping_add(data.len() as u64);
        if self.buflen > 0 {
            let take = (64 - self.buflen).min(data.len());
            self.buf[self.buflen..self.buflen + take].copy_from_slice(&data[..take]);
            self.buflen += take;
            data = &data[take..];
            if self.buflen == 64 {
                let block = self.buf;
                self.compress(&block);
                self.buflen = 0;
            }
        }
        while data.len() >= 64 {
            let (block, rest) = data.split_at(64);
            let mut b = [0u8; 64];
            b.copy_from_slice(block);
            self.compress(&b);
            data = rest;
        }
        if !data.is_empty() {
            self.buf[..data.len()].copy_from_slice(data);
            self.buflen = data.len();
        }
    }

    /// Absorbs without counting toward the length (padding).
    fn absorb_raw(&mut self, data: &[u8]) {
        let mut rest = data;
        while !rest.is_empty() {
            let take = (64 - self.buflen).min(rest.len());
            self.buf[self.buflen..self.buflen + take].copy_from_slice(&rest[..take]);
            self.buflen += take;
            rest = &rest[take..];
            if self.buflen == 64 {
                let block = self.buf;
                self.compress(&block);
                self.buflen = 0;
            }
        }
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let k = k_table();
        let mut m = [0u32; 16];
        for (i, word) in m.iter_mut().enumerate() {
            *word = u32::from_le_bytes([
                block[4 * i],
                block[4 * i + 1],
                block[4 * i + 2],
                block[4 * i + 3],
            ]);
        }
        let (mut a, mut b, mut c, mut d) =
            (self.state[0], self.state[1], self.state[2], self.state[3]);
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let tmp = d;
            d = c;
            c = b;
            let sum = a.wrapping_add(f).wrapping_add(k[i]).wrapping_add(m[g]);
            b = b.wrapping_add(sum.rotate_left(S[i]));
            a = tmp;
        }
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
    }

    fn finalize(mut self) -> String {
        let bit_len = self.len.wrapping_mul(8);
        self.absorb_raw(&[0x80]);
        while self.buflen != 56 {
            self.absorb_raw(&[0]);
        }
        let len_bytes = bit_len.to_le_bytes();
        self.absorb_raw(&len_bytes);

        let mut out = String::with_capacity(32);
        for word in self.state {
            for byte in word.to_le_bytes() {
                out.push_str(&format!("{byte:02x}"));
            }
        }
        out
    }
}

// --- KOReader's partial MD5 ----------------------------------------------

/// KOReader's partial MD5 of a document file: lowercase hex. Verifies a
/// user-provided EPUB against `book.md5` (spec.md "User-provided book
/// files (EPUB)").
pub fn partial_md5(path: &Path) -> Result<String> {
    let mut file =
        std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut hasher = Md5::new();
    for i in -1i32..=10 {
        let offset = if i == -1 { 256u64 } else { 1024u64 << (2 * i) };
        file.seek(SeekFrom::Start(offset))?;
        let mut sample = Vec::with_capacity(1024);
        (&mut file)
            .take(1024)
            .read_to_end(&mut sample)
            .context("reading a sample")?;
        if sample.is_empty() {
            break;
        }
        hasher.absorb(&sample);
    }
    Ok(hasher.finalize())
}

// --- Word counting --------------------------------------------------------

/// Words in an EPUB (spec.md "Words in book"): every HTML/XHTML document
/// in the container, markup stripped, Unicode word runs counted.
pub fn epub_word_count(path: &Path) -> Result<u64> {
    let file = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(file).context("reading the EPUB container")?;
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        if entry.is_dir() || !is_text_document(entry.name()) {
            continue;
        }
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes)?;
        total += count_words(&decode_text(bytes)) as u64;
    }
    Ok(total)
}

fn is_text_document(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".xhtml") || lower.ends_with(".html") || lower.ends_with(".htm")
}

/// EPUB text documents are UTF-8 or UTF-16; the BOM decides.
fn decode_text(bytes: Vec<u8>) -> String {
    if bytes.starts_with(&[0xFE, 0xFF]) || bytes.starts_with(&[0xFF, 0xFE]) {
        let little = bytes[0] == 0xFF;
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| {
                if little {
                    u16::from_le_bytes([c[0], c[1]])
                } else {
                    u16::from_be_bytes([c[0], c[1]])
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

/// Visible word count of an HTML document: `<style>`/`<script>` blocks
/// and markup removed, entities decoded, then Unicode word runs counted.
pub fn count_words(html: &str) -> usize {
    count_runs(&strip_markup(html))
}

/// A word is a maximal run of alphanumeric characters; an apostrophe or
/// hyphen inside a run keeps it one word ("don't", "state-of-the-art").
fn count_runs(text: &str) -> usize {
    let mut words = 0;
    let mut in_word = false;
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            if !in_word {
                words += 1;
                in_word = true;
            }
        } else if in_word && matches!(ch, '\'' | '\u{2019}' | '-' | '\u{2010}') {
            // A pause, not a break: the next letter continues the word.
        } else {
            in_word = false;
        }
    }
    words
}

fn strip_markup(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        let after = &rest[lt + 1..];
        let Some(gt) = after.find('>') else { break };
        let tag = &after[..gt];
        let name: String = tag
            .trim_start_matches('/')
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        rest = &after[gt + 1..];
        if !tag.starts_with('/') && (name == "style" || name == "script") {
            // Skip the block body; processing resumes at the closing
            // tag, which the loop consumes normally.
            let closer = format!("</{}", name);
            match rest.to_lowercase().find(&closer) {
                Some(pos) => rest = &rest[pos..],
                None => rest = "",
            }
        }
    }
    out.push_str(rest);
    decode_entities(&out)
}

fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        rest = &rest[pos..];
        let Some(semi) = rest.find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let name = &rest[1..semi];
        let decoded = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some('\u{a0}'),
            _ if name.len() > 2 && name.starts_with("#x") => u32::from_str_radix(&name[2..], 16)
                .ok()
                .and_then(char::from_u32),
            _ if name.len() > 1 && name.starts_with('#') => {
                name[1..].parse::<u32>().ok().and_then(char::from_u32)
            }
            // An unknown named entity is dropped, not passed through:
            // its letters must not count as words ("caf&eacute;" stays
            // one word, not two).
            _ => None,
        };
        match decoded {
            Some(ch) => {
                out.push(ch);
                rest = &rest[semi + 1..];
            }
            None => {
                rest = &rest[semi + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compress_matches_the_reference_block() {
        // The padded block for "abc" (IEEE test vector), compressed once
        // from the initial state.
        let block: [u8; 64] = [
            97, 98, 99, 128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            24, 0, 0, 0, 0, 0, 0, 0,
        ];
        let mut h = Md5::new();
        h.compress(&block);
        assert_eq!(h.state, [0x98500190, 0xb04fd23c, 0x7d3f96d6, 0x727fe128]);
    }

    #[test]
    fn md5_matches_the_published_vectors() {
        let digest = |data: &[u8]| {
            let mut h = Md5::new();
            h.absorb(data);
            h.finalize()
        };
        println!("empty: {}", digest(b""));
        println!("abc:   {}", digest(b"abc"));
        assert_eq!(digest(b""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(digest(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(
            digest(b"The quick brown fox jumps over the lazy dog"),
            "9e107d9d372bb6826bd81d3542a419d6"
        );
        // RFC 1321's million-'a' vector, the padding-edge case.
        let million = vec![b'a'; 1_000_000];
        assert_eq!(digest(&million), "7707d6ae4e027c70eea2a935c2296f21");
    }

    #[test]
    fn partial_md5_walks_the_documented_offsets() {
        // Golden vector computed by the same literal translation in
        // Python against a deterministic 70,000-byte pattern file
        // (buf[i] = i % 251); offsets 256, 1024, 4096, 16384, 65536
        // carry samples, everything past 65536+1024 is past EOF.
        let path = std::env::temp_dir().join(format!("colophon-partial-{}", std::process::id()));
        std::fs::write(
            &path,
            (0..70_000u32).map(|i| (i % 251) as u8).collect::<Vec<_>>(),
        )
        .unwrap();
        let got = partial_md5(&path).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(got, "2a0b98face1749afa3fdda4fc822b995");
    }

    #[test]
    fn count_words_strips_markup_and_counts_runs() {
        let doc = "<html><head><style>p { color: red }</style></head>\
                   <body><script>var x = 1 && 2;</script>\
                   <p>Don&#8217;t stop&#8212;now: it&#39;s state-of-the-art, caf&#233; time.</p>\
                   </body></html>";
        // don't | stop now(it counts as one hyphenated run? the em dash
        // is not a hyphen: "stop" and "now" are two) | it's |
        // state-of-the-art | café | time => 7 words.
        assert_eq!(count_words(doc), 7);
    }

    #[test]
    fn unknown_named_entities_do_not_become_words() {
        // &eacute; is outside the small named table: it is dropped, so
        // the accented word still counts once (numeric refs decode).
        assert_eq!(count_words("<p>caf&eacute;</p>"), 1);
        assert_eq!(count_words("<p>caf&#233;</p>"), 1);
        assert_eq!(count_words("<p>a &amp; b</p>"), 2);
    }

    #[test]
    fn epub_word_count_reads_the_documents() {
        let path = std::env::temp_dir().join(format!("colophon-epub-{}.epub", std::process::id()));
        let file = std::fs::File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options: zip::write::SimpleFileOptions = Default::default();
        zip.start_file("mimetype", options).unwrap();
        std::io::Write::write_all(&mut zip, b"application/epub+zip").unwrap();
        zip.start_file("OEBPS/content.opf", options).unwrap();
        std::io::Write::write_all(&mut zip, b"<package/>").unwrap();
        zip.start_file("OEBPS/ch1.xhtml", options).unwrap();
        std::io::Write::write_all(
            &mut zip,
            b"<html><body><p>one two three four</p></body></html>",
        )
        .unwrap();
        zip.start_file("OEBPS/ch2.xhtml", options).unwrap();
        std::io::Write::write_all(
            &mut zip,
            b"<html><style>p{}</style><body><p>five six</p></body></html>",
        )
        .unwrap();
        zip.start_file("OEBPS/cover.jpg", options).unwrap();
        std::io::Write::write_all(&mut zip, b"\xff\xd8\xff").unwrap();
        drop(zip);

        let got = epub_word_count(&path).unwrap();
        std::fs::remove_file(&path).ok();
        // The OPF and the JPEG are not text documents; the style block
        // is not words.
        assert_eq!(got, 6);
    }
}
