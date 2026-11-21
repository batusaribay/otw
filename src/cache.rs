use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Default)]
pub struct CacheStats {
    pub files: usize,
    pub bytes: u64,
    pub oldest: Option<SystemTime>,
    pub newest: Option<SystemTime>,
}

pub struct Cache {
    root: PathBuf,
}

impl Cache {
    pub fn new() -> Result<Self> {
        let base = dirs::cache_dir().context("could not determine a cache directory")?;
        Ok(Self {
            root: base.join("otw"),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Map a site-relative path to a file in the cache. Rejects anything
    /// that could escape the cache directory.
    fn path(&self, rel: &str) -> Option<PathBuf> {
        let rel = rel.trim_start_matches('/');
        if rel.is_empty() || rel.contains("..") || rel.contains('\\') {
            return None;
        }
        let owned;
        let rel = if rel.ends_with('/') {
            owned = format!("{rel}index.html");
            owned.as_str()
        } else {
            rel
        };
        let mut path = self.root.clone();
        for component in rel.split('/') {
            if component.is_empty()
                || component == "."
                || !component
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
            {
                return None;
            }
            path.push(component);
        }
        Some(path)
    }

    pub fn read_any(&self, rel: &str) -> Option<String> {
        let path = self.path(rel)?;
        fs::read_to_string(path).ok()
    }

    pub fn read_fresh(&self, rel: &str, ttl: Duration) -> Option<String> {
        let path = self.path(rel)?;
        let modified = fs::metadata(&path).ok()?.modified().ok()?;
        let age = SystemTime::now().duration_since(modified).ok()?;
        if age > ttl {
            return None;
        }
        fs::read_to_string(path).ok()
    }

    pub fn write(&self, rel: &str, body: &str) -> Result<()> {
        let path = self
            .path(rel)
            .ok_or_else(|| anyhow::anyhow!("invalid cache path: /{rel}"))?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating cache directory {}", parent.display()))?;
        }
        let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let tmp = path.with_extension(format!("tmp-{}-{n}", std::process::id()));
        fs::write(&tmp, body).with_context(|| format!("writing {}", tmp.display()))?;
        fs::rename(&tmp, &path)
            .with_context(|| format!("moving cache file into {}", path.display()))?;
        Ok(())
    }

    pub fn stats(&self) -> Result<CacheStats> {
        let mut stats = CacheStats::default();
        if self.root.exists() {
            walk(&self.root, &mut stats)?;
        }
        Ok(stats)
    }

    pub fn clear(&self) -> Result<CacheStats> {
        let stats = self.stats()?;
        if self.root.exists() {
            fs::remove_dir_all(&self.root)
                .with_context(|| format!("removing {}", self.root.display()))?;
        }
        Ok(stats)
    }
}

fn walk(dir: &Path, stats: &mut CacheStats) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        if metadata.is_dir() {
            walk(&entry.path(), stats)?;
            continue;
        }
        stats.files += 1;
        stats.bytes += metadata.len();
        if let Ok(modified) = metadata.modified() {
            stats.oldest = Some(match stats.oldest {
                Some(old) if old <= modified => old,
                _ => modified,
            });
            stats.newest = Some(match stats.newest {
                Some(new) if new >= modified => new,
                _ => modified,
            });
        }
    }
    Ok(())
}

pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub fn human_age(time: SystemTime) -> String {
    let Ok(age) = SystemTime::now().duration_since(time) else {
        return "in the future".to_string();
    };
    human_duration(age)
}

pub fn human_duration(age: Duration) -> String {
    let secs = age.as_secs();
    match secs {
        0..=59 => "just now".to_string(),
        60..=3599 => format!("{} min ago", secs / 60),
        3600..=86_399 => format!("{} h ago", secs / 3600),
        _ => format!("{} days ago", secs / 86_400),
    }
}
