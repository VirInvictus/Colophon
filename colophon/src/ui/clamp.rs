//! A width clamp (the AdwClamp replacement): allocates its single child
//! at most `maximum-size` wide, centered in any extra width. GTK CSS has
//! no max-width property, so this needs a real widget. AdwClamp's
//! tightening-threshold easing is deliberately not carried over: content
//! is simply capped and centered, per the Phase 6 flat/dense look.

use std::cell::Cell;

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

mod imp {
    use super::*;

    #[derive(glib::Properties, Default)]
    #[properties(wrapper_type = super::Clamp)]
    pub struct Clamp {
        #[property(get, set, construct, default = i32::MAX)]
        pub maximum_size: Cell<i32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Clamp {
        const NAME: &'static str = "Clamp";
        type Type = super::Clamp;
        type ParentType = gtk::Widget;
    }

    #[glib::derived_properties]
    impl ObjectImpl for Clamp {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for Clamp {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let Some(child) = self.obj().first_child() else {
                return (0, 0, -1, -1);
            };
            let max = self.maximum_size.get();
            if orientation == gtk::Orientation::Horizontal {
                let (min, nat, _, _) = child.measure(orientation, for_size);
                (min, nat.min(max).max(min), -1, -1)
            } else {
                // Height-for-width is answered at the width the child will
                // actually get, i.e. no wider than the clamp.
                let for_size = if for_size >= 0 {
                    for_size.min(max)
                } else {
                    for_size
                };
                let (min, nat, _, _) = child.measure(orientation, for_size);
                (min, nat, -1, -1)
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            let Some(child) = self.obj().first_child() else {
                return;
            };
            let w = width.min(self.maximum_size.get()).max(0);
            let x = (width - w) / 2;
            child.size_allocate(&gtk::Allocation::new(x, 0, w, height), baseline);
        }
    }
}

glib::wrapper! {
    pub struct Clamp(ObjectSubclass<imp::Clamp>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}
