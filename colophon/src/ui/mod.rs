pub mod actions;
pub mod book_page;
pub mod book_row;
pub mod clamp;
pub mod library_view;
pub mod overview_page;
pub mod preferences;
pub mod rows;
pub mod shortcuts;
pub mod window;

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;

/// Close `window` on Escape. Plain gtk::Window has no built-in Escape
/// handling; the adw dialogs this replaces did it for free.
pub fn close_on_escape(window: &gtk::Window) {
    let key = gtk::EventControllerKey::new();
    key.connect_key_pressed(glib::clone!(
        #[weak]
        window,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, keyval, _, _| {
            if keyval == gdk::Key::Escape {
                window.close();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        }
    ));
    window.add_controller(key);
}
