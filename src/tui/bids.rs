use crate::tui::data::api_err;
use kiteconnect::KiteConnect;
use kiteconnect::Request;
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Section {
    #[default]
    Ipo,
    Government,
    Auctions,
    CorporateActions,
    Sse,
}

#[derive(Debug, Default)]
pub struct State {
    pub ipo: Option<Vec<Instrument>>,
    pub government: Option<Vec<Instrument>>,
    pub corporate_actions: Option<Vec<Instrument>>,
    pub sse: Option<Vec<Instrument>>,
    pub applications: Option<Vec<Application>>,
    pub auctions_error: Option<String>,
    pub ipo_error: Option<String>,
    pub government_error: Option<String>,
    pub corporate_actions_error: Option<String>,
    pub sse_error: Option<String>,
    pub applications_error: Option<String>,
}

impl State {
    pub fn instruments(&self, section: Section) -> Option<&[Instrument]> {
        match section {
            Section::Ipo => self.ipo.as_deref(),
            Section::Government => self.government.as_deref(),
            Section::Auctions => None,
            Section::CorporateActions => self.corporate_actions.as_deref(),
            Section::Sse => self.sse.as_deref(),
        }
    }

    pub fn error(&self, section: Section) -> Option<&str> {
        match section {
            Section::Ipo => self.ipo_error.as_deref(),
            Section::Government => self.government_error.as_deref(),
            Section::Auctions => None,
            Section::CorporateActions => self.corporate_actions_error.as_deref(),
            Section::Sse => self.sse_error.as_deref(),
        }
    }

    pub fn set_instruments(&mut self, section: Section, result: Result<Vec<Instrument>, String>) {
        let (data, error) = match result {
            Ok(data) => (Some(data), None),
            Err(error) => (None, Some(error)),
        };
        match section {
            Section::Ipo => (self.ipo, self.ipo_error) = (data, error),
            Section::Government => (self.government, self.government_error) = (data, error),
            Section::Auctions => {}
            Section::CorporateActions => {
                (self.corporate_actions, self.corporate_actions_error) = (data, error);
            }
            Section::Sse => (self.sse, self.sse_error) = (data, error),
        }
    }

    pub fn set_applications(&mut self, result: Result<Vec<Application>, String>) {
        match result {
            Ok(data) => {
                self.applications = Some(data);
                self.applications_error = None;
            }
            Err(error) => {
                self.applications = None;
                self.applications_error = Some(error);
            }
        }
    }
}

impl Section {
    pub const ALL: [Self; 5] = [
        Self::Ipo,
        Self::Government,
        Self::Auctions,
        Self::CorporateActions,
        Self::Sse,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Ipo => "IPO",
            Self::Government => "Govt. securities",
            Self::Auctions => "Auctions",
            Self::CorporateActions => "Corporate actions",
            Self::Sse => "SSE IPO",
        }
    }

    pub fn cycle(self, delta: isize) -> Self {
        let index = Self::ALL.iter().position(|item| *item == self).unwrap_or(0);
        let next = (index.cast_signed() + delta).rem_euclid(Self::ALL.len().cast_signed());
        Self::ALL[usize::try_from(next).expect("remainder is nonnegative")]
    }

    pub fn web_url(self) -> &'static str {
        match self {
            Self::Ipo => "https://kite.zerodha.com/bids/ipo",
            Self::Government => "https://kite.zerodha.com/bids/gsecs",
            Self::Auctions => "https://kite.zerodha.com/bids/auctions",
            Self::CorporateActions => "https://kite.zerodha.com/bids/corporate-actions",
            Self::Sse => "https://kite.zerodha.com/bids/sse",
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct InvestorType {
    #[serde(default, deserialize_with = "null_default")]
    pub min_price: f64,
    #[serde(default, deserialize_with = "null_default")]
    pub max_price: f64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct BidOrder {
    #[serde(default)]
    pub status: String,
    #[serde(default, deserialize_with = "null_default")]
    pub price: f64,
    #[serde(default, deserialize_with = "null_default")]
    pub quantity: i64,
}

impl BidOrder {
    pub fn is_live(&self) -> bool {
        !self.status.eq_ignore_ascii_case("failed")
            && !self.status.eq_ignore_ascii_case("cancelled")
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Instrument {
    #[serde(default, deserialize_with = "null_default")]
    pub symbol: String,
    #[serde(default, deserialize_with = "null_default")]
    pub tradingsymbol: String,
    #[serde(default, deserialize_with = "null_default")]
    pub name: String,
    #[serde(default, deserialize_with = "null_default")]
    pub kind: String,
    #[serde(default, deserialize_with = "null_default")]
    pub sub_type: String,
    #[serde(default, deserialize_with = "null_default")]
    pub status: String,
    #[serde(default, deserialize_with = "null_default")]
    pub start_at: String,
    #[serde(default, deserialize_with = "null_default")]
    pub end_at: String,
    #[serde(default, deserialize_with = "null_default")]
    pub series: String,
    #[serde(default, deserialize_with = "null_default")]
    pub min_price: f64,
    #[serde(default, deserialize_with = "null_default")]
    pub min_qty: i64,
    #[serde(default, deserialize_with = "null_default")]
    pub indicative_yield: f64,
    #[serde(default)]
    pub investor_types: Vec<InvestorType>,
    #[serde(default)]
    pub order: Option<BidOrder>,
    #[serde(default)]
    pub orders: Option<Vec<BidOrder>>,
}

impl Instrument {
    pub fn label(&self) -> &str {
        if !self.symbol.is_empty() {
            &self.symbol
        } else if !self.tradingsymbol.is_empty() {
            &self.tradingsymbol
        } else {
            &self.name
        }
    }

    pub fn active_order(&self) -> Option<&BidOrder> {
        self.order.as_ref().or_else(|| {
            self.orders
                .as_deref()
                .and_then(|orders| orders.iter().find(|order| order.is_live()))
        })
    }

    pub fn is_sme(&self) -> bool {
        self.sub_type.eq_ignore_ascii_case("sme")
    }

    pub fn is_open(&self) -> bool {
        matches!(self.status.as_str(), "ongoing" | "preapply")
    }

    pub fn price_band(&self) -> Option<(f64, f64)> {
        let category = self.investor_types.first()?;
        (category.max_price > 0.0).then_some((category.min_price, category.max_price))
    }

    pub fn min_amount(&self) -> Option<f64> {
        let (_, max_price) = self.price_band()?;
        #[expect(
            clippy::cast_precision_loss,
            reason = "display-only minimum bid amount uses f64"
        )]
        let amount = max_price * self.min_qty as f64;
        (self.min_qty > 0).then_some(amount)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Application {
    #[serde(default, deserialize_with = "null_default")]
    pub symbol: String,
    #[serde(default, deserialize_with = "null_default")]
    pub investor_type: String,
    #[serde(default, deserialize_with = "null_default")]
    pub created_at: String,
    #[serde(default, deserialize_with = "null_default")]
    pub status: String,
    #[serde(default, deserialize_with = "null_default")]
    pub allotment_quantity: i64,
}

impl Application {
    pub fn is_live(&self) -> bool {
        !self.status.eq_ignore_ascii_case("failed")
            && !self.status.eq_ignore_ascii_case("cancelled")
    }
}

fn null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug)]
pub enum Event {
    Instruments {
        section: Section,
        result: Result<Vec<Instrument>, String>,
    },
    Applications(Result<Vec<Application>, String>),
}

pub async fn fetch_instruments(
    client: &KiteConnect,
    section: Section,
) -> Result<Vec<Instrument>, String> {
    let (path, query, grouped) = match section {
        Section::Ipo => ("/ipo/instruments", Vec::new(), false),
        Section::Government => ("/bids/gsecs/instruments", Vec::new(), true),
        Section::CorporateActions => ("/bids/corp_actions/instruments", Vec::new(), true),
        Section::Sse => (
            "/bids/ipo/instruments",
            vec![("type".to_string(), "sse".to_string())],
            true,
        ),
        Section::Auctions => return Err("auction holdings use the auction endpoint".to_string()),
    };
    let value = client
        .call(Request::get(path).query(query))
        .raw()
        .await
        .map_err(api_err)?;
    if grouped {
        flatten_grouped(value)
    } else {
        serde_json::from_value(value).map_err(|err| format!("bad IPO instruments payload: {err}"))
    }
}

pub async fn fetch_applications(client: &KiteConnect) -> Result<Vec<Application>, String> {
    let value = client
        .call(Request::get("/ipo/applications"))
        .raw()
        .await
        .map_err(api_err)?;
    serde_json::from_value(value).map_err(|err| format!("bad IPO applications payload: {err}"))
}

pub fn flatten_grouped(value: Value) -> Result<Vec<Instrument>, String> {
    #[derive(Deserialize)]
    struct Group {
        #[serde(default, rename = "type")]
        kind: String,
        #[serde(default)]
        items: Vec<Instrument>,
    }
    let groups: Vec<Group> =
        serde_json::from_value(value).map_err(|err| format!("bad grouped bids payload: {err}"))?;
    Ok(groups
        .into_iter()
        .flat_map(|group| {
            let kind = group.kind;
            group.items.into_iter().map(move |mut item| {
                item.kind.clone_from(&kind);
                item
            })
        })
        .collect())
}

pub fn spawn_refresh_instruments(
    client: KiteConnect,
    tx: mpsc::Sender<crate::tui::event::AppEvent>,
    section: Section,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let result = fetch_instruments(&client, section).await;
        crate::tui::data::send(
            &tx,
            crate::tui::data::DataEvent::Bids(Event::Instruments { section, result }),
        )
        .await;
    })
}

pub fn spawn_refresh_applications(
    client: KiteConnect,
    tx: mpsc::Sender<crate::tui::event::AppEvent>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let result = fetch_applications(&client).await;
        crate::tui::data::send(
            &tx,
            crate::tui::data::DataEvent::Bids(Event::Applications(result)),
        )
        .await;
    })
}

#[cfg(test)]
mod tests {
    use super::{Section, fetch_applications, fetch_instruments, flatten_grouped};
    use crate::tui::data::fetch_auctions;
    use httpmock::prelude::*;
    use kiteconnect::KiteConnect;
    use serde_json::json;

    fn client(server: &MockServer) -> KiteConnect {
        KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("test_token").unwrap())
            .base_url(server.base_url())
            .build()
            .expect("client")
    }

    fn response(data: &serde_json::Value) -> String {
        json!({"status": "success", "data": data}).to_string()
    }

    #[test]
    fn flatten_grouped_keeps_kind() {
        let rows = flatten_grouped(json!([
            {"type": "tbill", "items": [{"id": 1, "name": "T-Bill"}]},
            {"type": "sgb", "items": [{"id": 2, "tradingsymbol": "SGB"}]}
        ]))
        .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].kind, "tbill");
        assert_eq!(rows[1].label(), "SGB");
    }

    #[test]
    fn section_cycle_wraps() {
        assert_eq!(Section::Ipo.cycle(-1), Section::Sse);
        assert_eq!(Section::Sse.cycle(1), Section::Ipo);
        assert_eq!(
            Section::Government.web_url(),
            "https://kite.zerodha.com/bids/gsecs"
        );
    }

    #[tokio::test]
    async fn fetches_all_bid_section_contracts_and_existing_applications() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/ipo/instruments");
            then.status(200).body(response(&json!([])));
        });
        server.mock(|when, then| {
            when.method(GET).path("/bids/gsecs/instruments");
            then.status(200).body(response(&json!([{
                "type": "gsec",
                "items": [{"id": "G1", "symbol": "GSEC", "min_price": null,
                    "max_price": null, "indicative_yield": null}]
            }])));
        });
        server.mock(|when, then| {
            when.method(GET).path("/bids/corp_actions/instruments");
            then.status(200).body(response(&json!([{
                "type": "buyback",
                "items": [{"id": "C1", "symbol": "CORP", "orders": null}]
            }])));
        });
        server.mock(|when, then| {
            when.method(GET)
                .path("/bids/ipo/instruments")
                .query_param("type", "sse");
            then.status(200).body(response(&json!([{
                "type": "sse",
                "items": [{"id": "S1", "symbol": "SSE", "investor_types": [
                    {"code": "IND", "min_investment_amount": null, "cutoff_price": null}
                ]}]
            }])));
        });
        server.mock(|when, then| {
            when.method(GET).path("/ipo/applications");
            then.status(200).body(response(&json!([{
                "id": "A1", "instrument_id": "OLD", "symbol": "OLDIPO",
                "status": "allotted", "allotment_quantity": null
            }])));
        });
        server.mock(|when, then| {
            when.method(GET).path("/portfolio/holdings/auctions");
            then.status(200).body(response(&json!([{
                "tradingsymbol": "AUCTION", "exchange": "NSE", "instrument_token": 1,
                "isin": "", "product": "CNC", "price": 0, "quantity": 2,
                "t1_quantity": 0, "realised_quantity": 0, "authorised_quantity": 0,
                "opening_quantity": 0, "collateral_quantity": 0, "collateral_type": "",
                "discrepancy": false, "average_price": 0, "last_price": 0,
                "close_price": 0, "pnl": 0, "day_change": 0, "day_change_percentage": 0,
                "auction_number": "7"
            }])));
        });

        let client = client(&server);
        assert!(
            fetch_instruments(&client, Section::Ipo)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            fetch_instruments(&client, Section::Government)
                .await
                .unwrap()[0]
                .label(),
            "GSEC"
        );
        assert_eq!(
            fetch_instruments(&client, Section::CorporateActions)
                .await
                .unwrap()[0]
                .kind,
            "buyback"
        );
        assert_eq!(
            fetch_instruments(&client, Section::Sse).await.unwrap()[0].label(),
            "SSE"
        );
        assert_eq!(
            fetch_applications(&client).await.unwrap()[0].symbol,
            "OLDIPO"
        );
        assert_eq!(
            fetch_auctions(&client).await.unwrap()[0].auction_number,
            "7"
        );
    }

    #[test]
    fn nullable_numeric_bid_fields_default_without_dropping_issue() {
        let instrument: super::Instrument = serde_json::from_value(json!({
            "id": "IPO1",
            "symbol": "NULLABLE",
            "isin": null,
            "start_at": null,
            "bond_series": null,
            "min_price": null,
            "max_price": null,
            "indicative_yield": null,
            "investor_types": [{"code": "IND", "description": null, "cutoff_price": null}],
            "order": {"status": "PLACED", "quantity": null, "price": null,
                "bids": [{"quantity": null, "price": null}]}
        }))
        .expect("nullable bid payload");
        assert_eq!(instrument.label(), "NULLABLE");
        assert_eq!(instrument.active_order().unwrap().status, "PLACED");
        assert_eq!(instrument.min_price.to_bits(), 0.0_f64.to_bits());
        assert!(instrument.price_band().is_none());
        assert!(instrument.min_amount().is_none());
    }

    #[test]
    fn ipo_price_band_and_min_amount_follow_kite_web() {
        let instrument: super::Instrument = serde_json::from_value(json!({
            "symbol": "PRANAV",
            "status": "ongoing",
            "sub_type": "SME",
            "min_qty": 120,
            "investor_types": [
                {"code": "IND", "min_price": 118, "max_price": 124},
                {"code": "SHA", "min_price": 118, "max_price": 124}
            ],
            "orders": [
                {"status": "cancelled", "price": 124, "quantity": 120},
                {"status": "PLACED", "price": 124, "quantity": 240}
            ]
        }))
        .expect("ipo payload");
        assert!(instrument.is_open());
        assert!(instrument.is_sme());
        assert_eq!(instrument.price_band(), Some((118.0, 124.0)));
        assert_eq!(instrument.min_amount(), Some(14_880.0));
        assert_eq!(instrument.active_order().unwrap().quantity, 240);
    }
}
