use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gtk4::prelude::*;

use crate::mpv::thumbnailer::{THUMB_HEIGHT, THUMB_WIDTH};
use crate::ui::timeline::format_time;

pub struct WelcomeOverlay {
    pub revealer: gtk4::Revealer,
}

impl WelcomeOverlay {
    pub fn new<FOpen, FUrl, FKeymap>(
        on_open_file: FOpen,
        on_open_url: FUrl,
        on_keymap: FKeymap,
    ) -> Self
    where
        FOpen: Fn() + 'static,
        FUrl: Fn() + 'static,
        FKeymap: Fn() + 'static,
    {
        let revealer = gtk4::Revealer::builder()
            .transition_type(gtk4::RevealerTransitionType::Crossfade)
            .transition_duration(180)
            .reveal_child(true)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .build();

        let card = gtk4::Box::new(gtk4::Orientation::Vertical, 14);
        card.add_css_class("welcome-card");
        card.set_halign(gtk4::Align::Center);
        card.set_valign(gtk4::Align::Center);

        let emblem = gtk4::Label::new(Some("OMAPLAY"));
        emblem.add_css_class("title-label");

        let sub = gtk4::Label::new(Some(
            "Drop a video file, subtitle (.srt/.ass), or stream URL anywhere",
        ));
        sub.add_css_class("muted-label");

        let btn_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
        btn_row.set_halign(gtk4::Align::Center);

        let open_btn = gtk4::Button::with_label("Open File  [o]");
        open_btn.add_css_class("hud-btn-primary");
        open_btn.connect_clicked(move |_| on_open_file());

        let url_btn = gtk4::Button::with_label("Open Stream URL  [u]");
        url_btn.add_css_class("hud-btn");
        url_btn.connect_clicked(move |_| on_open_url());

        let keys_btn = gtk4::Button::with_label("Keybindings  [?]");
        keys_btn.add_css_class("hud-btn");
        keys_btn.connect_clicked(move |_| on_keymap());
        btn_row.append(&open_btn);
        btn_row.append(&url_btn);
        btn_row.append(&keys_btn);

        card.append(&emblem);
        card.append(&sub);
        card.append(&btn_row);

        revealer.set_child(Some(&card));
        Self { revealer }
    }

    pub fn set_visible(&self, visible: bool) {
        self.revealer.set_reveal_child(visible);
    }
}

#[derive(Clone)]
pub struct UrlDialog {
    pub revealer: gtk4::Revealer,
    entry: gtk4::Entry,
}

impl UrlDialog {
    pub fn new<FSubmit>(on_submit: FSubmit) -> Self
    where
        FSubmit: Fn(String, bool) + 'static,
    {
        let revealer = gtk4::Revealer::builder()
            .transition_type(gtk4::RevealerTransitionType::Crossfade)
            .transition_duration(150)
            .reveal_child(false)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .build();

        let card = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        card.add_css_class("modal-card");
        card.set_width_request(520);

        let header = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
        let title = gtk4::Label::new(Some("OPEN STREAM / REMOTE URL"));
        title.add_css_class("title-label");
        title.set_halign(gtk4::Align::Start);
        title.set_hexpand(true);

        let hint = gtk4::Label::new(Some("HTTP / HLS / YouTube / yt-dlp"));
        hint.add_css_class("muted-label");

        let close_btn = gtk4::Button::with_label("✕");
        close_btn.add_css_class("hud-btn");
        let rev_close = revealer.clone();
        close_btn.connect_clicked(move |_| {
            rev_close.set_reveal_child(false);
        });

        header.append(&title);
        header.append(&hint);
        header.append(&close_btn);

        let entry = gtk4::Entry::builder()
            .placeholder_text("https://...")
            .hexpand(true)
            .build();

        let on_submit_rc = Rc::new(on_submit);

        let entry_submit = Rc::clone(&on_submit_rc);
        let rev_enter = revealer.clone();
        entry.connect_activate(move |e| {
            let text = e.text().trim().to_string();
            if !text.is_empty() {
                e.set_text("");
                rev_enter.set_reveal_child(false);
                entry_submit(text, false);
            }
        });

        let key_ctrl = gtk4::EventControllerKey::new();
        let rev_esc = revealer.clone();
        key_ctrl.connect_key_pressed(move |_ctrl, keyval, _code, _state| {
            if keyval == gdk4::Key::Escape {
                rev_esc.set_reveal_child(false);
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        entry.add_controller(key_ctrl);

        let actions = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        actions.set_halign(gtk4::Align::End);

        let append_btn = gtk4::Button::with_label("+ Append to Queue");
        append_btn.add_css_class("hud-btn");
        let entry_append = entry.clone();
        let rev_append = revealer.clone();
        let submit_append = Rc::clone(&on_submit_rc);
        append_btn.connect_clicked(move |_| {
            let text = entry_append.text().trim().to_string();
            if !text.is_empty() {
                entry_append.set_text("");
                rev_append.set_reveal_child(false);
                submit_append(text, true);
            }
        });

        let play_btn = gtk4::Button::with_label("▶ Play Now");
        play_btn.add_css_class("hud-btn-primary");
        let entry_play = entry.clone();
        let rev_play = revealer.clone();
        let submit_play = Rc::clone(&on_submit_rc);
        play_btn.connect_clicked(move |_| {
            let text = entry_play.text().trim().to_string();
            if !text.is_empty() {
                entry_play.set_text("");
                rev_play.set_reveal_child(false);
                submit_play(text, false);
            }
        });

        actions.append(&append_btn);
        actions.append(&play_btn);

        card.append(&header);
        card.append(&entry);
        card.append(&actions);

        revealer.set_child(Some(&card));
        Self { revealer, entry }
    }

    pub fn open(&self) {
        self.revealer.set_reveal_child(true);
        let _ = self.entry.grab_focus();
    }

    pub fn close(&self) {
        self.revealer.set_reveal_child(false);
    }

    pub fn is_open(&self) -> bool {
        self.revealer.reveals_child()
    }
}

#[derive(Clone)]
pub struct CommandBar {
    pub revealer: gtk4::Revealer,
    entry: gtk4::Entry,
}

impl CommandBar {
    pub fn new<FSubmit>(on_submit: FSubmit) -> Self
    where
        FSubmit: Fn(String) + 'static,
    {
        let revealer = gtk4::Revealer::builder()
            .transition_type(gtk4::RevealerTransitionType::SlideUp)
            .transition_duration(120)
            .reveal_child(false)
            .halign(gtk4::Align::Fill)
            .valign(gtk4::Align::End)
            .build();

        let bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        bar.add_css_class("command-bar");

        let colon = gtk4::Label::new(Some(":"));
        colon.add_css_class("section-heading");

        let entry = gtk4::Entry::builder()
            .placeholder_text("seek 01:30 | speed 1.25 | vol 90 | sub-delay -0.2 | open <url> | theme | q")
            .hexpand(true)
            .build();
        entry.add_css_class("command-entry");

        let hint = gtk4::Label::new(Some("Enter to run · Esc to cancel"));
        hint.add_css_class("muted-label");

        let rev_act = revealer.clone();
        entry.connect_activate(move |e| {
            let text = e.text().trim().to_string();
            e.set_text("");
            rev_act.set_reveal_child(false);
            if !text.is_empty() {
                on_submit(text);
            }
        });

        let key_ctrl = gtk4::EventControllerKey::new();
        let rev_esc = revealer.clone();
        let entry_esc = entry.clone();
        key_ctrl.connect_key_pressed(move |_ctrl, keyval, _code, _state| {
            if keyval == gdk4::Key::Escape {
                entry_esc.set_text("");
                rev_esc.set_reveal_child(false);
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        entry.add_controller(key_ctrl);

        bar.append(&colon);
        bar.append(&entry);
        bar.append(&hint);

        revealer.set_child(Some(&bar));
        Self { revealer, entry }
    }

    pub fn open(&self) {
        self.revealer.set_reveal_child(true);
        let _ = self.entry.grab_focus();
    }

    pub fn close(&self) {
        self.entry.set_text("");
        self.revealer.set_reveal_child(false);
    }

    pub fn is_open(&self) -> bool {
        self.revealer.reveals_child()
    }
}

#[derive(Clone)]
pub struct OsdToast {
    pub revealer: gtk4::Revealer,
    label: gtk4::Label,
    generation: Rc<Cell<u64>>,
}

impl OsdToast {
    pub fn new() -> Self {
        let revealer = gtk4::Revealer::builder()
            .transition_type(gtk4::RevealerTransitionType::Crossfade)
            .transition_duration(140)
            .reveal_child(false)
            .halign(gtk4::Align::End)
            .valign(gtk4::Align::Start)
            .margin_top(64)
            .margin_end(20)
            .can_target(false)
            .build();

        let pill = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        pill.add_css_class("toast-pill");

        let label = gtk4::Label::new(None);
        pill.append(&label);
        revealer.set_child(Some(&pill));

        Self {
            revealer,
            label,
            generation: Rc::new(Cell::new(0)),
        }
    }

    pub fn show(&self, message: &str) {
        self.label.set_text(message);
        self.revealer.set_reveal_child(true);

        let next_gen = self.generation.get().wrapping_add(1);
        self.generation.set(next_gen);

        let gen_ref = Rc::clone(&self.generation);
        let rev_ref = self.revealer.clone();
        glib::timeout_add_local_once(Duration::from_millis(1400), move || {
            if gen_ref.get() == next_gen {
                rev_ref.set_reveal_child(false);
            }
        });
    }
}

#[derive(Clone)]
pub struct ThumbnailPreviewPopup {
    pub revealer: gtk4::Revealer,
    picture: gtk4::Picture,
    time_label: gtk4::Label,
    chapter_label: gtk4::Label,
}

impl ThumbnailPreviewPopup {
    pub fn new() -> Self {
        let revealer = gtk4::Revealer::builder()
            .transition_type(gtk4::RevealerTransitionType::Crossfade)
            .transition_duration(90)
            .reveal_child(false)
            .halign(gtk4::Align::Start)
            .valign(gtk4::Align::End)
            .margin_bottom(98)
            .can_target(false)
            .build();

        let card = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        card.add_css_class("thumb-preview-card");

        let picture = gtk4::Picture::builder()
            .width_request(THUMB_WIDTH as i32)
            .height_request(THUMB_HEIGHT as i32)
            .can_shrink(true)
            .build();

        let time_label = gtk4::Label::new(Some("00:00"));
        time_label.add_css_class("badge-accent");
        time_label.set_halign(gtk4::Align::Center);

        let chapter_label = gtk4::Label::new(None);
        chapter_label.add_css_class("muted-label");
        chapter_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        chapter_label.set_max_width_chars(22);
        chapter_label.set_visible(false);

        card.append(&picture);
        card.append(&time_label);
        card.append(&chapter_label);

        revealer.set_child(Some(&card));

        Self {
            revealer,
            picture,
            time_label,
            chapter_label,
        }
    }

    pub fn update_hover(
        &self,
        window_width: i32,
        timeline_x: f64,
        timeline_width: f64,
        timestamp: f64,
        chapter_title: Option<&str>,
    ) {
        self.time_label.set_text(&format_time(timestamp));
        if let Some(ch) = chapter_title.filter(|s| !s.is_empty()) {
            self.chapter_label.set_text(ch);
            self.chapter_label.set_visible(true);
        } else {
            self.chapter_label.set_visible(false);
        }

        let card_w = (THUMB_WIDTH as i32) + 12;
        let hud_w = timeline_width.max(1.0) as i32;
        let hud_left = ((window_width - hud_w) / 2).max(16);
        let center_x = hud_left + (timeline_x as i32);
        let left_margin = (center_x - card_w / 2).clamp(12, (window_width - card_w - 12).max(12));
        self.revealer.set_margin_start(left_margin);
        self.revealer.set_reveal_child(true);
    }

    pub fn set_texture(&self, rgba: &[u8], width: u32, height: u32) {
        let bytes = glib::Bytes::from(rgba);
        let texture = gdk4::MemoryTexture::new(
            width as i32,
            height as i32,
            gdk4::MemoryFormat::R8g8b8a8,
            &bytes,
            (width * 4) as usize,
        );
        self.picture.set_paintable(Some(&texture));
        self.picture.set_visible(true);
    }

    pub fn hide(&self) {
        self.revealer.set_reveal_child(false);
    }
}
