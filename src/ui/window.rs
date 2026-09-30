use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk4::prelude::*;
use libadwaita::prelude::*;

use crate::keymap::{PlayerAction, VimCommand, map_key_event, parse_vim_command};
use crate::mpris::{MprisCommand, MprisServer};
use crate::mpv::{
    ChapterItem, MpvPlayer, PlayerEvent, ThumbnailFrame, ThumbnailGenerator, TrackItem,
    collect_media_in_dir, collect_sibling_episodes, ffi,
};
use crate::theme::{ThemePalette, ThemeWatcher, install_global_css};
use crate::ui::dialogs::{
    CommandBar, OsdToast, ThumbnailPreviewPopup, UrlDialog, WelcomeOverlay,
};
use crate::ui::drawer::{DrawerAction, DrawerTab, SideDrawer};
use crate::ui::hud::{HudAction, HudMode, PlayerHud};
use crate::ui::keymap_sheet::KeymapSheet;
use crate::ui::timeline::format_time;

thread_local! {
    static ACTIVE_CTX: RefCell<Option<Rc<WindowContext>>> = const { RefCell::new(None) };
}

#[derive(Debug, Clone, Default)]
pub struct LaunchOptions {
    pub files: Vec<String>,
    pub minimal_hud: bool,
    pub fullscreen: bool,
    pub start_seconds: Option<f64>,
    pub sub_file: Option<String>,
}

pub struct WindowContext {
    pub app: libadwaita::Application,
    pub window: libadwaita::ApplicationWindow,
    pub gl_area: gtk4::GLArea,
    pub player: Rc<MpvPlayer>,
    pub thumbnailer: ThumbnailGenerator,
    pub hud: PlayerHud,
    pub drawer: SideDrawer,
    pub welcome: WelcomeOverlay,
    pub url_dialog: UrlDialog,
    pub command_bar: CommandBar,
    pub keymap_sheet: KeymapSheet,
    pub toast: OsdToast,
    pub thumb_popup: ThumbnailPreviewPopup,
    pub mpris: MprisServer,
    pub css_provider: gtk4::CssProvider,
    pub _theme_watcher: RefCell<Option<ThemeWatcher>>,
    pub current_path: RefCell<String>,
    pub chapters: RefCell<Vec<ChapterItem>>,
    pub tracks: RefCell<Vec<TrackItem>>,
    pub paused: Cell<bool>,
    pub muted: Cell<bool>,
    pub volume: Cell<f64>,
    pub speed: Cell<f64>,
    pub position: Cell<f64>,
    pub duration: Cell<f64>,
    pub core_idle: Cell<bool>,
    pub has_media: Cell<bool>,
    pub inhibit_cookie: Cell<u32>,
    pub last_activity: Cell<Instant>,
    pub last_pointer: Cell<(f64, f64)>,
    pub seeking_active: Cell<bool>,
    pub last_seek_sent: Cell<Instant>,
    pub pending_seek_delta: Cell<f64>,
    pub pending_start_sec: Cell<Option<f64>>,
    pub pending_sub_file: RefCell<Option<String>>,
}

impl WindowContext {
    pub fn record_user_activity(self: &Rc<Self>) {
        self.last_activity.set(Instant::now());
        self.hud.set_revealed(true);
        self.window.set_cursor(None);
    }

    pub fn hide_hud_immediately(&self) {
        self.hud.set_revealed(false);
        self.thumb_popup.hide();
        let blank_cursor = gdk4::Cursor::from_name("none", None);
        self.window.set_cursor(blank_cursor.as_ref());
    }

    pub fn check_auto_hide(&self) {
        let can_hide = self.has_media.get()
            && !self.paused.get()
            && !self.drawer.is_open()
            && !self.url_dialog.is_open()
            && !self.command_bar.is_open()
            && !self.keymap_sheet.is_visible();

        if can_hide {
            if self.last_activity.get().elapsed() >= Duration::from_millis(1200) {
                self.hide_hud_immediately();
            }
        } else {
            self.hud.set_revealed(true);
            self.window.set_cursor(None);
        }
    }

    pub fn update_idle_inhibitor(&self) {
        let should_inhibit = self.has_media.get() && !self.paused.get() && !self.core_idle.get();
        let current_cookie = self.inhibit_cookie.get();

        if should_inhibit && current_cookie == 0 {
            let cookie = self.app.inhibit(
                Some(&self.window),
                gtk4::ApplicationInhibitFlags::IDLE,
                Some("Playing video in Omaplay"),
            );
            self.inhibit_cookie.set(cookie);
        } else if !should_inhibit && current_cookie != 0 {
            self.app.uninhibit(current_cookie);
            self.inhibit_cookie.set(0);
        }
    }

    pub fn open_media(self: &Rc<Self>, path_or_url: &str, append: bool) {
        let trimmed = path_or_url.trim();
        if trimmed.is_empty() {
            return;
        }
        if is_subtitle_file(trimmed) {
            self.player.add_external_subtitle(trimmed);
            self.toast.show(&format!("Loaded subtitle: {}", short_name(trimmed)));
            return;
        }

        let local_path = Path::new(trimmed);
        if local_path.is_dir() {
            let episodes = collect_media_in_dir(local_path, 3);
            if episodes.is_empty() {
                self.toast
                    .show(&format!("No media files found in {}", short_name(trimmed)));
                return;
            }
            self.has_media.set(true);
            self.welcome.set_visible(false);
            if append {
                for ep in &episodes {
                    self.player.playlist_append_silent(ep);
                }
                self.toast.show(&format!(
                    "Queued {} episodes from {}",
                    episodes.len(),
                    short_name(trimmed)
                ));
            } else {
                self.player.load_playlist_with_active(&episodes, 0);
                self.paused.set(false);
                self.hud.set_paused(false);
                self.player.set_pause(false);
                self.toast.show(&format!(
                    "Playing {} · {} episodes",
                    short_name(trimmed),
                    episodes.len()
                ));
            }
            self.record_user_activity();
            return;
        }

        self.has_media.set(true);
        self.welcome.set_visible(false);

        if !append && local_path.is_file() {
            let (siblings, active_idx) = collect_sibling_episodes(local_path);
            self.player.load_playlist_with_active(&siblings, active_idx);
            self.paused.set(false);
            self.hud.set_paused(false);
            self.player.set_pause(false);
            if siblings.len() > 1 {
                self.toast.show(&format!(
                    "Episode {} / {} · {}",
                    active_idx + 1,
                    siblings.len(),
                    short_name(trimmed)
                ));
            }
            self.record_user_activity();
            return;
        }

        self.player.load_file(trimmed, append);
        if append {
            self.toast
                .show(&format!("Queued: {}", short_name(trimmed)));
        } else {
            self.paused.set(false);
            self.hud.set_paused(false);
            self.player.set_pause(false);
        }
        self.record_user_activity();
    }

    pub fn handle_player_event(self: &Rc<Self>, ev: PlayerEvent) {
        match ev {
            PlayerEvent::FileLoaded => {
                self.has_media.set(true);
                self.core_idle.set(false);
                self.welcome.set_visible(false);
                let paused = self.player.is_paused();
                self.paused.set(paused);
                self.hud.set_paused(paused);
                if let Some(start_s) = self.pending_start_sec.take() {
                    if start_s > 0.0 {
                        self.player.seek_absolute(start_s);
                    }
                }
                if let Some(sub_p) = self.pending_sub_file.borrow_mut().take() {
                    self.player.add_external_subtitle(&sub_p);
                }
                self.mpris.set_playback_status(paused, false);
                self.update_idle_inhibitor();
                self.record_user_activity();
            }
            PlayerEvent::EndFile { reason, error } => {
                if let Some(err) = error {
                    self.toast.show(&format!("Playback error ({reason}): {err}"));
                }
            }
            PlayerEvent::PositionChanged(pos) => {
                if self.pending_seek_delta.get().abs() < 0.01 {
                    self.position.set(pos);
                    self.hud.set_position(pos);
                }
                self.mpris.set_position(pos);
            }
            PlayerEvent::DurationChanged(dur) => {
                self.duration.set(dur);
                self.hud.set_duration(dur);
                self.mpris.set_metadata(None, None, Some(dur));
            }
            PlayerEvent::PauseChanged(paused) => {
                self.paused.set(paused);
                self.hud.set_paused(paused);
                self.mpris
                    .set_playback_status(paused, !self.has_media.get());
                self.update_idle_inhibitor();
                if paused {
                    self.hud.set_revealed(true);
                    self.window.set_cursor(None);
                } else {
                    self.record_user_activity();
                }
            }
            PlayerEvent::VolumeChanged(vol) => {
                self.volume.set(vol);
                self.hud.set_volume(vol, self.muted.get());
                self.mpris.set_volume(vol);
            }
            PlayerEvent::MuteChanged(muted) => {
                self.muted.set(muted);
                self.hud.set_volume(self.volume.get(), muted);
            }
            PlayerEvent::SpeedChanged(speed) => {
                self.speed.set(speed);
                self.hud.set_speed(speed);
                self.mpris.set_rate(speed);
            }
            PlayerEvent::MediaTitleChanged(title) => {
                self.hud.set_title(&title);
                self.window.set_title(Some(&format!("{title} — Omaplay")));
                self.mpris.set_metadata(Some(&title), None, None);
            }
            PlayerEvent::PathChanged(path) => {
                *self.current_path.borrow_mut() = path.clone();
                self.mpris.set_metadata(None, Some(&path), None);
            }
            PlayerEvent::VideoDimensionsChanged(w, h) => {
                let hw = self.player.hwdec_current();
                self.hud.set_video_dimensions(w, h, hw);
            }
            PlayerEvent::TracksChanged(tracks) => {
                let hw = self.player.hwdec_current();
                self.hud.set_tracks(&tracks, hw);
                self.drawer.update_tracks(&tracks);
                *self.tracks.borrow_mut() = tracks;
            }
            PlayerEvent::ChaptersChanged(chapters) => {
                self.drawer.update_chapters(&chapters, self.position.get());
                self.hud.set_chapters(chapters.clone());
                *self.chapters.borrow_mut() = chapters;
            }
            PlayerEvent::PlaylistChanged(entries) => {
                let non_empty = !entries.is_empty();
                self.has_media.set(non_empty);
                self.welcome.set_visible(!non_empty);
                self.drawer.update_playlist(&entries);
            }
            PlayerEvent::DemuxerCacheChanged(cache_t) => {
                self.hud.timeline.set_cache_time(cache_t);
            }
            PlayerEvent::LoopPointsChanged { a, b } => {
                self.hud.set_loop_points(a, b);
                match (a, b) {
                    (Some(av), None) => {
                        self.toast
                            .show(&format!("A-B Loop: A = {}", format_time(av)));
                    }
                    (Some(av), Some(bv)) => {
                        self.toast.show(&format!(
                            "A-B Loop: {} → {}",
                            format_time(av),
                            format_time(bv)
                        ));
                    }
                    (None, None) => {
                        self.toast.show("A-B Loop cleared");
                    }
                    _ => {}
                }
            }
            PlayerEvent::SeekingChanged(seeking) => {
                self.seeking_active.set(seeking);
                if !seeking {
                    let pending = self.pending_seek_delta.replace(0.0);
                    if pending.abs() > 0.05 {
                        self.last_seek_sent.set(Instant::now());
                        self.player.seek_relative(pending);
                    } else {
                        self.mpris.emit_seeked(self.position.get());
                    }
                }
            }
            PlayerEvent::CoreIdleChanged(idle) => {
                self.core_idle.set(idle);
                self.mpris
                    .set_playback_status(self.paused.get(), !self.has_media.get() && idle);
                self.update_idle_inhibitor();
            }
            PlayerEvent::HwdecChanged(hw) => {
                self.hud.set_hwdec(hw);
            }
        }
    }

    pub fn dispatch_action(self: &Rc<Self>, action: PlayerAction) {
        match action {
            PlayerAction::TogglePause => {
                self.player.toggle_pause();
            }
            PlayerAction::SeekRelative(delta) => {
                let dur = self.duration.get();
                let cur = self.position.get();
                let optimistic = if dur > 0.0 {
                    (cur + delta).clamp(0.0, dur)
                } else {
                    (cur + delta).max(0.0)
                };
                self.position.set(optimistic);
                self.hud.set_position(optimistic);

                if !self.seeking_active.get()
                    || self.last_seek_sent.get().elapsed() >= Duration::from_millis(55)
                {
                    let total = self.pending_seek_delta.replace(0.0) + delta;
                    self.last_seek_sent.set(Instant::now());
                    self.seeking_active.set(true);
                    self.player.seek_relative(total);
                } else {
                    self.pending_seek_delta
                        .set(self.pending_seek_delta.get() + delta);
                }
                let sign = if delta >= 0.0 { "+" } else { "" };
                self.toast
                    .show(&format!("{} ({sign}{delta:.0}s)", format_time(optimistic)));
            }
            PlayerAction::FrameStep(forward) => {
                self.player.frame_step(forward);
                self.toast
                    .show(if forward { "Frame +1" } else { "Frame -1" });
            }
            PlayerAction::PrevChapter => {
                if !self.chapters.borrow().is_empty() {
                    self.player.chapter_step(-1);
                    self.toast.show("Previous Chapter");
                } else {
                    self.player.playlist_prev();
                }
            }
            PlayerAction::NextChapter => {
                if !self.chapters.borrow().is_empty() {
                    self.player.chapter_step(1);
                    self.toast.show("Next Chapter");
                } else {
                    self.player.playlist_next();
                }
            }
            PlayerAction::AddVolume(delta) => {
                self.player.add_volume(delta);
                let next_v = (self.volume.get() + delta).clamp(0.0, 150.0);
                self.toast.show(&format!("Volume {:.0}%", next_v));
            }
            PlayerAction::ToggleMute => {
                self.player.toggle_mute();
                self.toast
                    .show(if !self.muted.get() { "Muted" } else { "Unmuted" });
            }
            PlayerAction::AddSpeed(delta) => {
                self.player.add_speed(delta);
                let next_s = (self.speed.get() + delta).clamp(0.1, 8.0);
                self.toast.show(&format!("Speed {next_s:.2}×"));
            }
            PlayerAction::MultiplySpeed(factor) => {
                self.player.multiply_speed(factor);
                let next_s = (self.speed.get() * factor).clamp(0.1, 8.0);
                self.toast.show(&format!("Speed {next_s:.2}×"));
            }
            PlayerAction::ResetSpeed => {
                self.player.set_speed(1.0);
                self.toast.show("Speed 1.00×");
            }
            PlayerAction::ToggleFullscreen => {
                let next_fs = !self.window.is_fullscreen();
                if next_fs {
                    self.window.fullscreen();
                    if self.has_media.get() && !self.paused.get() {
                        self.hide_hud_immediately();
                    }
                } else {
                    self.window.unfullscreen();
                    self.record_user_activity();
                }
                self.hud.set_fullscreen(next_fs);
            }
            PlayerAction::Escape => {
                if self.keymap_sheet.is_visible() {
                    self.keymap_sheet.set_visible(false);
                } else if self.url_dialog.is_open() {
                    self.url_dialog.close();
                } else if self.command_bar.is_open() {
                    self.command_bar.close();
                } else if self.drawer.is_open() {
                    self.drawer.close();
                } else if self.window.is_fullscreen() {
                    self.window.unfullscreen();
                    self.hud.set_fullscreen(false);
                }
            }
            PlayerAction::CycleAudio => {
                self.player.cycle_track("audio");
                self.toast.show("Cycled Audio Track");
            }
            PlayerAction::CycleSub => {
                self.player.cycle_track("sub");
                self.toast.show("Cycled Subtitle Track");
            }
            PlayerAction::ToggleSubVisibility => {
                self.player.toggle_subtitle_visibility();
                self.toast.show("Toggled Subtitles");
            }
            PlayerAction::AdjustSubDelay(delta) => {
                self.player.adjust_sub_delay(delta);
                let d = self.player.sub_delay();
                self.drawer.set_sub_delay(d);
                self.toast.show(&format!("Subtitle Delay: {d:+.1}s"));
            }
            PlayerAction::ToggleDrawerChapters => {
                self.drawer
                    .update_chapters(&self.chapters.borrow(), self.position.get());
                self.drawer.toggle_tab(DrawerTab::Chapters);
            }
            PlayerAction::ToggleDrawerPlaylist => {
                self.drawer.toggle_tab(DrawerTab::Playlist);
            }
            PlayerAction::ToggleDrawerTracks => {
                self.drawer.toggle_tab(DrawerTab::Tracks);
            }
            PlayerAction::ToggleHudMode => {
                let mode = self.hud.toggle_mode();
                if mode != HudMode::Zen {
                    self.record_user_activity();
                }
                self.toast.show(match mode {
                    HudMode::Cinema => "HUD: Cinema Edge-Hover",
                    HudMode::Minimal => "HUD: Minimal Vim Strip",
                    HudMode::Zen => "HUD: Zen Off",
                });
            }
            PlayerAction::Screenshot { include_subs } => match self.player.screenshot(include_subs)
            {
                Ok(path) => {
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("screenshot.png");
                    self.toast.show(&format!("Screenshot saved: {name}"));
                }
                Err(e) => {
                    self.toast.show(&format!("Screenshot failed: {e}"));
                }
            },
            PlayerAction::AbLoopCycle => {
                self.player.ab_loop_cycle();
            }
            PlayerAction::OpenFileDialog => {
                self.open_file_chooser(false);
            }
            PlayerAction::OpenFolderDialog => {
                self.open_folder_chooser(false);
            }
            PlayerAction::OpenUrlDialog => {
                self.url_dialog.open();
            }
            PlayerAction::PlaylistNext => {
                self.player.playlist_next();
                self.toast.show("Next Track");
            }
            PlayerAction::PlaylistPrev => {
                self.player.playlist_prev();
                self.toast.show("Previous Track");
            }
            PlayerAction::OpenCommandBar => {
                self.command_bar.open();
            }
            PlayerAction::ToggleKeymapSheet => {
                self.keymap_sheet.toggle();
            }
            PlayerAction::Quit => {
                self.window.close();
            }
        }
    }

    pub fn execute_vim_command(self: &Rc<Self>, raw: &str) {
        match parse_vim_command(raw) {
            Ok(VimCommand::SeekSeconds(sec)) => {
                self.player.seek_absolute(sec);
                self.toast.show(&format!("Seek → {}", format_time(sec)));
            }
            Ok(VimCommand::SeekRelative(delta)) => {
                self.player.seek_relative(delta);
            }
            Ok(VimCommand::SeekPercent(pct)) => {
                self.player.seek_percent(pct);
                self.toast.show(&format!("Seek → {pct:.0}%"));
            }
            Ok(VimCommand::Speed(spd)) => {
                self.player.set_speed(spd);
                self.toast.show(&format!("Speed {spd:.2}×"));
            }
            Ok(VimCommand::Volume(vol)) => {
                self.player.set_volume(vol);
                self.toast.show(&format!("Volume {vol:.0}%"));
            }
            Ok(VimCommand::SubDelay(delay)) => {
                self.player.set_sub_delay(delay);
                self.drawer.set_sub_delay(delay);
                self.toast.show(&format!("Subtitle delay: {delay:+.1}s"));
            }
            Ok(VimCommand::Open(target)) => {
                self.open_media(&target, false);
            }
            Ok(VimCommand::ThemeReload) => {
                let palette = ThemePalette::load();
                self.css_provider.load_from_string(&palette.to_css());
                self.hud.set_palette(palette.clone());
                self.toast.show(&format!("Theme reloaded: {}", palette.name));
            }
            Ok(VimCommand::Quit) => {
                self.window.close();
            }
            Err(err) => {
                self.toast.show(&err);
            }
        }
    }

    pub fn open_file_chooser(self: &Rc<Self>, subtitle_only: bool) {
        let dialog = gtk4::FileDialog::builder()
            .title(if subtitle_only {
                "Load Subtitle File"
            } else {
                "Open Media File"
            })
            .modal(true)
            .build();

        let ctx_clone = Rc::clone(self);
        dialog.open(
            Some(&self.window),
            gio::Cancellable::NONE,
            move |res| {
                if let Ok(file) = res {
                    if let Some(path) = file.path() {
                        if let Some(s) = path.to_str() {
                            if subtitle_only {
                                ctx_clone.player.add_external_subtitle(s);
                                ctx_clone
                                    .toast
                                    .show(&format!("Subtitle loaded: {}", short_name(s)));
                            } else {
                                ctx_clone.open_media(s, false);
                            }
                        }
                    }
                }
            },
        );
    }

    pub fn open_folder_chooser(self: &Rc<Self>, append: bool) {
        let dialog = gtk4::FileDialog::builder()
            .title("Open Season / Playlist Folder")
            .modal(true)
            .build();

        let ctx_clone = Rc::clone(self);
        dialog.select_folder(
            Some(&self.window),
            gio::Cancellable::NONE,
            move |res| {
                if let Ok(folder) = res {
                    if let Some(path) = folder.path() {
                        if let Some(s) = path.to_str() {
                            ctx_clone.open_media(s, append);
                        }
                    }
                }
            },
        );
    }
}

pub fn build_window(app: &libadwaita::Application, opts: LaunchOptions) -> Rc<WindowContext> {
    let (css_provider, initial_palette) = install_global_css();

    let window = libadwaita::ApplicationWindow::builder()
        .application(app)
        .title("Omaplay")
        .default_width(1280)
        .default_height(720)
        .decorated(false)
        .build();
    window.add_css_class("omaplay-window");

    let gl_area = gtk4::GLArea::builder()
        .has_depth_buffer(false)
        .has_stencil_buffer(false)
        .auto_render(false)
        .hexpand(true)
        .vexpand(true)
        .focusable(true)
        .build();

    let mpv = MpvPlayer::new().expect("Failed to initialize libmpv");
    let player = Rc::new(mpv);

    let ctx_slot: Rc<RefCell<Option<Rc<WindowContext>>>> = Rc::new(RefCell::new(None));

    let thumb_popup = ThumbnailPreviewPopup::new();
    let thumbnailer = ThumbnailGenerator::spawn(move |frame: ThumbnailFrame| {
        glib::MainContext::default().invoke(move || {
            ACTIVE_CTX.with(|slot| {
                if let Some(c) = slot.borrow().as_ref() {
                    if frame.path == *c.current_path.borrow() && frame.timestamp >= 0.0 {
                        c.thumb_popup
                            .set_texture(&frame.rgba, frame.width, frame.height);
                    }
                }
            });
        });
    });

    let initial_hud_mode = if opts.minimal_hud {
        HudMode::Minimal
    } else {
        HudMode::Cinema
    };

    let slot_hud_act = Rc::clone(&ctx_slot);
    let slot_hud_seek = Rc::clone(&ctx_slot);
    let slot_hud_hover = Rc::clone(&ctx_slot);

    let hud = PlayerHud::new(
        initial_hud_mode,
        initial_palette.clone(),
        move |act| {
            let Some(ctx) = slot_hud_act.borrow().clone() else {
                return;
            };
            match act {
                HudAction::TogglePause => ctx.dispatch_action(PlayerAction::TogglePause),
                HudAction::Prev => ctx.dispatch_action(PlayerAction::PrevChapter),
                HudAction::Next => ctx.dispatch_action(PlayerAction::NextChapter),
                HudAction::ToggleMute => ctx.dispatch_action(PlayerAction::ToggleMute),
                HudAction::SetVolume(v) => {
                    ctx.player.set_volume(v);
                    ctx.record_user_activity();
                }
                HudAction::CycleSpeed => {
                    let cur = ctx.speed.get();
                    let next = if (cur - 1.0).abs() < 0.05 {
                        1.25
                    } else if (cur - 1.25).abs() < 0.05 {
                        1.5
                    } else if (cur - 1.5).abs() < 0.05 {
                        2.0
                    } else {
                        1.0
                    };
                    ctx.player.set_speed(next);
                    ctx.toast.show(&format!("Speed {next:.2}×"));
                }
                HudAction::CycleAudio => ctx.dispatch_action(PlayerAction::CycleAudio),
                HudAction::CycleSub => ctx.dispatch_action(PlayerAction::CycleSub),
                HudAction::AbLoopCycle => ctx.dispatch_action(PlayerAction::AbLoopCycle),
                HudAction::Screenshot => {
                    ctx.dispatch_action(PlayerAction::Screenshot {
                        include_subs: false,
                    })
                }
                HudAction::ToggleFullscreen => {
                    ctx.dispatch_action(PlayerAction::ToggleFullscreen)
                }
                HudAction::TogglePlaylistDrawer => {
                    ctx.dispatch_action(PlayerAction::ToggleDrawerPlaylist)
                }
                HudAction::ToggleTracksDrawer => {
                    ctx.dispatch_action(PlayerAction::ToggleDrawerTracks)
                }
                HudAction::ToggleChaptersDrawer => {
                    ctx.dispatch_action(PlayerAction::ToggleDrawerChapters)
                }
                HudAction::OpenUrlDialog => ctx.dispatch_action(PlayerAction::OpenUrlDialog),
                HudAction::ToggleKeymapSheet => {
                    ctx.dispatch_action(PlayerAction::ToggleKeymapSheet)
                }
            }
        },
        move |target_sec| {
            let Some(ctx) = slot_hud_seek.borrow().clone() else {
                return;
            };
            ctx.player.seek_absolute(target_sec);
            ctx.mpris.emit_seeked(target_sec);
            ctx.record_user_activity();
        },
        move |hover_info| {
            let Some(ctx) = slot_hud_hover.borrow().clone() else {
                return;
            };
            match hover_info {
                Some((x, w, ts, ch)) => {
                    let win_w = ctx.window.width().max(640);
                    ctx.thumb_popup
                        .update_hover(win_w, x, w, ts, ch.as_deref());
                    let path = ctx.current_path.borrow().clone();
                    ctx.thumbnailer.request(path, ts);
                }
                None => {
                    ctx.thumb_popup.hide();
                }
            }
        },
    );

    let slot_drawer = Rc::clone(&ctx_slot);
    let drawer = SideDrawer::new(move |d_act| {
        let Some(ctx) = slot_drawer.borrow().clone() else {
            return;
        };
        ctx.record_user_activity();
        match d_act {
            DrawerAction::PlayPlaylistIndex(idx) => ctx.player.playlist_play_index(idx),
            DrawerAction::RemovePlaylistIndex(idx) => ctx.player.playlist_remove(idx),
            DrawerAction::ClearPlaylist => ctx.player.playlist_clear(),
            DrawerAction::AddFile => ctx.open_file_chooser(false),
            DrawerAction::AddFolder => ctx.open_folder_chooser(false),
            DrawerAction::OpenUrl => ctx.url_dialog.open(),
            DrawerAction::SelectAudioTrack(id) => ctx.player.select_track("audio", id),
            DrawerAction::SelectSubTrack(id) => ctx.player.select_track("sub", id),
            DrawerAction::LoadExternalSub => ctx.open_file_chooser(true),
            DrawerAction::AdjustSubDelay(d) => {
                ctx.dispatch_action(PlayerAction::AdjustSubDelay(d))
            }
            DrawerAction::ResetSubDelay => {
                ctx.player.set_sub_delay(0.0);
                ctx.drawer.set_sub_delay(0.0);
                ctx.toast.show("Subtitle delay reset");
            }
            DrawerAction::AdjustSubScale(d) => {
                ctx.player.adjust_sub_scale(d);
                ctx.toast.show("Adjusted subtitle scale");
            }
            DrawerAction::SelectChapter(idx) => ctx.player.set_chapter(idx),
        }
    });

    let slot_welcome_open = Rc::clone(&ctx_slot);
    let slot_welcome_folder = Rc::clone(&ctx_slot);
    let slot_welcome_url = Rc::clone(&ctx_slot);
    let slot_welcome_keys = Rc::clone(&ctx_slot);
    let welcome = WelcomeOverlay::new(
        move || {
            if let Some(ctx) = slot_welcome_open.borrow().clone() {
                ctx.open_file_chooser(false);
            }
        },
        move || {
            if let Some(ctx) = slot_welcome_folder.borrow().clone() {
                ctx.open_folder_chooser(false);
            }
        },
        move || {
            if let Some(ctx) = slot_welcome_url.borrow().clone() {
                ctx.url_dialog.open();
            }
        },
        move || {
            if let Some(ctx) = slot_welcome_keys.borrow().clone() {
                ctx.keymap_sheet.toggle();
            }
        },
    );

    let slot_url = Rc::clone(&ctx_slot);
    let url_dialog = UrlDialog::new(move |url, append| {
        if let Some(ctx) = slot_url.borrow().clone() {
            ctx.open_media(&url, append);
        }
    });

    let slot_cmd = Rc::clone(&ctx_slot);
    let command_bar = CommandBar::new(move |cmd_str| {
        if let Some(ctx) = slot_cmd.borrow().clone() {
            ctx.execute_vim_command(&cmd_str);
        }
    });

    let slot_keys_close = Rc::clone(&ctx_slot);
    let keymap_sheet = KeymapSheet::new(move || {
        if let Some(ctx) = slot_keys_close.borrow().clone() {
            ctx.keymap_sheet.set_visible(false);
        }
    });

    let toast = OsdToast::new();

    let slot_mpris = Rc::clone(&ctx_slot);
    let mpris = MprisServer::start(move |mcmd| {
        let Some(ctx) = slot_mpris.borrow().clone() else {
            return;
        };
        match mcmd {
            MprisCommand::Raise => ctx.window.present(),
            MprisCommand::Quit => ctx.window.close(),
            MprisCommand::Next => ctx.player.playlist_next(),
            MprisCommand::Previous => ctx.player.playlist_prev(),
            MprisCommand::Pause => ctx.player.set_pause(true),
            MprisCommand::PlayPause => ctx.player.toggle_pause(),
            MprisCommand::Stop => ctx.player.stop(),
            MprisCommand::Play => ctx.player.set_pause(false),
            MprisCommand::SeekRelativeUs(offset_us) => {
                let sec = (offset_us as f64) / 1_000_000.0;
                ctx.player.seek_relative(sec);
                let target = (ctx.position.get() + sec).max(0.0);
                ctx.mpris.emit_seeked(target);
            }
            MprisCommand::SetPositionUs(pos_us) => {
                let sec = (pos_us as f64) / 1_000_000.0;
                ctx.player.seek_absolute(sec);
                ctx.mpris.emit_seeked(sec);
            }
            MprisCommand::OpenUri(uri) => {
                if uri == "omaplay://screenshot" {
                    ctx.dispatch_action(PlayerAction::Screenshot {
                        include_subs: false,
                    });
                } else {
                    let clean = uri.strip_prefix("file://").unwrap_or(&uri);
                    ctx.open_media(clean, false);
                }
            }
            MprisCommand::SetVolume(vol) => ctx.player.set_volume(vol),
            MprisCommand::SetRate(rate) => ctx.player.set_speed(rate),
        }
    });

    let overlay = gtk4::Overlay::new();
    overlay.set_child(Some(&gl_area));
    overlay.add_overlay(&welcome.revealer);
    overlay.add_overlay(&hud.top_revealer);
    overlay.add_overlay(&hud.bottom_revealer);
    overlay.add_overlay(&hud.minimal_revealer);
    overlay.add_overlay(&thumb_popup.revealer);
    overlay.add_overlay(&drawer.revealer);
    overlay.add_overlay(&url_dialog.revealer);
    overlay.add_overlay(&keymap_sheet.revealer);
    overlay.add_overlay(&command_bar.revealer);
    overlay.add_overlay(&toast.revealer);
    window.set_content(Some(&overlay));

    let ctx = Rc::new(WindowContext {
        app: app.clone(),
        window: window.clone(),
        gl_area: gl_area.clone(),
        player: Rc::clone(&player),
        thumbnailer,
        hud,
        drawer,
        welcome,
        url_dialog,
        command_bar,
        keymap_sheet,
        toast,
        thumb_popup,
        mpris,
        css_provider: css_provider.clone(),
        _theme_watcher: RefCell::new(None),
        current_path: RefCell::new(String::new()),
        chapters: RefCell::new(Vec::new()),
        tracks: RefCell::new(Vec::new()),
        paused: Cell::new(false),
        muted: Cell::new(false),
        volume: Cell::new(100.0),
        speed: Cell::new(1.0),
        position: Cell::new(0.0),
        duration: Cell::new(0.0),
        core_idle: Cell::new(true),
        has_media: Cell::new(false),
        inhibit_cookie: Cell::new(0),
        last_activity: Cell::new(Instant::now()),
        last_pointer: Cell::new((-1.0, -1.0)),
        seeking_active: Cell::new(false),
        last_seek_sent: Cell::new(Instant::now()),
        pending_seek_delta: Cell::new(0.0),
        pending_start_sec: Cell::new(opts.start_seconds),
        pending_sub_file: RefCell::new(opts.sub_file.clone()),
    });

    *ctx_slot.borrow_mut() = Some(Rc::clone(&ctx));

    // Wire ThemeWatcher for live Omarchy theme changes
    let ctx_theme = Rc::downgrade(&ctx);
    let watcher = ThemeWatcher::start(css_provider, move |new_palette| {
        if let Some(c) = ctx_theme.upgrade() {
            c.hud.set_palette(new_palette.clone());
            c.toast.show(&format!("Theme: {}", new_palette.name));
        }
    });
    *ctx._theme_watcher.borrow_mut() = Some(watcher);
    ACTIVE_CTX.with(|slot| {
        *slot.borrow_mut() = Some(Rc::clone(&ctx));
    });

    // Wire mpv event wakeup via glib::MainContext::default().invoke
    unsafe {
        let player_mut = Rc::as_ptr(&ctx.player) as *mut MpvPlayer;
        (*player_mut).set_wakeup_notifier(move || {
            glib::MainContext::default().invoke(|| {
                ACTIVE_CTX.with(|slot| {
                    let maybe_ctx = slot.borrow().clone();
                    if let Some(c) = maybe_ctx {
                        for ev in c.player.drain_events() {
                            c.handle_player_event(ev);
                        }
                    }
                });
            });
        });
    }
    for ev in ctx.player.drain_events() {
        ctx.handle_player_event(ev);
    }

    // Wire GLArea lifecycle
    let ctx_realize = Rc::clone(&ctx);
    let initial_files = Rc::new(RefCell::new(opts.files.clone()));
    gl_area.connect_realize(move |area| {
        area.make_current();
        if let Some(err) = area.error() {
            eprintln!("[omaplay::gl] GLArea error on realize: {err}");
        } else {
            match ctx_realize.player.init_gl_with_notifier(move || {
                glib::MainContext::default().invoke(|| {
                    ACTIVE_CTX.with(|slot| {
                        let maybe_ctx = slot.borrow().clone();
                        if let Some(c) = maybe_ctx {
                            if c.player.poll_render_update() {
                                c.gl_area.queue_render();
                            }
                        }
                    });
                });
            }) {
                Ok(()) => eprintln!("[omaplay::gl] mpv_render_context initialized"),
                Err(e) => eprintln!("[omaplay::gl] init_gl failed: {e}"),
            }
        }
        let files: Vec<String> = initial_files.borrow_mut().drain(..).collect();
        for (idx, file_arg) in files.iter().enumerate() {
            ctx_realize.open_media(file_arg, idx > 0);
        }
    });

    let ctx_render = Rc::clone(&ctx);
    gl_area.connect_render(move |area, _gl_ctx| {
        let mut fbo: i32 = 0;
        unsafe {
            ffi::glGetIntegerv(ffi::GL_DRAW_FRAMEBUFFER_BINDING, &mut fbo);
        }
        let scale = area.scale_factor().max(1);
        let width = area.width() * scale;
        let height = area.height() * scale;
        ctx_render.player.render_gl(fbo, width, height);
        glib::Propagation::Stop
    });

    let ctx_unrealize = Rc::clone(&ctx);
    gl_area.connect_unrealize(move |area| {
        area.make_current();
        ctx_unrealize.player.destroy_gl();
    });

    // Video canvas click (single -> pause, double -> fullscreen) & scroll (volume)
    let click_gesture = gtk4::GestureClick::new();
    let ctx_click = Rc::clone(&ctx);
    click_gesture.connect_pressed(move |_gesture, n_press, _x, _y| {
        ctx_click.record_user_activity();
        if n_press == 2 {
            ctx_click.dispatch_action(PlayerAction::ToggleFullscreen);
        } else if n_press == 1 && ctx_click.has_media.get() {
            ctx_click.player.toggle_pause();
        }
    });
    gl_area.add_controller(click_gesture);

    let scroll_ctrl = gtk4::EventControllerScroll::new(gtk4::EventControllerScrollFlags::VERTICAL);
    let ctx_scroll = Rc::clone(&ctx);
    scroll_ctrl.connect_scroll(move |_ctrl, _dx, dy| {
        if dy < 0.0 {
            ctx_scroll.dispatch_action(PlayerAction::AddVolume(5.0));
        } else if dy > 0.0 {
            ctx_scroll.dispatch_action(PlayerAction::AddVolume(-5.0));
        }
        glib::Propagation::Stop
    });
    gl_area.add_controller(scroll_ctrl);

    // Periodic auto-hide check (every 200ms)
    let ctx_autohide = Rc::downgrade(&ctx);
    glib::timeout_add_local(Duration::from_millis(200), move || {
        if let Some(c) = ctx_autohide.upgrade() {
            c.check_auto_hide();
            glib::ControlFlow::Continue
        } else {
            glib::ControlFlow::Break
        }
    });

    // Window pointer motion: only reveal HUD when hovering top/bottom edge zones (or paused)
    let win_motion = gtk4::EventControllerMotion::new();
    let ctx_motion = Rc::clone(&ctx);
    win_motion.connect_motion(move |_ctrl, x, y| {
        let (px, py) = ctx_motion.last_pointer.get();
        if px < 0.0 || (x - px).hypot(y - py) > 3.0 {
            ctx_motion.last_pointer.set((x, y));
            ctx_motion.last_activity.set(Instant::now());
            ctx_motion.window.set_cursor(None);

            let win_h = f64::from(ctx_motion.window.height()).max(300.0);
            let in_edge_zone = y <= 64.0 || y >= (win_h - 110.0);
            if in_edge_zone || ctx_motion.paused.get() || !ctx_motion.has_media.get() {
                ctx_motion.hud.set_revealed(true);
            } else if !ctx_motion.drawer.is_open()
                && !ctx_motion.url_dialog.is_open()
                && !ctx_motion.command_bar.is_open()
                && !ctx_motion.keymap_sheet.is_visible()
            {
                ctx_motion.hud.set_revealed(false);
                ctx_motion.thumb_popup.hide();
            }
        }
    });
    window.add_controller(win_motion);

    // Window-level keyboard controller
    let key_ctrl = gtk4::EventControllerKey::new();
    key_ctrl.set_propagation_phase(gtk4::PropagationPhase::Capture);
    let ctx_key = Rc::clone(&ctx);
    key_ctrl.connect_key_pressed(move |_ctrl, keyval, _code, state| {
        if ctx_key.url_dialog.is_open() || ctx_key.command_bar.is_open() {
            return glib::Propagation::Proceed;
        }
        if let Some(action) = map_key_event(keyval, state) {
            ctx_key.dispatch_action(action);
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    window.add_controller(key_ctrl);

    // Drag and Drop support (FileList and String URLs)
    let drop_files = gtk4::DropTarget::new(gdk4::FileList::static_type(), gdk4::DragAction::COPY);
    let ctx_drop_f = Rc::clone(&ctx);
    drop_files.connect_drop(move |_target, value, _x, _y| {
        if let Ok(file_list) = value.get::<gdk4::FileList>() {
            for (idx, file) in file_list.files().iter().enumerate() {
                if let Some(path) = file.path() {
                    if let Some(s) = path.to_str() {
                        ctx_drop_f.open_media(s, idx > 0);
                    }
                }
            }
            return true;
        }
        false
    });
    window.add_controller(drop_files);

    let drop_str = gtk4::DropTarget::new(String::static_type(), gdk4::DragAction::COPY);
    let ctx_drop_s = Rc::clone(&ctx);
    drop_str.connect_drop(move |_target, value, _x, _y| {
        if let Ok(s) = value.get::<String>() {
            for (idx, line) in s.lines().map(str::trim).filter(|l| !l.is_empty()).enumerate() {
                let clean = line.strip_prefix("file://").unwrap_or(line);
                ctx_drop_s.open_media(clean, idx > 0);
            }
            return true;
        }
        false
    });
    window.add_controller(drop_str);

    // Clean up inhibitor on window close
    let ctx_close = Rc::clone(&ctx);
    window.connect_close_request(move |_| {
        let cookie = ctx_close.inhibit_cookie.replace(0);
        if cookie != 0 {
            ctx_close.app.uninhibit(cookie);
        }
        glib::Propagation::Proceed
    });

    if opts.fullscreen {
        window.fullscreen();
    }


    window.present();
    ctx
}

fn is_subtitle_file(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".srt")
        || lower.ends_with(".ass")
        || lower.ends_with(".ssa")
        || lower.ends_with(".vtt")
        || lower.ends_with(".sub")
}

fn short_name(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
}
