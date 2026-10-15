use super::super::{App, Modal, ToastLevel};
use super::ato::{BasketKey, basket_key};
use crate::tui::alerts::{self, Attribute, Operator};
use crate::tui::data::Alert;
use crate::tui::form::{ControlIntent, FormIssue, FormViewport};
use crate::tui::ticket::{edit_signed_numeric, step_signed_number};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use kiteconnect::{AlertParams, AlertRhsType, AlertStatus, AlertType, BasketWrite};

fn alert_lhs_exchange<'a>(exchange: &'a str, segment: &str) -> &'a str {
    if segment.eq_ignore_ascii_case("INDICES") {
        "INDICES"
    } else {
        exchange
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertFormFocus {
    Kind,
    Attribute,
    Level,
    Operator,

    Basket,
}

impl AlertFormFocus {
    pub const ALL: [AlertFormFocus; 5] = [
        AlertFormFocus::Kind,
        AlertFormFocus::Attribute,
        AlertFormFocus::Level,
        AlertFormFocus::Operator,
        AlertFormFocus::Basket,
    ];

    fn cycle(self, delta: isize, ato: bool) -> Self {
        let rows = if ato { &Self::ALL[..] } else { &Self::ALL[..4] };
        let index = rows
            .iter()
            .position(|focus| *focus == self)
            .unwrap_or(0)
            .cast_signed();
        rows[(index + delta).rem_euclid(rows.len().cast_signed()) as usize]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertFormStage {
    Edit,

    Review,

    Submitting(crate::tui::data::AlertRequestId),
}

#[derive(Debug)]
pub struct AlertFormState {
    pub exchange: String,
    pub tradingsymbol: String,

    pub name: String,
    pub instrument_token: u32,
    pub attribute: Attribute,
    pub level: String,

    pub level_edited: bool,
    pub operator: Operator,
    pub focus: AlertFormFocus,
    pub tick_size: f64,

    pub issue: Option<FormIssue<AlertFormFocus>>,

    pub uuid: Option<String>,

    pub status: AlertStatus,

    pub ato: bool,

    pub was_ato: bool,

    pub draft: alerts::AtoBasketDraft,

    pub basket_selected: usize,

    pub viewport: FormViewport,
    pub stage: AlertFormStage,
}

impl AlertFormState {
    fn toggle_kind(&mut self) {
        if !self.ato
            && let Some(reason) = self.ato_ineligible_reason()
        {
            self.fail(FormIssue::at(AlertFormFocus::Kind, reason));
            return;
        }
        self.ato = !self.ato;
        self.issue = None;
        if !self.ato && self.focus == AlertFormFocus::Basket {
            self.focus = AlertFormFocus::Operator;
        }
        self.viewport.request_reveal();
    }

    pub fn label(&self) -> String {
        if self.exchange.is_empty() {
            self.tradingsymbol.clone()
        } else {
            format!("{}:{}", self.exchange, self.tradingsymbol)
        }
    }

    pub fn is_edit(&self) -> bool {
        self.uuid.is_some()
    }

    pub fn kind_label(&self) -> &'static str {
        if self.ato { "ATO" } else { "alert" }
    }

    pub fn visible(&self) -> &'static [AlertFormFocus] {
        if self.ato {
            &AlertFormFocus::ALL
        } else {
            &AlertFormFocus::ALL[..4]
        }
    }

    pub fn clear_issue(&mut self, field: AlertFormFocus) {
        let Some(failed) = self.issue.as_ref().and_then(|issue| issue.field) else {
            return;
        };
        let controls = matches!(
            (field, failed),
            (AlertFormFocus::Attribute, AlertFormFocus::Level)
                | (AlertFormFocus::Kind, AlertFormFocus::Basket)
        );
        if failed == field || controls {
            self.issue = None;
        }
    }

    pub fn focus_field(&mut self, field: AlertFormFocus) {
        if self.visible().contains(&field) {
            self.focus = field;
            self.viewport.request_reveal();
        }
    }

    pub fn fail(&mut self, issue: FormIssue<AlertFormFocus>) {
        if let Some(field) = issue.field {
            self.focus_field(field);
        }
        self.viewport.request_reveal();
        self.issue = Some(issue);
    }

    pub fn fail_at(&mut self, field: AlertFormFocus, message: &str) {
        self.fail(FormIssue::at(field, message));
    }

    pub fn saves_enabled(&self) -> bool {
        self.status == AlertStatus::Enabled
    }

    pub fn edits_a_disabled_alert(&self) -> bool {
        self.is_edit() && !self.saves_enabled()
    }

    pub fn drops_the_basket(&self) -> bool {
        self.was_ato && !self.ato
    }

    pub fn ato_ineligible_reason(&self) -> Option<String> {
        alerts::ato_ineligible_reason(&self.exchange, &self.tradingsymbol)
    }

    pub fn basket_write(&self) -> Option<BasketWrite> {
        if !self.ato {
            return None;
        }
        self.draft.write(!self.was_ato)
    }

    pub fn save_blocker(&self) -> Option<FormIssue<AlertFormFocus>> {
        if !self.ato {
            return None;
        }
        if let Some(reason) = self.ato_ineligible_reason() {
            return Some(FormIssue::at(AlertFormFocus::Kind, reason));
        }
        self.basket_write()?;
        self.draft.is_empty().then(|| {
            FormIssue::at(
                AlertFormFocus::Basket,
                "an ATO needs at least one order; add one with a, or choose Simple",
            )
        })
    }

    pub(in crate::tui::app) fn params(&self, level: f64) -> AlertParams {
        let mut params = alerts::params(
            &self.name,
            &self.exchange,
            &self.tradingsymbol,
            self.attribute,
            self.operator,
            level,
            self.status == AlertStatus::Enabled,
        );
        if self.ato {
            params.alert_type = AlertType::Ato;
            params.basket = self.basket_write();
        }
        params
    }
}

impl App {
    fn seeded_level(&self, token: u32, attribute: Attribute) -> String {
        self.tick(token)
            .and_then(|tick| attribute.current_value(tick))
            .map(|value| format!("{value:.2}"))
            .unwrap_or_default()
    }

    pub(in crate::tui::app) fn open_alert_form(
        &mut self,
        exchange: &str,
        tradingsymbol: &str,
        token: u32,
        segment: &str,
    ) {
        let (tick_size, _, _, _) = self.contract_specs(token);
        let attribute = Attribute::LastTradedPrice;

        self.modals.push(Modal::Alert(Box::new(AlertFormState {
            exchange: alert_lhs_exchange(exchange, segment).to_string(),

            name: tradingsymbol.to_string(),
            tradingsymbol: tradingsymbol.to_string(),
            instrument_token: token,
            attribute,
            level: self.seeded_level(token, attribute),
            level_edited: false,
            operator: Operator::GreaterThanOrEqual,
            focus: AlertFormFocus::Kind,
            tick_size,
            issue: None,
            uuid: None,
            status: AlertStatus::Enabled,

            ato: false,
            was_ato: false,
            draft: alerts::AtoBasketDraft::new(),
            basket_selected: 0,
            viewport: FormViewport::default(),
            stage: AlertFormStage::Edit,
        })));
    }

    pub(in crate::tui::app) fn open_alert_edit(&mut self, alert: &Alert) {
        if alert.rhs_type != AlertRhsType::Constant.to_string() {
            self.toast(
                ToastLevel::Info,
                "instrument-vs-instrument alerts are read-only here, edit them on Kite Web"
                    .to_string(),
            );
            return;
        }
        let Some(attribute) = Attribute::from_wire(&alert.lhs_attribute) else {
            self.toast(
                ToastLevel::Info,
                format!(
                    "`{}` alerts are read-only here, edit them on Kite Web",
                    alert.lhs_attribute
                ),
            );
            return;
        };
        let Some(operator) = Operator::from_wire(&alert.operator) else {
            self.toast(
                ToastLevel::Info,
                format!(
                    "`{}` alerts are read-only here, edit them on Kite Web",
                    alert.operator
                ),
            );
            return;
        };

        let token = self
            .instruments
            .as_ref()
            .and_then(|store| {
                store
                    .by_exact_symbol(&alert.lhs_tradingsymbol)
                    .into_iter()
                    .find(|inst| {
                        alerts::lhs_matches(
                            &alert.lhs_exchange,
                            &alert.lhs_tradingsymbol,
                            &inst.exchange,
                            &inst.segment,
                            &inst.tradingsymbol,
                        )
                    })
                    .map(|inst| inst.instrument_token)
            })
            .unwrap_or(0);
        let (tick_size, _, _, _) = self.contract_specs(token);
        self.modals.push(Modal::Alert(Box::new(AlertFormState {
            exchange: alert.lhs_exchange.clone(),
            tradingsymbol: alert.lhs_tradingsymbol.clone(),

            name: if alert.name.is_empty() {
                alert.lhs_tradingsymbol.clone()
            } else {
                alert.name.clone()
            },
            instrument_token: token,
            attribute,
            level: alert.rhs_constant.to_string(),

            level_edited: true,
            operator,
            focus: AlertFormFocus::Kind,
            tick_size,
            issue: None,
            uuid: Some(alert.uuid.clone()),
            status: alert.status,
            ato: crate::tui::data::alert_is_ato(alert),
            was_ato: crate::tui::data::alert_is_ato(alert),
            draft: match &alert.basket {
                Some(basket) => alerts::AtoBasketDraft::from_fetched(&basket.items),

                None if crate::tui::data::alert_is_ato(alert) => {
                    alerts::AtoBasketDraft::unavailable()
                }
                None => alerts::AtoBasketDraft::new(),
            },
            basket_selected: 0,
            viewport: FormViewport::default(),
            stage: AlertFormStage::Edit,
        })));
    }

    pub(in crate::tui::app) fn key_alert(&mut self, key: KeyEvent, mut state: Box<AlertFormState>) {
        if matches!(state.stage, AlertFormStage::Submitting(_)) {
            if key.code == KeyCode::Esc {
                return;
            }
            if matches!(key.code, KeyCode::PageUp | KeyCode::PageDown) {
                state
                    .viewport
                    .page(if key.code == KeyCode::PageUp { -1 } else { 1 });
            }
            self.modals.push(Modal::Alert(state));
            return;
        }

        if state.stage == AlertFormStage::Review {
            match key.code {
                KeyCode::Esc => state.stage = AlertFormStage::Edit,

                KeyCode::Enter | KeyCode::Char('y')
                    if key.kind != KeyEventKind::Repeat && state.viewport.drawn() =>
                {
                    self.send_alert_form(&mut state);
                }
                KeyCode::PageUp => state.viewport.page(-1),
                KeyCode::PageDown => state.viewport.page(1),
                _ => {}
            }
            self.modals.push(Modal::Alert(state));
            return;
        }
        if state.focus == AlertFormFocus::Basket {
            match basket_key(key, &mut state) {
                BasketKey::NotOurs => {}
                BasketKey::Handled => {
                    self.modals.push(Modal::Alert(state));
                    return;
                }

                BasketKey::AddOrder => {
                    self.modals.push(Modal::Alert(state));
                    self.open_ato_item_search();
                    return;
                }
                BasketKey::EditOrder => {
                    self.modals.push(Modal::Alert(state));
                    self.open_ato_item_edit();
                    return;
                }
            }
        }
        match key.code {
            KeyCode::Esc => return self.close_alert_draft(state),
            KeyCode::Tab => state.focus_field(state.focus.cycle(1, state.ato)),
            KeyCode::BackTab => state.focus_field(state.focus.cycle(-1, state.ato)),
            KeyCode::Enter if state.level.trim().is_empty() => {
                state.fail_at(AlertFormFocus::Level, "Enter a level to alert on.");
            }
            KeyCode::Enter => match self.stage_entered_alert_level(&mut state) {
                Ok(()) if state.stage == AlertFormStage::Edit => return,
                Ok(()) => {}
                Err(issue) => state.fail(issue),
            },
            KeyCode::PageUp => state.viewport.page(-1),
            KeyCode::PageDown => state.viewport.page(1),
            KeyCode::Up | KeyCode::Down if state.focus == AlertFormFocus::Level => {
                let direction: i8 = if key.code == KeyCode::Up { 1 } else { -1 };
                let current_value = self
                    .tick(state.instrument_token)
                    .and_then(|tick| state.attribute.current_value(tick))
                    .unwrap_or(0.0);
                state.level =
                    step_signed_number(&state.level, state.tick_size, direction, current_value);
                state.level_edited = true;
                state.clear_issue(AlertFormFocus::Level);
                state.viewport.request_reveal();
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
                if state.focus != AlertFormFocus::Level =>
            {
                let intent = if key.code == KeyCode::Left {
                    ControlIntent::Previous
                } else {
                    ControlIntent::Next
                };
                let focus = state.focus;
                self.select_alert(&mut state, focus, intent);
            }
            code if state.focus == AlertFormFocus::Level => {
                if !state.level_edited && matches!(code, KeyCode::Char(_)) {
                    state.level.clear();
                }
                let before = state.level.clone();
                edit_signed_numeric(&mut state.level, code);
                if state.level != before {
                    state.level_edited = true;
                    state.clear_issue(AlertFormFocus::Level);
                    state.viewport.request_reveal();
                }
            }
            _ => {}
        }
        self.modals.push(Modal::Alert(state));
    }

    fn close_alert_draft(&mut self, state: Box<AlertFormState>) {
        if !state.draft.is_unchanged() {
            self.modals.push(Modal::Alert(state));
            self.modals.push(Modal::Confirm(Box::new(
                crate::tui::ticket::ConfirmState::plain(
                    "Discard alert basket changes?".to_string(),
                    vec![(
                        "Draft".to_string(),
                        "Unsaved basket changes will be lost".to_string(),
                    )],
                    crate::tui::ticket::PendingAction::DiscardAlertDraft,
                ),
            )));
        }
    }

    fn stage_entered_alert_level(
        &mut self,
        state: &mut AlertFormState,
    ) -> Result<(), FormIssue<AlertFormFocus>> {
        let level = state
            .level
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|level| level.is_finite())
            .ok_or_else(|| FormIssue::at(AlertFormFocus::Level, "Level must be a number."))?;
        let current_value = self
            .tick(state.instrument_token)
            .and_then(|tick| state.attribute.current_value(tick));

        alerts::validate_level(state.attribute, state.operator, level, current_value)
            .map_err(|message| FormIssue::at(AlertFormFocus::Level, message))?;
        self.stage_alert_save(state, level)
    }

    pub(in crate::tui::app) fn select_alert(
        &mut self,
        state: &mut AlertFormState,
        field: AlertFormFocus,
        intent: ControlIntent,
    ) {
        if !state.visible().contains(&field) {
            return;
        }
        state.focus_field(field);
        state.clear_issue(field);
        match field {
            AlertFormFocus::Kind => {
                let wanted = match intent {
                    ControlIntent::Focus => return,
                    ControlIntent::Option(index) => index == 1,
                    ControlIntent::Previous | ControlIntent::Next => !state.ato,
                };
                if wanted != state.ato {
                    state.toggle_kind();
                }
            }
            AlertFormFocus::Attribute => {
                let delta = match intent {
                    ControlIntent::Previous => -1,
                    ControlIntent::Next => 1,
                    ControlIntent::Focus | ControlIntent::Option(_) => return,
                };
                state.attribute = state.attribute.cycle(delta);

                if !state.level_edited {
                    state.level = self.seeded_level(state.instrument_token, state.attribute);
                }
                state.clear_issue(AlertFormFocus::Attribute);
            }
            AlertFormFocus::Operator => {
                let operators = crate::tui::alerts::Operator::ALL;
                let current = operators
                    .iter()
                    .position(|operator| *operator == state.operator)
                    .unwrap_or(0);
                if let Some(next) = intent.pick(current, operators.len()) {
                    state.operator = operators[next];
                }
            }

            AlertFormFocus::Basket => {
                if let ControlIntent::Option(index) = intent
                    && index < state.draft.len()
                {
                    state.basket_selected = index;
                }
            }
            AlertFormFocus::Level => {}
        }
    }
}
#[cfg(test)]
mod tests {
    use super::super::super::test_support::{
        alert_form, ato_alert, char_key, char_key_code, issue_message, master_inst, server_alert,
        test_app,
    };
    use super::super::super::{AlertFormFocus, Modal};
    use crate::tui::alerts::{self, Attribute, Operator};

    use crate::tui::instruments::InstrumentStore;
    use crate::tui::marketwatch::WatchItem;
    use crate::tui::workspace::PaneId;
    use crossterm::event::KeyCode;
    use kiteconnect::AlertStatus;
    use kiteconnect::ticker::Tick;
    use std::sync::Arc;

    #[test]
    fn alert_form_focus_cycles_and_only_level_accepts_numeric_input() {
        let mut app = test_app();
        app.key_alert(
            char_key_code(KeyCode::Right),
            alert_form(1, Attribute::LastTradedPrice, Operator::GreaterThanOrEqual),
        );
        let Some(Modal::Alert(state)) = app.modals.pop() else {
            panic!("the form remains open after changing an attribute");
        };
        assert_eq!(state.focus, AlertFormFocus::Attribute);
        assert_eq!(state.attribute, Attribute::HighPrice);
        assert_eq!(state.level.len(), 0);

        app.key_alert(char_key_code(KeyCode::Tab), state);
        let Some(Modal::Alert(state)) = app.modals.pop() else {
            panic!("Tab keeps the form open");
        };
        assert_eq!(state.focus, AlertFormFocus::Level);
        app.key_alert(char_key_code(KeyCode::Char('9')), state);
        let Some(Modal::Alert(state)) = app.modals.pop() else {
            panic!("numeric level edit keeps the form open");
        };
        assert_eq!(state.level, "9");

        app.key_alert(char_key_code(KeyCode::Tab), state);
        let Some(Modal::Alert(state)) = app.modals.pop() else {
            panic!("Tab keeps the form open");
        };
        assert_eq!(state.focus, AlertFormFocus::Operator);
        app.key_alert(char_key_code(KeyCode::Right), state);
        let Some(Modal::Alert(state)) = app.modals.pop() else {
            panic!("operator selection keeps the form open");
        };
        assert_eq!(state.operator, Operator::LessThan);

        app.key_alert(char_key_code(KeyCode::BackTab), state);
        let Some(Modal::Alert(state)) = app.modals.pop() else {
            panic!("BackTab keeps the form open");
        };
        assert_eq!(state.focus, AlertFormFocus::Level);
    }

    #[test]
    fn alert_change_thresholds_accept_and_step_negative_values() {
        let mut app = test_app();
        let mut state = alert_form(1, Attribute::DayChangePercent, Operator::LessThanOrEqual);
        state.focus = AlertFormFocus::Level;
        app.key_alert(char_key_code(KeyCode::Char('-')), state);
        let Some(Modal::Alert(state)) = app.modals.pop() else {
            panic!("signed edit keeps the form open");
        };
        app.key_alert(char_key_code(KeyCode::Char('3')), state);
        let Some(Modal::Alert(state)) = app.modals.pop() else {
            panic!("signed edit keeps the form open");
        };
        assert_eq!(state.level, "-3");
        app.key_alert(char_key_code(KeyCode::Up), state);
        let Some(Modal::Alert(state)) = app.modals.pop() else {
            panic!("signed step keeps the form open");
        };
        assert_eq!(state.level, "-2.95");
    }

    #[tokio::test]
    async fn the_level_prefill_follows_the_attribute_until_the_user_types() {
        let mut app = test_app();
        app.ephemeral = true;
        app.set_pane(PaneId::TradingWatch);
        app.watch.test_push(WatchItem {
            exchange: "NSE".into(),
            tradingsymbol: "INFY".into(),
            instrument_token: 408_065,
            segment: "NSE".into(),
            ..WatchItem::default()
        });
        app.ticks.insert(408_065, {
            let mut value = Tick::default();
            value.instrument_token = 408_065;
            value.last_price = Some(1520.0);
            value.volume = Some(1_000);
            value
        });

        app.on_key(char_key('a'));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("a must open the alert form");
        };
        assert_eq!(state.level, "1520.00", "seeded from the LTP it opens on");

        app.on_key(char_key_code(KeyCode::Tab));
        for _ in 0..11 {
            app.on_key(char_key_code(KeyCode::Right));
        }
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("the form must stay open while the attribute cycles");
        };
        assert_eq!(state.attribute, Attribute::VolumeTraded);
        assert_eq!(
            state.level, "1000.00",
            "an untouched prefill follows the attribute"
        );

        app.on_key(char_key_code(KeyCode::Tab));
        for ch in "2500".chars() {
            app.on_key(char_key(ch));
        }
        app.on_key(char_key_code(KeyCode::BackTab));
        app.on_key(char_key_code(KeyCode::Left));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("the form must still be open");
        };
        assert_eq!(state.level, "2500", "a typed level is never overwritten");
    }

    #[tokio::test]
    async fn the_form_reports_its_refusals_without_discarding_the_condition() {
        let mut app = test_app();
        app.set_alerts(vec![server_alert("INFY", "NSE", ">=", 110.0)]);

        app.modals.push(Modal::Alert(alert_form(
            100,
            Attribute::LastTradedPrice,
            Operator::GreaterThanOrEqual,
        )));
        app.on_key(char_key_code(KeyCode::Enter));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("a blank level must keep the form open");
        };
        assert_eq!(issue_message(state), Some("Enter a level to alert on."));

        app.modals.clear();
        let mut state = alert_form(
            100,
            Attribute::LastTradedPrice,
            Operator::GreaterThanOrEqual,
        );
        state.level = "110".into();
        app.modals.push(Modal::Alert(state));
        app.on_key(char_key_code(KeyCode::Enter));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("a duplicate must keep the form open");
        };
        assert_eq!(
            issue_message(state),
            Some("this alert already exists on your account")
        );
        assert_eq!(
            state.issue.as_ref().and_then(|issue| issue.field),
            None,
            "a refused request belongs to no field"
        );
        assert_eq!(state.level, "110", "the typed condition must survive");
    }

    #[test]
    fn editing_a_disabled_alert_is_flagged_as_still_muted() {
        let mut app = test_app();
        let mut fired = server_alert("INFY", "NSE", ">=", 110.0);
        fired.status = AlertStatus::Disabled;
        fired.alert_count = 1;
        app.set_alerts(vec![fired.clone()]);

        app.open_alert_edit(&fired);
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("e must open the edit form");
        };
        assert!(state.edits_a_disabled_alert());

        app.modals.clear();
        app.open_alert_edit(&server_alert("INFY", "NSE", ">=", 110.0));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("e must open the edit form");
        };
        assert!(!state.edits_a_disabled_alert());
        assert!(
            !alert_form(
                100,
                Attribute::LastTradedPrice,
                Operator::GreaterThanOrEqual
            )
            .edits_a_disabled_alert()
        );
    }

    #[test]
    fn an_index_row_creates_its_alert_under_the_indices_exchange() {
        let mut app = test_app();
        app.ephemeral = true;
        app.set_pane(PaneId::TradingWatch);
        app.watch.test_push(WatchItem {
            exchange: "NSE".into(),
            tradingsymbol: "NIFTY 50".into(),
            instrument_token: 256_265,
            segment: "INDICES".into(),
            ..WatchItem::default()
        });

        app.on_key(char_key('a'));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("a must open the alert form for an index too");
        };
        assert_eq!(state.exchange, "INDICES");
        assert_eq!(state.tradingsymbol, "NIFTY 50");

        assert!(alerts::lhs_matches(
            &state.exchange,
            &state.tradingsymbol,
            "NSE",
            "INDICES",
            "NIFTY 50"
        ));
    }

    #[tokio::test]
    async fn editing_an_alert_without_a_live_price_still_saves() {
        let mut app = test_app();
        let alert = server_alert("INFY", "NSE", ">=", 110.0);
        app.set_alerts(vec![alert.clone()]);
        app.open_alert_edit(&alert);
        let Some(Modal::Alert(state)) = app.modals.pop() else {
            panic!("e must open the edit form");
        };
        assert!(state.is_edit());
        assert_eq!(
            app.live_ltp(state.instrument_token),
            None,
            "an unsubscribed instrument has no live price"
        );

        app.key_alert(char_key_code(KeyCode::Enter), state);
        match app.modals.last() {
            None => {}
            Some(Modal::Alert(state)) => {
                panic!("the edit was refused: {:?}", state.issue)
            }
            Some(_) => panic!("unexpected modal after submitting the edit form"),
        }
    }

    #[test]
    fn alert_edit_resolves_its_instrument_by_exact_symbol() {
        let mut app = test_app();
        let mut rows = vec![master_inst("NSE", "INDICES", "NIFTY 50", 256_265)];
        for i in 0..40u32 {
            rows.push(master_inst(
                "NFO",
                "NFO-OPT",
                &format!("NIFTY24AUG{}CE", 20_000 + i * 50),
                900_000 + i,
            ));
        }
        app.instruments = Some(Arc::new(InstrumentStore::new(rows)));

        let alert = server_alert("NIFTY 50", "INDICES", ">=", 27_000.0);
        app.open_alert_edit(&alert);
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("e must open the edit form");
        };
        assert_eq!(
            state.instrument_token, 256_265,
            "the index row must be found among its own option strikes"
        );
    }

    #[test]
    fn editing_an_alert_never_renames_it() {
        let mut app = test_app();
        let mut alert = server_alert("INFY", "NSE", ">=", 110.0);
        alert.name = "buy the dip INFY".into();
        app.set_alerts(vec![alert.clone()]);
        app.open_alert_edit(&alert);
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("e must open the edit form");
        };
        assert_eq!(
            state.params(120.0).name,
            "buy the dip INFY",
            "a level edit must not rename the alert"
        );

        assert_eq!(
            alert_form(
                408_065,
                Attribute::LastTradedPrice,
                Operator::GreaterThanOrEqual,
            )
            .params(120.0)
            .name,
            "INFY"
        );
    }

    #[test]
    fn editing_an_ato_loads_its_basket_and_preserves_it_untouched() {
        let mut app = test_app();
        let alert = ato_alert();

        app.open_alert_edit(&alert);
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("an ATO must open for editing");
        };
        assert!(state.ato && state.was_ato);
        assert_eq!(state.draft.len(), 1);
        assert_eq!(state.draft.items()[0].label(), "NSE:GOLDBEES");
        assert_eq!(
            state.basket_write(),
            None,
            "an untouched basket must be omitted, not resent"
        );
        let call = kiteconnect::KiteConnect::builder("test-key")
            .build()
            .unwrap()
            .modify_alert("uuid", state.params(110.0));
        let kiteconnect::Body::Form(form) = call.request().body() else {
            panic!("an alert is a form");
        };
        assert!(form.iter().any(|(k, v)| k == "type" && v == "ato"));
        assert!(
            !form.iter().any(|(k, _)| k == "basket"),
            "a condition-only ATO edit must send no basket field"
        );
    }

    #[test]
    fn the_form_opens_simple_and_ato_is_chosen_on_the_kind_row() {
        let mut app = test_app();
        app.open_alert_form("NSE", "INFY", 408_065, "NSE");
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("the form must open");
        };
        assert!(!state.ato, "a keystroke must not arm orders");
        assert_eq!(state.focus, AlertFormFocus::Kind);
        assert!(state.draft.is_empty());

        app.on_key(char_key_code(KeyCode::Right));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("the form must stay open");
        };
        assert!(state.ato);
        assert!(
            state.draft.is_empty(),
            "choosing ATO must not invent an order the user never described"
        );

        app.on_key(char_key_code(KeyCode::Left));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("the form must stay open");
        };
        assert!(!state.ato);
    }

    #[test]
    fn an_excluded_condition_instrument_refuses_ato() {
        let mut app = test_app();
        app.open_alert_form("MCX", "GOLDM25SEPFUT", 0, "MCX");
        app.on_key(char_key_code(KeyCode::Right));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("the form must stay open");
        };
        assert!(!state.ato, "MCX conditions cannot place orders");
        assert!(
            issue_message(state).is_some_and(|err| err.contains("MCX")),
            "the refusal must say why: {:?}",
            state.issue
        );
        assert_eq!(
            state.issue.as_ref().and_then(|issue| issue.field),
            Some(AlertFormFocus::Kind),
            "an ineligible ATO is a fact about the Type row"
        );
    }

    #[tokio::test]
    async fn an_ato_whose_basket_was_not_attached_can_still_be_edited() {
        let mut app = test_app();
        let unattached = {
            let mut row = ato_alert();
            row.basket = None;
            row
        };
        app.open_alert_edit(&unattached);
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("an ATO with no attached basket must still open");
        };
        assert!(state.ato && state.was_ato);
        assert!(state.draft.is_empty());
        assert_eq!(
            state.basket_write(),
            None,
            "a write that says nothing about the basket preserves it"
        );
        assert_eq!(
            state.save_blocker(),
            None,
            "an unstated basket is not an empty one"
        );

        app.modals.clear();
        app.open_alert_edit(&ato_alert());
        for _ in 0..4 {
            app.on_key(char_key_code(KeyCode::Tab));
        }
        app.on_key(char_key('d'));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("open");
        };
        assert!(state.draft.is_empty());
        assert!(
            state
                .save_blocker()
                .is_some_and(|err| err.message.contains("at least one order")),
            "removing every order must be refused, not silently downgraded"
        );
    }

    #[test]
    fn alert_edit_round_trips_fractional_constants_without_rounding() {
        let mut app = test_app();
        let alert = server_alert("INFY", "NSE", ">", 1.2345);
        app.open_alert_edit(&alert);
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("alert edit must open");
        };
        assert_eq!(state.level, "1.2345");
        assert_eq!(
            state
                .params(state.level.parse().unwrap())
                .rhs_constant
                .to_bits(),
            1.2345_f64.to_bits()
        );
    }

    #[test]
    fn editing_preserves_every_supported_operator_and_high_price_payload() {
        let mut app = test_app();
        let mut high_price = server_alert("INFY", "NSE", ">", 120.0);
        high_price.lhs_attribute = "HighPrice".into();
        app.open_alert_edit(&high_price);
        let Some(Modal::Alert(state)) = app.modals.pop() else {
            panic!("a HighPrice alert must open for editing");
        };
        assert_eq!(state.attribute, Attribute::HighPrice);
        assert_eq!(state.operator, Operator::GreaterThan);
        let call = kiteconnect::KiteConnect::builder("test-key")
            .build()
            .unwrap()
            .modify_alert("uuid", state.params(130.0));
        let kiteconnect::Body::Form(payload) = call.request().body() else {
            panic!("an alert is a form");
        };
        assert!(
            payload
                .iter()
                .any(|(key, value)| key == "lhs_attribute" && value == "HighPrice")
        );
        assert!(
            payload
                .iter()
                .any(|(key, value)| key == "operator" && value == ">")
        );

        for (wire, operator) in [
            ("==", Operator::Equal),
            (">", Operator::GreaterThan),
            (">=", Operator::GreaterThanOrEqual),
            ("<", Operator::LessThan),
            ("<=", Operator::LessThanOrEqual),
        ] {
            app.open_alert_edit(&server_alert("INFY", "NSE", wire, 110.0));
            let Some(Modal::Alert(state)) = app.modals.pop() else {
                panic!("{wire} must not be rewritten or rejected");
            };
            assert_eq!(state.operator, operator);
            assert_eq!(state.params(120.0).operator.to_string(), wire);
        }
    }

    #[test]
    fn unknown_conditions_and_instrument_rhs_are_read_only_with_kite_web_guidance() {
        let mut app = test_app();
        let mut unknown_attribute = server_alert("INFY", "NSE", ">=", 110.0);
        unknown_attribute.lhs_attribute = "FutureAttribute".into();
        app.open_alert_edit(&unknown_attribute);
        assert!(app.modals.is_empty());
        assert!(
            app.toasts()
                .iter()
                .any(|toast| toast.text.contains("Kite Web"))
        );

        app.open_alert_edit(&server_alert("INFY", "NSE", "!=", 110.0));
        assert!(app.modals.is_empty());
        assert!(
            app.toasts()
                .iter()
                .any(|toast| toast.text.contains("Kite Web"))
        );

        let mut instrument_rhs = server_alert("INFY", "NSE", ">=", 110.0);
        instrument_rhs.rhs_type = "instrument".into();
        app.open_alert_edit(&instrument_rhs);
        assert!(app.modals.is_empty());
        assert!(
            app.toasts()
                .iter()
                .any(|toast| toast.text.contains("Kite Web"))
        );
    }
}
