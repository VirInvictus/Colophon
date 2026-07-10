//! The Preferences window. One group for now, the theme picker: a
//! dropdown of "Follow system" plus every palette in `theme::THEMES`.
//! Selecting one persists it to GSettings and applies it live via the
//! window.

use gtk::glib;
use gtk::prelude::*;

use crate::theme;
use crate::ui::window::ColophonWindow;

/// Dropdown index 0 is "Follow system"; the rest map to `theme::THEMES`.
fn selection_for_index(index: u32) -> String {
    if index == 0 {
        theme::SYSTEM_ID.to_string()
    } else {
        theme::THEMES
            .get((index - 1) as usize)
            .map(|t| t.id.to_string())
            .unwrap_or_else(|| theme::SYSTEM_ID.to_string())
    }
}

fn index_for_selection(selection: &str) -> u32 {
    if selection == theme::SYSTEM_ID {
        0
    } else {
        theme::THEMES
            .iter()
            .position(|t| t.id == selection)
            .map(|i| i as u32 + 1)
            .unwrap_or(0)
    }
}

pub fn present(window: &ColophonWindow) {
    let names = gtk::StringList::new(&["Follow system"]);
    for t in theme::THEMES {
        names.append(t.name);
    }
    let dropdown = gtk::DropDown::builder()
        .model(&names)
        .selected(index_for_selection(&crate::settings::theme()))
        .valign(gtk::Align::Center)
        .build();
    dropdown.connect_selected_notify(glib::clone!(
        #[weak]
        window,
        move |dd| {
            window.apply_theme(&selection_for_index(dd.selected()));
        }
    ));

    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    list.append(&crate::ui::rows::row(
        "Theme",
        None,
        Some(dropdown.upcast_ref()),
    ));

    let heading = gtk::Label::builder()
        .label("Appearance")
        .xalign(0.0)
        .css_classes(["heading"])
        .build();
    let description = gtk::Label::builder()
        .label("Charts and the whole window follow the chosen palette.")
        .xalign(0.0)
        .wrap(true)
        .css_classes(["caption", "dim-label"])
        .build();

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .margin_top(14)
        .margin_bottom(14)
        .margin_start(14)
        .margin_end(14)
        .build();
    content.append(&heading);
    content.append(&description);
    content.append(&list);

    let prefs = gtk::Window::builder()
        .title("Preferences")
        .transient_for(window)
        .modal(true)
        .default_width(420)
        .child(&content)
        .build();
    super::close_on_escape(&prefs);
    prefs.present();
}
