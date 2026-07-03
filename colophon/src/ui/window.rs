//! The main window: import/refresh flows, banner, toasts, and the
//! library stack. All database work happens in `crate::loader` on
//! blocking threads; results hop back here via weak refs.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};

use crate::library::LibraryEntry;
use crate::loader::LibrarySnapshot;
use crate::{library, loader, paths, settings};

mod imp {
    use super::*;
    use crate::ui::book_row::BookRow;
    use crate::ui::library_view::LibraryView;
    use gtk::CompositeTemplate;

    #[derive(CompositeTemplate, Default)]
    #[template(file = "window.ui")]
    pub struct ColophonWindow {
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub split_view: TemplateChild<adw::NavigationSplitView>,
        #[template_child]
        pub schema_banner: TemplateChild<adw::Banner>,
        #[template_child]
        pub library_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub library_view: TemplateChild<LibraryView>,
        #[template_child]
        pub refresh_button: TemplateChild<gtk::Button>,
        /// Unfiltered master copy; refilter() derives the visible groups.
        pub entries: RefCell<Vec<LibraryEntry>>,
        /// Reentrancy guard for import/refresh.
        pub loading: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ColophonWindow {
        const NAME: &'static str = "ColophonWindow";
        type Type = super::ColophonWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            LibraryView::ensure_type();
            BookRow::ensure_type();
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ColophonWindow {
        fn constructed(&self) {
            self.parent_constructed();
            let window = self.obj();
            crate::ui::actions::install_window_actions(&window);
            window.restore_geometry();
        }
    }

    impl WidgetImpl for ColophonWindow {}

    impl WindowImpl for ColophonWindow {
        fn close_request(&self) -> glib::Propagation {
            self.obj().save_geometry();
            self.parent_close_request()
        }
    }

    impl ApplicationWindowImpl for ColophonWindow {}
    impl AdwApplicationWindowImpl for ColophonWindow {}
}

glib::wrapper! {
    pub struct ColophonWindow(ObjectSubclass<imp::ColophonWindow>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl ColophonWindow {
    pub fn new(app: &adw::Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    pub fn show_toast(&self, message: &str) {
        self.imp().toast_overlay.add_toast(adw::Toast::new(message));
    }

    /// Kick the initial load: open the existing snapshot if there is one,
    /// otherwise show the empty state.
    pub fn startup_load(&self) {
        let snapshot = paths::snapshot_path();
        if !snapshot.exists() {
            self.imp().library_stack.set_visible_child_name("empty");
            return;
        }
        self.imp().library_stack.set_visible_child_name("loading");
        let weak = self.downgrade();
        glib::spawn_future_local(async move {
            let result = gio::spawn_blocking(move || loader::load_snapshot(&snapshot)).await;
            let Some(window) = weak.upgrade() else { return };
            match result {
                Ok(Ok(snap)) => window.apply_snapshot(snap),
                Ok(Err(err)) => {
                    window.imp().library_stack.set_visible_child_name("empty");
                    window.show_toast(&format!("Couldn't read saved statistics: {err:#}"));
                }
                Err(_) => window.show_toast("Background load crashed"),
            }
        });
    }

    pub fn act_import(&self) {
        if self.imp().loading.get() {
            return;
        }
        let sqlite = gtk::FileFilter::new();
        sqlite.set_name(Some("SQLite databases"));
        sqlite.add_pattern("*.sqlite3");
        sqlite.add_pattern("*.sqlite");
        sqlite.add_pattern("*.db");
        let all = gtk::FileFilter::new();
        all.set_name(Some("All files"));
        all.add_pattern("*");
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&sqlite);
        filters.append(&all);

        let dialog = gtk::FileDialog::builder()
            .title("Choose a KOReader statistics.sqlite3")
            .filters(&filters)
            .build();
        if let Some(dir) = settings::source_path().and_then(|p| p.parent().map(PathBuf::from)) {
            dialog.set_initial_folder(Some(&gio::File::for_path(dir)));
        }

        let weak = self.downgrade();
        dialog.open(Some(self), gio::Cancellable::NONE, move |result| {
            let Some(window) = weak.upgrade() else { return };
            match result {
                Ok(file) => match file.path() {
                    Some(path) => window.start_import(path),
                    None => window.show_toast("Only local files are supported"),
                },
                Err(err) if err.matches(gtk::DialogError::Dismissed) => {}
                Err(err) => window.show_toast(&format!("Couldn't open file: {err}")),
            }
        });
    }

    pub fn act_refresh(&self) {
        let Some(source) = settings::source_path() else {
            // Nothing remembered (or no schema installed): pick a file.
            self.act_import();
            return;
        };
        if !source.exists() {
            self.show_toast(&format!(
                "Source not found at {} \u{2014} is the device mounted?",
                source.display()
            ));
            return;
        }
        self.start_import(source);
    }

    fn start_import(&self, source: PathBuf) {
        if self.imp().loading.get() {
            return;
        }
        self.set_busy(true);
        if self.imp().entries.borrow().is_empty() {
            self.imp().library_stack.set_visible_child_name("loading");
        }

        let weak = self.downgrade();
        glib::spawn_future_local(async move {
            let task_source = source.clone();
            let result = gio::spawn_blocking(move || {
                loader::import(&task_source, &paths::staging_dir(), &paths::snapshot_path())
            })
            .await;

            let Some(window) = weak.upgrade() else { return };
            window.set_busy(false);
            match result {
                Ok(Ok(snap)) => {
                    // Persist the source only after a validated import.
                    if let Some(s) = settings::settings() {
                        let _ =
                            s.set_string(settings::KEY_SOURCE_PATH, &source.display().to_string());
                    }
                    let count = snap.entries.len();
                    let name = source
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| source.display().to_string());
                    window.apply_snapshot(snap);
                    window.show_toast(&format!("Imported {count} books from {name}"));
                }
                Ok(Err(err)) => {
                    if window.imp().entries.borrow().is_empty() {
                        window.imp().library_stack.set_visible_child_name("empty");
                    }
                    window.show_toast(&format!("Import failed: {err:#}"));
                }
                Err(_) => window.show_toast("Import crashed in the background"),
            }
        });
    }

    fn apply_snapshot(&self, snap: LibrarySnapshot) {
        let imp = self.imp();

        if snap.schema_version == colophon_core::EXPECTED_SCHEMA_VERSION {
            imp.schema_banner.set_revealed(false);
        } else {
            imp.schema_banner.set_title(&format!(
                "Unfamiliar schema version {} (expected {}); numbers may be off",
                snap.schema_version,
                colophon_core::EXPECTED_SCHEMA_VERSION
            ));
            imp.schema_banner.set_revealed(true);
        }

        let has_books = !snap.entries.is_empty();
        imp.entries.replace(snap.entries);
        self.refilter();
        imp.library_stack
            .set_visible_child_name(if has_books { "list" } else { "empty" });
    }

    /// Re-derives the visible groups from the unfiltered master list and
    /// the junk-filter action state. Pure recompute; no db round-trip.
    pub fn refilter(&self) {
        let junk_filter = self
            .lookup_action("junk-filter")
            .and_then(|a| a.state())
            .and_then(|v| v.get::<bool>())
            .unwrap_or(true);
        let groups = library::grouped(
            self.imp().entries.borrow().clone(),
            junk_filter,
            colophon_core::model::DEFAULT_JUNK_THRESHOLD_SECS,
        );
        self.imp().library_view.set_groups(&groups);
    }

    fn set_busy(&self, busy: bool) {
        self.imp().loading.set(busy);
        self.imp().refresh_button.set_sensitive(!busy);
    }

    fn restore_geometry(&self) {
        let Some(s) = settings::settings() else {
            self.set_default_size(1000, 700);
            return;
        };
        self.set_default_size(
            s.int(settings::KEY_WINDOW_WIDTH),
            s.int(settings::KEY_WINDOW_HEIGHT),
        );
        if s.boolean(settings::KEY_WINDOW_MAXIMIZED) {
            self.maximize();
        }
    }

    fn save_geometry(&self) {
        let Some(s) = settings::settings() else {
            return;
        };
        let _ = s.set_boolean(settings::KEY_WINDOW_MAXIMIZED, self.is_maximized());
        if !self.is_maximized() {
            let (width, height) = self.default_size();
            let _ = s.set_int(settings::KEY_WINDOW_WIDTH, width);
            let _ = s.set_int(settings::KEY_WINDOW_HEIGHT, height);
        }
    }
}
