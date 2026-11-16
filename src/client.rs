use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Result, bail};

use crate::cache::Cache;

pub const BASE: &str = "https://overthewire.org";
pub const PAGE_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);
pub const MANIFEST_TTL: Duration = Duration::from_secs(6 * 60 * 60);

pub struct Client {
    http: reqwest::blocking::Client,
    cache: Cache,
    offline: bool,
    refresh: bool,
    warned: AtomicBool,
}

impl Client {
    pub fn new(offline: bool, refresh: bool) -> Result<Self> {
        let http = reqwest::blocking::Client::builder()
            .user_agent(concat!(
                "over-the-wire/",
                env!("CARGO_PKG_VERSION"),
                " (+https://overthewire.org/wargames/)"
            ))
            .timeout(Duration::from_secs(25))
            .build()?;
        Ok(Self {
            http,
            cache: Cache::new()?,
            offline,
            refresh,
            warned: AtomicBool::new(false),
        })
    }

    pub fn cache(&self) -> &Cache {
        &self.cache
    }

    /// Fetch a site-relative path (for example `wargames/bandit/bandit0.html`),
    /// using the local cache when it is fresh. If the network fails but a
    /// stale cache entry exists, the stale entry is used with a warning.
    pub fn get(&self, rel: &str, ttl: Duration) -> Result<String> {
        let rel = self.normalize(rel)?;

        if !self.refresh
            && let Some(body) = self.cache.read_fresh(&rel, ttl)
        {
            return Ok(body);
        }

        if self.offline {
            return match self.cache.read_any(&rel) {
                Some(body) => Ok(body),
                None => bail!(
                    "no cached copy of /{rel}\nrun once without --offline, or use `over-the-wire cache fetch --all`"
                ),
            };
        }

        let url = format!("{BASE}/{rel}");
        let fetched = self
            .http
            .get(&url)
            .send()
            .and_then(|response| response.error_for_status())
            .and_then(|response| response.text());

        match fetched {
            Ok(body) => {
                if let Err(error) = self.cache.write(&rel, &body) {
                    self.warn_once(format!("could not write to the cache: {error}"));
                }
                Ok(body)
            }
            Err(error) => {
                if let Some(body) = self.cache.read_any(&rel) {
                    self.warn_once(format!(
                        "network unavailable ({error}); using cached copies of pages"
                    ));
                    Ok(body)
                } else {
                    bail!("could not fetch {url}: {error}");
                }
            }
        }
    }

    fn normalize(&self, rel: &str) -> Result<String> {
        let mut path = rel.trim();
        if let Some(rest) = path
            .strip_prefix("https://overthewire.org")
            .or_else(|| path.strip_prefix("http://overthewire.org"))
        {
            path = rest;
        } else if path.starts_with("http://") || path.starts_with("https://") {
            bail!("only pages on overthewire.org are supported: {rel}");
        }
        let path = path.trim_start_matches('/');
        if path.is_empty() {
            bail!("empty page path");
        }
        if path.contains("..") || path.contains('\\') {
            bail!("invalid page path: {rel}");
        }
        Ok(path.to_string())
    }

    fn warn_once(&self, message: String) {
        if !self.warned.swap(true, Ordering::Relaxed) {
            eprintln!("warning: {message}");
        }
    }
}
