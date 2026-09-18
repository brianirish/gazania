//! Home page: every drive as a group, every volume as a row.

use crate::pages::drive::DrivePage;
use crate::pages::volume_row::VolumeRow;
use crate::window::Window;
use adw::prelude::*;
use adw::subclass::prelude::*;
use futures_lite::StreamExt;
use gazania_core::format::human_size;
use gazania_core::volumes::{self, Source, VolumesReport};
use gazania_core::{Drive, Transport};
use gtk::{glib, CompositeTemplate};
use std::cell::{Cell, RefCell};
use std::time::Duration;

const REFRESH_INTERVAL: Duration = Duration::from_secs(30);
const DEBOUNCE: Duration = Duration::from_millis(300);
const FALLBACK_BANNER: &str = "Drive grouping is unavailable because udisks2 could not be reached";

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/io/github/brianirish/Gazania/overview_page.ui")]
    pub struct OverviewPage {
        #[template_child]
        pub banner: TemplateChild<adw::Banner>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub empty_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub groups: TemplateChild<gtk::Box>,
        pub rows: RefCell<Vec<VolumeRow>>,
        pub loading: Cell<bool>,
        pub reload_pending: Cell<bool>,
        pub timer: RefCell<Option<glib::SourceId>>,
        pub debounce: RefCell<Option<glib::SourceId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for OverviewPage {
        const NAME: &'static str = "GazaniaOverviewPage";
        type Type = super::OverviewPage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for OverviewPage {
        fn constructed(&self) {
            self.parent_constructed();
            let page = self.obj();
            page.reload();
            page.start_timer();
            page.start_watch();
        }

        fn dispose(&self) {
            if let Some(id) = self.timer.take() {
                id.remove();
            }
            if let Some(id) = self.debounce.take() {
                id.remove();
            }
        }
    }

    impl WidgetImpl for OverviewPage {}
    impl NavigationPageImpl for OverviewPage {}
}

glib::wrapper! {
    pub struct OverviewPage(ObjectSubclass<imp::OverviewPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl OverviewPage {
    fn window(&self) -> Option<Window> {
        self.root().and_then(|r| r.downcast::<Window>().ok())
    }

    /// Enumerate volumes and rebuild the list. Coalesces overlapping calls.
    pub fn reload(&self) {
        let imp = self.imp();
        if imp.loading.replace(true) {
            imp.reload_pending.set(true);
            return;
        }
        if imp.rows.borrow().is_empty() {
            imp.stack.set_visible_child_name("loading");
        }
        let page = self.clone();
        glib::spawn_future_local(async move {
            let result = volumes::list_volumes().await;
            page.imp().loading.set(false);
            match result {
                Ok(report) => page.show_report(report),
                Err(e) => page.show_error(&e.to_string()),
            }
            if page.imp().reload_pending.replace(false) {
                page.reload();
            }
        });
    }

    fn show_report(&self, report: VolumesReport) {
        let imp = self.imp();
        match report.source {
            Source::Udisks2 => imp.banner.set_revealed(false),
            Source::MountinfoFallback => {
                imp.banner.set_title(FALLBACK_BANNER);
                imp.banner.set_revealed(true);
            }
        }

        // A rebuild destroys the focused row, so remember which volume had
        // focus and restore it on the row that replaces it.
        let focused_id = self
            .focused_index()
            .and_then(|i| imp.rows.borrow().get(i).map(|r| r.volume().id));

        while let Some(child) = imp.groups.first_child() {
            imp.groups.remove(&child);
        }
        let mut rows = Vec::new();
        for drive in &report.drives {
            let group = adw::PreferencesGroup::new();
            group.set_title(&drive.model);
            group.set_description(Some(&format!(
                "{} · {}",
                drive.transport.label(),
                human_size(drive.size)
            )));
            group.set_header_suffix(Some(&gtk::Image::from_icon_name(drive_icon(drive))));
            for volume in &drive.volumes {
                let row = VolumeRow::new(drive, volume);
                row.connect_activated(glib::clone!(
                    #[weak(rename_to = page)]
                    self,
                    move |row| page.open_row(row)
                ));
                group.add(&row);
                rows.push(row);
            }
            imp.groups.append(&group);
        }
        let empty = rows.is_empty();
        imp.rows.replace(rows);
        if empty {
            imp.empty_page
                .set_description(Some("Nothing mounted or attached could be listed."));
            imp.stack.set_visible_child_name("empty");
        } else {
            imp.stack.set_visible_child_name("list");
        }

        if let Some(id) = focused_id {
            let restore = imp
                .rows
                .borrow()
                .iter()
                .find(|row| row.volume().id == id)
                .cloned();
            if let Some(row) = restore {
                row.grab_focus();
            }
        }
    }

    fn show_error(&self, message: &str) {
        let imp = self.imp();
        if imp.rows.borrow().is_empty() {
            imp.empty_page.set_description(Some(message));
            imp.stack.set_visible_child_name("empty");
        } else if let Some(window) = self.window() {
            window.toast(&format!("Refresh failed: {message}"));
        }
    }

    pub fn open_row(&self, row: &VolumeRow) {
        if let Some(window) = self.window() {
            window
                .navigation()
                .push(&DrivePage::new(&row.drive(), &row.volume()));
        }
    }

    fn focused_index(&self) -> Option<usize> {
        let focus = gtk::prelude::GtkWindowExt::focus(&self.window()?)?;
        self.imp()
            .rows
            .borrow()
            .iter()
            .position(|row| focus == *row.upcast_ref::<gtk::Widget>() || focus.is_ancestor(row))
    }

    pub fn focus_next(&self) {
        let len = self.imp().rows.borrow().len();
        if len == 0 {
            return;
        }
        let next = self
            .focused_index()
            .map(|i| (i + 1).min(len - 1))
            .unwrap_or(0);
        let row = self.imp().rows.borrow()[next].clone();
        row.grab_focus();
    }

    pub fn focus_prev(&self) {
        let len = self.imp().rows.borrow().len();
        if len == 0 {
            return;
        }
        let prev = self
            .focused_index()
            .map(|i| i.saturating_sub(1))
            .unwrap_or(0);
        let row = self.imp().rows.borrow()[prev].clone();
        row.grab_focus();
    }

    pub fn activate_focused(&self) {
        let Some(i) = self.focused_index() else {
            return;
        };
        let row = self.imp().rows.borrow()[i].clone();
        self.open_row(&row);
    }

    fn start_timer(&self) {
        let id = glib::timeout_add_local(
            REFRESH_INTERVAL,
            glib::clone!(
                #[weak(rename_to = page)]
                self,
                #[upgrade_or]
                glib::ControlFlow::Break,
                move || {
                    if page.is_mapped() {
                        page.reload();
                    }
                    glib::ControlFlow::Continue
                }
            ),
        );
        self.imp().timer.replace(Some(id));
    }

    fn start_watch(&self) {
        let weak = self.downgrade();
        glib::spawn_future_local(async move {
            let Ok(conn) = volumes::udisks::connect().await else {
                return;
            };
            let Ok(mut changes) = volumes::watch(&conn).await else {
                return;
            };
            while changes.next().await.is_some() {
                let Some(page) = weak.upgrade() else { break };
                page.schedule_reload();
            }
        });
    }

    /// Collapse bursts of udisks2 signals into one reload.
    fn schedule_reload(&self) {
        let imp = self.imp();
        if imp.debounce.borrow().is_some() {
            return;
        }
        let id = glib::timeout_add_local_once(
            DEBOUNCE,
            glib::clone!(
                #[weak(rename_to = page)]
                self,
                move || {
                    page.imp().debounce.take();
                    page.reload();
                }
            ),
        );
        imp.debounce.replace(Some(id));
    }
}

fn drive_icon(drive: &Drive) -> &'static str {
    if drive.removable || drive.transport == Transport::Usb {
        "drive-removable-media-symbolic"
    } else if drive.rotational {
        "drive-harddisk-symbolic"
    } else {
        "drive-harddisk-solidstate-symbolic"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drive(transport: Transport, rotational: bool, removable: bool) -> Drive {
        Drive {
            id: "/org/freedesktop/UDisks2/drives/Test".into(),
            model: "Test".into(),
            serial: None,
            vendor: None,
            size: 0,
            transport,
            rotational,
            removable,
            volumes: Vec::new(),
        }
    }

    #[test]
    fn removable_and_usb_drives_get_the_removable_icon() {
        assert_eq!(
            drive_icon(&drive(Transport::Sata, false, true)),
            "drive-removable-media-symbolic"
        );
        assert_eq!(
            drive_icon(&drive(Transport::Usb, false, false)),
            "drive-removable-media-symbolic"
        );
    }

    #[test]
    fn rotational_drives_get_the_harddisk_icon() {
        assert_eq!(
            drive_icon(&drive(Transport::Sata, true, false)),
            "drive-harddisk-symbolic"
        );
    }

    #[test]
    fn ssds_get_the_solidstate_icon() {
        assert_eq!(
            drive_icon(&drive(Transport::Nvme, false, false)),
            "drive-harddisk-solidstate-symbolic"
        );
    }
}
