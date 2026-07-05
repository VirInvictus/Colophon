//! The main window: import/refresh flows, banner, toasts, and the
//! library stack. All database work happens in `crate::loader` on
//! blocking threads; results hop back here via weak refs.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use chrono::Local;
use gtk::{gio, glib};

use crate::library::LibraryEntry;
use crate::loader::LibrarySnapshot;
use crate::ui::library_view::Selection;
use crate::{library, loader, paths, settings, stats};

mod imp {
    use super::*;
    use crate::ui::book_page::BookPage;
    use crate::ui::book_row::BookRow;
    use crate::ui::library_view::LibraryView;
    use crate::ui::overview_page::OverviewPage;
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
        #[template_child]
        pub content_page: TemplateChild<adw::NavigationPage>,
        #[template_child]
        pub content_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub overview_page: TemplateChild<OverviewPage>,
        #[template_child]
        pub book_page: TemplateChild<BookPage>,
        /// Unfiltered master copy; refilter() derives the visible groups.
        pub entries: RefCell<Vec<Rc<LibraryEntry>>>,
        /// Cached window-independent overview aggregates for the current
        /// filtered entry set, tagged with the date they were built for.
        /// Invalidated whenever the filtered set changes (junk toggle,
        /// re-import) so window toggles never recompute the expensive
        /// whole-history math. See `stats::OverviewBase`.
        pub overview_base: RefCell<Option<(chrono::NaiveDate, stats::OverviewBase)>>,
        pub selection: RefCell<Selection>,
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
            OverviewPage::ensure_type();
            BookPage::ensure_type();
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
            self.library_view.set_selection_handler(glib::clone!(
                #[weak]
                window,
                move |selection| window.on_select(selection)
            ));
            self.overview_page.set_on_window_changed(glib::clone!(
                #[weak]
                window,
                move || window.refresh_content()
            ));
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
        let library_dir = crate::settings::library_dir();
        let weak = self.downgrade();
        glib::spawn_future_local(async move {
            let result = gio::spawn_blocking(move || {
                loader::load_snapshot(&snapshot, library_dir.as_deref())
            })
            .await;
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

        let library_dir = crate::settings::library_dir();
        let weak = self.downgrade();
        glib::spawn_future_local(async move {
            let task_source = source.clone();
            let result = gio::spawn_blocking(move || {
                loader::import(
                    &task_source,
                    &paths::staging_dir(),
                    &paths::snapshot_path(),
                    library_dir.as_deref(),
                )
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
        imp.entries
            .replace(snap.entries.into_iter().map(Rc::new).collect());

        // A re-import may have dropped the selected book (or renumbered
        // ids); fall back to the overview rather than a stale page.
        let selection = *imp.selection.borrow();
        if let Selection::Book(id) = selection {
            let still_there = imp.entries.borrow().iter().any(|e| e.book.id == id);
            if !still_there {
                imp.selection.replace(Selection::Overview);
            }
        }

        self.refilter();
        imp.library_stack
            .set_visible_child_name(if has_books { "list" } else { "empty" });
    }

    /// Persist and apply a theme selection, then redraw the visible page
    /// so the cairo charts pick up the new palette (CSS-styled widgets
    /// restyle themselves when the provider swaps).
    pub fn apply_theme(&self, selection: &str) {
        if let Some(s) = settings::settings() {
            let _ = s.set_string(settings::KEY_THEME, selection);
        }
        crate::theme::set(selection);
        self.refresh_content();
    }

    /// Persist the KOReader library folder and reload from the canonical
    /// snapshot, so the sidecar-derived finished status is re-read (or
    /// dropped when cleared). A no-op path is stored as the empty string.
    pub fn set_library_dir(&self, dir: Option<std::path::PathBuf>) {
        if let Some(s) = settings::settings() {
            let value = dir
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let _ = s.set_string(settings::KEY_LIBRARY_DIR, &value);
        }
        self.startup_load();
    }

    fn junk_filter_on(&self) -> bool {
        self.lookup_action("junk-filter")
            .and_then(|a| a.state())
            .and_then(|v| v.get::<bool>())
            .unwrap_or(true)
    }

    /// The entries the current junk-filter state admits. Library-wide
    /// widgets respect the filter too (spec.md "Junk filter").
    fn filtered_entries(&self) -> Vec<Rc<LibraryEntry>> {
        let junk_filter = self.junk_filter_on();
        self.imp()
            .entries
            .borrow()
            .iter()
            .filter(|e| {
                !junk_filter
                    || !e
                        .book
                        .is_junk(colophon_core::model::DEFAULT_JUNK_THRESHOLD_SECS)
            })
            .cloned()
            .collect()
    }

    /// Re-derives the visible groups from the unfiltered master list and
    /// the junk-filter action state, then refreshes the content pane.
    /// Pure recompute; no db round-trip.
    pub fn refilter(&self) {
        // The filtered entry set is about to change (junk toggle or a fresh
        // import), so the cached whole-history overview aggregates no longer
        // apply. Drop them; refresh_content rebuilds on demand.
        self.imp().overview_base.replace(None);

        let entries = self.imp().entries.borrow();
        let groups = library::grouped(
            &entries,
            self.junk_filter_on(),
            colophon_core::model::DEFAULT_JUNK_THRESHOLD_SECS,
        );
        drop(entries);

        // If the selected book just got junk-filtered away, fall back.
        let selection = *self.imp().selection.borrow();
        if let Selection::Book(id) = selection {
            let visible = groups
                .iter()
                .flat_map(|g| &g.entries)
                .any(|e| e.book.id == id);
            if !visible {
                self.imp().selection.replace(Selection::Overview);
            }
        }

        self.imp()
            .library_view
            .set_groups(&groups, *self.imp().selection.borrow());
        self.refresh_content();
    }

    fn on_select(&self, selection: Selection) {
        if *self.imp().selection.borrow() == selection {
            return;
        }
        self.imp().selection.replace(selection);
        self.refresh_content();
        // In collapsed (narrow) mode, selecting navigates forward.
        self.imp().split_view.set_show_content(true);
    }

    /// Renders the content pane for the current selection.
    fn refresh_content(&self) {
        let imp = self.imp();
        let entries = self.filtered_entries();
        let today = Local::now().date_naive();

        if imp.entries.borrow().is_empty() {
            imp.content_stack.set_visible_child_name("placeholder");
            imp.content_page.set_title("Colophon");
            return;
        }

        match *imp.selection.borrow() {
            Selection::Overview => {
                // Reuse the cached whole-history aggregates if they were
                // built for today's date and the current filtered set;
                // otherwise build them once. Window toggles hit the cache.
                let mut cache = imp.overview_base.borrow_mut();
                if cache.as_ref().is_none_or(|(day, _)| *day != today) {
                    *cache = Some((today, stats::overview_base(&entries, &Local, today)));
                }
                let (_, base) = cache.as_ref().expect("just populated");
                let overview = stats::overview_windowed(
                    base,
                    &entries,
                    &Local,
                    today,
                    imp.overview_page.window_days(),
                );
                drop(cache);
                imp.overview_page.set_data(&overview, today);
                imp.content_stack.set_visible_child_name("overview");
                imp.content_page.set_title("All Books");
            }
            Selection::Book(id) => {
                let Some(entry) = entries.iter().find(|e| e.book.id == id) else {
                    imp.content_stack.set_visible_child_name("placeholder");
                    return;
                };
                let detail = stats::book_detail(entry, &Local, today);
                imp.book_page.set_book(entry, &detail);

                // Speed trend: this book against the library baseline,
                // bucketed by the library's full span so the series stay
                // commensurable.
                let all_events: Vec<colophon_core::PageEvent> = entries
                    .iter()
                    .flat_map(|e| e.events.iter().copied())
                    .collect();
                let first_day = all_events
                    .iter()
                    .map(|e| colophon_core::metrics::local_date(e.start_time, &Local))
                    .min();
                let bucket = stats::speed_bucket_for(first_day, today);
                let to_points = |events: &[colophon_core::PageEvent]| {
                    colophon_core::metrics::speed_series(events, &Local, bucket)
                        .into_iter()
                        .map(|(date, point)| crate::charts::line::Point {
                            date,
                            value: point.pages_per_hour,
                            display: format!(
                                "{:.0} pages/hour \u{b7} {} pages in {}",
                                point.pages_per_hour,
                                point.pages,
                                crate::fmt::humanize_secs(point.seconds)
                            ),
                        })
                        .collect::<Vec<_>>()
                };
                imp.book_page
                    .set_speed(to_points(&entry.events), to_points(&all_events), bucket);

                imp.content_stack.set_visible_child_name("book");
                imp.content_page.set_title(entry.book.title.trim());
            }
        }
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
