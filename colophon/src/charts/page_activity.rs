//! Per-page activity strip: one bar per page position across the book,
//! sqrt height scaling capped at the 90th percentile (KoShelf's numbers;
//! spec.md Tier A #5). Pages narrower than a pixel are binned, so an
//! 800-page book still reads on a 400px pane. Answers "did it drag in
//! the middle" at a glance.

use std::cell::RefCell;

use adw::subclass::prelude::*;
use gtk::glib;
use gtk::prelude::*;

use colophon_core::sidecar::AnnotationKind;

use crate::fmt::humanize_secs;
use crate::stats::PageActivity;

const HEIGHT: i32 = 96;
const PAD_BOTTOM: f64 = 16.0;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct PageActivityStrip {
        pub data: RefCell<Option<PageActivity>>,
        /// Annotation markers: fractional position through the book + kind.
        pub markers: RefCell<Vec<(f64, AnnotationKind)>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PageActivityStrip {
        const NAME: &'static str = "PageActivityStrip";
        type Type = super::PageActivityStrip;
        type ParentType = gtk::DrawingArea;
    }

    impl ObjectImpl for PageActivityStrip {
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
            widget.connect_query_tooltip(|this, x, _, _, tooltip| {
                match this.tooltip_at(f64::from(x)) {
                    Some(text) => {
                        tooltip.set_text(Some(&text));
                        true
                    }
                    None => false,
                }
            });
            let weak = widget.downgrade();
            adw::StyleManager::default().connect_dark_notify(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.queue_draw();
                }
            });
        }
    }
    impl WidgetImpl for PageActivityStrip {}
    impl DrawingAreaImpl for PageActivityStrip {}
}

glib::wrapper! {
    pub struct PageActivityStrip(ObjectSubclass<imp::PageActivityStrip>)
        @extends gtk::DrawingArea, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for PageActivityStrip {
    fn default() -> Self {
        glib::Object::new()
    }
}

/// Bin index for a page at a given width, and back.
fn bins(pages: i64, width: f64) -> usize {
    (width.max(1.0) as usize).min(pages.max(1) as usize)
}

impl PageActivityStrip {
    pub fn set_data(&self, data: PageActivity) {
        self.imp().data.replace(Some(data));
        self.queue_draw();
    }

    /// Annotation markers, as fractional positions through the book and their
    /// kind. Empty for a book with no sidecar provided.
    pub fn set_markers(&self, markers: Vec<(f64, AnnotationKind)>) {
        self.imp().markers.replace(markers);
        self.queue_draw();
    }

    fn draw(&self, cr: &gtk::cairo::Context, w: i32, h: i32) {
        let data = self.imp().data.borrow();
        let Some(data) = data.as_ref() else { return };
        if data.pages <= 0 {
            return;
        }
        let dark = super::is_dark();
        let w = f64::from(w);
        let h = f64::from(h);
        let baseline = h - PAD_BOTTOM;
        let plot = baseline - 4.0;

        super::set_source(cr, super::cell_bg(dark));
        cr.rectangle(0.0, baseline, w, 1.0);
        let _ = cr.fill();

        let bin_count = bins(data.pages, w);
        let mut binned = vec![0i64; bin_count];
        for &(page, secs, _) in &data.per_page {
            let bin = ((page - 1) * bin_count as i64 / data.pages).clamp(0, bin_count as i64 - 1);
            binned[bin as usize] += secs;
        }
        let cap = data.cap_secs.max(1);

        super::set_source(cr, super::accent(dark));
        let bin_width = w / bin_count as f64;
        for (bin, &secs) in binned.iter().enumerate() {
            if secs <= 0 {
                continue;
            }
            // sqrt scaling, capped: quiet pages stay visible, outliers
            // don't flatten everything else.
            let fraction = ((secs.min(cap)) as f64 / cap as f64).sqrt();
            let bar = (plot * fraction).max(1.5);
            cr.rectangle(
                bin as f64 * bin_width,
                baseline - bar,
                bin_width.max(1.0),
                bar,
            );
        }
        let _ = cr.fill();

        // Annotation markers as small triangles below the baseline (so they
        // never fight the bars): highlights and notes in accent, bookmarks
        // muted. Positions are fractions of the book, already rescaled off the
        // sidecar's own page count, so they land on the current axis.
        for &(pos, kind) in self.imp().markers.borrow().iter() {
            let x = pos.clamp(0.0, 1.0) * w;
            let color = match kind {
                AnnotationKind::Bookmark => super::muted(dark),
                _ => super::accent(dark),
            };
            super::set_source(cr, color);
            cr.move_to(x, baseline + 2.5);
            cr.line_to(x - 3.0, baseline + 8.0);
            cr.line_to(x + 3.0, baseline + 8.0);
            cr.close_path();
            let _ = cr.fill();
        }

        super::draw_text(cr, 0.0, h - 3.0, 10.0, super::muted(dark), "p. 1");
        let label = format!("p. {}", data.pages);
        let tw = super::text_width(cr, 10.0, &label);
        super::draw_text(cr, w - tw, h - 3.0, 10.0, super::muted(dark), &label);
    }

    fn tooltip_at(&self, x: f64) -> Option<String> {
        let data = self.imp().data.borrow();
        let data = data.as_ref()?;
        if data.pages <= 0 {
            return None;
        }
        let w = f64::from(self.width());
        let bin_count = bins(data.pages, w);
        let bin = ((x / w) * bin_count as f64).floor() as i64;
        let first = bin * data.pages / bin_count as i64 + 1;
        let last = ((bin + 1) * data.pages / bin_count as i64).max(first);

        let (mut secs, mut reads) = (0i64, 0u32);
        for &(page, s, count) in &data.per_page {
            if page >= first && page <= last {
                secs += s;
                reads += count;
            }
        }
        let range = if first == last {
            format!("Page {first}")
        } else {
            format!("Pages {first}\u{2013}{last}")
        };
        Some(if secs > 0 {
            format!(
                "{range} \u{b7} {} \u{b7} {reads} reads",
                humanize_secs(secs)
            )
        } else {
            format!("{range} \u{b7} unread")
        })
    }
}
