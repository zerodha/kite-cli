use crate::app::runtime::Runtime;
use crate::app_api::trusted::TrustedAuth;
use anyhow::Result;
use kiteconnect::error::sanitize_error;
use std::path::PathBuf;

/// What ending the saved session did on Kite.
#[derive(Debug)]
pub struct SessionEnd {
    pub path: PathBuf,
    pub had_session: bool,
    /// Kite confirmed the current session ended.
    pub revoked: bool,
    /// Sessions from earlier logouts that Kite has now confirmed ended.
    pub earlier_revoked: usize,
    /// Sessions Kite has not confirmed ended, kept for the next logout.
    pub pending: usize,
    pub error: Option<String>,
}

impl SessionEnd {
    #[must_use]
    pub fn warning(&self) -> Option<String> {
        let error = self.error.as_deref()?;
        Some(format!(
            "could not revoke {} on Kite: {error}. A session stays active until 6 AM IST unless revoked; run `kite auth logout` again when Kite is reachable, or log out on Kite",
            sessions(self.pending)
        ))
    }

    #[must_use]
    pub fn details(&self) -> Vec<String> {
        let path = self.path.display();
        let mut details = vec![if self.revoked {
            format!("Session revoked on Kite and removed from {path}")
        } else if self.had_session {
            format!("Session removed from {path}; it is not yet revoked on Kite")
        } else if self.earlier_revoked == 0 && self.pending == 0 {
            "No saved session; nothing to revoke.".to_string()
        } else {
            "No saved session.".to_string()
        }];
        if self.earlier_revoked > 0 {
            details.push(format!(
                "Revoked {} on Kite from an earlier logout",
                sessions(self.earlier_revoked)
            ));
        }
        details
    }
}

fn sessions(count: usize) -> String {
    if count == 1 {
        "1 session".to_string()
    } else {
        format!("{count} sessions")
    }
}

/// Revokes the saved session and any left over from earlier logouts, then
/// clears the saved session. A token is dropped only once Kite confirms it
/// ended, so a logout that cannot reach Kite is retried by the next one.
///
/// # Errors
/// Returns an error if the config cannot be read or written.
pub async fn end_session(rt: &Runtime) -> Result<SessionEnd> {
    let (_, _, config) = rt.load_config()?;
    let had_session = config.access_token.is_some();
    let mut tokens = config.revoke_pending;
    tokens.retain(|token| Some(token) != config.access_token.as_ref());
    let earlier = tokens.len();
    tokens.extend(config.access_token);

    let mut end = SessionEnd {
        path: PathBuf::new(),
        had_session,
        revoked: false,
        earlier_revoked: 0,
        pending: 0,
        error: None,
    };
    let mut pending = Vec::new();
    if !tokens.is_empty() {
        let auth = rt.resolve_credentials().and_then(|creds| {
            TrustedAuth::new(creds.api_root()?, creds.api_key()?, &rt.app_uuid()?)
        });
        for (index, token) in tokens.into_iter().enumerate() {
            let outcome = match &auth {
                Ok(auth) => auth.invalidate(&token).await.or_else(already_ended),
                Err(error) => Err(anyhow::anyhow!("{error}")),
            };
            match outcome {
                Ok(()) if index < earlier => end.earlier_revoked += 1,
                Ok(()) => end.revoked = true,
                Err(error) => {
                    end.error = Some(sanitize_error(&error.to_string()));
                    pending.push(token);
                }
            }
        }
    }
    end.pending = pending.len();
    end.path = rt.clear_session(pending)?;
    Ok(end)
}

/// Kite rejecting the token itself means the session has already ended.
fn already_ended(error: anyhow::Error) -> Result<()> {
    match error.downcast_ref::<kiteconnect::Error>() {
        Some(kite) if kite.is_session_expired() => Ok(()),
        _ => Err(error),
    }
}
