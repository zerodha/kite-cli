use crate::{AccessToken, Error, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::{mpsc, watch};

mod conn;
mod packet;
pub use packet::parse_binary;

/// Default Kite Connect ticker endpoint.
pub const DEFAULT_TICKER_URL: &str = "wss://ws.kite.trade";

/// Quote detail requested from the Kite ticker.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Last traded price only.
    Ltp,
    /// Price, volume, and OHLC fields.
    Quote,
    /// Quote fields, timestamps, open interest, and market depth.
    Full,
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mode::Ltp => write!(f, "ltp"),
            Mode::Quote => write!(f, "quote"),
            Mode::Full => write!(f, "full"),
        }
    }
}

/// One price level of the 5-level order book in a `full` mode packet.
#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[non_exhaustive]
pub struct DepthEntry {
    /// Total quantity at this price level.
    pub quantity: u32,
    /// Price at this order book level.
    pub price: f64,
    /// Number of orders at this price level.
    pub orders: u16,
}

/// Five-level bid and offer depth from the Kite ticker.
#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[non_exhaustive]
pub struct MarketDepth {
    /// Bid levels in ticker order.
    pub buy: Vec<DepthEntry>,
    /// Offer levels in ticker order.
    pub sell: Vec<DepthEntry>,
}

/// Quote fields decoded from a Kite ticker binary packet.
#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[non_exhaustive]
pub struct Tick {
    /// Instrument token for this quote.
    pub instrument_token: u32,
    /// Whether this quote uses the index packet format.
    pub is_index: bool,
    /// Last traded price.
    pub last_price: Option<f64>,
    /// Quantity of the last trade.
    pub last_quantity: Option<u32>,
    /// Average traded price for the day.
    pub average_price: Option<f64>,
    /// Total traded volume for the day.
    pub volume: Option<u64>,
    /// Total buy quantity.
    pub buy_quantity: Option<u64>,
    /// Total sell quantity.
    pub sell_quantity: Option<u64>,
    /// Opening price for the day.
    pub open: Option<f64>,
    /// Highest traded price for the day.
    pub high: Option<f64>,
    /// Lowest traded price for the day.
    pub low: Option<f64>,
    /// Previous closing price.
    pub close: Option<f64>,
    /// Current open interest.
    pub oi: Option<u64>,
    /// Highest open interest for the day.
    pub oi_day_high: Option<u64>,
    /// Lowest open interest for the day.
    pub oi_day_low: Option<u64>,
    /// Last trade timestamp in Unix seconds.
    pub last_trade_time: Option<u32>,
    /// Exchange timestamp in Unix seconds.
    pub exchange_time: Option<u32>,
    /// Five-level bid and offer depth when available.
    pub depth: Option<MarketDepth>,
}

/// Builder for a Kite Connect ticker task.
#[derive(Debug)]
#[must_use = "a ticker builder does nothing until spawned"]
pub struct TickerBuilder {
    pub(crate) api_key: String,
    pub(crate) access_token: Option<AccessToken>,
    pub(crate) url: url::Url,
    pub(crate) user_agent: String,
    pub(crate) headers: reqwest::header::HeaderMap,
}

impl TickerBuilder {
    /// Set the Kite ticker WebSocket endpoint.
    pub fn url(mut self, url: url::Url) -> Self {
        self.url = url;
        self
    }

    /// Spawn the Kite ticker on the current Tokio runtime.
    ///
    /// # Errors
    /// Returns [`Error::NoAccessToken`] if no token is set, or [`Error::InvalidInput`]
    /// if the URL is not a ws/wss URL with a host, contains a query, or no Tokio runtime is active.
    pub fn spawn(self) -> Result<(KiteTicker, TickerEvents)> {
        let Some(access_token) = self.access_token else {
            return Err(Error::NoAccessToken);
        };
        if !matches!(self.url.scheme(), "ws" | "wss")
            || self.url.host_str().is_none()
            || self.url.query().is_some()
        {
            return Err(Error::InvalidInput(
                "ticker URL must be a WS or WSS host without a query",
            ));
        }
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|_| Error::InvalidInput("ticker requires a Tokio runtime"))?;
        let (commands, receiver) = mpsc::unbounded_channel();
        let (stop, mut stopped) = watch::channel(());
        let (events, rx) = mpsc::channel(1024);
        let url = conn::ticker_url(self.url, &self.api_key, access_token.expose());
        runtime.spawn(async move {
            let reason = tokio::select! {
                biased;
                _ = stopped.changed() => CloseReason::Requested,
                () = events.closed() => return,
                reason = conn::run(&url, &self.user_agent, &self.headers, receiver, &events) => reason,
            };
            let _ = events.send(TickerEvent::Closed(reason)).await;
        });
        Ok((KiteTicker { commands, stop }, TickerEvents { rx }))
    }
}

/// Cloneable command handle for the Kite ticker.
#[derive(Debug, Clone)]
pub struct KiteTicker {
    commands: mpsc::UnboundedSender<Command>,
    stop: watch::Sender<()>,
}

impl KiteTicker {
    /// Subscribe instrument tokens on the Kite ticker at the requested mode.
    ///
    /// # Errors
    /// Returns [`Error::InvalidInput`] if `tokens` is empty, contains more than
    /// 3,000 entries, or the ticker task has closed.
    pub fn subscribe(&self, tokens: &[u32], mode: Mode) -> Result<()> {
        validate_tokens(tokens)?;
        self.send(Command::Subscribe(tokens.to_vec(), mode))
    }

    /// Unsubscribe instrument tokens from the Kite ticker.
    ///
    /// # Errors
    /// Returns [`Error::InvalidInput`] if `tokens` is empty, contains more than
    /// 3,000 entries, or the ticker task has closed.
    pub fn unsubscribe(&self, tokens: &[u32]) -> Result<()> {
        validate_tokens(tokens)?;
        self.send(Command::Unsubscribe(tokens.to_vec()))
    }

    /// Change the mode of subscribed instrument tokens on the Kite ticker.
    ///
    /// # Errors
    /// Returns [`Error::InvalidInput`] if `tokens` is empty, contains more than
    /// 3,000 entries, or the ticker task has closed.
    pub fn set_mode(&self, mode: Mode, tokens: &[u32]) -> Result<()> {
        validate_tokens(tokens)?;
        self.send(Command::SetMode(mode, tokens.to_vec()))
    }

    /// Replace the Kite ticker subscription set.
    ///
    /// # Errors
    /// Returns [`Error::InvalidInput`] if `all` contains more than 3,000 entries
    /// or the ticker task has closed.
    pub fn set_subscriptions(&self, all: HashMap<u32, Mode>) -> Result<()> {
        if all.len() > 3000 {
            return Err(Error::InvalidInput(
                "Kite WebSocket supports up to 3000 instruments per connection",
            ));
        }
        self.send(Command::SetSubscriptions(all))
    }

    /// Close the Kite ticker task and stop reconnecting.
    pub fn close(&self) {
        let _ = self.stop.send(());
    }

    fn send(&self, command: Command) -> Result<()> {
        self.commands
            .send(command)
            .map_err(|_| Error::InvalidInput("ticker is closed"))
    }
}

/// Bounded receiver of Kite ticker events. A slow consumer pauses socket
/// reads instead of losing events. `Closed` is the last event.
#[derive(Debug)]
pub struct TickerEvents {
    rx: mpsc::Receiver<TickerEvent>,
}

impl TickerEvents {
    /// Receive the next Kite ticker event.
    pub async fn recv(&mut self) -> Option<TickerEvent> {
        self.rx.recv().await
    }
}

/// Event received from the Kite ticker.
#[derive(Debug)]
#[non_exhaustive]
pub enum TickerEvent {
    /// Sent after each successful WebSocket handshake, before subscriptions are replayed.
    Connected,
    /// Quotes decoded from one binary frame.
    Ticks(Vec<Tick>),
    /// An order update decoded from a text frame.
    OrderUpdate(Box<crate::Order>),
    /// An order update text frame that does not decode; the ticker keeps streaming.
    InvalidOrderUpdate {
        /// Decode error for the order update.
        error: String,
        /// Order update frame text with secret values redacted.
        frame: String,
    },
    /// A retryable connection failure triggered a reconnect.
    Reconnecting {
        /// Reconnect attempt number, starting at one.
        attempt: u32,
        /// Delay before the next connection attempt.
        delay: Duration,
        /// Reason the previous connection failed.
        reason: String,
    },
    /// A text frame that is not an order update.
    Message(String),
    /// The ticker stopped and sends no more events.
    Closed(CloseReason),
}

/// Reason the Kite ticker stopped.
#[derive(Debug)]
#[non_exhaustive]
pub enum CloseReason {
    /// A handle requested closure or all handles were dropped.
    Requested,
    /// The endpoint rejected the authentication credentials.
    AuthRejected(String),
    /// The endpoint refused the WebSocket handshake.
    HandshakeRejected(String),
    /// The subscription set exceeded the per-connection limit.
    SubscriptionLimit,
}

impl std::fmt::Display for CloseReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Requested => f.write_str("ticker closed"),
            Self::AuthRejected(message) | Self::HandshakeRejected(message) => f.write_str(message),
            Self::SubscriptionLimit => f.write_str("ticker subscription limit exceeded"),
        }
    }
}

#[derive(Debug)]
enum Command {
    Subscribe(Vec<u32>, Mode),
    Unsubscribe(Vec<u32>),
    SetMode(Mode, Vec<u32>),
    SetSubscriptions(HashMap<u32, Mode>),
}

fn validate_tokens(tokens: &[u32]) -> Result<()> {
    if tokens.is_empty() {
        return Err(Error::InvalidInput(
            "at least one instrument token is required",
        ));
    }
    if tokens.len() > 3000 {
        return Err(Error::InvalidInput(
            "Kite WebSocket supports up to 3000 instruments per connection",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::KiteConnect;
    use futures_util::{SinkExt, StreamExt};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::oneshot;
    use tokio::time::timeout;
    use tokio_tungstenite::{WebSocketStream, tungstenite::Message};

    fn client() -> KiteConnect {
        KiteConnect::builder("test-key")
            .access_token(AccessToken::new("test-token").unwrap())
            .app_name("ticker-test/1.0")
            .build()
            .unwrap()
    }

    async fn listener() -> (TcpListener, url::Url) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}/socket", listener.local_addr().unwrap())
            .parse()
            .unwrap();
        (listener, url)
    }

    async fn accept(listener: &TcpListener) -> WebSocketStream<TcpStream> {
        let (stream, _) = timeout(Duration::from_secs(5), listener.accept())
            .await
            .unwrap()
            .unwrap();
        tokio_tungstenite::accept_async(stream).await.unwrap()
    }

    async fn text(ws: &mut WebSocketStream<TcpStream>) -> String {
        match timeout(Duration::from_secs(5), ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap()
        {
            Message::Text(text) => text.to_string(),
            other => panic!("expected a JSON text frame, received {other:?}"),
        }
    }

    async fn next(events: &mut TickerEvents) -> TickerEvent {
        timeout(Duration::from_secs(5), events.recv())
            .await
            .unwrap()
            .unwrap()
    }

    async fn connected(events: &mut TickerEvents) {
        match next(events).await {
            TickerEvent::Connected => {}
            other => panic!("unexpected event before connection: {other:?}"),
        }
    }

    #[test]
    fn spawn_requires_a_token_and_redacts_debug() {
        let builder = KiteConnect::builder("test-key").build().unwrap().ticker();
        assert!(matches!(builder.spawn(), Err(Error::NoAccessToken)));
        let client = KiteConnect::builder("test-key")
            .access_token(AccessToken::new("SENTINEL-TOKEN-91c2").unwrap())
            .build()
            .unwrap();
        let debug = format!("{:?}", client.ticker());
        assert!(!debug.contains("SENTINEL-TOKEN-91c2"));
        assert!(debug.contains("<redacted>"));
        assert_eq!(client.ticker().url.as_str(), "wss://ws.kite.trade/");
    }

    #[tokio::test]
    async fn ticker_uses_client_credentials_url_and_shared_user_agent() {
        use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};

        #[expect(
            clippy::result_large_err,
            clippy::unnecessary_wraps,
            reason = "tungstenite requires this callback Result and error type"
        )]
        fn check_handshake(
            request: &Request,
            response: Response,
        ) -> std::result::Result<Response, ErrorResponse> {
            assert_eq!(request.uri().path(), "/socket");
            assert_eq!(
                request.uri().query(),
                Some("api_key=test-key&access_token=test-token")
            );
            assert_eq!(
                request.headers()["user-agent"],
                "KiteCLI/0.1.0 (x86_64; Linux 6.1.0)"
            );
            assert_eq!(
                request.headers()["x-kite-app-uuid"],
                "5b697354-64d2-49c4-a080-6b25475555e3"
            );
            assert!(!request.headers().contains_key("authorization"));
            assert!(!request.headers().contains_key("x-kite-version"));
            Ok(response)
        }

        let (listener, url) = listener().await;
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            tokio_tungstenite::accept_hdr_async(stream, check_handshake)
                .await
                .unwrap()
        });
        let (ticker, mut events) = crate::KiteConnect::builder("test-key")
            .access_token(AccessToken::new("test-token").unwrap())
            .user_agent("KiteCLI/0.1.0 (x86_64; Linux 6.1.0)")
            .header("X-Kite-App-UUID", "5b697354-64d2-49c4-a080-6b25475555e3")
            .header("Authorization", "bad-auth")
            .header("X-Kite-Version", "bad-version")
            .header("User-Agent", "bad-agent")
            .build()
            .unwrap()
            .ticker()
            .url(url)
            .spawn()
            .unwrap();
        connected(&mut events).await;
        let _ws = server.await.unwrap();
        ticker.close();
        assert!(matches!(
            next(&mut events).await,
            TickerEvent::Closed(CloseReason::Requested)
        ));
        assert!(events.recv().await.is_none());
    }

    #[tokio::test]
    async fn commands_send_the_existing_json_text_frames() {
        let (listener, url) = listener().await;
        let (ticker, mut events) = client().ticker().url(url).spawn().unwrap();
        let server = tokio::spawn(async move { accept(&listener).await });
        connected(&mut events).await;
        let mut ws = server.await.unwrap();
        ticker.subscribe(&[408_065], Mode::Full).unwrap();
        assert_eq!(text(&mut ws).await, r#"{"a":"subscribe","v":[408065]}"#);
        assert_eq!(text(&mut ws).await, r#"{"a":"mode","v":["full",[408065]]}"#);
        let clone = ticker.clone();
        clone.subscribe(&[884_737], Mode::Ltp).unwrap();
        assert_eq!(text(&mut ws).await, r#"{"a":"subscribe","v":[884737]}"#);
        assert_eq!(text(&mut ws).await, r#"{"a":"mode","v":["ltp",[884737]]}"#);
        ticker.set_mode(Mode::Quote, &[408_065]).unwrap();
        assert_eq!(
            text(&mut ws).await,
            r#"{"a":"mode","v":["quote",[408065]]}"#
        );
        clone.unsubscribe(&[884_737]).unwrap();
        assert_eq!(text(&mut ws).await, r#"{"a":"unsubscribe","v":[884737]}"#);
        ticker.close();
    }

    #[tokio::test]
    async fn reconnect_replays_changes_received_during_backoff() {
        let (listener, url) = listener().await;
        let (ticker, mut events) = client().ticker().url(url).spawn().unwrap();
        ticker.subscribe(&[408_065], Mode::Full).unwrap();
        let (ready, first) = oneshot::channel();
        let server = tokio::spawn(async move {
            let mut ws = accept(&listener).await;
            assert_eq!(text(&mut ws).await, r#"{"a":"subscribe","v":[408065]}"#);
            assert_eq!(text(&mut ws).await, r#"{"a":"mode","v":["full",[408065]]}"#);
            ws.send(Message::Close(None)).await.unwrap();
            let _ = ready.send(());
            let mut ws = accept(&listener).await;
            assert_eq!(text(&mut ws).await, r#"{"a":"subscribe","v":[884737]}"#);
            assert_eq!(
                text(&mut ws).await,
                r#"{"a":"mode","v":["quote",[884737]]}"#
            );
            ws
        });
        connected(&mut events).await;
        first.await.unwrap();
        loop {
            if let TickerEvent::Reconnecting {
                attempt,
                delay,
                reason,
            } = next(&mut events).await
            {
                assert_eq!(attempt, 1);
                assert_eq!(delay, Duration::from_secs(1));
                assert!(
                    reason.starts_with("connection failed: server closed connection; 127.0.0.1:"),
                    "{reason}"
                );
                break;
            }
        }
        ticker
            .set_subscriptions(HashMap::from([(884_737, Mode::Quote)]))
            .unwrap();
        connected(&mut events).await;
        let _ws = server.await.unwrap();
        ticker.close();
    }

    #[tokio::test]
    async fn order_text_frame_decodes_the_postback_fixture() {
        let (listener, url) = listener().await;
        let server = tokio::spawn(async move {
            let mut ws = accept(&listener).await;
            let postback: serde_json::Value =
                serde_json::from_str(include_str!("../../../../mock_responses/postback.json"))
                    .unwrap();
            ws.send(Message::Text(
                serde_json::json!({"type": "order", "data": postback})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
            ws
        });
        let (ticker, mut events) = client().ticker().url(url).spawn().unwrap();
        connected(&mut events).await;
        match next(&mut events).await {
            TickerEvent::OrderUpdate(order) => {
                assert_eq!(order.order_id, "220303000308932");
                assert_eq!(order.tradingsymbol, "SBIN");
                assert_eq!(order.status, "COMPLETE");
                assert_eq!(order.average_price.to_bits(), f64::to_bits(470.0));
            }
            other => panic!("expected the typed postback: {other:?}"),
        }
        let _ws = server.await.unwrap();
        ticker.close();
    }

    #[tokio::test]
    async fn dropping_every_handle_closes_the_task() {
        let (listener, url) = listener().await;
        let server = tokio::spawn(async move { accept(&listener).await });
        let (ticker, mut events) = client().ticker().url(url).spawn().unwrap();
        connected(&mut events).await;
        let _ws = server.await.unwrap();
        let clone = ticker.clone();
        drop(ticker);
        assert!(
            timeout(Duration::from_millis(10), events.recv())
                .await
                .is_err()
        );
        drop(clone);
        assert!(matches!(
            next(&mut events).await,
            TickerEvent::Closed(CloseReason::Requested)
        ));
        assert!(events.recv().await.is_none());
    }

    #[tokio::test]
    async fn invalid_urls_and_subscription_sizes_fail_before_connecting() {
        for url in ["https://example.com", "ws://example.com?query=1"] {
            assert!(client().ticker().url(url.parse().unwrap()).spawn().is_err());
        }
        let (ticker, _events) = client().ticker().spawn().unwrap();
        assert!(ticker.subscribe(&[], Mode::Full).is_err());
        assert!(ticker.subscribe(&vec![1; 3001], Mode::Full).is_err());
        assert!(
            ticker
                .set_subscriptions((0..3001).map(|token| (token, Mode::Ltp)).collect())
                .is_err()
        );
        ticker.close();
    }

    fn ltp_frame(token: u32) -> Vec<u8> {
        let mut frame = vec![0, 1, 0, 8];
        frame.extend(token.to_be_bytes());
        frame.extend(157_315_i32.to_be_bytes());
        frame
    }

    #[tokio::test]
    async fn a_stalled_consumer_backpressures_without_losing_events() {
        const FRAMES: u32 = 2500;
        let (listener, url) = listener().await;
        let (sent, all_sent) = oneshot::channel();
        let server = tokio::spawn(async move {
            let mut ws = accept(&listener).await;
            let postback: serde_json::Value =
                serde_json::from_str(include_str!("../../../../mock_responses/postback.json"))
                    .unwrap();
            ws.send(Message::Text(
                serde_json::json!({"type": "order", "data": postback})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
            for token in 1..=FRAMES {
                ws.send(Message::Binary(ltp_frame(token).into()))
                    .await
                    .unwrap();
            }
            sent.send(()).unwrap();
            ws
        });
        let (ticker, mut events) = client().ticker().url(url).spawn().unwrap();
        connected(&mut events).await;
        timeout(Duration::from_secs(5), all_sent)
            .await
            .unwrap()
            .unwrap();
        for token in 1..=40 {
            ticker.subscribe(&[token], Mode::Ltp).unwrap();
        }
        let _ws = server.await.unwrap();
        match next(&mut events).await {
            TickerEvent::OrderUpdate(order) => assert_eq!(order.order_id, "220303000308932"),
            other => panic!("queued order was lost: {other:?}"),
        }
        for token in 1..=FRAMES {
            match next(&mut events).await {
                TickerEvent::Ticks(ticks) => {
                    assert_eq!(ticks.len(), 1);
                    assert_eq!(ticks[0].instrument_token, token);
                }
                other => panic!("tick frame {token} was lost: {other:?}"),
            }
        }
        assert!(
            timeout(Duration::from_millis(50), events.recv())
                .await
                .is_err()
        );
        ticker.close();
        assert!(matches!(
            next(&mut events).await,
            TickerEvent::Closed(CloseReason::Requested)
        ));
        assert!(events.recv().await.is_none());
    }

    #[tokio::test]
    async fn a_bad_order_frame_is_reported_and_streaming_continues() {
        let (listener, url) = listener().await;
        let bad = r#"{"type":"order","data":{"order_id":7}}"#;
        let server = tokio::spawn(async move {
            let mut ws = accept(&listener).await;
            ws.send(Message::Text(bad.into())).await.unwrap();
            ws.send(Message::Binary(ltp_frame(408_065).into()))
                .await
                .unwrap();
            ws
        });
        let (ticker, mut events) = client().ticker().url(url).spawn().unwrap();
        connected(&mut events).await;
        match next(&mut events).await {
            TickerEvent::InvalidOrderUpdate { error, frame } => {
                assert!(error.starts_with("invalid type: integer `7`"), "{error}");
                assert_eq!(frame, bad);
            }
            other => panic!("expected the invalid order update: {other:?}"),
        }
        match next(&mut events).await {
            TickerEvent::Ticks(ticks) => assert_eq!(ticks[0].instrument_token, 408_065),
            other => panic!("the feed ended after a bad order frame: {other:?}"),
        }
        let _ws = server.await.unwrap();
        ticker.close();
    }

    #[tokio::test]
    async fn dropping_events_stops_the_socket_task() {
        let (listener, url) = listener().await;
        let server = tokio::spawn(async move { accept(&listener).await });
        let (ticker, mut events) = client().ticker().url(url).spawn().unwrap();
        connected(&mut events).await;
        let mut ws = server.await.unwrap();
        drop(events);
        timeout(Duration::from_secs(5), ticker.commands.closed())
            .await
            .unwrap();
        assert!(ticker.subscribe(&[408_065], Mode::Ltp).is_err());
        let ended = timeout(Duration::from_secs(5), ws.next()).await.unwrap();
        assert!(matches!(ended, None | Some(Err(_))));
    }
}
