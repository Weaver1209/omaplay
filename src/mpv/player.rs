use std::cell::{Cell, RefCell};
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::PathBuf;
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

use crate::mpv::ffi;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackItem {
    #[serde(default)]
    pub id: i64,
    #[serde(rename = "type", default)]
    pub track_type: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub codec: Option<String>,
    #[serde(default)]
    pub selected: bool,
    #[serde(default)]
    pub external: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChapterItem {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub time: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaylistEntry {
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub current: bool,
    #[serde(default)]
    pub playing: bool,
}

#[derive(Debug, Clone)]
pub enum PlayerEvent {
    FileLoaded,
    EndFile { reason: i32, error: Option<String> },
    PositionChanged(f64),
    DurationChanged(f64),
    PauseChanged(bool),
    VolumeChanged(f64),
    MuteChanged(bool),
    SpeedChanged(f64),
    MediaTitleChanged(String),
    PathChanged(String),
    VideoDimensionsChanged(i64, i64),
    TracksChanged(Vec<TrackItem>),
    ChaptersChanged(Vec<ChapterItem>),
    PlaylistChanged(Vec<PlaylistEntry>),
    DemuxerCacheChanged(f64),
    LoopPointsChanged { a: Option<f64>, b: Option<f64> },
    SeekingChanged(bool),
    CoreIdleChanged(bool),
    HwdecChanged(Option<String>),
}

struct WakeupState {
    pending: AtomicBool,
    notify: Box<dyn Fn() + Send + Sync>,
}

unsafe extern "C" fn wakeup_trampoline(ctx: *mut c_void) {
    if ctx.is_null() {
        return;
    }
    let state = unsafe { &*(ctx as *const WakeupState) };
    if !state.pending.swap(true, Ordering::AcqRel) {
        (state.notify)();
    }
}

pub struct MpvPlayer {
    ctx: *mut ffi::mpv_handle,
    render_ctx: Cell<*mut ffi::mpv_render_context>,
    wakeup_state: Option<Arc<WakeupState>>,
    render_wakeup_state: RefCell<Option<Arc<WakeupState>>>,
    video_w: Cell<i64>,
    video_h: Cell<i64>,
    loop_a: Cell<Option<f64>>,
    loop_b: Cell<Option<f64>>,
    screenshot_dir: PathBuf,
}

impl MpvPlayer {
    pub fn new() -> Result<Self, String> {
        unsafe {
            libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr());
        }

        let ctx = unsafe { ffi::mpv_create() };
        if ctx.is_null() {
            return Err("mpv_create() returned NULL".to_string());
        }

        let screenshot_dir = glib::user_special_dir(glib::UserDirectory::Pictures)
            .unwrap_or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("/tmp"))
                    .join("Pictures")
            })
            .join("omaplay");
        let _ = std::fs::create_dir_all(&screenshot_dir);

        let player = Self {
            ctx,
            render_ctx: Cell::new(ptr::null_mut()),
            wakeup_state: None,
            render_wakeup_state: RefCell::new(None),
            video_w: Cell::new(0),
            video_h: Cell::new(0),
            loop_a: Cell::new(None),
            loop_b: Cell::new(None),
            screenshot_dir: screenshot_dir.clone(),
        };

        let _ = player.set_option_str("profile", "fast");
        player.set_option_str("vo", "libmpv")?;
        player.set_option_str("hwdec", "auto")?;
        let _ = player.set_option_str("hwdec-codecs", "all");
        let _ = player.set_option_str("vd-lavc-dr", "yes");
        let _ = player.set_option_str("video-timing-offset", "0");
        let _ = player.set_option_str("demuxer-max-bytes", "150MiB");
        let _ = player.set_option_str("demuxer-max-back-bytes", "75MiB");
        player.set_option_str("keep-open", "yes")?;
        player.set_option_str("ytdl", "yes")?;
        player.set_option_str("osc", "no")?;
        player.set_option_str("osd-level", "0")?;
        player.set_option_str("sub-auto", "fuzzy")?;
        let _ = player.set_option_str("sub-file-paths", "Subs:Subtitles:subs:subtitles");
        player.set_option_str("audio-display", "no")?;
        player.set_option_str("screenshot-format", "png")?;
        player.set_option_str("screenshot-template", "%F-%P")?;
        if let Some(dir_str) = screenshot_dir.to_str() {
            let _ = player.set_option_str("screenshot-directory", dir_str);
        }

        let rc = unsafe { ffi::mpv_initialize(ctx) };
        if rc < 0 {
            return Err(format!("mpv_initialize failed: {}", mpv_err_str(rc)));
        }

        player.observe_properties();
        Ok(player)
    }

    fn observe_properties(&self) {
        self.observe(1, "time-pos", ffi::MPV_FORMAT_DOUBLE);
        self.observe(2, "duration", ffi::MPV_FORMAT_DOUBLE);
        self.observe(3, "pause", ffi::MPV_FORMAT_FLAG);
        self.observe(4, "volume", ffi::MPV_FORMAT_DOUBLE);
        self.observe(5, "mute", ffi::MPV_FORMAT_FLAG);
        self.observe(6, "speed", ffi::MPV_FORMAT_DOUBLE);
        self.observe(7, "media-title", ffi::MPV_FORMAT_STRING);
        self.observe(8, "path", ffi::MPV_FORMAT_STRING);
        self.observe(9, "track-list", ffi::MPV_FORMAT_STRING);
        self.observe(10, "chapter-list", ffi::MPV_FORMAT_STRING);
        self.observe(11, "playlist", ffi::MPV_FORMAT_STRING);
        self.observe(12, "demuxer-cache-time", ffi::MPV_FORMAT_DOUBLE);
        self.observe(13, "ab-loop-a", ffi::MPV_FORMAT_STRING);
        self.observe(14, "ab-loop-b", ffi::MPV_FORMAT_STRING);
        self.observe(15, "seeking", ffi::MPV_FORMAT_FLAG);
        self.observe(16, "core-idle", ffi::MPV_FORMAT_FLAG);
        self.observe(17, "video-params/w", ffi::MPV_FORMAT_INT64);
        self.observe(18, "video-params/h", ffi::MPV_FORMAT_INT64);
        self.observe(19, "hwdec-current", ffi::MPV_FORMAT_STRING);
    }

    fn observe(&self, id: u64, name: &str, format: c_int) {
        if let Ok(c_name) = CString::new(name) {
            unsafe {
                ffi::mpv_observe_property(self.ctx, id, c_name.as_ptr(), format);
            }
        }
    }

    pub fn set_wakeup_notifier<F>(&mut self, on_wakeup: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        let state = Arc::new(WakeupState {
            pending: AtomicBool::new(false),
            notify: Box::new(on_wakeup),
        });
        let raw_ptr = Arc::as_ptr(&state) as *mut c_void;
        self.wakeup_state = Some(state);
        unsafe {
            ffi::mpv_set_wakeup_callback(self.ctx, wakeup_trampoline, raw_ptr);
        }
    }

    pub fn drain_events(&self) -> Vec<PlayerEvent> {
        if let Some(state) = &self.wakeup_state {
            state.pending.store(false, Ordering::Release);
        }

        let mut out = Vec::new();
        loop {
            let ev_ptr = unsafe { ffi::mpv_wait_event(self.ctx, 0.0) };
            if ev_ptr.is_null() {
                break;
            }
            let ev = unsafe { *ev_ptr };
            if ev.event_id == ffi::MPV_EVENT_NONE {
                break;
            }
            match ev.event_id {
                ffi::MPV_EVENT_FILE_LOADED => {
                    out.push(PlayerEvent::FileLoaded);
                }
                ffi::MPV_EVENT_END_FILE => {
                    if !ev.data.is_null() {
                        let ef = unsafe { *(ev.data as *const ffi::mpv_event_end_file) };
                        let error = if ef.error < 0 {
                            Some(mpv_err_str(ef.error))
                        } else {
                            None
                        };
                        out.push(PlayerEvent::EndFile {
                            reason: ef.reason,
                            error,
                        });
                    }
                }
                ffi::MPV_EVENT_PROPERTY_CHANGE => {
                    if !ev.data.is_null() {
                        let prop = unsafe { *(ev.data as *const ffi::mpv_event_property) };
                        if let Some(mapped) = self.map_property_change(ev.reply_userdata, prop) {
                            out.push(mapped);
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }

    fn map_property_change(
        &self,
        userdata: u64,
        prop: ffi::mpv_event_property,
    ) -> Option<PlayerEvent> {
        match userdata {
            1 => {
                if prop.format == ffi::MPV_FORMAT_DOUBLE && !prop.data.is_null() {
                    let val = unsafe { *(prop.data as *const f64) };
                    Some(PlayerEvent::PositionChanged(val.max(0.0)))
                } else {
                    None
                }
            }
            2 => {
                if prop.format == ffi::MPV_FORMAT_DOUBLE && !prop.data.is_null() {
                    let val = unsafe { *(prop.data as *const f64) };
                    Some(PlayerEvent::DurationChanged(val.max(0.0)))
                } else {
                    None
                }
            }
            3 => {
                if prop.format == ffi::MPV_FORMAT_FLAG && !prop.data.is_null() {
                    let val = unsafe { *(prop.data as *const c_int) } != 0;
                    Some(PlayerEvent::PauseChanged(val))
                } else {
                    None
                }
            }
            4 => {
                if prop.format == ffi::MPV_FORMAT_DOUBLE && !prop.data.is_null() {
                    let val = unsafe { *(prop.data as *const f64) };
                    Some(PlayerEvent::VolumeChanged(val.clamp(0.0, 150.0)))
                } else {
                    None
                }
            }
            5 => {
                if prop.format == ffi::MPV_FORMAT_FLAG && !prop.data.is_null() {
                    let val = unsafe { *(prop.data as *const c_int) } != 0;
                    Some(PlayerEvent::MuteChanged(val))
                } else {
                    None
                }
            }
            6 => {
                if prop.format == ffi::MPV_FORMAT_DOUBLE && !prop.data.is_null() {
                    let val = unsafe { *(prop.data as *const f64) };
                    Some(PlayerEvent::SpeedChanged(val))
                } else {
                    None
                }
            }
            7 => {
                let s = read_prop_string(prop)?;
                Some(PlayerEvent::MediaTitleChanged(s))
            }
            8 => {
                let s = read_prop_string(prop)?;
                Some(PlayerEvent::PathChanged(s))
            }
            9 => {
                let s = read_prop_string(prop)?;
                let tracks: Vec<TrackItem> = serde_json::from_str(&s).unwrap_or_default();
                Some(PlayerEvent::TracksChanged(tracks))
            }
            10 => {
                let s = read_prop_string(prop)?;
                let chapters: Vec<ChapterItem> = serde_json::from_str(&s).unwrap_or_default();
                Some(PlayerEvent::ChaptersChanged(chapters))
            }
            11 => {
                let s = read_prop_string(prop)?;
                let playlist: Vec<PlaylistEntry> = serde_json::from_str(&s).unwrap_or_default();
                Some(PlayerEvent::PlaylistChanged(playlist))
            }
            12 => {
                if prop.format == ffi::MPV_FORMAT_DOUBLE && !prop.data.is_null() {
                    let val = unsafe { *(prop.data as *const f64) };
                    Some(PlayerEvent::DemuxerCacheChanged(val.max(0.0)))
                } else {
                    None
                }
            }
            13 => {
                let val = read_prop_string(prop).and_then(|s| s.parse::<f64>().ok());
                self.loop_a.set(val);
                Some(PlayerEvent::LoopPointsChanged {
                    a: self.loop_a.get(),
                    b: self.loop_b.get(),
                })
            }
            14 => {
                let val = read_prop_string(prop).and_then(|s| s.parse::<f64>().ok());
                self.loop_b.set(val);
                Some(PlayerEvent::LoopPointsChanged {
                    a: self.loop_a.get(),
                    b: self.loop_b.get(),
                })
            }
            15 => {
                if prop.format == ffi::MPV_FORMAT_FLAG && !prop.data.is_null() {
                    let val = unsafe { *(prop.data as *const c_int) } != 0;
                    Some(PlayerEvent::SeekingChanged(val))
                } else {
                    None
                }
            }
            16 => {
                if prop.format == ffi::MPV_FORMAT_FLAG && !prop.data.is_null() {
                    let val = unsafe { *(prop.data as *const c_int) } != 0;
                    Some(PlayerEvent::CoreIdleChanged(val))
                } else {
                    None
                }
            }
            17 => {
                if prop.format == ffi::MPV_FORMAT_INT64 && !prop.data.is_null() {
                    let w = unsafe { *(prop.data as *const i64) };
                    self.video_w.set(w);
                    Some(PlayerEvent::VideoDimensionsChanged(w, self.video_h.get()))
                } else {
                    None
                }
            }
            18 => {
                if prop.format == ffi::MPV_FORMAT_INT64 && !prop.data.is_null() {
                    let h = unsafe { *(prop.data as *const i64) };
                    self.video_h.set(h);
                    Some(PlayerEvent::VideoDimensionsChanged(self.video_w.get(), h))
                } else {
                    None
                }
            }
            19 => {
                let hw = read_prop_string(prop)
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty() && s != "no");
                Some(PlayerEvent::HwdecChanged(hw))
            }
            _ => None,
        }
    }

    pub fn init_gl(
        &self,
        render_wakeup_cb: unsafe extern "C" fn(*mut c_void),
        cb_ctx: *mut c_void,
    ) -> Result<(), String> {
        if !self.render_ctx.get().is_null() {
            return Ok(());
        }

        let mut init_params = ffi::mpv_opengl_init_params {
            get_proc_address: Some(ffi::get_proc_address),
            get_proc_address_ctx: ptr::null_mut(),
        };

        let wl_display = unsafe {
            let dpy = ffi::gdk_display_get_default();
            if !dpy.is_null() {
                ffi::gdk_wayland_display_get_wl_display(dpy)
            } else {
                ptr::null_mut()
            }
        };

        let api_type = c"opengl";
        let mut params = [
            ffi::mpv_render_param {
                type_: ffi::MPV_RENDER_PARAM_API_TYPE,
                data: api_type.as_ptr() as *mut c_void,
            },
            ffi::mpv_render_param {
                type_: ffi::MPV_RENDER_PARAM_OPENGL_INIT_PARAMS,
                data: &mut init_params as *mut _ as *mut c_void,
            },
            ffi::mpv_render_param {
                type_: if !wl_display.is_null() {
                    ffi::MPV_RENDER_PARAM_WL_DISPLAY
                } else {
                    ffi::MPV_RENDER_PARAM_INVALID
                },
                data: wl_display,
            },
            ffi::mpv_render_param {
                type_: ffi::MPV_RENDER_PARAM_INVALID,
                data: ptr::null_mut(),
            },
        ];

        let mut rctx: *mut ffi::mpv_render_context = ptr::null_mut();
        let rc = unsafe {
            ffi::mpv_render_context_create(&mut rctx, self.ctx, params.as_mut_ptr())
        };
        if rc < 0 || rctx.is_null() {
            return Err(format!(
                "mpv_render_context_create failed: {}",
                mpv_err_str(rc)
            ));
        }

        unsafe {
            ffi::mpv_render_context_set_update_callback(rctx, render_wakeup_cb, cb_ctx);
        }
        self.render_ctx.set(rctx);
        Ok(())
    }

    pub fn init_gl_with_notifier<F>(&self, on_render_update: F) -> Result<(), String>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let state = Arc::new(WakeupState {
            pending: AtomicBool::new(false),
            notify: Box::new(on_render_update),
        });
        let raw_ptr = Arc::as_ptr(&state) as *mut c_void;
        *self.render_wakeup_state.borrow_mut() = Some(state);
        self.init_gl(wakeup_trampoline, raw_ptr)
    }

    pub fn poll_render_update(&self) -> bool {
        if let Some(state) = self.render_wakeup_state.borrow().as_ref() {
            state.pending.store(false, Ordering::Release);
        }
        let rctx = self.render_ctx.get();
        if rctx.is_null() {
            return false;
        }
        let flags = unsafe { ffi::mpv_render_context_update(rctx) };
        (flags & ffi::MPV_RENDER_UPDATE_FRAME) != 0
    }

    pub fn render_gl(&self, fbo: i32, width: i32, height: i32) {
        let rctx = self.render_ctx.get();
        if rctx.is_null() {
            return;
        }

        let mut fbo_param = ffi::mpv_opengl_fbo {
            fbo,
            w: width.max(1),
            h: height.max(1),
            internal_format: 0,
        };
        let mut flip_y: c_int = 1;
        let mut block_target: c_int = 0;

        let mut params = [
            ffi::mpv_render_param {
                type_: ffi::MPV_RENDER_PARAM_OPENGL_FBO,
                data: &mut fbo_param as *mut _ as *mut c_void,
            },
            ffi::mpv_render_param {
                type_: ffi::MPV_RENDER_PARAM_FLIP_Y,
                data: &mut flip_y as *mut _ as *mut c_void,
            },
            ffi::mpv_render_param {
                type_: ffi::MPV_RENDER_PARAM_BLOCK_FOR_TARGET_TIME,
                data: &mut block_target as *mut _ as *mut c_void,
            },
            ffi::mpv_render_param {
                type_: ffi::MPV_RENDER_PARAM_INVALID,
                data: ptr::null_mut(),
            },
        ];

        unsafe {
            ffi::mpv_render_context_render(rctx, params.as_mut_ptr());
            ffi::mpv_render_context_report_swap(rctx);
        }
    }

    pub fn destroy_gl(&self) {
        let rctx = self.render_ctx.replace(ptr::null_mut());
        if !rctx.is_null() {
            unsafe {
                ffi::mpv_render_context_free(rctx);
            }
        }
        *self.render_wakeup_state.borrow_mut() = None;
    }

    pub fn set_option_str(&self, name: &str, value: &str) -> Result<(), String> {
        let c_name = CString::new(name).map_err(|e| e.to_string())?;
        let c_val = CString::new(value).map_err(|e| e.to_string())?;
        let rc =
            unsafe { ffi::mpv_set_option_string(self.ctx, c_name.as_ptr(), c_val.as_ptr()) };
        if rc < 0 {
            Err(format!("mpv_set_option_string({name}) failed: {}", mpv_err_str(rc)))
        } else {
            Ok(())
        }
    }

    pub fn command(&self, args: &[&str]) -> Result<(), String> {
        let c_strings: Vec<CString> = args
            .iter()
            .filter_map(|s| CString::new(*s).ok())
            .collect();
        let mut ptrs: Vec<*const c_char> = c_strings.iter().map(|s| s.as_ptr()).collect();
        ptrs.push(ptr::null());
        let rc = unsafe { ffi::mpv_command(self.ctx, ptrs.as_ptr()) };
        if rc < 0 {
            Err(format!("mpv_command({args:?}) failed: {}", mpv_err_str(rc)))
        } else {
            Ok(())
        }
    }

    pub fn command_async(&self, args: &[&str]) {
        let c_strings: Vec<CString> = args
            .iter()
            .filter_map(|s| CString::new(*s).ok())
            .collect();
        let mut ptrs: Vec<*const c_char> = c_strings.iter().map(|s| s.as_ptr()).collect();
        ptrs.push(ptr::null());
        unsafe {
            ffi::mpv_command_async(self.ctx, 0, ptrs.as_ptr());
        }
    }

    pub fn set_property_str(&self, name: &str, value: &str) -> Result<(), String> {
        let c_name = CString::new(name).map_err(|e| e.to_string())?;
        let c_val = CString::new(value).map_err(|e| e.to_string())?;
        let rc = unsafe {
            ffi::mpv_set_property_string(self.ctx, c_name.as_ptr(), c_val.as_ptr())
        };
        if rc < 0 {
            Err(format!(
                "mpv_set_property_string({name}={value}) failed: {}",
                mpv_err_str(rc)
            ))
        } else {
            Ok(())
        }
    }

    pub fn get_property_str(&self, name: &str) -> Option<String> {
        let c_name = CString::new(name).ok()?;
        let ptr = unsafe { ffi::mpv_get_property_string(self.ctx, c_name.as_ptr()) };
        if ptr.is_null() {
            return None;
        }
        let s = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        unsafe {
            ffi::mpv_free(ptr as *mut c_void);
        }
        Some(s)
    }

    pub fn load_file(&self, uri_or_path: &str, append: bool) {
        let mode = if append { "append-play" } else { "replace" };
        let _ = self.command(&["loadfile", uri_or_path, mode]);
    }

    pub fn playlist_append_silent(&self, uri_or_path: &str) {
        let _ = self.command(&["loadfile", uri_or_path, "append"]);
    }

    pub fn load_playlist_with_active(&self, files: &[String], active_idx: usize) {
        if files.is_empty() {
            return;
        }
        let target_idx = active_idx.min(files.len() - 1);
        let _ = self.command(&["loadfile", &files[target_idx], "replace"]);
        for (i, item) in files.iter().enumerate().take(target_idx) {
            let _ = self.command(&["loadfile", item, "append"]);
            let from = (i + 1).to_string();
            let to = i.to_string();
            let _ = self.command(&["playlist-move", &from, &to]);
        }
        for item in files.iter().skip(target_idx + 1) {
            let _ = self.command(&["loadfile", item, "append"]);
        }
    }

    pub fn is_paused(&self) -> bool {
        self.get_property_str("pause").as_deref() == Some("yes")
    }

    pub fn toggle_pause(&self) {
        if self.get_property_str("eof-reached").as_deref() == Some("yes") {
            self.command_async(&["seek", "0", "absolute"]);
            let _ = self.set_property_str("pause", "no");
            return;
        }
        self.command_async(&["cycle", "pause"]);
    }

    pub fn set_pause(&self, paused: bool) {
        if !paused && self.get_property_str("eof-reached").as_deref() == Some("yes") {
            self.command_async(&["seek", "0", "absolute"]);
        }
        let _ = self.set_property_str("pause", if paused { "yes" } else { "no" });
    }

    pub fn seek_relative(&self, seconds: f64) {
        let s = format!("{seconds:.3}");
        let mode = if seconds.abs() <= 1.5 {
            "relative+exact"
        } else {
            "relative+keyframes"
        };
        self.command_async(&["seek", &s, mode]);
    }

    pub fn seek_absolute(&self, seconds: f64) {
        let s = format!("{:.3}", seconds.max(0.0));
        self.command_async(&["seek", &s, "absolute+exact"]);
    }

    pub fn seek_percent(&self, percent: f64) {
        let s = format!("{:.2}", percent.clamp(0.0, 100.0));
        self.command_async(&["seek", &s, "absolute-percent"]);
    }

    pub fn frame_step(&self, forward: bool) {
        if forward {
            self.command_async(&["frame-step"]);
        } else {
            self.command_async(&["frame-back-step"]);
        }
    }


    pub fn add_volume(&self, delta: f64) {
        let cur = self
            .get_property_str("volume")
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(100.0);
        self.set_volume((cur + delta).clamp(0.0, 150.0));
    }

    pub fn set_volume(&self, vol: f64) {
        let s = format!("{:.1}", vol.clamp(0.0, 150.0));
        let _ = self.set_property_str("volume", &s);
    }

    pub fn toggle_mute(&self) {
        let _ = self.command(&["cycle", "mute"]);
    }

    pub fn set_speed(&self, speed: f64) {
        let s = format!("{:.2}", speed.clamp(0.1, 8.0));
        let _ = self.set_property_str("speed", &s);
    }

    pub fn multiply_speed(&self, factor: f64) {
        let cur = self
            .get_property_str("speed")
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(1.0);
        self.set_speed((cur * factor).clamp(0.1, 8.0));
    }

    pub fn add_speed(&self, delta: f64) {
        let cur = self
            .get_property_str("speed")
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(1.0);
        self.set_speed((cur + delta).clamp(0.1, 8.0));
    }

    pub fn cycle_track(&self, kind: &str) {
        let prop = match kind {
            "audio" | "aid" => "aid",
            "sub" | "sid" => "sid",
            _ => kind,
        };
        let _ = self.command(&["cycle", prop]);
    }

    pub fn toggle_subtitle_visibility(&self) {
        let _ = self.command(&["cycle", "sub-visibility"]);
    }

    pub fn select_track(&self, kind: &str, id: Option<i64>) {
        let prop = match kind {
            "audio" | "aid" => "aid",
            "sub" | "sid" => "sid",
            _ => kind,
        };
        match id {
            Some(track_id) => {
                let s = track_id.to_string();
                let _ = self.set_property_str(prop, &s);
            }
            None => {
                let _ = self.set_property_str(prop, "no");
            }
        }
    }

    pub fn add_external_subtitle(&self, path: &str) {
        let _ = self.command(&["sub-add", path, "select"]);
    }

    pub fn adjust_sub_delay(&self, delta_sec: f64) {
        let s = format!("{delta_sec:.2}");
        let _ = self.command(&["add", "sub-delay", &s]);
    }

    pub fn set_sub_delay(&self, delay_sec: f64) {
        let s = format!("{delay_sec:.2}");
        let _ = self.set_property_str("sub-delay", &s);
    }

    pub fn sub_delay(&self) -> f64 {
        self.get_property_str("sub-delay")
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0)
    }

    pub fn adjust_sub_scale(&self, delta: f64) {
        let s = format!("{delta:.2}");
        let _ = self.command(&["add", "sub-scale", &s]);
    }

    pub fn chapter_step(&self, delta: i32) {
        let s = delta.to_string();
        let _ = self.command(&["add", "chapter", &s]);
    }

    pub fn set_chapter(&self, index: usize) {
        let s = index.to_string();
        let _ = self.set_property_str("chapter", &s);
    }

    pub fn playlist_play_index(&self, idx: usize) {
        let s = idx.to_string();
        let _ = self.set_property_str("playlist-pos", &s);
    }

    pub fn playlist_next(&self) {
        let _ = self.command(&["playlist-next", "weak"]);
    }

    pub fn playlist_prev(&self) {
        let _ = self.command(&["playlist-prev", "weak"]);
    }

    pub fn playlist_remove(&self, idx: usize) {
        let s = idx.to_string();
        let _ = self.command(&["playlist-remove", &s]);
    }

    pub fn playlist_clear(&self) {
        let _ = self.command(&["playlist-clear"]);
    }

    pub fn ab_loop_cycle(&self) {
        let _ = self.command(&["ab-loop"]);
    }

    pub fn stop(&self) {
        let _ = self.command(&["stop"]);
    }

    pub fn hwdec_current(&self) -> Option<String> {
        self.get_property_str("hwdec-current")
            .filter(|s| !s.is_empty() && s != "no")
    }

    pub fn screenshot(&self, include_subs: bool) -> Result<PathBuf, String> {
        let _ = std::fs::create_dir_all(&self.screenshot_dir);
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let pos = self
            .get_property_str("time-pos")
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0);
        let filename = format!("omaplay-{:06.2}s-{}.png", pos.max(0.0), ts % 100_000);
        let target = self.screenshot_dir.join(filename);
        let target_str = target
            .to_str()
            .ok_or_else(|| "Invalid screenshot path".to_string())?;
        let mode = if include_subs { "subtitles" } else { "video" };
        self.command(&["screenshot-to-file", target_str, mode])?;
        Ok(target)
    }
}

impl Drop for MpvPlayer {
    fn drop(&mut self) {
        self.destroy_gl();
        if !self.ctx.is_null() {
            unsafe {
                ffi::mpv_terminate_destroy(self.ctx);
            }
            self.ctx = ptr::null_mut();
        }
    }
}

fn read_prop_string(prop: ffi::mpv_event_property) -> Option<String> {
    if prop.format != ffi::MPV_FORMAT_STRING || prop.data.is_null() {
        return None;
    }
    let c_str_ptr = unsafe { *(prop.data as *const *const c_char) };
    if c_str_ptr.is_null() {
        return None;
    }
    Some(
        unsafe { CStr::from_ptr(c_str_ptr) }
            .to_string_lossy()
            .into_owned(),
    )
}

fn mpv_err_str(err: c_int) -> String {
    let ptr = unsafe { ffi::mpv_error_string(err) };
    if ptr.is_null() {
        format!("mpv error {err}")
    } else {
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

pub fn is_media_file(path: &std::path::Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "mkv"
            | "mp4"
            | "webm"
            | "avi"
            | "mov"
            | "m4v"
            | "ts"
            | "m2ts"
            | "flv"
            | "wmv"
            | "mpg"
            | "mpeg"
            | "ogv"
            | "mp3"
            | "flac"
            | "m4a"
            | "opus"
            | "ogg"
            | "wav"
    )
}

pub fn collect_media_in_dir(dir: &std::path::Path, max_depth: usize) -> Vec<String> {
    let mut out = Vec::new();
    collect_media_rec(dir, 0, max_depth, &mut out);
    out.sort_by(|a, b| natural_cmp(a, b));
    out
}

fn collect_media_rec(
    dir: &std::path::Path,
    depth: usize,
    max_depth: usize,
    out: &mut Vec<String>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        if path.is_dir() {
            if depth < max_depth {
                subdirs.push(path);
            }
        } else if path.is_file() && is_media_file(&path) {
            if let Some(s) = path.to_str() {
                out.push(s.to_string());
            }
        }
    }
    subdirs.sort_by(|a, b| natural_cmp(&a.to_string_lossy(), &b.to_string_lossy()));
    for sub in subdirs {
        collect_media_rec(&sub, depth + 1, max_depth, out);
    }
}

pub fn collect_sibling_episodes(file_path: &std::path::Path) -> (Vec<String>, usize) {
    let canonical = file_path
        .canonicalize()
        .unwrap_or_else(|_| file_path.to_path_buf());
    let Some(parent) = canonical.parent() else {
        let s = canonical.to_string_lossy().into_owned();
        return (vec![s], 0);
    };
    let siblings = collect_media_in_dir(parent, 0);
    let target_str = canonical.to_string_lossy();
    if siblings.is_empty() {
        return (vec![target_str.into_owned()], 0);
    }
    let idx = siblings
        .iter()
        .position(|p| {
            std::path::Path::new(p)
                .canonicalize()
                .map(|cp| cp == canonical)
                .unwrap_or_else(|_| p == target_str.as_ref())
        })
        .unwrap_or(0);
    (siblings, idx)
}

pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let mut ia = a.chars().peekable();
    let mut ib = b.chars().peekable();

    loop {
        match (ia.peek(), ib.peek()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(&ca), Some(&cb)) => {
                if ca.is_ascii_digit() && cb.is_ascii_digit() {
                    let mut na: u64 = 0;
                    while let Some(&d) = ia.peek() {
                        if let Some(digit) = d.to_digit(10) {
                            na = na.saturating_mul(10).saturating_add(u64::from(digit));
                            ia.next();
                        } else {
                            break;
                        }
                    }
                    let mut nb: u64 = 0;
                    while let Some(&d) = ib.peek() {
                        if let Some(digit) = d.to_digit(10) {
                            nb = nb.saturating_mul(10).saturating_add(u64::from(digit));
                            ib.next();
                        } else {
                            break;
                        }
                    }
                    match na.cmp(&nb) {
                        std::cmp::Ordering::Equal => continue,
                        non_eq => return non_eq,
                    }
                } else {
                    let la = ca.to_ascii_lowercase();
                    let lb = cb.to_ascii_lowercase();
                    match la.cmp(&lb) {
                        std::cmp::Ordering::Equal => {
                            ia.next();
                            ib.next();
                        }
                        non_eq => return non_eq,
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_sort_orders_episodes_numerically() {
        let mut eps = vec![
            "The.Gentlemen.S02E10.mkv",
            "The.Gentlemen.S02E2.mkv",
            "The.Gentlemen.S02E01.mkv",
            "The.Gentlemen.S01E08.mkv",
        ];
        eps.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            eps,
            vec![
                "The.Gentlemen.S01E08.mkv",
                "The.Gentlemen.S02E01.mkv",
                "The.Gentlemen.S02E2.mkv",
                "The.Gentlemen.S02E10.mkv",
            ]
        );
    }
}
