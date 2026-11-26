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

/// The remote manifest controls these values, so keep a corrupt or compromised
/// entry from making a command loop over billions of pages.
const MAX_LEVEL: u32 = 500;

fn valid_host(host: &str) -> Option<String> {
    let host = host.trim();
    if host.is_empty() {
        return None;
    }
    let valid = if host.contains(':') {
        host.parse::<std::net::Ipv6Addr>().is_ok()
    } else {
        host.len() <= 253
            && host.split('.').all(|label| {
                !label.is_empty()
                    && label.len() <= 63
                    && !label.starts_with('-')
                    && !label.ends_with('-')
                    && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            })
    };
    valid.then(|| host.to_string())
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
            host: entry
                .and_then(|entry| entry.host.as_deref())
                .and_then(valid_host),
            port: entry.and_then(|entry| entry.port),
            last: entry.map(|entry| entry.lastlevel.min(MAX_LEVEL)),
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

    /// The account this page is meant to be read as, if there is one.
    pub fn read_as(&self, page: u32) -> Option<String> {
        if self.key == "natas" {
            Some(format!("natas{page}"))
        } else if self.special {
            (page > 0).then(|| format!("{}{}", self.key, page))
        } else {
            Some(format!("{}{}", self.key, self.login_level(page)))
        }
    }

    /// The page that describes the task for a level you are logged in as.
    pub fn task_page(&self, login_level: u32) -> Result<u32> {
        let last = self.require_levels()?;
        if self.key == "natas" || self.special {
            return Ok(login_level);
        }
        if login_level >= last {
            bail!(
                "{} has no task after level {last}; the game ends there",
                self.name
            );
        }
        Ok(login_level + 1)
    }

    /// The level whose task is described by a page, if it has one.
    pub fn task_level(&self, page: u32) -> Option<u32> {
        if self.key == "natas" || self.special {
            Some(page)
        } else if page == 0 {
            None
        } else {
            Some(page - 1)
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

    /// The levels that have a task: `0..=last` for web and special-numbering
    /// games, `0..=(last - 1)` for normal shell games (the final login has no
    /// next task).
    pub fn level_range(&self) -> Result<std::ops::RangeInclusive<u32>> {
        let last = self.require_levels()?;
        if self.key == "natas" || self.special {
            Ok(0..=last)
        } else if last == 0 {
            bail!("{} has no task pages", self.name)
        } else {
            Ok(0..=(last - 1))
        }
    }

    pub fn levels_label(&self) -> String {
        match self.level_range() {
            Ok(range) => format!("{}-{}", range.start(), range.end()),
            Err(_) => "-".to_string(),
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
                "{} is a released, offline game and has no level pages\nsee: otw info {}",
                self.name,
                self.key
            ),
        }
    }
}

fn parse_level_number(game: &Game, input: &str) -> Result<u32> {
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

    number.ok_or_else(|| {
        anyhow::anyhow!(
            "invalid level '{input}' for {} (expected a number, e.g. 3)",
            game.name
        )
    })
}

/// Parse a login level, accepting both `3` and `bandit3`.
pub fn parse_level(game: &Game, input: &str) -> Result<u32> {
    let number = parse_level_number(game, input)?;
    let last = game.require_levels()?;
    if number > last {
        bail!("{} has levels 0-{} (got {number})", game.name, last);
    }
    Ok(number)
}

/// Parse a level that has a task (`otw level`).
pub fn parse_task_level(game: &Game, input: &str) -> Result<u32> {
    let number = parse_level_number(game, input)?;
    let range = game.level_range()?;
    if !range.contains(&number) {
        bail!(
            "{} has levels {}-{} (got {number})",
            game.name,
            range.start(),
            range.end()
        );
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
    fn only_accepts_plain_hostnames_from_the_manifest() {
        assert_eq!(
            valid_host("bandit.overthewire.org").as_deref(),
            Some("bandit.overthewire.org")
        );
        assert_eq!(valid_host("127.0.0.1").as_deref(), Some("127.0.0.1"));
        assert!(valid_host("::1").is_some());
        assert_eq!(valid_host("evil; rm -rf ~"), None);
        assert_eq!(valid_host("host $(id)"), None);
        assert_eq!(valid_host("-bad.example"), None);
        assert_eq!(valid_host(""), None);
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

    #[test]
    fn task_page_uses_the_current_login_level() {
        let bandit = game("bandit", false, 34);
        assert_eq!(bandit.task_page(0).unwrap(), 1);
        assert_eq!(bandit.task_page(3).unwrap(), 4);
        assert!(bandit.task_page(34).is_err());

        let krypton = game("krypton", true, 6);
        assert_eq!(krypton.task_page(0).unwrap(), 0);
        assert_eq!(krypton.task_page(3).unwrap(), 3);

        let natas = game("natas", false, 34);
        assert_eq!(natas.task_page(3).unwrap(), 3);
    }

    #[test]
    fn read_as_labels_pages() {
        let bandit = game("bandit", false, 34);
        assert_eq!(bandit.read_as(0).as_deref(), Some("bandit0"));
        assert_eq!(bandit.read_as(4).as_deref(), Some("bandit3"));

        let krypton = game("krypton", true, 6);
        assert_eq!(krypton.read_as(0), None);
        assert_eq!(krypton.read_as(3).as_deref(), Some("krypton3"));

        let natas = game("natas", false, 34);
        assert_eq!(natas.read_as(3).as_deref(), Some("natas3"));
    }

    #[test]
    fn task_level_is_the_inverse_of_task_page() {
        let bandit = game("bandit", false, 34);
        assert_eq!(bandit.task_level(0), None);
        assert_eq!(bandit.task_level(4), Some(3));

        let krypton = game("krypton", true, 6);
        assert_eq!(krypton.task_level(0), Some(0));
        assert_eq!(krypton.task_level(3), Some(3));

        let natas = game("natas", false, 34);
        assert_eq!(natas.task_level(3), Some(3));
    }

    #[test]
    fn level_ranges_cover_tasks_only() {
        let bandit = game("bandit", false, 34);
        assert_eq!(bandit.level_range().unwrap(), 0..=33);
        assert_eq!(bandit.levels_label(), "0-33");

        let krypton = game("krypton", true, 6);
        assert_eq!(krypton.level_range().unwrap(), 0..=6);
        assert_eq!(krypton.levels_label(), "0-6");

        let natas = game("natas", false, 34);
        assert_eq!(natas.level_range().unwrap(), 0..=34);
        assert_eq!(natas.levels_label(), "0-34");

        let empty = game("bandit", false, 0);
        assert!(empty.level_range().is_err());
        assert!(empty.task_page(0).is_err());
        assert_eq!(empty.levels_label(), "-");
    }

    #[test]
    fn parses_task_levels() {
        let bandit = game("bandit", false, 34);
        assert_eq!(parse_task_level(&bandit, "bandit3").unwrap(), 3);
        assert!(parse_task_level(&bandit, "34").is_err());

        let krypton = game("krypton", true, 6);
        assert_eq!(parse_task_level(&krypton, "6").unwrap(), 6);

        let natas = game("natas", false, 34);
        assert_eq!(parse_task_level(&natas, "34").unwrap(), 34);
    }
}
