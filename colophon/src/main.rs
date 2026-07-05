//! Colophon: a native GTK4/libadwaita statistics viewer for KOReader.

mod charts;
mod fmt;
mod library;
mod loader;
mod paths;
mod settings;
mod stats;
mod theme;
mod ui;

use adw::prelude::*;
use gtk::glib;

use crate::ui::window::ColophonWindow;

const APP_ID: &str = "org.virinvictus.Colophon";

fn main() -> glib::ExitCode {
    // Dev-run GSettings shim: point gio at the schema build.rs compiled
    // into the top-level data/, unless the environment already provides one
    // (installed builds). Must happen before any gio call or thread spawn;
    // set_var is unsafe in Rust 2024 exactly because of racing threads, and
    // none exist yet on line one of main.
    let dev_schema_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../data");
    if std::env::var_os("GSETTINGS_SCHEMA_DIR").is_none()
        && std::path::Path::new(dev_schema_dir)
            .join("gschemas.compiled")
            .exists()
    {
        unsafe {
            std::env::set_var("GSETTINGS_SCHEMA_DIR", dev_schema_dir);
        }
    }

    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_startup(|app| {
        theme::load(&settings::theme());
        ui::actions::install_app_actions(app);
    });
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &adw::Application) {
    // Single-instance re-summon: present the existing window if one is up.
    if let Some(window) = app
        .windows()
        .into_iter()
        .find_map(|w| w.downcast::<ColophonWindow>().ok())
    {
        window.present();
        return;
    }
    let window = ColophonWindow::new(app);
    window.present();
    window.startup_load();
}
