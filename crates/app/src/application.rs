use crate::config::{APP_ID, RESOURCE_PATH};
use crate::window::Window;
use adw::subclass::prelude::*;
use gtk::{gdk, gio, glib, prelude::*};
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Application {
        /// Optional path or volume given on the command line. Recorded only;
        /// sub-project 2 opens it.
        pub requested_target: RefCell<Option<String>>,
        pub theme_monitor: RefCell<Option<gio::FileMonitor>>,
        pub palette: RefCell<Option<crate::theme::omarchy::Palette>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Application {
        const NAME: &'static str = "GazaniaApplication";
        type Type = super::Application;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for Application {}

    impl ApplicationImpl for Application {
        fn startup(&self) {
            self.parent_startup();
            let provider = gtk::CssProvider::new();
            provider.load_from_resource(&format!("{RESOURCE_PATH}/style.css"));
            if let Some(display) = gdk::Display::default() {
                gtk::style_context_add_provider_for_display(
                    &display,
                    &provider,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            }

            let app = self.obj();
            let quit = gio::ActionEntry::builder("quit")
                .activate(|app: &super::Application, _, _| app.quit())
                .build();
            app.add_action_entries([quit]);

            let accels: &[(&str, &[&str])] = &[
                ("app.quit", &["<Control>q"]),
                ("win.refresh", &["<Control>r"]),
                ("win.cycle-view", &["<Control>Tab"]),
                ("win.shortcuts", &["question"]),
            ];
            for (action, keys) in accels {
                app.set_accels_for_action(action, keys);
            }

            crate::theme::install(&app);
        }

        fn activate(&self) {
            let app = self.obj();
            if let Some(window) = app.active_window() {
                window.present();
                return;
            }
            Window::new(&app).present();
        }

        fn command_line(&self, command_line: &gio::ApplicationCommandLine) -> glib::ExitCode {
            let args: Vec<String> = command_line
                .arguments()
                .into_iter()
                .filter_map(|a| a.into_string().ok())
                .collect();
            *self.requested_target.borrow_mut() = args.get(1).cloned();
            self.obj().activate();
            glib::ExitCode::SUCCESS
        }
    }

    impl GtkApplicationImpl for Application {}
    impl AdwApplicationImpl for Application {}
}

glib::wrapper! {
    pub struct Application(ObjectSubclass<imp::Application>)
        @extends adw::Application, gtk::Application, gio::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}

impl Application {
    pub fn new() -> Self {
        glib::Object::builder()
            .property("application-id", APP_ID)
            .property("flags", gio::ApplicationFlags::HANDLES_COMMAND_LINE)
            .property("resource-base-path", RESOURCE_PATH)
            .build()
    }

    pub fn requested_target(&self) -> Option<String> {
        self.imp().requested_target.borrow().clone()
    }

    /// Chart hues from the active Omarchy theme, if any.
    pub fn palette(&self) -> Option<crate::theme::omarchy::Palette> {
        self.imp().palette.borrow().clone()
    }
}
