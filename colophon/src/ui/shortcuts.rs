//! Hand-built keyboard-shortcuts reference (Ctrl+question / F1).
//! gtk::ShortcutsWindow is deprecated and its adwaita replacement left
//! with adwaita, so this is a plain modal window over the shared rows.

use gtk::prelude::*;

use crate::ui::rows;

const SHORTCUTS: &[(&str, &str)] = &[
    ("Import a statistics database", "Ctrl+O"),
    ("Refresh from the last source", "Ctrl+R or F5"),
    ("Show or hide the library sidebar", "F9"),
    ("Resize the sidebar by keyboard", "F8, then arrows"),
    ("Back to the library list", "Esc"),
    ("Preferences", "Ctrl+Comma"),
    ("Keyboard shortcuts", "Ctrl+? or F1"),
    ("Quit", "Ctrl+Q"),
];

pub fn present(parent: &impl IsA<gtk::Window>) {
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    for (what, keys) in SHORTCUTS {
        list.append(&rows::value_row(what, keys, None));
    }

    let heading = gtk::Label::builder()
        .label("Keyboard shortcuts")
        .xalign(0.0)
        .css_classes(["heading"])
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
    content.append(&list);

    let window = gtk::Window::builder()
        .title("Keyboard Shortcuts")
        .transient_for(parent)
        .modal(true)
        .default_width(420)
        .child(&content)
        .build();
    super::close_on_escape(&window);
    window.present();
}
