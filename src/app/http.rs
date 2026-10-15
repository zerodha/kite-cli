use std::sync::LazyLock;
use std::time::Duration;

/// Whole-request budget for public instrument downloads.
pub const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

static USER_AGENT: LazyLock<String> = LazyLock::new(|| {
    let os = match std::env::consts::OS {
        "macos" => "macOS",
        "windows" => "Windows",
        _ => "Linux",
    };
    let release = if cfg!(target_os = "linux") {
        std::fs::read_to_string("/proc/sys/kernel/osrelease").ok()
    } else {
        None
    };
    format_user_agent(
        env!("CARGO_PKG_VERSION"),
        std::env::consts::ARCH,
        os,
        release.as_deref(),
    )
});

fn format_user_agent(version: &str, arch: &str, os: &str, release: Option<&str>) -> String {
    let release = release.map(str::trim).filter(|value| !value.is_empty());
    match release {
        Some(release) => format!("KiteCLI/{version} ({arch}; {os} {release})"),
        None => format!("KiteCLI/{version} ({arch}; {os})"),
    }
}

/// Returns the process's User-Agent for REST and ticker requests.
#[must_use]
pub fn user_agent() -> &'static str {
    &USER_AGENT
}

/// Creates an HTTP builder with the CLI User-Agent and request timeouts.
pub fn client_builder(timeout: Duration) -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .user_agent(user_agent())
        .timeout(timeout)
        .connect_timeout(CONNECT_TIMEOUT)
}

#[cfg(test)]
mod tests {
    use super::format_user_agent;

    #[test]
    fn user_agent_uses_the_mobile_product_format() {
        assert_eq!(
            format_user_agent("0.1.0", "x86_64", "Linux", Some(" 6.1.0\n")),
            "KiteCLI/0.1.0 (x86_64; Linux 6.1.0)"
        );
        for (os, expected) in [
            ("Linux", "KiteCLI/0.1.0 (x86_64; Linux)"),
            ("macOS", "KiteCLI/0.1.0 (x86_64; macOS)"),
            ("Windows", "KiteCLI/0.1.0 (x86_64; Windows)"),
        ] {
            assert_eq!(format_user_agent("0.1.0", "x86_64", os, None), expected);
            assert_eq!(
                format_user_agent("0.1.0", "x86_64", os, Some(" \n")),
                expected
            );
        }
    }
}
