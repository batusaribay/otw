use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Result, bail};

use crate::cache::Cache;

pub const BASE: &str = "https://overthewire.org";
/// Never read more than this from a page. A broken or hostile response should
/// not be able to exhaust memory.
const MAX_RESPONSE_BYTES: u64 = 8 * 1024 * 1024;
pub const PAGE_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);
pub const MANIFEST_TTL: Duration = Duration::from_secs(6 * 60 * 60);

fn same_origin(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.host_str() == Some("overthewire.org")
        && url.port_or_known_default() == Some(443)
}

pub fn normalize_path(rel: &str) -> Result<String> {
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
    for component in path.strip_suffix('/').unwrap_or(path).split('/') {
        if component.is_empty()
            || component == "."
            || !component
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        {
            bail!("invalid page path: {rel}");
        }
    }
    Ok(path.to_string())
}

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
                "otw/",
                env!("CARGO_PKG_VERSION"),
                " (+https://overthewire.org/wargames/)"
            ))
            .timeout(Duration::from_secs(25))
            .connect_timeout(Duration::from_secs(10))
            .https_only(true)
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 5 {
                    attempt.error("too many redirects")
                } else if !same_origin(attempt.url()) {
                    attempt.error("redirect left the OverTheWire HTTPS origin")
                } else {
                    attempt.follow()
                }
            }))
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
        let rel = normalize_path(rel)?;

        if !self.refresh
            && let Some(body) = self.cache.read_fresh(&rel, ttl)
        {
            return Ok(body);
        }

        if self.offline {
            return match self.cache.read_any(&rel) {
                Some(body) => Ok(body),
                None => bail!(
                    "no cached copy of /{rel}\nrun once without --offline, or use `otw cache fetch --all`"
                ),
            };
        }

        let url = format!("{BASE}/{rel}");
        let fetched = (|| -> Result<String> {
            let response = self.http.get(&url).send()?.error_for_status()?;
            let mut bytes = Vec::new();
            response
                .take(MAX_RESPONSE_BYTES + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 > MAX_RESPONSE_BYTES {
                bail!("response exceeds the {MAX_RESPONSE_BYTES}-byte limit");
            }
            Ok(String::from_utf8_lossy(&bytes).into_owned())
        })();

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

    fn warn_once(&self, message: String) {
        if !self.warned.swap(true, Ordering::Relaxed) {
            eprintln!("warning: {message}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_only_safe_site_relative_paths() {
        assert_eq!(normalize_path("/rules/").unwrap(), "rules/");
        assert_eq!(
            normalize_path("https://overthewire.org/wargames/bandit/bandit0.html").unwrap(),
            "wargames/bandit/bandit0.html"
        );
        assert!(normalize_path("https://example.org/rules/").is_err());
        assert!(normalize_path("../rules/").is_err());
        assert!(normalize_path("rules/%2e%2e/secret").is_err());
        assert!(normalize_path("rules/?x=1").is_err());
        assert!(normalize_path("rules/#fragment").is_err());
        assert!(normalize_path("rules//").is_err());
        assert!(normalize_path("rules/\u{1b}[31m").is_err());
    }

    #[test]
    fn redirects_stay_on_the_original_https_origin() {
        assert!(same_origin(
            &reqwest::Url::parse("https://overthewire.org/wargames/").unwrap()
        ));
        assert!(!same_origin(
            &reqwest::Url::parse("http://overthewire.org/wargames/").unwrap()
        ));
        assert!(!same_origin(
            &reqwest::Url::parse("https://www.overthewire.org/wargames/").unwrap()
        ));
        assert!(!same_origin(
            &reqwest::Url::parse("https://overthewire.org:444/wargames/").unwrap()
        ));
        assert!(!same_origin(
            &reqwest::Url::parse("https://example.org/wargames/").unwrap()
        ));
        assert!(!same_origin(
            &reqwest::Url::parse("https://overthewire.org@example.org/").unwrap()
        ));
    }
}
