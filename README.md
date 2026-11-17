# KeyNinja

KeyNinja switches your keyboard layout automatically when you switch between applications.

By default it turns on **English** whenever you switch to any app, and **Russian** when you switch to **Telegram** or **Mattermost**. Both the rules and the layouts can be changed in a small config file.

| Platform      | Status                                                |
|---------------|-------------------------------------------------------|
| macOS 11+     | ✅ Supported, with a menu bar icon                    |
| Linux (X11)   | ✅ Supported, runs from the terminal (no tray icon yet) |
| Linux (Wayland) | 🚧 Not supported yet (planned)                      |

## Features

- Switches to a default layout (English) whenever the active app changes.
- Per-app rules, for example Russian in Telegram and Mattermost, or any other language in any app.
- An ignore list for apps where the layout should be left alone (Spotlight, Raycast, Alfred, …).
- macOS menu bar icon with **Pause / Resume**, **About** and **Quit**.
- No special permissions needed on macOS: no Accessibility or Input Monitoring access.
- One small native binary written in Rust.

## Installation

### macOS

Requires [Rust](https://rustup.rs).

```sh
git clone https://github.com/<your-username>/KeyNinja.git
cd KeyNinja
./scripts/bundle-macos.sh          # builds target/KeyNinja.app
cp -R target/KeyNinja.app /Applications/
open /Applications/KeyNinja.app
```

The app shows up only in the menu bar, not in the Dock.

If you downloaded a prebuilt `KeyNinja.app` from the Releases page, macOS may block it because it is not notarized. Either right-click the app and choose **Open**, or run:

```sh
xattr -dr com.apple.quarantine /Applications/KeyNinja.app
```

### Linux (X11)

```sh
cargo build --release
./target/release/keyninja
```

## Usage

```
keyninja [run] [-c <config>] [-v]   run (default)
keyninja list                        list available keyboard layouts
keyninja config                      print the config path (creates it if missing)
keyninja install [-c <config>]       start automatically at login
keyninja uninstall                   remove autostart
```

Options:

- `-c, --config <path>`: use a different config file (default: `~/.config/keyninja/config.toml`)
- `-v, --verbose`: log every app switch and layout change to stderr

### Start at login

- **macOS:** add `KeyNinja.app` under *System Settings → General → Login Items*, or run `keyninja install`. The latter installs a LaunchAgent (`~/Library/LaunchAgents/com.keyninja.agent.plist`) and logs to `/tmp/keyninja.log`.
- **Linux:** `keyninja install` creates `~/.config/autostart/keyninja.desktop`.

## Configuration

The config file is created on first run at `~/.config/keyninja/config.toml` (or under `$XDG_CONFIG_HOME/keyninja/`):

```toml
# Layout used for every app that does not match a rule.
default = "en"

# Delay before switching, in ms. Increase to 50–150 if an app overrides the layout itself.
delay_ms = 0

# Apps where KeyNinja does nothing.
ignore = ["com.apple.Spotlight", "Raycast"]

[[rule]]
apps = ["telegram", "mattermost"]
layout = "ru"

[[rule]]
apps = ["com.example.GermanApp"]
layout = "de"
```

Restart KeyNinja after editing the config.

### Matching apps

Each entry in `apps` is matched **case-insensitively as a substring** against:

- **macOS:** the bundle identifier (`ru.keepcoder.Telegram`) and the app name (`Telegram`)
- **Linux:** the window's `WM_CLASS` (`telegram-desktop`, `TelegramDesktop`)

Rules are checked from top to bottom and the first match wins. Run `keyninja -v` to see the identifiers of each app you switch to.

### Specifying a layout

Run `keyninja list` to see the layouts you can use.

- **macOS:** a language code (`en`, `ru`, `de`), a layout name (`ABC`, `Russian - PC`) or a full input source ID (`com.apple.keylayout.Russian`). The layout must be enabled in *System Settings → Keyboard → Input Sources*.
- **Linux:** a layout or language code (`us`, `en`, `ru`, `de`) or an XKB group number (`0`–`3`). The layout must be configured in your X11 session (for example `setxkbmap us,ru`).

## Troubleshooting

- **The layout switches but then changes back.** Turn off *System Settings → Keyboard → Input Sources → "Automatically switch to a document's input source"*, or set `delay_ms` to around `100`.
- **A warning that a layout was not found.** Check the value against `keyninja list`.
- **Linux: KeyNinja exits saying Wayland is not supported.** Log in to an X11 (Xorg) session for now.

## How it works

- **macOS:** listens for `NSWorkspaceDidActivateApplicationNotification` and switches the input source with the Text Input Sources API (`TISSelectInputSource`).
- **Linux (X11):** watches the `_NET_ACTIVE_WINDOW` property on the root window and locks the XKB group.

## Building from source

```sh
cargo build --release      # binary: target/release/keyninja
cargo test
```

## License

MIT. See [LICENSE](LICENSE).
