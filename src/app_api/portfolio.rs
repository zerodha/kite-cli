use kiteconnect::{KiteConnect, MfHolding, Request};

/// Mutual-fund holdings used by the TUI.
///
/// # Errors
/// Returns an error if the request fails or the response cannot be decoded.
pub async fn mf_holdings(client: &KiteConnect) -> kiteconnect::Result<Vec<MfHolding>> {
    let data = client.call(Request::get("/portfolio/holdings/mf")).await?;
    serde_json::from_value(data).map_err(|source| kiteconnect::Error::Decode {
        path: "/portfolio/holdings/mf".to_string(),
        source,
    })
}
