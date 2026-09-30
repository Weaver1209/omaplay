use std::ffi::{CString, c_int, c_void};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::mpv::ffi;

pub const THUMB_WIDTH: u32 = 160;
pub const THUMB_HEIGHT: u32 = 90;

#[derive(Debug, Clone)]
pub struct ThumbnailFrame {
    pub path: String,
    pub timestamp: f64,
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
struct ThumbRequest {
    path: String,
    timestamp: f64,
}

struct SharedRequestQueue {
    latest: Mutex<Option<ThumbRequest>>,
    shutdown: AtomicBool,
    cvar: Condvar,
}

pub struct ThumbnailGenerator {
    queue: Arc<SharedRequestQueue>,
}

impl ThumbnailGenerator {
    pub fn spawn<F>(on_thumbnail: F) -> Self
    where
        F: Fn(ThumbnailFrame) + Send + Sync + 'static,
    {
        let queue = Arc::new(SharedRequestQueue {
            latest: Mutex::new(None),
            shutdown: AtomicBool::new(false),
            cvar: Condvar::new(),
        });

        let worker_queue = Arc::clone(&queue);
        let callback = Arc::new(on_thumbnail);

        thread::Builder::new()
            .name("omaplay-thumb".to_string())
            .spawn(move || {
                run_thumbnail_worker(worker_queue, callback);
            })
            .ok();

        Self { queue }
    }

    pub fn request(&self, path: String, timestamp: f64) {
        if path.is_empty() || is_remote_url(&path) || !timestamp.is_finite() || timestamp < 0.0 {
            return;
        }
        if let Ok(mut guard) = self.queue.latest.lock() {
            *guard = Some(ThumbRequest { path, timestamp });
            self.queue.cvar.notify_one();
        }
    }
}

impl Drop for ThumbnailGenerator {
    fn drop(&mut self) {
        self.queue.shutdown.store(true, Ordering::Release);
        self.queue.cvar.notify_all();
    }
}

pub fn is_remote_url(path: &str) -> bool {
    path.starts_with("http://")
        || path.starts_with("https://")
        || path.starts_with("ytdl://")
        || path.starts_with("rtsp://")
        || path.starts_with("rtmp://")
}

fn run_thumbnail_worker(
    queue: Arc<SharedRequestQueue>,
    callback: Arc<dyn Fn(ThumbnailFrame) + Send + Sync>,
) {
    unsafe {
        libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr());
    }

    let ctx = unsafe { ffi::mpv_create() };
    if ctx.is_null() {
        return;
    }

    let set_opt = |k: &str, v: &str| {
        if let (Ok(ck), Ok(cv)) = (CString::new(k), CString::new(v)) {
            unsafe {
                ffi::mpv_set_option_string(ctx, ck.as_ptr(), cv.as_ptr());
            }
        }
    };

    set_opt("profile", "fast");
    set_opt("sws-scaler", "fast-bilinear");
    set_opt("vo", "libmpv");
    set_opt("vid", "1");
    set_opt("aid", "no");
    set_opt("sid", "no");
    set_opt("audio", "no");
    set_opt("osc", "no");
    set_opt("osd-level", "0");
    set_opt("hr-seek", "no");
    set_opt("hwdec", "no");
    set_opt("keep-open", "yes");
    set_opt("pause", "yes");
    set_opt("vd-lavc-fast", "yes");
    set_opt("vd-lavc-skiploopfilter", "all");

    if unsafe { ffi::mpv_initialize(ctx) } < 0 {
        unsafe {
            ffi::mpv_terminate_destroy(ctx);
        }
        return;
    }

    let api_sw = c"sw";
    let mut create_params = [
        ffi::mpv_render_param {
            type_: ffi::MPV_RENDER_PARAM_API_TYPE,
            data: api_sw.as_ptr() as *mut c_void,
        },
        ffi::mpv_render_param {
            type_: ffi::MPV_RENDER_PARAM_INVALID,
            data: ptr::null_mut(),
        },
    ];

    let mut rctx: *mut ffi::mpv_render_context = ptr::null_mut();
    if unsafe { ffi::mpv_render_context_create(&mut rctx, ctx, create_params.as_mut_ptr()) } < 0
        || rctx.is_null()
    {
        unsafe {
            ffi::mpv_terminate_destroy(ctx);
        }
        return;
    }

    let mut loaded_path = String::new();
    let mut last_rendered_ts: f64 = -999.0;
    let stride: usize = (THUMB_WIDTH as usize) * 4;
    let mut pixel_buf = vec![0u8; stride * (THUMB_HEIGHT as usize)];
    loop {
        let req = {
            let mut guard = match queue.latest.lock() {
                Ok(g) => g,
                Err(_) => break,
            };
            while guard.is_none() && !queue.shutdown.load(Ordering::Acquire) {
                guard = match queue.cvar.wait(guard) {
                    Ok(g) => g,
                    Err(_) => return,
                };
            }
            if queue.shutdown.load(Ordering::Acquire) {
                break;
            }
            match guard.take() {
                Some(r) => r,
                None => continue,
            }
        };

        if req.path != loaded_path {
            if !mpv_cmd(ctx, &["loadfile", &req.path, "replace"]) {
                continue;
            }
            if !wait_for_event(ctx, ffi::MPV_EVENT_FILE_LOADED, Duration::from_millis(1800)) {
                continue;
            }
            loaded_path = req.path.clone();
            last_rendered_ts = -999.0;
        } else if (req.timestamp - last_rendered_ts).abs() < 1.0 {
            continue;
        }

        let ts_str = format!("{:.2}", req.timestamp.max(0.0));
        if !mpv_cmd(ctx, &["seek", &ts_str, "absolute+keyframes"]) {
            continue;
        }
        let _ = wait_for_event(ctx, ffi::MPV_EVENT_PLAYBACK_RESTART, Duration::from_millis(450));
        last_rendered_ts = req.timestamp;

        let mut sw_size: [c_int; 2] = [THUMB_WIDTH as c_int, THUMB_HEIGHT as c_int];
        let sw_fmt = c"rgb0";
        let mut sw_stride: usize = stride;

        let mut render_params = [
            ffi::mpv_render_param {
                type_: ffi::MPV_RENDER_PARAM_SW_SIZE,
                data: sw_size.as_mut_ptr() as *mut c_void,
            },
            ffi::mpv_render_param {
                type_: ffi::MPV_RENDER_PARAM_SW_FORMAT,
                data: sw_fmt.as_ptr() as *mut c_void,
            },
            ffi::mpv_render_param {
                type_: ffi::MPV_RENDER_PARAM_SW_STRIDE,
                data: &mut sw_stride as *mut _ as *mut c_void,
            },
            ffi::mpv_render_param {
                type_: ffi::MPV_RENDER_PARAM_SW_POINTER,
                data: pixel_buf.as_mut_ptr() as *mut c_void,
            },
            ffi::mpv_render_param {
                type_: ffi::MPV_RENDER_PARAM_INVALID,
                data: ptr::null_mut(),
            },
        ];

        let rc = unsafe { ffi::mpv_render_context_render(rctx, render_params.as_mut_ptr()) };
        if rc >= 0 {
            let mut rgba = pixel_buf.clone();
            for px in rgba.chunks_exact_mut(4) {
                px[3] = 255;
            }
            callback(ThumbnailFrame {
                path: req.path,
                timestamp: req.timestamp,
                rgba,
                width: THUMB_WIDTH,
                height: THUMB_HEIGHT,
            });
        }
    }

    unsafe {
        ffi::mpv_render_context_free(rctx);
        ffi::mpv_terminate_destroy(ctx);
    }
}

fn mpv_cmd(ctx: *mut ffi::mpv_handle, args: &[&str]) -> bool {
    let c_args: Vec<CString> = args
        .iter()
        .filter_map(|s| CString::new(*s).ok())
        .collect();
    let mut ptrs: Vec<*const i8> = c_args.iter().map(|s| s.as_ptr()).collect();
    ptrs.push(ptr::null());
    unsafe { ffi::mpv_command(ctx, ptrs.as_ptr()) >= 0 }
}

fn wait_for_event(ctx: *mut ffi::mpv_handle, target_event: c_int, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let rem = deadline
            .saturating_duration_since(Instant::now())
            .as_secs_f64()
            .min(0.05);
        let ev_ptr = unsafe { ffi::mpv_wait_event(ctx, rem) };
        if ev_ptr.is_null() {
            break;
        }
        let ev = unsafe { *ev_ptr };
        if ev.event_id == target_event {
            return true;
        }
        if ev.event_id == ffi::MPV_EVENT_END_FILE || ev.event_id == ffi::MPV_EVENT_SHUTDOWN {
            return false;
        }
    }
    false
}
