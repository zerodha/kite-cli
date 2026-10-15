use std::time::Duration;

/// Whole-request budget for authenticated API calls.
pub const API_TIMEOUT: Duration = Duration::from_secs(15);
/// Whole-request budget for the multi-MB public instrument master downloads.
pub const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);
const SDK: &str = concat!("zerodha-kiteconnect/", env!("CARGO_PKG_VERSION"));

/// `{app_name} zerodha-kiteconnect/<version>`, sent on REST and WebSocket calls.
pub(crate) fn user_agent(app_name: Option<&str>) -> String {
    match app_name {
        Some(app_name) => format!("{app_name} {SDK}"),
        None => SDK.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_agent_appends_the_sdk_version_to_the_app_name() {
        let sdk = format!("zerodha-kiteconnect/{}", env!("CARGO_PKG_VERSION"));
        assert_eq!(user_agent(None), sdk);
        assert_eq!(
            user_agent(Some("kite-cli/1.2.3")),
            format!("kite-cli/1.2.3 {sdk}")
        );
    }
}
