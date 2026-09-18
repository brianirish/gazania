use crate::application::Application;
use crate::config::APP_ID;
use crate::pages::drive::DrivePage;
use crate::pages::overview::OverviewPage;
use adw::subclass::prelude::*;
use gtk::{gio, glib, prelude::*, CompositeTemplate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageAction {
    Refresh,
    Next,
    Prev,
    Activate,
    View(i32),
    CycleView,
}

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/io/github/brianirish/Gazania/window.ui")]
    pub struct Window {
        #[template_child]
        pub navigation: TemplateChild<adw::NavigationView>,
        #[template_child]
        pub toasts: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub overview: TemplateChild<OverviewPage>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "GazaniaWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            OverviewPage::ensure_type();
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Window {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().bind_settings();
            self.obj().setup_actions();
        }
    }

    impl WidgetImpl for Window {}
    impl WindowImpl for Window {}
    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}
}

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl Window {
    pub fn new(app: &Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    /// Requires the compiled gschema to be discoverable; `scripts/dev-run.sh`
    /// arranges that for source-tree runs.
    fn bind_settings(&self) {
        let settings = gio::Settings::new(APP_ID);
        settings.bind("window-width", self, "default-width").build();
        settings
            .bind("window-height", self, "default-height")
            .build();
        settings.bind("is-maximized", self, "maximized").build();
    }

    pub fn navigation(&self) -> adw::NavigationView {
        self.imp().navigation.get()
    }

    pub fn toast(&self, text: &str) {
        self.imp().toasts.add_toast(adw::Toast::new(text));
    }

    fn setup_actions(&self) {
        let back = gio::ActionEntry::builder("back")
            .activate(|win: &Self, _, _| {
                win.navigation().pop();
            })
            .build();
        let shortcuts = gio::ActionEntry::builder("shortcuts")
            .activate(|win: &Self, _, _| crate::shortcuts::present(win.upcast_ref()))
            .build();
        let refresh = gio::ActionEntry::builder("refresh")
            .activate(|win: &Self, _, _| win.dispatch(PageAction::Refresh))
            .build();
        let next = gio::ActionEntry::builder("next")
            .activate(|win: &Self, _, _| win.dispatch(PageAction::Next))
            .build();
        let prev = gio::ActionEntry::builder("prev")
            .activate(|win: &Self, _, _| win.dispatch(PageAction::Prev))
            .build();
        let activate = gio::ActionEntry::builder("activate")
            .activate(|win: &Self, _, _| win.dispatch(PageAction::Activate))
            .build();
        let view = gio::ActionEntry::builder("view")
            .parameter_type(Some(glib::VariantTy::INT32))
            .activate(|win: &Self, _, param| {
                let n = param.and_then(|p| p.get::<i32>()).unwrap_or(1);
                win.dispatch(PageAction::View(n));
            })
            .build();
        let cycle = gio::ActionEntry::builder("cycle-view")
            .activate(|win: &Self, _, _| win.dispatch(PageAction::CycleView))
            .build();
        self.add_action_entries([back, shortcuts, refresh, next, prev, activate, view, cycle]);

        // Unmodified keys go on a bubble-phase controller so the focus widget
        // (entries, dialogs, list boxes) sees them first. Escape is left alone:
        // AdwNavigationView pops and AdwDialog closes on it natively.
        let keys = gtk::ShortcutController::new();
        keys.set_scope(gtk::ShortcutScope::Local);
        keys.set_propagation_phase(gtk::PropagationPhase::Bubble);
        for (trigger, action, target) in [
            ("h", "win.back", None),
            ("j", "win.next", None),
            ("k", "win.prev", None),
            ("l", "win.activate", None),
            ("1", "win.view", Some(1i32)),
            ("2", "win.view", Some(2i32)),
            ("3", "win.view", Some(3i32)),
            ("4", "win.view", Some(4i32)),
        ] {
            let shortcut = gtk::Shortcut::new(
                gtk::ShortcutTrigger::parse_string(trigger),
                Some(gtk::NamedAction::new(action)),
            );
            if let Some(n) = target {
                shortcut.set_arguments(Some(&n.to_variant()));
            }
            keys.add_shortcut(shortcut);
        }
        self.add_controller(keys);
    }

    /// Route a page-level action to whichever page is visible.
    pub fn dispatch(&self, action: PageAction) {
        let Some(page) = self.navigation().visible_page() else {
            return;
        };
        if let Some(overview) = page.downcast_ref::<OverviewPage>() {
            match action {
                PageAction::Refresh => overview.reload(),
                PageAction::Next => overview.focus_next(),
                PageAction::Prev => overview.focus_prev(),
                PageAction::Activate => overview.activate_focused(),
                PageAction::View(_) | PageAction::CycleView => {}
            }
            return;
        }
        if let Some(drive) = page.downcast_ref::<DrivePage>() {
            match action {
                PageAction::View(n) => drive.select_view(n),
                PageAction::CycleView => drive.cycle_view(),
                PageAction::Refresh => drive.refresh(),
                PageAction::Next | PageAction::Prev | PageAction::Activate => {}
            }
            return;
        }
        glib::g_debug!("gazania", "unhandled page action {action:?}");
    }
}
