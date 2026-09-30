# Omaplay

Fast, native GTK4/Libadwaita video player in Rust backed by `libmpv` (`mpv_render_context` OpenGL + Wayland DMABUF hardware decoding) for Omarchy and Arch Linux.

## Features

- **Edge-to-Edge Hardware-Accelerated Video**: Direct `libmpv` OpenGL rendering with Wayland `wl_display` zero-copy hardware decode interop (`VAAPI` / `NVDEC` / `Vulkan`), `vd-lavc-dr` direct texture rendering, and a 150 MiB demuxer cache for instantaneous seeking.
- **Sleek Flea-Inspired Chrome & Dual HUD Modes**:
  - **Cinema HUD** (`HudMode::Cinema`): Auto-hiding floating top strip and compact bottom transport pill with Cairo-rendered 24×24 stroke-2 vector glyphs, hairline progress bar, chapter ticks, and `160×90` hover seek thumbnails.
  - **Minimal Vim Strip** (`HudMode::Minimal`, toggle with `b` or `Tab`): Single `26px` bottom status strip built for tiling window managers.
- **Live Omarchy Theming**: Automatically loads and live-reloads `~/.local/state/omarchy/current/theme/{colors,shell}.toml`, `theme.name`, Hyprland corner rounding (`decoration:rounding`), and active Omarchy font (`omarchy-font-current`), with user override support via `~/.config/omaplay/theme.toml` and Catppuccin Mocha fallback on stock Arch Linux.
- **Vim & Keyboard-First Workflow**:
  - `Space` / `k`: Play / Pause
  - `h` / `l` or `←` / `→`: Smooth coalesced keyframe seek `-5s` / `+5s` (`Shift` for exact `±1s`)
  - `j` / `↓` / `9` and `↑` / `0`: Volume `-5%` / `+5%` (`m` to mute)
  - `[` / `]`, `{` / `}`, `Backspace`: Adjust / reset playback speed
  - `.` / `,`: Frame step forward / backward
  - `a` / `v` / `V`: Cycle audio track / subtitle track / subtitle visibility (`z` / `x` for subtitle delay)
  - `p` / `t` / `c`: Slide-over drawer for Playlist Queue, Audio/Subtitle Tracks, and Chapters
  - `r`: Cycle A-B loop points
  - `s` / `S`: Save screenshot to `~/Pictures/omaplay/`
  - `o` / `u`: Open local media file / Open stream URL (`yt-dlp` / HTTP / HLS)
  - `:`: Vim command bar (`:seek 01:30`, `:seek 50%`, `:speed 1.25`, `:vol 90`, `:sub-delay -0.2`, `:open <url>`, `:theme`, `:q`)
  - `?`: Two-column keybinding reference sheet
- **Desktop Integration**: Full MPRIS2 D-Bus server (`org.mpris.MediaPlayer2.omaplay`), Wayland idle inhibitor (`hypridle`), and drag-and-drop for media files, URLs, and external `.srt`/`.ass`/`.vtt` subtitles.

## Build & Run

```bash
cargo build --release
./target/release/omaplay [OPTIONS] [FILES_OR_URLS]...
```

### CLI Options

```text
-f, --fullscreen         Start in fullscreen mode
    --minimal            Start in minimal Vim status-strip HUD mode
    --start <SECONDS>    Seek to timestamp (seconds) on load
    --sub-file <PATH>    Attach external subtitle file on load
    --dump-theme         Print resolved Omarchy/Arch theme palette and exit
```
