use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;

use gtk4::prelude::*;

use crate::mpv::{ChapterItem, PlaylistEntry, TrackItem};
use crate::ui::timeline::format_time;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawerTab {
    Playlist,
    Tracks,
    Chapters,
}

#[derive(Debug, Clone)]
pub enum DrawerAction {
    PlayPlaylistIndex(usize),
    RemovePlaylistIndex(usize),
    ClearPlaylist,
    AddFile,
    AddFolder,
    OpenUrl,
    SelectAudioTrack(Option<i64>),
    SelectSubTrack(Option<i64>),
    LoadExternalSub,
    AdjustSubDelay(f64),
    ResetSubDelay,
    AdjustSubScale(f64),
    SelectChapter(usize),
}

#[derive(Clone)]
pub struct SideDrawer {
    pub revealer: gtk4::Revealer,
    stack: gtk4::Stack,
    active_tab: Rc<Cell<DrawerTab>>,
    btn_playlist: gtk4::Button,
    btn_tracks: gtk4::Button,
    btn_chapters: gtk4::Button,
    playlist_list: gtk4::Box,
    playlist_status: gtk4::Label,
    audio_list: gtk4::Box,
    sub_list: gtk4::Box,
    chapters_list: gtk4::Box,
    sub_delay_label: gtk4::Label,
    on_action: Rc<dyn Fn(DrawerAction)>,
}

impl SideDrawer {
    pub fn new<F>(on_action: F) -> Self
    where
        F: Fn(DrawerAction) + 'static,
    {
        let on_action_rc: Rc<dyn Fn(DrawerAction)> = Rc::new(on_action);

        let revealer = gtk4::Revealer::builder()
            .transition_type(gtk4::RevealerTransitionType::SlideLeft)
            .transition_duration(180)
            .reveal_child(false)
            .halign(gtk4::Align::End)
            .valign(gtk4::Align::Fill)
            .build();

        let panel = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
        panel.add_css_class("drawer-panel");
        panel.set_width_request(340);
        panel.set_vexpand(true);

        let tab_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        let btn_playlist = gtk4::Button::with_label("Playlist [p]");
        btn_playlist.add_css_class("hud-btn-primary");
        btn_playlist.set_hexpand(true);

        let btn_tracks = gtk4::Button::with_label("Tracks [t]");
        btn_tracks.add_css_class("hud-btn");
        btn_tracks.set_hexpand(true);

        let btn_chapters = gtk4::Button::with_label("Chapters [c]");
        btn_chapters.add_css_class("hud-btn");
        btn_chapters.set_hexpand(true);

        let close_btn = gtk4::Button::with_label("✕");
        close_btn.add_css_class("hud-btn");
        let rev_close = revealer.clone();
        close_btn.connect_clicked(move |_| {
            rev_close.set_reveal_child(false);
        });

        tab_bar.append(&btn_playlist);
        tab_bar.append(&btn_tracks);
        tab_bar.append(&btn_chapters);
        tab_bar.append(&close_btn);
        panel.append(&tab_bar);

        let stack = gtk4::Stack::builder()
            .transition_type(gtk4::StackTransitionType::Crossfade)
            .transition_duration(120)
            .vexpand(true)
            .build();

        let playlist_page = gtk4::Box::new(gtk4::Orientation::Vertical, 8);

        let playlist_status = gtk4::Label::new(Some("QUEUE EMPTY"));
        playlist_status.add_css_class("section-heading");
        playlist_status.set_halign(gtk4::Align::Start);
        playlist_status.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        playlist_page.append(&playlist_status);

        let playlist_actions = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);

        let add_file_btn = gtk4::Button::with_label("+ File [o]");
        add_file_btn.add_css_class("hud-btn-primary");
        let act_add = Rc::clone(&on_action_rc);
        add_file_btn.connect_clicked(move |_| act_add(DrawerAction::AddFile));

        let add_folder_btn = gtk4::Button::with_label("+ Folder [O]");
        add_folder_btn.add_css_class("hud-btn");
        let act_folder = Rc::clone(&on_action_rc);
        add_folder_btn.connect_clicked(move |_| act_folder(DrawerAction::AddFolder));

        let add_url_btn = gtk4::Button::with_label("+ URL [u]");
        add_url_btn.add_css_class("hud-btn");
        let act_url = Rc::clone(&on_action_rc);
        add_url_btn.connect_clicked(move |_| act_url(DrawerAction::OpenUrl));

        let clear_btn = gtk4::Button::with_label("Clear");
        clear_btn.add_css_class("hud-btn");
        let act_clear = Rc::clone(&on_action_rc);
        clear_btn.connect_clicked(move |_| act_clear(DrawerAction::ClearPlaylist));

        playlist_actions.append(&add_file_btn);
        playlist_actions.append(&add_folder_btn);
        playlist_actions.append(&add_url_btn);
        playlist_actions.append(&clear_btn);
        playlist_page.append(&playlist_actions);
        let playlist_scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vexpand(true)
            .build();
        let playlist_list = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        playlist_scroll.set_child(Some(&playlist_list));
        playlist_page.append(&playlist_scroll);
        stack.add_named(&playlist_page, Some("playlist"));

        let tracks_scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vexpand(true)
            .build();
        let tracks_page = gtk4::Box::new(gtk4::Orientation::Vertical, 12);

        let audio_hdr = gtk4::Label::new(Some("AUDIO TRACKS [a]"));
        audio_hdr.add_css_class("section-heading");
        audio_hdr.set_halign(gtk4::Align::Start);
        let audio_list = gtk4::Box::new(gtk4::Orientation::Vertical, 4);

        let sub_hdr_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        let sub_hdr = gtk4::Label::new(Some("SUBTITLE TRACKS [v]"));
        sub_hdr.add_css_class("section-heading");
        sub_hdr.set_halign(gtk4::Align::Start);
        sub_hdr.set_hexpand(true);

        let load_sub_btn = gtk4::Button::with_label("+ Load .srt/.ass");
        load_sub_btn.add_css_class("hud-btn-primary");
        let act_sub = Rc::clone(&on_action_rc);
        load_sub_btn.connect_clicked(move |_| act_sub(DrawerAction::LoadExternalSub));
        sub_hdr_row.append(&sub_hdr);
        sub_hdr_row.append(&load_sub_btn);

        let sub_list = gtk4::Box::new(gtk4::Orientation::Vertical, 4);

        let sub_tune_hdr = gtk4::Label::new(Some("SUBTITLE TIMING & SCALE"));
        sub_tune_hdr.add_css_class("section-heading");
        sub_tune_hdr.set_halign(gtk4::Align::Start);

        let delay_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        let sub_delay_label = gtk4::Label::new(Some("Delay: 0.0s"));
        sub_delay_label.set_halign(gtk4::Align::Start);
        sub_delay_label.set_hexpand(true);

        let delay_minus = gtk4::Button::with_label("-0.1s [z]");
        delay_minus.add_css_class("hud-btn");
        let act_dm = Rc::clone(&on_action_rc);
        delay_minus.connect_clicked(move |_| act_dm(DrawerAction::AdjustSubDelay(-0.1)));

        let delay_plus = gtk4::Button::with_label("+0.1s [x]");
        delay_plus.add_css_class("hud-btn");
        let act_dp = Rc::clone(&on_action_rc);
        delay_plus.connect_clicked(move |_| act_dp(DrawerAction::AdjustSubDelay(0.1)));

        let delay_reset = gtk4::Button::with_label("Reset");
        delay_reset.add_css_class("hud-btn");
        let act_dr = Rc::clone(&on_action_rc);
        delay_reset.connect_clicked(move |_| act_dr(DrawerAction::ResetSubDelay));

        delay_row.append(&sub_delay_label);
        delay_row.append(&delay_minus);
        delay_row.append(&delay_plus);
        delay_row.append(&delay_reset);

        let scale_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        let scale_lbl = gtk4::Label::new(Some("Font Scale"));
        scale_lbl.set_halign(gtk4::Align::Start);
        scale_lbl.set_hexpand(true);

        let scale_minus = gtk4::Button::with_label("A-");
        scale_minus.add_css_class("hud-btn");
        let act_sm = Rc::clone(&on_action_rc);
        scale_minus.connect_clicked(move |_| act_sm(DrawerAction::AdjustSubScale(-0.1)));

        let scale_plus = gtk4::Button::with_label("A+");
        scale_plus.add_css_class("hud-btn");
        let act_sp = Rc::clone(&on_action_rc);
        scale_plus.connect_clicked(move |_| act_sp(DrawerAction::AdjustSubScale(0.1)));

        scale_row.append(&scale_lbl);
        scale_row.append(&scale_minus);
        scale_row.append(&scale_plus);

        tracks_page.append(&audio_hdr);
        tracks_page.append(&audio_list);
        tracks_page.append(&sub_hdr_row);
        tracks_page.append(&sub_list);
        tracks_page.append(&sub_tune_hdr);
        tracks_page.append(&delay_row);
        tracks_page.append(&scale_row);
        tracks_scroll.set_child(Some(&tracks_page));
        stack.add_named(&tracks_scroll, Some("tracks"));

        let chapters_scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vexpand(true)
            .build();
        let chapters_list = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        chapters_scroll.set_child(Some(&chapters_list));
        stack.add_named(&chapters_scroll, Some("chapters"));

        panel.append(&stack);
        revealer.set_child(Some(&panel));

        let drawer = Self {
            revealer,
            stack,
            active_tab: Rc::new(Cell::new(DrawerTab::Playlist)),
            btn_playlist: btn_playlist.clone(),
            btn_tracks: btn_tracks.clone(),
            btn_chapters: btn_chapters.clone(),
            playlist_list,
            playlist_status,
            audio_list,
            sub_list,
            chapters_list,
            sub_delay_label,
            on_action: on_action_rc,
        };

        let d_pl = drawer.clone();
        btn_playlist.connect_clicked(move |_| d_pl.switch_tab(DrawerTab::Playlist));
        let d_tr = drawer.clone();
        btn_tracks.connect_clicked(move |_| d_tr.switch_tab(DrawerTab::Tracks));
        let d_ch = drawer.clone();
        btn_chapters.connect_clicked(move |_| d_ch.switch_tab(DrawerTab::Chapters));

        drawer
    }

    pub fn switch_tab(&self, tab: DrawerTab) {
        self.active_tab.set(tab);
        self.btn_playlist.remove_css_class("hud-btn-primary");
        self.btn_playlist.add_css_class("hud-btn");
        self.btn_tracks.remove_css_class("hud-btn-primary");
        self.btn_tracks.add_css_class("hud-btn");
        self.btn_chapters.remove_css_class("hud-btn-primary");
        self.btn_chapters.add_css_class("hud-btn");

        match tab {
            DrawerTab::Playlist => {
                self.btn_playlist.remove_css_class("hud-btn");
                self.btn_playlist.add_css_class("hud-btn-primary");
                self.stack.set_visible_child_name("playlist");
            }
            DrawerTab::Tracks => {
                self.btn_tracks.remove_css_class("hud-btn");
                self.btn_tracks.add_css_class("hud-btn-primary");
                self.stack.set_visible_child_name("tracks");
            }
            DrawerTab::Chapters => {
                self.btn_chapters.remove_css_class("hud-btn");
                self.btn_chapters.add_css_class("hud-btn-primary");
                self.stack.set_visible_child_name("chapters");
            }
        }
    }

    pub fn toggle_tab(&self, tab: DrawerTab) {
        if self.revealer.reveals_child() && self.active_tab.get() == tab {
            self.revealer.set_reveal_child(false);
        } else {
            self.switch_tab(tab);
            self.revealer.set_reveal_child(true);
        }
    }

    pub fn close(&self) {
        self.revealer.set_reveal_child(false);
    }

    pub fn is_open(&self) -> bool {
        self.revealer.reveals_child()
    }

    pub fn set_sub_delay(&self, delay: f64) {
        self.sub_delay_label.set_text(&format!("Delay: {delay:+.1}s"));
    }

    pub fn update_playlist(&self, entries: &[PlaylistEntry]) {
        clear_box(&self.playlist_list);
        if entries.is_empty() {
            self.playlist_status.set_text("QUEUE EMPTY");
            let empty = gtk4::Label::new(Some("Drop a file or season folder to queue episodes"));
            empty.add_css_class("muted-label");
            self.playlist_list.append(&empty);
            return;
        }

        let active_pos = entries
            .iter()
            .position(|e| e.current || e.playing)
            .map(|i| i + 1)
            .unwrap_or(1);
        let folder_hint = entries
            .iter()
            .find(|e| e.current || e.playing)
            .or_else(|| entries.first())
            .and_then(|e| Path::new(&e.filename).parent())
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("PLAYLIST");
        self.playlist_status.set_text(&format!(
            "{}  ·  EPISODE {} / {}",
            folder_hint.to_uppercase(),
            active_pos,
            entries.len()
        ));

        for (idx, item) in entries.iter().enumerate() {
            let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
            let display_name = item
                .title
                .clone()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    Path::new(&item.filename)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&item.filename)
                        .to_string()
                });

            let prefix = if item.current || item.playing {
                format!("▶ {:02}. ", idx + 1)
            } else {
                format!("   {:02}. ", idx + 1)
            };
            let play_btn = gtk4::Button::with_label(&format!("{prefix}{display_name}"));
            play_btn.set_hexpand(true);
            if item.current || item.playing {
                play_btn.add_css_class("drawer-row-active");
            } else {
                play_btn.add_css_class("drawer-row");
            }
            let act_play = Rc::clone(&self.on_action);
            play_btn.connect_clicked(move |_| act_play(DrawerAction::PlayPlaylistIndex(idx)));

            let rm_btn = gtk4::Button::with_label("✕");
            rm_btn.add_css_class("hud-btn");
            let act_rm = Rc::clone(&self.on_action);
            rm_btn.connect_clicked(move |_| act_rm(DrawerAction::RemovePlaylistIndex(idx)));

            row.append(&play_btn);
            row.append(&rm_btn);
            self.playlist_list.append(&row);
        }
    }

    pub fn update_tracks(&self, tracks: &[TrackItem]) {
        clear_box(&self.audio_list);
        clear_box(&self.sub_list);

        let audio_tracks: Vec<&TrackItem> = tracks
            .iter()
            .filter(|t| t.track_type == "audio")
            .collect();
        if audio_tracks.is_empty() {
            let lbl = gtk4::Label::new(Some("No audio tracks"));
            lbl.add_css_class("muted-label");
            self.audio_list.append(&lbl);
        } else {
            for tr in audio_tracks {
                let label = format_track_label(tr);
                let btn = gtk4::Button::with_label(&label);
                if tr.selected {
                    btn.add_css_class("drawer-row-active");
                } else {
                    btn.add_css_class("drawer-row");
                }
                let id = tr.id;
                let act = Rc::clone(&self.on_action);
                btn.connect_clicked(move |_| act(DrawerAction::SelectAudioTrack(Some(id))));
                self.audio_list.append(&btn);
            }
        }

        let sub_tracks: Vec<&TrackItem> =
            tracks.iter().filter(|t| t.track_type == "sub").collect();
        let any_sub_selected = sub_tracks.iter().any(|t| t.selected);

        let off_btn = gtk4::Button::with_label(if !any_sub_selected {
            "● Off / Disabled"
        } else {
            "○ Off / Disabled"
        });
        if !any_sub_selected {
            off_btn.add_css_class("drawer-row-active");
        } else {
            off_btn.add_css_class("drawer-row");
        }
        let act_off = Rc::clone(&self.on_action);
        off_btn.connect_clicked(move |_| act_off(DrawerAction::SelectSubTrack(None)));
        self.sub_list.append(&off_btn);

        for tr in sub_tracks {
            let label = format_track_label(tr);
            let btn = gtk4::Button::with_label(&label);
            if tr.selected {
                btn.add_css_class("drawer-row-active");
            } else {
                btn.add_css_class("drawer-row");
            }
            let id = tr.id;
            let act = Rc::clone(&self.on_action);
            btn.connect_clicked(move |_| act(DrawerAction::SelectSubTrack(Some(id))));
            self.sub_list.append(&btn);
        }
    }

    pub fn update_chapters(&self, chapters: &[ChapterItem], current_pos: f64) {
        clear_box(&self.chapters_list);
        if chapters.is_empty() {
            let lbl = gtk4::Label::new(Some("No chapters in media"));
            lbl.add_css_class("muted-label");
            self.chapters_list.append(&lbl);
            return;
        }

        let mut active_idx = None;
        for (i, ch) in chapters.iter().enumerate() {
            if ch.time <= current_pos + 0.05 {
                active_idx = Some(i);
            }
        }

        for (idx, ch) in chapters.iter().enumerate() {
            let title = ch
                .title
                .clone()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| format!("Chapter {}", idx + 1));
            let ts = format_time(ch.time);
            let btn = gtk4::Button::with_label(&format!("[{ts}]  {title}"));
            if active_idx == Some(idx) {
                btn.add_css_class("drawer-row-active");
            } else {
                btn.add_css_class("drawer-row");
            }
            let act = Rc::clone(&self.on_action);
            btn.connect_clicked(move |_| act(DrawerAction::SelectChapter(idx)));
            self.chapters_list.append(&btn);
        }
    }
}

fn format_track_label(tr: &TrackItem) -> String {
    let bullet = if tr.selected { "●" } else { "○" };
    let mut parts = vec![format!("{bullet} #{}", tr.id)];
    if let Some(title) = tr.title.as_ref().filter(|s| !s.is_empty()) {
        parts.push(title.clone());
    }
    if let Some(lang) = tr.lang.as_ref().filter(|s| !s.is_empty()) {
        parts.push(format!("[{}]", lang.to_uppercase()));
    }
    if let Some(codec) = tr.codec.as_ref().filter(|s| !s.is_empty()) {
        parts.push(codec.to_uppercase());
    }
    if tr.external {
        parts.push("(EXT)".to_string());
    }
    parts.join(" · ")
}

fn clear_box(container: &gtk4::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}
