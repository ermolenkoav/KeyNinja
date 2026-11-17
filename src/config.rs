use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const DEFAULT_CONFIG: &str = r#"# KeyNinja configuration
#
# A layout can be specified as:
#   macOS: a language code ("en", "ru", "de"...), a layout name ("ABC", "Russian - PC")
#          or a full input source ID ("com.apple.keylayout.Russian").
#          Run `keyninja list` to see what is available.
#   Linux (X11): a layout or language code ("en"/"us", "ru", "de"...) or an XKB group number ("0", "1").

# Layout used for every app that does not match a rule.
default = "en"

# Delay before switching, in ms (increase to 50-150 if an app overrides the layout itself).
delay_ms = 0

# Apps where KeyNinja leaves the layout alone.
# ignore = ["com.apple.Spotlight", "Raycast", "Alfred"]
ignore = []

# Rules. Each entry in `apps` is matched case-insensitively as a substring against
# the bundle id and app name (macOS) or the window's WM_CLASS (Linux).
# The first matching rule wins.
[[rule]]
apps = ["telegram", "mattermost"]
layout = "ru"

# Another language, for example:
# [[rule]]
# apps = ["com.example.GermanApp"]
# layout = "de"
"#;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub default: Option<String>,
    #[serde(default)]
    pub delay_ms: u64,
    #[serde(default)]
    pub ignore: Vec<String>,
    #[serde(default, rename = "rule")]
    pub rules: Vec<Rule>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub apps: Vec<String>,
    pub layout: String,
}

/// What to do when an application is activated.
#[derive(Debug, PartialEq)]
pub enum Action<'a> {
    Switch(&'a str),
    Nothing,
}

impl Config {
    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    /// `ids` are all identifiers of the app (bundle id, name, WM_CLASS...).
    pub fn action_for(&self, ids: &[&str]) -> Action<'_> {
        let ids: Vec<String> = ids.iter().map(|s| s.to_lowercase()).collect();
        let hit = |pattern: &String| {
            let p = pattern.to_lowercase();
            !p.is_empty() && ids.iter().any(|id| id.contains(&p))
        };
        if self.ignore.iter().any(hit) {
            return Action::Nothing;
        }
        for rule in &self.rules {
            if rule.apps.iter().any(hit) {
                return Action::Switch(&rule.layout);
            }
        }
        match &self.default {
            Some(d) => Action::Switch(d),
            None => Action::Nothing,
        }
    }
}

pub fn default_path() -> PathBuf {
    if let Some(x) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(x).join("keyninja/config.toml");
    }
    let home = std::env::var_os("HOME").unwrap_or_default();
    PathBuf::from(home).join(".config/keyninja/config.toml")
}

/// Loads the config, creating it with default contents if it does not exist.
pub fn load_or_create(path: &Path) -> Result<Config, String> {
    if !path.exists() {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::write(path, DEFAULT_CONFIG).map_err(|e| format!("{}: {e}", path.display()))?;
        eprintln!("keyninja: created config {}", path.display());
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Config::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_rules() {
        let c = Config::parse(DEFAULT_CONFIG).unwrap();
        assert_eq!(c.action_for(&["ru.keepcoder.Telegram", "Telegram"]), Action::Switch("ru"));
        assert_eq!(c.action_for(&["telegram-desktop", "TelegramDesktop"]), Action::Switch("ru"));
        assert_eq!(c.action_for(&["Mattermost.Desktop", "Mattermost"]), Action::Switch("ru"));
        assert_eq!(c.action_for(&["com.apple.Safari", "Safari"]), Action::Switch("en"));
    }

    #[test]
    fn ignore_wins() {
        let c = Config::parse("default = \"en\"\nignore = [\"raycast\"]").unwrap();
        assert_eq!(c.action_for(&["com.raycast.macos", "Raycast"]), Action::Nothing);
    }
}
