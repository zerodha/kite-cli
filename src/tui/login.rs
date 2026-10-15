use crate::app::runtime::Runtime;
use crate::app_api::trusted::{TrustedAuth, TwofaChallenge};
use crate::tui::format;
use crate::tui::settings;
use crate::tui::ui;
use anyhow::{Context, Result, bail};
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures_util::StreamExt;
use kiteconnect::Error;
use ratatui::Frame;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use std::io::{self, IsTerminal};

const CARD_WIDTH: u16 = 56;
const CARD_HEIGHT: u16 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginReason {
    FirstRun,
    Expired,
}

impl LoginReason {
    fn subtitle(self) -> &'static str {
        match self {
            LoginReason::FirstRun => "Sign in to Kite",
            LoginReason::Expired => "Session expired, please sign in again.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum CredsField {
    #[default]
    UserId,
    Password,
}

#[derive(Debug, Default)]
struct CredsState {
    user_id: String,
    password: String,
    field: CredsField,
    error: Option<String>,
    busy: bool,
    remembered: bool,
}

impl CredsState {
    fn new(remembered_user_id: Option<String>) -> Self {
        let remembered = remembered_user_id
            .map(|id| id.trim().to_uppercase())
            .filter(|id| !id.is_empty());
        match remembered {
            Some(user_id) => Self {
                user_id,
                field: CredsField::Password,
                remembered: true,
                ..Default::default()
            },
            None => Self::default(),
        }
    }
}

#[derive(Debug)]
struct TotpState {
    challenge: TwofaChallenge,
    otp: String,
    error: Option<String>,
    busy: bool,
}

impl TotpState {
    fn new(challenge: TwofaChallenge) -> Self {
        Self {
            challenge,
            otp: String::new(),
            error: None,
            busy: false,
        }
    }

    fn label(&self) -> &str {
        if self.challenge.twofa_type.eq_ignore_ascii_case("totp") {
            "TOTP"
        } else {
            &self.challenge.twofa_type
        }
    }
}

enum Stage {
    Credentials(CredsState),
    Totp(TotpState),
}

pub async fn run_login(rt: &Runtime, reason: LoginReason) -> Result<()> {
    if !io::stdin().is_terminal() {
        bail!(
            "no Kite session configured and stdin is not a terminal; run `kite` or `kite auth login` interactively to log in"
        );
    }

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel(1);
    let shutdown_task = crate::tui::event::spawn_shutdown_events(shutdown_tx)?;
    let mut terminal = match super::setup_terminal() {
        Ok(terminal) => terminal,
        Err(error) => {
            shutdown_task.abort();
            return Err(error);
        }
    };
    let outcome = run_event_loop(rt, reason, &mut terminal, &mut shutdown_rx).await;
    shutdown_task.abort();
    super::restore_terminal(&mut terminal);
    outcome
}

async fn run_event_loop(
    rt: &Runtime,
    reason: LoginReason,
    terminal: &mut ratatui::Terminal<CrosstermBackend<io::Stdout>>,
    shutdown: &mut tokio::sync::mpsc::Receiver<crate::tui::event::AppEvent>,
) -> Result<()> {
    let mut term_events = EventStream::new();

    terminal.draw(draw_connecting)?;
    let credentials = rt.resolve_credentials()?;
    let auth = TrustedAuth::new(
        credentials.api_root()?,
        credentials.api_key()?,
        &rt.app_uuid()?,
    )?;

    let remembered_user_id = credentials.user_id.clone();
    let mut screen = Stage::Credentials(CredsState::new(remembered_user_id));

    loop {
        terminal.draw(|frame| draw(frame, &screen, reason))?;

        let next_event = tokio::select! {
            _ = shutdown.recv() => bail!("login cancelled"),
            event = term_events.next() => event,
        };
        let event = match next_event {
            Some(Ok(event)) => event,
            Some(Err(err)) => return Err(err).context("terminal event stream failed"),
            None => bail!("terminal event stream ended unexpectedly"),
        };
        if let Event::Paste(text) = &event {
            let text: String = text
                .split(['\r', '\n'])
                .next()
                .unwrap_or_default()
                .chars()
                .filter(|character| !character.is_control())
                .collect();
            match &mut screen {
                Stage::Credentials(state) => active_creds_field(state).push_str(&text),
                Stage::Totp(state) => state.otp.push_str(&text),
            }
            continue;
        }
        let Event::Key(key) = event else { continue };
        let key = crate::tui::event::normalize_key(key);
        if key.kind == KeyEventKind::Release {
            continue;
        }
        if is_abort_key(key) {
            bail!("login cancelled");
        }

        screen = match screen {
            Stage::Credentials(mut state) => match apply_creds_key(&mut state, key) {
                None => Stage::Credentials(state),
                Some((user_id, password)) => {
                    state.busy = true;
                    state.error = None;
                    terminal.draw(|frame| draw_credentials(frame, &state, reason))?;
                    match auth.login(&user_id, &password).await {
                        Ok(challenge) => {
                            let mut totp = TotpState::new(challenge);
                            request_sms_code(&auth, &mut totp).await;
                            Stage::Totp(totp)
                        }
                        Err(err) => {
                            state.busy = false;
                            state.error = Some(describe_error(&err));
                            Stage::Credentials(state)
                        }
                    }
                }
            },
            Stage::Totp(mut state)
                if key.code == KeyCode::Tab && state.challenge.twofa_types.len() > 1 =>
            {
                state.busy = true;
                terminal.draw(|frame| draw_totp(frame, &state, reason))?;
                select_next_method(&auth, &mut state).await;
                state.busy = false;
                Stage::Totp(state)
            }
            Stage::Totp(mut state) => match apply_totp_key(&mut state, key) {
                None => Stage::Totp(state),
                Some(otp) => {
                    state.busy = true;
                    state.error = None;
                    terminal.draw(|frame| draw_totp(frame, &state, reason))?;
                    match auth
                        .twofa(&state.challenge, &state.challenge.twofa_type, &otp)
                        .await
                    {
                        Ok(session) => {
                            rt.save_session(&session)?;
                            return Ok(());
                        }
                        Err(err) => {
                            state.busy = false;
                            state.error = Some(describe_error(&err));
                            Stage::Totp(state)
                        }
                    }
                }
            },
        };
    }
}

fn is_abort_key(key: KeyEvent) -> bool {
    key.code == KeyCode::Esc
        || (key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL)
}

fn apply_creds_key(state: &mut CredsState, key: KeyEvent) -> Option<(String, String)> {
    if key.code == KeyCode::Char('u') && key.modifiers == KeyModifiers::CONTROL {
        state.remembered = false;
        state.user_id.clear();
        state.password.clear();
        state.field = CredsField::UserId;
        state.error = None;
        return None;
    }
    match key.code {
        KeyCode::Tab | KeyCode::Down if !state.remembered => {
            state.field = CredsField::Password;
            None
        }
        KeyCode::BackTab | KeyCode::Up if !state.remembered => {
            state.field = CredsField::UserId;
            None
        }
        KeyCode::Backspace => {
            active_creds_field(state).pop();
            None
        }
        KeyCode::Char(c) if crate::tui::event::is_text_key(key) => {
            active_creds_field(state).push(c);
            None
        }
        KeyCode::Enter => match state.field {
            CredsField::UserId => {
                state.field = CredsField::Password;
                None
            }
            CredsField::Password => {
                let user_id = state.user_id.trim().to_uppercase();
                if user_id.is_empty() {
                    state.field = CredsField::UserId;
                    state.error = Some("user ID is required".to_string());
                    return None;
                }
                if state.password.is_empty() {
                    state.error = Some("password is required".to_string());
                    return None;
                }
                Some((user_id, state.password.clone()))
            }
        },
        _ => None,
    }
}

fn active_creds_field(state: &mut CredsState) -> &mut String {
    match state.field {
        CredsField::UserId => &mut state.user_id,
        CredsField::Password => &mut state.password,
    }
}

fn apply_totp_key(state: &mut TotpState, key: KeyEvent) -> Option<String> {
    match key.code {
        KeyCode::Backspace => {
            state.otp.pop();
            None
        }
        KeyCode::Char(c) if crate::tui::event::is_text_key(key) => {
            state.otp.push(c);
            None
        }
        KeyCode::Enter => {
            let otp = state.otp.trim().to_string();
            if otp.is_empty() {
                state.error = Some(format!("{} code is required", state.label()));
                return None;
            }
            Some(otp)
        }
        _ => None,
    }
}

async fn select_next_method(auth: &TrustedAuth, state: &mut TotpState) {
    let types = &state.challenge.twofa_types;
    let current = types
        .iter()
        .position(|kind| kind == &state.challenge.twofa_type)
        .unwrap_or(0);
    state.challenge.twofa_type = types[(current + 1) % types.len()].clone();
    state.otp.clear();
    state.error = None;
    request_sms_code(auth, state).await;
}

async fn request_sms_code(auth: &TrustedAuth, state: &mut TotpState) {
    if !state.challenge.twofa_type.eq_ignore_ascii_case("sms") {
        return;
    }
    if let Err(err) = auth.send_otp(&state.challenge).await {
        state.error = Some(describe_error(&err));
    }
}

fn describe_error(err: &anyhow::Error) -> String {
    use kiteconnect::error::sanitize_error;
    match err.downcast_ref::<Error>() {
        Some(Error::Api(error)) => sanitize_error(&error.message),
        _ => sanitize_error(&err.to_string()),
    }
}

fn mask(value: &str) -> String {
    "•".repeat(value.chars().count())
}

fn draw(frame: &mut Frame, screen: &Stage, reason: LoginReason) {
    fill_root_background(frame);
    match screen {
        Stage::Credentials(state) => draw_credentials(frame, state, reason),
        Stage::Totp(state) => draw_totp(frame, state, reason),
    }
}

fn draw_connecting(frame: &mut Frame) {
    fill_root_background(frame);
    let area = card(frame.area());
    let inner = card_block(frame, area, " Login to Kite ");
    frame.render_widget(
        Paragraph::new(Span::styled("connecting to Kite…", format::dim())),
        inner,
    );
}

fn draw_credentials(frame: &mut Frame, state: &CredsState, reason: LoginReason) {
    let area = card(frame.area());
    let inner = card_block(frame, area, " Login to Kite ");

    let mut lines = vec![
        Line::from(Span::styled(reason.subtitle(), format::dim())),
        Line::default(),
    ];

    let password_mask = mask(&state.password);
    let hint = if state.remembered {
        lines.push(Line::from(vec![
            Span::styled("User ID: ", format::dim()),
            Span::raw(state.user_id.clone()),
            Span::styled("  (Ctrl-U: change user)", format::dim()),
        ]));
        lines.push(Line::default());
        lines.push(field_line("Password", &password_mask, true));
        "Enter confirm · Ctrl-U change user · Esc cancel"
    } else {
        lines.push(field_line(
            "User ID",
            &state.user_id,
            state.field == CredsField::UserId,
        ));
        lines.push(Line::default());
        lines.push(field_line(
            "Password",
            &password_mask,
            state.field == CredsField::Password,
        ));
        "Enter next/confirm · Tab switch field · Esc cancel"
    };

    lines.push(Line::default());
    lines.push(status_line(
        state.busy,
        "authenticating…",
        state.error.as_deref(),
    ));
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(hint, format::dim())));

    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_totp(frame: &mut Frame, state: &TotpState, reason: LoginReason) {
    let area = card(frame.area());
    let inner = card_block(frame, area, " Login to Kite ");

    let greeting = match &state.challenge.user_name {
        Some(name) => format!("Signed in as {name} ({})", state.challenge.user_id),
        None => format!("Signed in as {}", state.challenge.user_id),
    };

    let methods = if state.challenge.twofa_types.len() > 1 {
        Line::from(Span::styled(
            format!("Methods: {}", state.challenge.twofa_types.join(", ")),
            format::dim(),
        ))
    } else {
        Line::default()
    };
    let mut lines = vec![
        Line::from(Span::styled(reason.subtitle(), format::dim())),
        Line::default(),
        Line::from(Span::styled(greeting, format::dim())),
        methods,
        field_line(state.label(), &state.otp, true),
        Line::default(),
    ];
    lines.push(status_line(
        state.busy,
        "verifying…",
        state.error.as_deref(),
    ));
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        if state.challenge.twofa_types.len() > 1 {
            "Tab method · Enter confirm · Esc cancel"
        } else {
            "Enter confirm · Esc cancel"
        },
        format::dim(),
    )));

    frame.render_widget(Paragraph::new(lines), inner);
}

fn field_line<'a>(label: &str, value: &'a str, active: bool) -> Line<'a> {
    let prefix = if active {
        Span::styled("❯ ", Style::default().fg(format::accent()))
    } else {
        Span::raw("  ")
    };
    let mut spans = vec![
        Span::styled(format!("{label:<9}"), format::dim()),
        prefix,
        Span::raw(value),
    ];
    if active {
        spans.push(Span::styled("▏", Style::default().fg(format::accent())));
    }
    Line::from(spans)
}

fn status_line<'a>(busy: bool, busy_text: &'a str, error: Option<&'a str>) -> Line<'a> {
    if busy {
        Line::from(Span::styled(busy_text, format::dim()))
    } else if let Some(message) = error {
        Line::from(Span::styled(message, Style::default().fg(format::error())))
    } else {
        Line::default()
    }
}

fn card(area: Rect) -> Rect {
    ui::centered(area, CARD_WIDTH, CARD_HEIGHT)
}

fn card_block(frame: &mut Frame, area: Rect, title: &str) -> Rect {
    let theme = settings::theme();
    let mut style = Style::default();
    if let Some(bg) = theme.surface {
        style = style.bg(bg);
    }
    if let Some(fg) = format::text() {
        style = style.fg(fg);
    }
    let block = Block::bordered()
        .title(title)
        .style(style)
        .border_style(format::border_style());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    inner
}

fn fill_root_background(frame: &mut Frame) {
    let theme = settings::theme();
    if theme.background.is_none() && theme.text.is_none() {
        return;
    }
    let mut style = Style::default();
    if let Some(bg) = theme.background {
        style = style.bg(bg);
    }
    if let Some(fg) = theme.text {
        style = style.fg(fg);
    }
    frame.render_widget(Block::default().style(style), frame.area());
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiteconnect::error::ApiError;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn masks_by_char_count() {
        assert_eq!(mask(""), "");
        assert_eq!(mask("hunter2"), "•••••••");
    }

    #[test]
    fn describe_error_sanitizes_api_error_message() {
        let err: anyhow::Error = Error::Api(ApiError::new(
            "TokenException".into(),
            r#"session rejected: {"access_token":"SECRETVAL123"}"#,
            403,
            None,
        ))
        .into();
        let out = describe_error(&err);
        assert!(!out.contains("SECRETVAL123"), "access_token leaked: {out}");
        assert!(out.contains("<redacted>"));
    }

    #[test]
    fn describe_error_sanitizes_fallback_error_string() {
        let err = anyhow::anyhow!(r"network error: access_token=SECRETVAL123 timed out");
        let out = describe_error(&err);
        assert!(!out.contains("SECRETVAL123"), "access_token leaked: {out}");
        assert!(out.contains("<redacted>"));
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    #[test]
    fn creds_new_without_remembered_id_starts_empty_on_user_id() {
        let state = CredsState::new(None);
        assert_eq!(state.user_id, "");
        assert_eq!(state.field, CredsField::UserId);
        assert!(!state.remembered);

        let state = CredsState::new(Some("   ".to_string()));
        assert_eq!(state.field, CredsField::UserId);
        assert!(!state.remembered);
    }

    #[test]
    fn creds_new_with_remembered_id_prefills_and_starts_on_password() {
        let state = CredsState::new(Some("  ZV3952  ".to_string()));
        assert_eq!(state.user_id, "ZV3952");
        assert_eq!(state.field, CredsField::Password);
        assert!(state.remembered);
    }

    #[test]
    fn ctrl_u_clears_remembered_user_and_refocuses_user_id() {
        let mut state = CredsState::new(Some("ZV3952".to_string()));
        for c in "hunter2".chars() {
            apply_creds_key(&mut state, key(KeyCode::Char(c)));
        }
        assert_eq!(state.password, "hunter2");

        let result = apply_creds_key(&mut state, ctrl('u'));
        assert_eq!(result, None);
        assert!(!state.remembered);
        assert_eq!(state.user_id, "");
        assert_eq!(state.password, "");
        assert_eq!(state.field, CredsField::UserId);
    }

    #[test]
    fn remembered_mode_ignores_field_navigation() {
        let mut state = CredsState::new(Some("ZV3952".to_string()));
        apply_creds_key(&mut state, key(KeyCode::BackTab));
        assert_eq!(state.field, CredsField::Password);
        apply_creds_key(&mut state, key(KeyCode::Up));
        assert_eq!(state.field, CredsField::Password);
    }

    #[test]
    fn remembered_mode_submits_password_with_prefilled_user_id() {
        let mut state = CredsState::new(Some("ZV3952".to_string()));
        for c in "hunter2".chars() {
            apply_creds_key(&mut state, key(KeyCode::Char(c)));
        }
        let result = apply_creds_key(&mut state, key(KeyCode::Enter));
        assert_eq!(result, Some(("ZV3952".to_string(), "hunter2".to_string())));
    }

    #[test]
    fn login_reason_subtitle_distinguishes_first_run_and_expiry() {
        assert_eq!(LoginReason::FirstRun.subtitle(), "Sign in to Kite");
        assert!(LoginReason::Expired.subtitle().contains("expired"));
    }

    #[test]
    fn tab_advances_and_stops_at_password() {
        let mut state = CredsState::default();
        assert_eq!(state.field, CredsField::UserId);
        apply_creds_key(&mut state, key(KeyCode::Tab));
        assert_eq!(state.field, CredsField::Password);
        apply_creds_key(&mut state, key(KeyCode::Tab));
        assert_eq!(state.field, CredsField::Password);
    }

    #[test]
    fn backtab_retreats_and_stops_at_user_id() {
        let mut state = CredsState {
            field: CredsField::Password,
            ..Default::default()
        };
        apply_creds_key(&mut state, key(KeyCode::BackTab));
        assert_eq!(state.field, CredsField::UserId);
        apply_creds_key(&mut state, key(KeyCode::BackTab));
        assert_eq!(state.field, CredsField::UserId);
    }

    #[test]
    fn typing_and_backspace_edit_active_field() {
        let mut state = CredsState::default();
        for c in "AB1234".chars() {
            apply_creds_key(&mut state, key(KeyCode::Char(c)));
        }
        assert_eq!(state.user_id, "AB1234");
        apply_creds_key(&mut state, key(KeyCode::Backspace));
        assert_eq!(state.user_id, "AB123");

        apply_creds_key(&mut state, key(KeyCode::Tab));
        for c in "hunter2".chars() {
            apply_creds_key(&mut state, key(KeyCode::Char(c)));
        }
        assert_eq!(state.password, "hunter2");
        assert_eq!(state.user_id, "AB123");
    }

    #[test]
    fn enter_on_user_id_moves_focus_without_submitting() {
        let mut state = CredsState {
            user_id: "AB1234".to_string(),
            ..Default::default()
        };
        let result = apply_creds_key(&mut state, key(KeyCode::Enter));
        assert_eq!(result, None);
        assert_eq!(state.field, CredsField::Password);
    }

    #[test]
    fn enter_on_password_submits_when_both_filled() {
        let mut state = CredsState {
            user_id: "  AB1234  ".to_string(),
            password: "hunter2".to_string(),
            field: CredsField::Password,
            ..Default::default()
        };
        let result = apply_creds_key(&mut state, key(KeyCode::Enter));
        assert_eq!(result, Some(("AB1234".to_string(), "hunter2".to_string())));
    }

    #[test]
    fn submit_with_empty_user_id_errors_and_refocuses() {
        let mut state = CredsState {
            password: "hunter2".to_string(),
            field: CredsField::Password,
            ..Default::default()
        };
        let result = apply_creds_key(&mut state, key(KeyCode::Enter));
        assert_eq!(result, None);
        assert_eq!(state.field, CredsField::UserId);
        assert!(state.error.is_some());
    }

    #[test]
    fn submit_with_empty_password_errors_in_place() {
        let mut state = CredsState {
            user_id: "AB1234".to_string(),
            field: CredsField::Password,
            ..Default::default()
        };
        let result = apply_creds_key(&mut state, key(KeyCode::Enter));
        assert_eq!(result, None);
        assert_eq!(state.field, CredsField::Password);
        assert!(state.error.is_some());
    }

    #[test]
    fn ctrl_char_is_ignored_by_field_input() {
        let mut state = CredsState::default();
        let ctrl_a = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        apply_creds_key(&mut state, ctrl_a);
        assert_eq!(state.user_id, "");
    }

    fn totp_challenge() -> TwofaChallenge {
        TwofaChallenge {
            request_id: "req-1".to_string(),
            twofa_type: "totp".to_string(),
            twofa_types: vec!["totp".to_string()],
            user_name: Some("Test User".to_string()),
            user_id: "AB1234".to_string(),
        }
    }

    #[test]
    fn totp_label_uses_challenge_type() {
        let state = TotpState::new(totp_challenge());
        assert_eq!(state.label(), "TOTP");

        let mut other = totp_challenge();
        other.twofa_type = "sms".to_string();
        let state = TotpState::new(other);
        assert_eq!(state.label(), "sms");
    }

    fn generate_otp_path() -> &'static str {
        "/trusted/test-key/user/AB1234/twofa/generate_otp"
    }

    #[tokio::test]
    async fn twofa_method_selection_cycles_sends_sms_and_clears_the_previous_code() {
        let server = httpmock::MockServer::start_async().await;
        let otp = server
            .mock_async(|when, then| {
                when.method(httpmock::Method::POST)
                    .path(generate_otp_path())
                    .form_urlencoded_tuple("request_id", "req-1");
                then.status(200)
                    .json_body(serde_json::json!({"status": "success", "data": {}}));
            })
            .await;
        let auth = TrustedAuth::new(
            &server.base_url(),
            "test-key",
            "5b697354-64d2-49c4-a080-6b25475555e3",
        )
        .unwrap();
        let mut challenge = totp_challenge();
        challenge.twofa_types = vec!["totp".into(), "sms".into()];
        let mut state = TotpState::new(challenge);
        state.otp = "123456".into();

        select_next_method(&auth, &mut state).await;
        assert_eq!(state.challenge.twofa_type, "sms");
        assert_eq!(state.otp, "");
        assert_eq!(state.error, None);
        otp.assert_calls_async(1).await;

        select_next_method(&auth, &mut state).await;
        assert_eq!(state.challenge.twofa_type, "totp");
        otp.assert_calls_async(1).await;
    }

    #[tokio::test]
    async fn sms_send_failure_keeps_the_challenge_and_allows_switching_or_retrying() {
        let server = httpmock::MockServer::start_async().await;
        let otp = server
            .mock_async(|when, then| {
                when.method(httpmock::Method::POST)
                    .path(generate_otp_path());
                then.status(400).json_body(serde_json::json!({
                    "status": "error",
                    "message": "OTP limit exceeded.",
                    "error_type": "InputException"
                }));
            })
            .await;
        let auth = TrustedAuth::new(
            &server.base_url(),
            "test-key",
            "5b697354-64d2-49c4-a080-6b25475555e3",
        )
        .unwrap();
        let mut challenge = totp_challenge();
        challenge.twofa_type = "sms".into();
        challenge.twofa_types = vec!["sms".into(), "totp".into()];
        let mut state = TotpState::new(challenge);

        request_sms_code(&auth, &mut state).await;
        assert_eq!(state.error.as_deref(), Some("OTP limit exceeded."));
        assert_eq!(state.challenge.request_id, "req-1");
        assert_eq!(state.challenge.twofa_type, "sms");

        select_next_method(&auth, &mut state).await;
        assert_eq!(state.challenge.twofa_type, "totp");
        assert_eq!(state.error, None);
        for c in "123456".chars() {
            apply_totp_key(&mut state, key(KeyCode::Char(c)));
        }
        assert_eq!(
            apply_totp_key(&mut state, key(KeyCode::Enter)),
            Some("123456".to_string())
        );
        otp.assert_calls_async(1).await;

        select_next_method(&auth, &mut state).await;
        assert_eq!(state.challenge.twofa_type, "sms");
        assert_eq!(state.error.as_deref(), Some("OTP limit exceeded."));
        otp.assert_calls_async(2).await;
    }

    #[test]
    fn expired_login_screen_shows_the_remembered_user_and_password_prompt() {
        let state = CredsState::new(Some("AB1234".into()));
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| draw_credentials(frame, &state, LoginReason::Expired))
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(text.contains("Session expired"), "{text}");
        assert!(text.contains("AB1234"), "{text}");
        assert!(text.contains("Password"), "{text}");
    }

    #[test]
    fn twofa_screen_shows_offered_methods_and_the_selected_code_field() {
        let mut challenge = totp_challenge();
        challenge.twofa_type = "sms".into();
        challenge.twofa_types = vec!["totp".into(), "sms".into()];
        let state = TotpState::new(challenge);
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| draw_totp(frame, &state, LoginReason::FirstRun))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let text: String = buffer
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(text.contains("Methods: totp, sms"), "{text}");
        assert!(text.contains("sms"), "{text}");
        assert!(text.contains("Tab method"), "{text}");
    }

    #[test]
    fn credentials_submit_uppercases_the_user_id() {
        let mut state = CredsState {
            user_id: " ab1234 ".into(),
            password: "example".into(),
            field: CredsField::Password,
            ..Default::default()
        };
        assert_eq!(
            apply_creds_key(&mut state, key(KeyCode::Enter)),
            Some(("AB1234".into(), "example".into()))
        );
    }

    #[test]
    fn totp_enter_requires_non_empty_code() {
        let mut state = TotpState::new(totp_challenge());
        let result = apply_totp_key(&mut state, key(KeyCode::Enter));
        assert_eq!(result, None);
        assert!(state.error.is_some());

        for c in "123456".chars() {
            apply_totp_key(&mut state, key(KeyCode::Char(c)));
        }
        let result = apply_totp_key(&mut state, key(KeyCode::Enter));
        assert_eq!(result, Some("123456".to_string()));
    }

    #[test]
    fn is_abort_key_matches_esc_and_ctrl_c() {
        assert!(is_abort_key(key(KeyCode::Esc)));
        assert!(is_abort_key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL
        )));
        assert!(!is_abort_key(key(KeyCode::Char('c'))));
        assert!(!is_abort_key(key(KeyCode::Enter)));
    }
}
