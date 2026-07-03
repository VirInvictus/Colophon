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
