//! Menu bar icon: pause/resume, About, Quit.

use crate::Switcher;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAlert, NSApplication, NSImage, NSMenu, NSMenuItem, NSStatusBar, NSStatusItem,
    NSVariableStatusItemLength,
};
use objc2_foundation::{NSData, NSSize, NSString};
use std::rc::Rc;

/// Monochrome template icon (macOS tints it for light/dark mode).
const MENUBAR_ICON: &[u8] = include_bytes!("../../assets/menubar.png");
/// Full-color app icon (used in the About window).
const APP_ICON: &[u8] = include_bytes!("../../assets/DonStrike.ico");

pub struct Ivars {
    switcher: Rc<Switcher>,
    status_item: Retained<NSStatusItem>,
    state_item: Retained<NSMenuItem>,
    pause_item: Retained<NSMenuItem>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "KeyNinjaMenuBar"]
    #[ivars = Ivars]
    pub struct MenuBar;

    unsafe impl NSObjectProtocol for MenuBar {}

    impl MenuBar {
        #[unsafe(method(togglePause:))]
        fn toggle_pause(&self, _sender: Option<&AnyObject>) {
            let sw = &self.ivars().switcher;
            let paused = !sw.paused.get();
            sw.paused.set(paused);
            if sw.verbose {
                eprintln!("keyninja: {}", if paused { "paused" } else { "resumed" });
            }
            self.refresh();
            if !paused {
                super::apply_frontmost(sw);
            }
        }

        #[unsafe(method(showAbout:))]
        fn show_about(&self, _sender: Option<&AnyObject>) {
            let mtm = self.mtm();
            let app = NSApplication::sharedApplication(mtm);
            let alert = NSAlert::new(mtm);
            alert.setMessageText(&NSString::from_str("KeyNinja"));
            let info = format!(
                "Version {}\n\n\
                 Automatically switches the keyboard layout \
                 when the active application changes.\n\n\
                 Config file:\n{}",
                env!("CARGO_PKG_VERSION"),
                self.ivars().switcher.config_path.display()
            );
            alert.setInformativeText(&NSString::from_str(&info));
            if let Some(icon) = image(APP_ICON) {
                unsafe { alert.setIcon(Some(&icon)) };
            }
            app.activate();
            alert.runModal();
        }

        #[unsafe(method(quit:))]
        fn quit(&self, _sender: Option<&AnyObject>) {
            NSApplication::sharedApplication(self.mtm()).terminate(None);
        }
    }
);

fn image(bytes: &[u8]) -> Option<Retained<NSImage>> {
    let data = NSData::with_bytes(bytes);
    NSImage::initWithData(NSImage::alloc(), &data)
}

fn menu_item(mtm: MainThreadMarker, title: &str, action: Option<objc2::runtime::Sel>, key: &str) -> Retained<NSMenuItem> {
    unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(title),
            action,
            &NSString::from_str(key),
        )
    }
}

impl MenuBar {
    pub fn install(mtm: MainThreadMarker, switcher: Rc<Switcher>) -> Retained<Self> {
        let status_item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
        if let Some(button) = status_item.button(mtm) {
            match image(MENUBAR_ICON) {
                Some(icon) => {
                    icon.setSize(NSSize::new(18.0, 18.0));
                    icon.setTemplate(true);
                    button.setImage(Some(&icon));
                }
                None => button.setTitle(&NSString::from_str("KN")),
            }
        }

        let state_item = menu_item(mtm, "", None, "");
        state_item.setEnabled(false);
        let pause_item = menu_item(mtm, "", Some(sel!(togglePause:)), "p");
        let about_item = menu_item(mtm, "About KeyNinja", Some(sel!(showAbout:)), "");
        let quit_item = menu_item(mtm, "Quit KeyNinja", Some(sel!(quit:)), "q");

        let this = Self::alloc(mtm).set_ivars(Ivars {
            switcher,
            status_item: status_item.clone(),
            state_item: state_item.clone(),
            pause_item: pause_item.clone(),
        });
        let this: Retained<Self> = unsafe { msg_send![super(this), init] };

        let menu = NSMenu::new(mtm);
        // Enable items manually; otherwise AppKit validates them via the responder chain.
        menu.setAutoenablesItems(false);
        for item in [&pause_item, &about_item, &quit_item] {
            unsafe { item.setTarget(Some(&this)) };
        }
        menu.addItem(&state_item);
        menu.addItem(&NSMenuItem::separatorItem(mtm));
        menu.addItem(&pause_item);
        menu.addItem(&NSMenuItem::separatorItem(mtm));
        menu.addItem(&about_item);
        menu.addItem(&quit_item);
        status_item.setMenu(Some(&menu));

        this.refresh();
        this
    }

    fn refresh(&self) {
        let iv = self.ivars();
        let paused = iv.switcher.paused.get();
        let (state, action, tip) = if paused {
            ("KeyNinja: Paused", "Resume", "KeyNinja is paused")
        } else {
            ("KeyNinja: Running", "Pause", "KeyNinja is running")
        };
        iv.state_item.setTitle(&NSString::from_str(state));
        iv.pause_item.setTitle(&NSString::from_str(action));
        if let Some(button) = iv.status_item.button(self.mtm()) {
            // Dim the icon while paused.
            button.setAppearsDisabled(paused);
            button.setToolTip(Some(&NSString::from_str(tip)));
        }
    }
}
