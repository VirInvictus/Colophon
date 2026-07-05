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

    // Library folder for `.sdr` sidecars (the device's declared finished
    // status). Read-only and optional.
    let lib_group = adw::PreferencesGroup::builder()
        .title("Library")
        .description(
            "Point Colophon at the folder holding your books and their .sdr \
             sidecars to use the device's own finished status. It is read \
             only; leave it empty to infer finished from reading position.",
        )
        .build();

    let folder_row = adw::ActionRow::builder()
        .title("KOReader library folder")
        .build();
    let update_row = std::rc::Rc::new({
        let row = folder_row.downgrade();
        move || {
            if let Some(row) = row.upgrade() {
                match crate::settings::library_dir() {
                    Some(p) => row.set_subtitle(&p.display().to_string()),
                    None => row.set_subtitle("Not set; finished status is inferred"),
                }
            }
        }
    });
    update_row();

    let clear = gtk::Button::builder()
        .icon_name("edit-clear-symbolic")
        .valign(gtk::Align::Center)
        .tooltip_text("Clear")
        .css_classes(["flat"])
        .build();
    let choose = gtk::Button::builder()
        .label("Choose\u{2026}")
        .valign(gtk::Align::Center)
        .build();
    folder_row.add_suffix(&clear);
    folder_row.add_suffix(&choose);

    let uc = update_row.clone();
    clear.connect_clicked(glib::clone!(
        #[weak]
        window,
        move |_| {
            window.set_library_dir(None);
            uc();
        }
    ));
    choose.connect_clicked(glib::clone!(
        #[weak]
        window,
        move |_| {
            let picker = gtk::FileDialog::builder()
                .title("Choose KOReader library folder")
                .build();
            let uc = update_row.clone();
            picker.select_folder(
                Some(&window),
                gtk::gio::Cancellable::NONE,
                glib::clone!(
                    #[weak]
                    window,
                    move |result| {
                        if let Ok(folder) = result
                            && let Some(path) = folder.path()
                        {
                            window.set_library_dir(Some(path));
                            uc();
                        }
                    }
                ),
            );
        }
    ));

    lib_group.add(&folder_row);
    page.add(&lib_group);

    dialog.add(&page);
    dialog.present(Some(window));
}
