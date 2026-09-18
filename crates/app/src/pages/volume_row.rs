//! One volume in the overview: ring, name, filesystem and mounts, lock, usage.

use crate::widgets::geometry::fraction;
use crate::widgets::usage_ring::UsageRing;
use adw::prelude::*;
use adw::subclass::prelude::*;
use zinnia_core::format::human_size;
use zinnia_core::{Drive, Volume};
use gtk::glib;
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct VolumeRow {
        pub drive: RefCell<Option<Drive>>,
        pub volume: RefCell<Option<Volume>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for VolumeRow {
        const NAME: &'static str = "ZinniaVolumeRow";
        type Type = super::VolumeRow;
        type ParentType = adw::ActionRow;
    }

    impl ObjectImpl for VolumeRow {}
    impl WidgetImpl for VolumeRow {}
    impl ListBoxRowImpl for VolumeRow {}
    impl PreferencesRowImpl for VolumeRow {}
    impl ActionRowImpl for VolumeRow {}
}

glib::wrapper! {
    pub struct VolumeRow(ObjectSubclass<imp::VolumeRow>)
        @extends adw::ActionRow, adw::PreferencesRow, gtk::ListBoxRow, gtk::Widget,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl VolumeRow {
    pub fn new(drive: &Drive, volume: &Volume) -> Self {
        let row: Self = glib::Object::new();
        row.set_activatable(true);
        row.set_title(&volume.label.clone().unwrap_or_else(|| volume.device.display().to_string()));

        let mounts = if volume.mount_points.is_empty() {
            "Not mounted".to_string()
        } else {
            volume
                .mount_points
                .iter()
                .map(|m| m.path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let fs = volume.fs_type.as_deref().unwrap_or("unformatted");
        row.set_subtitle(&format!("{fs} · {mounts}"));

        let ring = UsageRing::new();
        ring.set_fraction(volume.usage.map(|u| fraction(u.used, volume.size)).unwrap_or(0.0));
        row.add_prefix(&ring);

        if volume.encrypted {
            let lock = gtk::Image::from_icon_name("channel-secure-symbolic");
            lock.set_tooltip_text(Some("Encrypted"));
            row.add_suffix(&lock);
        }

        let usage = match volume.usage {
            Some(u) => format!("{} of {}", human_size(u.used), human_size(volume.size)),
            None => human_size(volume.size),
        };
        let label = gtk::Label::new(Some(&usage));
        label.add_css_class("dim-label");
        label.add_css_class("numeric");
        row.add_suffix(&label);
        row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));

        row.imp().drive.replace(Some(drive.clone()));
        row.imp().volume.replace(Some(volume.clone()));
        row
    }

    pub fn drive(&self) -> Drive {
        self.imp().drive.borrow().clone().expect("row built with a drive")
    }

    pub fn volume(&self) -> Volume {
        self.imp().volume.borrow().clone().expect("row built with a volume")
    }
}
