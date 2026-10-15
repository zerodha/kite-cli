use super::test_support::{
    H, W, ato_alert, char_key, char_key_code, render, server_alert, test_app,
};
use super::{Modal, SearchState};
use crate::tui::data::{AlertListSequence, DataEvent};
use crate::tui::event::AppEvent;
use crate::tui::ticket::PendingAction;
use crate::tui::workspace::PaneId;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use kiteconnect::AlertStatus;

#[test]
fn paste_into_search_cannot_execute_a_second_line() {
    let mut app = test_app();
    app.modals.push(Modal::Search(SearchState::default()));
    app.on_event(AppEvent::Paste("INFY\r\nq\n\u{1b}".into()));
    let Some(Modal::Search(search)) = app.modals.last() else {
        panic!("search closed")
    };
    assert_eq!(search.input, "INFY");
    assert!(!app.should_quit);
    app.on_key(KeyEvent::new(
        KeyCode::Char('€'),
        KeyModifiers::CONTROL | KeyModifiers::ALT,
    ));
    app.on_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL));
    let Some(Modal::Search(search)) = app.modals.last() else {
        panic!("search closed")
    };
    assert_eq!(search.input, "INFY");
    app.on_key(char_key_code(KeyCode::Enter));
    assert!(
        matches!(app.modals.last(), Some(Modal::Search(_))),
        "missing master must retain search"
    );
}

#[test]
fn pasted_note_keeps_lines_and_expands_tabs_without_quitting() {
    let mut app = test_app();
    app.ephemeral = true;
    app.on_key(char_key('n'));
    app.on_key(char_key('n'));
    app.on_event(AppEvent::Paste("q\r\n\t👩🏽‍💻\u{1b}".into()));
    app.commit_open_editor();
    assert_eq!(app.notes.newest_first()[0].body, "q\n    👩🏽‍💻");
    assert!(!app.should_quit);
}

#[tokio::test]
async fn older_alert_snapshots_cannot_resurrect_deleted_rows() {
    let mut app = test_app();
    let alert = server_alert("INFY", "NSE", ">=", 100.0);
    app.on_data(DataEvent::Alerts {
        sequence: AlertListSequence(1),
        result: Ok(vec![alert.clone()]),
    });
    app.on_data(DataEvent::Alerts {
        sequence: AlertListSequence(3),
        result: Ok(vec![]),
    });
    app.on_data(DataEvent::Alerts {
        sequence: AlertListSequence(2),
        result: Ok(vec![alert]),
    });
    assert!(app.alert_list().is_empty());
    assert_eq!(app.alerts_applied, AlertListSequence(3));
}

#[tokio::test]
async fn enabling_ato_requires_a_rendered_basket_review() {
    let mut app = test_app();
    let mut alert = ato_alert();
    alert.status = AlertStatus::Disabled;
    app.set_alerts(vec![alert]);
    app.show_pane(PaneId::OrdersAlerts);
    app.on_key(char_key('r'));
    let Some(Modal::Confirm(review)) = app.modals.last() else {
        panic!("missing ATO review")
    };
    assert!(matches!(
        review.action,
        PendingAction::SetAlertStatus { .. }
    ));
    assert!(
        review
            .lines
            .iter()
            .any(|(label, _)| label.contains("GOLDBEES"))
    );
    assert!(!app.pending_action);
    app.on_event(AppEvent::Paste("y\n".into()));
    app.on_key(char_key_code(KeyCode::Enter));
    assert!(!app.pending_action, "unseen review cannot send");
    app.on_key(char_key_code(KeyCode::Esc));
    assert!(app.modals.is_empty());
}

#[tokio::test]
async fn changing_an_ato_basket_requires_a_discard_decision() {
    let mut app = test_app();
    app.open_alert_edit(&ato_alert());
    let Some(Modal::Alert(form)) = app.modals.last_mut() else {
        panic!("missing form")
    };
    form.draft.remove(0);
    app.on_key(char_key_code(KeyCode::Esc));
    assert!(
        matches!(app.modals.last(), Some(Modal::Confirm(review)) if matches!(review.action, PendingAction::DiscardAlertDraft))
    );
    app.on_key(char_key('n'));
    assert!(matches!(app.modals.last(), Some(Modal::Alert(form)) if form.draft.is_empty()));
    app.on_key(char_key_code(KeyCode::Esc));
    render(&app, W, H);
    app.on_key(char_key('y'));
    assert!(app.modals.is_empty());
}

#[tokio::test]
async fn an_inflight_alert_status_change_blocks_a_second_toggle() {
    let mut app = test_app();
    let alert = server_alert("INFY", "NSE", ">=", 100.0);
    let uuid = alert.uuid.clone();
    app.set_alerts(vec![alert]);
    app.show_pane(PaneId::OrdersAlerts);
    app.on_key(char_key('r'));
    assert!(app.alert_status_pending.contains(&uuid));
    app.on_key(char_key('r'));
    assert!(
        app.toasts()
            .iter()
            .any(|toast| toast.text.contains("already being updated"))
    );
    app.on_data(DataEvent::AlertStatusSet {
        uuid,
        status: AlertStatus::Disabled,
        confirmed: false,
        result: Ok("INFY".into()),
    });
    assert_eq!(app.alert_list()[0].status, AlertStatus::Disabled);
    assert!(app.alert_status_pending.is_empty());
}

#[tokio::test]
async fn reads_started_before_an_alert_write_cannot_undo_the_status() {
    let mut app = test_app();
    let alert = server_alert("INFY", "NSE", ">=", 100.0);
    let uuid = alert.uuid.clone();
    app.set_alerts(vec![alert.clone()]);
    app.alert_fetch_requests
        .store(4, std::sync::atomic::Ordering::SeqCst);
    app.on_data(DataEvent::AlertStatusSet {
        uuid,
        status: AlertStatus::Disabled,
        confirmed: false,
        result: Ok("INFY".into()),
    });
    app.on_data(DataEvent::Alerts {
        sequence: AlertListSequence(4),
        result: Ok(vec![alert]),
    });
    assert_eq!(app.alert_list()[0].status, AlertStatus::Disabled);
}

#[tokio::test]
async fn an_mtf_only_holding_seeds_its_own_product_and_quantity() {
    use super::test_support::{holding, master_inst};
    use crate::tui::instruments::InstrumentStore;
    let mut app = test_app();
    let mut row = holding("INFY", 408_065, 100.0, 110.0, 100.0, 0);
    row.mtf = Some(serde_json::from_value(serde_json::json!({"quantity":50,"used_quantity":0,"average_price":100.0,"value":5000.0,"initial_margin":2500.0})).unwrap());
    app.instruments = Some(std::sync::Arc::new(InstrumentStore::new(vec![
        master_inst("NSE", "NSE", "INFY", 408_065),
    ])));
    app.on_data(DataEvent::Holdings(Ok(vec![row])));
    app.show_pane(PaneId::PortfolioHoldings);
    for key in ['s', 'g'] {
        app.on_key(char_key(key));
        let Some(Modal::Ticket(ticket)) = app.modals.last() else {
            panic!("missing ticket")
        };
        assert_eq!(ticket.qty, "50");
        assert_eq!(super::PRODUCTS[ticket.product], "MTF");
        app.on_key(char_key_code(KeyCode::Esc));
    }
}

#[test]
fn an_unavailable_watchlist_catalog_never_implies_a_missing_name() {
    let state = crate::tui::marketwatch::State::remote(None, "AB1234".into(), None);
    assert!(!state.catalog_available());
    assert!(state.resolve("Core").is_err());
}

#[test]
fn duplicate_watchlist_names_have_distinct_id_actions() {
    use crate::tui::marketwatch::{Catalog, Section, Summary, WatchId};
    use crate::tui::palette::WatchlistCmd;
    let mut app = test_app();
    app.watch.set_catalog(Ok(Catalog {
        sections: vec![Section {
            items: vec![
                Summary {
                    id: WatchId::from("W1"),
                    name: "Core".into(),
                    is_static: false,
                },
                Summary {
                    id: WatchId::from("W2"),
                    name: "Core".into(),
                    is_static: false,
                },
            ],
        }],
    }));
    app.open_watchlist_manager();
    let Some(Modal::Palette(palette)) = app.modals.last() else {
        panic!("missing palette")
    };
    let ids: Vec<_> = palette
        .results
        .iter()
        .filter_map(|row| {
            let super::PaletteRow::Item {
                action: super::PaletteAction::Execute { item, .. },
                ..
            } = row
            else {
                return None;
            };
            let super::ExecItem::Watchlist(WatchlistCmd::SwitchId { id, .. }) = item.as_ref()
            else {
                return None;
            };
            Some(id.as_str())
        })
        .collect();
    assert_eq!(ids, ["W1", "W2"]);
}

#[tokio::test]
async fn a_fresh_buy_gtt_seeds_the_limit_at_the_trigger() {
    use super::test_support::{ltp_tick, master_inst};
    use crate::tui::instruments::InstrumentStore;
    let mut app = test_app();
    app.instruments = Some(std::sync::Arc::new(InstrumentStore::new(vec![
        master_inst("NSE", "NSE", "INFY", 408_065),
    ])));
    app.on_ws(kiteconnect::ticker::TickerEvent::Ticks(vec![ltp_tick(
        408_065, 100.0,
    )]));
    app.open_gtt_ticket(
        super::Side::Buy,
        "NSE".into(),
        "INFY".into(),
        408_065,
        1,
        Some("CNC"),
        None,
        None,
        None,
    );
    let Some(Modal::Ticket(ticket)) = app.modals.last() else {
        panic!("missing ticket")
    };
    assert_eq!(ticket.price, ticket.trigger);
    assert_eq!(ticket.price, "105.00");
}
