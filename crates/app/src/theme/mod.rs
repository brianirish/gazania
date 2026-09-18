//! Live theming from Omarchy. Does nothing on systems without Omarchy.

pub mod omarchy;

use crate::application::Application;
use gtk::subclass::prelude::ObjectSubclassIsExt;
use gtk::{gdk, gio, glib, prelude::*};
use std::path::PathBuf;

fn state_dir() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| glib::home_dir().join(".local/state"))
}

#[allow(deprecated)]
pub fn install(app: &Application) {
    let current = state_dir().join("omarchy/current");
    let colors = current.join("theme/colors.toml");
    if !colors.exists() {
        return;
    }
    let Some(display) = gdk::Display::default() else {
        return;
    };

    let provider = gtk::CssProvider::new();
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    apply(app, &provider, &colors);

    // Omarchy swaps the whole `current/theme` directory on a theme change, so a
    // monitor on the file itself would go stale. Watch the parent directory.
    match gio::File::for_path(&current)
        .monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
    {
        Ok(monitor) => {
            monitor.connect_changed(glib::clone!(
                #[weak]
                app,
                #[strong]
                provider,
                #[strong]
                colors,
                move |_, _, _, _| apply(&app, &provider, &colors)
            ));
            app.imp().theme_monitor.replace(Some(monitor));
        }
        Err(e) => glib::g_debug!("zinnia", "theme monitor unavailable: {e}"),
    }
}

fn apply(app: &Application, provider: &gtk::CssProvider, colors: &PathBuf) {
    match std::fs::read_to_string(colors)
        .ok()
        .and_then(|t| omarchy::parse(&t))
    {
        Some(theme) => {
            provider.load_from_string(&omarchy::css(&theme));
            app.imp().palette.replace(Some(theme.palette));
        }
        None => {
            provider.load_from_string("");
            app.imp().palette.replace(None);
            glib::g_debug!(
                "zinnia",
                "omarchy colors unreadable at {}, using stock look",
                colors.display()
            );
        }
    }
}
