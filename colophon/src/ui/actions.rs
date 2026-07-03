//! Action wiring, Viaduct-style: a small register helper for window
//! actions whose bodies are `ColophonWindow` methods, app-level actions,
//! and the accelerator table.

use adw::prelude::*;
use gtk::gio;

use crate::settings;
use crate::ui::window::ColophonWindow;

fn register<F>(window: &ColophonWindow, name: &str, handler: F)
where
    F: Fn(&ColophonWindow) + 'static,
{
    let action = gio::SimpleAction::new(name, None);
    let weak = window.downgrade();
    action.connect_activate(move |_, _| {
        if let Some(window) = weak.upgrade() {
            handler(&window);
        }
    });
    window.add_action(&action);
}

pub fn install_window_actions(window: &ColophonWindow) {
    register(window, "import", ColophonWindow::act_import);
    register(window, "refresh", ColophonWindow::act_refresh);

    // Junk filter: GSettings-backed when the schema is installed (persists
    // and stays in sync), a plain stateful action otherwise (defaults on,
    // forgets on exit — the degraded no-schema mode).
    match settings::settings() {
        Some(s) => {
            window.add_action(&s.create_action(settings::KEY_JUNK_FILTER));
            let weak = window.downgrade();
            s.connect_changed(Some(settings::KEY_JUNK_FILTER), move |_, _| {
                if let Some(window) = weak.upgrade() {
                    window.refilter();
                }
            });
        }
        None => {
            let action = gio::SimpleAction::new_stateful(
                settings::KEY_JUNK_FILTER,
                None,
                &true.to_variant(),
            );
            let weak = window.downgrade();
            action.connect_activate(move |action, _| {
                let current = action.state().and_then(|v| v.get::<bool>()).unwrap_or(true);
                action.set_state(&(!current).to_variant());
                if let Some(window) = weak.upgrade() {
                    window.refilter();
                }
            });
            window.add_action(&action);
        }
    }
}

pub fn install_app_actions(app: &adw::Application) {
    let quit = gio::SimpleAction::new("quit", None);
    let weak = app.downgrade();
    quit.connect_activate(move |_, _| {
        if let Some(app) = weak.upgrade() {
            app.quit();
        }
    });
    app.add_action(&quit);

    let about = gio::SimpleAction::new("about", None);
    let weak = app.downgrade();
    about.connect_activate(move |_, _| {
        let Some(app) = weak.upgrade() else { return };
        let dialog = adw::AboutDialog::builder()
            .application_name("Colophon")
            .application_icon("org.virinvictus.Colophon")
            .developer_name("Brandon LaRocque")
            .version(env!("CARGO_PKG_VERSION"))
            .website("https://github.com/VirInvictus/Colophon")
            .license_type(gtk::License::MitX11)
            .comments("A reading-statistics viewer for KOReader")
            .build();
        dialog.present(app.active_window().as_ref());
    });
    app.add_action(&about);

    app.set_accels_for_action("win.import", &["<Ctrl>o"]);
    app.set_accels_for_action("win.refresh", &["<Ctrl>r", "F5"]);
    app.set_accels_for_action("app.quit", &["<Ctrl>q"]);
}
