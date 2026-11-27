use std::collections::BTreeMap;
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

#[derive(Debug, Default, Deserialize)]
pub struct Credentials {
    #[serde(flatten)]
    games: BTreeMap<String, BTreeMap<String, String>>,
}

impl Credentials {
    pub fn password(&self, game: &str, level: u32) -> Option<&str> {
        self.games
            .get(game)
            .and_then(|levels| levels.get(&level.to_string()))
            .map(String::as_str)
    }
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

pub fn load_credentials() -> Result<Credentials> {
    let path = credentials_path();
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Credentials::default());
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

pub fn credentials_path() -> PathBuf {
    config_home()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("otw")
        .join("credentials.toml")
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
        assert!(credentials_path().ends_with("otw/credentials.toml"));
    }

    #[test]
    fn reads_passwords_by_game_and_level() {
        let credentials: Credentials =
            toml::from_str("[bandit]\n0 = \"one\"\n1 = \"two\"\n").unwrap();
        assert_eq!(credentials.password("bandit", 0), Some("one"));
        assert_eq!(credentials.password("bandit", 1), Some("two"));
        assert_eq!(credentials.password("bandit", 2), None);
        assert_eq!(credentials.password("natas", 0), None);
    }
}
