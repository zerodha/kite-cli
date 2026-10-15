use super::{KiteConnect, api_err};
pub use kiteconnect::{AllMargins as Funds, Margins as FundsSegment};

pub fn funds_margin_available(funds: &Funds) -> f64 {
    funds
        .equity
        .available
        .live_balance
        .unwrap_or(funds.equity.net)
}

pub async fn fetch_funds(client: &KiteConnect) -> Result<Funds, String> {
    client.margins().await.map_err(api_err)
}

#[cfg(test)]
mod tests {
    use super::Funds;
    use crate::tui::data::tests::{connect_client, next_event, reply};
    use crate::tui::data::{DataEvent, spawn_refresh_margins};
    use httpmock::Method::GET;
    use httpmock::MockServer;
    use serde_json::json;
    use tokio::sync::mpsc;

    fn full_margins() -> serde_json::Value {
        serde_json::json!({
                "equity": {
                    "enabled": true,
                    "net": 99725.05,
                    "available": {
                        "adhoc_margin": 0,
                        "cash": 245_431.6,
                        "opening_balance": 245_431.6,
                        "live_balance": 99725.05,
                        "collateral": 0,
                        "intraday_payin": 0
                    },
                    "utilised": {
                        "debits": 145_706.55,
                        "exposure": 38981.25,
                        "m2m_realised": 761.7,
                        "m2m_unrealised": 0,
                        "option_premium": 0,
                        "payout": 0,
                        "span": 101_989,
                        "holding_sales": 0,
                        "turnover": 0,
                        "liquid_collateral": 123.45,
                        "stock_collateral": 678.9,
                        "delivery": 0
                    }
                },
                "commodity": {
                    "enabled": true,
                    "net": 100_661.7,
                    "available": {
                        "adhoc_margin": 0,
                        "cash": 100_661.7,
                        "opening_balance": 100_661.7,
                        "live_balance": 100_661.7,
                        "collateral": 0,
                        "intraday_payin": 0
                    },
                    "utilised": {
                        "debits": 0,
                        "exposure": 0,
                        "m2m_realised": 0,
                        "m2m_unrealised": 0,
                        "option_premium": 0,
                        "payout": 0,
                        "span": 0,
                        "holding_sales": 0,
                        "turnover": 0,
                        "liquid_collateral": 0,
                        "stock_collateral": 0,
                        "delivery": 0
                    }
                }
        })
    }

    #[test]
    fn funds_parse_full_margins_payload() {
        let json = full_margins();

        let funds: Funds = serde_json::from_value(json).expect("funds parse");
        assert_eq!(
            funds.equity.available.cash.to_bits(),
            245_431.6_f64.to_bits()
        );
        assert_eq!(funds.equity.available.live_balance, Some(99725.05));
        assert_eq!(
            funds.equity.utilised.span.to_bits(),
            101_989.0_f64.to_bits()
        );
        assert_eq!(
            funds.equity.utilised.exposure.to_bits(),
            38981.25_f64.to_bits()
        );
        assert_eq!(
            funds.equity.utilised.liquid_collateral.to_bits(),
            123.45_f64.to_bits()
        );
        assert_eq!(
            funds.equity.utilised.stock_collateral.to_bits(),
            678.9_f64.to_bits()
        );
        assert_eq!(
            funds
                .commodity
                .as_ref()
                .expect("commodity")
                .available
                .cash
                .to_bits(),
            100_661.7_f64.to_bits()
        );
        assert_eq!(
            funds
                .commodity
                .as_ref()
                .expect("commodity")
                .utilised
                .span
                .to_bits(),
            0.0_f64.to_bits()
        );
        assert_eq!(
            crate::tui::data::funds_margin_available(&funds).to_bits(),
            99725.05_f64.to_bits()
        );
    }

    #[test]
    fn funds_margin_available_falls_back_to_net_when_live_balance_absent() {
        let mut json = full_margins();
        json["equity"]["net"] = json!(12345.0);
        json["equity"]["available"]
            .as_object_mut()
            .unwrap()
            .remove("live_balance");
        let funds: Funds = serde_json::from_value(json).expect("funds parse");
        assert_eq!(funds.equity.available.live_balance, None);
        assert_eq!(
            crate::tui::data::funds_margin_available(&funds).to_bits(),
            12345.0_f64.to_bits()
        );
    }

    #[tokio::test]
    async fn margins_events_require_complete_equity() {
        let margins = |data: serde_json::Value| async move {
            let server = MockServer::start_async().await;
            let mock = reply(&server, GET, "/user/margins", data).await;
            let (tx, mut rx) = mpsc::channel(1);
            spawn_refresh_margins(connect_client(&server), tx)
                .await
                .expect("margins task");
            mock.assert_calls_async(1).await;
            match next_event(&mut rx).await {
                DataEvent::Margins(result) => result,
                other => panic!("unexpected {other:?}"),
            }
        };

        let err = margins(json!({})).await.expect_err("no equity");
        assert!(
            err.starts_with("bad /user/margins payload") && err.contains("equity"),
            "{err}"
        );
        let err = margins(json!({"equity": {"net": 5.0}}))
            .await
            .expect_err("sparse margins are rejected");
        assert!(err.starts_with("bad /user/margins payload"), "{err}");
        let mut zero_live_json = full_margins();
        zero_live_json["equity"]["available"]["live_balance"] = json!(0);
        let zero_live = margins(zero_live_json).await.expect("zero live balance");
        assert_eq!(
            crate::tui::data::funds_margin_available(&zero_live).to_bits(),
            0.0_f64.to_bits()
        );
        let mut null_net = full_margins();
        null_net["equity"]["net"] = json!(null);
        let err = margins(null_net).await.expect_err("null net");
        assert!(
            err.starts_with("bad /user/margins payload") && err.contains("null"),
            "{err}"
        );
        let mut leaky = full_margins();
        leaky["equity"]["net"] = json!("access_token=SENTINEL_TOKEN");
        let err = margins(leaky).await.expect_err("string net");
        assert!(!err.contains("SENTINEL_TOKEN"), "token leaked: {err}");
    }
}
