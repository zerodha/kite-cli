use kiteconnect::Request;
pub(crate) mod alerts;
mod app;
mod bids;
mod candles;
pub(crate) mod data;
mod event;
mod form;
pub(crate) mod format;
mod indicators;
mod instruments;
mod login;
mod marketwatch;
mod notes;
mod palette;
mod pane_state;
mod settings;
mod split;
mod subscriptions;
mod ticket;
mod ticks;
mod ui;
mod workspace;

use crate::app::runtime::Runtime;
use crate::cli::TuiArgs;
use crate::tui::app::App;
use crate::tui::event::AppEvent;
use crate::tui::login::LoginReason;
use crate::tui::marketwatch::{WatchId, WatchItem};
use crate::tui::notes::Pad;
use crate::util::tui_tokens;
use anyhow::{Context, Result};
use crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use kiteconnect::KiteConnect;
use ratatui::Terminal;
use std::io::{self, IsTerminal};
use tokio::sync::mpsc;

enum SessionExit {
    Quit,
    Relogin,
    Logout,
}

/// # Errors
/// Returns an error if login, client setup, terminal setup, or the session fails.
pub async fn run(rt: &Runtime, args: TuiArgs) -> Result<()> {
    if let Some(reason) = startup_login_reason(rt).await {
        login::run_login(rt, reason).await?;
    }

    loop {
        let exit = run_session(rt, &args).await?;
        match exit {
            SessionExit::Quit => return Ok(()),
            SessionExit::Relogin => {
                login::run_login(rt, LoginReason::Expired).await?;
            }
            SessionExit::Logout => {
                rt.clear_session()?;
                return Ok(());
            }
        }
    }
}

async fn run_session(rt: &Runtime, args: &TuiArgs) -> Result<SessionExit> {
    let client = rt.client()?;

    let settings = settings::load();
    settings::set_theme(settings.theme());

    let tokens = tui_tokens(args.clone());
    let (watch, ephemeral) = if tokens.is_empty() {
        let user_id = rt.resolve_credentials()?.user_id.unwrap_or_default();
        let active = settings.selected_watch(&user_id).map(WatchId::from);
        (
            marketwatch::State::remote(Some(client.clone()), user_id, active),
            false,
        )
    } else {
        let rows = tokens.into_iter().map(WatchItem::from_token).collect();
        (marketwatch::State::ephemeral(rows), true)
    };

    let notes = if ephemeral {
        Pad::default()
    } else {
        notes::load()
    };

    let (events_tx, mut events_rx) = mpsc::channel::<AppEvent>(1024);

    let mut app = App::new(
        client.clone(),
        events_tx.clone(),
        watch,
        notes,
        ephemeral,
        &settings,
    );

    app.uuid = rt.app_uuid()?;
    let ticker_url =
        url::Url::parse(rt.resolve_credentials()?.ticker_url()?).context("invalid ticker URL")?;
    let tasks = spawn_session_tasks(client, ticker_url, events_tx, &mut app, &settings)?;

    let mut terminal = match setup_terminal() {
        Ok(terminal) => terminal,
        Err(error) => {
            for task in tasks {
                task.abort();
            }
            return Err(error);
        }
    };
    let result = event_loop(&mut terminal, &mut app, &mut events_rx).await;

    for task in tasks {
        task.abort();
    }

    restore_terminal(&mut terminal);

    if let Some(notice) = app.take_exit_notice() {
        eprintln!("kite: {notice}");
    }

    result
}

fn spawn_session_tasks(
    client: KiteConnect,
    ticker_url: url::Url,
    events_tx: mpsc::Sender<AppEvent>,
    app: &mut App,
    settings: &settings::Settings,
) -> Result<Vec<tokio::task::JoinHandle<()>>> {
    let movers_task = app.ensure_movers();

    let marketwatch_tasks = app.start_marketwatch();

    let (ticker, ticker_events) = client.ticker().url(ticker_url).spawn()?;
    ticker.set_subscriptions(
        app.desired_subscriptions()
            .into_iter()
            .map(|(token, mode)| (token, mode.into()))
            .collect(),
    )?;
    app.ticker = Some(ticker);
    let mut tasks: Vec<tokio::task::JoinHandle<()>> = vec![
        event::forward_ws_events(ticker_events, events_tx.clone()),
        event::spawn_terminal_events(events_tx.clone()),
        event::spawn_timer(events_tx.clone()),
        event::spawn_shutdown_events(events_tx.clone())?,
    ];
    tasks.extend(movers_task);
    tasks.extend(marketwatch_tasks);
    tasks.extend(data::spawn_pollers(
        client.clone(),
        events_tx.clone(),
        settings.poll_intervals(),
        app.alert_fetch_requests.clone(),
    ));
    for section in bids::Section::ALL {
        if section != bids::Section::Auctions {
            tasks.push(bids::spawn_refresh_instruments(
                client.clone(),
                events_tx.clone(),
                section,
            ));
        }
    }
    tasks.push(bids::spawn_refresh_applications(
        client.clone(),
        events_tx.clone(),
    ));
    tasks.push(data::spawn_refresh_profile(client, events_tx.clone()));
    tasks.push(data::spawn_load_instruments(app.uuid.clone(), events_tx));

    Ok(tasks)
}

async fn startup_login_reason(rt: &Runtime) -> Option<LoginReason> {
    if !matches!(rt.resolve_credentials(), Ok(credentials) if credentials.access_token.is_some()) {
        return Some(LoginReason::FirstRun);
    }
    if !io::stdin().is_terminal() {
        return None;
    }
    match rt.client() {
        Ok(client) => is_auth_expired(&client)
            .await
            .then_some(LoginReason::Expired),
        Err(_) => Some(LoginReason::FirstRun),
    }
}

async fn is_auth_expired(client: &KiteConnect) -> bool {
    matches!(client.call(Request::get("/user/profile/full")).raw().await,
        Err(err) if is_auth_error(&err))
}

fn is_auth_error(err: &kiteconnect::Error) -> bool {
    err.is_session_expired()
}

async fn event_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    events: &mut mpsc::Receiver<AppEvent>,
) -> Result<SessionExit>
where
    B::Error: Send + Sync + 'static,
{
    let exit = drive(terminal, app, events).await;
    app.commit_open_editor();
    exit
}

async fn drive<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    events: &mut mpsc::Receiver<AppEvent>,
) -> Result<SessionExit>
where
    B::Error: Send + Sync + 'static,
{
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;

        let Some(event) = events.recv().await else {
            return Ok(SessionExit::Quit);
        };
        app.on_event(event);
        while let Ok(event) = events.try_recv() {
            app.on_event(event);
        }

        if app.should_quit {
            return Ok(SessionExit::Quit);
        }
        if app.should_logout {
            return Ok(SessionExit::Logout);
        }
        if app.ws_fatal.as_deref() == Some(app::SESSION_EXPIRED) {
            return Ok(SessionExit::Relogin);
        }
    }
}

pub(crate) fn setup_terminal() -> Result<Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>> {
    static PANIC_HOOK: std::sync::Once = std::sync::Once::new();
    enable_raw_mode().context("failed to enable raw terminal mode")?;
    let mut stdout = io::stdout();
    if let Err(error) = execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste
    ) {
        restore_terminal_modes();
        return Err(error.into());
    }

    PANIC_HOOK.call_once(|| {
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore_terminal_modes();
            original_hook(info);
        }));
    });

    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    match Terminal::new(backend) {
        Ok(terminal) => Ok(terminal),
        Err(error) => {
            restore_terminal_modes();
            Err(error.into())
        }
    }
}

fn restore_terminal_modes() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), DisableMouseCapture);
    let _ = execute!(io::stdout(), DisableBracketedPaste);
    let _ = execute!(io::stdout(), LeaveAlternateScreen);
}

pub(crate) fn restore_terminal(
    terminal: &mut Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>,
) {
    restore_terminal_modes();
    let _ = terminal.show_cursor();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::{Modal, Side};
    use crate::tui::notes;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use kiteconnect::{CloseReason, TickerEvent};
    use ratatui::backend::TestBackend;
    use std::path::{Path, PathBuf};

    fn session(
        name: &str,
    ) -> (
        App,
        Terminal<TestBackend>,
        mpsc::Sender<AppEvent>,
        mpsc::Receiver<AppEvent>,
        PathBuf,
    ) {
        let dir = std::env::temp_dir().join(format!("kite-loop-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let path = dir.join("notes.json");

        let client = kiteconnect::KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("tok").unwrap())
            .build()
            .expect("dummy client builds");
        let (events_tx, events_rx) = mpsc::channel(64);
        let mut app = App::new(
            client,
            events_tx.clone(),
            marketwatch::State::remote(None, String::new(), None),
            Pad::default(),
            false,
            &crate::tui::settings::Settings::default(),
        );
        app.notes_path = Some(path.clone());
        let terminal = Terminal::new(TestBackend::new(100, 30)).expect("test terminal");
        (app, terminal, events_tx, events_rx, path)
    }

    fn key(events_tx: &mpsc::Sender<AppEvent>, code: KeyCode, mods: KeyModifiers) {
        events_tx
            .try_send(AppEvent::Key(KeyEvent::new(code, mods)))
            .expect("channel has room");
    }

    fn type_into_a_new_note(events_tx: &mpsc::Sender<AppEvent>, text: &str) {
        key(events_tx, KeyCode::Char('n'), KeyModifiers::NONE); // open the pad
        key(events_tx, KeyCode::Char('n'), KeyModifiers::NONE); // start a draft
        for ch in text.chars() {
            key(events_tx, KeyCode::Char(ch), KeyModifiers::NONE);
        }
    }

    fn saved_bodies(path: &Path) -> Vec<String> {
        notes::load_from(path)
            .newest_first()
            .iter()
            .map(|note| note.body.clone())
            .collect()
    }

    #[tokio::test]
    async fn ctrl_c_saves_an_open_note_editor() {
        let (mut app, mut terminal, events_tx, mut events_rx, path) = session("ctrlc");
        type_into_a_new_note(&events_tx, "1520 is the level");
        key(&events_tx, KeyCode::Char('c'), KeyModifiers::CONTROL);

        let exit = event_loop(&mut terminal, &mut app, &mut events_rx)
            .await
            .expect("loop ends cleanly");
        assert!(matches!(exit, SessionExit::Quit));
        assert_eq!(
            saved_bodies(&path),
            vec!["1520 is the level".to_string()],
            "Ctrl-C threw away the open editor's text"
        );
    }

    #[tokio::test]
    async fn q_flushes_a_pad_with_unsaved_changes() {
        let (mut app, mut terminal, events_tx, mut events_rx, path) = session("quit-key");
        app.notes.upsert(None, "unflushed thought", None, 1_000);
        key(&events_tx, KeyCode::Char('q'), KeyModifiers::NONE);

        let exit = event_loop(&mut terminal, &mut app, &mut events_rx)
            .await
            .expect("loop ends cleanly");
        assert!(matches!(exit, SessionExit::Quit));
        assert_eq!(saved_bodies(&path), vec!["unflushed thought".to_string()]);
    }

    #[tokio::test]
    async fn a_relogin_teardown_saves_an_open_note_editor() {
        let (mut app, mut terminal, events_tx, mut events_rx, path) = session("relogin");
        type_into_a_new_note(&events_tx, "thesis for the swing");
        app.ws_fatal = Some("access token expired".to_string());
        events_tx
            .try_send(AppEvent::Timer)
            .expect("channel has room");

        let exit = event_loop(&mut terminal, &mut app, &mut events_rx)
            .await
            .expect("loop ends cleanly");
        assert!(
            matches!(exit, SessionExit::Relogin),
            "an expired token must end the session for re-login"
        );
        assert_eq!(
            saved_bodies(&path),
            vec!["thesis for the swing".to_string()],
            "a relogin teardown threw away the open editor's text"
        );
    }

    #[tokio::test]
    async fn queued_enters_cannot_send_an_order_whose_review_was_never_drawn() {
        let (mut app, mut terminal, events_tx, mut events_rx, _) = session("confirm");
        app.open_ticket(Side::Buy, "NSE".into(), "INFY".into(), 1, 0, None);
        let Some(Modal::Ticket(ticket)) = app.modals.last_mut() else {
            panic!("a ticket opens");
        };
        ticket.qty = "10".to_string();
        ticket.price = "100".to_string();
        ticket.order_type = kiteconnect::OrderType::Limit;
        key(&events_tx, KeyCode::Enter, KeyModifiers::NONE);
        key(&events_tx, KeyCode::Enter, KeyModifiers::NONE);
        key(&events_tx, KeyCode::Char('c'), KeyModifiers::CONTROL);

        drive(&mut terminal, &mut app, &mut events_rx)
            .await
            .expect("loop ends cleanly");
        assert!(
            matches!(app.modals.last(), Some(Modal::Confirm(_))),
            "the review is still open"
        );
        assert!(!app.pending_action, "the order was sent before its review");

        app.should_quit = false;
        key(&events_tx, KeyCode::Enter, KeyModifiers::NONE);
        key(&events_tx, KeyCode::Char('c'), KeyModifiers::CONTROL);
        drive(&mut terminal, &mut app, &mut events_rx)
            .await
            .expect("loop ends cleanly");
        assert!(app.pending_action, "Enter on a drawn review sends it");
        assert!(app.modals.is_empty());
    }

    #[tokio::test]
    async fn an_unsupported_ticker_auth_method_keeps_the_rest_session_open() {
        let (mut app, mut terminal, events_tx, mut events_rx, path) = session("ws-method");
        events_tx
            .try_send(AppEvent::Ws(TickerEvent::Closed(
                CloseReason::HandshakeRejected(
                    "Invalid auth token; check ticker authentication settings".into(),
                ),
            )))
            .unwrap();
        key(&events_tx, KeyCode::Char('q'), KeyModifiers::NONE);
        let exit = event_loop(&mut terminal, &mut app, &mut events_rx)
            .await
            .unwrap();
        assert!(matches!(exit, SessionExit::Quit));
        assert_ne!(app.ws_fatal.as_deref(), Some(app::SESSION_EXPIRED));
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[tokio::test]
    async fn a_ticker_auth_rejection_ends_the_session_for_recovery() {
        let (mut app, mut terminal, events_tx, mut events_rx, path) = session("ws-auth");
        events_tx
            .try_send(AppEvent::Ws(TickerEvent::Closed(
                CloseReason::AuthRejected("WebSocket auth rejected (HTTP 403)".into()),
            )))
            .unwrap();
        let exit = event_loop(&mut terminal, &mut app, &mut events_rx)
            .await
            .unwrap();
        assert!(matches!(exit, SessionExit::Relogin));
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[tokio::test]
    async fn a_ticker_closed_for_good_stays_fatal_without_restarting() {
        for (name, reason) in [
            (
                "ws-refusal",
                CloseReason::HandshakeRejected(
                    "WebSocket handshake refused (HTTP 403); blocked by proxy".into(),
                ),
            ),
            ("ws-limit", CloseReason::SubscriptionLimit),
            ("ws-requested", CloseReason::Requested),
        ] {
            let expected = reason.to_string();
            let (mut app, mut terminal, events_tx, mut events_rx, path) = session(name);
            let sender = tokio::spawn(async move {
                events_tx
                    .send(AppEvent::Ws(TickerEvent::Closed(reason)))
                    .await
                    .unwrap();
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                key(&events_tx, KeyCode::Char('c'), KeyModifiers::CONTROL);
            });
            let exit = event_loop(&mut terminal, &mut app, &mut events_rx)
                .await
                .unwrap();
            assert!(matches!(exit, SessionExit::Quit), "{name}");
            assert_eq!(app.ws_fatal.as_deref(), Some(expected.as_str()), "{name}");
            assert_eq!(app.ws_state, crate::tui::event::WsState::Closed, "{name}");
            assert!(
                app.toasts().iter().any(|toast| toast.text == expected),
                "{name}"
            );
            sender.await.unwrap();
            std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
        }
    }

    #[tokio::test]
    async fn an_expired_fetch_ends_the_session_for_login() {
        use crate::tui::data::DataEvent;
        let (mut app, mut terminal, events_tx, mut events_rx, notes) = session("expired");
        events_tx
            .try_send(AppEvent::Data(DataEvent::Orders(Err(
                "orders: kite API error TokenException (HTTP 403): expired".into(),
            ))))
            .unwrap();
        let exit = event_loop(&mut terminal, &mut app, &mut events_rx)
            .await
            .unwrap();
        assert!(matches!(exit, SessionExit::Relogin));
        std::fs::remove_dir_all(notes.parent().unwrap()).unwrap();
    }

    fn api_error(error_type: &str, http_status: u16) -> kiteconnect::Error {
        kiteconnect::error::ApiError::new(error_type.into(), "boom", http_status, None).into()
    }

    #[test]
    fn token_exception_is_an_auth_error() {
        assert!(is_auth_error(&api_error("TokenException", 403)));
    }

    #[test]
    fn plain_403_is_not_an_auth_error() {
        assert!(!is_auth_error(&api_error("Forbidden", 403)));
    }

    #[test]
    fn non_auth_api_error_is_not_an_auth_error() {
        assert!(!is_auth_error(&api_error("InputException", 400)));
        assert!(!is_auth_error(&api_error("GeneralException", 500)));
    }

    #[test]
    fn non_api_error_is_not_an_auth_error() {
        assert!(!is_auth_error(&kiteconnect::Error::Transport(Box::new(
            reqwest::Client::new()
                .get("invalid URL")
                .build()
                .unwrap_err()
        ))));
    }
}
