//! The library list: an "All Books" entry, then a boxed-list of books
//! with header rows for groups of same-title/author copies (two files of
//! one work). Full rebuild on change; the list is small and rebuilds are
//! far less fiddly than incremental header invalidation.

use std::cell::{Cell, RefCell};

use adw::subclass::prelude::*;
use gtk::glib;
use gtk::prelude::*;

use crate::library::LibraryGroup;
use crate::ui::book_row::BookRow;

/// What the content pane should show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Selection {
    #[default]
    Overview,
    /// Canonical book id (`Book::id`).
    Book(i64),
}

const ALL_BOOKS_NAME: &str = "all-books";

type SelectionHandler = Box<dyn Fn(Selection)>;

mod imp {
    use super::*;
    use gtk::CompositeTemplate;

    #[derive(CompositeTemplate, Default)]
    #[template(file = "library_view.ui")]
    pub struct LibraryView {
        #[template_child]
        pub list: TemplateChild<gtk::ListBox>,
        pub handler: RefCell<Option<super::SelectionHandler>>,
        /// Suppresses row-selected while set_groups rebuilds the list.
        pub rebuilding: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LibraryView {
        const NAME: &'static str = "LibraryView";
        type Type = super::LibraryView;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LibraryView {
        fn constructed(&self) {
            self.parent_constructed();
            let view = self.obj();
            self.list.connect_row_selected(glib::clone!(
                #[weak(rename_to = this)]
                view,
                move |_, row| {
                    if this.imp().rebuilding.get() {
                        return;
                    }
                    let Some(row) = row else { return };
                    let Some(selection) = selection_of(row) else {
                        return;
                    };
                    if let Some(handler) = this.imp().handler.borrow().as_ref() {
                        handler(selection);
                    }
                }
            ));
        }
    }
    impl WidgetImpl for LibraryView {}
    impl BinImpl for LibraryView {}
}

glib::wrapper! {
    pub struct LibraryView(ObjectSubclass<imp::LibraryView>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

fn selection_of(row: &gtk::ListBoxRow) -> Option<Selection> {
    if row.widget_name() == ALL_BOOKS_NAME {
        return Some(Selection::Overview);
    }
    row.downcast_ref::<BookRow>()
        .and_then(|r| r.entry())
        .map(|e| Selection::Book(e.book.id))
}

impl LibraryView {
    pub fn set_selection_handler(&self, handler: impl Fn(Selection) + 'static) {
        self.imp().handler.replace(Some(Box::new(handler)));
    }

    /// Rebuilds the list and restores the selected row.
    pub fn set_groups(&self, groups: &[LibraryGroup], selected: Selection) {
        let imp = self.imp();
        imp.rebuilding.set(true);
        let list = &imp.list;
        list.remove_all();

        list.append(&all_books_row());

        for group in groups {
            if group.is_multi() {
                list.append(&group_header(group));
                for entry in &group.entries {
                    list.append(&BookRow::new(entry, true));
                }
            } else if let Some(entry) = group.entries.first() {
                list.append(&BookRow::new(entry, false));
            }
        }

        // Restore selection (fall back to All Books).
        let mut index = 0;
        let mut fallback = None;
        while let Some(row) = list.row_at_index(index) {
            match selection_of(&row) {
                Some(s) if s == selected => {
                    list.select_row(Some(&row));
                    fallback = None;
                    break;
                }
                Some(Selection::Overview) => fallback = Some(row),
                _ => {}
            }
            index += 1;
        }
        if let Some(row) = fallback {
            list.select_row(Some(&row));
        }
        imp.rebuilding.set(false);
    }
}

fn all_books_row() -> gtk::ListBoxRow {
    let label = gtk::Label::builder()
        .label("All Books")
        .xalign(0.0)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(12)
        .margin_end(12)
        .css_classes(["heading"])
        .build();
    gtk::ListBoxRow::builder()
        .name(ALL_BOOKS_NAME)
        .child(&label)
        .build()
}

fn group_header(group: &LibraryGroup) -> gtk::ListBoxRow {
    let text = if group.authors.is_empty() {
        format!("{} \u{b7} {} copies", group.title, group.entries.len())
    } else {
        format!(
            "{} \u{b7} {} \u{b7} {} copies",
            group.title,
            group.authors,
            group.entries.len()
        )
    };
    let label = gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .margin_top(8)
        .margin_bottom(4)
        .margin_start(12)
        .margin_end(12)
        .build();
    let row = gtk::ListBoxRow::builder()
        .selectable(false)
        .activatable(false)
        .child(&label)
        .build();
    row.add_css_class("group-header");
    row
}
