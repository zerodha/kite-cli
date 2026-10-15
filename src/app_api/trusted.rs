use anyhow::{Context, Result, anyhow};
use kiteconnect::error::{ApiError, ErrorKind};
use kiteconnect::{Error, KiteConnect, Request};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use url::Url;

pub struct TrustedAuth {
    client: KiteConnect,
    root: Url,
}

#[derive(Debug, Clone)]
pub struct TwofaChallenge {
    pub user_id: String,
    pub request_id: String,
    pub twofa_type: String,
    pub twofa_types: Vec<String>,
    pub user_name: Option<String>,
}

#[derive(Clone)]
pub struct Session {
    pub user_id: String,
    pub access_token: String,
    pub user_name: Option<String>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("user_id", &self.user_id)
            .field("access_token", &"<redacted>")
            .field("user_name", &self.user_name)
            .finish()
    }
}

impl TrustedAuth {
    /// `root` is the API root, or a mock server URL in tests.
    ///
    /// # Errors
    /// Returns an error if the root URL is invalid or the HTTP client cannot be built.
    pub fn new(root: &str, api_key: &str, app_uuid: &str) -> Result<Self> {
        Ok(Self {
            client: KiteConnect::builder(api_key)
                .base_url(root)
                .user_agent(crate::app::http::user_agent())
                .header("X-Kite-App-UUID", app_uuid)
                .build()?,
            root: Url::parse(root).context("invalid login root URL")?,
        })
    }

    #[must_use]
    pub fn api_key(&self) -> &str {
        self.client.api_key()
    }

    /// # Errors
    /// Returns an error if the server rejects login or the request or response fails.
    pub async fn login(&self, user_id: &str, password: &str) -> Result<TwofaChallenge> {
        let path = ["trusted", self.api_key(), "user", user_id, "login"];
        let data: LoginData = self.post(&path, &[("password", password)]).await?;
        Ok(TwofaChallenge {
            user_id: data.user_id,
            request_id: data.request_id,
            twofa_type: data.twofa_type,
            twofa_types: data.twofa_types,
            user_name: data.profile.and_then(|profile| profile.user_name),
        })
    }

    /// Asks the server to send an SMS/email OTP for the `sms` twofa type.
    ///
    /// # Errors
    /// Returns an error if OTP generation is rejected or the request or response fails.
    pub async fn send_otp(&self, challenge: &TwofaChallenge) -> Result<()> {
        let path = [
            "trusted",
            self.api_key(),
            "user",
            &challenge.user_id,
            "twofa",
            "generate_otp",
        ];
        let form = [
            ("request_id", challenge.request_id.as_str()),
            ("twofa_type", "sms"),
        ];
        self.post::<Value>(&path, &form).await.map(drop)
    }

    /// # Errors
    /// Returns an error if verification is rejected or the request or response fails.
    pub async fn twofa(
        &self,
        challenge: &TwofaChallenge,
        twofa_type: &str,
        value: &str,
    ) -> Result<Session> {
        let path = [
            "trusted",
            self.api_key(),
            "user",
            &challenge.user_id,
            "twofa",
        ];
        let form = [
            ("request_id", challenge.request_id.as_str()),
            ("twofa_type", twofa_type),
            ("twofa_value", value),
        ];
        let data: SessionData = self.post(&path, &form).await?;
        Ok(Session {
            user_id: challenge.user_id.clone(),
            access_token: data.access_token,
            user_name: data.user_name,
        })
    }

    /// # Errors
    /// Returns an error if invalidation is rejected or the request or response fails.
    pub async fn invalidate(&self, access_token: &str) -> Result<()> {
        let request = Request::delete("/session/token")
            .query([("api_key", self.api_key()), ("access_token", access_token)]);
        self.send::<Value>(request).await.map(drop)
    }

    async fn post<T: DeserializeOwned>(&self, path: &[&str], form: &[(&str, &str)]) -> Result<T> {
        let path = self.url(path)?.path().to_string();
        self.send(Request::post(path).form(form.iter().copied()))
            .await
    }

    /// The server can answer an error with HTTP 200, so the envelope status is
    /// checked here as well as the HTTP status in the client.
    async fn send<T: DeserializeOwned>(&self, request: Request) -> Result<T> {
        let response = match self.client.call(request).response().await {
            Ok(response) => response,
            Err(err @ Error::Transport(_)) => {
                return Err(anyhow::Error::new(err).context("Kite login request failed"));
            }
            Err(err) => return Err(err.into()),
        };
        let http_status = response.status();
        let envelope: LoginEnvelope = serde_json::from_slice(response.body())
            .map_err(|_| anyhow!("unexpected Kite login response (HTTP {http_status})"))?;
        if envelope.status != "success" {
            return Err(Error::Api(ApiError::new(
                ErrorKind::from(envelope.error_type.as_deref().unwrap_or("Unknown")),
                envelope
                    .message
                    .unwrap_or_else(|| "login failed".to_string()),
                http_status,
                None,
            ))
            .into());
        }
        let data = envelope.data.context("Kite login response had no data")?;
        serde_json::from_value(data)
            .map_err(|_| anyhow!("unexpected Kite login response (HTTP {http_status})"))
    }

    fn url(&self, segments: &[&str]) -> Result<Url> {
        let mut url = self.root.clone();
        url.path_segments_mut()
            .map_err(|()| anyhow!("login root URL cannot take a path"))?
            .clear()
            .extend(segments);
        Ok(url)
    }
}

#[derive(Deserialize)]
struct LoginEnvelope {
    status: String,
    data: Option<Value>,
    message: Option<String>,
    error_type: Option<String>,
}

#[derive(Deserialize)]
struct LoginProfile {
    user_name: Option<String>,
}

#[derive(Deserialize)]
struct LoginData {
    user_id: String,
    request_id: String,
    twofa_type: String,
    twofa_types: Vec<String>,
    profile: Option<LoginProfile>,
}

#[derive(Deserialize)]
struct SessionData {
    access_token: String,
    user_name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::Method::{DELETE, POST};
    use httpmock::MockServer;
    use serde_json::json;

    fn challenge() -> TwofaChallenge {
        TwofaChallenge {
            user_id: "AB1234".to_string(),
            request_id: "request-123".to_string(),
            twofa_type: "totp".to_string(),
            twofa_types: vec!["totp".to_string(), "sms".to_string()],
            user_name: Some("Test User".to_string()),
        }
    }

    #[tokio::test]
    async fn login_posts_password_and_parses_challenge() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/trusted/test-key/user/AB1234/login")
                    .header("X-Kite-Version", "3")
                    .header("user-agent", crate::app::http::user_agent())
                    .header("x-kite-app-uuid", "5b697354-64d2-49c4-a080-6b25475555e3")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .header_missing("cookie")
                    .form_urlencoded_tuple("password", "SENTINEL PASSWORD&+");
                then.status(200).json_body(json!({
                    "status": "success",
                    "data": {
                        "user_id": "AB1234",
                        "request_id": "request-123",
                        "twofa_type": "totp",
                        "twofa_types": ["totp", "sms"],
                        "profile": {"user_name": "Test User"}
                    }
                }));
            })
            .await;
        let auth = TrustedAuth::new(
            &server.base_url(),
            "test-key",
            "5b697354-64d2-49c4-a080-6b25475555e3",
        )
        .unwrap();
        assert_eq!(auth.api_key(), "test-key");
        let result = auth.login("AB1234", "SENTINEL PASSWORD&+").await.unwrap();
        assert_eq!(result.user_id, "AB1234");
        assert_eq!(result.request_id, "request-123");
        assert_eq!(result.twofa_type, "totp");
        assert_eq!(result.twofa_types, ["totp", "sms"]);
        assert_eq!(result.user_name.as_deref(), Some("Test User"));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn login_surfaces_user_exception_without_password() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/trusted/test-key/user/AB1234/login");
                then.status(403).json_body(json!({
                    "status": "error",
                    "message": "Invalid username or password.",
                    "error_type": "UserException"
                }));
            })
            .await;
        let auth = TrustedAuth::new(
            &server.base_url(),
            "test-key",
            "5b697354-64d2-49c4-a080-6b25475555e3",
        )
        .unwrap();
        let error = auth.login("AB1234", "SENTINEL_PASSWORD").await.unwrap_err();
        assert_eq!(
            error.to_string(),
            "kite API error UserException (HTTP 403): Invalid username or password."
        );
        assert!(matches!(
            error.downcast_ref::<Error>(),
            Some(Error::Api(ApiError { status: 403, .. }))
        ));
        assert!(!format!("{error:?}").contains("SENTINEL_PASSWORD"));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn twofa_posts_challenge_and_returns_tokens() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/trusted/test-key/user/AB1234/twofa")
                    .header("X-Kite-Version", "3")
                    .header("user-agent", crate::app::http::user_agent())
                    .header("x-kite-app-uuid", "5b697354-64d2-49c4-a080-6b25475555e3")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .form_urlencoded_tuple("request_id", "request-123")
                    .form_urlencoded_tuple("twofa_type", "sms")
                    .form_urlencoded_tuple("twofa_value", "123456");
                then.status(200).json_body(json!({
                    "status": "success",
                    "data": {
                        "access_token": "ACCESS_SENTINEL",
                        "refresh_token": "REFRESH_SENTINEL",
                        "user_name": "Test User"
                    }
                }));
            })
            .await;
        let auth = TrustedAuth::new(
            &server.base_url(),
            "test-key",
            "5b697354-64d2-49c4-a080-6b25475555e3",
        )
        .unwrap();
        let result = auth.twofa(&challenge(), "sms", "123456").await.unwrap();
        assert_eq!(result.user_id, "AB1234");
        assert_eq!(result.access_token, "ACCESS_SENTINEL");
        assert_eq!(result.user_name.as_deref(), Some("Test User"));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn send_otp_posts_request_id_and_sms_type() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/trusted/test-key/user/AB1234/twofa/generate_otp")
                    .header("X-Kite-Version", "3")
                    .header("user-agent", crate::app::http::user_agent())
                    .header("x-kite-app-uuid", "5b697354-64d2-49c4-a080-6b25475555e3")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .form_urlencoded_tuple("request_id", "request-123")
                    .form_urlencoded_tuple("twofa_type", "sms");
                then.status(200).json_body(json!({
                    "status": "success",
                    "data": {"refresh_interval": 30, "attempts_remaining": 3}
                }));
            })
            .await;
        let auth = TrustedAuth::new(
            &server.base_url(),
            "test-key",
            "5b697354-64d2-49c4-a080-6b25475555e3",
        )
        .unwrap();
        auth.send_otp(&challenge()).await.unwrap();
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn invalidate_deletes_token_with_query_parameters() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(DELETE)
                    .path("/session/token")
                    .header("X-Kite-Version", "3")
                    .header("user-agent", crate::app::http::user_agent())
                    .header("x-kite-app-uuid", "5b697354-64d2-49c4-a080-6b25475555e3")
                    .query_param("api_key", "key&+")
                    .query_param("access_token", "ACCESS_SENTINEL&+");
                then.status(200)
                    .json_body(json!({"status": "success", "data": true}));
            })
            .await;
        let auth = TrustedAuth::new(
            &server.base_url(),
            "key&+",
            "5b697354-64d2-49c4-a080-6b25475555e3",
        )
        .unwrap();
        auth.invalidate("ACCESS_SENTINEL&+").await.unwrap();
        mock.assert_async().await;
    }

    #[test]
    fn session_debug_redacts_access_token() {
        let session = Session {
            user_id: "AB1234".to_string(),
            access_token: "ACCESS_SENTINEL".to_string(),
            user_name: Some("Test User".to_string()),
        };
        for output in [format!("{session:?}"), format!("{session:#?}")] {
            assert!(!output.contains("ACCESS_SENTINEL"));
            assert!(!output.contains("REFRESH_SENTINEL"));
            assert!(output.contains("AB1234"));
            assert!(output.contains("Test User"));
        }
    }

    #[tokio::test]
    async fn user_and_api_key_are_encoded_as_path_segments() {
        let server = MockServer::start_async().await;
        let login = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/trusted/key%2F%3F%23%25/user/user%2F%3F%23%25/login");
                then.status(200).json_body(json!({
                    "status": "success",
                    "data": {
                        "user_id": "challenge/?#%",
                        "request_id": "request-123",
                        "twofa_type": "sms",
                        "twofa_types": ["sms"]
                    }
                }));
            })
            .await;
        let otp = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/trusted/key%2F%3F%23%25/user/challenge%2F%3F%23%25/twofa/generate_otp");
                then.status(200)
                    .json_body(json!({"status": "success", "data": {}}));
            })
            .await;
        let twofa = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/trusted/key%2F%3F%23%25/user/challenge%2F%3F%23%25/twofa");
                then.status(200).json_body(json!({
                    "status": "success",
                    "data": {"access_token": "access", "refresh_token": "refresh"}
                }));
            })
            .await;
        let auth = TrustedAuth::new(
            &server.base_url(),
            "key/?#%",
            "5b697354-64d2-49c4-a080-6b25475555e3",
        )
        .unwrap();
        let challenge = auth.login("user/?#%", "password").await.unwrap();
        assert_eq!(challenge.user_name, None);
        auth.send_otp(&challenge).await.unwrap();
        let result = auth.twofa(&challenge, "sms", "123456").await.unwrap();
        assert_eq!(result.user_id, "challenge/?#%");
        login.assert_async().await;
        otp.assert_async().await;
        twofa.assert_async().await;
    }

    #[tokio::test]
    async fn success_http_status_still_surfaces_twofa_exception() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/trusted/test-key/user/AB1234/twofa");
                then.status(200).json_body(json!({
                    "status": "error",
                    "message": "Invalid TOTP.",
                    "error_type": "TwoFAException"
                }));
            })
            .await;
        let auth = TrustedAuth::new(
            &server.base_url(),
            "test-key",
            "5b697354-64d2-49c4-a080-6b25475555e3",
        )
        .unwrap();
        let error = auth
            .twofa(&challenge(), "totp", "123456")
            .await
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "kite API error TwoFAException (HTTP 200): Invalid TOTP."
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn an_error_envelope_without_a_message_reads_login_failed() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/trusted/test-key/user/AB1234/login");
                then.status(200)
                    .json_body(json!({"status": "error", "error_type": "UserException"}));
            })
            .await;
        let auth = TrustedAuth::new(
            &server.base_url(),
            "test-key",
            "5b697354-64d2-49c4-a080-6b25475555e3",
        )
        .unwrap();
        let error = auth.login("AB1234", "password").await.unwrap_err();
        assert_eq!(
            error.to_string(),
            "kite API error UserException (HTTP 200): login failed"
        );
    }

    #[tokio::test]
    async fn malformed_success_bodies_stay_out_of_errors() {
        for (status, body) in [
            (
                200,
                r#"{"status":"success","data":{"access_token":123,"user_name":"ACCESS_SENTINEL"}}"#,
            ),
            (200, "invalid JSON ACCESS_SENTINEL"),
        ] {
            let server = MockServer::start_async().await;
            let mock = server
                .mock_async(|when, then| {
                    when.method(POST)
                        .path("/trusted/test-key/user/AB1234/twofa");
                    then.status(status).body(body);
                })
                .await;
            let auth = TrustedAuth::new(
                &server.base_url(),
                "test-key",
                "5b697354-64d2-49c4-a080-6b25475555e3",
            )
            .unwrap();
            let error = auth
                .twofa(&challenge(), "totp", "TWOFA_SENTINEL")
                .await
                .unwrap_err();
            for output in [error.to_string(), format!("{error:?}")] {
                assert!(!output.contains("TWOFA_SENTINEL"));
                assert!(!output.contains("ACCESS_SENTINEL"));
            }
            mock.assert_async().await;
        }
    }

    #[tokio::test]
    async fn status_error_with_data_keeps_server_message() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/trusted/test-key/user/AB1234/twofa");
                then.status(200).body(
                    r#"{"status":"error","message":"Invalid TOTP.","error_type":"TwoFAException","data":{}}"#,
                );
            })
            .await;
        let auth = TrustedAuth::new(
            &server.base_url(),
            "test-key",
            "5b697354-64d2-49c4-a080-6b25475555e3",
        )
        .unwrap();
        let error = auth
            .twofa(&challenge(), "totp", "TWOFA_SENTINEL")
            .await
            .unwrap_err();
        let Some(Error::Api(ApiError { kind, message, .. })) = error.downcast_ref::<Error>() else {
            panic!("expected an API error, got {error:#}");
        };
        assert_eq!(kind.as_str(), "TwoFAException");
        assert_eq!(message, "Invalid TOTP.");
    }

    #[tokio::test]
    async fn invalidate_transport_error_does_not_expose_query_token() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let auth = TrustedAuth::new(
            &format!("http://{address}"),
            "test-key",
            "5b697354-64d2-49c4-a080-6b25475555e3",
        )
        .unwrap();
        let error = auth.invalidate("ACCESS_SENTINEL").await.unwrap_err();
        assert_eq!(error.to_string(), "Kite login request failed");
        assert!(!error.to_string().contains("ACCESS_SENTINEL"));
        assert!(!format!("{error:?}").contains("ACCESS_SENTINEL"));
    }
}
