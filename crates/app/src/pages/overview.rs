//! Home page: every drive as a group, every volume as a row.

use crate::pages::drive::DrivePage;
use crate::pages::volume_row::VolumeRow;
use crate::window::Window;
use adw::prelude::*;
use adw::subclass::prelude::*;
use futures_lite::StreamExt;
use gtk::{glib, CompositeTemplate};
use std::cell::{Cell, RefCell};
use std::time::Duration;
use zinnia_core::format::human_size;
use zinnia_core::volumes::{self, Source, VolumesReport};
use zinnia_core::{Drive, Transport};

const REFRESH_INTERVAL: Duration = Duration::from_secs(30);
const DEBOUNCE: Duration = Duration::from_millis(300);
const FALLBACK_BANNER: &str = "Drive grouping is unavailable because udisks2 could not be reached";

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/io/github/brianirish/Zinnia/overview_page.ui")]
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
        const NAME: &'static str = "ZinniaOverviewPage";
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
        let rows = self.imp().rows.borrow();
        if rows.is_empty() {
            return;
        }
        let next = self
            .focused_index()
            .map(|i| (i + 1).min(rows.len() - 1))
            .unwrap_or(0);
        rows[next].grab_focus();
    }

    pub fn focus_prev(&self) {
        let rows = self.imp().rows.borrow();
        if rows.is_empty() {
            return;
        }
        let prev = self
            .focused_index()
            .map(|i| i.saturating_sub(1))
            .unwrap_or(0);
        rows[prev].grab_focus();
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
