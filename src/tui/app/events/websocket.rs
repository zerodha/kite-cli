use super::super::charts::{candle_bucket, candle_tick_epoch, now_epoch};
use super::super::{App, FLASH_TTL, ToastLevel};
use crate::tui::data;
use crate::tui::event::WsState;
use crate::tui::format;
use crate::tui::ticks;
use kiteconnect::ticker::{CloseReason, Tick, TickerEvent};
use std::collections::hash_map::Entry;
use std::time::{Duration, Instant};

impl App {
    pub(in crate::tui::app) fn on_ws(&mut self, event: TickerEvent) {
        match event {
            TickerEvent::Ticks(ticks) => {
                for tick in ticks {
                    self.on_tick(tick);
                }
            }
            TickerEvent::Connected => {
                self.ws_state = WsState::Connected;
                self.circuits_inflight.clear();
                self.ensure_circuits();
            }
            TickerEvent::Reconnecting { reason, .. } => {
                self.ws_state = WsState::Reconnecting;
                self.toast_error(reason);
            }
            TickerEvent::OrderUpdate(value) => {
                self.toast(
                    ToastLevel::Info,
                    format!("order update: {} {}", value.tradingsymbol, value.status),
                );

                let now = Instant::now();
                let recent = self
                    .last_postback_refresh
                    .is_some_and(|at| now.duration_since(at) < Duration::from_secs(2));
                if !recent {
                    self.last_postback_refresh = Some(now);
                    data::spawn_refresh_orders(self.client.clone(), self.events_tx.clone());
                    data::spawn_refresh_positions(self.client.clone(), self.events_tx.clone());
                }
            }
            TickerEvent::InvalidOrderUpdate { error, .. } => {
                self.toast_error(format!("order update could not be decoded: {error}"));
            }
            TickerEvent::Closed(reason) => {
                self.ws_state = WsState::Closed;
                match reason {
                    CloseReason::AuthRejected(_) => self.expire_session(),

                    reason if self.ws_fatal.is_none() => {
                        let message = reason.to_string();
                        self.ws_fatal = Some(message.clone());
                        self.toast(ToastLevel::Error, message);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn on_tick(&mut self, tick: Tick) {
        let token = tick.instrument_token;

        let wall_epoch = now_epoch();
        let tick_epoch = candle_tick_epoch(&tick, wall_epoch);
        let cached = match self.ticks.entry(token) {
            Entry::Occupied(mut entry) => {
                let existing = entry.get_mut();
                #[expect(
                    clippy::float_cmp,
                    reason = "any changed quote must flash, even below an epsilon"
                )]
                if let (Some(old), Some(new)) = (existing.last_price, tick.last_price)
                    && old != new
                    && new != 0.0
                {
                    let dir = if new > old { 1 } else { -1 };
                    self.flash.insert(token, (Instant::now(), dir));
                }
                ticks::merge(existing, tick);
                entry.into_mut()
            }
            Entry::Vacant(entry) => {
                let mut t = tick;
                ticks::discard_zero_prices(&mut t);
                entry.insert(t)
            }
        };
        let (last_price, vol_cum) = (cached.last_price, cached.volume);

        let session_open = self
            .instruments
            .as_ref()
            .and_then(|store| store.by_token(token))
            .is_some_and(|instrument| {
                format::exchange_session_open(&instrument.exchange, tick_epoch)
            });

        if let Some(price) = last_price
            && session_open
        {
            self.candles
                .fold_tick(token, price, vol_cum, |interval, last_epoch| {
                    candle_bucket(last_epoch, tick_epoch, interval)
                });
        }
    }

    pub fn tick(&self, token: u32) -> Option<&Tick> {
        self.ticks.get(&token)
    }

    pub fn live_ltp(&self, token: u32) -> Option<f64> {
        self.ticks.get(&token).and_then(|tick| tick.last_price)
    }

    pub fn flash_dir(&self, token: u32) -> Option<i8> {
        self.flash
            .get(&token)
            .filter(|(at, _)| at.elapsed() < FLASH_TTL)
            .map(|(_, dir)| *dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::test_support::test_app;

    #[test]
    fn a_non_auth_fatal_close_refuses_trading_with_its_actual_reason() {
        let mut app = test_app();
        app.on_ws(TickerEvent::Closed(CloseReason::HandshakeRejected(
            "unsupported ticker protocol".to_string(),
        )));
        assert_eq!(app.ws_fatal.as_deref(), Some("unsupported ticker protocol"));
        assert!(app.trading_disabled());
        assert_eq!(
            app.toasts().back().unwrap().text,
            "unsupported ticker protocol"
        );
    }

    #[test]
    fn an_auth_fatal_close_keeps_the_auth_expired_refusal() {
        let mut app = test_app();
        app.on_ws(TickerEvent::Closed(CloseReason::AuthRejected(
            "TokenException".to_string(),
        )));
        assert!(app.trading_disabled());
        assert_eq!(
            app.toasts().back().unwrap().text,
            "auth expired, run `kite auth login`"
        );
    }
}
