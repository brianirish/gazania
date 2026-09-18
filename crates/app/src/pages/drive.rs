//! Per-volume page: Usage, Health and Benchmark placeholders plus Details.

use crate::window::Window;
use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{glib, CompositeTemplate};
use std::cell::RefCell;
use zinnia_core::format::human_size;
use zinnia_core::volumes;
use zinnia_core::{Drive, Volume};

const VIEWS: [&str; 4] = ["usage", "health", "benchmark", "details"];

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/io/github/brianirish/Zinnia/drive_page.ui")]
    pub struct DrivePage {
        #[template_child]
        pub views: TemplateChild<adw::ViewStack>,
        #[template_child]
        pub drive_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub volume_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub mounts_group: TemplateChild<adw::PreferencesGroup>,
        pub volume_id: RefCell<String>,
        pub rows: RefCell<Vec<gtk::Widget>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DrivePage {
        const NAME: &'static str = "ZinniaDrivePage";
        type Type = super::DrivePage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for DrivePage {}
    impl WidgetImpl for DrivePage {}
    impl NavigationPageImpl for DrivePage {}
}

glib::wrapper! {
    pub struct DrivePage(ObjectSubclass<imp::DrivePage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl DrivePage {
    pub fn new(drive: &Drive, volume: &Volume) -> Self {
        let page: Self = glib::Object::new();
        page.imp().volume_id.replace(volume.id.clone());
        page.fill(drive, volume);
        page
    }

    fn window(&self) -> Option<Window> {
        self.root().and_then(|r| r.downcast::<Window>().ok())
    }

    fn fill(&self, drive: &Drive, volume: &Volume) {
        let imp = self.imp();
        self.set_title(
            &volume
                .label
                .clone()
                .unwrap_or_else(|| volume.device.display().to_string()),
        );

        for row in imp.rows.take() {
            if let Some(group) = row
                .parent()
                .and_then(|p| p.ancestor(adw::PreferencesGroup::static_type()))
            {
                group
                    .downcast::<adw::PreferencesGroup>()
                    .unwrap()
                    .remove(&row);
            }
        }

        let yes_no = |b: bool| if b { "Yes" } else { "No" };
        let mut rows = Vec::new();
        for (title, value) in [
            ("Model", drive.model.clone()),
            (
                "Serial",
                drive.serial.clone().unwrap_or_else(|| "Unknown".into()),
            ),
            (
                "Vendor",
                drive.vendor.clone().unwrap_or_else(|| "Unknown".into()),
            ),
            ("Transport", drive.transport.label()),
            ("Rotational", yes_no(drive.rotational).into()),
            ("Removable", yes_no(drive.removable).into()),
            ("Size", human_size(drive.size)),
        ] {
            rows.push(property_row(&imp.drive_group, title, &value));
        }

        let encrypted = match (&volume.encrypted, &volume.backing_device) {
            (true, Some(dev)) => format!("Yes, on {}", dev.display()),
            (true, None) => "Yes".into(),
            (false, _) => "No".into(),
        };
        let (used, available) = match volume.usage {
            Some(u) => (human_size(u.used), human_size(u.available)),
            None => ("Not mounted".into(), "Not mounted".into()),
        };
        for (title, value) in [
            ("Device", volume.device.display().to_string()),
            (
                "Filesystem",
                volume
                    .fs_type
                    .clone()
                    .unwrap_or_else(|| "Unformatted".into()),
            ),
            (
                "Label",
                volume.label.clone().unwrap_or_else(|| "None".into()),
            ),
            ("UUID", volume.uuid.clone().unwrap_or_else(|| "None".into())),
            ("Encrypted", encrypted),
            ("Size", human_size(volume.size)),
            ("Used", used),
            ("Available", available),
        ] {
            rows.push(property_row(&imp.volume_group, title, &value));
        }

        if volume.mount_points.is_empty() {
            rows.push(property_row(&imp.mounts_group, "Not mounted", ""));
        }
        for mp in &volume.mount_points {
            rows.push(property_row(
                &imp.mounts_group,
                &mp.path.display().to_string(),
                &mp.options.join(", "),
            ));
        }
        imp.rows.replace(rows);
    }

    pub fn select_view(&self, n: i32) {
        if let Some(name) = usize::try_from(n - 1).ok().and_then(|i| VIEWS.get(i)) {
            self.imp().views.set_visible_child_name(name);
        }
    }

    pub fn cycle_view(&self) {
        let current = self.imp().views.visible_child_name().map(|n| n.to_string());
        let idx = VIEWS
            .iter()
            .position(|v| Some(*v) == current.as_deref())
            .unwrap_or(0);
        self.imp()
            .views
            .set_visible_child_name(VIEWS[(idx + 1) % VIEWS.len()]);
    }

    /// Re-enumerate and refill Details for this volume.
    pub fn refresh(&self) {
        let page = self.clone();
        glib::spawn_future_local(async move {
            let id = page.imp().volume_id.borrow().clone();
            match volumes::list_volumes().await {
                Ok(report) => {
                    let found = report.drives.iter().find_map(|d| {
                        d.volumes
                            .iter()
                            .find(|v| v.id == id)
                            .map(|v| (d.clone(), v.clone()))
                    });
                    match found {
                        Some((drive, volume)) => page.fill(&drive, &volume),
                        None => {
                            if let Some(w) = page.window() {
                                w.toast("This volume is no longer present");
                            }
                        }
                    }
                }
                Err(e) => {
                    if let Some(w) = page.window() {
                        w.toast(&format!("Refresh failed: {e}"));
                    }
                }
            }
        });
    }
}

fn property_row(group: &adw::PreferencesGroup, title: &str, value: &str) -> gtk::Widget {
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(value)
        .build();
    row.add_css_class("property");
    row.set_subtitle_selectable(true);
    group.add(&row);
    row.upcast()
}
