//! A simple vertical bar chart: labels below, values above, the maximum
//! bar highlighted (spec.md Tier B #11 wants the weekday/monthly
//! distributions; this widget serves both).

use std::cell::RefCell;

use adw::subclass::prelude::*;
use gtk::glib;
use gtk::prelude::*;

#[derive(Debug, Clone, Default)]
pub struct Bar {
    pub label: String,
    pub value: f64,
    /// Pre-formatted value shown above the bar (e.g. "1h 5m").
    pub display: String,
    /// Hover text; useful when bars are too narrow for `display` (the
    /// 24-column session-starts chart) or labels are elided.
    pub tooltip: Option<String>,
}

const HEIGHT: i32 = 150;
const LABEL_AREA: f64 = 18.0;
const VALUE_AREA: f64 = 16.0;
const GAP_FRACTION: f64 = 0.35;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct BarChart {
        pub bars: RefCell<Vec<Bar>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for BarChart {
        const NAME: &'static str = "BarChart";
        type Type = super::BarChart;
        type ParentType = gtk::DrawingArea;
    }

    impl ObjectImpl for BarChart {
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
    impl WidgetImpl for BarChart {}
    impl DrawingAreaImpl for BarChart {}
}

glib::wrapper! {
    pub struct BarChart(ObjectSubclass<imp::BarChart>)
        @extends gtk::DrawingArea, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for BarChart {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl BarChart {
    pub fn set_bars(&self, bars: Vec<Bar>) {
        self.imp().bars.replace(bars);
        self.queue_draw();
    }

    fn tooltip_at(&self, x: f64) -> Option<String> {
        let bars = self.imp().bars.borrow();
        if bars.is_empty() {
            return None;
        }
        let slot = f64::from(self.width()) / bars.len() as f64;
        let index = ((x / slot).floor() as usize).min(bars.len() - 1);
        bars[index].tooltip.clone()
    }

    fn draw(&self, cr: &gtk::cairo::Context, w: i32, h: i32) {
        let bars = self.imp().bars.borrow();
        if bars.is_empty() {
            return;
        }
        let dark = super::is_dark();
        let max = bars.iter().map(|b| b.value).fold(0.0, f64::max);

        let w = f64::from(w);
        let h = f64::from(h);
        let slot = w / bars.len() as f64;
        let bar_width = slot * (1.0 - GAP_FRACTION);
        let plot_height = h - LABEL_AREA - VALUE_AREA;
        let baseline = h - LABEL_AREA;

        // Baseline rule.
        super::set_source(cr, super::cell_bg(dark));
        cr.rectangle(0.0, baseline, w, 1.0);
        let _ = cr.fill();

        for (i, bar) in bars.iter().enumerate() {
            let x = slot * i as f64 + (slot - bar_width) / 2.0;
            let center = slot * i as f64 + slot / 2.0;

            let fraction = if max > 0.0 { bar.value / max } else { 0.0 };
            let bar_height = (plot_height * fraction).max(if bar.value > 0.0 { 2.0 } else { 0.0 });
            let color = if bar.value == max && max > 0.0 {
                super::highlight(dark)
            } else {
                super::accent(dark)
            };
            super::set_source(cr, color);
            cr.rectangle(x, baseline - bar_height, bar_width, bar_height);
            let _ = cr.fill();

            if bar.value > 0.0 {
                let tw = super::text_width(cr, 10.0, &bar.display);
                super::draw_text(
                    cr,
                    center - tw / 2.0,
                    baseline - bar_height - 4.0,
                    10.0,
                    super::text(dark),
                    &bar.display,
                );
            }
            let lw = super::text_width(cr, 10.0, &bar.label);
            super::draw_text(
                cr,
                center - lw / 2.0,
                h - 5.0,
                10.0,
                super::muted(dark),
                &bar.label,
            );
        }
    }
}
