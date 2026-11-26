use std::env;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub width: Option<usize>,
}

pub fn load() -> Result<Config> {
    let path = config_path();
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Config::default());
        }
        Err(error) => {
            return Err(error).with_context(|| format!("could not read {}", path.display()));
        }
    };
    toml::from_str(&text).with_context(|| format!("could not parse {}", path.display()))
}

pub fn config_path() -> PathBuf {
    config_home()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("otw")
        .join("config.toml")
}

fn config_home() -> Option<PathBuf> {
    if let Some(path) = env::var_os("XDG_CONFIG_HOME")
        && !path.is_empty()
    {
        return Some(PathBuf::from(path));
    }
    #[cfg(windows)]
    if let Some(path) = env::var_os("APPDATA")
        && !path.is_empty()
    {
        return Some(PathBuf::from(path));
    }
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .map(|home| home.join(".config"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_width_and_rejects_unknown_settings() {
        let config: Config = toml::from_str("width = 100").unwrap();
        assert_eq!(config.width, Some(100));
        assert!(toml::from_str::<Config>("widht = 100").is_err());
    }

    #[test]
    fn locates_the_standard_config_path() {
        assert!(config_path().ends_with("otw/config.toml"));
    }
}
