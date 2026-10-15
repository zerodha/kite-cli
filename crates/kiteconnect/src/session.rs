use crate::secret::deserialize_optional_refresh_token;
use crate::{AccessToken, ApiSecret, Call, KiteConnect, RefreshToken, Request, UserProfile};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// Session returned by POST /session/token.
#[derive(Clone, Deserialize, Serialize)]
#[non_exhaustive]
pub struct UserSession {
    /// User profile of the session.
    #[serde(flatten)]
    pub profile: UserProfile,
    /// API key of the app.
    pub api_key: String,
    /// Public token of the session.
    pub public_token: String,
    /// Access token for authenticated requests.
    pub access_token: AccessToken,
    /// `None` when the app has no refresh-token access.
    #[serde(default, deserialize_with = "deserialize_optional_refresh_token")]
    pub refresh_token: Option<RefreshToken>,
    /// Time the session was created.
    #[serde(with = "crate::time::datetime")]
    pub login_time: NaiveDateTime,
}

impl fmt::Debug for UserSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserSession")
            .field("profile", &self.profile)
            .field("api_key", &"<redacted>")
            .field("public_token", &"<redacted>")
            .field("access_token", &self.access_token)
            .field("refresh_token", &self.refresh_token)
            .field("login_time", &self.login_time)
            .finish()
    }
}

/// Tokens returned by `POST /session/refresh_token`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[non_exhaustive]
pub struct UserSessionTokens {
    /// User ID.
    pub user_id: String,
    /// New access token.
    pub access_token: AccessToken,
    /// New refresh token.
    pub refresh_token: RefreshToken,
}

fn checksum(api_key: &str, token: &str, secret: &ApiSecret) -> String {
    let mut digest = Sha256::new();
    digest.update(api_key);
    digest.update(token);
    digest.update(secret.expose());
    format!("{:x}", digest.finalize())
}

impl KiteConnect {
    /// `GET https://kite.zerodha.com/connect/login` URL for the browser login.
    #[must_use]
    pub fn login_url(&self) -> String {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("api_key", self.api_key())
            .append_pair("v", "3")
            .finish();
        format!("https://kite.zerodha.com/connect/login?{query}")
    }

    /// `POST /session/token`.
    /// The caller sets the returned access token.
    pub fn generate_session(
        &self,
        request_token: &str,
        api_secret: &ApiSecret,
    ) -> Call<UserSession> {
        Call::new(
            self.clone(),
            Request::post("/session/token").form([
                ("api_key", self.api_key().to_string()),
                ("request_token", request_token.to_string()),
                (
                    "checksum",
                    checksum(self.api_key(), request_token, api_secret),
                ),
            ]),
        )
    }

    /// `POST /session/refresh_token`.
    /// The caller sets the returned access token.
    pub fn renew_access_token(
        &self,
        refresh_token: &RefreshToken,
        api_secret: &ApiSecret,
    ) -> Call<UserSessionTokens> {
        Call::new(
            self.clone(),
            Request::post("/session/refresh_token").form([
                ("api_key", self.api_key().to_string()),
                ("refresh_token", refresh_token.expose().to_string()),
                (
                    "checksum",
                    checksum(self.api_key(), refresh_token.expose(), api_secret),
                ),
            ]),
        )
    }

    /// `DELETE /session/token` with the current access token.
    pub fn invalidate_access_token(&self) -> Call<bool> {
        Call::with_decoder(
            self.clone(),
            Request::delete("/session/token").query([
                ("api_key", self.api_key()),
                (
                    "access_token",
                    self.access_token().map_or("", AccessToken::expose),
                ),
            ]),
            crate::call::success_bool,
        )
    }

    /// `DELETE /session/token` with a refresh token.
    pub fn invalidate_refresh_token(&self, refresh_token: &RefreshToken) -> Call<bool> {
        Call::with_decoder(
            self.clone(),
            Request::delete("/session/token").query([
                ("api_key", self.api_key()),
                ("refresh_token", refresh_token.expose()),
            ]),
            crate::call::success_bool,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;
    use serde_json::{Value, json};

    #[test]
    fn session_fixture_decodes_and_tokens_redact() {
        let mut data = serde_json::from_str::<Value>(include_str!(
            "../../../mock_responses/generate_session.json"
        ))
        .unwrap()["data"]
            .clone();
        let session: UserSession = serde_json::from_value(data.clone()).unwrap();
        assert_eq!(session.profile.user_id, "XX0000");
        assert_eq!(session.profile.meta.demat_consent, "physical");
        assert_eq!(session.api_key, "XXXXXX");
        assert_eq!(session.access_token.expose(), "XXXXXX");
        assert_eq!(session.public_token, "XXXXXXXX");
        assert!(session.refresh_token.is_none());
        assert_eq!(session.login_time.to_string(), "2021-01-01 16:15:14");
        assert_eq!(
            serde_json::to_value(&session).unwrap()["access_token"],
            "XXXXXX"
        );
        for field in ["access_token", "refresh_token", "public_token", "api_key"] {
            data[field] = json!(format!("SENTINEL-{field}"));
        }
        let session: UserSession = serde_json::from_value(data.clone()).unwrap();
        assert_eq!(
            session.refresh_token.as_ref().unwrap().expose(),
            "SENTINEL-refresh_token"
        );
        let tokens: UserSessionTokens = serde_json::from_value(data).unwrap();
        assert!(!format!("{session:?} {tokens:?}").contains("SENTINEL"));
    }

    #[test]
    fn login_url_encodes_api_key() {
        let client = KiteConnect::builder("key +&?").build().unwrap();
        assert_eq!(
            client.login_url(),
            "https://kite.zerodha.com/connect/login?api_key=key+%2B%26%3F&v=3"
        );
    }

    #[tokio::test]
    async fn generate_session_http_contract_does_not_set_token() {
        let server = MockServer::start_async().await;
        let mock = server.mock_async(|when, then| {
            when.method(POST).path("/session/token")
                .header_missing("authorization")
                .query_param_count(".*", ".*", 0)
                .body("api_key=key&request_token=request&checksum=e4465f58be6159d2a7f087f4292515e9d631b4f91e056b53e6f14456222ff709");
            then.body(include_str!("../../../mock_responses/generate_session.json"));
        }).await;
        let mut client = KiteConnect::builder("key")
            .base_url(server.base_url())
            .build()
            .unwrap();
        let secret = ApiSecret::new("secret").unwrap();
        let call = client.generate_session("request", &secret);
        assert!(!format!("{:?}", call.request()).contains("e4465f58"));
        let session = call.await.unwrap();
        assert!(client.access_token().is_none());
        client.set_access_token(session.access_token);
        assert_eq!(client.access_token().unwrap().expose(), "XXXXXX");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn renew_access_token_http_contract_does_not_set_token() {
        let server = MockServer::start_async().await;
        let mock = server.mock_async(|when, then| {
            when.method(POST).path("/session/refresh_token")
                .query_param_count(".*", ".*", 0)
                .body("api_key=key&refresh_token=refresh&checksum=7ab722824a8bd4228cbbf8b89f8aeccb88b4f50d684b624e151d610784a37365");
            then.json_body(json!({"status":"success","data":{"user_id":"AB1234","access_token":"new-token","refresh_token":"new-refresh"}}));
        }).await;
        let client = KiteConnect::builder("key")
            .access_token(AccessToken::new("old-token").unwrap())
            .base_url(server.base_url())
            .build()
            .unwrap();
        let tokens = client
            .renew_access_token(
                &RefreshToken::new("refresh").unwrap(),
                &ApiSecret::new("secret").unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(tokens.user_id, "AB1234");
        assert_eq!(tokens.access_token.expose(), "new-token");
        assert_eq!(tokens.refresh_token.expose(), "new-refresh");
        assert_eq!(client.access_token().unwrap().expose(), "old-token");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn invalidate_access_token_http_contract() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(DELETE)
                    .path("/session/token")
                    .body("")
                    .query_param("api_key", "key")
                    .query_param("access_token", "token")
                    .query_param_missing("refresh_token");
                then.body(include_str!("../../../mock_responses/session_logout.json"));
            })
            .await;
        let client = KiteConnect::builder("key")
            .access_token(AccessToken::new("token").unwrap())
            .base_url(server.base_url())
            .build()
            .unwrap();
        assert!(client.invalidate_access_token().await.unwrap());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn invalidate_refresh_token_http_contract() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(DELETE)
                    .path("/session/token")
                    .body("")
                    .query_param("api_key", "key")
                    .query_param("refresh_token", "refresh")
                    .query_param_missing("access_token");
                then.body(include_str!("../../../mock_responses/session_logout.json"));
            })
            .await;
        let client = KiteConnect::builder("key")
            .base_url(server.base_url())
            .build()
            .unwrap();
        assert!(
            client
                .invalidate_refresh_token(&RefreshToken::new("refresh").unwrap())
                .await
                .unwrap()
        );
        mock.assert_async().await;
    }
}
