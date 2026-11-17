//! Tells a running TUI that a newer kite release exists.

use crate::app::release::{self, Version};
use crate::tui::data::{DataEvent, send};
use crate::tui::event::AppEvent;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

const DISABLE_ENV: &str = "KITE_NO_UPDATE_CHECK";
const CHECK_EVERY_SECS: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub latest: Version,
    /// False once an earlier session already announced this version.
    pub announce: bool,
    pub hint: &'static str,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Cache {
    checked_at: u64,
    latest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    notified: Option<String>,
}

#[must_use]
pub fn disabled_by_env() -> bool {
    std::env::var(DISABLE_ENV).is_ok_and(|value| !value.is_empty() && value != "0")
}

pub fn spawn_check(tx: mpsc::Sender<AppEvent>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let Ok(installed) = Version::installed() else {
            return;
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        let path = cache_path();
        let Some((latest, announce)) =
            resolve(path.as_deref(), now, installed, release::latest).await
        else {
            return;
        };
        let notice = Notice {
            latest,
            announce,
            hint: upgrade_hint(),
        };
        send(&tx, DataEvent::UpdateAvailable(notice)).await;
    })
}

/// Returns the newer release and whether to announce it, reusing a lookup
/// younger than a day so several open windows share one request.
async fn resolve<F, Fut>(
    path: Option<&Path>,
    now: u64,
    installed: Version,
    fetch: F,
) -> Option<(Version, bool)>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = anyhow::Result<Version>>,
{
    let mut cache = path.and_then(read_cache).unwrap_or_default();
    let cached = Version::parse(&cache.latest)
        .ok()
        .filter(|_| now.saturating_sub(cache.checked_at) < CHECK_EVERY_SECS);
    let mut dirty = false;
    let latest = if let Some(latest) = cached {
        latest
    } else {
        let latest = fetch().await.ok()?;
        cache.checked_at = now;
        cache.latest = latest.to_string();
        dirty = true;
        latest
    };
    let newer = latest > installed;
    let announce = newer && cache.notified.as_deref() != Some(cache.latest.as_str());
    if announce {
        cache.notified = Some(cache.latest.clone());
        dirty = true;
    }
    if dirty && let Some(path) = path {
        let _ = crate::util::save_json_atomic(path, &cache);
    }
    newer.then_some((latest, announce))
}

fn cache_path() -> Option<PathBuf> {
    let mut path = std::env::var_os("XDG_CACHE_HOME")
        .filter(|path| Path::new(path).is_absolute())
        .map(PathBuf::from)
        .or_else(dirs::cache_dir)?;
    path.push("kite");
    path.push("update.json");
    Some(path)
}

fn read_cache(path: &Path) -> Option<Cache> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn upgrade_hint() -> &'static str {
    let exe = std::env::current_exe().and_then(std::fs::canonicalize);
    hint_for(exe.as_deref().unwrap_or(Path::new("")))
}

fn hint_for(exe: &Path) -> &'static str {
    let exe = exe.to_string_lossy();
    if exe.contains("/nix/store/") || exe.contains("/Cellar/") {
        "upgrade with your package manager"
    } else {
        "run kite upgrade"
    }
}

#[cfg(test)]
mod tests {
    use super::{Cache, hint_for, read_cache, resolve};
    use crate::app::release::Version;
    use std::cell::Cell;
    use std::path::{Path, PathBuf};

    const DAY: u64 = 24 * 60 * 60;

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    fn temp_cache(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "kite-update-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("update.json")
    }

    async fn run(
        path: &Path,
        now: u64,
        installed: &str,
        remote: &str,
        calls: &Cell<u32>,
    ) -> Option<(String, bool)> {
        resolve(Some(path), now, v(installed), || async {
            calls.set(calls.get() + 1);
            Ok(v(remote))
        })
        .await
        .map(|(latest, announce)| (latest.to_string(), announce))
    }

    #[tokio::test]
    async fn a_newer_release_is_announced_once_and_then_shown_quietly() {
        let path = temp_cache("announce");
        let calls = Cell::new(0);
        assert_eq!(
            run(&path, 1_000, "0.2.0", "0.3.0", &calls).await,
            Some(("0.3.0".into(), true))
        );
        assert_eq!(
            run(&path, 2_000, "0.2.0", "0.3.0", &calls).await,
            Some(("0.3.0".into(), false)),
            "a second window does not toast again"
        );
        assert_eq!(calls.get(), 1, "the second window reuses the cached lookup");
    }

    #[tokio::test]
    async fn the_lookup_repeats_after_a_day_and_announces_a_new_version() {
        let path = temp_cache("expire");
        let calls = Cell::new(0);
        run(&path, 1_000, "0.2.0", "0.3.0", &calls).await;
        assert_eq!(
            run(&path, 1_000 + DAY, "0.2.0", "0.4.0", &calls).await,
            Some(("0.4.0".into(), true))
        );
        assert_eq!(calls.get(), 2);
    }

    #[tokio::test]
    async fn an_up_to_date_install_gets_no_notice_but_caches_the_lookup() {
        let path = temp_cache("current");
        let calls = Cell::new(0);
        assert_eq!(run(&path, 1_000, "0.3.0", "0.3.0", &calls).await, None);
        assert_eq!(run(&path, 2_000, "0.3.0", "0.3.0", &calls).await, None);
        assert_eq!(calls.get(), 1);
    }

    #[tokio::test]
    async fn upgrading_past_the_cached_release_clears_the_notice() {
        let path = temp_cache("upgraded");
        let calls = Cell::new(0);
        run(&path, 1_000, "0.2.0", "0.3.0", &calls).await;
        assert_eq!(run(&path, 2_000, "0.3.0", "0.3.0", &calls).await, None);
    }

    #[tokio::test]
    async fn a_failed_lookup_is_silent_and_not_cached() {
        let path = temp_cache("failed");
        let notice = resolve(Some(&path), 1_000, v("0.2.0"), || async {
            Err(anyhow::anyhow!("offline"))
        })
        .await;
        assert_eq!(notice, None);
        assert!(read_cache(&path).is_none());
    }

    #[tokio::test]
    async fn a_corrupt_cache_falls_back_to_a_fresh_lookup() {
        let path = temp_cache("corrupt");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "not json").unwrap();
        let calls = Cell::new(0);
        assert_eq!(
            run(&path, 1_000, "0.2.0", "0.3.0", &calls).await,
            Some(("0.3.0".into(), true))
        );
        let cache: Cache = read_cache(&path).unwrap();
        assert_eq!(cache.latest, "0.3.0");
    }

    #[test]
    fn package_manager_installs_are_told_to_use_their_package_manager() {
        assert_eq!(
            hint_for(Path::new("/nix/store/abc-kite-0.2.0/bin/kite")),
            "upgrade with your package manager"
        );
        assert_eq!(
            hint_for(Path::new("/opt/homebrew/Cellar/kite/0.2.0/bin/kite")),
            "upgrade with your package manager"
        );
        assert_eq!(
            hint_for(Path::new("/home/u/.local/bin/kite")),
            "run kite upgrade"
        );
    }
}
