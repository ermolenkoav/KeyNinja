//! Linux: picks a backend based on the session type.

mod wayland;
mod x11;

use crate::Switcher;


enum Session {
    X11,
    Wayland,
}

fn session() -> Session {
    let ty = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    if ty.eq_ignore_ascii_case("wayland")
        || (ty.is_empty() && std::env::var_os("WAYLAND_DISPLAY").is_some())
    {
        Session::Wayland
    } else {
        Session::X11
    }
}

pub fn run(switcher: Switcher) -> Result<(), String> {
    match session() {
        Session::X11 => x11::run(switcher),
        Session::Wayland => wayland::run(switcher),
    }
}

pub fn list() -> Result<(), String> {
    match session() {
        Session::X11 => x11::list(),
        Session::Wayland => wayland::list(),
    }
}

pub use x11::{install, uninstall};

