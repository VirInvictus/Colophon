//! Colophon: a native GTK4/libadwaita statistics viewer for KOReader.
//!
//! This is Phase 1 scaffolding only — an empty shell window to prove the
//! toolchain, not a UI design. See `../roadmap.md`.

use adw::prelude::*;
use gtk::glib;

const APP_ID: &str = "org.virinvictus.Colophon";

fn main() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &adw::Application) {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Colophon")
        .default_width(900)
        .default_height(640)
        .build();

    let status = adw::StatusPage::builder()
        .title("Colophon")
        .description("Reading statistics viewer — scaffolding only")
        .icon_name("org.virinvictus.Colophon-symbolic")
        .build();

    window.set_content(Some(&status));
    window.present();
}
