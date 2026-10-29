use anyhow::Context;
use serde::Deserialize;
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use crate::events::EventsConfig;

/// Top-level YAML config schema.
#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub tabs: Vec<TabConfig>,
    /// Screen-triggered automation events.
    #[serde(default)]
    pub events: EventsConfig,
    /// Scan/debug mode: evaluate and log events without sending keystrokes.
    #[serde(default)]
    pub debug: bool,
    /// Show the colored frame and status bar.
    #[serde(default = "default_show_desktop")]
    pub show_desktop: bool,
}

fn default_show_desktop() -> bool {
    true
}

/// A single tab definition from the YAML config.
#[derive(Debug, Deserialize, Clone)]
pub struct TabConfig {
    pub name: String,
    pub shortcut: char,
    pub command: Vec<String>,
    /// Working directory for the tab's program.
    ///
    /// Relative paths resolve from the directory deskitty was launched in,
    /// and a leading `~` expands to `$HOME`. When omitted, the launch
    /// directory is used.
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub default: bool,
}

impl TabConfig {
    /// Resolve the configured working directory, expanding a leading `~`.
    ///
    /// Returns `None` when the tab should use deskitty's launch directory.
    pub fn resolved_cwd(&self) -> Option<PathBuf> {
        self.cwd.as_ref().map(|path| match std::env::var_os("HOME") {
            Some(home) => expand_tilde(path, Path::new(&home)),
            None => path.clone(),
        })
    }
}

/// Expand a leading `~` component of `path` using `home`.
///
/// Paths without a leading `~` are returned unchanged. A leading `~user`
/// component is not expanded.
pub(crate) fn expand_tilde(path: &Path, home: &Path) -> PathBuf {
    let mut components = path.components();
    let Some(Component::Normal(first)) = components.next() else {
        return path.to_path_buf();
    };
    if first != OsStr::new("~") {
        return path.to_path_buf();
    }

    let rest = components.as_path();
    if rest.as_os_str().is_empty() { home.to_path_buf() } else { home.join(rest) }
}

/// Load config from the given path, falling back to
/// `~/.config/deskitty/config.yaml`, and finally to a single `bash` tab.
///
/// A malformed config is reported as an error instead of being silently
/// replaced by the default tab, so event and keystroke typos are visible.
pub fn load(path: Option<&str>) -> anyhow::Result<Config> {
    if let Some(path) = path {
        return load_with_default_tab(Path::new(path));
    }

    let home = std::env::var("HOME").unwrap_or_default();
    let default_path = PathBuf::from(home).join(".config/deskitty/config.yaml");
    if default_path.is_file() {
        return load_with_default_tab(&default_path);
    }

    Ok(default_config())
}

fn load_with_default_tab(path: &Path) -> anyhow::Result<Config> {
    let mut config = load_from_path(path)?;
    if config.tabs.is_empty() {
        config.tabs = default_config().tabs;
    }
    Ok(config)
}

fn load_from_path(path: &Path) -> anyhow::Result<Config> {
    let content = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let config: Config = serde_yaml::from_str(&content).with_context(|| format!("parsing {}", path.display()))?;
    config.events.validate().with_context(|| format!("validating {}", path.display()))?;
    Ok(config)
}

fn default_config() -> Config {
    Config {
        tabs: vec![TabConfig { name: "Shell".to_string(), shortcut: 's', command: vec!["bash".to_string()], cwd: None, default: true }],
        events: EventsConfig::default(),
        debug: false,
        show_desktop: true,
    }
}
