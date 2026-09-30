use gdk4::{Key, ModifierType};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeybindingEntry {
    pub keys: &'static str,
    pub description: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeymapGroup {
    pub title: &'static str,
    pub entries: &'static [KeybindingEntry],
}

pub const KEYMAP_GROUPS: &[KeymapGroup] = &[
    KeymapGroup {
        title: "PLAYBACK & NAVIGATION",
        entries: &[
            KeybindingEntry {
                keys: "Space / k",
                description: "Play / Pause",
            },
            KeybindingEntry {
                keys: "h / l / ← / →",
                description: "Seek -5s / +5s",
            },
            KeybindingEntry {
                keys: "H / L / Shift+←/→",
                description: "Exact seek -1s / +1s",
            },
            KeybindingEntry {
                keys: ". / ,",
                description: "Frame step forward / backward",
            },
            KeybindingEntry {
                keys: "Ctrl+← / Ctrl+→",
                description: "Previous / Next chapter",
            },
            KeybindingEntry {
                keys: "n / >  ·  N / <",
                description: "Next / Previous playlist item",
            },
            KeybindingEntry {
                keys: "r",
                description: "Cycle A-B loop (A → B → Clear)",
            },
        ],
    },
    KeymapGroup {
        title: "AUDIO, SUBTITLES & SPEED",
        entries: &[
            KeybindingEntry {
                keys: "j / ↓ / 9  ·  ↑ / 0",
                description: "Volume -5% / +5%",
            },
            KeybindingEntry {
                keys: "m",
                description: "Toggle mute",
            },
            KeybindingEntry {
                keys: "[ / ]",
                description: "Playback speed -0.1× / +0.1×",
            },
            KeybindingEntry {
                keys: "{ / }  ·  Bksp",
                description: "Halve / Double / Reset speed (1.0×)",
            },
            KeybindingEntry {
                keys: "a",
                description: "Cycle audio track",
            },
            KeybindingEntry {
                keys: "v / V",
                description: "Cycle subtitle track / Toggle visibility",
            },
            KeybindingEntry {
                keys: "z / x",
                description: "Subtitle delay -0.1s / +0.1s",
            },
        ],
    },
    KeymapGroup {
        title: "INTERFACE, PANELS & CAPTURE",
        entries: &[
            KeybindingEntry {
                keys: "f / F11 / Esc",
                description: "Toggle / Exit fullscreen",
            },
            KeybindingEntry {
                keys: "b / Tab",
                description: "Switch Cinema HUD / Minimal Vim bar",
            },
            KeybindingEntry {
                keys: "p / t / c",
                description: "Toggle Playlist / Tracks / Chapters drawer",
            },
            KeybindingEntry {
                keys: "s / S",
                description: "Screenshot video / with subtitles",
            },
            KeybindingEntry {
                keys: "o / u",
                description: "Open local file / Open stream URL",
            },
            KeybindingEntry {
                keys: ":  ·  ?  ·  q",
                description: "Command bar / Keymap sheet / Quit",
            },
        ],
    },
];

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerAction {
    TogglePause,
    SeekRelative(f64),
    FrameStep(bool),
    PrevChapter,
    NextChapter,
    AddVolume(f64),
    ToggleMute,
    AddSpeed(f64),
    MultiplySpeed(f64),
    ResetSpeed,
    ToggleFullscreen,
    Escape,
    CycleAudio,
    CycleSub,
    ToggleSubVisibility,
    AdjustSubDelay(f64),
    ToggleDrawerChapters,
    ToggleDrawerPlaylist,
    ToggleDrawerTracks,
    ToggleHudMode,
    Screenshot { include_subs: bool },
    AbLoopCycle,
    OpenFileDialog,
    OpenUrlDialog,
    PlaylistNext,
    PlaylistPrev,
    OpenCommandBar,
    ToggleKeymapSheet,
    Quit,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VimCommand {
    SeekSeconds(f64),
    SeekRelative(f64),
    SeekPercent(f64),
    Speed(f64),
    Volume(f64),
    SubDelay(f64),
    Open(String),
    ThemeReload,
    Quit,
}

pub fn parse_vim_command(input: &str) -> Result<VimCommand, String> {
    let trimmed = input.trim().trim_start_matches(':').trim();
    if trimmed.is_empty() {
        return Err("Empty command".to_string());
    }

    let (cmd, rest) = match trimmed.split_once(char::is_whitespace) {
        Some((c, r)) => (c.trim(), r.trim()),
        None => (trimmed, ""),
    };

    match cmd {
        "q" | "quit" | "exit" => Ok(VimCommand::Quit),
        "theme" | "reload-theme" => Ok(VimCommand::ThemeReload),
        "open" | "e" | "edit" | "play" => {
            if rest.is_empty() {
                Err("Usage: :open <path-or-url>".to_string())
            } else {
                Ok(VimCommand::Open(rest.to_string()))
            }
        }
        "seek" => {
            if rest.is_empty() {
                return Err("Usage: :seek <sec|+sec|-sec|N%>".to_string());
            }
            if let Some(pct_str) = rest.strip_suffix('%') {
                let pct = pct_str
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| format!("Invalid percentage: {rest}"))?;
                Ok(VimCommand::SeekPercent(pct.clamp(0.0, 100.0)))
            } else if rest.starts_with('+') || rest.starts_with('-') {
                let delta = rest
                    .parse::<f64>()
                    .map_err(|_| format!("Invalid relative seek: {rest}"))?;
                Ok(VimCommand::SeekRelative(delta))
            } else if let Some(secs) = parse_timestamp_or_seconds(rest) {
                Ok(VimCommand::SeekSeconds(secs))
            } else {
                Err(format!("Invalid seek target: {rest}"))
            }
        }
        "speed" | "rate" => {
            let val = rest
                .trim_end_matches('x')
                .parse::<f64>()
                .map_err(|_| "Usage: :speed <0.1..8.0>".to_string())?;
            Ok(VimCommand::Speed(val.clamp(0.1, 8.0)))
        }
        "vol" | "volume" => {
            let val = rest
                .trim_end_matches('%')
                .parse::<f64>()
                .map_err(|_| "Usage: :vol <0..150>".to_string())?;
            Ok(VimCommand::Volume(val.clamp(0.0, 150.0)))
        }
        "sub-delay" | "subdelay" => {
            let val = rest
                .trim_end_matches('s')
                .parse::<f64>()
                .map_err(|_| "Usage: :sub-delay <seconds>".to_string())?;
            Ok(VimCommand::SubDelay(val))
        }
        _ => Err(format!("Unknown command: :{cmd}")),
    }
}

fn parse_timestamp_or_seconds(s: &str) -> Option<f64> {
    if let Ok(sec) = s.parse::<f64>() {
        return Some(sec.max(0.0));
    }
    let parts: Vec<&str> = s.split(':').collect();
    match parts.as_slice() {
        [m, sec] => {
            let mins = m.parse::<f64>().ok()?;
            let secs = sec.parse::<f64>().ok()?;
            Some((mins * 60.0 + secs).max(0.0))
        }
        [h, m, sec] => {
            let hrs = h.parse::<f64>().ok()?;
            let mins = m.parse::<f64>().ok()?;
            let secs = sec.parse::<f64>().ok()?;
            Some((hrs * 3600.0 + mins * 60.0 + secs).max(0.0))
        }
        _ => None,
    }
}

pub fn map_key_event(keyval: Key, state: ModifierType) -> Option<PlayerAction> {
    let ctrl = state.contains(ModifierType::CONTROL_MASK);
    let shift = state.contains(ModifierType::SHIFT_MASK);
    let alt = state.contains(ModifierType::ALT_MASK);

    if alt {
        return None;
    }

    if ctrl {
        return match keyval {
            Key::Left => Some(PlayerAction::PrevChapter),
            Key::Right => Some(PlayerAction::NextChapter),
            Key::o | Key::O => Some(PlayerAction::OpenFileDialog),
            Key::u | Key::U => Some(PlayerAction::OpenUrlDialog),
            Key::q | Key::Q => Some(PlayerAction::Quit),
            _ => None,
        };
    }

    match keyval {
        Key::space | Key::k => Some(PlayerAction::TogglePause),
        Key::h => Some(PlayerAction::SeekRelative(-5.0)),
        Key::H => Some(PlayerAction::SeekRelative(-1.0)),
        Key::l => Some(PlayerAction::SeekRelative(5.0)),
        Key::L => Some(PlayerAction::SeekRelative(1.0)),
        Key::Left => {
            if shift {
                Some(PlayerAction::SeekRelative(-1.0))
            } else {
                Some(PlayerAction::SeekRelative(-5.0))
            }
        }
        Key::Right => {
            if shift {
                Some(PlayerAction::SeekRelative(1.0))
            } else {
                Some(PlayerAction::SeekRelative(5.0))
            }
        }
        Key::j | Key::Down | Key::_9 => Some(PlayerAction::AddVolume(-5.0)),
        Key::Up | Key::_0 => Some(PlayerAction::AddVolume(5.0)),
        Key::m | Key::M => Some(PlayerAction::ToggleMute),
        Key::bracketleft => Some(PlayerAction::AddSpeed(-0.1)),
        Key::bracketright => Some(PlayerAction::AddSpeed(0.1)),
        Key::braceleft => Some(PlayerAction::MultiplySpeed(0.5)),
        Key::braceright => Some(PlayerAction::MultiplySpeed(2.0)),
        Key::BackSpace => Some(PlayerAction::ResetSpeed),
        Key::period => Some(PlayerAction::FrameStep(true)),
        Key::comma => Some(PlayerAction::FrameStep(false)),
        Key::f | Key::F | Key::F11 => Some(PlayerAction::ToggleFullscreen),
        Key::Escape => Some(PlayerAction::Escape),
        Key::a | Key::A => Some(PlayerAction::CycleAudio),
        Key::v => Some(PlayerAction::CycleSub),
        Key::V => Some(PlayerAction::ToggleSubVisibility),
        Key::z | Key::Z => Some(PlayerAction::AdjustSubDelay(-0.1)),
        Key::x | Key::X => Some(PlayerAction::AdjustSubDelay(0.1)),
        Key::c | Key::C => Some(PlayerAction::ToggleDrawerChapters),
        Key::p | Key::P => Some(PlayerAction::ToggleDrawerPlaylist),
        Key::t | Key::T => Some(PlayerAction::ToggleDrawerTracks),
        Key::b | Key::B | Key::Tab | Key::ISO_Left_Tab => Some(PlayerAction::ToggleHudMode),
        Key::s => Some(PlayerAction::Screenshot {
            include_subs: false,
        }),
        Key::S => Some(PlayerAction::Screenshot { include_subs: true }),
        Key::r | Key::R => Some(PlayerAction::AbLoopCycle),
        Key::o | Key::O => Some(PlayerAction::OpenFileDialog),
        Key::u | Key::U => Some(PlayerAction::OpenUrlDialog),
        Key::n | Key::greater => Some(PlayerAction::PlaylistNext),
        Key::N | Key::less => Some(PlayerAction::PlaylistPrev),
        Key::colon => Some(PlayerAction::OpenCommandBar),
        Key::question => Some(PlayerAction::ToggleKeymapSheet),
        Key::q | Key::Q => Some(PlayerAction::Quit),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_vim_commands() {
        assert_eq!(
            parse_vim_command(":seek 45"),
            Ok(VimCommand::SeekSeconds(45.0))
        );
        assert_eq!(
            parse_vim_command("seek 01:30"),
            Ok(VimCommand::SeekSeconds(90.0))
        );
        assert_eq!(
            parse_vim_command(":seek +15"),
            Ok(VimCommand::SeekRelative(15.0))
        );
        assert_eq!(
            parse_vim_command(":seek 50%"),
            Ok(VimCommand::SeekPercent(50.0))
        );
        assert_eq!(
            parse_vim_command(":speed 1.25x"),
            Ok(VimCommand::Speed(1.25))
        );
        assert_eq!(parse_vim_command(":vol 85"), Ok(VimCommand::Volume(85.0)));
        assert_eq!(
            parse_vim_command(":sub-delay -0.3"),
            Ok(VimCommand::SubDelay(-0.3))
        );
        assert_eq!(
            parse_vim_command(":open https://example.com/stream.mp4"),
            Ok(VimCommand::Open("https://example.com/stream.mp4".to_string()))
        );
        assert_eq!(parse_vim_command(":theme"), Ok(VimCommand::ThemeReload));
        assert_eq!(parse_vim_command(":q"), Ok(VimCommand::Quit));
    }
}
