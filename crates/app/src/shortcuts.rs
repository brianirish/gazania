//! Keyboard shortcuts dialog. Built in code so it never drifts from the accels.

use adw::prelude::*;

pub fn present(parent: &gtk::Widget) {
    let dialog = adw::ShortcutsDialog::new();

    let nav = adw::ShortcutsSection::new(Some("Navigation"));
    nav.add(adw::ShortcutsItem::new("Move down", "j Down"));
    nav.add(adw::ShortcutsItem::new("Move up", "k Up"));
    nav.add(adw::ShortcutsItem::new("Open selected", "l Return"));
    nav.add(adw::ShortcutsItem::new("Go back", "h Escape"));
    dialog.add(nav);

    let views = adw::ShortcutsSection::new(Some("Drive page"));
    views.add(adw::ShortcutsItem::new("Usage", "1"));
    views.add(adw::ShortcutsItem::new("Health", "2"));
    views.add(adw::ShortcutsItem::new("Benchmark", "3"));
    views.add(adw::ShortcutsItem::new("Details", "4"));
    views.add(adw::ShortcutsItem::new("Cycle views", "<Control>Tab"));
    dialog.add(views);

    let general = adw::ShortcutsSection::new(Some("General"));
    general.add(adw::ShortcutsItem::new("Refresh", "<Control>r"));
    general.add(adw::ShortcutsItem::new("Keyboard shortcuts", "question"));
    general.add(adw::ShortcutsItem::new("Quit", "<Control>q"));
    dialog.add(general);

    dialog.present(Some(parent));
}
