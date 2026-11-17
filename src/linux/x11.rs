//! Linux (X11): слежение за _NET_ACTIVE_WINDOW + переключение группы XKB.

use crate::Switcher;
use std::path::{Path, PathBuf};
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xkb::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{
    AtomEnum, ChangeWindowAttributesAux, ConnectionExt as _, EventMask, Window,
};
use x11rb::rust_connection::RustConnection;

const USE_CORE_KBD: xkb::DeviceSpec = 0x100;

struct X {
    conn: RustConnection,
    root: Window,
    net_active_window: u32,
    rules_names: u32,
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

impl X {
    fn connect() -> Result<Self, String> {
        if std::env::var_os("DISPLAY").is_none() {
            return Err("DISPLAY is not set: no X server available".into());
        }
        let (conn, screen) = x11rb::connect(None).map_err(err)?;
        let root = conn.setup().roots[screen].root;
        let reply = conn.xkb_use_extension(1, 0).map_err(err)?.reply().map_err(err)?;
        if !reply.supported {
            return Err("the X server does not support the XKB extension".into());
        }
        let atom = |name: &str| -> Result<u32, String> {
            Ok(conn.intern_atom(false, name.as_bytes()).map_err(err)?.reply().map_err(err)?.atom)
        };
        let net_active_window = atom("_NET_ACTIVE_WINDOW")?;
        let rules_names = atom("_XKB_RULES_NAMES")?;
        Ok(Self { conn, root, net_active_window, rules_names })
    }

    fn property(&self, win: Window, prop: u32, ty: impl Into<u32>) -> Option<Vec<u8>> {
        let r = self.conn.get_property(false, win, prop, ty, 0, 4096).ok()?.reply().ok()?;
        (r.format != 0).then_some(r.value)
    }

    fn active_window(&self) -> Option<Window> {
        let v = self.property(self.root, self.net_active_window, AtomEnum::WINDOW)?;
        let w = u32::from_ne_bytes(v.get(..4)?.try_into().ok()?);
        (w != 0).then_some(w)
    }

    /// WM_CLASS = "instance\0Class\0"
    fn wm_class(&self, win: Window) -> Vec<String> {
        self.property(win, AtomEnum::WM_CLASS.into(), AtomEnum::STRING)
            .map(|v| {
                v.split(|b| *b == 0)
                    .filter(|s| !s.is_empty())
                    .map(|s| String::from_utf8_lossy(s).into_owned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Раскладки из _XKB_RULES_NAMES: "rules\0model\0us,ru\0variants\0options\0".
    fn layouts(&self) -> Vec<String> {
        self.property(self.root, self.rules_names, AtomEnum::STRING)
            .and_then(|v| {
                let field = v.split(|b| *b == 0).nth(2)?.to_vec();
                Some(String::from_utf8_lossy(&field).split(',').map(|s| s.trim().to_string()).collect())
            })
            .unwrap_or_default()
    }

    fn current_group(&self) -> Option<u8> {
        let st = self.conn.xkb_get_state(USE_CORE_KBD).ok()?.reply().ok()?;
        Some(u8::from(st.locked_group))
    }

    fn resolve(&self, spec: &str) -> Result<u8, String> {
        if let Ok(n) = spec.parse::<u8>() {
            return if n < 4 { Ok(n) } else { Err("XKB group number must be 0..3".into()) };
        }
        let layouts = self.layouts();
        let spec = spec.to_ascii_lowercase();
        // Коды языков, у которых раскладка XKB называется иначе.
        let aliases: &[&str] = match spec.as_str() {
            "en" => &["us", "gb"],
            "uk" => &["ua"],
            "ja" => &["jp"],
            "ko" => &["kr"],
            "cs" => &["cz"],
            "da" => &["dk"],
            "sv" => &["se"],
            "el" => &["gr"],
            "he" => &["il"],
            _ => &[],
        };
        std::iter::once(spec.as_str())
            .chain(aliases.iter().copied())
            .find_map(|want| layouts.iter().position(|l| l.eq_ignore_ascii_case(want)))
            .map(|i| i as u8)
            .ok_or_else(|| format!("not among the X11 layouts {layouts:?} (see `keyninja list`)"))
    }

    fn select(&self, spec: &str) -> Result<bool, String> {
        let group = self.resolve(spec)?;
        if self.current_group() == Some(group) {
            return Ok(false);
        }
        self.conn
            .xkb_latch_lock_state(
                USE_CORE_KBD,
                0u8.into(),
                0u8.into(),
                true,
                xkb::Group::from(group),
                0u8.into(),
                false,
                0,
            )
            .map_err(err)?
            .check()
            .map_err(err)?;
        Ok(true)
    }
}

pub fn run(switcher: Switcher) -> Result<(), String> {
    let x = X::connect()?;
    let specs = switcher.config.default.iter().chain(switcher.config.rules.iter().map(|r| &r.layout));
    for spec in specs {
        match x.resolve(spec) {
            Ok(g) if switcher.verbose => eprintln!("keyninja: '{spec}' -> group {g}"),
            Ok(_) => {}
            Err(e) => eprintln!("keyninja: warning: '{spec}': {e}"),
        }
    }

    x.conn
        .change_window_attributes(
            x.root,
            &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )
        .map_err(err)?
        .check()
        .map_err(err)?;

    let mut last = None;
    let mut on_window = |w: Window| {
        if last == Some(w) {
            return;
        }
        last = Some(w);
        let class = x.wm_class(w);
        let ids: Vec<&str> = class.iter().map(String::as_str).collect();
        switcher.on_activate(&ids, |spec| x.select(spec));
    };
    if let Some(w) = x.active_window() {
        on_window(w);
    }
    eprintln!("keyninja: running (Ctrl+C to quit)");
    loop {
        let event = x.conn.wait_for_event().map_err(err)?;
        if let Event::PropertyNotify(e) = event
            && e.window == x.root
            && e.atom == x.net_active_window
            && let Some(w) = x.active_window()
        {
            on_window(w);
        }
    }
}

pub fn list() -> Result<(), String> {
    let x = X::connect()?;
    let cur = x.current_group();
    let layouts = x.layouts();
    if layouts.is_empty() {
        return Err("failed to read _XKB_RULES_NAMES".into());
    }
    println!("  Group   Layout");
    for (i, l) in layouts.iter().enumerate() {
        let mark = if cur == Some(i as u8) { '*' } else { ' ' };
        println!("{mark} {i:<7} {l}");
    }
    println!("\n* = current. In the config, use a layout (\"ru\"), a language (\"en\") or a group number.");
    Ok(())
}

fn desktop_file() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"));
    base.join("autostart/keyninja.desktop")
}

fn desktop_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('`', "\\`").replace('$', "\\$"))
}

pub fn install(config: &Path) -> Result<(), String> {
    crate::config::load_or_create(config)?;
    let exe = std::env::current_exe().map_err(err)?;
    let path = desktop_file();
    let body = format!(
        "[Desktop Entry]\nType=Application\nName=KeyNinja\nComment=Automatic keyboard layout switching\nExec={} run -c {}\nX-GNOME-Autostart-enabled=true\nNoDisplay=true\n",
        desktop_quote(&exe.to_string_lossy()),
        desktop_quote(&config.to_string_lossy())
    );
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(err)?;
    }
    std::fs::write(&path, body).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("Autostart installed: {} (takes effect at next login)", path.display());
    Ok(())
}

pub fn uninstall() -> Result<(), String> {
    let path = desktop_file();
    if path.exists() {
        std::fs::remove_file(&path).map_err(err)?;
    }
    println!("Autostart removed");
    Ok(())
}
