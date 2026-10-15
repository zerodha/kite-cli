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
