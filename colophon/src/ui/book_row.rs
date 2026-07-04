//! One book in the library list.

use std::cell::RefCell;
use std::rc::Rc;

use adw::subclass::prelude::*;
use gtk::glib;
use gtk::prelude::*;

use crate::fmt::{humanize_secs, relative_date};
use crate::library::LibraryEntry;

mod imp {
    use super::*;
    use gtk::CompositeTemplate;

    #[derive(CompositeTemplate, Default)]
    #[template(file = "book_row.ui")]
    pub struct BookRow {
        #[template_child]
        pub title_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub authors_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub stats_label: TemplateChild<gtk::Label>,
        pub entry: RefCell<Option<Rc<LibraryEntry>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for BookRow {
        const NAME: &'static str = "BookRow";
        type Type = super::BookRow;
        type ParentType = gtk::ListBoxRow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for BookRow {}
    impl WidgetImpl for BookRow {}
    impl ListBoxRowImpl for BookRow {}
}

glib::wrapper! {
    pub struct BookRow(ObjectSubclass<imp::BookRow>)
        @extends gtk::ListBoxRow, gtk::Widget,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl BookRow {
    pub fn new(entry: &Rc<LibraryEntry>, in_group: bool) -> Self {
        let row: Self = glib::Object::new();
        row.bind(entry, in_group);
        row.imp().entry.replace(Some(Rc::clone(entry)));
        row
    }

    pub fn entry(&self) -> Option<Rc<LibraryEntry>> {
        self.imp().entry.borrow().clone()
    }

    fn bind(&self, entry: &LibraryEntry, in_group: bool) {
        let imp = self.imp();
        let book = &entry.book;

        if in_group {
            // Title/authors live on the group header; disambiguate the
            // copies by layout and file identity instead.
            let md5_short: String = book.md5.as_deref().unwrap_or("?").chars().take(8).collect();
            imp.title_label
                .set_text(&format!("{} pages \u{b7} {md5_short}", book.pages));
            imp.title_label.remove_css_class("heading");
            imp.authors_label.set_visible(false);
            self.add_css_class("group-member");
        } else {
            let title = if book.title.trim().is_empty() {
                "(untitled)"
            } else {
                book.title.trim()
            };
            imp.title_label.set_text(title);
            let authors = book.authors.trim();
            imp.authors_label.set_visible(!authors.is_empty());
            imp.authors_label.set_text(authors);
        }

        imp.stats_label.set_text(&format!(
            "{} \u{b7} {}/{} pages \u{b7} {}",
            humanize_secs(book.total_read_time),
            entry.unique_pages,
            book.pages,
            relative_date(book.last_open, chrono::Local::now()),
        ));
    }
}
