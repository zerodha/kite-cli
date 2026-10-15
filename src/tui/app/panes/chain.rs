use super::super::{
    App, CHAIN_UNDERLYINGS, Modal, PaneId, SearchPurpose, SearchState, Side, ToastLevel,
};
use crate::tui::format;
#[cfg(test)]
use crate::tui::instruments::OptionChain;
use crate::tui::instruments::{Instrument, InstrumentStore, OptionLeg};
use crate::tui::pane_state;
use crossterm::event::{KeyCode, KeyEvent};
use std::time::SystemTime;

impl App {
    pub fn chain_pane_of(&self, pane: PaneId) -> Option<&pane_state::ChainPane> {
        (pane == PaneId::DerivativesChain).then_some(&self.derivatives_chain)
    }

    pub(in crate::tui::app) fn chain_instance_mut(
        &mut self,
        pane: PaneId,
    ) -> Option<&mut pane_state::ChainPane> {
        (pane == PaneId::DerivativesChain).then_some(&mut self.derivatives_chain)
    }

    pub(in crate::tui::app) fn key_chain(&mut self, pane: PaneId, key: KeyEvent) {
        match key.code {
            KeyCode::Char('[') => self.cycle_chain_expiry(pane, -1),
            KeyCode::Char(']') => self.cycle_chain_expiry(pane, 1),
            KeyCode::Char('u') => self.cycle_chain_underlying(pane, 1),
            KeyCode::Char('/') => {
                self.modals.push(Modal::Search(SearchState {
                    purpose: SearchPurpose::ChainUnderlying,
                    ..SearchState::from_pane(pane)
                }));
            }
            KeyCode::Char('h') => self.set_chain_call_focus(pane, true),
            KeyCode::Char('l') => self.set_chain_call_focus(pane, false),
            KeyCode::Char('b') => self.open_chain_ticket(pane, Side::Buy),
            KeyCode::Char('s') => self.open_chain_ticket(pane, Side::Sell),

            KeyCode::Char('a') => self.open_chain_alert(pane),
            _ => {}
        }
    }

    pub(in crate::tui::app) fn set_chain_call_focus(&mut self, pane: PaneId, call: bool) {
        if let Some(instance) = self.chain_instance_mut(pane) {
            instance.set_call_focus(call);
        }
    }

    pub(in crate::tui::app) fn ensure_chain(&mut self) {
        let mut rebuilt = false;
        for pane in self.workspace.panes().iter().copied() {
            if self
                .chain_pane_of(pane)
                .is_some_and(|instance| instance.chain().is_none())
                && self.instruments.is_some()
            {
                self.rebuild_chain(pane);
                rebuilt = true;
            }
        }
        if !rebuilt {
            self.sync_subscriptions();
        }
    }

    pub(in crate::tui::app) fn rebuild_chain(&mut self, pane: PaneId) {
        let Some(store) = self.instruments.clone() else {
            if let Some(instance) = self.chain_instance_mut(pane) {
                instance.clear();
            }
            return;
        };
        let Some(instance) = self.chain_pane_of(pane) else {
            return;
        };
        let today = today_ist();
        let underlying = instance.underlying().to_string();
        let chain = store.option_chain(&underlying, instance.expiry(), &today);
        let spot_token = resolve_chain_spot_token(&store, &underlying);
        let spot = spot_token.and_then(|token| self.live_ltp(token));
        if let Some(instance) = self.chain_instance_mut(pane) {
            instance.install(chain, spot_token, spot);
        }
        self.sync_subscriptions();
    }

    fn cycle_chain_expiry(&mut self, pane: PaneId, delta: isize) {
        let Some(chain) = self.chain_pane_of(pane).and_then(|i| i.chain()) else {
            return;
        };
        if chain.expiries.is_empty() {
            return;
        }
        let len = chain.expiries.len().cast_signed();
        let cur = chain
            .expiry
            .as_ref()
            .and_then(|exp| chain.expiries.iter().position(|e| e == exp))
            .unwrap_or(0)
            .cast_signed();
        let next = (cur + delta).rem_euclid(len) as usize;
        let expiry = chain.expiries[next].clone();
        if let Some(instance) = self.chain_instance_mut(pane) {
            instance.set_expiry(Some(expiry));
        }
        self.rebuild_chain(pane);
    }

    pub(in crate::tui::app) fn set_chain_underlying(&mut self, pane: PaneId, name: String) {
        if name.is_empty() {
            return;
        }
        let Some(instance) = self.chain_pane_of(pane) else {
            return;
        };
        if name.eq_ignore_ascii_case(instance.underlying()) {
            return;
        }

        if let Some(store) = self.instruments.clone() {
            let today = today_ist();
            let probe = store.option_chain(&name, None, &today);
            if probe.rows.is_empty() {
                self.toast(ToastLevel::Error, format!("no option chain for {name}"));
                return;
            }
        }
        self.set_chain_underlying_to(pane, name);
    }

    fn cycle_chain_underlying(&mut self, pane: PaneId, delta: isize) {
        let Some(instance) = self.chain_pane_of(pane) else {
            return;
        };
        let idx = CHAIN_UNDERLYINGS
            .iter()
            .position(|u| *u == instance.underlying())
            .unwrap_or(0)
            .cast_signed();
        let next = (idx + delta).rem_euclid(CHAIN_UNDERLYINGS.len().cast_signed()) as usize;
        self.set_chain_underlying_to(pane, CHAIN_UNDERLYINGS[next].to_string());
    }

    pub(in crate::tui::app) fn set_chain_underlying_to(&mut self, pane: PaneId, name: String) {
        let Some(instance) = self.chain_instance_mut(pane) else {
            return;
        };
        let message = format!("chain: {name}");
        instance.set_underlying(name);
        self.rebuild_chain(pane);
        self.toast(ToastLevel::Info, message);
    }

    fn chain_focused_leg(&mut self, pane: PaneId) -> Option<OptionLeg> {
        let instance = self.chain_pane_of(pane)?;
        let call_focus = instance.call_focus();
        match instance.focused_leg()? {
            Ok(leg) => Some(leg),
            Err(strike) => {
                self.toast(
                    ToastLevel::Error,
                    format!(
                        "no {} at {}",
                        if call_focus { "call" } else { "put" },
                        format::price(strike),
                    ),
                );
                None
            }
        }
    }

    fn open_chain_ticket(&mut self, pane: PaneId, side: Side) {
        let Some(leg) = self.chain_focused_leg(pane) else {
            return;
        };
        self.open_ticket(
            side,
            leg.exchange,
            leg.tradingsymbol,
            leg.instrument_token,
            0,
            Some("NRML"),
        );
    }

    fn open_chain_alert(&mut self, pane: PaneId) {
        let Some(leg) = self.chain_focused_leg(pane) else {
            return;
        };
        self.open_alert_form(&leg.exchange, &leg.tradingsymbol, leg.instrument_token, "");
    }

    pub(in crate::tui::app) fn recenter_chain(&mut self) {
        let mut moved = false;
        for pane in self.workspace.panes().iter().copied() {
            let Some(spot) = self
                .chain_pane_of(pane)
                .and_then(pane_state::ChainPane::spot_token)
                .map(|token| self.live_ltp(token))
            else {
                continue;
            };
            if let Some(instance) = self.chain_instance_mut(pane) {
                moved |= instance.recenter(spot);
            }
        }
        if moved {
            self.sync_subscriptions();
        }
    }

    #[cfg(test)]
    pub fn set_chain_for_test(&mut self, chain: OptionChain) {
        self.derivatives_chain.install(chain, None, None);
    }

    pub fn chain_atm_strike_of(&self, pane: PaneId) -> Option<f64> {
        let instance = self.chain_pane_of(pane)?;
        let spot = instance.spot_token().and_then(|token| self.live_ltp(token));
        let row = instance.atm_row(spot)?;
        instance.rows().get(row).map(|row| row.strike)
    }
}

pub(in crate::tui::app) fn today_ist() -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (y, m, d) = format::date_ist(now);
    format!("{y:04}-{m:02}-{d:02}")
}

fn chain_spot_symbol(underlying: &str) -> Option<&'static str> {
    match underlying {
        "NIFTY" => Some("NIFTY 50"),
        "BANKNIFTY" => Some("NIFTY BANK"),
        "FINNIFTY" => Some("NIFTY FIN SERVICE"),
        "MIDCPNIFTY" => Some("NIFTY MID SELECT"),
        _ => None,
    }
}

pub(in crate::tui::app) fn underlying_name_of(inst: &Instrument) -> String {
    if (inst.segment.ends_with("-OPT") || inst.segment.ends_with("-FUT")) && !inst.name.is_empty() {
        return inst.name.clone();
    }
    inst.tradingsymbol.clone()
}

pub(in crate::tui::app) fn resolve_chain_spot_token(
    store: &InstrumentStore,
    underlying: &str,
) -> Option<u32> {
    if let Some(symbol) = chain_spot_symbol(underlying) {
        return store
            .resolve(symbol, Some("NSE"), 10)
            .into_iter()
            .find(|inst| inst.tradingsymbol == symbol && inst.exchange == "NSE")
            .map(|inst| inst.instrument_token);
    }

    store
        .resolve(underlying, Some("NSE"), 10)
        .into_iter()
        .find(|inst| {
            inst.exchange == "NSE"
                && inst.segment == "NSE"
                && inst.tradingsymbol.eq_ignore_ascii_case(underlying)
        })
        .map(|inst| inst.instrument_token)
}

#[cfg(test)]
mod tests {
    use super::super::super::test_support::{
        H, W, chain_inst, char_key, click, region_of, render, test_app, text_at,
    };
    use super::super::super::{Modal, ui};
    use super::underlying_name_of;
    use crate::tui::event::AppEvent;
    use crate::tui::workspace::PaneId;
    use crate::tui::workspace::Preset;
    use kiteconnect::ticker::{Tick, TickerEvent};

    fn test_chain() -> crate::tui::instruments::OptionChain {
        use crate::instruments::{OptionLeg, OptionRow};
        let leg = |token: u32, tradingsymbol: String| OptionLeg {
            instrument_token: token,
            tradingsymbol,
            exchange: "NFO".into(),
            lot_size: 50,
            tick_size: 0.05,
        };
        crate::tui::instruments::OptionChain {
            underlying: "NIFTY".into(),
            expiries: vec!["2026-09-24".into(), "2026-10-29".into()],
            expiry: Some("2026-09-24".into()),
            rows: (0..6)
                .map(|i| {
                    let strike = 24000 + i * 100;
                    OptionRow {
                        strike: f64::from(strike),
                        call: Some(leg(1000 + i, format!("NIFTY{strike}CE"))),
                        put: Some(leg(2000 + i, format!("NIFTY{strike}PE"))),
                    }
                })
                .collect(),
        }
    }

    #[tokio::test]
    async fn the_first_spot_recentres_the_derivatives_chain() {
        let mut app = test_app();
        app.set_workspace(Preset::Derivatives);
        let mut chain = test_chain();
        chain.rows.truncate(3);
        for (row, strike) in chain.rows.iter_mut().zip([100.0, 200.0, 300.0]) {
            row.strike = strike;
        }
        app.derivatives_chain.install(chain, Some(256_265), None);
        assert_eq!(app.derivatives_chain.display_row(), 1, "median, no spot");

        app.on_ws(TickerEvent::Ticks(vec![{
            let mut value = Tick::default();
            value.instrument_token = 256_265;
            value.last_price = Some(100.0);
            value
        }]));
        app.on_event(AppEvent::Timer);
        assert_eq!(app.derivatives_chain.display_row(), 0, "recentred on ATM");
    }

    #[test]
    fn chain_click_selects_the_strike_and_focuses_the_leg_it_landed_on() {
        let mut app = test_app();
        app.set_pane(PaneId::DerivativesChain);
        app.set_chain_for_test(test_chain());
        render(&app, W, H);
        let band = |want: ui::ChainBand| {
            region_of(&app, |action| {
                matches!(
                    action,
                    ui::HitAction::Rows {
                        list: ui::HitList::Chain { band: got, .. },
                        ..
                    } if got == want
                )
            })
        };
        let calls = band(ui::ChainBand::Call).rect;
        let puts = band(ui::ChainBand::Put).rect;
        assert!(calls.right() <= puts.x, "the calls sit left of the puts");

        click(&mut app, W, H, puts.x + 1, puts.y + 2);
        assert!(
            !app.derivatives_chain.call_focus(),
            "clicking a put focuses the put leg"
        );
        assert_eq!(app.pane_row(PaneId::DerivativesChain), 2);

        click(&mut app, W, H, calls.x + 1, calls.y);
        assert!(
            app.derivatives_chain.call_focus(),
            "clicking a call focuses the call leg"
        );
        assert_eq!(app.pane_row(PaneId::DerivativesChain), 0);
    }

    #[tokio::test]
    async fn a_ladder_rebuild_retires_the_chain_row_targets() {
        use super::super::super::test_support::press_at;
        use crate::tui::data::DataEvent;
        use crate::tui::instruments::InstrumentStore;
        use std::sync::Arc;
        let option = |strike: f64, kind: &str, token: u32| {
            let mut inst = chain_inst(&format!("NIFTY{strike}{kind}"), "NIFTY", "NFO-OPT");
            inst.instrument_token = token;
            inst.instrument_type = kind.into();
            inst.strike = Some(strike);
            inst.expiry = Some("2099-12-30".into());
            inst
        };
        let mut app = test_app();
        app.set_pane(PaneId::DerivativesChain);
        app.set_chain_for_test(test_chain());
        app.set_selection(4);
        render(&app, W, H);
        let calls = region_of(&app, |action| {
            matches!(
                action,
                ui::HitAction::Rows {
                    list: ui::HitList::Chain {
                        band: ui::ChainBand::Call,
                        ..
                    },
                    ..
                }
            )
        })
        .rect;

        let master = [100.0, 200.0, 300.0]
            .into_iter()
            .zip(0u32..)
            .flat_map(|(strike, i)| [option(strike, "CE", 10 + i), option(strike, "PE", 20 + i)])
            .collect();
        app.on_data(DataEvent::Instruments(Ok(Arc::new(InstrumentStore::new(
            master,
        )))));
        assert_eq!(
            app.derivatives_chain.chain().map(|chain| chain.rows.len()),
            Some(3)
        );
        let selected = app.pane_row(PaneId::DerivativesChain);
        press_at(&mut app, calls.x + 1, calls.y);
        assert_eq!(
            app.pane_row(PaneId::DerivativesChain),
            selected,
            "the click was aimed at strike 24000, which is gone"
        );

        render(&app, W, H);
        app.on_ws(TickerEvent::Ticks(vec![{
            let mut value = Tick::default();
            value.instrument_token = 10;
            value.last_price = Some(5.0);
            value
        }]));
        press_at(&mut app, calls.x + 1, calls.y);
        assert_eq!(app.pane_row(PaneId::DerivativesChain), 0);
    }

    #[test]
    fn chain_click_on_a_strike_selects_without_changing_the_leg() {
        let mut app = test_app();
        app.set_pane(PaneId::DerivativesChain);
        app.set_chain_for_test(test_chain());
        app.derivatives_chain.set_call_focus(true);
        render(&app, W, H);
        let strike = region_of(&app, |action| {
            matches!(
                action,
                ui::HitAction::Rows {
                    list: ui::HitList::Chain {
                        band: ui::ChainBand::Strike,
                        ..
                    },
                    ..
                }
            )
        });
        click(&mut app, W, H, strike.rect.x, strike.rect.y + 1);
        assert!(app.derivatives_chain.call_focus());
        assert_eq!(app.pane_row(PaneId::DerivativesChain), 1);
    }

    #[test]
    fn chain_a_sets_an_alert_on_the_focused_leg() {
        for (call_focus, tradingsymbol, token) in [
            (true, "NIFTY24100CE", 1001_u32),
            (false, "NIFTY24100PE", 2001),
        ] {
            let mut app = test_app();
            app.set_pane(PaneId::DerivativesChain);
            app.set_chain_for_test(test_chain());
            app.derivatives_chain.select(1);
            app.derivatives_chain.set_call_focus(call_focus);

            app.on_key(char_key('a'));
            let Some(Modal::Alert(state)) = app.modals.last() else {
                panic!("a must open the alert form for the focused leg");
            };
            assert_eq!(state.tradingsymbol, tradingsymbol, "{call_focus}");
            assert_eq!(state.exchange, "NFO", "{call_focus}");
            assert_eq!(state.instrument_token, token, "{call_focus}");
            assert!(!state.is_edit(), "a creates, it does not edit");
        }
    }

    #[test]
    fn chain_footer_hint_sets_an_alert_on_the_focused_leg() {
        let mut app = test_app();
        app.set_pane(PaneId::DerivativesChain);
        app.set_chain_for_test(test_chain());
        app.derivatives_chain.select(0);
        app.derivatives_chain.set_call_focus(false);

        let x = text_at(&app, W, H, H - 1, "a:alert");
        click(&mut app, W, H, x, H - 1);
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("`a:alert` must open the alert form");
        };
        assert_eq!(state.tradingsymbol, "NIFTY24000PE");
    }

    #[test]
    fn chain_a_reports_a_strike_with_no_leg_on_the_focused_side() {
        let mut app = test_app();
        app.set_pane(PaneId::DerivativesChain);
        let mut chain = test_chain();
        chain.rows[0].put = None;
        app.set_chain_for_test(chain);
        app.derivatives_chain.select(0);
        app.derivatives_chain.set_call_focus(false);

        app.on_key(char_key('a'));
        assert!(app.modals.is_empty(), "no leg, no form");
        assert!(
            app.visible_toast()
                .is_some_and(|toast| toast.text.contains("no put")),
            "the missing side is named"
        );
    }

    #[test]
    fn underlying_name_of_derives_from_row_kind() {
        let opt = chain_inst("INFY26JUL1600CE", "INFY", "NFO-OPT");
        assert_eq!(underlying_name_of(&opt), "INFY");
        let fut = chain_inst("INFY26JULFUT", "INFY", "NFO-FUT");
        assert_eq!(underlying_name_of(&fut), "INFY");

        let eq = chain_inst("INFY", "INFOSYS LTD", "NSE");
        assert_eq!(underlying_name_of(&eq), "INFY");

        let bare = chain_inst("WEIRDFUT", "", "NFO-FUT");
        assert_eq!(underlying_name_of(&bare), "WEIRDFUT");
    }
}
