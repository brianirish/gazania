//! A single-arc ring showing used over size. Drawn with GSK paths in the
//! widget's CSS `color`, so the accent and the warning and error colors come
//! from CSS classes, not from code.

use crate::widgets::geometry::{self, Level};
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk, prelude::*};
use std::cell::Cell;

mod imp {
    use super::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::UsageRing)]
    pub struct UsageRing {
        #[property(get, set = Self::set_fraction, minimum = 0.0, maximum = 1.0, default = 0.0)]
        fraction: Cell<f64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for UsageRing {
        const NAME: &'static str = "ZinniaUsageRing";
        type Type = super::UsageRing;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("usage-ring");
            klass.set_accessible_role(gtk::AccessibleRole::Img);
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for UsageRing {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.set_size_request(40, 40);
            obj.set_valign(gtk::Align::Center);
            obj.set_halign(gtk::Align::Center);
        }
    }

    impl WidgetImpl for UsageRing {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let obj = self.obj();
            let w = obj.width() as f64;
            let h = obj.height() as f64;
            let size = w.min(h);
            if size <= 0.0 {
                return;
            }
            let stroke_width = (size * 0.14).max(2.0);
            let radius = size / 2.0 - stroke_width / 2.0;
            let (cx, cy) = (w / 2.0, h / 2.0);
            let color = obj.color();

            let track = gsk::PathBuilder::new();
            track.add_circle(&graphene::Point::new(cx as f32, cy as f32), radius as f32);
            let track_path = track.to_path();
            let stroke = gsk::Stroke::new(stroke_width as f32);
            stroke.set_line_cap(gsk::LineCap::Round);
            let track_color = gdk::RGBA::new(color.red(), color.green(), color.blue(), 0.15);
            snapshot.append_stroke(&track_path, &stroke, &track_color);

            let f = self.fraction.get();
            if f <= 0.0 {
                return;
            }
            if f >= 1.0 {
                snapshot.append_stroke(&track_path, &stroke, &color);
                return;
            }
            let arc = geometry::arc_for(f);
            let (sx, sy) = geometry::point(cx, cy, radius, arc.start);
            let (ex, ey) = geometry::point(cx, cy, radius, arc.end);
            let builder = gsk::PathBuilder::new();
            builder.move_to(sx as f32, sy as f32);
            builder.svg_arc_to(
                radius as f32,
                radius as f32,
                0.0,
                geometry::large_arc(f),
                true,
                ex as f32,
                ey as f32,
            );
            snapshot.append_stroke(&builder.to_path(), &stroke, &color);
        }
    }

    impl UsageRing {
        fn set_fraction(&self, value: f64) {
            self.fraction.set(value.clamp(0.0, 1.0));
            let obj = self.obj();
            obj.remove_css_class("warning");
            obj.remove_css_class("error");
            match geometry::level(value) {
                Level::Normal => {}
                Level::Warning => obj.add_css_class("warning"),
                Level::Critical => obj.add_css_class("error"),
            }
            obj.queue_draw();
        }
    }
}

glib::wrapper! {
    pub struct UsageRing(ObjectSubclass<imp::UsageRing>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for UsageRing {
    fn default() -> Self {
        Self::new()
    }
}

impl UsageRing {
    pub fn new() -> Self {
        glib::Object::new()
    }
}
