use crate::tui::app::App;
use crate::tui::data::{Holding, Position};
use crate::tui::notes::Pad;
use crate::tui::settings::Settings;
use crate::tui::workspace::{PaneId, Preset};
use kiteconnect::KiteConnect;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

pub(super) fn test_app() -> App {
    let client = KiteConnect::builder("test-key")
        .access_token(kiteconnect::AccessToken::new("tok").unwrap())
        .build()
        .expect("dummy client builds");
    let (events_tx, _events_rx) = tokio::sync::mpsc::channel(8);
    App::new(
        client,
        events_tx,
        crate::tui::marketwatch::State::test_loaded(Vec::new()),
        Pad::default(),
        false,
        &Settings::default(),
    )
}

pub(super) fn position(symbol: &str) -> Position {
    {
        let mut row = crate::tui::data::test_models::position();
        row.tradingsymbol = symbol.into();
        row.exchange = "NSE".into();
        row.instrument_token = 1;
        row.product = "MIS".into();
        row.overnight_quantity = 0;
        row.day_buy_quantity = 0;
        row.day_sell_quantity = 0;
        row.quantity = 10;
        row.average_price = 100.0;
        row.last_price = 105.0;
        row.close_price = 100.0;
        row.pnl = 50.0;
        row.realised = 0.0;
        row.multiplier = 1.0;
        row
    }
}

pub(super) fn pledged_holding(symbol: &str, free_qty: i64, collateral: i64) -> Holding {
    {
        let mut row = crate::tui::data::test_models::holding();
        row.tradingsymbol = symbol.into();
        row.exchange = "NSE".into();
        row.instrument_token = 1;
        row.quantity = free_qty;
        row.t1_quantity = 0;
        row.collateral_quantity = collateral;
        row.mtf = None;
        row.average_price = 100.0;
        row.last_price = 100.0;
        row.close_price = 100.0;
        row.day_change = None;
        row.discrepancy = false;
        row
    }
}

pub(super) fn book_app(pane: PaneId) -> App {
    let mut app = test_app();
    app.set_workspace(Preset::Overview);
    app.workspaces.focus(Preset::Overview, pane);
    app.positions = Some({
        let mut row = crate::tui::data::test_models::positions();
        row.net = vec![position("P0"), position("P1")];
        row
    });
    app.holdings = Some(
        (0u32..3)
            .map(|i| {
                let mut h = pledged_holding(&format!("H{i}"), 1, 0);
                h.last_price = 110.0;
                h
            })
            .collect(),
    );
    app
}

pub(super) fn root_buffer(app: &App, width: u16, height: u16) -> ratatui::buffer::Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, app))
        .unwrap();
    terminal.backend().buffer().clone()
}

pub(super) fn buffer_rows(buffer: &ratatui::buffer::Buffer) -> Vec<String> {
    let area = buffer.area;
    (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect()
}

pub(super) fn find_all(rows: &[String], needle: &str) -> Vec<(u16, u16)> {
    rows.iter()
        .enumerate()
        .filter_map(|(y, row)| {
            row.find(needle).map(|byte| {
                (
                    u16::try_from(y).unwrap(),
                    u16::try_from(row[..byte].chars().count()).unwrap(),
                )
            })
        })
        .collect()
}

pub(super) fn find_once(rows: &[String], needle: &str) -> (u16, u16) {
    let found = find_all(rows, needle);
    assert_eq!(
        found.len(),
        1,
        "{needle:?} is not on exactly one row of\n{}",
        rows.join("\n")
    );
    found[0]
}
