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

/// Spin display: minutes past midnight as HH:MM.
fn format_day_start(spin: &gtk::SpinButton) {
    let minutes = spin.value().round() as u16;
    spin.set_text(&format!("{:02}:{:02}", minutes / 60, minutes % 60));
}

/// Spin input: accept "HH:MM" (a bare number is the default entry).
fn parse_day_start(spin: &gtk::SpinButton) -> Option<Result<f64, ()>> {
    let text = spin.text();
    let (h, m) = text.trim().split_once(':')?;
    let h: f64 = h.trim().parse().ok()?;
    let m: f64 = m.trim().parse().ok()?;
    Some(Ok(h * 60.0 + m))
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

    // Reading day start (spec.md "Day"): minutes past midnight, shown as
    // HH:MM. Arrows step 30 minutes; any minute can be typed.
    let saved = crate::settings::day_start();
    let adjustment = gtk::Adjustment::new(f64::from(saved.0), 0.0, 1439.0, 30.0, 60.0, 0.0);
    let spin = gtk::SpinButton::builder()
        .adjustment(&adjustment)
        .valign(gtk::Align::Center)
        .build();
    format_day_start(&spin);
    spin.connect_input(parse_day_start);
    spin.connect_output(|spin| {
        format_day_start(spin);
        glib::Propagation::Stop
    });
    spin.connect_value_changed(glib::clone!(
        #[weak]
        window,
        move |spin| {
            window.apply_day_start(spin.value().round() as u16);
        }
    ));

    let day_list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    day_list.append(&crate::ui::rows::row(
        "Reading day starts at",
        Some("Late-night reading before this time counts toward the previous day."),
        Some(spin.upcast_ref()),
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

    let day_heading = gtk::Label::builder()
        .label("Reading day")
        .xalign(0.0)
        .css_classes(["heading"])
        .build();
    let day_description = gtk::Label::builder()
        .label("Where a day begins and ends, for totals, streaks, the heatmap, and time windows.")
        .xalign(0.0)
        .wrap(true)
        .css_classes(["caption", "dim-label"])
        .build();
    content.append(&day_heading);
    content.append(&day_description);
    content.append(&day_list);

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
