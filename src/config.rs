use anyhow::Context;
use serde::Deserialize;
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::events::EventsConfig;

/// Top-level YAML config schema.
#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub tabs: Vec<TabConfig>,
    /// Screen-triggered automation events.
    #[serde(default)]
    pub events: EventsConfig,
    /// Execute automatic event actions (keystrokes and sounds).
    #[serde(default = "default_actions")]
    pub actions: bool,
    /// Show the event trace console.
    #[serde(default)]
    pub debug: bool,
    /// Show the colored frame and status bar.
    #[serde(default = "default_show_desktop")]
    pub show_desktop: bool,
}

fn default_actions() -> bool {
    true
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

/// Load an explicit config or the config selected by the invocation name.
///
/// `deskitty` uses `~/.config/deskitty/default.conf`; other invocation names
/// use `~/.config/deskitty/<name>.conf`. Missing automatic configs start a
/// single `bash` tab. Explicit paths and malformed configs report errors.
pub fn load(path: Option<&str>, invocation: Option<&OsStr>) -> anyhow::Result<Config> {
    let config_path = active_config_path(path, invocation);
    if path.is_some() || config_path.is_file() {
        return load_with_default_tab(&config_path);
    }

    Ok(default_config())
}

/// Updates the desktop frame visibility in the selected configuration file.
///
/// Creates the configuration with the default settings when it does not yet
/// exist. The replacement is written to a sibling temporary file before it is
/// atomically renamed into place.
pub fn set_show_desktop(path: &Path, visible: bool) -> anyhow::Result<()> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error).with_context(|| format!("reading {}", path.display())),
    };
    let updated = update_show_desktop(&content, visible);
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;

    let temporary = temporary_path(path);
    std::fs::write(&temporary, updated).with_context(|| format!("writing {}", temporary.display()))?;
    std::fs::rename(&temporary, path).with_context(|| format!("replacing {}", path.display()))
}

fn update_show_desktop(content: &str, visible: bool) -> String {
    let replacement = format!("show_desktop: {visible}");
    let mut replaced = false;
    let updated = content
        .lines()
        .map(|line| {
            if !replaced && line.trim_start().starts_with("show_desktop:") {
                replaced = true;
                replacement.clone()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>();

    if replaced {
        return format!("{}\n", updated.join("\n"));
    }

    let separator = if content.trim().is_empty() { "" } else { "\n" };
    format!("{}{separator}{replacement}\n", content.trim_end())
}

fn temporary_path(path: &Path) -> PathBuf {
    static NEXT_TEMPORARY_FILE: AtomicUsize = AtomicUsize::new(0);

    let suffix = NEXT_TEMPORARY_FILE.fetch_add(1, Ordering::Relaxed);
    let filename = path.file_name().unwrap_or_else(|| OsStr::new("config"));
    path.with_file_name(format!(".{}.{}.tmp", filename.to_string_lossy(), suffix))
}

/// Returns the config file selected by this invocation.
pub fn active_config_path(path: Option<&str>, invocation: Option<&OsStr>) -> PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_default();
    selected_config_path(path, invocation, Path::new(&home))
}

/// Choose a config using the original invocation, without resolving symlinks.
pub(crate) fn selected_config_path(path: Option<&str>, invocation: Option<&OsStr>, home: &Path) -> PathBuf {
    if let Some(path) = path {
        return PathBuf::from(path);
    }

    let config_dir = home.join(".config/deskitty");
    match invocation.and_then(|name| Path::new(name).file_name()) {
        Some(name) if name != OsStr::new("deskitty") => {
            let mut filename = name.to_os_string();
            filename.push(".conf");
            config_dir.join(filename)
        }
        _ => config_dir.join("default.conf"),
    }
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
        actions: true,
        debug: false,
        show_desktop: true,
    }
}
