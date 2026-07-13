use serde::Deserialize;
use std::path::PathBuf;

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
    pub default: bool,
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
    Config { tabs: vec![TabConfig { name: "Shell".to_string(), shortcut: 's', command: vec!["bash".to_string()], default: true }] }
}
