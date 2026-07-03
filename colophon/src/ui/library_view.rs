//! The library list: a boxed-list of books, with header rows for groups
//! of same-title/author copies (the Jingo case). Full rebuild on change;
//! the list is small and rebuilds are far less fiddly than incremental
//! header invalidation.

use adw::subclass::prelude::*;
use gtk::glib;
use gtk::prelude::*;

use crate::library::LibraryGroup;
use crate::ui::book_row::BookRow;

mod imp {
    use super::*;
    use gtk::CompositeTemplate;

    #[derive(CompositeTemplate, Default)]
    #[template(file = "library_view.ui")]
    pub struct LibraryView {
        #[template_child]
        pub list: TemplateChild<gtk::ListBox>,
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

    impl ObjectImpl for LibraryView {}
    impl WidgetImpl for LibraryView {}
    impl BinImpl for LibraryView {}
}

glib::wrapper! {
    pub struct LibraryView(ObjectSubclass<imp::LibraryView>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl LibraryView {
    pub fn set_groups(&self, groups: &[LibraryGroup]) {
        let list = &self.imp().list;
        list.remove_all();

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
    }
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
