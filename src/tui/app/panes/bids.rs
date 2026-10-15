use super::super::App;
use crate::tui::bids::{self, Section as BidsSection};
use crate::tui::data;
use crate::tui::pane_state;
use crossterm::event::{KeyCode, KeyEvent};

impl App {
    pub(in crate::tui::app) fn set_bids_section(&mut self, section: BidsSection) {
        self.bids_pane.set_section(section);
    }

    pub(in crate::tui::app) fn key_bids(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('a') && self.bids_pane.section() == BidsSection::Ipo {
            self.bids_pane.toggle_application_focus();
        } else if key.code == KeyCode::Enter {
            self.open_bids_web();
        } else if key.code == KeyCode::Char('s')
            && self.bids_pane.section() == BidsSection::Auctions
        {
            let row = self.bids_pane.target(&self.bids_row_keys());
            let Some(auction) = row
                .and_then(|row| {
                    self.auctions
                        .as_ref()
                        .and_then(|auctions| auctions.get(row))
                })
                .cloned()
            else {
                return;
            };
            self.open_auction_ticket(auction);
        }
    }

    pub(in crate::tui::app) fn open_bids_web(&mut self) {
        data::spawn_external_link(
            self.events_tx.clone(),
            self.bids_pane.section().web_url().to_string(),
        );
    }

    pub(in crate::tui::app) fn refresh_bids(&mut self) {
        for section in BidsSection::ALL {
            if section != BidsSection::Auctions {
                std::mem::drop(bids::spawn_refresh_instruments(
                    self.client.clone(),
                    self.events_tx.clone(),
                    section,
                ));
            }
        }
        std::mem::drop(bids::spawn_refresh_applications(
            self.client.clone(),
            self.events_tx.clone(),
        ));
    }

    pub(in crate::tui::app) fn bids_row_keys(&self) -> Vec<Option<pane_state::BidsRowId>> {
        use pane_state::BidsRowId;
        let named = |id: BidsRowId, key: &str| (!key.is_empty()).then_some(id);
        match self.bids_pane.section() {
            BidsSection::Auctions => self
                .auctions
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(|auction| {
                    named(
                        BidsRowId::Auction(auction.auction_number.clone()),
                        &auction.auction_number,
                    )
                })
                .collect(),
            BidsSection::Ipo if self.bids_pane.application_focus() => self
                .bids
                .applications
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(|application| {
                    named(
                        BidsRowId::Application {
                            symbol: application.symbol.clone(),
                            created_at: application.created_at.clone(),
                        },
                        &application.symbol,
                    )
                })
                .collect(),
            section => self
                .bids
                .instruments(section)
                .unwrap_or_default()
                .iter()
                .map(|instrument| {
                    named(
                        BidsRowId::Instrument(instrument.label().to_string()),
                        instrument.label(),
                    )
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::test_support::{H, W, holding, press_at, region_of, render, test_app};
    use super::super::super::ui;
    use crate::tui::bids::{self, Section as BidsSection};
    use crate::tui::data::DataEvent;
    use crate::tui::workspace::PaneId;

    fn first_row(app: &super::App, applications: bool) -> ratatui::layout::Rect {
        region_of(app, |action| match action {
            ui::HitAction::Rows { list, .. } => {
                matches!(list, ui::HitList::BidsApplications(_)) == applications
            }
            _ => false,
        })
        .rect
    }

    #[test]
    fn a_bids_refresh_that_replaces_a_drawn_record_retires_its_row_target() {
        let issue = |symbol: &str| bids::Instrument {
            symbol: symbol.into(),
            ..Default::default()
        };
        for section in [
            BidsSection::Ipo,
            BidsSection::Government,
            BidsSection::CorporateActions,
            BidsSection::Sse,
        ] {
            let mut app = test_app();
            app.set_pane(PaneId::InvestBids);
            app.set_bids_section(section);
            app.bids
                .set_instruments(section, Ok(vec![issue("AAA"), issue("ZZZ")]));
            app.set_selection(1);
            render(&app, W, H);
            let row = first_row(&app, false);

            app.on_data(DataEvent::Bids(bids::Event::Instruments {
                section,
                result: Ok(vec![issue("BBB"), issue("ZZZ")]),
            }));
            let selected = app.pane_row(PaneId::InvestBids);
            press_at(&mut app, row.x + 2, row.y);
            assert_eq!(
                app.pane_row(PaneId::InvestBids),
                selected,
                "{section:?}: the click was aimed at AAA, which is gone"
            );
        }

        let application = |symbol: &str| bids::Application {
            symbol: symbol.into(),
            created_at: "2026-09-01".into(),
            ..Default::default()
        };
        let mut app = test_app();
        app.set_pane(PaneId::InvestBids);
        app.bids_pane.focus_applications();
        app.bids
            .set_applications(Ok(vec![application("AAA"), application("ZZZ")]));
        app.set_selection(1);
        render(&app, W, H);
        let row = first_row(&app, true);

        app.on_data(DataEvent::Bids(bids::Event::Applications(Ok(vec![
            application("BBB"),
            application("ZZZ"),
        ]))));
        let selected = app.pane_row(PaneId::InvestBids);
        press_at(&mut app, row.x + 2, row.y);
        assert_eq!(
            app.pane_row(PaneId::InvestBids),
            selected,
            "the click was aimed at the AAA application, which is gone"
        );

        render(&app, W, H);
        let mut updated = vec![application("BBB"), application("ZZZ")];
        updated[0].status = "failed".into();
        app.on_data(DataEvent::Bids(bids::Event::Applications(Ok(updated))));
        press_at(&mut app, row.x + 2, row.y);
        assert_eq!(app.pane_row(PaneId::InvestBids), 0);
    }

    #[test]
    fn an_auctions_refresh_that_replaces_a_drawn_record_retires_its_row_target() {
        let auction = |symbol: &str, number: &str| {
            let mut row = crate::tui::data::test_models::auction();
            let holding = holding(symbol, 1, 100.0, 110.0, 100.0, 1);
            row.tradingsymbol = holding.tradingsymbol;
            row.exchange = holding.exchange;
            row.instrument_token = holding.instrument_token;
            row.quantity = holding.quantity;
            row.t1_quantity = holding.t1_quantity;
            row.collateral_quantity = holding.collateral_quantity;
            row.average_price = holding.average_price;
            row.last_price = holding.last_price;
            row.close_price = holding.close_price;
            row.discrepancy = holding.discrepancy;
            row.auction_number = number.into();
            row
        };
        let mut app = test_app();
        app.set_pane(PaneId::InvestBids);
        app.set_bids_section(BidsSection::Auctions);
        app.on_data(DataEvent::Auctions(Ok(vec![
            auction("AAA", "AUC-1"),
            auction("ZZZ", "AUC-9"),
        ])));
        app.set_selection(1);
        render(&app, W, H);
        let row = first_row(&app, false);

        app.on_data(DataEvent::Auctions(Ok(vec![
            auction("BBB", "AUC-2"),
            auction("ZZZ", "AUC-9"),
        ])));
        let selected = app.pane_row(PaneId::InvestBids);
        press_at(&mut app, row.x + 2, row.y);
        assert_eq!(
            app.pane_row(PaneId::InvestBids),
            selected,
            "the click was aimed at AUC-1, which is gone"
        );

        render(&app, W, H);
        let mut repriced = vec![auction("BBB", "AUC-2"), auction("ZZZ", "AUC-9")];
        repriced[0].last_price = 120.0;
        app.on_data(DataEvent::Auctions(Ok(repriced)));
        press_at(&mut app, row.x + 2, row.y);
        assert_eq!(app.pane_row(PaneId::InvestBids), 0);
    }

    #[test]
    fn bids_external_link_completion_keeps_success_and_failure_actionable() {
        let mut app = test_app();
        let url = "https://kite.zerodha.com/bids/ipo";
        app.on_data(DataEvent::ExternalLink(Ok(url.into())));
        assert!(app.toasts().back().is_some_and(|toast| {
            toast
                .text
                .contains("browser launch started: https://kite.zerodha.com/bids/ipo")
        }));

        app.on_data(DataEvent::ExternalLink(Err(format!(
            "could not start xdg-open. Open this URL manually: {url}"
        ))));
        assert!(
            app.toasts()
                .back()
                .is_some_and(|toast| toast.text.contains("Open this URL manually"))
        );
    }
}
