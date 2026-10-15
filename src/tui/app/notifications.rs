use super::{App, charts::now_epoch};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

const TOAST_TTL: Duration = Duration::from_secs(5);

const EVENT_LOG_CAP: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Success,
    Error,
}

#[derive(Debug)]
pub struct Toast {
    pub at: Instant,

    pub unix: u64,
    pub level: ToastLevel,
    pub text: String,
}

impl App {
    pub fn toast(&mut self, level: ToastLevel, text: impl Into<String>) {
        let unix = now_epoch();
        self.toasts.push_back(Toast {
            at: Instant::now(),
            unix,
            level,
            text: text.into(),
        });
        if self.toasts.len() > EVENT_LOG_CAP {
            self.toasts.pop_front();
        }
    }

    pub fn toasts(&self) -> &VecDeque<Toast> {
        &self.toasts
    }

    #[cfg(test)]
    pub fn push_event(&mut self, unix: u64, level: ToastLevel, text: &str) {
        self.toasts.push_back(Toast {
            at: Instant::now(),
            unix,
            level,
            text: text.to_string(),
        });
    }

    pub(super) fn toast_error(&mut self, text: String) {
        if should_show_error(&mut self.error_toasts, &text, Instant::now()) {
            self.toast(ToastLevel::Error, text);
        }
    }

    pub fn visible_toast(&self) -> Option<&Toast> {
        self.toasts
            .back()
            .filter(|toast| toast.at.elapsed() < TOAST_TTL)
    }
}

const ERROR_TOAST_DEDUP: Duration = Duration::from_secs(30);

const ERROR_TOAST_CAP: usize = 16;

fn should_show_error(seen: &mut HashMap<String, Instant>, text: &str, now: Instant) -> bool {
    if let Some(at) = seen.get(text)
        && now.duration_since(*at) < ERROR_TOAST_DEDUP
    {
        return false;
    }
    if !seen.contains_key(text)
        && seen.len() >= ERROR_TOAST_CAP
        && let Some(oldest) = seen
            .iter()
            .min_by_key(|(_, at)| **at)
            .map(|(key, _)| key.clone())
    {
        seen.remove(&oldest);
    }
    seen.insert(text.to_string(), now);
    true
}

#[cfg(test)]
mod tests {
    use super::should_show_error;
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    #[test]
    fn error_toast_dedup_suppresses_recent_alternating() {
        let mut seen: HashMap<String, Instant> = HashMap::new();
        let t0 = Instant::now();

        assert!(should_show_error(&mut seen, "a", t0));
        assert!(should_show_error(&mut seen, "b", t0));
        assert!(!should_show_error(&mut seen, "a", t0));
        assert!(!should_show_error(&mut seen, "b", t0));

        let later = t0 + Duration::from_secs(31);
        assert!(should_show_error(&mut seen, "a", later));
    }

    #[test]
    fn error_toast_dedup_caps_at_16_entries() {
        let mut seen: HashMap<String, Instant> = HashMap::new();
        let base = Instant::now();

        for i in 0..16u64 {
            let at = base + Duration::from_millis(i);
            assert!(should_show_error(&mut seen, &format!("m{i}"), at));
        }
        assert_eq!(seen.len(), 16);

        let at = base + Duration::from_millis(100);
        assert!(should_show_error(&mut seen, "m16", at));
        assert_eq!(seen.len(), 16);
        assert!(!seen.contains_key("m0"));
        assert!(seen.contains_key("m16"));
    }
}
