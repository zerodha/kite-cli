use crate::{Error, Result};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{fmt, sync::Arc};

/// Access token sent as `Authorization: token api_key:access_token`.
#[derive(Clone, PartialEq, Eq)]
pub struct AccessToken(Arc<str>);

impl AccessToken {
    /// Validates the token.
    ///
    /// # Errors
    /// Returns [`Error::InvalidInput`] if the token is blank or contains a line break.
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        validate(
            &value,
            "Kite Connect access_token is blank",
            "Kite Connect access_token contains a line break",
        )?;
        Ok(Self(value.into()))
    }

    /// Returns the token text. Do not log it.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for AccessToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AccessToken(<redacted>)")
    }
}

impl Serialize for AccessToken {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.expose())
    }
}

impl<'de> Deserialize<'de> for AccessToken {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// Refresh token for `POST /session/refresh_token`.
#[derive(Clone, PartialEq, Eq)]
pub struct RefreshToken(Arc<str>);

impl RefreshToken {
    /// Validates the token.
    ///
    /// # Errors
    /// Returns [`Error::InvalidInput`] if the token is blank or contains a line break.
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        validate(
            &value,
            "Kite Connect refresh_token is blank",
            "Kite Connect refresh_token contains a line break",
        )?;
        Ok(Self(value.into()))
    }

    /// Returns the token text. Do not log it.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for RefreshToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RefreshToken(<redacted>)")
    }
}

impl Serialize for RefreshToken {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.expose())
    }
}

impl<'de> Deserialize<'de> for RefreshToken {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// Kite sends `""` when the app has no refresh-token access.
pub(crate) fn deserialize_optional_refresh_token<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<RefreshToken>, D::Error> {
    match Option::<String>::deserialize(deserializer)? {
        Some(value) if !value.is_empty() => RefreshToken::new(value)
            .map(Some)
            .map_err(de::Error::custom),
        _ => Ok(None),
    }
}

/// API secret for `POST /session/token` and `POST /session/refresh_token`.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiSecret(Arc<str>);

impl ApiSecret {
    /// Validates the secret.
    ///
    /// # Errors
    /// Returns [`Error::InvalidInput`] if the secret is blank or contains a line break.
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        validate(
            &value,
            "Kite Connect api_secret is blank",
            "Kite Connect api_secret contains a line break",
        )?;
        Ok(Self(value.into()))
    }

    /// Returns the secret text. Do not log it.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ApiSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiSecret(<redacted>)")
    }
}

pub(crate) fn validate(value: &str, blank: &'static str, line_break: &'static str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(Error::InvalidInput(blank));
    }
    if value.contains(['\r', '\n']) {
        return Err(Error::InvalidInput(line_break));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn secrets_validate_and_redact() {
        for value in ["", " \t", "secret\n", "secret\r"] {
            assert!(ApiSecret::new(value).is_err());
            assert!(AccessToken::new(value).is_err());
            assert!(RefreshToken::new(value).is_err());
        }
        let secret = ApiSecret::new("SENTINEL-SECRET").unwrap();
        assert_eq!(secret.expose(), "SENTINEL-SECRET");
        assert_eq!(format!("{secret:?}"), "ApiSecret(<redacted>)");
        let token = RefreshToken::new("SENTINEL-REFRESH").unwrap();
        assert_eq!(format!("{token:?}"), "RefreshToken(<redacted>)");
        assert_eq!(
            format!("{:?}", AccessToken::new("SENTINEL").unwrap()),
            "AccessToken(<redacted>)"
        );
    }

    #[test]
    fn tokens_round_trip_through_serde_and_reject_invalid_values() {
        let token: RefreshToken = serde_json::from_value(json!("refresh")).unwrap();
        assert_eq!(serde_json::to_value(&token).unwrap(), json!("refresh"));
        let token: AccessToken = serde_json::from_value(json!("access")).unwrap();
        assert_eq!(serde_json::to_value(&token).unwrap(), json!("access"));
        assert!(serde_json::from_value::<RefreshToken>(json!("")).is_err());
        assert!(serde_json::from_value::<AccessToken>(json!("a\nb")).is_err());
    }
}
