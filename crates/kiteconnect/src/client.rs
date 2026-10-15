use crate::error::error_from_response;
use crate::http::{API_TIMEOUT, DOWNLOAD_TIMEOUT, user_agent};
use crate::secret::{AccessToken, validate};
use crate::{Body, Call, Error, RawResponse, Request, Result};
use reqwest::header::{
    ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, USER_AGENT,
};
use serde_json::Value;
use std::{fmt, sync::Arc, time::Duration};
use url::Url;

/// Kite Connect REST API root used when the builder sets no base URL.
pub const DEFAULT_BASE_URL: &str = "https://api.kite.trade";

/// Kite Connect REST client.
///
/// Cloning is cheap: clones share one HTTP connection pool. Each endpoint method
/// returns a [`Call`] that sends nothing until it is awaited.
#[derive(Clone)]
pub struct KiteConnect {
    inner: Arc<Inner>,
    access_token: Option<AccessToken>,
}

struct Inner {
    http: reqwest::Client,
    base_url: Url,
    api_key: String,
    user_agent: String,
    headers: HeaderMap,
}

impl fmt::Debug for KiteConnect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KiteConnect")
            .field("access_token", &self.access_token)
            .finish_non_exhaustive()
    }
}

/// Builder for [`KiteConnect`], created by [`KiteConnect::builder`].
#[must_use]
pub struct KiteConnectBuilder {
    api_key: String,
    access_token: Option<AccessToken>,
    base_url: String,
    timeout: Duration,
    app_name: Option<String>,
    user_agent: Option<String>,
    headers: Vec<(String, String)>,
}

impl KiteConnectBuilder {
    /// Sets the access token sent in the `Authorization` header.
    pub fn access_token(mut self, token: AccessToken) -> Self {
        self.access_token = Some(token);
        self
    }
    /// Sets the REST API root, for example a local mock server. It must have no path.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }
    /// Sets the whole-request timeout for REST calls (default 15 seconds).
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
    /// Sets an app name that the User-Agent puts before `zerodha-kiteconnect/<version>`.
    pub fn app_name(mut self, name: impl Into<String>) -> Self {
        self.app_name = Some(name.into());
        self
    }

    /// Replaces the complete User-Agent sent on REST calls and ticker handshakes.
    pub fn user_agent(mut self, value: impl Into<String>) -> Self {
        self.user_agent = Some(value.into());
        self
    }

    /// Adds a REST and ticker header; Authorization, X-Kite-Version and User-Agent remain client-controlled.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Builds the client.
    ///
    /// # Errors
    /// Returns an error if the API key is blank or invalid as an HTTP header,
    /// the base URL is invalid or contains credentials or a path, or the HTTP
    /// client cannot be initialized, or a header name or value is invalid.
    pub fn build(self) -> Result<KiteConnect> {
        validate(
            &self.api_key,
            "Kite Connect api_key is blank",
            "Kite Connect api_key contains a line break",
        )?;
        HeaderValue::from_str(&self.api_key)
            .map_err(|_| Error::InvalidInput("invalid Kite Connect api_key"))?;
        let base_url = Url::parse(&self.base_url).map_err(invalid_url)?;
        if !matches!(base_url.scheme(), "http" | "https")
            || base_url.host_str().is_none()
            || !base_url.username().is_empty()
            || base_url.password().is_some()
        {
            return Err(Error::InvalidInput(
                "base URL must be an HTTP or HTTPS host without credentials",
            ));
        }
        if base_url.path() != "/" {
            return Err(Error::InvalidInput("base URL must not have a path"));
        }
        let user_agent = self
            .user_agent
            .unwrap_or_else(|| user_agent(self.app_name.as_deref()));
        HeaderValue::from_str(&user_agent)
            .map_err(|_| Error::InvalidInput("invalid User-Agent"))?;
        let mut headers = HeaderMap::new();
        for (name, value) in self.headers {
            let name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| Error::InvalidInput("invalid header name"))?;
            let mut value = HeaderValue::from_str(&value)
                .map_err(|_| Error::InvalidInput("invalid header value"))?;
            value.set_sensitive(true);
            if name != AUTHORIZATION && name != USER_AGENT && name != "x-kite-version" {
                headers.insert(name, value);
            }
        }
        let http = reqwest::Client::builder()
            .user_agent(&user_agent)
            .default_headers(headers.clone())
            .timeout(self.timeout)
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(transport_error)?;
        Ok(KiteConnect {
            inner: Arc::new(Inner {
                http,
                base_url,
                api_key: self.api_key,
                user_agent,
                headers,
            }),
            access_token: self.access_token,
        })
    }
}

impl KiteConnect {
    /// Starts a builder for the given Kite Connect API key.
    pub fn builder(api_key: impl Into<String>) -> KiteConnectBuilder {
        KiteConnectBuilder {
            api_key: api_key.into(),
            access_token: None,
            base_url: DEFAULT_BASE_URL.to_string(),
            timeout: API_TIMEOUT,
            app_name: None,
            user_agent: None,
            headers: Vec::new(),
        }
    }

    /// Build a Kite Connect ticker with this client's credentials and User-Agent.
    #[expect(
        clippy::missing_panics_doc,
        reason = "DEFAULT_TICKER_URL is a valid constant URL"
    )]
    pub fn ticker(&self) -> crate::ticker::TickerBuilder {
        crate::ticker::TickerBuilder {
            api_key: self.api_key().to_owned(),
            access_token: self.access_token.clone(),
            url: url::Url::parse(crate::ticker::DEFAULT_TICKER_URL).expect("valid ticker URL"),
            user_agent: self.inner.user_agent.clone(),
            headers: self.inner.headers.clone(),
        }
    }

    /// Returns the API key.
    #[must_use]
    pub fn api_key(&self) -> &str {
        &self.inner.api_key
    }
    /// Returns the access token, if one is set.
    #[must_use]
    pub fn access_token(&self) -> Option<&AccessToken> {
        self.access_token.as_ref()
    }
    /// Sets the access token for later calls, for example after [`KiteConnect::generate_session`].
    pub fn set_access_token(&mut self, token: AccessToken) {
        self.access_token = Some(token);
    }
    /// Builds a call to any Kite API path, for endpoints this crate does not wrap.
    pub fn call(&self, request: Request) -> Call<Value> {
        Call::new(self.clone(), request)
    }

    pub(crate) async fn send(&self, request: &Request) -> Result<RawResponse> {
        let path = request.path().split('?').next().unwrap_or_default();
        let path = path.strip_prefix('/').unwrap_or(path);
        let public = path == "instruments"
            || path == "mf/instruments"
            || (path.starts_with("instruments/") && !path.starts_with("instruments/historical/"));
        let mut http_request = self
            .inner
            .http
            .request(
                request.method().clone(),
                build_url_with(&self.inner.base_url, request.path())?,
            )
            .header(ACCEPT, "application/json")
            .header("X-Kite-Version", request.kite_version_header())
            .query(request.query_pairs());
        if public {
            http_request = http_request.timeout(DOWNLOAD_TIMEOUT);
        } else if let Some(token) = &self.access_token {
            let mut auth =
                HeaderValue::from_str(&format!("token {}:{}", self.api_key(), token.expose()))
                    .map_err(|_| {
                        Error::InvalidInput("invalid Kite Connect api_key or access_token")
                    })?;
            auth.set_sensitive(true);
            http_request = http_request.header(AUTHORIZATION, auth);
        }
        http_request = match request.body() {
            Body::None => http_request,
            Body::Form(form) => http_request.form(form),
            Body::Json(value) => http_request.json(value),
        };
        let response = http_request.send().await.map_err(transport_error)?;
        if !response.status().is_success() {
            return Err(error_from_response(response).await);
        }
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let body = response.bytes().await.map_err(transport_error)?.into();
        Ok(RawResponse {
            status,
            content_type,
            body,
        })
    }
}

pub(crate) fn transport_error(error: reqwest::Error) -> Error {
    Error::Transport(Box::new(error.without_url()))
}

fn invalid_url(error: url::ParseError) -> Error {
    Error::InvalidUrl(Box::new(error))
}

pub(crate) fn path_segment(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes())
        .map(|part| if part == "+" { "%20" } else { part })
        .collect()
}

fn build_url_with(base_url: &Url, path: &str) -> Result<Url> {
    let lower = path.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Err(Error::InvalidInput(
            "raw request path must be a Kite API path such as /orders, not a full URL",
        ));
    }
    let mut url = base_url.clone();
    let (clean_path, query) = path
        .split_once('?')
        .map_or((path, None), |(path, query)| (path, Some(query)));
    if clean_path.split('/').any(|segment| {
        let decoded = segment.to_ascii_lowercase().replace("%2e", ".");
        matches!(decoded.as_str(), "." | "..")
    }) {
        return Err(Error::InvalidInput(
            "request path must not contain dot segments",
        ));
    }
    url.set_path(clean_path);
    url.set_query(query);
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const TEST_USER_AGENT: &str = "kite-cli/test";

    fn connect_client(base_url: &str) -> KiteConnect {
        KiteConnect::builder("key")
            .access_token(AccessToken::new("token").unwrap())
            .base_url(base_url)
            .app_name(TEST_USER_AGENT)
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn user_agent_and_extra_headers_reach_rest_without_overriding_reserved_headers() {
        let server = httpmock::MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(httpmock::Method::GET)
                    .path("/user/profile")
                    .header("user-agent", "KiteCLI/0.1.0 (x86_64; Linux 6.1.0)")
                    .header("x-kite-app-uuid", "5b697354-64d2-49c4-a080-6b25475555e3")
                    .header("authorization", "token key:token")
                    .header("x-kite-version", "3");
                then.json_body(json!({"status":"success", "data":{}}));
            })
            .await;
        KiteConnect::builder("key")
            .access_token(AccessToken::new("token").unwrap())
            .base_url(server.base_url())
            .user_agent("KiteCLI/0.1.0 (x86_64; Linux 6.1.0)")
            .header("X-Kite-App-UUID", "5b697354-64d2-49c4-a080-6b25475555e3")
            .header("Authorization", "bad-auth")
            .header("X-Kite-Version", "bad-version")
            .header("User-Agent", "bad-agent")
            .build()
            .unwrap()
            .profile()
            .raw()
            .await
            .unwrap();
        mock.assert_async().await;
    }

    #[test]
    fn ticker_debug_redacts_extra_header_values() {
        let client = KiteConnect::builder("key")
            .header("X-Test", "HEADER_SENTINEL")
            .build()
            .unwrap();
        for output in [
            format!("{:?}", client.ticker()),
            format!("{:#?}", client.ticker()),
        ] {
            assert!(!output.contains("HEADER_SENTINEL"));
        }
    }

    #[test]
    fn invalid_extra_headers_do_not_echo_values() {
        for (name, value, expected) in [
            ("bad name", "HEADER_SENTINEL", "invalid header name"),
            ("X-Test", "HEADER_SENTINEL\r\n", "invalid header value"),
            ("Authorization", "HEADER_SENTINEL\n", "invalid header value"),
        ] {
            let err = KiteConnect::builder("key")
                .header(name, value)
                .build()
                .unwrap_err();
            assert_eq!(err.to_string(), expected);
            assert!(!format!("{err:?}").contains("HEADER_SENTINEL"));
        }
        assert_eq!(
            KiteConnect::builder("key")
                .user_agent("HEADER_SENTINEL\n")
                .build()
                .unwrap_err()
                .to_string(),
            "invalid User-Agent"
        );
    }

    #[test]
    fn debug_omits_credentials() {
        let client = KiteConnect::builder("SENTINEL-KEY-4d1e")
            .access_token(AccessToken::new("SENTINEL-TOKEN-91c2").unwrap())
            .build()
            .unwrap();
        let out = format!("{client:?} {:?}", client.access_token());
        assert!(!out.contains("SENTINEL"), "credentials leaked: {out}");
    }

    #[tokio::test]
    async fn connect_requests_carry_no_prefix_version_3_and_the_token_pair_only() {
        let server = httpmock::MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(httpmock::Method::GET)
                    .path("/user/profile")
                    .header("authorization", "token key:token")
                    .header("x-kite-version", "3")
                    .header_missing("x-kite-userid")
                    .header_missing("x-kite-app-uuid")
                    .header_missing("cookie")
                    .header_missing("x-csrftoken");
                then.status(200)
                    .body(success(&json!({ "user_id": "AB1234" })));
            })
            .await;

        let data = connect_client(&server.base_url())
            .profile()
            .raw()
            .await
            .unwrap();

        mock.assert_async().await;
        assert_eq!(data["user_id"], "AB1234");
    }

    #[tokio::test]
    async fn a_request_can_override_the_kite_version_header() {
        let server = httpmock::MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(httpmock::Method::GET)
                    .path("/api/marketwatch")
                    .header("x-kite-version", "3.2.0");
                then.status(200).body(success(&json!([])));
            })
            .await;

        connect_client(&server.base_url())
            .call(Request::get("/api/marketwatch").kite_version("3.2.0"))
            .raw()
            .await
            .unwrap();

        mock.assert_async().await;
    }

    fn expected_user_agent() -> String {
        format!(
            "{TEST_USER_AGENT} zerodha-kiteconnect/{}",
            env!("CARGO_PKG_VERSION")
        )
    }

    fn success(data: &Value) -> String {
        json!({ "status": "success", "data": data }).to_string()
    }

    /// A Cloudflare challenge page must become one readable line with the Ray
    /// ID, not raw HTML.
    #[tokio::test]
    async fn a_cloudflare_challenge_becomes_a_typed_error_with_its_ray_id() {
        let server = httpmock::MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(httpmock::Method::PUT).path("/alerts/uuid-1");
                then.status(403)
                    .header("cf-mitigated", "challenge")
                    .header("cf-ray", "8c1f2a3b4c5d6e7f-BOM")
                    .header("content-type", "text/html; charset=UTF-8")
                    .body(
                        "<!DOCTYPE html><html><head><title>Just a moment...</title></head></html>",
                    );
            })
            .await;
        let err = connect_client(&server.base_url())
            .call(Request::put("/alerts/uuid-1").form([("name", "a")]))
            .raw()
            .await
            .unwrap_err();

        assert!(matches!(
            &err,
            Error::Blocked { status: 403, ray: Some(ray) }
                if ray == "8c1f2a3b4c5d6e7f-BOM"
        ));
        let message = err.to_string();
        assert_eq!(
            message,
            "Cloudflare blocked the request (HTTP 403, Ray 8c1f2a3b4c5d6e7f-BOM)"
        );
        assert!(!err.is_session_expired());
    }

    #[tokio::test]
    async fn connect_client_sends_the_app_name_then_the_sdk_version() {
        let server = httpmock::MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(httpmock::Method::GET)
                    .path("/user/profile")
                    .header("user-agent", expected_user_agent())
                    .header("x-kite-version", "3");
                then.status(200)
                    .body(success(&json!({ "user_id": "AB1234" })));
            })
            .await;
        let data = connect_client(&server.base_url())
            .profile()
            .raw()
            .await
            .unwrap();

        mock.assert_async().await;
        assert_eq!(data["user_id"], "AB1234");
    }

    #[tokio::test]
    async fn instrument_paths_are_public_with_or_without_a_leading_slash() {
        let server = httpmock::MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(httpmock::Method::GET)
                    .path("/instruments")
                    .header("user-agent", expected_user_agent())
                    .header("x-kite-version", "3")
                    .header_missing("authorization");
                then.status(200).body("instrument_token\n1\n");
            })
            .await;
        let client = connect_client(&server.base_url());
        for path in ["instruments", "/instruments"] {
            let response = client.call(Request::get(path)).response().await.unwrap();
            assert_eq!(response.body(), b"instrument_token\n1\n");
        }
        mock.assert_calls_async(2).await;
    }

    #[test]
    fn a_base_url_with_a_path_is_rejected() {
        for base_url in ["https://proxy.example/kite", "https://proxy.example/kite/"] {
            let err = KiteConnect::builder("key")
                .base_url(base_url)
                .build()
                .unwrap_err();
            assert_eq!(err.to_string(), "base URL must not have a path");
        }
        for base_url in ["https://proxy.example", "https://proxy.example/"] {
            KiteConnect::builder("key")
                .base_url(base_url)
                .build()
                .unwrap();
        }
    }

    #[test]
    fn identifiers_cannot_change_the_endpoint_or_query() {
        let client = connect_client("https://api.example");
        let id = "../../gtt/triggers/999?x=1#fragment";
        let encoded = "..%2F..%2Fgtt%2Ftriggers%2F999%3Fx%3D1%23fragment";
        let requests = [
            client.order_history(id).request().clone(),
            client.order_trades(id).request().clone(),
            client
                .cancel_order(crate::enums::Variety::Regular, id, None)
                .request()
                .clone(),
            client.mf_order_info(id).request().clone(),
            client.cancel_mf_order(id).request().clone(),
            client.mf_sip_info(id).request().clone(),
            client.cancel_mf_sip(id).request().clone(),
            client.mf_holding_info(id).request().clone(),
            client.alert(id).request().clone(),
            client.alert_history(id).request().clone(),
        ];
        for request in requests {
            let url = build_url_with(&client.inner.base_url, request.path()).unwrap();
            assert!(url.path().contains(encoded), "{url}");
            assert_eq!(url.query(), None);
            assert_eq!(url.fragment(), None);
        }
        assert_eq!(path_segment("a b%2F\\é"), "a%20b%252F%5C%C3%A9");
        for id in [".", ".."] {
            let request = client.cancel_order(crate::enums::Variety::Regular, id, None);
            assert!(matches!(
                build_url_with(&client.inner.base_url, request.request().path()),
                Err(Error::InvalidInput(_))
            ));
        }
    }

    #[tokio::test]
    async fn dot_identifier_is_rejected_before_any_delete_is_sent() {
        let server = httpmock::MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(httpmock::Method::DELETE);
                then.status(200)
                    .json_body(json!({"status":"success","data":{"order_id":"test"}}));
            })
            .await;
        let client = connect_client(&server.base_url());
        assert!(matches!(
            client
                .cancel_order(crate::enums::Variety::Regular, "..", None)
                .await,
            Err(Error::InvalidInput(_))
        ));
        mock.assert_calls_async(0).await;
    }

    #[test]
    fn request_urls_preserve_path_encoding_queries_and_root_fragments() {
        for root in ["https://api.example", "https://api.example/"] {
            let base = Url::parse(root).unwrap();
            for (path, expected) in [
                ("", "https://api.example/"),
                ("orders", "https://api.example/orders"),
                ("/orders", "https://api.example/orders"),
                (
                    "/orders?tag=a%2Bb&tag=c",
                    "https://api.example/orders?tag=a%2Bb&tag=c",
                ),
                ("/orders/id%2Fpart", "https://api.example/orders/id%2Fpart"),
                (
                    "orders/id #?tag=a#b",
                    "https://api.example/orders/id%20%23?tag=a%23b",
                ),
            ] {
                assert_eq!(build_url_with(&base, path).unwrap().as_str(), expected);
            }
            for path in [
                "https://other.example/orders",
                "http://other.example/orders",
                "HTTP://other.example/orders",
                "HTTPS://other.example/orders",
            ] {
                assert!(matches!(
                    build_url_with(&base, path),
                    Err(Error::InvalidInput(_))
                ));
            }
        }
    }

    #[tokio::test]
    async fn sdk_error_envelopes_keep_http_status_data_and_distinct_defaults() {
        for (status, body, kind, message, data) in [
            (
                200,
                json!({"status":"error"}),
                "Unknown",
                "Unknown Kite API error",
                None,
            ),
            (403, json!({"status":"error"}), "Unknown", "HTTP 403", None),
            (
                200,
                json!({"status":"error","error_type":"FutureException","message":"rejected","data":{"hints":["refresh"]}}),
                "FutureException",
                "rejected",
                Some(json!({"hints":["refresh"]})),
            ),
            (
                403,
                json!({"status":"error","error_type":"TokenException","message":"expired","data":{"hints":["refresh"]}}),
                "TokenException",
                "expired",
                Some(json!({"hints":["refresh"]})),
            ),
        ] {
            let server = httpmock::MockServer::start_async().await;
            let mock = server
                .mock_async(|when, then| {
                    when.method(httpmock::Method::GET).path("/orders");
                    then.status(status).json_body(body);
                })
                .await;
            let Error::Api(error) = connect_client(&server.base_url())
                .orders()
                .raw()
                .await
                .unwrap_err()
            else {
                panic!("expected an API error");
            };
            assert_eq!(error.kind.as_str(), kind);
            assert_eq!(error.message, message);
            assert_eq!(error.status, status);
            assert_eq!(error.data, data);
            mock.assert_async().await;
        }
    }

    #[tokio::test]
    async fn connect_public_instrument_download_sends_no_token_pair() {
        let server = httpmock::MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(httpmock::Method::GET)
                    .path("/mf/instruments")
                    .header_missing("authorization")
                    .header_missing("cookie");
                then.status(200).body("tradingsymbol\n");
            })
            .await;

        let response = connect_client(&server.base_url())
            .mf_instruments()
            .response()
            .await
            .unwrap();

        mock.assert_async().await;
        assert_eq!(response.body(), b"tradingsymbol\n");
    }

    #[test]
    fn blank_or_line_broken_pairs_are_rejected_without_echoing_them() {
        for (api_key, access_token, expected) in [
            ("   ", "SENTINEL-TOKEN-91c2", "api_key is blank"),
            ("SENTINEL-KEY-4d1e", "", "access_token is blank"),
            (
                "SENTINEL-KEY-4d1e",
                "SENTINEL-TOKEN-91c2\r\nX-Evil: 1",
                "access_token contains a line break",
            ),
            (
                "SENTINEL-KEY-4d1e\n",
                "SENTINEL-TOKEN-91c2",
                "api_key contains a line break",
            ),
        ] {
            let result = AccessToken::new(access_token)
                .and_then(|token| KiteConnect::builder(api_key).access_token(token).build());
            let err = result.unwrap_err();
            let message = format!("{err:#} {err:?}");
            assert!(message.contains(expected), "{message}");
            assert!(
                !message.contains("SENTINEL"),
                "credential leaked: {message}"
            );
        }
    }

    #[tokio::test]
    async fn unauthenticated_call_omits_authorization() {
        let server = httpmock::MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(httpmock::Method::GET)
                    .path("/user/profile")
                    .header_missing("authorization")
                    .header("X-Kite-Version", "3");
                then.json_body(json!({"status":"success","data":{"user_id":"AB1234"}}));
            })
            .await;
        let client = KiteConnect::builder("key")
            .base_url(server.base_url())
            .build()
            .unwrap();
        assert_eq!(
            client.profile().raw().await.unwrap(),
            json!({"user_id":"AB1234"})
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn raw_returns_data_verbatim_and_bypasses_the_decoder() {
        fn refuse(_client: &KiteConnect, _response: &RawResponse, _path: &str) -> Result<Value> {
            Err(Error::InvalidInput("decoder must not run"))
        }
        let server = httpmock::MockServer::start_async().await;
        let mock = server.mock_async(|when, then| {
            when.method(httpmock::Method::GET).path("/user/profile");
            then.json_body(json!({"status":"success","data":{"extra":[null,true,42,"field"],"user_id":"AB1234"}}));
        }).await;
        let call = Call::with_decoder(
            connect_client(&server.base_url()),
            Request::get("/user/profile"),
            refuse,
        );
        assert_eq!(
            call.raw().await.unwrap(),
            json!({"extra":[null,true,42,"field"],"user_id":"AB1234"})
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn response_returns_csv_bytes_without_envelope_decoding() {
        let server = httpmock::MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(httpmock::Method::GET)
                    .path("/mf/instruments")
                    .header_missing("authorization");
                then.status(200)
                    .header("content-type", "text/csv")
                    .body("tradingsymbol,name\nINFY,Infosys\n");
            })
            .await;
        let response = connect_client(&server.base_url())
            .mf_instruments()
            .response()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.content_type(), Some("text/csv"));
        assert_eq!(response.body(), b"tradingsymbol,name\nINFY,Infosys\n");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn order_request_is_the_request_sent_and_owns_the_client() {
        use std::future::IntoFuture;
        fn send_static<T: Send + 'static>(value: T) -> T {
            value
        }
        let server = httpmock::MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(httpmock::Method::POST)
                    .path("/orders/regular")
                    .header("authorization", "token key:token")
                    .body("exchange=NSE&tradingsymbol=INFY&quantity=1");
                then.json_body(json!({"status":"success","data":{"order_id":"order-1"}}));
            })
            .await;
        let client = connect_client(&server.base_url());
        let call = send_static(
            client.place_order(
                crate::Variety::Regular,
                crate::OrderParams::new()
                    .exchange(crate::Exchange::Nse)
                    .tradingsymbol("INFY")
                    .quantity(1),
            ),
        );
        assert_eq!(call.request().method(), http::Method::POST);
        assert_eq!(call.request().path(), "/orders/regular");
        let Body::Form(form) = call.request().body() else {
            panic!("expected a form body");
        };
        assert_eq!(
            form,
            &[
                ("exchange".to_string(), "NSE".to_string()),
                ("tradingsymbol".to_string(), "INFY".to_string()),
                ("quantity".to_string(), "1".to_string()),
            ]
        );
        drop(client);
        let result = tokio::spawn(send_static(call.into_future()))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.order_id, "order-1");
        mock.assert_async().await;
    }
}
