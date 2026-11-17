//! Wayland: stub.
//!
//! Wayland has no common protocol for tracking the active window or switching
//! the layout, so each desktop needs its own implementation
//! (GNOME Shell: extension/D-Bus, KDE: KWin scripting + org.kde.keyboard,
//! Sway/Hyprland: their IPC). Planned for a future release.

use crate::Switcher;

const NOT_YET: &str = "Wayland sessions are not supported yet (planned). \
Log in to an X11 (Xorg) session to use KeyNinja.";

pub fn run(_switcher: Switcher) -> Result<(), String> {
    Err(NOT_YET.into())
}

pub fn list() -> Result<(), String> {
    Err(NOT_YET.into())
}
