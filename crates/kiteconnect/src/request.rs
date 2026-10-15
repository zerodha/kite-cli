use crate::error::is_secret_key;
use http::Method;
use serde_json::Value;
use std::fmt;

fn redact<'a>(key: &str, value: &'a str) -> &'a str {
    if is_secret_key(key) {
        "<redacted>"
    } else {
        value
    }
}

fn redacted_pairs(pairs: &[(String, String)]) -> Vec<(&str, &str)> {
    pairs
        .iter()
        .map(|(key, value)| (key.as_str(), redact(key, value)))
        .collect()
}

/// Request body. `Debug` hides form values for secret keys and every JSON body.
#[derive(Clone)]
#[non_exhaustive]
pub enum Body {
    /// No body.
    None,
    /// Form-encoded fields, sent in order.
    Form(Vec<(String, String)>),
    /// JSON body.
    Json(Value),
}

impl fmt::Debug for Body {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => f.write_str("None"),
            Self::Form(pairs) => f.debug_tuple("Form").field(&redacted_pairs(pairs)).finish(),
            Self::Json(_) => f.write_str("Json(<redacted>)"),
        }
    }
}

/// A Kite API request: method, path, query, body and `X-Kite-Version` header.
///
/// `Debug` hides the values of secret keys in the query and the form.
#[derive(Clone)]
#[must_use]
pub struct Request {
    method: Method,
    path: String,
    query: Vec<(String, String)>,
    body: Body,
    kite_version: &'static str,
}

impl Request {
    /// Creates a request with no query and no body.
    pub fn new(method: Method, path: impl Into<String>) -> Self {
        Self {
            method,
            path: path.into(),
            query: Vec::new(),
            body: Body::None,
            kite_version: "3",
        }
    }

    /// Creates a `GET` request.
    pub fn get(path: impl Into<String>) -> Self {
        Self::new(Method::GET, path)
    }
    /// Creates a `POST` request.
    pub fn post(path: impl Into<String>) -> Self {
        Self::new(Method::POST, path)
    }
    /// Creates a `PUT` request.
    pub fn put(path: impl Into<String>) -> Self {
        Self::new(Method::PUT, path)
    }
    /// Creates a `DELETE` request.
    pub fn delete(path: impl Into<String>) -> Self {
        Self::new(Method::DELETE, path)
    }

    /// Appends query parameters.
    pub fn query<K: Into<String>, V: Into<String>>(
        mut self,
        pairs: impl IntoIterator<Item = (K, V)>,
    ) -> Self {
        self.query.extend(
            pairs
                .into_iter()
                .map(|(key, value)| (key.into(), value.into())),
        );
        self
    }

    /// Sets a form-encoded body.
    pub fn form<K: Into<String>, V: Into<String>>(
        self,
        pairs: impl IntoIterator<Item = (K, V)>,
    ) -> Self {
        self.with_body(Body::Form(
            pairs
                .into_iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        ))
    }

    /// Sets a JSON body.
    pub fn json(self, value: Value) -> Self {
        self.with_body(Body::Json(value))
    }
    /// Sets the body.
    pub fn with_body(mut self, body: Body) -> Self {
        self.body = body;
        self
    }
    /// Overrides the `X-Kite-Version` header, which defaults to the Kite Connect
    /// version `3`. Some routes outside the Kite Connect docs serve a different
    /// response shape per version.
    pub fn kite_version(mut self, version: &'static str) -> Self {
        self.kite_version = version;
        self
    }
    /// Returns the HTTP method.
    #[must_use]
    pub fn method(&self) -> &Method {
        &self.method
    }
    /// Returns the path, including any query string written into it.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
    /// Returns the query parameters added with [`Request::query`].
    #[must_use]
    pub fn query_pairs(&self) -> &[(String, String)] {
        &self.query
    }
    /// Returns the body.
    #[must_use]
    pub fn body(&self) -> &Body {
        &self.body
    }
    pub(crate) fn kite_version_header(&self) -> &'static str {
        self.kite_version
    }
}

impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Request")
            .field("method", &self.method)
            .field("path", &self.path.split('?').next().unwrap_or_default())
            .field("query", &redacted_pairs(&self.query))
            .field("body", &self.body)
            .field("kite_version", &self.kite_version)
            .finish()
    }
}

/// HTTP response from a successful (2xx) request, before envelope decoding.
#[derive(Debug)]
#[non_exhaustive]
pub struct RawResponse {
    pub(crate) status: u16,
    pub(crate) content_type: Option<String>,
    pub(crate) body: Vec<u8>,
}

impl RawResponse {
    /// Returns the HTTP status code.
    #[must_use]
    pub fn status(&self) -> u16 {
        self.status
    }
    /// Returns the `Content-Type` header, if present.
    #[must_use]
    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }
    /// Returns the response body bytes.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRETS: [(&str, &str); 7] = [
        ("api_secret", "SENTINEL-SECRET"),
        ("ACCESS_TOKEN", "SENTINEL-ACCESS"),
        ("refresh_token", "SENTINEL-REFRESH"),
        ("request_token", "SENTINEL-REQUEST"),
        ("password", "SENTINEL-PASSWORD"),
        ("checksum", "SENTINEL-CHECKSUM"),
        ("twofa_value", "SENTINEL-OTP"),
    ];

    #[test]
    fn query_debug_redacts_every_secret_key() {
        let request = Request::get("/session/token?access_token=SENTINEL-PATH")
            .query(SECRETS)
            .query([("i", "NSE:INFY")]);
        let debug = format!("{request:?}");
        assert!(!debug.contains("SENTINEL"), "{debug}");
        assert!(debug.contains("NSE:INFY"), "{debug}");
    }

    #[test]
    fn form_debug_redacts_every_secret_key() {
        let request = Request::post("/session/token")
            .form(SECRETS.into_iter().chain([("tradingsymbol", "INFY")]));
        let debug = format!("{request:?}");
        assert!(!debug.contains("SENTINEL"), "{debug}");
        assert!(debug.contains("INFY"), "{debug}");
    }
}
