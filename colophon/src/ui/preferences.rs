//! The Preferences dialog. One group for now, the theme picker: a combo of
//! "Follow system" plus every palette in `theme::THEMES`. Selecting one
//! persists it to GSettings and applies it live via the window.

use adw::prelude::*;
use gtk::glib;

use crate::theme;
use crate::ui::window::ColophonWindow;

/// Combo index 0 is "Follow system"; the rest map to `theme::THEMES`.
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
    let dialog = adw::PreferencesDialog::new();
    dialog.set_title("Preferences");

    let page = adw::PreferencesPage::new();
    let group = adw::PreferencesGroup::builder()
        .title("Appearance")
        .description("Charts and the whole window follow the chosen palette.")
        .build();

    let names = gtk::StringList::new(&["Follow system"]);
    for t in theme::THEMES {
        names.append(t.name);
    }

    let combo = adw::ComboRow::builder()
        .title("Theme")
        .model(&names)
        .selected(index_for_selection(&crate::settings::theme()))
        .build();

    combo.connect_selected_notify(glib::clone!(
        #[weak]
        window,
        move |row| {
            window.apply_theme(&selection_for_index(row.selected()));
        }
    ));

    group.add(&combo);
    page.add(&group);
    dialog.add(&page);
    dialog.present(Some(window));
}
