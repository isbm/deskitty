use serde::Deserialize;
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

/// Top-level YAML config schema.
#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub tabs: Vec<TabConfig>,
}

/// A single tab definition from the YAML config.
#[derive(Debug, Deserialize, Clone)]
pub struct TabConfig {
    pub name: String,
    pub shortcut: char,
    pub command: Vec<String>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub default: bool,
}

impl TabConfig {
    pub fn resolved_cwd(&self) -> Option<PathBuf> {
        self.cwd.as_ref().map(|path| match std::env::var_os("HOME") {
            Some(home) => expand_tilde(path, Path::new(&home)),
            None => path.clone(),
        })
    }
}

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
pub fn load(path: Option<&str>) -> Config {
    if let Some(p) = path
        && let Ok(cfg) = load_from_path(&PathBuf::from(p))
        && !cfg.tabs.is_empty()
    {
        return cfg;
    }

    let home = std::env::var("HOME").unwrap_or_default();
    let default_path = PathBuf::from(home).join(".config/deskitty/config.yaml");
    if let Ok(cfg) = load_from_path(&default_path)
        && !cfg.tabs.is_empty()
    {
        return cfg;
    }

    default_config()
}

fn load_from_path(path: &PathBuf) -> anyhow::Result<Config> {
    let content = std::fs::read_to_string(path)?;
    Ok(serde_yaml::from_str(&content)?)
}

fn default_config() -> Config {
    Config { tabs: vec![TabConfig { name: "Shell".to_string(), shortcut: 's', command: vec!["bash".to_string()], cwd: None, default: true }] }
}
