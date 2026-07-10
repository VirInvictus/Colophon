//! XDG path helpers. The app owns exactly one working database: the
//! canonical snapshot copy. Its name is fixed so startup never has to
//! remember where the last import landed; only the *source* path is
//! persisted (in GSettings, for Refresh).

use std::path::PathBuf;

pub fn app_data_dir() -> PathBuf {
    gtk::glib::user_data_dir().join("colophon")
}

/// The canonical working copy Colophon reads from.
pub fn snapshot_path() -> PathBuf {
    app_data_dir().join("statistics.sqlite3")
}

/// Imports land here first and are promoted only after validating, so a
/// bad pick can never clobber a good snapshot.
pub fn staging_dir() -> PathBuf {
    app_data_dir().join("import-tmp")
}

/// User-provided `.sdr` sidecars, one per book, named by the book's md5,
/// plus a `<md5>.origin` note of where each was attached from so auto-pull
/// can keep the copy fresh (spec "Device auto-pull"). Colophon copies here
/// what the user hands it and only ever re-reads those exact paths; a book
/// with no file here simply falls back to inferred stats.
pub fn sidecar_dir() -> PathBuf {
    app_data_dir().join("sidecars")
}

/// The cached sidecar path for a given book md5 (lowercased).
pub fn sidecar_for(md5: &str) -> PathBuf {
    sidecar_dir().join(format!("{}.lua", md5.to_lowercase()))
}
