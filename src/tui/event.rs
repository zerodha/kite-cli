use crate::app::runtime::Runtime;
use crate::tui::data::DataEvent;
use crossterm::event::{
    Event, EventStream, KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind,
};
use futures_util::StreamExt;
use kiteconnect::ticker::{TickerEvent, TickerEvents};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

#[derive(Debug)]
pub enum AppEvent {
    Key(KeyEvent),
    Paste(String),
    Shutdown,
    Mouse(MouseEvent),
    Resize,
    Ws(TickerEvent),
    Data(DataEvent),
    Timer,
    Session(SessionChange),
}

/// How the saved session changed under a running TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionChange {
    /// `kite auth logout` cleared it.
    SignedOut,
    /// A new login replaced it.
    Replaced,
}

/// Reports the first change to the saved access token the TUI started with,
/// checking every `every`.
pub fn spawn_session_watch(
    rt: Runtime,
    token: String,
    every: Duration,
    tx: mpsc::Sender<AppEvent>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(every);
        loop {
            interval.tick().await;
            let change = match rt.saved_access_token() {
                Ok(Some(saved)) if saved == token => continue,
                Ok(Some(_)) => SessionChange::Replaced,
                Ok(None) => SessionChange::SignedOut,
                Err(_) => continue,
            };
            let _ = tx.send(AppEvent::Session(change)).await;
            return;
        }
    })
}

pub fn spawn_terminal_events(tx: mpsc::Sender<AppEvent>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut stream = EventStream::new();
        while let Some(Ok(event)) = stream.next().await {
            let app_event = match event {
                Event::Key(key) if key.kind != KeyEventKind::Release => AppEvent::Key(key),
                Event::Mouse(mouse)
                    if matches!(
                        mouse.kind,
                        MouseEventKind::Down(MouseButton::Left)
                            | MouseEventKind::Up(MouseButton::Left)
                            | MouseEventKind::ScrollUp
                            | MouseEventKind::ScrollDown
                            | MouseEventKind::Moved
                            | MouseEventKind::Drag(MouseButton::Left)
                    ) =>
                {
                    AppEvent::Mouse(mouse)
                }
                Event::Resize(_, _) => AppEvent::Resize,
                Event::Paste(text) => AppEvent::Paste(text),
                _ => continue,
            };
            if tx.send(app_event).await.is_err() {
                return;
            }
        }
    })
}

pub fn normalize_key(mut key: KeyEvent) -> KeyEvent {
    if key.code == crossterm::event::KeyCode::Char('h')
        && key.modifiers == crossterm::event::KeyModifiers::CONTROL
    {
        key.code = crossterm::event::KeyCode::Backspace;
        key.modifiers = crossterm::event::KeyModifiers::NONE;
    }
    key
}

pub fn is_text_key(key: KeyEvent) -> bool {
    use crossterm::event::{KeyCode, KeyModifiers};
    matches!(key.code, KeyCode::Char(character) if !character.is_control())
        && (!key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
            || key
                .modifiers
                .contains(KeyModifiers::CONTROL | KeyModifiers::ALT))
}

pub fn spawn_shutdown_events(tx: mpsc::Sender<AppEvent>) -> std::io::Result<JoinHandle<()>> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut terminate = signal(SignalKind::terminate())?;
        let mut hangup = signal(SignalKind::hangup())?;
        let mut interrupt = signal(SignalKind::interrupt())?;
        Ok(tokio::spawn(async move {
            tokio::select! { _ = terminate.recv() => {}, _ = hangup.recv() => {}, _ = interrupt.recv() => {} }
            let _ = tx.send(AppEvent::Shutdown).await;
        }))
    }
    #[cfg(windows)]
    {
        let mut close = tokio::signal::windows::ctrl_close()?;
        let mut logoff = tokio::signal::windows::ctrl_logoff()?;
        let mut shutdown = tokio::signal::windows::ctrl_shutdown()?;
        let mut interrupt = tokio::signal::windows::ctrl_c()?;
        Ok(tokio::spawn(async move {
            tokio::select! { _ = close.recv() => {}, _ = logoff.recv() => {}, _ = shutdown.recv() => {}, _ = interrupt.recv() => {} }
            let _ = tx.send(AppEvent::Shutdown).await;
        }))
    }
}

pub fn spawn_timer(tx: mpsc::Sender<AppEvent>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(250));
        loop {
            interval.tick().await;
            if tx.send(AppEvent::Timer).await.is_err() {
                return;
            }
        }
    })
}

pub fn forward_ws_events(mut rx: TickerEvents, tx: mpsc::Sender<AppEvent>) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            if tx.send(AppEvent::Ws(event)).await.is_err() {
                return;
            }
        }
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsState {
    Connecting,
    Connected,
    Reconnecting,
    Closed,
}

#[cfg(test)]
mod tests {
    use super::{AppEvent, SessionChange, spawn_session_watch};
    use crate::app::runtime::Runtime;
    use crate::cli::Cli;
    use clap::Parser;
    use std::time::Duration;

    #[tokio::test]
    async fn the_session_watch_reports_a_cleared_or_replaced_token() {
        for (saved_after, expected) in [
            (None, SessionChange::SignedOut),
            (Some("new-token"), SessionChange::Replaced),
        ] {
            let dir = std::env::temp_dir().join(format!(
                "kite-session-watch-{}-{expected:?}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join("config.json");
            let write = |token: Option<&str>| {
                std::fs::write(
                    &path,
                    serde_json::json!({ "access_token": token }).to_string(),
                )
                .unwrap();
            };
            write(Some("old-token"));
            let rt = Runtime::from_cli(
                &Cli::try_parse_from(["kite", "--config", path.to_str().unwrap()]).unwrap(),
            );
            let (tx, mut rx) = tokio::sync::mpsc::channel(1);
            let watch = spawn_session_watch(rt, "old-token".into(), Duration::from_millis(10), tx);

            tokio::time::sleep(Duration::from_millis(50)).await;
            assert!(
                rx.try_recv().is_err(),
                "an unchanged session is not reported"
            );
            write(saved_after);
            let event = rx.recv().await.expect("the change is reported");
            assert!(
                matches!(event, AppEvent::Session(change) if change == expected),
                "{event:?}"
            );
            watch.await.unwrap();
            std::fs::remove_dir_all(&dir).ok();
        }
    }
}
