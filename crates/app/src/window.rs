use crate::application::Application;
use crate::config::APP_ID;
use adw::subclass::prelude::*;
use gtk::{gio, glib, prelude::*, CompositeTemplate};

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/io/github/brianirish/Zinnia/window.ui")]
    pub struct Window {
        #[template_child]
        pub navigation: TemplateChild<adw::NavigationView>,
        #[template_child]
        pub toasts: TemplateChild<adw::ToastOverlay>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "ZinniaWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
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
        settings.bind("window-height", self, "default-height").build();
        settings.bind("is-maximized", self, "maximized").build();
    }

    pub fn navigation(&self) -> adw::NavigationView {
        self.imp().navigation.get()
    }

    pub fn toast(&self, text: &str) {
        self.imp().toasts.add_toast(adw::Toast::new(text));
    }
}
