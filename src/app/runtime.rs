use crate::app::errors::MissingSession;
use crate::app::http::user_agent;
use crate::app_api::trusted::Session;
use crate::cli::Cli;
use anyhow::{Context, Result, bail};
use kiteconnect::KiteConnect;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::{self, OpenOptions};
use std::future::Future;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

pub const API_ROOT: Option<&str> = option_env!("KITE_API_ROOT");
pub const TICKER_URL: Option<&str> = option_env!("KITE_TICKER_URL");
pub const DEFAULT_API_KEY: Option<&str> = option_env!("KITE_DEFAULT_API_KEY");

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct ConfigFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_root: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticker_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    pub access_token: Option<String>,
}

pub const SESSION_FIELDS: [&str; 1] = ["access_token"];

impl ConfigFile {
    fn clear_session(&mut self) {
        self.access_token = None;
    }
}

impl std::fmt::Debug for ConfigFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigFile")
            .field("api_root", &self.api_root)
            .field("ticker_url", &self.ticker_url)
            .field("app_uuid", &self.app_uuid)
            .field("api_key", &self.api_key)
            .field("user_id", &self.user_id)
            .field("access_token", &redacted_option(self.access_token.as_ref()))
            .finish()
    }
}

/// Renders `Some(_)` as `Some("<redacted>")` and preserves `None`, so Debug
/// output stays useful for troubleshooting without leaking the secret.
fn redacted_option(value: Option<&String>) -> Option<&'static str> {
    value.map(|_| "<redacted>")
}

#[derive(Debug, Clone)]
pub struct Runtime {
    pub json: bool,
    config: Option<PathBuf>,
    app_uuid: OnceLock<String>,
}

#[derive(Clone)]
pub struct ResolvedCredentials {
    pub path: PathBuf,
    pub config_exists: bool,
    pub api_root: Option<String>,
    pub ticker_url: Option<String>,
    pub app_uuid: Option<String>,
    pub api_key: Option<String>,
    pub user_id: Option<String>,
    pub access_token: Option<String>,
}

impl std::fmt::Debug for ResolvedCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedCredentials")
            .field("path", &self.path)
            .field("config_exists", &self.config_exists)
            .field("api_root", &self.api_root)
            .field("ticker_url", &self.ticker_url)
            .field("app_uuid", &self.app_uuid)
            .field("api_key", &self.api_key)
            .field("user_id", &self.user_id)
            .field("access_token", &redacted_option(self.access_token.as_ref()))
            .finish()
    }
}

impl ResolvedCredentials {
    /// # Errors
    /// Returns an error when no API root is configured.
    pub fn api_root(&self) -> Result<&str> {
        self.api_root
            .as_deref()
            .context("no API root configured; set KITE_API_ROOT or \"api_root\" in config.json")
    }

    /// # Errors
    /// Returns an error when no ticker URL is configured.
    pub fn ticker_url(&self) -> Result<&str> {
        self.ticker_url.as_deref().context(
            "no ticker URL configured; set KITE_TICKER_URL or \"ticker_url\" in config.json",
        )
    }

    /// # Errors
    /// Returns an error when no API key is configured.
    pub fn api_key(&self) -> Result<&str> {
        self.api_key
            .as_deref()
            .context("no API key configured; set KITE_API_KEY or \"api_key\" in config.json")
    }
}

impl Runtime {
    #[must_use]
    pub fn from_cli(cli: &Cli) -> Self {
        Self {
            json: cli.json,
            config: cli.config.clone(),
            app_uuid: OnceLock::new(),
        }
    }

    /// # Errors
    /// Returns an error when the default config directory is unavailable.
    pub fn config_path(&self) -> Result<PathBuf> {
        if let Some(path) = &self.config {
            return Ok(path.clone());
        }
        if let Ok(path) = env::var("KITE_CONFIG")
            && !path.trim().is_empty()
        {
            return Ok(PathBuf::from(path));
        }
        let mut path = dirs::config_dir().context("missing config directory")?;
        path.push("kite");
        path.push("config.json");
        Ok(path)
    }

    /// # Errors
    /// Returns an error if the config path cannot be resolved, read, or decoded.
    pub fn load_config(&self) -> Result<(PathBuf, bool, ConfigFile)> {
        let path = self.config_path()?;
        if !path.exists() {
            return Ok((path, false, ConfigFile::default()));
        }
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read config: {}", path.display()))?;
        let config: ConfigFile = serde_json::from_str(contents.trim_start_matches('\u{feff}')).context(
            "invalid config JSON; expected api_root/api_key/ticker_url/app_uuid/access_token/user_id fields",
        )?;
        Ok((path, true, config))
    }

    /// # Errors
    /// Returns an error if the config cannot be encoded or atomically saved.
    pub fn save_config(&self, config: &ConfigFile) -> Result<PathBuf> {
        let path = self.config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        let mut contents = serde_json::to_string_pretty(config)?;
        contents.push('\n');
        replace_private_file(&path, contents.as_bytes(), || {
            CONFIG_TMP_SEQ.fetch_add(1, Ordering::Relaxed)
        })?;
        Ok(path)
    }

    /// # Errors
    /// Returns an error if the config cannot be loaded or the session saved.
    pub fn save_session(&self, session: &Session) -> Result<PathBuf> {
        let app_uuid = self.app_uuid()?;
        let (_, _, mut config) = self.load_config()?;
        config.app_uuid = Some(app_uuid);
        config.user_id = Some(session.user_id.clone());
        config.access_token = Some(session.access_token.clone());
        self.save_config(&config)
    }

    /// Returns the saved app identity, or a stable identity for this runtime.
    /// Login persists the identity with the session.
    ///
    /// # Errors
    /// Returns an error if the config cannot be loaded.
    pub fn app_uuid(&self) -> Result<String> {
        let (_, _, config) = self.load_config()?;
        if let Some(value) = config.app_uuid
            && uuid::Uuid::parse_str(&value).is_ok()
        {
            return Ok(value);
        }
        Ok(self
            .app_uuid
            .get_or_init(|| uuid::Uuid::new_v4().to_string())
            .clone())
    }

    /// Clears [`SESSION_FIELDS`] from the saved config.
    ///
    /// # Errors
    /// Returns an error if the config cannot be loaded or saved.
    pub fn clear_session(&self) -> Result<PathBuf> {
        let (_, _, mut config) = self.load_config()?;
        config.clear_session();
        self.save_config(&config)
    }

    /// # Errors
    /// Returns an error if the saved config cannot be loaded.
    pub fn resolve_credentials(&self) -> Result<ResolvedCredentials> {
        let (path, config_exists, config) = self.load_config()?;
        Ok(ResolvedCredentials {
            path,
            config_exists,
            api_root: resolve_config_value("KITE_API_ROOT", config.api_root, API_ROOT),
            ticker_url: resolve_config_value("KITE_TICKER_URL", config.ticker_url, TICKER_URL),
            app_uuid: config.app_uuid,
            api_key: resolve_config_value("KITE_API_KEY", config.api_key, DEFAULT_API_KEY),
            user_id: resolve_config_value("KITE_USER_ID", config.user_id, None),
            access_token: resolve_config_value("KITE_ACCESS_TOKEN", config.access_token, None),
        })
    }

    /// # Errors
    /// Returns an error if credentials are missing or the client cannot be built.
    pub fn client(&self) -> Result<KiteConnect> {
        let creds = self.resolve_credentials()?;
        let root = creds.api_root()?;
        let api_key = creds.api_key()?;
        let access_token = creds.access_token.as_deref().ok_or(MissingSession)?;
        Ok(KiteConnect::builder(api_key)
            .access_token(kiteconnect::AccessToken::new(access_token)?)
            .base_url(root)
            .user_agent(user_agent())
            .header("X-Kite-App-UUID", self.app_uuid()?)
            .build()?)
    }

    /// # Errors
    /// Returns an error if the request or client setup fails.
    pub async fn with_session<T, F, Fut>(&self, f: F) -> Result<T>
    where
        F: FnOnce(KiteConnect) -> Fut,
        Fut: Future<Output = kiteconnect::Result<T>>,
    {
        match f(self.client()?).await {
            Ok(value) => Ok(value),
            Err(err) if self.json && err.is_session_expired() => {
                bail!("session expired; run `kite auth login`")
            }
            Err(err) => Err(err.into()),
        }
    }
}

/// Distinguishes temp candidates from concurrent saves in this process.
pub(crate) static CONFIG_TMP_SEQ: AtomicU64 = AtomicU64::new(0);
const CONFIG_TMP_ATTEMPTS: u32 = 16;

fn config_tmp_path(path: &Path, seq: u64) -> PathBuf {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config.json");
    dir.join(format!(".{file_name}.{}.{seq}.tmp", std::process::id()))
}

/// Publishes `contents` at `path` through a fresh sibling and a
/// rename, so readers see the old file or the new one, never a partial write.
/// `create_new` refuses any existing candidate, including a planted symlink,
/// and the cleanup paths remove only the file this call created.
pub(crate) fn replace_private_file(
    path: &Path,
    contents: &[u8],
    mut next_seq: impl FnMut() -> u64,
) -> Result<()> {
    let resolved;
    let path = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            resolved = fs::canonicalize(path)
                .with_context(|| format!("cannot resolve config symlink: {}", path.display()))?;
            resolved.as_path()
        }
        Ok(_) => path,
        Err(error) if error.kind() == ErrorKind::NotFound => path,
        Err(error) => {
            return Err(error).with_context(|| format!("cannot inspect {}", path.display()));
        }
    };
    let mut attempts = 0;
    let (tmp_path, mut file) = loop {
        let tmp_path = config_tmp_path(path, next_seq());
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        match options.open(&tmp_path) {
            Ok(file) => break (tmp_path, file),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => {
                attempts += 1;
                if attempts == CONFIG_TMP_ATTEMPTS {
                    bail!(
                        "failed to write {}: {CONFIG_TMP_ATTEMPTS} temp file names were taken",
                        path.display()
                    );
                }
            }
            Err(err) => {
                return Err(err).with_context(|| format!("failed to write {}", tmp_path.display()));
            }
        }
    };

    let written = file.write_all(contents).and_then(|()| file.sync_all());
    drop(file);
    if let Err(err) = written {
        let _ = fs::remove_file(&tmp_path);
        return Err(err).with_context(|| format!("failed to write {}", tmp_path.display()));
    }
    if let Err(err) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(err).with_context(|| format!("failed to persist config: {}", path.display()));
    }
    Ok(())
}

fn resolve_config_value(
    env_name: &str,
    config_value: Option<String>,
    build_value: Option<&str>,
) -> Option<String> {
    if let Ok(value) = env::var(env_name)
        && !value.trim().is_empty()
    {
        return Some(value);
    }
    config_value
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            build_value
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string)
        })
}

#[cfg(test)]
pub(crate) struct TestEnv {
    _lock: tokio::sync::MutexGuard<'static, ()>,
    saved: Vec<(std::ffi::OsString, std::ffi::OsString)>,
}

#[cfg(test)]
impl TestEnv {
    pub(crate) async fn clear() -> Self {
        static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
        let lock = LOCK.lock().await;
        let saved: Vec<_> = env::vars_os()
            .filter(|(name, _)| name.to_string_lossy().starts_with("KITE_"))
            .collect();
        for (name, _) in &saved {
            unsafe { env::remove_var(name) };
        }
        Self { _lock: lock, saved }
    }
}

#[cfg(test)]
impl Drop for TestEnv {
    fn drop(&mut self) {
        for (name, _) in
            env::vars_os().filter(|(name, _)| name.to_string_lossy().starts_with("KITE_"))
        {
            unsafe { env::remove_var(name) };
        }
        for (name, value) in &self.saved {
            unsafe { env::set_var(name, value) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CONFIG_TMP_ATTEMPTS, ConfigFile, OnceLock, ResolvedCredentials, Runtime, SESSION_FIELDS,
        TestEnv, config_tmp_path, replace_private_file, resolve_config_value,
    };
    use crate::app_api::trusted::Session;
    use kiteconnect::Request;

    #[test]
    fn invalid_config_message_survives_sanitization() {
        let dir = TestDir::new("invalid-config");
        let path = dir.0.join("config.json");
        std::fs::write(&path, "{invalid json").unwrap();
        let rt = Runtime {
            json: false,
            config: Some(path),
            app_uuid: OnceLock::new(),
        };
        let err = rt.load_config().unwrap_err();
        assert_eq!(
            crate::app::errors::format_error(&err),
            "invalid config JSON; expected api_root/api_key/ticker_url/app_uuid/access_token/user_id fields: key must be a string at line 1 column 2"
        );
    }

    #[test]
    fn config_file_debug_redacts_secrets_but_preserves_none() {
        let cfg = ConfigFile {
            user_id: Some("AB1234".to_string()),
            access_token: Some("SECRETVAL123".to_string()),
            api_key: Some("test-key".to_string()),
            api_root: Some("https://example.invalid".to_string()),
            app_uuid: None,
            ticker_url: None,
        };
        let out = format!("{cfg:?}");
        assert!(!out.contains("SECRETVAL123"), "access_token leaked: {out}");
        assert!(out.contains("access_token: Some(\"<redacted>\")"));
        assert!(out.contains("AB1234"));
    }

    #[test]
    fn resolved_credentials_debug_redacts_secrets_but_preserves_none() {
        let resolved = ResolvedCredentials {
            path: std::env::temp_dir().join("config.json"),
            config_exists: true,
            user_id: Some("AB1234".to_string()),
            access_token: Some("SECRETVAL123".to_string()),
            api_key: Some("test-key".to_string()),
            api_root: Some("https://example.invalid".to_string()),
            app_uuid: None,
            ticker_url: None,
        };
        let out = format!("{resolved:?}");
        assert!(!out.contains("SECRETVAL123"), "access_token leaked: {out}");
        assert!(out.contains("access_token: Some(\"<redacted>\")"));
        assert!(out.contains("AB1234"));
    }

    #[cfg(unix)]
    #[test]
    fn save_config_forces_0600_even_over_existing_0644_file() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!(
            "kite-cfg-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");

        std::fs::write(&path, "{}\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let before = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(before, 0o644);

        let rt = Runtime {
            json: false,
            config: Some(path.clone()),
            app_uuid: OnceLock::new(),
        };
        let cfg = ConfigFile {
            user_id: Some("AB1234".to_string()),
            ..ConfigFile::default()
        };
        let saved = rt.save_config(&cfg).unwrap();
        assert_eq!(saved, path);

        let after = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(after, 0o600);

        let (_, exists, loaded) = rt.load_config().unwrap();
        assert!(exists);
        assert_eq!(loaded.user_id.as_deref(), Some("AB1234"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn login_and_logout_keep_the_config_schema_and_api_key() {
        let dir = std::env::temp_dir().join(format!(
            "kite-session-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(
            &path,
            r#"{"api_root":"https://example.invalid","api_key":"test-key","refresh_token":"old-refresh","user_id":"OLD1","enctoken":"old-enc","kf_session":"old-cookie","app_uuid":"5b697354-64d2-49c4-a080-6b25475555e3","public_token":"old-public"}"#,
        )
        .unwrap();
        let rt = Runtime {
            json: false,
            config: Some(path.clone()),
            app_uuid: OnceLock::new(),
        };
        let (_, exists, old) = rt.load_config().unwrap();
        assert!(exists);
        assert_eq!(old.access_token, None);

        let session = Session {
            access_token: "new-enc".to_string(),
            user_id: "AB1234".to_string(),
            user_name: Some("Customer A".to_string()),
        };
        assert_eq!(rt.save_session(&session).unwrap(), path);
        let saved: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            saved,
            serde_json::json!({
                "api_root": "https://example.invalid",
                "user_id": "AB1234",
                "access_token": "new-enc",
                "api_key": "test-key",
                "app_uuid": "5b697354-64d2-49c4-a080-6b25475555e3",
            })
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }

        rt.clear_session().unwrap();
        let cleared: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        for field in SESSION_FIELDS {
            assert_eq!(cleared[field], serde_json::Value::Null, "{field} cleared");
        }
        assert_eq!(SESSION_FIELDS, ["access_token"]);
        assert_eq!(
            cleared,
            serde_json::json!({"api_root":"https://example.invalid","api_key":"test-key","user_id":"AB1234","access_token":null,"app_uuid":"5b697354-64d2-49c4-a080-6b25475555e3"})
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn client_uses_token_headers_and_the_supplied_root() {
        let _env = TestEnv::clear().await;
        let server = httpmock::MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(httpmock::Method::GET)
                    .path("/user/profile")
                    .header("authorization", "token test-key:TOKEN")
                    .header("X-Kite-Version", "3")
                    .header("user-agent", crate::app::http::user_agent())
                    .header("x-kite-app-uuid", "5b697354-64d2-49c4-a080-6b25475555e3")
                    .header_missing("cookie");
                then.status(200)
                    .json_body(serde_json::json!({"status":"success","data":{"user_id":"AB1234"}}));
            })
            .await;
        let dir = TestDir::new("runtime-client");
        let rt = Runtime {
            json: false,
            config: Some(dir.0.join("config.json")),
            app_uuid: OnceLock::new(),
        };
        rt.save_config(&ConfigFile {
            api_key: Some("test-key".into()),
            access_token: Some("TOKEN".into()),
            api_root: Some(server.base_url()),
            app_uuid: Some("5b697354-64d2-49c4-a080-6b25475555e3".into()),
            ..Default::default()
        })
        .unwrap();
        let data = rt
            .client()
            .unwrap()
            .call(Request::get("/user/profile"))
            .raw()
            .await
            .unwrap();
        assert_eq!(data, serde_json::json!({"user_id":"AB1234"}));
        mock.assert_async().await;
    }

    #[test]
    fn app_uuid_is_stable_without_writing_config_and_login_persists_it() {
        let dir = TestDir::new("app-uuid");
        let path = dir.0.join("config.json");
        let rt = Runtime {
            json: false,
            config: Some(path.clone()),
            app_uuid: OnceLock::new(),
        };
        let first = rt.app_uuid().unwrap();
        assert!(uuid::Uuid::parse_str(&first).is_ok());
        assert_eq!(rt.app_uuid().unwrap(), first);
        assert!(!path.exists());
        std::fs::write(&path, r#"{"app_uuid":"invalid","user_id":"AB1234"}"#).unwrap();
        assert_eq!(rt.app_uuid().unwrap(), first);
        rt.save_session(&Session {
            user_id: "AB1234".into(),
            access_token: "test-token".into(),
            user_name: None,
        })
        .unwrap();
        assert_eq!(
            rt.load_config().unwrap().2.app_uuid.as_deref(),
            Some(first.as_str())
        );
    }

    #[test]
    fn login_save_writes_only_identity_and_access_token_without_adding_overrides() {
        let dir = TestDir::new("login-keys");
        let rt = Runtime {
            json: false,
            config: Some(dir.0.join("config.json")),
            app_uuid: OnceLock::new(),
        };
        let session = Session {
            user_id: "AB1234".into(),
            access_token: "access".into(),
            user_name: Some("Test User".into()),
        };
        let path = rt.save_session(&session).unwrap();
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let keys: Vec<_> = saved
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys, ["access_token", "app_uuid", "user_id"]);
        rt.clear_session().unwrap();
        let config = rt.load_config().unwrap().2;
        assert_eq!(config.access_token, None);
        assert_eq!(config.user_id.as_deref(), Some("AB1234"));
        assert_eq!(config.app_uuid.as_deref(), saved["app_uuid"].as_str());
    }

    #[tokio::test]
    async fn expired_command_requires_login_without_refresh_or_retry() {
        let _env = TestEnv::clear().await;
        let server = httpmock::MockServer::start_async().await;
        let rejected = server
            .mock_async(|when, then| {
                when.method(httpmock::Method::GET).path("/orders");
                then.status(403).json_body(serde_json::json!({
                    "status":"error", "error_type":"TokenException", "message":"expired"
                }));
            })
            .await;
        let refresh = server
            .mock_async(|when, then| {
                when.method(httpmock::Method::POST)
                    .path("/trusted/test-key/session/refresh_token");
                then.status(500);
            })
            .await;
        let dir = TestDir::new("expired-command");
        let rt = Runtime {
            json: false,
            config: Some(dir.0.join("config.json")),
            app_uuid: OnceLock::new(),
        };
        rt.save_config(&ConfigFile {
            api_root: Some(server.base_url()),
            api_key: Some("test-key".into()),
            access_token: Some("OLD_ACCESS".into()),
            ..ConfigFile::default()
        })
        .unwrap();
        let err = rt.with_session(|api| api.orders().raw()).await.unwrap_err();
        assert_eq!(crate::app::errors::error_type(&err), "TokenException");
        rejected.assert_calls_async(1).await;
        refresh.assert_calls_async(0).await;
    }

    #[tokio::test]
    async fn env_wins_over_config_when_both_set() {
        let _env = TestEnv::clear().await;
        unsafe {
            std::env::set_var("KITE_CONFIG_TEST_ENV_WINS", "env-value");
        }
        let resolved = resolve_config_value(
            "KITE_CONFIG_TEST_ENV_WINS",
            Some("cfg-value".into()),
            Some("build-value"),
        );
        assert_eq!(resolved.as_deref(), Some("env-value"));
    }

    #[tokio::test]
    async fn blank_env_falls_back_to_config() {
        let _env = TestEnv::clear().await;
        unsafe {
            std::env::set_var("KITE_CONFIG_TEST_BLANK_ENV", "   ");
        }
        let resolved = resolve_config_value(
            "KITE_CONFIG_TEST_BLANK_ENV",
            Some("cfg-value".into()),
            Some("build-value"),
        );
        assert_eq!(resolved.as_deref(), Some("cfg-value"));
    }

    #[test]
    fn falls_back_to_config_when_env_unset() {
        let value = resolve_config_value(
            "KITE_CONFIG_TEST_UNSET",
            Some("config-value".to_string()),
            Some("build-value"),
        );
        assert_eq!(value.as_deref(), Some("config-value"));
    }

    #[test]
    fn absent_or_blank_config_falls_back_to_build_value() {
        for value in [None, Some(" ".to_string())] {
            assert_eq!(
                resolve_config_value("KITE_CONFIG_TEST_BUILD_DEFAULT", value, Some("build-value")),
                Some("build-value".to_string())
            );
        }
    }

    #[test]
    fn absent_or_blank_build_value_stays_unset() {
        for value in [None, Some(" ")] {
            assert_eq!(
                resolve_config_value("KITE_CONFIG_TEST_MISSING", None, value),
                None
            );
        }
    }

    #[test]
    fn missing_api_values_give_configuration_errors() {
        let creds = ResolvedCredentials {
            path: "/unused/config.json".into(),
            config_exists: false,
            api_root: None,
            api_key: None,
            user_id: None,
            access_token: None,
            app_uuid: None,
            ticker_url: None,
        };
        assert_eq!(
            creds.api_root().unwrap_err().to_string(),
            "no API root configured; set KITE_API_ROOT or \"api_root\" in config.json"
        );
        assert_eq!(
            creds.ticker_url().unwrap_err().to_string(),
            "no ticker URL configured; set KITE_TICKER_URL or \"ticker_url\" in config.json"
        );
        assert_eq!(
            creds.api_key().unwrap_err().to_string(),
            "no API key configured; set KITE_API_KEY or \"api_key\" in config.json"
        );
    }

    #[tokio::test]
    async fn ticker_url_resolves_environment_config_and_build_values() {
        let _env = TestEnv::clear().await;
        assert_eq!(
            resolve_config_value("KITE_TICKER_URL", None, Some("wss://example.invalid")),
            Some("wss://example.invalid".into())
        );
        assert_eq!(
            resolve_config_value(
                "KITE_TICKER_URL",
                Some("wss://config.example.invalid".into()),
                Some("wss://example.invalid")
            ),
            Some("wss://config.example.invalid".into())
        );
        unsafe {
            std::env::set_var("KITE_TICKER_URL", "wss://env.example.invalid");
        }
        assert_eq!(
            resolve_config_value(
                "KITE_TICKER_URL",
                Some("wss://config.example.invalid".into()),
                Some("wss://example.invalid")
            ),
            Some("wss://env.example.invalid".into())
        );
        unsafe {
            std::env::set_var("KITE_TICKER_URL", " ");
        }
        assert_eq!(resolve_config_value("KITE_TICKER_URL", None, None), None);
    }

    /// A fresh directory under the system temp dir, removed on drop.
    struct TestDir(std::path::PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            loop {
                let seq = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let dir =
                    std::env::temp_dir().join(format!("kite-{label}-{}-{seq}", std::process::id()));
                match std::fs::create_dir(&dir) {
                    Ok(()) => return Self(dir),
                    Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(err) => panic!("create {}: {err}", dir.display()),
                }
            }
        }

        fn entries(&self) -> Vec<String> {
            let mut names: Vec<String> = std::fs::read_dir(&self.0)
                .unwrap()
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn sequence_from(start: u64, used: &std::cell::Cell<u64>) -> impl FnMut() -> u64 + '_ {
        move || {
            used.set(used.get() + 1);
            start + used.get() - 1
        }
    }

    fn tmp_name(path: &std::path::Path, seq: u64) -> String {
        config_tmp_path(path, seq)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn save_skips_an_occupied_temp_name_without_touching_it() {
        let dir = TestDir::new("cfg-occupied");
        let path = dir.0.join("config.json");
        let occupied = config_tmp_path(&path, 0);
        std::fs::write(&occupied, "someone else's bytes").unwrap();

        let used = std::cell::Cell::new(0);
        replace_private_file(
            &path,
            b"{\"user_id\":\"AB1234\"}\n",
            sequence_from(0, &used),
        )
        .unwrap();

        assert_eq!(used.get(), 2, "the second candidate was used");
        assert_eq!(
            std::fs::read_to_string(&occupied).unwrap(),
            "someone else's bytes"
        );
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{\"user_id\":\"AB1234\"}\n"
        );
        let mut expected = vec!["config.json".to_string(), tmp_name(&path, 0)];
        expected.sort();
        assert_eq!(dir.entries(), expected);
    }

    #[cfg(unix)]
    #[test]
    fn save_does_not_follow_a_symlink_at_the_temp_name() {
        let dir = TestDir::new("cfg-symlink");
        let path = dir.0.join("config.json");
        let victim = dir.0.join("victim");
        std::fs::write(&victim, "victim bytes").unwrap();
        let planted = config_tmp_path(&path, 7);
        std::os::unix::fs::symlink(&victim, &planted).unwrap();
        let dangling = config_tmp_path(&path, 8);
        std::os::unix::fs::symlink(dir.0.join("absent"), &dangling).unwrap();

        let used = std::cell::Cell::new(0);
        replace_private_file(&path, b"{}\n", sequence_from(7, &used)).unwrap();

        assert_eq!(used.get(), 3);
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "victim bytes");
        assert!(
            !dir.0.join("absent").exists(),
            "dangling symlink not followed"
        );
        assert!(
            std::fs::symlink_metadata(&planted)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}\n");
    }

    #[test]
    fn save_gives_up_after_bounded_temp_name_collisions() {
        let dir = TestDir::new("cfg-exhausted");
        let path = dir.0.join("config.json");
        std::fs::write(&path, "old\n").unwrap();
        std::fs::write(config_tmp_path(&path, 0), "taken").unwrap();

        let mut calls = 0;
        let err = replace_private_file(&path, b"new\n", || {
            calls += 1;
            0
        })
        .unwrap_err();

        assert_eq!(calls, CONFIG_TMP_ATTEMPTS);
        assert!(
            err.to_string().contains("temp file names were taken"),
            "{err}"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "old\n");
        assert_eq!(
            std::fs::read_to_string(config_tmp_path(&path, 0)).unwrap(),
            "taken"
        );
    }

    #[test]
    fn failed_rename_leaves_the_target_and_removes_its_own_temp() {
        let dir = TestDir::new("cfg-rename");
        let path = dir.0.join("config.json");
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("keep"), "kept").unwrap();
        let rt = Runtime {
            json: false,
            config: Some(path.clone()),
            app_uuid: OnceLock::new(),
        };

        let err = rt.save_config(&ConfigFile::default()).unwrap_err();

        assert!(
            err.to_string().contains("failed to persist config"),
            "{err}"
        );
        assert_eq!(std::fs::read_to_string(path.join("keep")).unwrap(), "kept");
        assert_eq!(dir.entries(), vec!["config.json".to_string()]);
    }

    #[test]
    fn concurrent_saves_publish_one_complete_config_and_leave_no_temps() {
        let dir = TestDir::new("cfg-concurrent");
        let path = dir.0.join("config.json");
        let users: Vec<String> = (0..16).map(|n| format!("AB{n:04}")).collect();

        std::thread::scope(|scope| {
            for user in &users {
                let path = path.clone();
                scope.spawn(move || {
                    let rt = Runtime {
                        json: false,
                        config: Some(path),
                        app_uuid: OnceLock::new(),
                    };
                    for _ in 0..8 {
                        let cfg = ConfigFile {
                            user_id: Some(user.clone()),
                            access_token: Some("synthetic-enc".repeat(64)),
                            ..ConfigFile::default()
                        };
                        rt.save_config(&cfg).unwrap();
                    }
                });
            }
        });

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.ends_with("}\n"));
        let saved: ConfigFile = serde_json::from_str(&text).unwrap();
        assert!(users.contains(saved.user_id.as_ref().unwrap()));
        assert_eq!(
            saved.access_token.as_deref(),
            Some("synthetic-enc".repeat(64).as_str())
        );
        assert_eq!(dir.entries(), vec!["config.json".to_string()]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }
    #[cfg(unix)]
    #[test]
    fn save_preserves_the_config_symlink_and_updates_its_target() {
        let dir = TestDir::new("cfg-target-symlink");
        let target = dir.0.join("actual.json");
        let path = dir.0.join("config.json");
        std::fs::write(&target, "old").unwrap();
        std::os::unix::fs::symlink("actual.json", &path).unwrap();
        replace_private_file(&path, b"new", || 0).unwrap();
        assert!(
            std::fs::symlink_metadata(&path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "new");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
    }
}
