//! Device auto-pull support (spec "Device auto-pull"): sidecar origin
//! bookkeeping and the re-copy pass that runs before an automatic import.
//! Std-only and window-free so it stays unit-testable; the mount watch and
//! import trigger live in `ui::window`.

use std::fs;
use std::path::{Path, PathBuf};

/// Remember where a sidecar was attached from, as `<md5>.origin` beside the
/// cached `<md5>.lua`. Best-effort: auto-pull is an enhancement, so a
/// failure to record the origin must never fail the attach itself.
pub fn remember_origin(sidecar_dir: &Path, md5: &str, origin: &Path) {
    let _ = fs::create_dir_all(sidecar_dir);
    let _ = fs::write(
        sidecar_dir.join(format!("{}.origin", md5.to_lowercase())),
        origin.display().to_string(),
    );
}

/// Every remembered (md5, origin path) pair in the cache dir.
pub fn origins(sidecar_dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = fs::read_dir(sidecar_dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("origin") {
            continue;
        }
        let Some(md5) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if let Ok(raw) = fs::read_to_string(&path) {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                out.push((md5.to_string(), PathBuf::from(trimmed)));
            }
        }
    }
    out
}

/// Re-copy every cached sidecar whose remembered origin is readable and
/// still verifies (same md5 join as attach). Unreadable or mismatched
/// origins are skipped and the cache keeps its last good copy.
pub fn refresh_sidecars(sidecar_dir: &Path) -> usize {
    let mut refreshed = 0;
    for (md5, origin) in origins(sidecar_dir) {
        let Ok(meta) = colophon_core::sidecar::parse_sidecar_file(&origin) else {
            continue;
        };
        if let Some(file_md5) = &meta.partial_md5
            && !file_md5.eq_ignore_ascii_case(&md5)
        {
            continue;
        }
        let dest = sidecar_dir.join(format!("{md5}.lua"));
        if fs::copy(&origin, &dest).is_ok() {
            refreshed += 1;
        }
    }
    refreshed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("colophon-autopull-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_sidecar(path: &Path, md5: &str, status: &str) {
        fs::write(
            path,
            format!(
                "return {{ partial_md5_checksum = \"{md5}\", \
                 summary = {{ status = \"{status}\" }} }}"
            ),
        )
        .unwrap();
    }

    #[test]
    fn origin_round_trips() {
        let cache = temp_dir("roundtrip");
        remember_origin(
            &cache,
            "ABC123",
            Path::new("/mnt/Kindle/Books/x.sdr/metadata.epub.lua"),
        );
        let got = origins(&cache);
        assert_eq!(
            got,
            vec![(
                "abc123".to_string(),
                PathBuf::from("/mnt/Kindle/Books/x.sdr/metadata.epub.lua")
            )]
        );
        let _ = fs::remove_dir_all(&cache);
    }

    #[test]
    fn refresh_copies_verified_origins() {
        let cache = temp_dir("refresh");
        let device = temp_dir("refresh-device");
        let origin = device.join("metadata.epub.lua");
        write_sidecar(&origin, "abc123", "reading");
        remember_origin(&cache, "abc123", &origin);
        write_sidecar(&cache.join("abc123.lua"), "abc123", "reading");

        // Device copy advances; refresh must pull the new content in.
        write_sidecar(&origin, "abc123", "complete");
        assert_eq!(refresh_sidecars(&cache), 1);
        let cached = fs::read_to_string(cache.join("abc123.lua")).unwrap();
        assert!(cached.contains("complete"));
        let _ = fs::remove_dir_all(&cache);
        let _ = fs::remove_dir_all(&device);
    }

    #[test]
    fn refresh_skips_missing_and_mismatched_origins() {
        let cache = temp_dir("skip");
        let device = temp_dir("skip-device");

        // Origin vanished (device gone): skipped, cache untouched.
        remember_origin(&cache, "gone11", device.join("nope.lua").as_path());
        write_sidecar(&cache.join("gone11.lua"), "gone11", "reading");

        // Origin now holds a different book: skipped, cache untouched.
        let swapped = device.join("swapped.lua");
        write_sidecar(&swapped, "other9", "complete");
        remember_origin(&cache, "swap22", &swapped);
        write_sidecar(&cache.join("swap22.lua"), "swap22", "reading");

        assert_eq!(refresh_sidecars(&cache), 0);
        let kept = fs::read_to_string(cache.join("swap22.lua")).unwrap();
        assert!(kept.contains("reading"));
        let _ = fs::remove_dir_all(&cache);
        let _ = fs::remove_dir_all(&device);
    }
}
