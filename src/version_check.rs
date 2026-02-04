use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Once;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use serde::{Deserialize, Serialize};

use crate::VERSION;

static CHECK_ONCE: Once = Once::new();
const SDK_NAME: &str = "rust";
const TWELVE_HOURS_SECS: u64 = 12 * 60 * 60;

#[derive(Serialize, Deserialize, Default)]
struct VersionEntry {
    current: Option<String>,
    latest: Option<String>,
    last_notified: Option<String>,
}

type VersionCache = HashMap<String, VersionEntry>;

pub fn check_for_updates(headers: &HashMap<String, String>) {
    CHECK_ONCE.call_once(|| {
        if !is_dev_mode() {
            return;
        }

        let latest = headers.get("x-muxi-sdk-latest")
            .or_else(|| headers.get("X-Muxi-SDK-Latest"));
        
        let latest = match latest {
            Some(v) => v.clone(),
            None => return,
        };

        if !is_newer_version(&latest, VERSION) {
            return;
        }

        update_latest_version(&latest);

        if !notified_recently() {
            eprintln!("[muxi] SDK update available: {} (current: {})", latest, VERSION);
            eprintln!("[muxi] Run: cargo update -p muxi");
            mark_notified();
        }
    });
}

fn is_dev_mode() -> bool {
    std::env::var("MUXI_DEBUG").map(|v| v == "1").unwrap_or(false)
}

fn get_cache_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".muxi").join("sdk-versions.json"))
}

fn load_cache() -> VersionCache {
    let path = match get_cache_path() {
        Some(p) => p,
        None => return HashMap::new(),
    };

    if !path.exists() {
        return HashMap::new();
    }

    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

fn save_cache(cache: &VersionCache) {
    let path = match get_cache_path() {
        Some(p) => p,
        None => return,
    };

    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }

    if let Ok(content) = serde_json::to_string_pretty(cache) {
        let _ = fs::write(&path, content);
    }
}

fn is_newer_version(latest: &str, current: &str) -> bool {
    latest > current
}

fn notified_recently() -> bool {
    let cache = load_cache();
    let entry = match cache.get(SDK_NAME) {
        Some(e) => e,
        None => return false,
    };

    let last_notified = match &entry.last_notified {
        Some(t) => t,
        None => return false,
    };

    let last_secs: u64 = last_notified.parse().unwrap_or(0);
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();

    now_secs - last_secs < TWELVE_HOURS_SECS
}

fn update_latest_version(latest: &str) {
    let mut cache = load_cache();
    let entry = cache.entry(SDK_NAME.to_string()).or_default();
    entry.current = Some(VERSION.to_string());
    entry.latest = Some(latest.to_string());
    save_cache(&cache);
}

fn mark_notified() {
    let mut cache = load_cache();
    if let Some(entry) = cache.get_mut(SDK_NAME) {
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();
        entry.last_notified = Some(now_secs.to_string());
        save_cache(&cache);
    }
}
