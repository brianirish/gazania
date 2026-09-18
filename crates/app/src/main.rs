mod application;
mod config;
mod shortcuts;
mod widgets;
mod window;

use gtk::{gio, glib, prelude::*};

fn main() -> glib::ExitCode {
    gio::resources_register_include!("zinnia.gresource")
        .expect("gresource is compiled into the binary by build.rs");
    application::Application::new().run()
}
