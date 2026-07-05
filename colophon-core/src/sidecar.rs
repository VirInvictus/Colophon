//! Reads KOReader's per-book `.sdr` sidecar: the Lua table KOReader writes
//! next to each book (`<book>.sdr/metadata.<ext>.lua`). The stats DB carries
//! no user-declared "finished" flag; the sidecar's `summary.status` is the
//! only source of one, alongside `percent_finished` and the
//! `partial_md5_checksum` that joins back to `book.md5`.
//!
//! The chunk is arbitrary user data (highlight text, notes), so it runs in a
//! locked-down `mlua` VM: no standard library (`StdLib::NONE`, hence no
//! `os`/`io`/`require`), text chunks only (never precompiled bytecode), and
//! invalid UTF-8 is repaired lossily rather than dropping the whole file.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use mlua::{ChunkMode, Lua, LuaOptions, StdLib, Table};

/// The user-declared reading status from the sidecar's `summary.status`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadStatus {
    Reading,
    Complete,
    Abandoned,
    /// Any other value KOReader might write, preserved verbatim.
    Other(String),
}

impl ReadStatus {
    fn parse(s: &str) -> Self {
        match s {
            "reading" => Self::Reading,
            "complete" => Self::Complete,
            "abandoned" => Self::Abandoned,
            other => Self::Other(other.to_string()),
        }
    }

    /// Whether this declares the book finished.
    pub fn is_finished(&self) -> bool {
        matches!(self, Self::Complete)
    }
}

/// The fields Colophon reads from a sidecar. All optional: sidecars vary by
/// KOReader version and document type.
#[derive(Debug, Clone, PartialEq)]
pub struct SidecarMeta {
    /// Joins to `book.md5` (KOReader's partial MD5).
    pub partial_md5: Option<String>,
    /// Fraction in `[0, 1]` KOReader records as read.
    pub percent_finished: Option<f64>,
    /// The user-declared status, if any.
    pub status: Option<ReadStatus>,
}

/// Parses a sidecar from raw bytes (a Lua `return { ... }` chunk).
pub fn parse_sidecar_bytes(bytes: &[u8]) -> Result<SidecarMeta> {
    // KOReader can truncate highlight text mid-multibyte-character; repair
    // rather than reject the file (KoShelf does the same).
    let text = String::from_utf8_lossy(bytes);
    // mlua's error type is not Send + Sync, so it can't flow through anyhow's
    // `?`; keep the VM work in its own mlua::Result and stringify at the edge.
    eval_sidecar(&text).map_err(|e| anyhow::anyhow!("parsing sidecar: {e}"))
}

fn eval_sidecar(text: &str) -> mlua::Result<SidecarMeta> {
    let lua = Lua::new_with(StdLib::NONE, LuaOptions::default())?;
    let table: Table = lua.load(text).set_mode(ChunkMode::Text).eval()?;
    let partial_md5 = table.get::<Option<String>>("partial_md5_checksum")?;
    let percent_finished = table.get::<Option<f64>>("percent_finished")?;
    let status = match table.get::<Option<Table>>("summary")? {
        Some(summary) => summary
            .get::<Option<String>>("status")?
            .map(|s| ReadStatus::parse(&s)),
        None => None,
    };
    Ok(SidecarMeta {
        partial_md5,
        percent_finished,
        status,
    })
}

/// Parses a single `<book>.sdr/metadata.*.lua` file.
pub fn parse_sidecar_file(path: &Path) -> Result<SidecarMeta> {
    let bytes =
        std::fs::read(path).with_context(|| format!("reading sidecar {}", path.display()))?;
    parse_sidecar_bytes(&bytes)
}

/// Recursively scans `root` for `metadata.*.lua` sidecars, keyed by their
/// `partial_md5_checksum` (lowercased) for a direct join to `book.md5`.
/// Sidecars that fail to parse or carry no md5 are skipped, never fatal;
/// `*.lua.old` backups are ignored. The whole scan is read-only.
pub fn scan_sidecars(root: &Path) -> HashMap<String, SidecarMeta> {
    let mut out = HashMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if !is_sidecar_file(&path) {
                continue;
            }
            if let Ok(meta) = parse_sidecar_file(&path)
                && let Some(md5) = &meta.partial_md5
            {
                out.insert(md5.to_lowercase(), meta);
            }
        }
    }
    out
}

/// A `metadata.<ext>.lua` sidecar, but not a `.lua.old` backup.
fn is_sidecar_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|name| name.starts_with("metadata.") && name.ends_with(".lua"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_status_percent_and_md5() {
        // Synthetic sidecar (fabricated md5, not a real book's).
        let chunk = br#"
            return {
                ["partial_md5_checksum"] = "abc123def456",
                ["percent_finished"] = 1,
                ["summary"] = {
                    ["status"] = "complete",
                    ["modified"] = "2026-07-03",
                },
            }
        "#;
        let meta = parse_sidecar_bytes(chunk).unwrap();
        assert_eq!(meta.partial_md5.as_deref(), Some("abc123def456"));
        assert_eq!(meta.percent_finished, Some(1.0));
        assert_eq!(meta.status, Some(ReadStatus::Complete));
        assert!(meta.status.unwrap().is_finished());
    }

    #[test]
    fn tolerates_missing_fields_and_bad_utf8() {
        let meta = parse_sidecar_bytes(br#"return { ["percent_finished"] = 0.5 }"#).unwrap();
        assert_eq!(meta.partial_md5, None);
        assert_eq!(meta.status, None);
        assert_eq!(meta.percent_finished, Some(0.5));

        // Invalid UTF-8 inside a string is repaired, not fatal.
        let mut bytes = br#"return { ["partial_md5_checksum"] = "ab"#.to_vec();
        bytes.push(0xff);
        bytes.extend_from_slice(br#"" }"#);
        assert!(parse_sidecar_bytes(&bytes).is_ok());
    }

    #[test]
    fn read_status_parses_known_and_unknown() {
        let mk = |s: &str| {
            let chunk = format!(r#"return {{ ["summary"] = {{ ["status"] = "{s}" }} }}"#);
            parse_sidecar_bytes(chunk.as_bytes())
                .unwrap()
                .status
                .unwrap()
        };
        assert_eq!(mk("reading"), ReadStatus::Reading);
        assert_eq!(mk("complete"), ReadStatus::Complete);
        assert_eq!(mk("abandoned"), ReadStatus::Abandoned);
        assert_eq!(mk("dnf"), ReadStatus::Other("dnf".into()));
        assert!(!ReadStatus::Reading.is_finished());
        assert!(ReadStatus::Complete.is_finished());
    }

    /// The gitignored real sidecars copied from the device; skips cleanly
    /// when they are absent (matches the live-sample DB test).
    fn samples_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../research/samples")
    }

    #[test]
    fn parses_real_finished_sidecar_when_present() {
        let root = samples_dir();
        if !root.exists() {
            eprintln!("skipping: real sidecar samples absent");
            return;
        }
        let map = scan_sidecars(&root);
        // The Royal Assassin sidecar is a finished book with a real md5.
        assert!(
            map.values()
                .any(|m| m.status == Some(ReadStatus::Complete) && m.percent_finished == Some(1.0)),
            "expected a complete sidecar in the samples"
        );
        assert!(map.keys().all(|k| !k.is_empty()));
    }
}
