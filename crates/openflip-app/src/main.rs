//! openflip application entry point.

// A GUI program on Windows: no console window pops up behind the clock or
// the screensaver. Started from a terminal, it attaches to that console.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod cli;
mod screensaver;
mod screenshot;
mod window;

use std::process::ExitCode;

use openflip_core::Config;
use openflip_host::{load_config, Look};

fn main() -> ExitCode {
    attach_console();
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Windows opens a screensaver without arguments when the user wants to
    // configure it (e.g. double-click on openflip.scr).
    let args = if args.is_empty() && started_as_scr() {
        vec!["/c".to_string()]
    } else {
        args
    };
    let options = match cli::parse(args) {
        Ok(options) => options,
        Err(e) => {
            eprintln!("openflip: {e}\n\n{}", cli::USAGE);
            return ExitCode::FAILURE;
        }
    };
    if options.help {
        println!("{}", cli::USAGE);
        return ExitCode::SUCCESS;
    }

    let config_path = Config::default_path().ok();
    let result = match &options.screenshot {
        Some(path) => {
            let look = Look::from_config(load_config(config_path.as_deref()));
            screenshot::run(&look, &options, path)
        }
        None => window::run(config_path, &options),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("openflip: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Whether this program runs as `openflip.scr`, i.e. as a Windows screensaver.
fn started_as_scr() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.extension().map(|e| e.eq_ignore_ascii_case("scr")))
        .unwrap_or(false)
}

/// Lets `--help` and error messages reach the terminal that started us; a
/// Windows GUI program has no console of its own.
#[cfg(windows)]
fn attach_console() {
    use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
    // SAFETY: plain Win32 call; failing (no parent console) is fine.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(windows))]
fn attach_console() {}
