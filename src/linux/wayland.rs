//! Wayland: заглушка.
//!
//! В Wayland нет общего протокола для отслеживания активного окна и смены
//! раскладки — нужна отдельная реализация под каждое окружение
//! (GNOME Shell — расширение/D-Bus, KDE — KWin scripting + org.kde.keyboard,
//! Sway/Hyprland — их IPC). Будет сделано позже.

use crate::Switcher;

const NOT_YET: &str = "Wayland sessions are not supported yet (planned). \
Log in to an X11 (Xorg) session to use KeyNinja.";

pub fn run(_switcher: Switcher) -> Result<(), String> {
    Err(NOT_YET.into())
}

pub fn list() -> Result<(), String> {
    Err(NOT_YET.into())
}
