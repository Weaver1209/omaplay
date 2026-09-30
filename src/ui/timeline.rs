use std::cell::RefCell;
use std::f64::consts::PI;
use std::rc::Rc;

use gtk4::prelude::*;

use crate::mpv::ChapterItem;
use crate::theme::{ThemePalette, parse_hex_rgba_f64};

#[derive(Debug, Clone)]
struct TimelineState {
    position: f64,
    duration: f64,
    cache_time: f64,
    chapters: Vec<ChapterItem>,
    loop_a: Option<f64>,
    loop_b: Option<f64>,
    hover_x: Option<f64>,
    dragging: bool,
    drag_start_x: f64,
    palette: ThemePalette,
}

#[derive(Clone)]
pub struct TimelineWidget {
    pub area: gtk4::DrawingArea,
    state: Rc<RefCell<TimelineState>>,
}

impl TimelineWidget {
    pub fn new<FSeek, FHover>(
        palette: ThemePalette,
        on_seek: FSeek,
        on_hover: FHover,
    ) -> Self
    where
        FSeek: Fn(f64) + 'static,
        FHover: Fn(Option<(f64, f64, f64, Option<String>)>) + 'static,
    {
        let area = gtk4::DrawingArea::builder()
            .content_height(28)
            .hexpand(true)
            .valign(gtk4::Align::Center)
            .build();

        let state = Rc::new(RefCell::new(TimelineState {
            position: 0.0,
            duration: 0.0,
            cache_time: 0.0,
            chapters: Vec::new(),
            loop_a: None,
            loop_b: None,
            hover_x: None,
            dragging: false,
            drag_start_x: 0.0,
            palette,
        }));

        let state_draw = Rc::clone(&state);
        area.set_draw_func(move |_area, cr, width, height| {
            let st = state_draw.borrow();
            draw_timeline(cr, f64::from(width), f64::from(height), &st);
        });

        let on_seek_rc = Rc::new(on_seek);
        let on_hover_rc = Rc::new(on_hover);

        let motion = gtk4::EventControllerMotion::new();
        let state_motion = Rc::clone(&state);
        let area_motion = area.clone();
        let hover_cb = Rc::clone(&on_hover_rc);
        motion.connect_motion(move |_ctrl, x, _y| {
            let width = f64::from(area_motion.width()).max(1.0);
            let clamped_x = x.clamp(0.0, width);
            let (dur, ch_title) = {
                let mut st = state_motion.borrow_mut();
                st.hover_x = Some(clamped_x);
                let dur = st.duration;
                let ts = if dur > 0.0 {
                    (clamped_x / width) * dur
                } else {
                    0.0
                };
                let ch = chapter_at(&st.chapters, ts);
                (dur, ch)
            };
            area_motion.queue_draw();
            if dur > 0.0 {
                let ts = (clamped_x / width) * dur;
                hover_cb(Some((clamped_x, width, ts, ch_title)));
            } else {
                hover_cb(None);
            }
        });

        let state_leave = Rc::clone(&state);
        let area_leave = area.clone();
        let hover_leave_cb = Rc::clone(&on_hover_rc);
        motion.connect_leave(move |_ctrl| {
            state_leave.borrow_mut().hover_x = None;
            area_leave.queue_draw();
            hover_leave_cb(None);
        });
        area.add_controller(motion);

        let click = gtk4::GestureClick::new();
        let state_click = Rc::clone(&state);
        let area_click = area.clone();
        let seek_click_cb = Rc::clone(&on_seek_rc);
        click.connect_pressed(move |_gesture, _n_press, x, _y| {
            let width = f64::from(area_click.width()).max(1.0);
            let dur = state_click.borrow().duration;
            if dur > 0.0 {
                let target = ((x / width).clamp(0.0, 1.0)) * dur;
                state_click.borrow_mut().position = target;
                area_click.queue_draw();
                seek_click_cb(target);
            }
        });
        area.add_controller(click);

        let drag = gtk4::GestureDrag::new();
        let state_drag_begin = Rc::clone(&state);
        drag.connect_drag_begin(move |_gesture, start_x, _start_y| {
            let mut st = state_drag_begin.borrow_mut();
            st.dragging = true;
            st.drag_start_x = start_x;
        });

        let state_drag_update = Rc::clone(&state);
        let area_drag = area.clone();
        let seek_drag_cb = Rc::clone(&on_seek_rc);
        drag.connect_drag_update(move |_gesture, offset_x, _offset_y| {
            let width = f64::from(area_drag.width()).max(1.0);
            let target_opt = {
                let mut st = state_drag_update.borrow_mut();
                if st.duration > 0.0 {
                    let cur_x = (st.drag_start_x + offset_x).clamp(0.0, width);
                    st.hover_x = Some(cur_x);
                    let t = (cur_x / width) * st.duration;
                    st.position = t;
                    Some(t)
                } else {
                    None
                }
            };
            area_drag.queue_draw();
            if let Some(t) = target_opt {
                seek_drag_cb(t);
            }
        });

        let state_drag_end = Rc::clone(&state);
        let area_drag_end = area.clone();
        drag.connect_drag_end(move |_gesture, _offset_x, _offset_y| {
            state_drag_end.borrow_mut().dragging = false;
            area_drag_end.queue_draw();
        });
        area.add_controller(drag);

        Self { area, state }
    }

    pub fn set_position(&self, pos: f64) {
        let mut st = self.state.borrow_mut();
        if !st.dragging {
            let next = pos.max(0.0);
            let dur = st.duration;
            let prev = st.position;
            st.position = next;
            drop(st);
            let w = f64::from(self.area.width()).max(400.0);
            if dur <= 0.0 || ((next - prev).abs() / dur) * w >= 0.35 {
                self.area.queue_draw();
            }
        }
    }

    pub fn set_duration(&self, dur: f64) {
        self.state.borrow_mut().duration = dur.max(0.0);
        self.area.queue_draw();
    }

    pub fn set_cache_time(&self, cache_time: f64) {
        let mut st = self.state.borrow_mut();
        let next = cache_time.max(0.0);
        if (next - st.cache_time).abs() >= 1.0 {
            st.cache_time = next;
            drop(st);
            self.area.queue_draw();
        }
    }

    pub fn set_chapters(&self, chapters: Vec<ChapterItem>) {
        self.state.borrow_mut().chapters = chapters;
        self.area.queue_draw();
    }

    pub fn set_loop_points(&self, a: Option<f64>, b: Option<f64>) {
        let mut st = self.state.borrow_mut();
        st.loop_a = a;
        st.loop_b = b;
        drop(st);
        self.area.queue_draw();
    }

    pub fn set_palette(&self, palette: ThemePalette) {
        self.state.borrow_mut().palette = palette;
        self.area.queue_draw();
    }

    pub fn current_chapter_title(&self) -> Option<String> {
        let st = self.state.borrow();
        chapter_at(&st.chapters, st.position)
    }
}

fn chapter_at(chapters: &[ChapterItem], timestamp: f64) -> Option<String> {
    if chapters.is_empty() {
        return None;
    }
    let mut active: Option<&ChapterItem> = None;
    for ch in chapters {
        if ch.time <= timestamp + 0.05 {
            active = Some(ch);
        } else {
            break;
        }
    }
    active.and_then(|c| c.title.clone())
}

fn draw_timeline(cr: &gtk4::cairo::Context, width: f64, height: f64, st: &TimelineState) {
    let pad_x = 8.0;
    let track_w = (width - pad_x * 2.0).max(4.0);
    let hovered = st.hover_x.is_some() || st.dragging;
    let bar_h = if hovered { 5.0 } else { 3.0 };
    let bar_y = (height - bar_h) * 0.5;
    let radius = if st.palette.corner_radius > 0 {
        bar_h * 0.5
    } else {
        0.0
    };

    let (sr, sg, sb, sa) = parse_hex_rgba_f64(&st.palette.selection, 0.45);
    cr.set_source_rgba(sr, sg, sb, sa);
    rounded_rect(cr, pad_x, bar_y, track_w, bar_h, radius);
    let _ = cr.fill();

    let dur = st.duration;
    if dur > 0.0 {
        let cache_end = st.cache_time.max(st.position).clamp(0.0, dur);
        let cache_w = (cache_end / dur) * track_w;
        if cache_w > 1.0 {
            let (fr, fg, fb, fa) = parse_hex_rgba_f64(&st.palette.foreground, 0.25);
            cr.set_source_rgba(fr, fg, fb, fa);
            rounded_rect(cr, pad_x, bar_y, cache_w, bar_h, radius);
            let _ = cr.fill();
        }

        if let Some(a) = st.loop_a {
            let ax = pad_x + (a / dur).clamp(0.0, 1.0) * track_w;
            let (yr, yg, yb, _) = parse_hex_rgba_f64(&st.palette.yellow, 1.0);
            if let Some(b) = st.loop_b {
                let bx = pad_x + (b / dur).clamp(0.0, 1.0) * track_w;
                let left = ax.min(bx);
                let span = (bx - ax).abs().max(2.0);
                cr.set_source_rgba(yr, yg, yb, 0.30);
                cr.rectangle(left, bar_y - 2.0, span, bar_h + 4.0);
                let _ = cr.fill();

                cr.set_source_rgba(yr, yg, yb, 0.95);
                cr.rectangle(bx - 1.0, bar_y - 4.0, 2.0, bar_h + 8.0);
                let _ = cr.fill();
            }
            cr.set_source_rgba(yr, yg, yb, 0.95);
            cr.rectangle(ax - 1.0, bar_y - 4.0, 2.0, bar_h + 8.0);
            let _ = cr.fill();
        }

        let prog_ratio = (st.position / dur).clamp(0.0, 1.0);
        let prog_w = prog_ratio * track_w;
        if prog_w > 0.5 {
            let (ar, ag, ab, aa) = parse_hex_rgba_f64(&st.palette.accent, 1.0);
            cr.set_source_rgba(ar, ag, ab, aa);
            rounded_rect(cr, pad_x, bar_y, prog_w.max(bar_h), bar_h, radius);
            let _ = cr.fill();
        }

        let (cr_r, cr_g, cr_b, _) = parse_hex_rgba_f64(&st.palette.darker_background, 0.9);
        for ch in &st.chapters {
            if ch.time > 0.1 && ch.time < dur {
                let cx = pad_x + (ch.time / dur) * track_w;
                cr.set_source_rgba(cr_r, cr_g, cr_b, 0.88);
                cr.rectangle(cx - 1.0, bar_y, 2.0, bar_h);
                let _ = cr.fill();
            }
        }

        if let Some(hx) = st.hover_x {
            let clamped_hx = hx.clamp(pad_x, pad_x + track_w);
            let (fr, fg, fb, _) = parse_hex_rgba_f64(&st.palette.foreground, 0.55);
            cr.set_source_rgba(fr, fg, fb, 0.55);
            cr.rectangle(clamped_hx - 0.5, bar_y - 2.0, 1.0, bar_h + 4.0);
            let _ = cr.fill();
        }

        let thumb_x = pad_x + prog_w;
        let thumb_y = height * 0.5;
        let (ar, ag, ab, _) = parse_hex_rgba_f64(&st.palette.accent, 1.0);
        let (fr, fg, fb, _) = parse_hex_rgba_f64(&st.palette.foreground, 1.0);

        if st.palette.corner_radius == 0 {
            let sz = if hovered { 9.0 } else { 7.0 };
            cr.rectangle(thumb_x - sz * 0.5, thumb_y - sz * 0.5, sz, sz);
            if hovered {
                cr.set_source_rgba(fr, fg, fb, 1.0);
            } else {
                cr.set_source_rgba(ar, ag, ab, 1.0);
            }
            let _ = cr.fill();
        } else {
            let thumb_r = if hovered { 5.5 } else { 4.0 };
            cr.arc(thumb_x, thumb_y, thumb_r, 0.0, 2.0 * PI);
            if hovered {
                cr.set_source_rgba(fr, fg, fb, 1.0);
            } else {
                cr.set_source_rgba(ar, ag, ab, 1.0);
            }
            let _ = cr.fill();
        }
    }
}

fn rounded_rect(cr: &gtk4::cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let radius = r.min(w * 0.5).min(h * 0.5).max(0.0);
    cr.new_sub_path();
    cr.arc(x + w - radius, y + radius, radius, -PI * 0.5, 0.0);
    cr.arc(x + w - radius, y + h - radius, radius, 0.0, PI * 0.5);
    cr.arc(x + radius, y + h - radius, radius, PI * 0.5, PI);
    cr.arc(x + radius, y + radius, radius, PI, PI * 1.5);
    cr.close_path();
}

pub fn format_time(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "00:00".to_string();
    }
    let total_secs = seconds.round() as u64;
    let hrs = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    if hrs > 0 {
        format!("{hrs:02}:{mins:02}:{secs:02}")
    } else {
        format!("{mins:02}:{secs:02}")
    }
}
