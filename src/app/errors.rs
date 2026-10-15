use kiteconnect::Error;

#[derive(Debug)]
pub struct MissingSession;

impl std::fmt::Display for MissingSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("not logged in; run `kite auth login`")
    }
}

impl std::error::Error for MissingSession {}

#[must_use]
pub fn error_type(err: &anyhow::Error) -> &str {
    if err.downcast_ref::<MissingSession>().is_some() {
        return "MissingSession";
    }
    if let Some(kite_error) = err.downcast_ref::<Error>() {
        return match kite_error {
            Error::Api(error) => error.kind.as_str(),
            Error::Blocked { .. } => "CloudflareBlocked",
            _ => "Error",
        };
    }
    "Error"
}

#[must_use]
pub fn format_error(err: &anyhow::Error) -> String {
    let message = match err.downcast_ref::<Error>() {
        Some(error @ Error::Decode { .. }) => error.to_string(),
        _ => format!("{err:#}"),
    };
    kiteconnect::error::sanitize_error(&message)
}

/// Collapse a transport-level failure into one short, actionable line, or
/// `None` when the error isn't a transport failure.
///
/// reqwest's `Display` chain is precise but unreadable as a one-line banner:
///
/// ```text
/// request failed: error sending request for url
/// (https://api.kite.trade/portfolio/positions): client error (Connect):
/// tcp connect error: Network unreachable (os error 101)
/// ```
///
/// Every poller emits that on each cycle while a laptop's `WiFi` is down, and it
/// reads as the terminal being broken rather than the connection being gone. An
/// API error, by contrast, carries a real message from Kite that the user needs
/// to see word-for-word, so this only ever fires for the transport layer.
#[must_use]
pub fn transport_error_summary(err: &str) -> Option<&'static str> {
    let lower = err.to_ascii_lowercase();
    if lower.contains("kite api error") || lower.contains("http ") {
        return None;
    }
    let unreachable = [
        "network unreachable",
        "network is unreachable",
        "no route to host",
        "dns error",
        "failed to lookup address information",
        "name or service not known",
        "temporary failure in name resolution",
    ];
    if unreachable.iter().any(|needle| lower.contains(needle)) {
        return Some("offline, no network route to Kite; retrying automatically");
    }
    if lower.contains("timed out") || lower.contains("timeout") {
        return Some("Kite is slow to respond (request timed out); retrying automatically");
    }
    let dropped = [
        "error sending request",
        "tcp connect error",
        "connection reset",
        "connection refused",
        "connection closed",
        "connection aborted",
        "broken pipe",
    ];
    if dropped.iter().any(|needle| lower.contains(needle)) {
        return Some("lost connection to Kite; retrying automatically");
    }
    None
}

/// Detect session expiry after the CLI poller has converted an error to text.
/// Only a `TokenException` means the server rejected the session.
#[must_use]
pub fn is_auth_error_text(err: &str) -> bool {
    err.contains("TokenException")
}

#[cfg(test)]
mod tests {
    use super::is_auth_error_text;
    #[test]
    fn is_auth_error_text_matches_token_exception() {
        assert!(is_auth_error_text(
            "kite API error TokenException (HTTP 403): Invalid session"
        ));
    }

    #[test]
    fn is_auth_error_text_matches_a_prefixed_token_exception() {
        assert!(is_auth_error_text(
            "marketwatch: kite API error TokenException (HTTP 403): Invalid session"
        ));
    }

    #[test]
    fn is_auth_error_text_ignores_plain_403() {
        assert!(!is_auth_error_text(
            "kite API error Forbidden (HTTP 403): no permission"
        ));
    }

    #[test]
    fn is_auth_error_text_ignores_unrelated_errors() {
        assert!(!is_auth_error_text("request failed: connection reset"));
        assert!(!is_auth_error_text(
            "kite API error InputException (HTTP 400): bad request"
        ));
        assert!(!is_auth_error_text("retrying after 403ms backoff"));
    }

    use super::transport_error_summary;

    /// The exact banner text from the reported "Failed to Load" recording. It
    /// has to collapse to one short line, not span the width of the terminal.
    const REPORTED: &str = "request failed: error sending request for url \
         (https://example.invalid/portfolio/positions): client error \
         (Connect): tcp connect error: Network unreachable (os error 101)";

    #[test]
    fn transport_summary_collapses_the_reported_connectivity_dump() {
        assert_eq!(
            transport_error_summary(REPORTED),
            Some("offline, no network route to Kite; retrying automatically")
        );
        for err in [
            "error sending request for url (...): dns error: failed to lookup address information",
            "No route to host (os error 113)",
        ] {
            assert_eq!(
                transport_error_summary(err),
                Some("offline, no network route to Kite; retrying automatically"),
                "{err}"
            );
        }
        assert_eq!(
            transport_error_summary("request failed: operation timed out"),
            Some("Kite is slow to respond (request timed out); retrying automatically")
        );
        for err in [
            "request failed: connection reset by peer",
            "request failed: error sending request for url (...)",
            "connection refused",
        ] {
            assert_eq!(
                transport_error_summary(err),
                Some("lost connection to Kite; retrying automatically"),
                "{err}"
            );
        }
    }

    #[test]
    fn transport_summary_leaves_real_api_errors_verbatim() {
        for err in [
            "kite API error TokenException (HTTP 403): Invalid session",
            "kite API error InputException (HTTP 400): Trigger cannot be created",
            "HTTP 502: <html>bad gateway</html>",
            "bad positions payload: missing field `net`",
        ] {
            assert_eq!(transport_error_summary(err), None, "{err}");
        }
    }

    #[test]
    fn transport_summary_never_leaks_a_redacted_url() {
        let summary = transport_error_summary(REPORTED).unwrap();
        assert!(!summary.contains("kite.zerodha.com"));
        assert!(!summary.contains("os error"));
    }
    #[test]
    fn error_messages_keep_context_and_suppress_decode_sources() {
        let error = anyhow::anyhow!("request_token=ERROR_SENTINEL").context("refresh session");
        assert_eq!(
            super::format_error(&error),
            "refresh session: request_token=<redacted>"
        );
        let source = <serde_json::Error as serde::de::Error>::custom("DECODE_SENTINEL");
        let error = anyhow::Error::new(kiteconnect::Error::Decode {
            path: "/audit".into(),
            source,
        })
        .context("load data");
        assert_eq!(
            super::format_error(&error),
            "could not decode the response from /audit"
        );
        assert!(!super::format_error(&error).contains("DECODE_SENTINEL"));
    }
}
