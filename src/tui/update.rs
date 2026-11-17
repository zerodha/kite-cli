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
    let known = Version::parse(&cache.latest).ok();
    let fresh = cache.checked_at <= now && now - cache.checked_at < CHECK_EVERY_SECS;
    let mut dirty = false;
    let latest = match known.filter(|_| fresh) {
        Some(latest) => latest,
        None => match fetch().await {
            Ok(latest) => {
                cache.checked_at = now;
                cache.latest = latest.to_string();
                dirty = true;
                latest
            }
            Err(_) => known?,
        },
    };
    let newer = latest > installed;
    let mut announce = newer && cache.notified.as_deref() != Some(cache.latest.as_str());
    if announce {
        cache.notified = Some(cache.latest.clone());
        dirty = true;
    }
    if dirty {
        let saved = path.is_some_and(|path| crate::util::save_json_atomic(path, &cache).is_ok());
        // Without a cache every launch would announce again, so stay quiet and
        // rely on the header item.
        announce &= saved;
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
    if crate::commands::upgrade::can_self_upgrade() {
        "run kite upgrade"
    } else {
        "upgrade with your package manager"
    }
}

#[cfg(test)]
mod tests {
    use super::{Cache, read_cache, resolve};
    use crate::app::release::Version;
    use std::cell::Cell;
    use std::path::{Path, PathBuf};

    const DAY: u64 = 24 * 60 * 60;

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    struct TempCache(PathBuf);

    impl TempCache {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("kite-update-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            Self(dir)
        }

        fn file(&self) -> PathBuf {
            self.0.join("update.json")
        }
    }

    impl Drop for TempCache {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
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
        let dir = TempCache::new("announce");
        let path = dir.file();
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
        let dir = TempCache::new("expire");
        let path = dir.file();
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
        let dir = TempCache::new("current");
        let path = dir.file();
        let calls = Cell::new(0);
        assert_eq!(run(&path, 1_000, "0.3.0", "0.3.0", &calls).await, None);
        assert_eq!(run(&path, 2_000, "0.3.0", "0.3.0", &calls).await, None);
        assert_eq!(calls.get(), 1);
    }

    #[tokio::test]
    async fn upgrading_past_the_cached_release_clears_the_notice() {
        let dir = TempCache::new("upgraded");
        let path = dir.file();
        let calls = Cell::new(0);
        run(&path, 1_000, "0.2.0", "0.3.0", &calls).await;
        assert_eq!(run(&path, 2_000, "0.3.0", "0.3.0", &calls).await, None);
    }

    #[tokio::test]
    async fn a_failed_lookup_is_silent_and_not_cached() {
        let dir = TempCache::new("failed");
        let path = dir.file();
        let notice = resolve(Some(&path), 1_000, v("0.2.0"), || async {
            Err(anyhow::anyhow!("offline"))
        })
        .await;
        assert_eq!(notice, None);
        assert!(read_cache(&path).is_none());
    }

    #[tokio::test]
    async fn a_corrupt_cache_falls_back_to_a_fresh_lookup() {
        let dir = TempCache::new("corrupt");
        let path = dir.file();
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

    #[tokio::test]
    async fn a_cache_dated_in_the_future_is_refreshed() {
        let dir = TempCache::new("future");
        let path = dir.file();
        let calls = Cell::new(0);
        run(&path, 10 * DAY, "0.2.0", "0.3.0", &calls).await;
        run(&path, 1_000, "0.2.0", "0.3.0", &calls).await;
        assert_eq!(calls.get(), 2);
    }

    #[tokio::test]
    async fn an_offline_launch_keeps_showing_a_known_newer_release() {
        let dir = TempCache::new("offline");
        let path = dir.file();
        let calls = Cell::new(0);
        run(&path, 1_000, "0.2.0", "0.3.0", &calls).await;
        let notice = resolve(Some(&path), 1_000 + DAY, v("0.2.0"), || async {
            Err(anyhow::anyhow!("offline"))
        })
        .await;
        assert_eq!(notice, Some((v("0.3.0"), false)));
    }

    #[tokio::test]
    async fn without_a_cache_the_release_is_shown_but_never_announced() {
        let notice = resolve(None, 1_000, v("0.2.0"), || async { Ok(v("0.3.0")) }).await;
        assert_eq!(notice, Some((v("0.3.0"), false)));
    }
}
