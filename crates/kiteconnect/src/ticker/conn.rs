use super::{CloseReason, Command, Mode, TickerEvent, parse_binary};
use crate::Result;
use crate::error::{sanitize_error, snippet};
use futures_util::{SinkExt, StreamExt};
use std::collections::{HashMap, HashSet};
use std::fmt::{self, Write};
use std::future::Future;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::{TcpStream, lookup_host};
use tokio::sync::mpsc;
use tokio::time::{Instant, MissedTickBehavior};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::error::UrlError;
use tokio_tungstenite::tungstenite::handshake::client::{Request, Response};
use tokio_tungstenite::tungstenite::http::{HeaderMap, HeaderValue, header::USER_AGENT};
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const ADDRESS_ATTEMPT_CAP: Duration = Duration::from_secs(5);

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct ConnectedWs {
    stream: WsStream,
    route: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectStage {
    Tcp,
    TlsOrUpgrade,
    Tls,
    Upgrade,
}

impl fmt::Display for ConnectStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tcp => f.write_str("tcp"),
            Self::TlsOrUpgrade => f.write_str("tls/upgrade"),
            Self::Tls => f.write_str("tls"),
            Self::Upgrade => f.write_str("upgrade"),
        }
    }
}

#[derive(Debug)]
struct AddressFailure {
    address: SocketAddr,
    stage: ConnectStage,
    detail: String,
}

#[derive(Debug)]
enum ConnectError {
    Http {
        response: Box<Response>,
        message: String,
    },
    Retryable(String),
}

impl fmt::Display for ConnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http { message, .. } | Self::Retryable(message) => f.write_str(message),
        }
    }
}

enum AttemptError {
    Http(Box<Response>),
    Transport { stage: ConnectStage, error: WsError },
    Timeout { stage: ConnectStage },
}

#[derive(Debug)]
struct CandidateSuccess<T> {
    value: T,
    address: SocketAddr,
    resolved: Vec<SocketAddr>,
}

pub(super) async fn run(
    url: &str,
    user_agent: &str,
    headers: &HeaderMap,
    mut commands: mpsc::UnboundedReceiver<Command>,
    events: &mpsc::Sender<TickerEvent>,
) -> CloseReason {
    let mut desired = HashMap::new();
    let mut attempt: u32 = 0;
    loop {
        while let Ok(command) = commands.try_recv() {
            if let Err(reason) = update_desired(&mut desired, command) {
                return reason;
            }
        }
        let connection = connect(url, user_agent, headers);
        tokio::pin!(connection);
        let connected = loop {
            tokio::select! {
                result = &mut connection => break result,
                command = commands.recv() => {
                    let Some(command) = command else { return CloseReason::Requested };
                    if let Err(reason) = update_desired(&mut desired, command) {
                        return reason;
                    }
                }
            }
        };
        let failure = match connected {
            Ok(ConnectedWs { stream, route }) => {
                if let Err(reason) = emit(events, TickerEvent::Connected).await {
                    return reason;
                }
                let started = Instant::now();
                match run_connection(stream, &mut desired, &mut commands, events, &route).await {
                    ConnectionEnd::Retry(failure) => {
                        if started.elapsed() >= Duration::from_secs(30) {
                            attempt = 0;
                        }
                        failure
                    }
                    ConnectionEnd::Closed(reason) => return reason,
                }
            }
            Err(err) => {
                if let Some(reason) = fatal_reason(&err) {
                    return reason;
                }
                format!("connect failed: {err}")
            }
        };
        attempt = attempt.saturating_add(1);
        let delay = backoff(attempt);
        let reconnecting = TickerEvent::Reconnecting {
            attempt,
            delay,
            reason: failure,
        };
        if let Err(reason) = emit(events, reconnecting).await {
            return reason;
        }
        let deadline = Instant::now() + delay;
        loop {
            tokio::select! {
                () = tokio::time::sleep_until(deadline) => break,
                command = commands.recv() => {
                    let Some(command) = command else { return CloseReason::Requested };
                    if let Err(reason) = update_desired(&mut desired, command) {
                        return reason;
                    }
                }
            }
        }
    }
}

enum ConnectionEnd {
    Retry(String),
    Closed(CloseReason),
}

fn update_desired(
    desired: &mut HashMap<u32, Mode>,
    command: Command,
) -> std::result::Result<(), CloseReason> {
    match command {
        Command::Subscribe(tokens, mode) => {
            for token in tokens {
                desired.insert(token, mode);
            }
        }
        Command::Unsubscribe(tokens) => {
            for token in tokens {
                desired.remove(&token);
            }
        }
        Command::SetMode(mode, tokens) => {
            for token in tokens {
                if let Some(current) = desired.get_mut(&token) {
                    *current = mode;
                }
            }
        }
        Command::SetSubscriptions(all) => *desired = all,
    }
    if desired.len() > 3000 {
        return Err(CloseReason::SubscriptionLimit);
    }
    Ok(())
}

async fn emit(
    events: &mpsc::Sender<TickerEvent>,
    event: TickerEvent,
) -> std::result::Result<(), CloseReason> {
    events.send(event).await.map_err(|_| CloseReason::Requested)
}

pub(super) fn ticker_url(mut url: url::Url, api_key: &str, access_token: &str) -> String {
    url.query_pairs_mut()
        .append_pair("api_key", api_key)
        .append_pair("access_token", access_token);
    url.to_string()
}

fn websocket_request(
    url: &str,
    user_agent: &str,
    headers: &HeaderMap,
) -> std::result::Result<Request, WsError> {
    let mut request = url.into_client_request()?;
    let user_agent =
        HeaderValue::from_str(user_agent).map_err(|error| WsError::HttpFormat(error.into()))?;
    for (name, value) in headers {
        request.headers_mut().entry(name).or_insert(value.clone());
    }
    request.headers_mut().insert(USER_AGENT, user_agent);
    Ok(request)
}

fn connection_target(request: &Request) -> std::result::Result<(String, u16), UrlError> {
    let port = match request.uri().scheme_str() {
        Some("wss") => request.uri().port_u16().unwrap_or(443),
        Some("ws") => request.uri().port_u16().unwrap_or(80),
        _ => return Err(UrlError::UnsupportedUrlScheme),
    };
    let host = request.uri().host().ok_or(UrlError::NoHostName)?;
    // Match tokio-tungstenite's `domain()`: rustls and `lookup_host` both need
    // an IPv6 literal without URI brackets.
    let host = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host)
        .to_string();
    Ok((host, port))
}

fn deduplicate_addresses(addresses: impl IntoIterator<Item = SocketAddr>) -> Vec<SocketAddr> {
    let mut seen = HashSet::new();
    addresses
        .into_iter()
        .filter(|address| seen.insert(*address))
        .collect()
}

async fn connect_candidates<T, F, Fut>(
    resolved: Vec<SocketAddr>,
    deadline: Instant,
    total_timeout: Duration,
    attempt_cap: Duration,
    mut attempt: F,
) -> std::result::Result<CandidateSuccess<T>, ConnectError>
where
    F: FnMut(SocketAddr, Instant) -> Fut,
    Fut: Future<Output = std::result::Result<T, AttemptError>>,
{
    let mut failures = Vec::new();
    for (index, address) in resolved.iter().copied().enumerate() {
        let now = Instant::now();
        if now >= deadline {
            return Err(deadline_error(
                total_timeout,
                address,
                ConnectStage::Tcp,
                &resolved,
                &failures,
            ));
        }

        let candidates_left = u32::try_from(resolved.len() - index).unwrap_or(u32::MAX);
        let remaining = deadline.duration_since(now);
        let attempt_timeout = if candidates_left == 1 {
            remaining
        } else {
            (remaining / candidates_left).min(attempt_cap)
        };
        let attempt_deadline = now + attempt_timeout;

        match attempt(address, attempt_deadline).await {
            Ok(value) => {
                return Ok(CandidateSuccess {
                    value,
                    address,
                    resolved,
                });
            }
            Err(AttemptError::Http(response)) => {
                let status = response.status();
                let detail = http_response_detail(&response);
                if should_try_next_address(status.as_u16()) {
                    failures.push(AddressFailure {
                        address,
                        stage: ConnectStage::Upgrade,
                        detail: format!("HTTP {status}{detail}"),
                    });
                    continue;
                }
                let message = format!(
                    "upgrade at {address} returned HTTP {status}{detail}; resolved order [{}]",
                    display_addresses(&resolved)
                );
                return Err(ConnectError::Http { response, message });
            }
            Err(AttemptError::Transport { stage, error }) => {
                failures.push(AddressFailure {
                    address,
                    stage,
                    detail: transport_error_detail(error),
                });
            }
            Err(AttemptError::Timeout { stage }) => {
                if attempt_deadline >= deadline {
                    return Err(deadline_error(
                        total_timeout,
                        address,
                        stage,
                        &resolved,
                        &failures,
                    ));
                }
                failures.push(AddressFailure {
                    address,
                    stage,
                    detail: format!("timed out after {}", display_duration(attempt_timeout)),
                });
            }
        }
    }

    Err(ConnectError::Retryable(format!(
        "all {} addresses failed; resolved order [{}]: {}",
        resolved.len(),
        display_addresses(&resolved),
        display_failures(&failures)
    )))
}

fn deadline_error(
    total_timeout: Duration,
    address: SocketAddr,
    stage: ConnectStage,
    resolved: &[SocketAddr],
    failures: &[AddressFailure],
) -> ConnectError {
    let mut message = format!(
        "connect deadline exceeded after {} at {stage} for {address}; resolved order [{}]",
        display_duration(total_timeout),
        display_addresses(resolved)
    );
    if !failures.is_empty() {
        write!(message, "; prior failures: {}", display_failures(failures)).unwrap();
    }
    ConnectError::Retryable(message)
}

async fn connect_address(
    request: Request,
    address: SocketAddr,
    deadline: Instant,
) -> std::result::Result<WsStream, AttemptError> {
    let tcp = tokio::time::timeout_at(deadline, TcpStream::connect(address))
        .await
        .map_err(|_| AttemptError::Timeout {
            stage: ConnectStage::Tcp,
        })?
        .map_err(|error| AttemptError::Transport {
            stage: ConnectStage::Tcp,
            error: WsError::Io(error),
        })?;
    tcp.set_nodelay(true)
        .map_err(|error| AttemptError::Transport {
            stage: ConnectStage::Tcp,
            error: WsError::Io(error),
        })?;

    // Passing the original request is essential: `client_async_tls` derives
    // SNI and certificate verification from its URI host, never the peer IP.
    match tokio::time::timeout_at(deadline, tokio_tungstenite::client_async_tls(request, tcp))
        .await
        .map_err(|_| AttemptError::Timeout {
            stage: ConnectStage::TlsOrUpgrade,
        })? {
        Ok((stream, _response)) => Ok(stream),
        Err(WsError::Http(response)) => Err(AttemptError::Http(response)),
        Err(error) => Err(AttemptError::Transport {
            stage: websocket_error_stage(&error),
            error,
        }),
    }
}

async fn connect(
    url: &str,
    user_agent: &str,
    headers: &HeaderMap,
) -> std::result::Result<ConnectedWs, ConnectError> {
    let request = websocket_request(url, user_agent, headers).map_err(|error| {
        ConnectError::Retryable(format!("request rejected before dns: {error}"))
    })?;
    let (host, port) = connection_target(&request).map_err(|error| {
        ConnectError::Retryable(format!(
            "request rejected before dns: {}",
            WsError::Url(error)
        ))
    })?;
    let started = Instant::now();
    let deadline = started + CONNECT_TIMEOUT;

    // Tokio's `lookup_host` runs getaddrinfo on `spawn_blocking`; timing out
    // abandons this waiter, but the OS call itself cannot be cancelled.
    let addresses = tokio::time::timeout_at(deadline, lookup_host((host.as_str(), port)))
        .await
        .map_err(|_| {
            ConnectError::Retryable(format!(
                "connect deadline exceeded after {} at dns for {host}",
                display_duration(CONNECT_TIMEOUT)
            ))
        })?
        .map_err(|error| ConnectError::Retryable(format!("dns for {host} failed: {error}")))?;
    let resolved = deduplicate_addresses(addresses);
    if resolved.is_empty() {
        return Err(ConnectError::Retryable(format!(
            "dns for {host} returned no addresses"
        )));
    }

    let connected = connect_candidates(
        resolved,
        deadline,
        CONNECT_TIMEOUT,
        ADDRESS_ATTEMPT_CAP,
        |address, attempt_deadline| {
            let request = websocket_request(url, user_agent, headers)
                .expect("a previously validated WebSocket URL must remain valid");
            connect_address(request, address, attempt_deadline)
        },
    )
    .await?;
    let route = format!(
        "{}; resolved order [{}]",
        connected.address,
        display_addresses(&connected.resolved)
    );
    Ok(ConnectedWs {
        stream: connected.value,
        route,
    })
}

/// Whether a failed connect should stop the ticker, and why.
///
/// Only 400, 401, and 403 are fatal. Expanding the set to every 4xx would turn
/// recoverable responses into shutdowns.
fn fatal_reason(err: &ConnectError) -> Option<CloseReason> {
    let ConnectError::Http { response, .. } = err else {
        return None;
    };
    let status = response.status();
    if !matches!(status.as_u16(), 400 | 401 | 403) {
        return None;
    }
    if kite_error_message(response).is_some_and(|message| {
        let normalized = message
            .trim()
            .trim_end_matches('.')
            .replace('`', "")
            .to_ascii_lowercase();
        matches!(
            normalized.as_str(),
            "invalid access_token or api_key" | "authentication failed"
        )
    }) {
        let detail = http_response_detail(response);
        return Some(CloseReason::AuthRejected(format!(
            "WebSocket auth rejected (HTTP {status}){detail}; the access token may have expired"
        )));
    }
    let detail = http_response_detail(response);
    Some(CloseReason::HandshakeRejected(format!(
        "WebSocket handshake refused (HTTP {status}){detail}; the endpoint rejected the request. Check ticker authentication settings, the proxy, firewall, and endpoint access."
    )))
}

fn kite_error_message(response: &Response) -> Option<String> {
    let body = response.body().as_deref()?;
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    if value.get("status")?.as_str()? != "error" {
        return None;
    }
    value.get("message")?.as_str().map(str::to_owned)
}

fn http_response_detail(response: &Response) -> String {
    let Some(body) = response.body().as_deref() else {
        return String::new();
    };
    let detail =
        kite_error_message(response).unwrap_or_else(|| String::from_utf8_lossy(body).into_owned());
    let detail = sanitize_error(&snippet(&detail));
    if detail.is_empty() {
        String::new()
    } else {
        format!(": {detail}")
    }
}

fn should_try_next_address(status: u16) -> bool {
    matches!(status, 408 | 421 | 425 | 500..=599)
}

fn websocket_error_stage(error: &WsError) -> ConnectStage {
    match error {
        WsError::Tls(_) => ConnectStage::Tls,
        WsError::Io(_) => ConnectStage::TlsOrUpgrade,
        _ => ConnectStage::Upgrade,
    }
}

fn transport_error_detail(error: WsError) -> String {
    match error {
        WsError::Io(error) => error.to_string(),
        WsError::Tls(error) => error.to_string(),
        error => error.to_string(),
    }
}

fn display_duration(duration: Duration) -> String {
    if duration.subsec_nanos() == 0 {
        format!("{}s", duration.as_secs())
    } else {
        format!("{}ms", duration.as_millis())
    }
}

fn display_addresses(addresses: &[SocketAddr]) -> String {
    addresses
        .iter()
        .map(SocketAddr::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn display_failures(failures: &[AddressFailure]) -> String {
    failures
        .iter()
        .map(|failure| format!("{} {}: {}", failure.address, failure.stage, failure.detail))
        .collect::<Vec<_>>()
        .join("; ")
}

fn backoff(attempt: u32) -> Duration {
    let secs = 2u64.saturating_pow(attempt.saturating_sub(1).min(5));
    Duration::from_secs(secs.min(30))
}

async fn run_connection(
    mut ws: WsStream,
    desired: &mut HashMap<u32, Mode>,
    commands: &mut mpsc::UnboundedReceiver<Command>,
    events: &mpsc::Sender<TickerEvent>,
    route: &str,
) -> ConnectionEnd {
    let mut current = HashMap::new();
    if let Err(err) = apply_subscriptions(&mut ws, &mut current, desired).await {
        return connection_failure(&format!("subscribe failed: {err}; {route}"));
    }
    let mut last_msg = Instant::now();
    let mut idle = tokio::time::interval(Duration::from_secs(5));
    idle.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            msg = ws.next() => {
                last_msg = Instant::now();
                match msg {
                    Some(Ok(Message::Binary(data))) => {
                        let ticks = parse_binary(&data);
                        if !ticks.is_empty()
                            && let Err(reason) = emit(events, TickerEvent::Ticks(ticks)).await {
                            return ConnectionEnd::Closed(reason);
                        }
                    }
                    Some(Ok(Message::Text(text))) => {
                        if let Err(reason) = handle_text(text.as_str(), events).await {
                            return ConnectionEnd::Closed(reason);
                        }
                    }
                    Some(Ok(Message::Close(_))) => return connection_failure(&format!("connection failed: server closed connection; {route}")),
                    Some(Ok(_)) => {},
                    Some(Err(err)) => return connection_failure(&format!("connection failed: websocket: {err}; {route}")),
                    None => return connection_failure(&format!("connection failed: stream ended; {route}")),
                }
            }
            command = commands.recv() => {
                let Some(command) = command else { return ConnectionEnd::Closed(CloseReason::Requested) };
                if let Err(reason) = update_desired(desired, command) {
                    return ConnectionEnd::Closed(reason);
                }
                if let Err(err) = apply_subscriptions(&mut ws, &mut current, desired).await {
                    return connection_failure(&format!("subscribe failed: {err}; {route}"));
                }
            },
            _ = idle.tick() => {
                if last_msg.elapsed() > Duration::from_secs(20) {
                    return connection_failure(&format!("connection failed: no data for 20s; {route}"));
                }
            }
        }
    }
}

fn connection_failure(message: &str) -> ConnectionEnd {
    ConnectionEnd::Retry(sanitize_error(message))
}

async fn handle_text(
    text: &str,
    events: &mpsc::Sender<TickerEvent>,
) -> std::result::Result<(), CloseReason> {
    #[derive(serde::Deserialize)]
    struct Envelope<'a> {
        #[serde(borrow, rename = "type")]
        kind: &'a str,
        #[serde(default)]
        data: serde_json::Value,
    }
    let event = match serde_json::from_str::<Envelope<'_>>(text) {
        Ok(envelope) if envelope.kind == "order" => match serde_json::from_value(envelope.data) {
            Ok(order) => TickerEvent::OrderUpdate(Box::new(order)),
            Err(error) => TickerEvent::InvalidOrderUpdate {
                error: sanitize_error(&error.to_string()),
                frame: sanitize_error(text),
            },
        },
        _ => TickerEvent::Message(sanitize_error(&text.chars().take(200).collect::<String>())),
    };
    emit(events, event).await
}

async fn apply_subscriptions(
    ws: &mut WsStream,
    current: &mut HashMap<u32, Mode>,
    desired: &HashMap<u32, Mode>,
) -> Result<()> {
    let unsubscribe: Vec<u32> = current
        .keys()
        .filter(|token| !desired.contains_key(token))
        .copied()
        .collect();
    if !unsubscribe.is_empty() {
        send_json(
            ws,
            serde_json::json!({ "a": "unsubscribe", "v": unsubscribe }),
        )
        .await?;
    }

    let subscribe: Vec<u32> = desired
        .keys()
        .filter(|token| !current.contains_key(token))
        .copied()
        .collect();
    if !subscribe.is_empty() {
        send_json(ws, serde_json::json!({ "a": "subscribe", "v": subscribe })).await?;
    }

    let mut by_mode: HashMap<Mode, Vec<u32>> = HashMap::new();
    for (token, mode) in desired {
        if current.get(token) != Some(mode) {
            by_mode.entry(*mode).or_default().push(*token);
        }
    }
    for (mode, tokens) in by_mode {
        send_json(
            ws,
            serde_json::json!({ "a": "mode", "v": [mode.to_string(), tokens] }),
        )
        .await?;
    }

    current.clone_from(desired);
    Ok(())
}

async fn send_json(ws: &mut WsStream, value: serde_json::Value) -> Result<()> {
    ws.send(Message::Text(value.to_string().into()))
        .await
        .map_err(|error| crate::Error::WebSocket(Box::new(error)))
}

#[cfg(test)]
mod tests {
    use super::{
        AttemptError, ConnectError, ConnectStage, backoff, connect_address, connect_candidates,
        connection_target, deduplicate_addresses, fatal_reason, ticker_url, websocket_request,
    };
    use futures_util::FutureExt;
    use reqwest::header::{HeaderMap, HeaderValue};
    use std::cell::{Cell, RefCell};
    use std::net::SocketAddr;
    use std::rc::Rc;
    use std::time::Duration;
    use tokio::net::TcpListener;
    use tokio::time::{Instant, sleep_until};
    use tokio_tungstenite::tungstenite::Error as WsError;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    fn address(value: &str) -> SocketAddr {
        value.parse().expect("valid test address")
    }

    fn http_response(status: u16, body: &[u8]) -> Box<super::Response> {
        Box::new(
            tokio_tungstenite::tungstenite::http::Response::builder()
                .status(status)
                .body(Some(body.to_vec()))
                .expect("valid test response"),
        )
    }

    #[test]
    fn ticker_url_encodes_token_query() {
        assert_eq!(
            ticker_url(
                url::Url::parse(super::super::DEFAULT_TICKER_URL).unwrap(),
                "k",
                "t+"
            ),
            "wss://ws.kite.trade/?api_key=k&access_token=t%2B"
        );
    }

    #[test]
    fn connection_target_preserves_websocket_uri_semantics() {
        let secure = "wss://example.com/socket"
            .into_client_request()
            .expect("valid request");
        assert_eq!(
            connection_target(&secure).expect("supported target"),
            ("example.com".to_string(), 443)
        );

        let plain = "ws://example.com:8080/socket"
            .into_client_request()
            .expect("valid request");
        assert_eq!(
            connection_target(&plain).expect("supported target"),
            ("example.com".to_string(), 8080)
        );

        let ipv6 = "wss://[::1]/socket"
            .into_client_request()
            .expect("valid request");
        assert_eq!(
            connection_target(&ipv6).expect("supported target"),
            ("::1".to_string(), 443)
        );

        let unsupported = "https://example.com:8443/socket"
            .into_client_request()
            .expect("syntactically valid request");
        assert!(connection_target(&unsupported).is_err());
    }

    #[test]
    fn address_deduplication_keeps_resolver_order() {
        let first = address("[2001:db8::1]:443");
        let second = address("192.0.2.1:443");
        let third = address("[2001:db8::2]:443");
        assert_eq!(
            deduplicate_addresses([first, second, first, third, second]),
            vec![first, second, third]
        );
    }

    #[test]
    fn each_connection_request_gets_a_fresh_websocket_key() {
        let first = websocket_request(
            "wss://example.com/socket",
            &crate::http::user_agent(None),
            &HeaderMap::new(),
        )
        .expect("valid request");
        let second = websocket_request(
            "wss://example.com/socket",
            &crate::http::user_agent(None),
            &HeaderMap::new(),
        )
        .expect("valid request");

        assert!(!first.headers().contains_key("Origin"));
        assert_ne!(
            first.headers()["Sec-WebSocket-Key"],
            second.headers()["Sec-WebSocket-Key"],
            "RFC 6455 requires a fresh nonce for every handshake"
        );
    }

    #[test]
    fn extra_headers_cannot_replace_handshake_headers() {
        let mut headers = HeaderMap::new();
        for name in [
            "Sec-WebSocket-Key",
            "Host",
            "Upgrade",
            "Connection",
            "Sec-WebSocket-Version",
        ] {
            headers.insert(name, HeaderValue::from_static("EXTRA_SENTINEL"));
        }
        let request = websocket_request("wss://example.invalid/socket", "test-agent", &headers)
            .expect("valid request");
        assert_ne!(request.headers()["Sec-WebSocket-Key"], "EXTRA_SENTINEL");
        assert_eq!(request.headers()["Host"], "example.invalid");
        assert_eq!(request.headers()["Upgrade"], "websocket");
        assert_eq!(request.headers()["Connection"], "Upgrade");
        assert_eq!(request.headers()["Sec-WebSocket-Version"], "13");
    }

    #[test]
    fn the_handshake_sends_the_app_name_then_the_sdk_version() {
        let request = websocket_request(
            "wss://example.com/socket",
            &crate::http::user_agent(Some("kite-cli/1.2.3")),
            &HeaderMap::new(),
        )
        .expect("valid request");
        assert_eq!(
            request.headers()["User-Agent"],
            format!(
                "kite-cli/1.2.3 zerodha-kiteconnect/{}",
                env!("CARGO_PKG_VERSION")
            )
        );
    }

    #[tokio::test]
    async fn per_attempt_timeout_falls_through_sequentially() {
        let stalled = address("[2001:db8::1]:443");
        let healthy = address("192.0.2.1:443");
        let calls = Rc::new(RefCell::new(Vec::new()));
        let observed_calls = calls.clone();

        let connected = connect_candidates(
            vec![stalled, healthy],
            Instant::now() + Duration::from_millis(100),
            Duration::from_millis(100),
            Duration::from_millis(20),
            move |candidate, attempt_deadline| {
                observed_calls.borrow_mut().push(candidate);
                async move {
                    if candidate == stalled {
                        sleep_until(attempt_deadline).await;
                        Err(AttemptError::Timeout {
                            stage: ConnectStage::TlsOrUpgrade,
                        })
                    } else {
                        Ok("connected")
                    }
                }
                .boxed()
            },
        )
        .await
        .expect("the second address must get its turn");

        assert_eq!(connected.address, healthy);
        assert_eq!(connected.resolved, vec![stalled, healthy]);
        assert_eq!(&*calls.borrow(), &[stalled, healthy]);
    }

    #[tokio::test]
    async fn connect_address_reports_the_stage_of_a_cancelled_attempt() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback listener");
        let peer = listener.local_addr().expect("listener address");
        let server = tokio::spawn(async move {
            let (_stream, _) = listener.accept().await.expect("accept client");
            std::future::pending::<()>().await;
        });
        let request = websocket_request(
            "wss://example.com/socket",
            &crate::http::user_agent(None),
            &HeaderMap::new(),
        )
        .expect("valid request");

        let error = connect_address(request, peer, Instant::now() + Duration::from_millis(50))
            .await
            .expect_err("a peer that never answers TLS must time out");
        server.abort();

        assert!(matches!(
            error,
            AttemptError::Timeout {
                stage: ConnectStage::TlsOrUpgrade
            }
        ));
    }

    #[tokio::test]
    async fn full_path_transport_failure_falls_through() {
        let broken_edge = address("192.0.2.1:443");
        let healthy_edge = address("192.0.2.2:443");
        let calls = Rc::new(RefCell::new(Vec::new()));
        let observed_calls = calls.clone();

        let connected = connect_candidates(
            vec![broken_edge, healthy_edge],
            Instant::now() + Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_millis(100),
            move |candidate, _attempt_deadline| {
                observed_calls.borrow_mut().push(candidate);
                async move {
                    if candidate == broken_edge {
                        Err(AttemptError::Transport {
                            stage: ConnectStage::Tls,
                            error: WsError::Io(std::io::Error::other(
                                "TLS peer stopped responding",
                            )),
                        })
                    } else {
                        Ok("connected")
                    }
                }
            },
        )
        .await
        .expect("a TLS failure must advance to the next address");

        assert_eq!(connected.address, healthy_edge);
        assert_eq!(&*calls.borrow(), &[broken_edge, healthy_edge]);
    }

    #[tokio::test]
    async fn http_response_is_authoritative() {
        let first = address("192.0.2.1:443");
        let second = address("192.0.2.2:443");
        let calls = Rc::new(Cell::new(0));
        let observed_calls = calls.clone();

        let result = connect_candidates(
            vec![first, second],
            Instant::now() + Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_millis(100),
            move |_candidate, _attempt_deadline| {
                observed_calls.set(observed_calls.get() + 1);
                std::future::ready(Err::<(), _>(AttemptError::Http(http_response(
                    403,
                    b"Cloudflare policy",
                ))))
            },
        )
        .await;

        let error = result.expect_err("HTTP response must stop candidate iteration");
        assert!(matches!(&error, ConnectError::Http { .. }));
        assert!(error.to_string().contains("Cloudflare policy"));
        assert_eq!(calls.get(), 1, "the second address must not be attempted");
    }

    #[tokio::test]
    async fn transient_http_response_tries_the_next_address() {
        let first = address("192.0.2.1:443");
        let second = address("192.0.2.2:443");
        let calls = Rc::new(RefCell::new(Vec::new()));
        let observed_calls = calls.clone();

        let connected = connect_candidates(
            vec![first, second],
            Instant::now() + Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_millis(100),
            move |candidate, _attempt_deadline| {
                observed_calls.borrow_mut().push(candidate);
                std::future::ready(if candidate == first {
                    Err(AttemptError::Http(http_response(503, b"edge unavailable")))
                } else {
                    Ok("connected")
                })
            },
        )
        .await
        .expect("a transient edge response must advance to the next address");

        assert_eq!(connected.address, second);
        assert_eq!(&*calls.borrow(), &[first, second]);
    }

    #[tokio::test]
    async fn rate_limit_does_not_multiply_requests_across_addresses() {
        let first = address("192.0.2.1:443");
        let second = address("192.0.2.2:443");
        let calls = Rc::new(Cell::new(0));
        let observed_calls = calls.clone();

        let error = connect_candidates(
            vec![first, second],
            Instant::now() + Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_millis(100),
            move |_candidate, _attempt_deadline| {
                observed_calls.set(observed_calls.get() + 1);
                std::future::ready(Err::<(), _>(AttemptError::Http(http_response(
                    429,
                    b"rate limited",
                ))))
            },
        )
        .await
        .expect_err("rate limiting must remain retryable without address fan-out");

        assert!(matches!(error, ConnectError::Http { .. }));
        assert_eq!(calls.get(), 1);
    }

    #[tokio::test]
    async fn aggregate_error_keeps_address_order_and_stages() {
        let ipv6 = address("[2001:db8::1]:443");
        let ipv4 = address("192.0.2.1:443");

        let error = connect_candidates(
            vec![ipv6, ipv4],
            Instant::now() + Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_millis(100),
            move |candidate, _attempt_deadline| {
                std::future::ready(Err::<(), _>(AttemptError::Transport {
                    stage: if candidate.is_ipv6() {
                        ConnectStage::Tcp
                    } else {
                        ConnectStage::Upgrade
                    },
                    error: WsError::Io(std::io::Error::new(
                        std::io::ErrorKind::ConnectionRefused,
                        if candidate.is_ipv6() {
                            "network unreachable"
                        } else {
                            "connection refused"
                        },
                    )),
                }))
            },
        )
        .await
        .expect_err("both candidates fail");
        let message = error.to_string();

        assert!(message.contains("all 2 addresses failed"));
        assert!(message.contains("resolved order [[2001:db8::1]:443, 192.0.2.1:443]"));
        assert!(message.contains("[2001:db8::1]:443 tcp: network unreachable"));
        assert!(message.contains("192.0.2.1:443 upgrade: connection refused"));
    }

    #[tokio::test]
    async fn operation_deadline_names_stage_and_address() {
        let peer = address("192.0.2.1:443");
        let deadline = Instant::now() + Duration::from_millis(50);
        let error = connect_candidates(
            vec![peer],
            deadline,
            Duration::from_millis(50),
            Duration::from_millis(5),
            move |_candidate, attempt_deadline| {
                assert_eq!(
                    attempt_deadline, deadline,
                    "the final candidate gets all remaining time"
                );
                std::future::ready(Err::<(), _>(AttemptError::Timeout {
                    stage: ConnectStage::TlsOrUpgrade,
                }))
            },
        )
        .await
        .expect_err("the shared deadline must stop the attempt");
        let message = error.to_string();

        assert!(message.contains("connect deadline exceeded after 50ms"));
        assert!(message.contains("at tls/upgrade for 192.0.2.1:443"));
        assert!(message.contains("resolved order [192.0.2.1:443]"));
    }

    #[test]
    fn http_rejections_have_actionable_fatality() {
        for (status, body) in [
            (
                400,
                br#"{"status":"error","message":"Invalid `access_token` or `api_key`."}"#
                    .as_slice(),
            ),
            (
                403,
                br#"{"status":"error","message":"Authentication failed."}"#.as_slice(),
            ),
        ] {
            let invalid_token = ConnectError::Http {
                response: http_response(status, body),
                message: String::new(),
            };
            assert!(
                fatal_reason(&invalid_token)
                    .expect("a known Zerodha invalid-token response is fatal")
                    .to_string()
                    .contains("auth rejected")
            );
        }

        let unsupported_auth = ConnectError::Http {
            response: http_response(
                400,
                br#"{"status":"error","message":"Invalid auth token."}"#,
            ),
            message: String::new(),
        };
        let reason = fatal_reason(&unsupported_auth).expect("unsupported auth is fatal");
        assert!(matches!(reason, super::CloseReason::HandshakeRejected(_)));
        assert!(reason.to_string().contains("Invalid auth token."));
        assert!(reason.to_string().contains("authentication settings"));

        let unstructured_body = ConnectError::Http {
            response: http_response(400, b"Invalid auth token."),
            message: String::new(),
        };
        assert!(
            !fatal_reason(&unstructured_body)
                .expect("HTTP 400 remains fatal")
                .to_string()
                .contains("auth rejected"),
            "text outside Kite's JSON error envelope must not be classified as auth"
        );

        for (status, body) in [
            (400, b"malformed handshake".as_slice()),
            (403, b"Cloudflare policy".as_slice()),
            (401, b"proxy authentication required".as_slice()),
            (403, b"".as_slice()),
            (403, br#"{"status":"error","message":"Invalid"#.as_slice()),
        ] {
            let edge_rejection = ConnectError::Http {
                response: http_response(status, body),
                message: String::new(),
            };
            let reason = fatal_reason(&edge_rejection)
                .expect("a permanent 4xx must stop the manager rather than retry forever");
            assert!(
                !reason.to_string().contains("auth rejected"),
                "must not send the user to re-login for an edge rejection: {reason}"
            );
            assert!(reason.to_string().contains("proxy"), "unhelpful: {reason}");
        }

        for status in [404u16, 408, 409, 421, 425, 426, 429, 500, 502, 503] {
            let outage = ConnectError::Http {
                response: http_response(status, b"upstream unavailable"),
                message: String::new(),
            };
            assert!(
                fatal_reason(&outage).is_none(),
                "HTTP {status} must stay retryable"
            );
        }
    }

    #[test]
    fn backoff_grows_then_caps() {
        assert_eq!(backoff(1), Duration::from_secs(1));
        assert_eq!(backoff(2), Duration::from_secs(2));
        assert_eq!(backoff(3), Duration::from_secs(4));
        assert_eq!(backoff(4), Duration::from_secs(8));
        assert_eq!(backoff(5), Duration::from_secs(16));
        assert_eq!(backoff(6), Duration::from_secs(30));
        assert_eq!(backoff(100), Duration::from_secs(30));
        assert_eq!(backoff(0), Duration::from_secs(1));
    }
    #[tokio::test]
    async fn order_update_waits_for_capacity_instead_of_dropping() {
        let (events, mut receiver) = tokio::sync::mpsc::channel(1);
        events.try_send(super::TickerEvent::Connected).unwrap();
        let postback: serde_json::Value =
            serde_json::from_str(include_str!("../../../../mock_responses/postback.json")).unwrap();
        let text = serde_json::json!({"type": "order", "data": postback}).to_string();
        let sending = tokio::spawn(async move { super::handle_text(&text, &events).await });
        tokio::task::yield_now().await;
        assert!(!sending.is_finished());
        assert!(matches!(
            receiver.recv().await,
            Some(super::TickerEvent::Connected)
        ));
        match receiver.recv().await.unwrap() {
            super::TickerEvent::OrderUpdate(order) => assert_eq!(order.order_id, "220303000308932"),
            other => panic!("order update was lost: {other:?}"),
        }
        sending.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn malformed_order_update_is_reported_with_its_frame() {
        let (events, mut receiver) = tokio::sync::mpsc::channel(1);
        for (text, expected) in [
            (
                r#"{"type":"order","data":{},"access_token":"SENTINEL-91c2"}"#,
                "missing field `placed_by`",
            ),
            (r#"{"type":"order"}"#, "invalid type: null"),
        ] {
            super::handle_text(text, &events).await.unwrap();
            match receiver.recv().await.unwrap() {
                super::TickerEvent::InvalidOrderUpdate { error, frame } => {
                    assert!(error.starts_with(expected), "{error}");
                    assert_eq!(frame, super::sanitize_error(text));
                    assert!(!frame.contains("SENTINEL-91c2"), "{frame}");
                }
                other => panic!("expected an invalid order update: {other:?}"),
            }
        }
    }
}
