mod config;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as platform;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as platform;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
compile_error!("KeyNinja supports only macOS and Linux");

use config::{Action, Config};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
KeyNinja switches the keyboard layout when the active application changes.

Usage:
  keyninja [run] [-c <config>] [-v]   run (default)
  keyninja list                        list available keyboard layouts
  keyninja config                      print the config path (creates it if missing)
  keyninja install [-c <config>]       start automatically at login
  keyninja uninstall                   remove autostart

Options:
  -c, --config <path>   config file (default: ~/.config/keyninja/config.toml)
  -v, --verbose         log every app switch and layout change
";

/// Shared handler used by the platform backends.
pub struct Switcher {
    pub config: Config,
    pub config_path: PathBuf,
    pub verbose: bool,
    /// Paused from the menu: events still arrive, but the layout is left alone.
    pub paused: std::cell::Cell<bool>,
}

impl Switcher {
    /// Called by the backend when an application is activated.
    /// `ids` are the app identifiers, `select` switches the layout.
    pub fn on_activate(&self, ids: &[&str], select: impl FnOnce(&str) -> Result<bool, String>) {
        if self.paused.get() {
            if self.verbose {
                eprintln!("[{}] paused", ids.join(" | "));
            }
            return;
        }
        let Action::Switch(layout) = self.config.action_for(ids) else {
            if self.verbose {
                eprintln!("[{}] ignored", ids.join(" | "));
            }
            return;
        };
        if self.config.delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(self.config.delay_ms));
        }
        match select(layout) {
            Ok(changed) if self.verbose => eprintln!(
                "[{}] -> {layout}{}",
                ids.join(" | "),
                if changed { "" } else { " (already active)" }
            ),
            Ok(_) => {}
            Err(e) => eprintln!("keyninja: [{}] failed to switch to '{layout}': {e}", ids.join(" | ")),
        }
    }
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut cmd = String::from("run");
    let mut config_path: Option<PathBuf> = None;
    let mut verbose = false;
    let mut cmd_set = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "-v" | "--verbose" => verbose = true,
            "-c" | "--config" => match args.next() {
                Some(p) => config_path = Some(PathBuf::from(p)),
                None => return fail("-c requires a path"),
            },
            c if !cmd_set && !c.starts_with('-') => {
                cmd = c.to_string();
                cmd_set = true;
            }
            other => return fail(&format!("unknown argument '{other}'\n\n{USAGE}")),
        }
    }
    let config_path = config_path.unwrap_or_else(config::default_path);
    let config_path = std::path::absolute(&config_path).unwrap_or(config_path);

    let result = match cmd.as_str() {
        "run" => config::load_or_create(&config_path).and_then(|config| {
            if verbose {
                eprintln!("keyninja: config {}", config_path.display());
            }
            platform::run(Switcher {
                config,
                config_path: config_path.clone(),
                verbose,
                paused: Default::default(),
            })
        }),
        "list" => platform::list(),
        "config" => config::load_or_create(&config_path).map(|_| println!("{}", config_path.display())),
        "install" => platform::install(&config_path),
        "uninstall" => platform::uninstall(),
        other => Err(format!("unknown command '{other}'\n\n{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(&e),
    }
}

fn fail(msg: &str) -> ExitCode {
    eprintln!("keyninja: {msg}");
    ExitCode::FAILURE
}
