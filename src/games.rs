use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::client::{BASE, Client, MANIFEST_TTL};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Online,
    Offline,
    Released,
}

impl Category {
    pub fn title(self) -> &'static str {
        match self {
            Category::Online => "ONLINE",
            Category::Offline => "OFFLINE",
            Category::Released => "RELEASED",
        }
    }
}

pub struct Meta {
    pub key: &'static str,
    pub name: &'static str,
    pub category: Category,
    pub blurb: &'static str,
}

/// Menu order and one-line descriptions. Server details and level counts come
/// from overthewire.org/games.json so they stay current.
pub const META: &[Meta] = &[
    Meta {
        key: "bandit",
        name: "Bandit",
        category: Category::Online,
        blurb: "Unix/Linux basics",
    },
    Meta {
        key: "natas",
        name: "Natas",
        category: Category::Online,
        blurb: "Server-side web security",
    },
    Meta {
        key: "leviathan",
        name: "Leviathan",
        category: Category::Online,
        blurb: "Reverse engineering basics",
    },
    Meta {
        key: "krypton",
        name: "Krypton",
        category: Category::Online,
        blurb: "Cryptography",
    },
    Meta {
        key: "narnia",
        name: "Narnia",
        category: Category::Online,
        blurb: "Binary exploitation",
    },
    Meta {
        key: "behemoth",
        name: "Behemoth",
        category: Category::Online,
        blurb: "Binary exploitation",
    },
    Meta {
        key: "utumno",
        name: "Utumno",
        category: Category::Online,
        blurb: "Binary exploitation",
    },
    Meta {
        key: "maze",
        name: "Maze",
        category: Category::Online,
        blurb: "Binary exploitation",
    },
    Meta {
        key: "vortex",
        name: "Vortex",
        category: Category::Online,
        blurb: "Binary exploitation",
    },
    Meta {
        key: "manpage",
        name: "Manpage",
        category: Category::Online,
        blurb: "Man pages and RTFM",
    },
    Meta {
        key: "drifter",
        name: "Drifter",
        category: Category::Online,
        blurb: "Binary exploitation",
    },
    Meta {
        key: "formulaone",
        name: "FormulaOne",
        category: Category::Online,
        blurb: "Binary exploitation",
    },
    Meta {
        key: "semtex",
        name: "Semtex",
        category: Category::Offline,
        blurb: "Offline shell game",
    },
    Meta {
        key: "hes2010",
        name: "HES2010",
        category: Category::Released,
        blurb: "Conference game (2010)",
    },
    Meta {
        key: "abraxas",
        name: "Abraxas",
        category: Category::Released,
        blurb: "Conference game (2011)",
    },
    Meta {
        key: "monxla",
        name: "Monxla",
        category: Category::Released,
        blurb: "Conference game (2012)",
    },
    Meta {
        key: "kishi",
        name: "Kishi",
        category: Category::Released,
        blurb: "Conference game (2013)",
    },
];

#[derive(Clone, Debug, Deserialize)]
struct GameEntry {
    host: Option<String>,
    port: Option<u16>,
    lastlevel: u32,
    #[serde(rename = "specialNumbering", default)]
    special_numbering: bool,
}

#[derive(Clone, Debug)]
pub struct Game {
    pub key: String,
    pub name: String,
    pub category: Category,
    pub blurb: &'static str,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub last: Option<u32>,
    pub special: bool,
}

pub fn load(client: &Client) -> Result<Vec<Game>> {
    let raw = client.get("games.json", MANIFEST_TTL)?;
    let entries: BTreeMap<String, GameEntry> =
        serde_json::from_str(&raw).context("parsing games.json from overthewire.org")?;

    let mut games = Vec::with_capacity(META.len());
    for meta in META {
        let entry = entries.get(meta.key);
        games.push(Game {
            key: meta.key.to_string(),
            name: meta.name.to_string(),
            category: meta.category,
            blurb: meta.blurb,
            host: entry.and_then(|entry| entry.host.clone()),
            port: entry.and_then(|entry| entry.port),
            last: entry.map(|entry| entry.lastlevel),
            special: entry.map(|entry| entry.special_numbering).unwrap_or(false),
        });
    }
    Ok(games)
}

impl Game {
    pub fn find<'a>(games: &'a [Game], name: &str) -> Result<&'a Game> {
        let key = name.trim().to_ascii_lowercase();
        games.iter().find(|game| game.key == key).ok_or_else(|| {
            anyhow::anyhow!(
                "unknown game '{name}'\navailable games: {}",
                games
                    .iter()
                    .map(|game| game.key.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
    }

    pub fn info_path(&self) -> String {
        format!("wargames/{}/", self.key)
    }

    pub fn info_url(&self) -> String {
        format!("{BASE}/{}", self.info_path())
    }

    pub fn level_path(&self, page: u32) -> String {
        format!("wargames/{}/{}{}.html", self.key, self.key, page)
    }

    pub fn level_url(&self, page: u32) -> String {
        format!("{BASE}/{}", self.level_path(page))
    }

    /// The site labels shell level pages with the transition they describe.
    /// Natas is different: each page simply holds the credentials for level N.
    pub fn level_title(&self, page: u32) -> String {
        if self.key == "natas" {
            format!("Level {page}")
        } else if self.special {
            format!("Level {page} → Level {}", page + 1)
        } else if page == 0 {
            "Level 0".to_string()
        } else {
            format!("Level {} → Level {}", page - 1, page)
        }
    }

    /// Which account you need to be logged into in order to read this page.
    pub fn login_level(&self, page: u32) -> u32 {
        if self.special {
            if page == 0 { 1 } else { page }
        } else if page == 0 {
            0
        } else {
            page - 1
        }
    }

    pub fn ssh_argv(&self, login_level: u32) -> Option<Vec<String>> {
        let host = self.host.as_ref()?;
        let port = self.port?;
        Some(vec![
            "-p".to_string(),
            port.to_string(),
            format!("{}{}@{}", self.key, login_level, host),
        ])
    }

    pub fn ssh_command(&self, login_level: u32) -> Option<String> {
        let argv = self.ssh_argv(login_level)?;
        Some(format!("ssh {}", argv.join(" ")))
    }

    pub fn web_url(&self, level: u32) -> Option<String> {
        if self.key == "natas" {
            Some(format!("http://natas{level}.natas.labs.overthewire.org"))
        } else {
            None
        }
    }

    pub fn levels_label(&self) -> String {
        match self.last {
            Some(last) => format!("0–{last}"),
            None => "—".to_string(),
        }
    }

    pub fn where_label(&self) -> String {
        match (&self.host, self.port) {
            (Some(host), Some(port)) => format!("{host}:{port}"),
            _ if self.key == "natas" => "natasX.natas.labs.overthewire.org".to_string(),
            _ => "download only".to_string(),
        }
    }

    pub fn require_levels(&self) -> Result<u32> {
        match self.last {
            Some(last) => Ok(last),
            None => bail!(
                "{} is a released, offline game and has no level pages\nsee: over-the-wire info {}",
                self.name,
                self.key
            ),
        }
    }
}

/// Parse a level page number, accepting both `3` and `bandit3`.
pub fn parse_level(game: &Game, input: &str) -> Result<u32> {
    let raw = input.trim();
    let lower = raw.to_ascii_lowercase();
    let number = if let Ok(number) = raw.parse::<u32>() {
        Some(number)
    } else {
        [game.key.as_str(), "level", "l"].iter().find_map(|prefix| {
            lower
                .strip_prefix(prefix)
                .and_then(|rest| rest.trim_start_matches(['-', '_', ' ']).parse::<u32>().ok())
        })
    };

    let number = number.ok_or_else(|| {
        anyhow::anyhow!(
            "invalid level '{input}' for {} (expected a number, e.g. 3)",
            game.name
        )
    })?;

    let last = game.require_levels()?;
    if number > last {
        bail!("{} has levels 0–{} (got {number})", game.name, last);
    }
    Ok(number)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(key: &str, special: bool, last: u32) -> Game {
        Game {
            key: key.to_string(),
            name: key.to_string(),
            category: Category::Online,
            blurb: "",
            host: Some("example.org".to_string()),
            port: Some(1234),
            last: Some(last),
            special,
        }
    }

    #[test]
    fn shell_titles_describe_transitions() {
        let bandit = game("bandit", false, 34);
        assert_eq!(bandit.level_title(0), "Level 0");
        assert_eq!(bandit.level_title(1), "Level 0 → Level 1");
        assert_eq!(bandit.level_title(34), "Level 33 → Level 34");
    }

    #[test]
    fn special_titles_describe_transitions() {
        let krypton = game("krypton", true, 6);
        assert_eq!(krypton.level_title(0), "Level 0 → Level 1");
        assert_eq!(krypton.level_title(6), "Level 6 → Level 7");
    }

    #[test]
    fn natas_pages_are_plain_levels() {
        let natas = game("natas", false, 34);
        assert_eq!(natas.level_title(0), "Level 0");
        assert_eq!(natas.level_title(12), "Level 12");
    }

    #[test]
    fn login_level_matches_page() {
        let bandit = game("bandit", false, 34);
        assert_eq!(bandit.login_level(0), 0);
        assert_eq!(bandit.login_level(4), 3);
        assert!(bandit.ssh_command(3).unwrap().contains("bandit3@"));

        let krypton = game("krypton", true, 6);
        assert_eq!(krypton.login_level(0), 1);
        assert_eq!(krypton.login_level(2), 2);
    }

    #[test]
    fn parses_common_level_forms() {
        let bandit = game("bandit", false, 34);
        for input in ["3", "bandit3", "level3", "level 3", "l3"] {
            assert_eq!(parse_level(&bandit, input).unwrap(), 3, "{input}");
        }
        assert!(parse_level(&bandit, "35").is_err());
        assert!(parse_level(&bandit, "nope").is_err());
    }
}
