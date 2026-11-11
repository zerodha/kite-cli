//! Finds published kite releases and their installer scripts on GitHub.

use anyhow::{Context, Result, anyhow, bail, ensure};
use reqwest::StatusCode;
use reqwest::header::LOCATION;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

const RELEASES_URL: &str = "https://github.com/zerodha/kite-cli/releases";
const INSTALLER_ROOT: &str = "https://raw.githubusercontent.com/zerodha/kite-cli";
/// Tests point the latest-release lookup at a mock server through this variable
/// so they never call GitHub. It only changes which tag is reported: the tag is
/// validated strictly and installers always come from `INSTALLER_ROOT`.
const RELEASES_URL_ENV: &str = "KITE_RELEASES_URL";
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(5);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_INSTALLER_BYTES: usize = 1 << 20;

/// A release version: numeric `MAJOR.MINOR.PATCH`, ordered numerically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    major: u64,
    minor: u64,
    patch: u64,
}

impl Version {
    /// Parses `MAJOR.MINOR.PATCH` with an optional leading `v`.
    ///
    /// # Errors
    /// Returns an error for anything else, including pre-release suffixes.
    pub fn parse(text: &str) -> Result<Self> {
        Self::parse_numbers(text.strip_prefix('v').unwrap_or(text))
            .ok_or_else(|| anyhow!("invalid version {text:?}; expected MAJOR.MINOR.PATCH"))
    }

    /// Parses a release tag, which must be `v` followed by `MAJOR.MINOR.PATCH`.
    ///
    /// # Errors
    /// Returns an error when the tag has any other shape.
    pub fn parse_tag(tag: &str) -> Result<Self> {
        tag.strip_prefix('v')
            .and_then(Self::parse_numbers)
            .ok_or_else(|| anyhow!("invalid release tag {tag:?}"))
    }

    /// The version of this binary.
    ///
    /// # Errors
    /// Returns an error when the crate version is not a plain release version.
    pub fn installed() -> Result<Self> {
        Self::parse(env!("CARGO_PKG_VERSION"))
    }

    fn parse_numbers(text: &str) -> Option<Self> {
        let mut parts = text.split('.').map(|part| {
            let canonical = part == "0" || !part.starts_with('0');
            (canonical && !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
                .then(|| part.parse::<u64>().ok())
                .flatten()
        });
        let version = Self {
            major: parts.next()??,
            minor: parts.next()??,
            patch: parts.next()??,
        };
        parts.next().is_none().then_some(version)
    }

    #[must_use]
    pub fn tag(self) -> String {
        format!("v{self}")
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// The installed version compared with the latest release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpdateStatus {
    pub installed: Version,
    pub latest: Version,
}

impl UpdateStatus {
    #[must_use]
    pub fn update_available(self) -> bool {
        self.latest > self.installed
    }

    #[must_use]
    pub fn to_json(self) -> serde_json::Value {
        serde_json::json!({
            "installed": self.installed.to_string(),
            "latest": self.latest.to_string(),
            "update_available": self.update_available(),
        })
    }
}

/// Compares this binary with the latest published release.
///
/// # Errors
/// Returns an error when either version cannot be determined.
pub async fn check() -> Result<UpdateStatus> {
    Ok(UpdateStatus {
        installed: Version::installed()?,
        latest: latest().await?,
    })
}

/// Finds the latest published release without the GitHub API.
///
/// # Errors
/// Returns an error when GitHub cannot be reached or does not redirect to a valid tag.
pub async fn latest() -> Result<Version> {
    let base = std::env::var(RELEASES_URL_ENV).unwrap_or_else(|_| RELEASES_URL.to_string());
    latest_from(&base).await
}

async fn latest_from(releases_url: &str) -> Result<Version> {
    // GitHub redirects /releases/latest to /releases/tag/<tag>, as install.sh relies on.
    let client = super::http::client_builder(LOOKUP_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let response = client
        .get(format!("{}/latest", releases_url.trim_end_matches('/')))
        .send()
        .await
        .context("could not reach GitHub releases")?;
    let status = response.status();
    ensure!(
        status.is_redirection(),
        "GitHub releases answered HTTP {} instead of a redirect",
        status.as_u16()
    );
    let location = response
        .headers()
        .get(LOCATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| anyhow!("GitHub releases redirect has no Location header"))?;
    tag_from_location(location)
}

fn tag_from_location(location: &str) -> Result<Version> {
    let Some((_, tag)) = location.rsplit_once("/releases/tag/") else {
        bail!("unexpected release redirect {location:?}");
    };
    Version::parse_tag(tag)
}

/// The installer script published with `version`, for this platform's installer.
#[must_use]
pub fn installer_url(version: Version, windows: bool) -> String {
    let script = if windows { "install.ps1" } else { "install.sh" };
    format!("{INSTALLER_ROOT}/{}/scripts/{script}", version.tag())
}

/// Downloads `url` into `directory` and returns the saved file.
///
/// # Errors
/// Returns an error when the download fails, is not HTTP 200, or is too large.
pub async fn download_installer(url: &str, directory: &Path) -> Result<PathBuf> {
    let client = super::http::client_builder(DOWNLOAD_TIMEOUT).build()?;
    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("could not download {url}"))?;
    ensure!(
        response.status() == StatusCode::OK,
        "could not download {url}: HTTP {}",
        response.status().as_u16()
    );
    // Read in chunks so an oversized response is rejected without buffering all of it.
    let mut response = response;
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .with_context(|| format!("could not download {url}"))?
    {
        ensure!(
            body.len() + chunk.len() <= MAX_INSTALLER_BYTES,
            "installer from {url} is larger than {MAX_INSTALLER_BYTES} bytes"
        );
        body.extend_from_slice(&chunk);
    }
    ensure!(!body.is_empty(), "installer from {url} is empty");
    let name = url.rsplit('/').next().unwrap_or("installer");
    let path = directory.join(name);
    tokio::fs::write(&path, &body)
        .await
        .with_context(|| format!("could not save {}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    fn version(major: u64, minor: u64, patch: u64) -> Version {
        Version {
            major,
            minor,
            patch,
        }
    }

    #[test]
    fn versions_parse_and_compare_numerically() {
        assert_eq!(Version::parse("0.2.1").unwrap(), version(0, 2, 1));
        assert_eq!(Version::parse("v0.10.0").unwrap(), version(0, 10, 0));
        assert!(Version::parse("0.2.1").unwrap() < Version::parse("0.2.2").unwrap());
        assert!(Version::parse("0.2.2").unwrap() < Version::parse("0.10.0").unwrap());
        assert!(Version::parse("0.10.0").unwrap() < Version::parse("1.0.0").unwrap());
        assert_eq!(version(0, 10, 0).to_string(), "0.10.0");
        assert_eq!(version(0, 2, 1).tag(), "v0.2.1");
    }

    #[test]
    fn invalid_versions_and_tags_are_rejected() {
        for text in [
            "",
            "v",
            "0.2",
            "0.2.1.0",
            "0.2.x",
            "0.2.1-rc1",
            "0.2.1+meta",
            "00.2.1",
            "0.02.1",
            "-1.0.0",
            " 0.2.1",
            "0..1",
            "vv0.2.1",
            "0.2.99999999999999999999",
        ] {
            assert!(Version::parse(text).is_err(), "{text:?}");
        }
        for tag in [
            "0.2.1",
            "V0.2.1",
            "v0.2.1-rc1",
            "v0.2.1/../x",
            "latest",
            "v0.2.1?x=1",
        ] {
            assert!(Version::parse_tag(tag).is_err(), "{tag:?}");
        }
        assert_eq!(Version::parse_tag("v0.2.1").unwrap(), version(0, 2, 1));
    }

    #[test]
    fn redirect_locations_yield_the_release_tag() {
        assert_eq!(
            tag_from_location("https://github.com/zerodha/kite-cli/releases/tag/v0.2.1").unwrap(),
            version(0, 2, 1)
        );
        assert_eq!(
            tag_from_location("/zerodha/kite-cli/releases/tag/v1.20.3").unwrap(),
            version(1, 20, 3)
        );
        for location in [
            "https://github.com/zerodha/kite-cli/releases",
            "https://github.com/zerodha/kite-cli/releases/tag/nightly",
            "https://github.com/zerodha/kite-cli/releases/tag/v0.2.1/extra",
        ] {
            assert!(tag_from_location(location).is_err(), "{location}");
        }
    }

    #[tokio::test]
    async fn latest_reads_the_redirect_without_following_it() {
        let server = MockServer::start_async().await;
        let latest = server
            .mock_async(|when, then| {
                when.method(GET).path("/releases/latest");
                then.status(302).header(
                    "location",
                    "https://github.com/zerodha/kite-cli/releases/tag/v0.2.2",
                );
            })
            .await;
        let found = latest_from(&server.url("/releases")).await.unwrap();
        assert_eq!(found, version(0, 2, 2));
        latest.assert_calls_async(1).await;
    }

    #[tokio::test]
    async fn latest_rejects_missing_redirects_and_bad_tags() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.path("/bad/latest");
                then.status(302)
                    .header("location", "/zerodha/kite-cli/releases/tag/v0.2.2-rc1");
            })
            .await;
        server
            .mock_async(|when, then| {
                when.path("/ok/latest");
                then.status(200).body("no redirect");
            })
            .await;
        let bad = latest_from(&server.url("/bad")).await.unwrap_err();
        assert_eq!(bad.to_string(), "invalid release tag \"v0.2.2-rc1\"");
        let ok = latest_from(&server.url("/ok")).await.unwrap_err();
        assert_eq!(
            ok.to_string(),
            "GitHub releases answered HTTP 200 instead of a redirect"
        );
        let missing = latest_from(&server.url("/missing")).await.unwrap_err();
        assert_eq!(
            missing.to_string(),
            "GitHub releases answered HTTP 404 instead of a redirect"
        );
    }

    #[test]
    fn installer_urls_pin_the_release_tag() {
        assert_eq!(
            installer_url(version(0, 2, 1), false),
            "https://raw.githubusercontent.com/zerodha/kite-cli/v0.2.1/scripts/install.sh"
        );
        assert_eq!(
            installer_url(version(0, 2, 1), true),
            "https://raw.githubusercontent.com/zerodha/kite-cli/v0.2.1/scripts/install.ps1"
        );
    }

    #[tokio::test]
    async fn installers_download_into_the_given_directory() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.path("/v0.2.1/scripts/install.sh");
                then.status(200).body("#!/bin/sh\nexit 0\n");
            })
            .await;
        let directory =
            std::env::temp_dir().join(format!("kite-release-download-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = download_installer(&server.url("/v0.2.1/scripts/install.sh"), &directory)
            .await
            .unwrap();
        assert_eq!(path, directory.join("install.sh"));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "#!/bin/sh\nexit 0\n"
        );
        let missing = download_installer(&server.url("/missing.sh"), &directory)
            .await
            .unwrap_err();
        assert!(
            missing.to_string().ends_with("/missing.sh: HTTP 404"),
            "{missing}"
        );
        server
            .mock_async(|when, then| {
                when.path("/huge.sh");
                then.status(200).body(vec![b'#'; (1 << 20) + 1]);
            })
            .await;
        let huge = download_installer(&server.url("/huge.sh"), &directory)
            .await
            .unwrap_err();
        assert!(
            huge.to_string().ends_with("is larger than 1048576 bytes"),
            "{huge}"
        );
        assert!(!directory.join("huge.sh").exists());
        std::fs::remove_dir_all(directory).unwrap();
    }
}
