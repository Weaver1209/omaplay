use gtk4::prelude::*;

use crate::keymap::KEYMAP_GROUPS;

pub struct KeymapSheet {
    pub revealer: gtk4::Revealer,
}

impl KeymapSheet {
    pub fn new<F>(on_close: F) -> Self
    where
        F: Fn() + 'static,
    {
        let revealer = gtk4::Revealer::builder()
            .transition_type(gtk4::RevealerTransitionType::Crossfade)
            .transition_duration(150)
            .reveal_child(false)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .build();

        let card = gtk4::Box::new(gtk4::Orientation::Vertical, 14);
        card.add_css_class("keymap-card");
        card.set_width_request(680);

        let header = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        let title = gtk4::Label::new(Some("OMAPLAY KEYBINDINGS"));
        title.add_css_class("title-label");
        title.set_halign(gtk4::Align::Start);
        title.set_hexpand(true);

        let hint = gtk4::Label::new(Some("Press ? or Esc to close"));
        hint.add_css_class("muted-label");

        let close_btn = gtk4::Button::with_label("✕");
        close_btn.add_css_class("hud-btn");
        close_btn.connect_clicked(move |_| {
            on_close();
        });

        header.append(&title);
        header.append(&hint);
        header.append(&close_btn);
        card.append(&header);

        let columns = gtk4::Box::new(gtk4::Orientation::Horizontal, 24);
        columns.set_homogeneous(true);

        let left_col = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        let right_col = gtk4::Box::new(gtk4::Orientation::Vertical, 12);

        for (idx, group) in KEYMAP_GROUPS.iter().enumerate() {
            let group_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
            let group_title = gtk4::Label::new(Some(group.title));
            group_title.add_css_class("section-heading");
            group_title.set_halign(gtk4::Align::Start);
            group_box.append(&group_title);

            for entry in group.entries {
                let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
                let key_lbl = gtk4::Label::new(Some(entry.keys));
                key_lbl.add_css_class("keycap");
                key_lbl.set_halign(gtk4::Align::Start);

                let desc_lbl = gtk4::Label::new(Some(entry.description));
                desc_lbl.set_halign(gtk4::Align::End);
                desc_lbl.set_hexpand(true);

                row.append(&key_lbl);
                row.append(&desc_lbl);
                group_box.append(&row);
            }

            if idx == 0 {
                left_col.append(&group_box);
            } else {
                right_col.append(&group_box);
            }
        }

        columns.append(&left_col);
        columns.append(&right_col);
        card.append(&columns);

        revealer.set_child(Some(&card));
        Self { revealer }
    }

    pub fn is_visible(&self) -> bool {
        self.revealer.reveals_child()
    }

    pub fn set_visible(&self, visible: bool) {
        self.revealer.set_reveal_child(visible);
    }

    pub fn toggle(&self) {
        self.set_visible(!self.is_visible());
    }
}
