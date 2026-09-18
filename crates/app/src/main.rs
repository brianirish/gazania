mod application;
mod config;
mod pages;
mod shortcuts;
mod theme;
mod widgets;
mod window;

use gtk::{gio, glib, prelude::*};

fn main() -> glib::ExitCode {
    gio::resources_register_include!("gazania.gresource")
        .expect("gresource is compiled into the binary by build.rs");
    application::Application::new().run()
}
