mod keymap;
mod mpris;
mod mpv;
mod theme;
mod ui;

use std::cell::RefCell;
use std::rc::Rc;

use gio::prelude::*;
use gtk4::prelude::*;

use crate::theme::ThemePalette;
use crate::ui::{LaunchOptions, WindowContext, build_window};

enum CliMode {
    DumpTheme,
    Help,
    Version,
    Run(LaunchOptions),
}

fn parse_cli_args() -> Result<CliMode, String> {
    let mut opts = LaunchOptions::default();
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dump-theme" => return Ok(CliMode::DumpTheme),
            "-h" | "--help" => return Ok(CliMode::Help),
            "-V" | "--version" => return Ok(CliMode::Version),
            "--minimal" => opts.minimal_hud = true,
            "-f" | "--fullscreen" => opts.fullscreen = true,
            "--start" => {
                let val = args
                    .next()
                    .ok_or_else(|| "--start requires a value in seconds".to_string())?;
                let secs = val
                    .parse::<f64>()
                    .map_err(|_| format!("Invalid --start seconds: {val}"))?;
                opts.start_seconds = Some(secs.max(0.0));
            }
            "--sub-file" => {
                let path = args
                    .next()
                    .ok_or_else(|| "--sub-file requires a path".to_string())?;
                opts.sub_file = Some(path);
            }
            _ if arg.starts_with("--start=") => {
                let val = &arg["--start=".len()..];
                let secs = val
                    .parse::<f64>()
                    .map_err(|_| format!("Invalid --start seconds: {val}"))?;
                opts.start_seconds = Some(secs.max(0.0));
            }
            _ if arg.starts_with("--sub-file=") => {
                let path = &arg["--sub-file=".len()..];
                opts.sub_file = Some(path.to_string());
            }
            _ => opts.files.push(arg),
        }
    }

    Ok(CliMode::Run(opts))
}

fn print_help() {
    println!(
        "\
omaplay 0.1.0 — Fast, themed libmpv video player for Omarchy & Arch Linux

USAGE:
    omaplay [OPTIONS] [FILES_OR_URLS]...

OPTIONS:
    -f, --fullscreen         Start in fullscreen mode
        --minimal            Start in minimal Vim status-strip HUD mode
        --start <SECONDS>    Seek to timestamp (seconds) on load
        --sub-file <PATH>    Attach external subtitle file on load
        --dump-theme         Print resolved Omarchy/Arch theme palette and exit
    -h, --help               Print help information
    -V, --version            Print version information"
    );
}

fn main() {
    let opts = match parse_cli_args() {
        Ok(CliMode::DumpTheme) => {
            let palette = ThemePalette::load();
            println!("{}", palette.dump_key_values());
            return;
        }
        Ok(CliMode::Help) => {
            print_help();
            return;
        }
        Ok(CliMode::Version) => {
            println!("omaplay {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Ok(CliMode::Run(o)) => o,
        Err(e) => {
            eprintln!("omaplay: {e}");
            std::process::exit(2);
        }
    };

    let app = libadwaita::Application::builder()
        .application_id("org.omarchy.Omaplay")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();

    let active_ctx: Rc<RefCell<Option<Rc<WindowContext>>>> = Rc::new(RefCell::new(None));
    let opts_cell = Rc::new(RefCell::new(opts));

    let ctx_activate = Rc::clone(&active_ctx);
    let opts_activate = Rc::clone(&opts_cell);
    app.connect_activate(move |app| {
        if let Some(existing) = ctx_activate.borrow().as_ref() {
            existing.window.present();
            return;
        }
        let launch_opts = opts_activate.borrow().clone();
        let ctx = build_window(app, launch_opts);
        *ctx_activate.borrow_mut() = Some(ctx);
    });

    let empty_args: [&str; 0] = [];
    app.run_with_args(&empty_args);
}
