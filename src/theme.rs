use std::cell::RefCell;
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

use gio::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemePalette {
    pub name: String,
    pub mode: String,
    pub background: String,
    pub dark_background: String,
    pub darker_background: String,
    pub lighter_background: String,
    pub foreground: String,
    pub muted: String,
    pub accent: String,
    pub selection: String,
    pub active_border: String,
    pub red: String,
    pub yellow: String,
    pub green: String,
    pub cyan: String,
    pub blue: String,
    pub magenta: String,
    pub font_family: String,
    pub base_font_size: u32,
    pub corner_radius: u32,
}

#[derive(Debug, Default, Deserialize)]
struct PartialColorsToml {
    name: Option<String>,
    mode: Option<String>,
    background: Option<String>,
    dark_background: Option<String>,
    darker_background: Option<String>,
    lighter_background: Option<String>,
    foreground: Option<String>,
    muted: Option<String>,
    accent: Option<String>,
    selection: Option<String>,
    active_border: Option<String>,
    red: Option<String>,
    yellow: Option<String>,
    green: Option<String>,
    cyan: Option<String>,
    blue: Option<String>,
    magenta: Option<String>,
    font_family: Option<String>,
    base_font_size: Option<u32>,
    corner_radius: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
struct ShellToml {
    #[serde(default)]
    hyprland: Option<ShellHyprlandSection>,
    #[serde(default)]
    font: Option<ShellFontSection>,
}

#[derive(Debug, Default, Deserialize)]
struct ShellHyprlandSection {
    #[serde(rename = "active-border")]
    active_border: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ShellFontSection {
    #[serde(rename = "base-size")]
    base_size: Option<u32>,
}

impl Default for ThemePalette {
    fn default() -> Self {
        Self {
            name: "catppuccin".to_string(),
            mode: "dark".to_string(),
            background: "#1e1e2e".to_string(),
            dark_background: "#161622".to_string(),
            darker_background: "#101019".to_string(),
            lighter_background: "#313244".to_string(),
            foreground: "#cdd6f4".to_string(),
            muted: "#585b70".to_string(),
            accent: "#89b4fa".to_string(),
            selection: "#45475a".to_string(),
            active_border: "#89b4fa".to_string(),
            red: "#f38ba8".to_string(),
            yellow: "#f9e2af".to_string(),
            green: "#a6e3a1".to_string(),
            cyan: "#94e2d5".to_string(),
            blue: "#89b4fa".to_string(),
            magenta: "#f5c2e7".to_string(),
            font_family: "JetBrainsMono Nerd Font".to_string(),
            base_font_size: 12,
            corner_radius: 12,
        }
    }
}

impl ThemePalette {
    pub fn load() -> Self {
        let mut palette = Self::default();

        let omarchy_dir = omarchy_current_dir();
        let name_path = omarchy_dir.join("theme.name");
        if let Ok(raw_name) = std::fs::read_to_string(&name_path) {
            let trimmed = raw_name.trim();
            if !trimmed.is_empty() {
                palette.name = trimmed.to_string();
            }
        }

        let colors_path = omarchy_dir.join("theme").join("colors.toml");
        if let Ok(content) = std::fs::read_to_string(&colors_path) {
            palette.apply_colors_toml(&content);
        }

        let shell_path = omarchy_dir.join("theme").join("shell.toml");
        if let Ok(content) = std::fs::read_to_string(&shell_path) {
            palette.apply_shell_toml(&content);
        }

        if let Some(rounding) = query_hyprland_rounding() {
            palette.corner_radius = rounding;
        }
        if let Some(font) = query_omarchy_font() {
            palette.font_family = font;
        }

        let cfg_dir = omaplay_config_dir();
        let staged_omarchy = cfg_dir.join("omarchy.toml");
        if let Ok(content) = std::fs::read_to_string(&staged_omarchy) {
            if !content.trim().is_empty() {
                palette.apply_colors_toml(&content);
            }
        }

        let user_override = cfg_dir.join("theme.toml");
        if let Ok(content) = std::fs::read_to_string(&user_override) {
            if !content.trim().is_empty() {
                palette.apply_colors_toml(&content);
            }
        }

        palette
    }

    pub fn apply_colors_toml(&mut self, toml_str: &str) {
        let Ok(parsed) = toml::from_str::<PartialColorsToml>(toml_str) else {
            return;
        };
        if let Some(v) = parsed.name.filter(|s| !s.trim().is_empty()) {
            self.name = v;
        }
        if let Some(v) = parsed.mode.filter(|s| !s.trim().is_empty()) {
            self.mode = v;
        }
        if let Some(v) = parsed.background.filter(|s| is_hex_color(s)) {
            self.background = v;
        }
        if let Some(v) = parsed.dark_background.filter(|s| is_hex_color(s)) {
            self.dark_background = v;
        }
        if let Some(v) = parsed.darker_background.filter(|s| is_hex_color(s)) {
            self.darker_background = v;
        }
        if let Some(v) = parsed.lighter_background.filter(|s| is_hex_color(s)) {
            self.lighter_background = v;
        }
        if let Some(v) = parsed.foreground.filter(|s| is_hex_color(s)) {
            self.foreground = v;
        }
        if let Some(v) = parsed.muted.filter(|s| is_hex_color(s)) {
            self.muted = v;
        }
        if let Some(v) = parsed.accent.filter(|s| is_hex_color(s)) {
            self.accent = v.clone();
            if parsed.active_border.is_none() {
                self.active_border = v;
            }
        }
        if let Some(v) = parsed.selection.filter(|s| is_hex_color(s)) {
            self.selection = v;
        }
        if let Some(v) = parsed.active_border.filter(|s| is_hex_color(s)) {
            self.active_border = v;
        }
        if let Some(v) = parsed.red.filter(|s| is_hex_color(s)) {
            self.red = v;
        }
        if let Some(v) = parsed.yellow.filter(|s| is_hex_color(s)) {
            self.yellow = v;
        }
        if let Some(v) = parsed.green.filter(|s| is_hex_color(s)) {
            self.green = v;
        }
        if let Some(v) = parsed.cyan.filter(|s| is_hex_color(s)) {
            self.cyan = v;
        }
        if let Some(v) = parsed.blue.filter(|s| is_hex_color(s)) {
            self.blue = v;
        }
        if let Some(v) = parsed.magenta.filter(|s| is_hex_color(s)) {
            self.magenta = v;
        }
        if let Some(v) = parsed.font_family.filter(|s| !s.trim().is_empty()) {
            self.font_family = v;
        }
        if let Some(v) = parsed.base_font_size.filter(|&n| (8..=32).contains(&n)) {
            self.base_font_size = v;
        }
        if let Some(v) = parsed.corner_radius.filter(|&n| n <= 36) {
            self.corner_radius = v;
        }
    }

    pub fn apply_shell_toml(&mut self, toml_str: &str) {
        let Ok(parsed) = toml::from_str::<ShellToml>(toml_str) else {
            return;
        };
        if let Some(hypr) = parsed.hyprland {
            if let Some(border) = hypr.active_border {
                if let Some(first_hex) = extract_first_hex(&border) {
                    self.active_border = first_hex;
                }
            }
        }
        if let Some(font) = parsed.font {
            if let Some(sz) = font.base_size.filter(|&n| (8..=32).contains(&n)) {
                self.base_font_size = sz;
            }
        }
    }

    pub fn dump_key_values(&self) -> String {
        format!(
            "name={}\nmode={}\nbackground={}\ndark_background={}\ndarker_background={}\nlighter_background={}\nforeground={}\nmuted={}\naccent={}\nselection={}\nactive_border={}\nred={}\nyellow={}\ngreen={}\ncyan={}\nblue={}\nmagenta={}\nfont_family={}\nbase_font_size={}\ncorner_radius={}",
            self.name,
            self.mode,
            self.background,
            self.dark_background,
            self.darker_background,
            self.lighter_background,
            self.foreground,
            self.muted,
            self.accent,
            self.selection,
            self.active_border,
            self.red,
            self.yellow,
            self.green,
            self.cyan,
            self.blue,
            self.magenta,
            self.font_family,
            self.base_font_size,
            self.corner_radius
        )
    }

    pub fn to_css(&self) -> String {
        let r = self.corner_radius;
        let pill_r = r;
        let small_r = if r > 0 { (r / 2).max(2) } else { 0 };
        let font_sz = self.base_font_size.max(10);
        let title_sz = font_sz + 2;
        let small_sz = font_sz.saturating_sub(1).max(9);

        let hud_bg = css_rgba(&self.dark_background, 0.88);
        let hud_border = css_rgba(&self.foreground, 0.14);
        let top_bg = css_rgba(&self.dark_background, 0.86);
        let top_border = css_rgba(&self.foreground, 0.14);
        let drawer_bg = css_rgba(&self.dark_background, 0.96);
        let card_bg = css_rgba(&self.background, 0.96);
        let toast_bg = css_rgba(&self.dark_background, 0.90);
        let keycap_border = css_rgba(&self.foreground, 0.22);
        let subtle_hover = css_rgba(&self.foreground, 0.10);
        let active_fill = css_rgba(&self.accent, 0.20);
        let selection_bg = css_rgba(&self.selection, 0.65);

        format!(
            r#"
.omaplay-window {{
    background-color: {darker_bg};
    color: {fg};
    font-family: "{font}", monospace, sans-serif;
    font-size: {font_sz}px;
    font-variant-numeric: tabular-nums;
}}

.hud-pill {{
    background-color: {hud_bg};
    color: {fg};
    border: 1px solid {hud_border};
    border-radius: {pill_r}px;
    padding: 8px 14px;
}}

.top-bar-pill {{
    background-color: {top_bg};
    color: {fg};
    border: 1px solid {top_border};
    border-radius: {pill_r}px;
    padding: 4px 12px;
}}
.minimal-strip {{
    background-color: {dark_bg};
    color: {fg};
    border-top: 1px solid {hud_border};
    padding: 2px 10px;
    font-family: "{font}", monospace;
    font-size: {small_sz}px;
}}

.drawer-panel {{
    background-color: {drawer_bg};
    color: {fg};
    border-left: 1px solid {hud_border};
    padding: 14px;
}}

.keymap-card, .modal-card {{
    background-color: {card_bg};
    color: {fg};
    border: 1px solid {hud_border};
    border-radius: {pill_r}px;
    padding: 18px 22px;
}}

.welcome-card {{
    background-color: {hud_bg};
    color: {fg};
    border: 1px solid {top_border};
    border-radius: {pill_r}px;
    padding: 24px 30px;
}}

.command-bar {{
    background-color: {card_bg};
    color: {fg};
    border-top: 1px solid {accent};
    padding: 4px 12px;
    font-family: "{font}", monospace;
}}

.command-entry {{
    background: transparent;
    color: {fg};
    border: none;
    outline: none;
    box-shadow: none;
    font-family: "{font}", monospace;
    font-size: {font_sz}px;
}}

.toast-pill {{
    background-color: {toast_bg};
    color: {fg};
    border: 1px solid {hud_border};
    border-radius: {pill_r}px;
    padding: 6px 14px;
    font-weight: 600;
}}

.thumb-preview-card {{
    background-color: {card_bg};
    color: {fg};
    border: 1px solid {hud_border};
    border-radius: {small_r}px;
    padding: 4px;
}}

.keycap {{
    background-color: {lighter_bg};
    color: {accent};
    border: 1px solid {keycap_border};
    border-radius: {small_r}px;
    padding: 2px 7px;
    font-family: "{font}", monospace;
    font-size: {small_sz}px;
    font-weight: 700;
}}

.badge-chip {{
    background-color: {selection_bg};
    color: {fg};
    border-radius: {small_r}px;
    padding: 2px 8px;
    font-size: {small_sz}px;
    font-weight: 600;
}}

.badge-accent {{
    background-color: {active_fill};
    color: {accent};
    border: 1px solid {hud_border};
    border-radius: {small_r}px;
    padding: 2px 8px;
    font-size: {small_sz}px;
    font-weight: 700;
}}

.badge-loop {{
    background-color: {lighter_bg};
    color: {yellow};
    border: 1px solid {yellow};
    border-radius: {small_r}px;
    padding: 2px 8px;
    font-size: {small_sz}px;
    font-weight: 700;
}}

.hud-btn {{
    background: transparent;
    color: {fg};
    border: 1px solid transparent;
    border-radius: {small_r}px;
    padding: 3px 6px;
    min-height: 22px;
    min-width: 22px;
}}

.hud-btn:hover {{
    background-color: {subtle_hover};
}}

.hud-btn-primary {{
    background-color: {active_fill};
    color: {accent};
    border: 1px solid {hud_border};
    border-radius: {small_r}px;
    padding: 4px 12px;
    font-weight: 700;
}}

.hud-btn-primary:hover {{
    background-color: {accent};
    color: {darker_bg};
}}

.drawer-row {{
    background-color: transparent;
    color: {fg};
    border-radius: {small_r}px;
    padding: 6px 10px;
}}

.drawer-row:hover {{
    background-color: {subtle_hover};
}}

.drawer-row-active {{
    background-color: {active_fill};
    color: {accent};
    border: 1px solid {hud_border};
    border-radius: {small_r}px;
    padding: 6px 10px;
    font-weight: 700;
}}

.muted-label {{
    color: {muted};
    font-size: {small_sz}px;
}}

.title-label {{
    color: {fg};
    font-size: {title_sz}px;
    font-weight: 700;
}}

.section-heading {{
    color: {accent};
    font-size: {small_sz}px;
    font-weight: 700;
}}

.seek-scale trough {{
    background-color: {selection};
    border-radius: {small_r}px;
    min-height: 3px;
}}

.seek-scale highlight {{
    background-color: {accent};
    border-radius: {small_r}px;
    min-height: 3px;
}}

.seek-scale slider {{
    background-color: {accent};
    border: 1px solid {fg};
    border-radius: {small_r}px;
    min-width: 7px;
    min-height: 7px;
    margin: -3px;
}}
"#,
            darker_bg = self.darker_background,
            dark_bg = self.dark_background,
            lighter_bg = self.lighter_background,
            fg = self.foreground,
            muted = self.muted,
            accent = self.accent,
            selection = self.selection,
            yellow = self.yellow,
            font = self.font_family,
            font_sz = font_sz,
            title_sz = title_sz,
            small_sz = small_sz,
            pill_r = pill_r,
            small_r = small_r,
            hud_bg = hud_bg,
            hud_border = hud_border,
            top_bg = top_bg,
            top_border = top_border,
            drawer_bg = drawer_bg,
            card_bg = card_bg,
            toast_bg = toast_bg,
            keycap_border = keycap_border,
            subtle_hover = subtle_hover,
            active_fill = active_fill,
            selection_bg = selection_bg,
        )
    }
}

pub fn parse_hex_rgb(hex: &str) -> (u8, u8, u8) {
    let clean = hex.trim().trim_start_matches('#');
    if clean.len() >= 6 {
        let r = u8::from_str_radix(&clean[0..2], 16).unwrap_or(30);
        let g = u8::from_str_radix(&clean[2..4], 16).unwrap_or(30);
        let b = u8::from_str_radix(&clean[4..6], 16).unwrap_or(46);
        (r, g, b)
    } else {
        (30, 30, 46)
    }
}

pub fn parse_hex_rgba_f64(hex: &str, alpha: f64) -> (f64, f64, f64, f64) {
    let (r, g, b) = parse_hex_rgb(hex);
    (
        f64::from(r) / 255.0,
        f64::from(g) / 255.0,
        f64::from(b) / 255.0,
        alpha.clamp(0.0, 1.0),
    )
}

pub fn css_rgba(hex: &str, alpha: f64) -> String {
    let (r, g, b) = parse_hex_rgb(hex);
    format!("rgba({}, {}, {}, {:.2})", r, g, b, alpha.clamp(0.0, 1.0))
}

fn is_hex_color(s: &str) -> bool {
    let t = s.trim();
    t.starts_with('#') && (t.len() == 7 || t.len() == 9) && t[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn extract_first_hex(s: &str) -> Option<String> {
    if let Some(idx) = s.find('#') {
        let rest = &s[idx..];
        if rest.len() >= 7 && rest[1..7].chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(rest[..7].to_string());
        }
    }
    None
}

pub fn omarchy_current_dir() -> PathBuf {
    if let Some(override_dir) = std::env::var_os("OMAPLAY_OMARCHY_DIR") {
        return PathBuf::from(override_dir);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join(".local").join("state").join("omarchy").join("current")
}

pub fn omaplay_config_dir() -> PathBuf {
    if let Some(override_dir) = std::env::var_os("OMAPLAY_CONFIG_DIR") {
        return PathBuf::from(override_dir);
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        let p = PathBuf::from(xdg);
        if !p.as_os_str().is_empty() {
            return p.join("omaplay");
        }
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join(".config").join("omaplay")
}

fn query_hyprland_rounding() -> Option<u32> {
    let output = Command::new("hyprctl")
        .args(["-j", "getoption", "decoration:rounding"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    v.get("int")
        .and_then(|n| n.as_u64())
        .map(|n| (n as u32).min(36))
}

fn query_omarchy_font() -> Option<String> {
    if let Ok(out) = Command::new("omarchy-font-current").output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    if let Ok(out) = Command::new("omarchy").args(["font", "current"]).output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    None
}

pub struct ThemeWatcher {
    _monitors: Vec<gio::FileMonitor>,
}

impl ThemeWatcher {
    pub fn start<F>(provider: gtk4::CssProvider, on_theme_reloaded: F) -> Self
    where
        F: Fn(ThemePalette) + 'static,
    {
        let mut monitors = Vec::new();
        let callback = Rc::new(on_theme_reloaded);
        let debounce_scheduled = Rc::new(RefCell::new(false));

        let omarchy_dir = omarchy_current_dir();
        let theme_name_file = omarchy_dir.join("theme.name");
        let cfg_dir = omaplay_config_dir();
        let _ = std::fs::create_dir_all(&cfg_dir);

        let watch_paths: Vec<PathBuf> = vec![omarchy_dir, theme_name_file, cfg_dir];

        for path in watch_paths {
            let file = gio::File::for_path(&path);
            let mon = if path.is_dir() {
                file.monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE)
            } else {
                file.monitor_file(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE)
            };

            if let Ok(monitor) = mon {
                let provider_clone = provider.clone();
                let cb_clone = Rc::clone(&callback);
                let debounce_clone = Rc::clone(&debounce_scheduled);

                monitor.connect_changed(move |_mon, _file, _other, _event| {
                    if *debounce_clone.borrow() {
                        return;
                    }
                    *debounce_clone.borrow_mut() = true;

                    let provider_inner = provider_clone.clone();
                    let cb_inner = Rc::clone(&cb_clone);
                    let debounce_inner = Rc::clone(&debounce_clone);

                    glib::timeout_add_local_once(Duration::from_millis(80), move || {
                        *debounce_inner.borrow_mut() = false;
                        let new_palette = ThemePalette::load();
                        eprintln!(
                            "[omaplay::theme] live reload: name={} accent={} mode={}",
                            new_palette.name, new_palette.accent, new_palette.mode
                        );
                        provider_inner.load_from_string(&new_palette.to_css());
                        cb_inner(new_palette);
                    });
                });

                monitors.push(monitor);
            }
        }

        Self {
            _monitors: monitors,
        }
    }
}

pub fn install_global_css() -> (gtk4::CssProvider, ThemePalette) {
    let palette = ThemePalette::load();
    let provider = gtk4::CssProvider::new();
    provider.load_from_string(&palette.to_css());
    if let Some(display) = gdk4::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    (provider, palette)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_omarchy_colors_and_shell_toml() {
        let mut p = ThemePalette::default();
        let colors = r##"
mode = "dark"
accent = "#ff6699"
selection = "#334455"
muted = "#778899"
background = "#11121d"
dark_background = "#0d0e17"
darker_background = "#090a10"
lighter_background = "#1f2233"
foreground = "#e0e6f0"
red = "#f7768e"
yellow = "#e0af68"
green = "#9ece6a"
cyan = "#7dcfff"
blue = "#7aa2f7"
magenta = "#bb9af7"
"##;
        p.apply_colors_toml(colors);
        assert_eq!(p.accent, "#ff6699");
        assert_eq!(p.background, "#11121d");
        assert_eq!(p.darker_background, "#090a10");
        assert_eq!(p.foreground, "#e0e6f0");

        let shell = r##"
[hyprland]
active-border = "#7aa2f7"

[font]
base-size = 14
"##;
        p.apply_shell_toml(shell);
        assert_eq!(p.active_border, "#7aa2f7");
        assert_eq!(p.base_font_size, 14);
    }

    #[test]
    fn generates_themed_gtk_css() {
        let mut p = ThemePalette::default();
        p.accent = "#89b4fa".to_string();
        p.corner_radius = 12;
        let css = p.to_css();
        assert!(css.contains(".omaplay-window"));
        assert!(css.contains(".hud-pill"));
        assert!(css.contains(".top-bar-pill"));
        assert!(css.contains(".drawer-panel"));
        assert!(css.contains(".keymap-card"));
        assert!(css.contains(".command-bar"));
        assert!(css.contains(".toast-pill"));
        assert!(css.contains(".seek-scale trough"));
        assert!(css.contains(".keycap"));
        assert!(css.contains("#89b4fa"));
    }
}
