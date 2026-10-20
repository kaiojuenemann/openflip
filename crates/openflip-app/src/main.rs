//! openflip application entry point.
//!
//! For now this prints the clock face to the terminal so the core can be
//! exercised end to end; the GPU renderer replaces it in milestone 1.

use std::process::ExitCode;

use openflip_core::{ClockFace, Config, DateLine, Language};

fn main() -> ExitCode {
    let config = match Config::default_path().and_then(|path| Config::load(&path)) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("openflip: {e}");
            return ExitCode::FAILURE;
        }
    };
    let language = config.language.unwrap_or_else(system_language);
    let now = chrono::Local::now().naive_local();

    let face = ClockFace::at(now, &config);
    let mut line = format!(
        "{}{}:{}{}",
        face.hours[0], face.hours[1], face.minutes[0], face.minutes[1]
    );
    if let Some(seconds) = face.seconds {
        line.push_str(&format!(":{}{}", seconds[0], seconds[1]));
    }
    if let Some(pm) = face.pm {
        line.push_str(if pm { " PM" } else { " AM" });
    }
    println!("{line}");
    if config.show_date {
        println!("{}", DateLine::at(now, language).0);
    }
    ExitCode::SUCCESS
}

fn system_language() -> Language {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|var| std::env::var(var).ok().filter(|v| !v.is_empty()))
        .map(|tag| Language::from_locale(&tag))
        .unwrap_or_default()
}
