#![allow(dead_code)]

use std::cell::{Cell, RefCell};
use std::f64::consts::PI;
use std::rc::Rc;

use gtk4::cairo::{Context, LineCap, LineJoin};
use gtk4::prelude::*;

use crate::theme::{ThemePalette, parse_hex_rgba_f64};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconGlyph {
    Play,
    Pause,
    SkipBack,
    SkipForward,
    VolumeHigh,
    VolumeLow,
    VolumeMute,
    Repeat,
    Camera,
    Maximize,
    Minimize,
    List,
    Sliders,
    Chapters,
    Globe,
    Keyboard,
    Close,
    Film,
    FolderOpen,
    Plus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconTone {
    Foreground,
    Accent,
    Muted,
    Yellow,
}

#[derive(Clone)]
pub struct FleaIcon {
    pub area: gtk4::DrawingArea,
    glyph: Rc<Cell<IconGlyph>>,
    tone: Rc<Cell<IconTone>>,
    palette: Rc<RefCell<ThemePalette>>,
}

impl FleaIcon {
    pub fn new(glyph: IconGlyph, size: i32, tone: IconTone, palette: ThemePalette) -> Self {
        let area = gtk4::DrawingArea::builder()
            .content_width(size)
            .content_height(size)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .build();

        let glyph_cell = Rc::new(Cell::new(glyph));
        let tone_cell = Rc::new(Cell::new(tone));
        let palette_cell = Rc::new(RefCell::new(palette));

        let g_draw = Rc::clone(&glyph_cell);
        let t_draw = Rc::clone(&tone_cell);
        let p_draw = Rc::clone(&palette_cell);

        area.set_draw_func(move |_area, cr, w, h| {
            let pal = p_draw.borrow();
            let hex = match t_draw.get() {
                IconTone::Foreground => &pal.foreground,
                IconTone::Accent => &pal.accent,
                IconTone::Muted => &pal.muted,
                IconTone::Yellow => &pal.yellow,
            };
            let (r, g, b, a) = parse_hex_rgba_f64(hex, 1.0);
            draw_glyph(
                cr,
                f64::from(w),
                f64::from(h),
                g_draw.get(),
                (r, g, b, a),
                pal.corner_radius > 0,
            );
        });

        Self {
            area,
            glyph: glyph_cell,
            tone: tone_cell,
            palette: palette_cell,
        }
    }

    pub fn set_glyph(&self, glyph: IconGlyph) {
        if self.glyph.replace(glyph) != glyph {
            self.area.queue_draw();
        }
    }

    pub fn set_tone(&self, tone: IconTone) {
        if self.tone.replace(tone) != tone {
            self.area.queue_draw();
        }
    }

    pub fn set_palette(&self, palette: ThemePalette) {
        *self.palette.borrow_mut() = palette;
        self.area.queue_draw();
    }
}

pub fn icon_button(
    glyph: IconGlyph,
    size: i32,
    tone: IconTone,
    palette: ThemePalette,
    css_class: &str,
) -> (gtk4::Button, FleaIcon) {
    let icon = FleaIcon::new(glyph, size, tone, palette);
    let btn = gtk4::Button::new();
    btn.add_css_class(css_class);
    btn.set_child(Some(&icon.area));
    (btn, icon)
}

pub fn icon_label_button(
    glyph: IconGlyph,
    label_text: &str,
    size: i32,
    tone: IconTone,
    palette: ThemePalette,
    css_class: &str,
) -> (gtk4::Button, FleaIcon, gtk4::Label) {
    let icon = FleaIcon::new(glyph, size, tone, palette);
    let label = gtk4::Label::new(Some(label_text));
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    row.set_halign(gtk4::Align::Center);
    row.set_valign(gtk4::Align::Center);
    row.append(&icon.area);
    row.append(&label);

    let btn = gtk4::Button::new();
    btn.add_css_class(css_class);
    btn.set_child(Some(&row));
    (btn, icon, label)
}

fn draw_glyph(
    cr: &Context,
    w: f64,
    h: f64,
    glyph: IconGlyph,
    rgba: (f64, f64, f64, f64),
    rounded: bool,
) {
    let size = w.min(h).max(1.0);
    let scale = size / 24.0;
    let offset_x = (w - size) * 0.5;
    let offset_y = (h - size) * 0.5;

    let _ = cr.save();
    cr.translate(offset_x, offset_y);
    cr.scale(scale, scale);

    cr.set_source_rgba(rgba.0, rgba.1, rgba.2, rgba.3);
    cr.set_line_width(2.0);
    if rounded {
        cr.set_line_cap(LineCap::Round);
        cr.set_line_join(LineJoin::Round);
    } else {
        cr.set_line_cap(LineCap::Square);
        cr.set_line_join(LineJoin::Miter);
    }

    match glyph {
        IconGlyph::Play => {
            // Flea's "play": "M6 4l14 8-14 8z"
            cr.move_to(6.0, 4.0);
            cr.line_to(20.0, 12.0);
            cr.line_to(6.0, 20.0);
            cr.close_path();
            let _ = cr.stroke();
        }
        IconGlyph::Pause => {
            // Flea's "pause": "M14 3h5v18h-5z M5 3h5v18H5z"
            cr.rectangle(5.0, 3.0, 5.0, 18.0);
            cr.rectangle(14.0, 3.0, 5.0, 18.0);
            let _ = cr.stroke();
        }
        IconGlyph::SkipBack => {
            // Sharp-cut skip-back: "M19 20L9 12l10-8v16z M5 19V5"
            cr.move_to(19.0, 20.0);
            cr.line_to(9.0, 12.0);
            cr.line_to(19.0, 4.0);
            cr.close_path();
            cr.move_to(5.0, 19.0);
            cr.line_to(5.0, 5.0);
            let _ = cr.stroke();
        }
        IconGlyph::SkipForward => {
            // Sharp-cut skip-forward: "M5 4l10 8-10 8V4z M19 5v14"
            cr.move_to(5.0, 4.0);
            cr.line_to(15.0, 12.0);
            cr.line_to(5.0, 20.0);
            cr.close_path();
            cr.move_to(19.0, 5.0);
            cr.line_to(19.0, 19.0);
            let _ = cr.stroke();
        }
        IconGlyph::VolumeHigh => {
            // Flea's "volume": "M11 5L6 9H2v6h4l5 4z M15.5 8.5a5 5 0 0 1 0 7 M19 5a10 10 0 0 1 0 14"
            draw_speaker_body(cr);
            cr.new_sub_path();
            cr.arc(12.0, 12.0, 5.0, -PI * 0.25, PI * 0.25);
            cr.new_sub_path();
            cr.arc(12.0, 12.0, 9.5, -PI * 0.25, PI * 0.25);
            let _ = cr.stroke();
        }
        IconGlyph::VolumeLow => {
            draw_speaker_body(cr);
            cr.new_sub_path();
            cr.arc(12.0, 12.0, 5.0, -PI * 0.25, PI * 0.25);
            let _ = cr.stroke();
        }
        IconGlyph::VolumeMute => {
            // Flea's "volume-x": "M11 5L6 9H2v6h4l5 4z M22 9l-6 6 M16 9l6 6"
            draw_speaker_body(cr);
            cr.move_to(22.0, 9.0);
            cr.line_to(16.0, 15.0);
            cr.move_to(16.0, 9.0);
            cr.line_to(22.0, 15.0);
            let _ = cr.stroke();
        }
        IconGlyph::Repeat => {
            // Sharp-cut repeat / A-B loop
            cr.move_to(17.0, 3.0);
            cr.line_to(21.0, 7.0);
            cr.line_to(17.0, 11.0);
            cr.move_to(3.0, 11.0);
            cr.line_to(3.0, 7.0);
            cr.line_to(21.0, 7.0);
            cr.move_to(7.0, 21.0);
            cr.line_to(3.0, 17.0);
            cr.line_to(7.0, 13.0);
            cr.move_to(21.0, 13.0);
            cr.line_to(21.0, 17.0);
            cr.line_to(3.0, 17.0);
            let _ = cr.stroke();
        }
        IconGlyph::Camera => {
            // Flea's "camera": "M14.5 4h-5L7 7H2v13h20V7h-5z M15 13a3 3 0 1 1-6 0a3 3 0 1 1 6 0"
            cr.move_to(14.5, 4.0);
            cr.line_to(9.5, 4.0);
            cr.line_to(7.0, 7.0);
            cr.line_to(2.0, 7.0);
            cr.line_to(2.0, 20.0);
            cr.line_to(22.0, 20.0);
            cr.line_to(22.0, 7.0);
            cr.line_to(17.0, 7.0);
            cr.close_path();
            cr.new_sub_path();
            cr.arc(12.0, 13.0, 3.2, 0.0, 2.0 * PI);
            let _ = cr.stroke();
        }
        IconGlyph::Maximize => {
            // Flea's "maximize": "M8 3H3v5 M16 3h5v5 M8 21H3v-5 M16 21h5v-5"
            cr.move_to(8.0, 3.0);
            cr.line_to(3.0, 3.0);
            cr.line_to(3.0, 8.0);
            cr.move_to(16.0, 3.0);
            cr.line_to(21.0, 3.0);
            cr.line_to(21.0, 8.0);
            cr.move_to(8.0, 21.0);
            cr.line_to(3.0, 21.0);
            cr.line_to(3.0, 16.0);
            cr.move_to(16.0, 21.0);
            cr.line_to(21.0, 21.0);
            cr.line_to(21.0, 16.0);
            let _ = cr.stroke();
        }
        IconGlyph::Minimize => {
            cr.move_to(4.0, 9.0);
            cr.line_to(9.0, 9.0);
            cr.line_to(9.0, 4.0);
            cr.move_to(20.0, 9.0);
            cr.line_to(15.0, 9.0);
            cr.line_to(15.0, 4.0);
            cr.move_to(4.0, 15.0);
            cr.line_to(9.0, 15.0);
            cr.line_to(9.0, 20.0);
            cr.move_to(20.0, 15.0);
            cr.line_to(15.0, 15.0);
            cr.line_to(15.0, 20.0);
            let _ = cr.stroke();
        }
        IconGlyph::List => {
            // Flea's "list": "M8 6h13 M8 12h13 M8 18h13 M3 6L3.01 6 M3 12L3.01 12 M3 18L3.01 18"
            cr.move_to(8.0, 6.0);
            cr.line_to(21.0, 6.0);
            cr.move_to(8.0, 12.0);
            cr.line_to(21.0, 12.0);
            cr.move_to(8.0, 18.0);
            cr.line_to(21.0, 18.0);
            cr.move_to(3.0, 6.0);
            cr.line_to(3.05, 6.0);
            cr.move_to(3.0, 12.0);
            cr.line_to(3.05, 12.0);
            cr.move_to(3.0, 18.0);
            cr.line_to(3.05, 18.0);
            let _ = cr.stroke();
        }
        IconGlyph::Sliders => {
            // Flea's "sliders": "M4 21v-7 M4 10V3 M12 21v-9 M12 8V3 M20 21v-5 M20 12V3 M2 14h4 M10 8h4 M18 16h4"
            cr.move_to(4.0, 21.0);
            cr.line_to(4.0, 14.0);
            cr.move_to(4.0, 10.0);
            cr.line_to(4.0, 3.0);
            cr.move_to(12.0, 21.0);
            cr.line_to(12.0, 12.0);
            cr.move_to(12.0, 8.0);
            cr.line_to(12.0, 3.0);
            cr.move_to(20.0, 21.0);
            cr.line_to(20.0, 16.0);
            cr.move_to(20.0, 12.0);
            cr.line_to(20.0, 3.0);
            cr.move_to(2.0, 14.0);
            cr.line_to(6.0, 14.0);
            cr.move_to(10.0, 8.0);
            cr.line_to(14.0, 8.0);
            cr.move_to(18.0, 16.0);
            cr.line_to(22.0, 16.0);
            let _ = cr.stroke();
        }
        IconGlyph::Chapters => {
            // Bookmark / chapters glyph: "M5 3h14v18l-7-4-7 4V3z"
            cr.move_to(5.0, 3.0);
            cr.line_to(19.0, 3.0);
            cr.line_to(19.0, 21.0);
            cr.line_to(12.0, 16.5);
            cr.line_to(5.0, 21.0);
            cr.close_path();
            let _ = cr.stroke();
        }
        IconGlyph::Globe => {
            // Flea's "globe"
            cr.arc(12.0, 12.0, 9.5, 0.0, 2.0 * PI);
            cr.move_to(2.5, 12.0);
            cr.line_to(21.5, 12.0);
            cr.move_to(12.0, 2.5);
            cr.curve_to(16.5, 6.5, 16.5, 17.5, 12.0, 21.5);
            cr.move_to(12.0, 2.5);
            cr.curve_to(7.5, 6.5, 7.5, 17.5, 12.0, 21.5);
            let _ = cr.stroke();
        }
        IconGlyph::Keyboard => {
            // Flea's "keyboard": "M2 6h20v12H2z M6 10L6.01 10 ... M8 14h8"
            cr.rectangle(2.0, 5.0, 20.0, 14.0);
            for x in [6.0, 10.0, 14.0, 18.0] {
                cr.move_to(x, 10.0);
                cr.line_to(x + 0.1, 10.0);
            }
            cr.move_to(8.0, 15.0);
            cr.line_to(16.0, 15.0);
            let _ = cr.stroke();
        }
        IconGlyph::Close => {
            // Flea's "x": "M6 6l12 12 M18 6 6 18"
            cr.move_to(6.0, 6.0);
            cr.line_to(18.0, 18.0);
            cr.move_to(18.0, 6.0);
            cr.line_to(6.0, 18.0);
            let _ = cr.stroke();
        }
        IconGlyph::Film => {
            // Flea's "film": "M3 3h18v18H3z M7 3v18 M3 7.5h4 M3 12h18 M3 16.5h4 M17 3v18 M17 7.5h4 M17 16.5h4"
            cr.rectangle(3.0, 3.0, 18.0, 18.0);
            cr.move_to(7.0, 3.0);
            cr.line_to(7.0, 21.0);
            cr.move_to(17.0, 3.0);
            cr.line_to(17.0, 21.0);
            cr.move_to(3.0, 12.0);
            cr.line_to(21.0, 12.0);
            for y in [7.5, 16.5] {
                cr.move_to(3.0, y);
                cr.line_to(7.0, y);
                cr.move_to(17.0, y);
                cr.line_to(21.0, y);
            }
            let _ = cr.stroke();
        }
        IconGlyph::FolderOpen => {
            // Flea's "folder-open": "M2 20V3h6l2 3h12v3 M22 11l-2.5 9H2l2.5-9z"
            cr.move_to(2.0, 20.0);
            cr.line_to(2.0, 3.0);
            cr.line_to(8.0, 3.0);
            cr.line_to(10.0, 6.0);
            cr.line_to(22.0, 6.0);
            cr.line_to(22.0, 9.0);
            cr.move_to(22.0, 11.0);
            cr.line_to(19.5, 20.0);
            cr.line_to(2.0, 20.0);
            cr.line_to(4.5, 11.0);
            cr.close_path();
            let _ = cr.stroke();
        }
        IconGlyph::Plus => {
            // Flea's "plus": "M5 12h14 M12 5v14"
            cr.move_to(5.0, 12.0);
            cr.line_to(19.0, 12.0);
            cr.move_to(12.0, 5.0);
            cr.line_to(12.0, 19.0);
            let _ = cr.stroke();
        }
    }

    let _ = cr.restore();
}

fn draw_speaker_body(cr: &Context) {
    cr.move_to(11.0, 5.0);
    cr.line_to(6.0, 9.0);
    cr.line_to(2.0, 9.0);
    cr.line_to(2.0, 15.0);
    cr.line_to(6.0, 15.0);
    cr.line_to(11.0, 19.0);
    cr.close_path();
}
