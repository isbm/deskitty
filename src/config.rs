use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub tabs: Vec<TabConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TabConfig {
    pub name: String,
    pub shortcut: char,
    pub command: Vec<String>,
    #[serde(default)]
    pub default: bool,
}

pub fn load(path: Option<&str>) -> Config {
    if let Some(p) = path {
        if let Ok(cfg) = load_from_path(&PathBuf::from(p)) {
            if !cfg.tabs.is_empty() {
                return cfg;
            }
        }
    }

    let home = std::env::var("HOME").unwrap_or_default();
    let default_path = PathBuf::from(home).join(".config/deskitty/config.yaml");
    if let Ok(cfg) = load_from_path(&default_path) {
        if !cfg.tabs.is_empty() {
            return cfg;
        }
    }

    default_config()
}

fn load_from_path(path: &PathBuf) -> anyhow::Result<Config> {
    let content = std::fs::read_to_string(path)?;
    Ok(serde_yaml::from_str(&content)?)
}

fn default_config() -> Config {
    Config {
        tabs: vec![TabConfig {
            name: "Shell".to_string(),
            shortcut: 's',
            command: vec!["bash".to_string()],
            default: true,
        }],
    }
}
