//! macOS: NSWorkspace app activation notifications + Text Input Sources (Carbon).

mod menubar;

use crate::Switcher;
use block2::RcBlock;
use core_foundation::base::TCFType;
use core_foundation::string::{CFString, CFStringRef};
use core_foundation_sys::array::{CFArrayGetCount, CFArrayGetValueAtIndex, CFArrayRef};
use core_foundation_sys::base::{CFEqual, CFGetTypeID, CFRelease, CFTypeRef};
use core_foundation_sys::dictionary::CFDictionaryRef;
use core_foundation_sys::number::{CFBooleanGetTypeID, CFBooleanGetValue, CFBooleanRef};
use core_foundation_sys::string::CFStringGetTypeID;
use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSRunningApplication, NSWorkspace,
    NSWorkspaceApplicationKey, NSWorkspaceDidActivateApplicationNotification,
};
use objc2_foundation::{NSNotification, NSOperationQueue};
use std::ffi::c_void;
use std::path::Path;
use std::ptr::NonNull;
use std::rc::Rc;

type TISInputSourceRef = *const c_void;

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn TISCreateInputSourceList(properties: CFDictionaryRef, include_all_installed: u8) -> CFArrayRef;
    fn TISCopyCurrentKeyboardInputSource() -> TISInputSourceRef;
    fn TISGetInputSourceProperty(source: TISInputSourceRef, key: CFStringRef) -> *const c_void;
    fn TISSelectInputSource(source: TISInputSourceRef) -> i32;

    static kTISPropertyInputSourceID: CFStringRef;
    static kTISPropertyLocalizedName: CFStringRef;
    static kTISPropertyInputSourceLanguages: CFStringRef;
    static kTISPropertyInputSourceCategory: CFStringRef;
    static kTISPropertyInputSourceIsSelectCapable: CFStringRef;
    static kTISCategoryKeyboardInputSource: CFStringRef;
}

struct Source {
    raw: TISInputSourceRef,
    id: String,
    name: String,
    langs: Vec<String>,
}

fn prop_string(src: TISInputSourceRef, key: CFStringRef) -> Option<String> {
    unsafe {
        let v = TISGetInputSourceProperty(src, key);
        if v.is_null() || CFGetTypeID(v) != CFStringGetTypeID() {
            return None;
        }
        Some(CFString::wrap_under_get_rule(v as CFStringRef).to_string())
    }
}

/// Keyboard input sources enabled by the user.
/// The sources stay valid while `list` is alive (released in Drop).
struct SourceList {
    list: CFArrayRef,
    items: Vec<Source>,
}

impl SourceList {
    fn enabled() -> Self {
        let mut items = Vec::new();
        unsafe {
            let list = TISCreateInputSourceList(std::ptr::null(), 0);
            if list.is_null() {
                return Self { list, items };
            }
            for i in 0..CFArrayGetCount(list) {
                let src = CFArrayGetValueAtIndex(list, i) as TISInputSourceRef;
                let cat = TISGetInputSourceProperty(src, kTISPropertyInputSourceCategory);
                if cat.is_null() || CFEqual(cat, kTISCategoryKeyboardInputSource as CFTypeRef) == 0 {
                    continue;
                }
                let sel = TISGetInputSourceProperty(src, kTISPropertyInputSourceIsSelectCapable);
                if sel.is_null()
                    || CFGetTypeID(sel) != CFBooleanGetTypeID()
                    || !CFBooleanGetValue(sel as CFBooleanRef)
                {
                    continue;
                }
                let langs_ref = TISGetInputSourceProperty(src, kTISPropertyInputSourceLanguages) as CFArrayRef;
                let mut langs = Vec::new();
                if !langs_ref.is_null() {
                    for j in 0..CFArrayGetCount(langs_ref) {
                        let s = CFArrayGetValueAtIndex(langs_ref, j);
                        if CFGetTypeID(s) == CFStringGetTypeID() {
                            langs.push(CFString::wrap_under_get_rule(s as CFStringRef).to_string());
                        }
                    }
                }
                items.push(Source {
                    raw: src,
                    id: prop_string(src, kTISPropertyInputSourceID).unwrap_or_default(),
                    name: prop_string(src, kTISPropertyLocalizedName).unwrap_or_default(),
                    langs,
                });
            }
            Self { list, items }
        }
    }

    /// Lookup order: exact ID -> name -> primary language -> any language.
    fn find(&self, spec: &str) -> Option<&Source> {
        let eq = |a: &str| a.eq_ignore_ascii_case(spec);
        self.items
            .iter()
            .find(|s| eq(&s.id))
            .or_else(|| self.items.iter().find(|s| eq(&s.name)))
            .or_else(|| self.items.iter().find(|s| s.langs.first().is_some_and(|l| eq(l))))
            .or_else(|| self.items.iter().find(|s| s.langs.iter().any(|l| eq(l))))
    }
}

impl Drop for SourceList {
    fn drop(&mut self) {
        if !self.list.is_null() {
            unsafe { CFRelease(self.list as CFTypeRef) };
        }
    }
}

fn current_source_id() -> Option<String> {
    unsafe {
        let cur = TISCopyCurrentKeyboardInputSource();
        if cur.is_null() {
            return None;
        }
        let id = prop_string(cur, kTISPropertyInputSourceID);
        CFRelease(cur as CFTypeRef);
        id
    }
}

/// Selects a layout. Ok(true) if switched, Ok(false) if it was already active.
fn select(spec: &str) -> Result<bool, String> {
    let sources = SourceList::enabled();
    let src = sources.find(spec).ok_or_else(|| {
        "layout not found among enabled input sources (see `keyninja list`)".to_string()
    })?;
    if current_source_id().as_deref() == Some(src.id.as_str()) {
        return Ok(false);
    }
    let status = unsafe { TISSelectInputSource(src.raw) };
    if status != 0 {
        return Err(format!("TISSelectInputSource returned {status}"));
    }
    Ok(true)
}

fn app_ids(app: &NSRunningApplication) -> (String, String) {
    let bundle = app.bundleIdentifier().map(|s| s.to_string()).unwrap_or_default();
    let name = app.localizedName().map(|s| s.to_string()).unwrap_or_default();
    (bundle, name)
}

fn handle(switcher: &Switcher, app: &NSRunningApplication) {
    // Our own windows (e.g. About) must not change the layout.
    if app.processIdentifier() as u32 == std::process::id() {
        return;
    }
    let (bundle, name) = app_ids(app);
    switcher.on_activate(&[&bundle, &name], select);
}

pub fn run(switcher: Switcher) -> Result<(), String> {
    // Validate configured layouts up front so typos show up immediately.
    let sources = SourceList::enabled();
    let specs = switcher.config.default.iter().chain(switcher.config.rules.iter().map(|r| &r.layout));
    for spec in specs {
        match sources.find(spec) {
            Some(s) if switcher.verbose => eprintln!("keyninja: '{spec}' -> {} ({})", s.name, s.id),
            Some(_) => {}
            None => eprintln!("keyninja: warning: layout '{spec}' not found among enabled input sources (see `keyninja list`)"),
        }
    }
    drop(sources);

    let mtm = MainThreadMarker::new().ok_or("must run on the main thread")?;
    let app = NSApplication::sharedApplication(mtm);
    // Menu bar icon only: no Dock icon and no main menu.
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let switcher = Rc::new(switcher);
    let _menu = menubar::MenuBar::install(mtm, switcher.clone());

    apply_frontmost(&switcher);

    let workspace = NSWorkspace::sharedWorkspace();
    let observer_switcher = switcher.clone();
    let block = RcBlock::new(move |note: NonNull<NSNotification>| {
        let switcher = &observer_switcher;
        let note = unsafe { note.as_ref() };
        let app = note
            .userInfo()
            .and_then(|info| unsafe { info.objectForKey(NSWorkspaceApplicationKey) })
            .and_then(|obj| obj.downcast::<NSRunningApplication>().ok());
        if let Some(app) = app {
            handle(switcher, &app);
        }
    });

    let center = workspace.notificationCenter();
    let _observer = unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidActivateApplicationNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &block,
        )
    };
    eprintln!("keyninja: running, see the menu bar icon (Ctrl+C to quit)");
    app.run();
    Ok(())
}

/// Applies the matching rule to the current frontmost app.
fn apply_frontmost(switcher: &Switcher) {
    if let Some(app) = NSWorkspace::sharedWorkspace().frontmostApplication() {
        handle(switcher, &app);
    }
}

pub fn list() -> Result<(), String> {
    let current = current_source_id().unwrap_or_default();
    let sources = SourceList::enabled();
    if sources.items.is_empty() {
        return Err("no enabled keyboard layouts found".into());
    }
    println!("  {:<24} {:<6} ID", "Name", "Lang");
    for s in &sources.items {
        let mark = if s.id == current { '*' } else { ' ' };
        let lang = s.langs.first().map(String::as_str).unwrap_or("-");
        println!("{mark} {:<24} {lang:<6} {}", s.name, s.id);
    }
    println!("\n* = current. In the config, use the name, language or ID.");
    Ok(())
}

const AGENT_LABEL: &str = "com.keyninja.agent";

fn agent_plist_path() -> std::path::PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_default();
    Path::new(&home).join("Library/LaunchAgents").join(format!("{AGENT_LABEL}.plist"))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn launchctl(args: &[&str]) -> Result<(), String> {
    let st = std::process::Command::new("launchctl")
        .args(args)
        .status()
        .map_err(|e| format!("launchctl: {e}"))?;
    if st.success() { Ok(()) } else { Err(format!("launchctl {} failed with {st}", args.join(" "))) }
}

fn gui_domain() -> String {
    let uid = std::process::Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "501".into());
    format!("gui/{uid}")
}

/// LaunchAgent definition. launchd restarts KeyNinja only after a crash
/// (non-zero exit); "Quit KeyNinja" exits with 0 and stays quit.
fn agent_plist(exe: &Path, config: &Path) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>{AGENT_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>run</string>
        <string>-c</string>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key><false/>
    </dict>
    <key>ProcessType</key><string>Interactive</string>
    <key>StandardErrorPath</key><string>/tmp/keyninja.log</string>
</dict>
</plist>
"#,
        xml_escape(&exe.to_string_lossy()),
        xml_escape(&config.to_string_lossy())
    )
}

pub fn install(config: &Path) -> Result<(), String> {
    crate::config::load_or_create(config)?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let plist = agent_plist(&exe, config);
    let path = agent_plist_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let domain = gui_domain();
    let _ = launchctl(&["bootout", &format!("{domain}/{AGENT_LABEL}")]);
    std::fs::write(&path, plist).map_err(|e| format!("{}: {e}", path.display()))?;
    launchctl(&["bootstrap", &domain, &path.to_string_lossy()])?;
    println!("Autostart installed: {}\nLog: /tmp/keyninja.log", path.display());
    Ok(())
}

pub fn uninstall() -> Result<(), String> {
    let path = agent_plist_path();
    let _ = launchctl(&["bootout", &format!("{}/{AGENT_LABEL}", gui_domain())]);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    println!("Autostart removed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::process::{Command, Stdio};

    #[test]
    fn agent_plist_restarts_only_after_crash() {
        let plist = agent_plist(Path::new("/Apps/Key & Ninja/keyninja"), Path::new("/tmp/<cfg>.toml"));
        let mut child = Command::new("plutil")
            .args(["-extract", "KeepAlive.SuccessfulExit", "raw", "-o", "-", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(plist.as_bytes()).unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success(), "invalid plist:\n{plist}");
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "false");
    }
}
