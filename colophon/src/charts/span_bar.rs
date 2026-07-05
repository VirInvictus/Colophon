//! Positional progress bar (spec.md "Per-book progress display"): draws
//! the merged read spans on the [0,1] page axis so a book read from the
//! middle onward shows *where* reading was logged, not a single
//! left-anchored fraction. A marker sits at the furthest position reached.
//! Unlogged gaps (e.g. a stretch read before KOReader was installed) stay
//! empty rather than masquerading as unread.

use std::cell::{Cell, RefCell};
use std::f64::consts::PI;

use adw::subclass::prelude::*;
use gtk::glib;
use gtk::prelude::*;

const HEIGHT: i32 = 26;
const BAR_H: f64 = 10.0;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct SpanBar {
        /// Merged read spans on [0,1], sorted, non-overlapping.
        pub spans: RefCell<Vec<(f64, f64)>>,
        /// Furthest fractional position reached (the marker).
        pub furthest: Cell<f64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SpanBar {
        const NAME: &'static str = "SpanBar";
        type Type = super::SpanBar;
        type ParentType = gtk::DrawingArea;
    }

    impl ObjectImpl for SpanBar {
        fn constructed(&self) {
            self.parent_constructed();
            let widget = self.obj();
            widget.set_content_height(HEIGHT);
            widget.set_hexpand(true);
            widget.set_draw_func(glib::clone!(
                #[weak(rename_to = this)]
                widget,
                move |_, cr, w, h| this.draw(cr, w, h)
            ));
            widget.set_has_tooltip(true);
            widget.connect_query_tooltip(|this, _, _, _, tooltip| match this.tooltip() {
                Some(text) => {
                    tooltip.set_text(Some(&text));
                    true
                }
                None => false,
            });
            let weak = widget.downgrade();
            adw::StyleManager::default().connect_dark_notify(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.queue_draw();
                }
            });
        }
    }
    impl WidgetImpl for SpanBar {}
    impl DrawingAreaImpl for SpanBar {}
}

glib::wrapper! {
    pub struct SpanBar(ObjectSubclass<imp::SpanBar>)
        @extends gtk::DrawingArea, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for SpanBar {
    fn default() -> Self {
        glib::Object::new()
    }
}

/// A horizontal pill (rounded-both-ends rect) path.
fn pill(cr: &gtk::cairo::Context, x: f64, y: f64, w: f64, h: f64) {
    let r = (h / 2.0).min(w / 2.0).max(0.0);
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -PI / 2.0, PI / 2.0);
    cr.arc(x + r, y + r, r, PI / 2.0, 3.0 * PI / 2.0);
    cr.close_path();
}

impl SpanBar {
    pub fn set_data(&self, spans: Vec<(f64, f64)>, furthest: f64) {
        let imp = self.imp();
        imp.spans.replace(spans);
        imp.furthest.set(furthest.clamp(0.0, 1.0));
        self.queue_draw();
    }

    fn draw(&self, cr: &gtk::cairo::Context, w: i32, h: i32) {
        let spans = self.imp().spans.borrow();
        let furthest = self.imp().furthest.get();
        let dark = super::is_dark();
        let w = f64::from(w);
        let h = f64::from(h);
        let y = (h - BAR_H) / 2.0;

        // Track: the whole page axis, empty by default.
        super::set_source(cr, super::cell_bg(dark));
        pill(cr, 0.0, y, w, BAR_H);
        let _ = cr.fill();

        // Read spans.
        super::set_source(cr, super::accent(dark));
        for &(lo, hi) in spans.iter() {
            let x = lo * w;
            let sw = ((hi - lo) * w).max(2.0);
            pill(cr, x, y, sw, BAR_H);
            let _ = cr.fill();
        }

        // Furthest-position marker: a slim vertical rule spanning the bar,
        // pinned inside the widget so it stays visible at either edge.
        if furthest > 0.0 {
            let mx = (furthest * w).clamp(1.0, w - 1.0);
            super::set_source(cr, super::highlight(dark));
            cr.rectangle(mx - 1.0, y - 3.0, 2.0, BAR_H + 6.0);
            let _ = cr.fill();
        }
    }

    fn tooltip(&self) -> Option<String> {
        let spans = self.imp().spans.borrow();
        if spans.is_empty() {
            return None;
        }
        let covered: f64 = spans.iter().map(|(lo, hi)| hi - lo).sum();
        Some(format!(
            "{:.0}% logged \u{b7} furthest {:.0}%",
            covered * 100.0,
            self.imp().furthest.get() * 100.0
        ))
    }
}
