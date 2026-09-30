use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;

use crate::mpv::{ChapterItem, TrackItem};
use crate::theme::ThemePalette;
use crate::ui::icons::{FleaIcon, IconGlyph, IconTone, icon_button, icon_label_button};
use crate::ui::timeline::{TimelineWidget, format_time};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudMode {
    Cinema,
    Minimal,
    Zen,
}

#[derive(Debug, Clone)]
pub enum HudAction {
    TogglePause,
    Prev,
    Next,
    ToggleMute,
    SetVolume(f64),
    CycleSpeed,
    CycleAudio,
    CycleSub,
    AbLoopCycle,
    Screenshot,
    ToggleFullscreen,
    TogglePlaylistDrawer,
    ToggleTracksDrawer,
    ToggleChaptersDrawer,
    OpenUrlDialog,
    ToggleKeymapSheet,
}

#[derive(Clone)]
pub struct PlayerHud {
    pub top_revealer: gtk4::Revealer,
    pub bottom_revealer: gtk4::Revealer,
    pub minimal_revealer: gtk4::Revealer,
    pub timeline: TimelineWidget,
    mode: Rc<Cell<HudMode>>,
    revealed: Rc<Cell<bool>>,
    show_remaining: Rc<Cell<bool>>,
    position: Rc<Cell<f64>>,
    duration: Rc<Cell<f64>>,
    last_displayed_sec: Rc<Cell<i64>>,
    updating_vol: Rc<Cell<bool>>,
    title_label: gtk4::Label,
    chapter_badge: gtk4::Button,
    chapter_label: gtk4::Label,
    spec_badge: gtk4::Label,
    play_icon: FleaIcon,
    mute_icon: FleaIcon,
    fs_icon: FleaIcon,
    loop_icon: FleaIcon,
    loop_label: gtk4::Label,
    icons: Rc<Vec<FleaIcon>>,
    elapsed_label: gtk4::Label,
    total_btn: gtk4::Button,
    vol_scale: gtk4::Scale,
    speed_btn: gtk4::Button,
    loop_btn: gtk4::Button,
    audio_btn: gtk4::Button,
    sub_btn: gtk4::Button,
    min_mode_badge: gtk4::Label,
    min_title_label: gtk4::Label,
    min_progress_label: gtk4::Label,
    min_time_label: gtk4::Label,
    min_tracks_label: gtk4::Label,
    min_vol_label: gtk4::Label,
    min_speed_label: gtk4::Label,
    video_dims: Rc<Cell<(i64, i64)>>,
    video_codec: Rc<RefCell<String>>,
    hwdec_name: Rc<RefCell<Option<String>>>,
}

impl PlayerHud {
    pub fn new<FAction, FSeek, FHover>(
        initial_mode: HudMode,
        palette: ThemePalette,
        on_action: FAction,
        on_seek: FSeek,
        on_hover: FHover,
    ) -> Self
    where
        FAction: Fn(HudAction) + 'static,
        FSeek: Fn(f64) + 'static,
        FHover: Fn(Option<(f64, f64, f64, Option<String>)>) + 'static,
    {
        let on_action_rc: Rc<dyn Fn(HudAction)> = Rc::new(on_action);
        let timeline = TimelineWidget::new(palette.clone(), on_seek, on_hover);
        let mut all_icons = Vec::new();

        // Top floating bar (Cinema mode)
        let top_revealer = gtk4::Revealer::builder()
            .transition_type(gtk4::RevealerTransitionType::Crossfade)
            .transition_duration(180)
            .reveal_child(initial_mode == HudMode::Cinema)
            .halign(gtk4::Align::Fill)
            .valign(gtk4::Align::Start)
            .margin_top(14)
            .margin_start(14)
            .margin_end(14)
            .build();

        let top_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
        top_bar.add_css_class("top-bar-pill");

        let title_label = gtk4::Label::new(Some("Omaplay"));
        title_label.add_css_class("title-label");
        title_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        title_label.set_halign(gtk4::Align::Start);
        title_label.set_hexpand(true);

        let (chapter_badge, ch_icon, chapter_label) = icon_label_button(
            IconGlyph::Chapters,
            "",
            14,
            IconTone::Accent,
            palette.clone(),
            "badge-accent",
        );
        all_icons.push(ch_icon);
        chapter_badge.set_visible(false);
        let act_ch = Rc::clone(&on_action_rc);
        chapter_badge.connect_clicked(move |_| act_ch(HudAction::ToggleChaptersDrawer));

        let spec_badge = gtk4::Label::new(None);
        spec_badge.add_css_class("badge-chip");
        spec_badge.set_visible(false);

        let (btn_url, url_icon) = icon_button(
            IconGlyph::Globe,
            15,
            IconTone::Foreground,
            palette.clone(),
            "hud-btn",
        );
        btn_url.set_tooltip_text(Some("Open Stream URL [u]"));
        all_icons.push(url_icon);
        let act_url = Rc::clone(&on_action_rc);
        btn_url.connect_clicked(move |_| act_url(HudAction::OpenUrlDialog));

        let (btn_tracks, tr_icon) = icon_button(
            IconGlyph::Sliders,
            15,
            IconTone::Foreground,
            palette.clone(),
            "hud-btn",
        );
        btn_tracks.set_tooltip_text(Some("Audio & Subtitle Tracks [t]"));
        all_icons.push(tr_icon);
        let act_tr = Rc::clone(&on_action_rc);
        btn_tracks.connect_clicked(move |_| act_tr(HudAction::ToggleTracksDrawer));

        let (btn_playlist, pl_icon) = icon_button(
            IconGlyph::List,
            15,
            IconTone::Foreground,
            palette.clone(),
            "hud-btn",
        );
        btn_playlist.set_tooltip_text(Some("Playlist Queue [p]"));
        all_icons.push(pl_icon);
        let act_pl = Rc::clone(&on_action_rc);
        btn_playlist.connect_clicked(move |_| act_pl(HudAction::TogglePlaylistDrawer));
        let (btn_keys, keys_icon) = icon_button(
            IconGlyph::Keyboard,
            16,
            IconTone::Foreground,
            palette.clone(),
            "hud-btn",
        );
        btn_keys.set_tooltip_text(Some("Keybindings [?]"));
        all_icons.push(keys_icon);
        let act_keys = Rc::clone(&on_action_rc);
        btn_keys.connect_clicked(move |_| act_keys(HudAction::ToggleKeymapSheet));

        top_bar.append(&title_label);
        top_bar.append(&chapter_badge);
        top_bar.append(&spec_badge);
        top_bar.append(&btn_url);
        top_bar.append(&btn_tracks);
        top_bar.append(&btn_playlist);
        top_bar.append(&btn_keys);
        top_revealer.set_child(Some(&top_bar));

        // Bottom floating pill (Cinema mode)
        let bottom_revealer = gtk4::Revealer::builder()
            .transition_type(gtk4::RevealerTransitionType::Crossfade)
            .transition_duration(180)
            .reveal_child(initial_mode == HudMode::Cinema)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::End)
            .margin_bottom(22)
            .margin_start(16)
            .margin_end(16)
            .build();

        let bottom_pill = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        bottom_pill.add_css_class("hud-pill");
        bottom_pill.set_width_request(640);

        // Row 1: Elapsed + Timeline + Total/Remaining
        let row1 = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
        let elapsed_label = gtk4::Label::new(Some("00:00"));
        elapsed_label.add_css_class("muted-label");

        let total_btn = gtk4::Button::with_label("00:00");
        total_btn.add_css_class("hud-btn");

        row1.append(&elapsed_label);
        row1.append(&timeline.area);
        row1.append(&total_btn);

        // Row 2: Transport & Controls with Flea stroke-2 glyphs
        let row2 = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);

        let (prev_btn, prev_icon) = icon_button(
            IconGlyph::SkipBack,
            16,
            IconTone::Foreground,
            palette.clone(),
            "hud-btn",
        );
        prev_btn.set_tooltip_text(Some("Previous Chapter / Track [Ctrl+Left / N]"));
        all_icons.push(prev_icon);
        let act_prev = Rc::clone(&on_action_rc);
        prev_btn.connect_clicked(move |_| act_prev(HudAction::Prev));

        let (play_btn, play_icon) = icon_button(
            IconGlyph::Pause,
            16,
            IconTone::Accent,
            palette.clone(),
            "hud-btn",
        );
        play_btn.set_tooltip_text(Some("Play / Pause [Space / k]"));
        all_icons.push(play_icon.clone());
        let act_play = Rc::clone(&on_action_rc);
        play_btn.connect_clicked(move |_| act_play(HudAction::TogglePause));

        let (next_btn, next_icon) = icon_button(
            IconGlyph::SkipForward,
            16,
            IconTone::Foreground,
            palette.clone(),
            "hud-btn",
        );
        next_btn.set_tooltip_text(Some("Next Chapter / Track [Ctrl+Right / n]"));
        all_icons.push(next_icon);
        let act_next = Rc::clone(&on_action_rc);
        next_btn.connect_clicked(move |_| act_next(HudAction::Next));

        let (mute_btn, mute_icon) = icon_button(
            IconGlyph::VolumeHigh,
            16,
            IconTone::Foreground,
            palette.clone(),
            "hud-btn",
        );
        mute_btn.set_tooltip_text(Some("Toggle Mute [m]"));
        all_icons.push(mute_icon.clone());
        let act_mute = Rc::clone(&on_action_rc);
        mute_btn.connect_clicked(move |_| act_mute(HudAction::ToggleMute));

        let vol_scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.0, 150.0, 1.0);
        vol_scale.add_css_class("seek-scale");
        vol_scale.set_draw_value(false);
        vol_scale.set_value(100.0);
        vol_scale.set_width_request(92);

        let updating_vol = Rc::new(Cell::new(false));
        let upd_vol_flag = Rc::clone(&updating_vol);
        let act_vol = Rc::clone(&on_action_rc);
        vol_scale.connect_value_changed(move |scale| {
            if !upd_vol_flag.get() {
                act_vol(HudAction::SetVolume(scale.value()));
            }
        });

        let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);

        let (loop_btn, loop_icon, loop_label) = icon_label_button(
            IconGlyph::Repeat,
            "A-B",
            14,
            IconTone::Foreground,
            palette.clone(),
            "hud-btn",
        );
        loop_btn.set_tooltip_text(Some("Cycle A-B Loop [r]"));
        all_icons.push(loop_icon.clone());
        let act_loop = Rc::clone(&on_action_rc);
        loop_btn.connect_clicked(move |_| act_loop(HudAction::AbLoopCycle));

        let speed_btn = gtk4::Button::with_label("1.00×");
        speed_btn.add_css_class("badge-chip");
        speed_btn.set_tooltip_text(Some("Playback Speed [[ / ]]"));
        let act_spd = Rc::clone(&on_action_rc);
        speed_btn.connect_clicked(move |_| act_spd(HudAction::CycleSpeed));

        let audio_btn = gtk4::Button::with_label("AUD");
        audio_btn.add_css_class("hud-btn");
        audio_btn.set_tooltip_text(Some("Cycle Audio Track [a]"));
        let act_aud = Rc::clone(&on_action_rc);
        audio_btn.connect_clicked(move |_| act_aud(HudAction::CycleAudio));

        let sub_btn = gtk4::Button::with_label("SUB");
        sub_btn.add_css_class("hud-btn");
        sub_btn.set_tooltip_text(Some("Cycle Subtitle Track [v]"));
        let act_sub = Rc::clone(&on_action_rc);
        sub_btn.connect_clicked(move |_| act_sub(HudAction::CycleSub));

        let (shot_btn, shot_icon) = icon_button(
            IconGlyph::Camera,
            16,
            IconTone::Foreground,
            palette.clone(),
            "hud-btn",
        );
        shot_btn.set_tooltip_text(Some("Screenshot [s / S]"));
        all_icons.push(shot_icon);
        let act_shot = Rc::clone(&on_action_rc);
        shot_btn.connect_clicked(move |_| act_shot(HudAction::Screenshot));

        let (fs_btn, fs_icon) = icon_button(
            IconGlyph::Maximize,
            16,
            IconTone::Foreground,
            palette,
            "hud-btn",
        );
        fs_btn.set_tooltip_text(Some("Toggle Fullscreen [f]"));
        all_icons.push(fs_icon.clone());
        let act_fs = Rc::clone(&on_action_rc);
        fs_btn.connect_clicked(move |_| act_fs(HudAction::ToggleFullscreen));

        row2.append(&prev_btn);
        row2.append(&play_btn);
        row2.append(&next_btn);
        row2.append(&mute_btn);
        row2.append(&vol_scale);
        row2.append(&spacer);
        row2.append(&loop_btn);
        row2.append(&speed_btn);
        row2.append(&audio_btn);
        row2.append(&sub_btn);
        row2.append(&shot_btn);
        row2.append(&fs_btn);

        bottom_pill.append(&row1);
        bottom_pill.append(&row2);
        bottom_revealer.set_child(Some(&bottom_pill));

        // Minimal Vim status strip (flush at bottom edge)
        let minimal_revealer = gtk4::Revealer::builder()
            .transition_type(gtk4::RevealerTransitionType::Crossfade)
            .transition_duration(140)
            .reveal_child(initial_mode == HudMode::Minimal)
            .halign(gtk4::Align::Fill)
            .valign(gtk4::Align::End)
            .build();

        let min_strip = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
        min_strip.add_css_class("minimal-strip");
        min_strip.set_height_request(26);

        let min_mode_badge = gtk4::Label::new(Some("[IDLE]"));
        min_mode_badge.add_css_class("badge-accent");

        let min_title_label = gtk4::Label::new(Some("Omaplay"));
        min_title_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        min_title_label.set_halign(gtk4::Align::Start);
        min_title_label.set_hexpand(true);

        let min_progress_label = gtk4::Label::new(Some("[----------]"));
        min_progress_label.add_css_class("muted-label");

        let min_time_label = gtk4::Label::new(Some("00:00 / 00:00"));
        let min_tracks_label = gtk4::Label::new(Some("[AID:- SID:-]"));
        min_tracks_label.add_css_class("muted-label");

        let min_vol_label = gtk4::Label::new(Some("[VOL:100%]"));
        let min_speed_label = gtk4::Label::new(Some("[1.00×]"));

        min_strip.append(&min_mode_badge);
        min_strip.append(&min_title_label);
        min_strip.append(&min_progress_label);
        min_strip.append(&min_time_label);
        min_strip.append(&min_tracks_label);
        min_strip.append(&min_vol_label);
        min_strip.append(&min_speed_label);
        minimal_revealer.set_child(Some(&min_strip));

        let show_remaining = Rc::new(Cell::new(false));
        let position = Rc::new(Cell::new(0.0));
        let duration = Rc::new(Cell::new(0.0));

        let hud = Self {
            top_revealer,
            bottom_revealer,
            minimal_revealer,
            timeline,
            mode: Rc::new(Cell::new(initial_mode)),
            revealed: Rc::new(Cell::new(true)),
            show_remaining: Rc::clone(&show_remaining),
            position: Rc::clone(&position),
            duration: Rc::clone(&duration),
            last_displayed_sec: Rc::new(Cell::new(-1)),
            updating_vol,
            title_label,
            chapter_badge,
            chapter_label,
            spec_badge,
            play_icon,
            mute_icon,
            fs_icon,
            loop_icon,
            loop_label,
            icons: Rc::new(all_icons),
            elapsed_label,
            total_btn: total_btn.clone(),
            vol_scale,
            speed_btn,
            loop_btn,
            audio_btn,
            sub_btn,
            min_mode_badge,
            min_title_label,
            min_progress_label,
            min_time_label,
            min_tracks_label,
            min_vol_label,
            min_speed_label,
            video_dims: Rc::new(Cell::new((0, 0))),
            video_codec: Rc::new(RefCell::new(String::new())),
            hwdec_name: Rc::new(RefCell::new(None)),
        };

        let hud_toggle_rem = hud.clone();
        total_btn.connect_clicked(move |_| {
            let cur = hud_toggle_rem.show_remaining.get();
            hud_toggle_rem.show_remaining.set(!cur);
            hud_toggle_rem.refresh_time_labels();
        });

        hud
    }

    pub fn mode(&self) -> HudMode {
        self.mode.get()
    }

    pub fn set_mode(&self, mode: HudMode) {
        self.mode.set(mode);
        self.apply_visibility();
    }

    pub fn toggle_mode(&self) -> HudMode {
        let next = match self.mode() {
            HudMode::Cinema => HudMode::Minimal,
            HudMode::Minimal => HudMode::Zen,
            HudMode::Zen => HudMode::Cinema,
        };
        self.set_mode(next);
        next
    }

    pub fn set_revealed(&self, revealed: bool) {
        if self.revealed.replace(revealed) != revealed {
            if revealed {
                let pos = self.position.get();
                self.timeline.set_position(pos);
                self.last_displayed_sec.set(pos.round() as i64);
                self.refresh_time_labels();
                self.refresh_chapter_badge();
            }
            self.apply_visibility();
        }
    }

    fn apply_visibility(&self) {
        let rev = self.revealed.get();
        match self.mode.get() {
            HudMode::Cinema => {
                self.top_revealer.set_reveal_child(rev);
                self.bottom_revealer.set_reveal_child(rev);
                self.minimal_revealer.set_reveal_child(false);
            }
            HudMode::Minimal => {
                self.top_revealer.set_reveal_child(false);
                self.bottom_revealer.set_reveal_child(false);
                self.minimal_revealer.set_reveal_child(rev);
            }
            HudMode::Zen => {
                self.top_revealer.set_reveal_child(false);
                self.bottom_revealer.set_reveal_child(false);
                self.minimal_revealer.set_reveal_child(false);
            }
        }
    }

    pub fn set_title(&self, title: &str) {
        let display = if title.trim().is_empty() {
            "Omaplay"
        } else {
            title.trim()
        };
        self.title_label.set_text(display);
        self.min_title_label.set_text(display);
    }

    pub fn set_paused(&self, paused: bool) {
        self.play_icon.set_glyph(if paused {
            IconGlyph::Play
        } else {
            IconGlyph::Pause
        });
        self.min_mode_badge
            .set_text(if paused { "[PAUSE]" } else { "[PLAY]" });
    }

    pub fn set_fullscreen(&self, fullscreen: bool) {
        self.fs_icon.set_glyph(if fullscreen {
            IconGlyph::Minimize
        } else {
            IconGlyph::Maximize
        });
    }

    pub fn set_position(&self, pos: f64) {
        let clamped = pos.max(0.0);
        self.position.set(clamped);
        if !self.revealed.get() {
            return;
        }
        self.timeline.set_position(clamped);
        let sec = clamped.round() as i64;
        if self.last_displayed_sec.replace(sec) != sec {
            self.refresh_time_labels();
            self.refresh_chapter_badge();
        }
    }

    pub fn set_duration(&self, dur: f64) {
        self.duration.set(dur.max(0.0));
        self.timeline.set_duration(dur);
        self.last_displayed_sec.set(-1);
        if self.revealed.get() {
            self.refresh_time_labels();
        }
    }

    fn refresh_time_labels(&self) {
        let pos = self.position.get();
        let dur = self.duration.get();
        let pos_s = format_time(pos);
        let dur_s = format_time(dur);
        self.elapsed_label.set_text(&pos_s);

        if self.show_remaining.get() && dur > 0.0 {
            let rem = (dur - pos).max(0.0);
            self.total_btn.set_label(&format!("-{}", format_time(rem)));
        } else {
            self.total_btn.set_label(&dur_s);
        }

        self.min_time_label.set_text(&format!("{pos_s} / {dur_s}"));

        let ratio = if dur > 0.0 {
            (pos / dur).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let filled = (ratio * 10.0).round() as usize;
        let bar = format!(
            "[{}{}]",
            "=".repeat(filled.min(10)),
            "-".repeat(10usize.saturating_sub(filled.min(10)))
        );
        self.min_progress_label.set_text(&bar);
    }

    fn refresh_chapter_badge(&self) {
        if let Some(ch) = self.timeline.current_chapter_title() {
            self.chapter_label.set_text(&ch);
            self.chapter_badge.set_visible(true);
        } else {
            self.chapter_badge.set_visible(false);
        }
    }

    pub fn set_chapters(&self, chapters: Vec<ChapterItem>) {
        self.timeline.set_chapters(chapters);
        self.refresh_chapter_badge();
    }

    pub fn set_volume(&self, vol: f64, muted: bool) {
        self.updating_vol.set(true);
        self.vol_scale.set_value(vol.clamp(0.0, 150.0));
        self.updating_vol.set(false);

        let glyph = if muted || vol <= 0.1 {
            IconGlyph::VolumeMute
        } else if vol < 50.0 {
            IconGlyph::VolumeLow
        } else {
            IconGlyph::VolumeHigh
        };
        self.mute_icon.set_glyph(glyph);

        if muted {
            self.min_vol_label.set_text("[MUTE]");
        } else {
            self.min_vol_label
                .set_text(&format!("[VOL:{:.0}%]", vol.round()));
        }
    }

    pub fn set_speed(&self, speed: f64) {
        let lbl = format!("{speed:.2}×");
        self.speed_btn.set_label(&lbl);
        self.min_speed_label.set_text(&format!("[{lbl}]"));
    }

    pub fn set_loop_points(&self, a: Option<f64>, b: Option<f64>) {
        self.timeline.set_loop_points(a, b);
        match (a, b) {
            (Some(av), Some(bv)) => {
                self.loop_label
                    .set_text(&format!("{}→{}", format_time(av), format_time(bv)));
                self.loop_icon.set_tone(IconTone::Yellow);
                self.loop_btn.remove_css_class("hud-btn");
                self.loop_btn.add_css_class("badge-loop");
            }
            (Some(av), None) => {
                self.loop_label
                    .set_text(&format!("A={}…", format_time(av)));
                self.loop_icon.set_tone(IconTone::Yellow);
                self.loop_btn.remove_css_class("hud-btn");
                self.loop_btn.add_css_class("badge-loop");
            }
            _ => {
                self.loop_label.set_text("A-B");
                self.loop_icon.set_tone(IconTone::Foreground);
                self.loop_btn.remove_css_class("badge-loop");
                self.loop_btn.add_css_class("hud-btn");
            }
        }
    }

    pub fn set_tracks(&self, tracks: &[TrackItem], hwdec: Option<String>) {
        if hwdec.is_some() {
            *self.hwdec_name.borrow_mut() = hwdec;
        }

        let vcodec = tracks
            .iter()
            .find(|t| t.track_type == "video" && t.selected)
            .and_then(|t| t.codec.clone())
            .unwrap_or_default();
        *self.video_codec.borrow_mut() = vcodec;
        self.refresh_spec_badge();

        let aid_str = tracks
            .iter()
            .find(|t| t.track_type == "audio" && t.selected)
            .map(|t| {
                t.lang
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .map(|l| format!("AUD:{}", l.to_uppercase()))
                    .unwrap_or_else(|| format!("AUD:#{}", t.id))
            })
            .unwrap_or_else(|| "AUD:-".to_string());
        self.audio_btn.set_label(&aid_str);

        let sid_str = tracks
            .iter()
            .find(|t| t.track_type == "sub" && t.selected)
            .map(|t| {
                t.lang
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .map(|l| format!("SUB:{}", l.to_uppercase()))
                    .unwrap_or_else(|| format!("SUB:#{}", t.id))
            })
            .unwrap_or_else(|| "SUB:OFF".to_string());
        self.sub_btn.set_label(&sid_str);

        self.min_tracks_label
            .set_text(&format!("[{aid_str} {sid_str}]"));
    }

    pub fn set_video_dimensions(&self, w: i64, h: i64, hwdec: Option<String>) {
        self.video_dims.set((w, h));
        if hwdec.is_some() {
            *self.hwdec_name.borrow_mut() = hwdec;
        }
        self.refresh_spec_badge();
    }

    pub fn set_hwdec(&self, hwdec: Option<String>) {
        *self.hwdec_name.borrow_mut() = hwdec;
        self.refresh_spec_badge();
    }

    fn refresh_spec_badge(&self) {
        let (w, h) = self.video_dims.get();
        if w <= 0 || h <= 0 {
            self.spec_badge.set_visible(false);
            return;
        }
        // Check both width and height so widescreen/cinemascope (e.g. 1920x800) maps to 1080p
        let res = if w >= 3800 || h >= 2160 {
            "4K".to_string()
        } else if w >= 2500 || h >= 1440 {
            "1440p".to_string()
        } else if w >= 1900 || h >= 1080 {
            "1080p".to_string()
        } else if w >= 1260 || h >= 720 {
            "720p".to_string()
        } else if w >= 840 || h >= 480 {
            "480p".to_string()
        } else {
            format!("{w}×{h}")
        };

        let mut parts = vec![res];
        let codec = self.video_codec.borrow().clone();
        if !codec.is_empty() {
            parts.push(codec.to_uppercase());
        }
        if let Some(hw) = self
            .hwdec_name
            .borrow()
            .as_deref()
            .filter(|s| !s.is_empty() && *s != "no")
        {
            parts.push(format!("HW ({})", hw.to_uppercase()));
        } else {
            parts.push("SW".to_string());
        }
        self.spec_badge.set_text(&parts.join(" · "));
        self.spec_badge.set_visible(true);
    }

    pub fn set_palette(&self, palette: ThemePalette) {
        self.timeline.set_palette(palette.clone());
        for icon in self.icons.iter() {
            icon.set_palette(palette.clone());
        }
    }
}
