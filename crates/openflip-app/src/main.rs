//! openflip application entry point.

mod cli;
mod screenshot;
mod window;

use std::process::ExitCode;

use openflip_core::Config;
use openflip_host::{load_config, Look};

fn main() -> ExitCode {
    let options = match cli::parse(std::env::args().skip(1)) {
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
