use super::table::{TableRows, right, selectable_table};
use super::{bid_kind_cell, bid_status_cell, bid_symbol_cell, offer_price};
use crate::tui::bids::{self, Section as BidsSection};
use crate::tui::format;
use crate::tui::ui;
use crate::tui::ui::columns::{
    self, FitCol, col, fit_constraints, fit_header, fit_row, table_budget,
};
use crate::tui::ui::pane::{PaneHits, PlainList};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Table;

const CORPORATE_ACTIONS_COLS: [FitCol; 6] = [
    col(9, 3),  // type
    col(12, 0), // symbol
    col(13, 4), // starts
    col(13, 1), // ends
    col(11, 2), // offer price
    col(10, 5), // status
];
const CORPORATE_ACTIONS_LABELS: [(&str, bool); 6] = [
    ("TYPE", false),
    ("SYMBOL", false),
    ("STARTS", true),
    ("ENDS", true),
    ("OFFER", true),
    ("STATUS", true),
];
const CORPORATE_ACTIONS_SYM_MAX: u16 = 24;

pub(super) fn draw_corporate_action_table(
    frame: &mut Frame,
    area: Rect,
    rows: &[bids::Instrument],
    section: BidsSection,
    list: &PlainList<'_>,
    hits: &PaneHits<'_>,
) {
    let PlainList {
        selected,
        focused,
        viewport,
    } = *list;

    let fit = columns::fit_columns(table_budget(area.width), 1, &CORPORATE_ACTIONS_COLS);
    let layout = columns::spread(
        &fit,
        &CORPORATE_ACTIONS_COLS,
        &[columns::text(
            1,
            columns::widest(rows.iter().map(|row| {
                format::display_width(row.label())
                    + if row.kind == "ofs" && !row.series.is_empty() {
                        1 + format::display_width(&row.series)
                    } else {
                        0
                    }
            })),
            CORPORATE_ACTIONS_SYM_MAX,
        )],
    );
    let offer_width = layout.width(4) as usize;
    let table_rows = rows
        .iter()
        .map(|row| {
            let orders = row.orders.as_deref().unwrap_or_default();
            let live = orders.iter().filter(|order| order.is_live()).count();
            let current = orders
                .iter()
                .find(|order| order.is_live())
                .or_else(|| orders.first());
            let status = match current {
                Some(order) if live > 1 && row.kind == "ofs" => {
                    format!("{} ({live})", order.status)
                }
                Some(order) => order.status.clone(),
                None => String::new(),
            };
            let series =
                (row.kind == "ofs" && !row.series.is_empty()).then_some(row.series.as_str());
            fit_row(
                &fit,
                &[
                    &|| bid_kind_cell(&row.kind),
                    &|| bid_symbol_cell(row.label(), series),
                    &|| right(format::day_month_time(&row.start_at)),
                    &|| right(format::day_month_time(&row.end_at)),
                    &|| right(offer_price(row.min_price, offer_width)),
                    &|| {
                        bid_status_cell(
                            &status,
                            current.is_some_and(crate::tui::bids::BidOrder::is_live),
                        )
                    },
                ],
            )
        })
        .collect::<Vec<_>>();
    let table = Table::new(table_rows, fit_constraints(&fit, &layout))
        .header(fit_header(&fit, &CORPORATE_ACTIONS_LABELS))
        .column_spacing(layout.spacing)
        .block(ui::focus_panel(section.title(), focused));
    selectable_table(
        frame,
        area,
        table,
        &layout,
        TableRows {
            selected,
            count: rows.len(),
            viewport,
        },
        hits,
    );
}

#[cfg(test)]
mod corporate_action_render_tests {
    use super::super::draw_bids;
    use crate::tui::app::App;
    use crate::tui::bids::{BidOrder, Instrument, Section};
    use crate::tui::notes::Pad;
    use crate::tui::settings::Settings;
    use crate::tui::workspace::PaneId;
    use kiteconnect::KiteConnect;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use tokio::sync::mpsc;

    fn app() -> App {
        let client = KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("tok").unwrap())
            .build()
            .unwrap();
        let (events_tx, _events_rx) = mpsc::channel(8);
        let mut app = App::new(
            client,
            events_tx,
            crate::tui::marketwatch::State::test_loaded(Vec::new()),
            Pad::default(),
            false,
            &Settings::default(),
        );
        app.set_pane(PaneId::InvestBids);
        app
    }

    fn rendered(app: &App) -> String {
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
        terminal
            .draw(|frame| draw_bids(frame, frame.area(), app))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect()
    }

    #[test]
    fn corporate_action_table_shows_window_and_offer() {
        let mut app = app();
        app.bids_pane.set_section(Section::CorporateActions);
        app.bids.corporate_actions = Some(vec![Instrument {
            tradingsymbol: "SHANKARA".into(),
            kind: "takeover".into(),
            start_at: "2026-09-07 08:00:00".into(),
            end_at: "2026-09-21 13:00:00".into(),
            min_price: 150.0,
            ..Default::default()
        }]);
        let rows = rendered(&app);
        assert!(rows.contains("TAKEOVER"), "{rows}");
        assert!(rows.contains("SHANKARA"), "{rows}");
        assert!(rows.contains("7 Sep 08:00"), "{rows}");
        assert!(rows.contains("21 Sep 13:00"), "{rows}");
        assert!(rows.contains("150.00"), "{rows}");
    }

    #[test]
    fn a_large_offer_price_shortens_rather_than_lose_leading_digits() {
        let mut app = app();
        app.bids_pane.set_section(Section::CorporateActions);
        app.bids.corporate_actions = Some(vec![Instrument {
            tradingsymbol: "SHANKARA".into(),
            kind: "buyback".into(),
            start_at: "2026-09-07 08:00:00".into(),
            end_at: "2026-09-21 13:00:00".into(),
            min_price: 1_234_567.89,
            ..Default::default()
        }]);
        let rows = rendered(&app);
        assert!(rows.contains("12.35L"), "{rows}");
        assert!(!rows.contains("2,34,567.89"), "{rows}");
    }

    #[test]
    fn a_cancelled_first_order_does_not_hide_a_live_one() {
        let mut app = app();
        app.bids_pane.set_section(Section::CorporateActions);
        app.bids.corporate_actions = Some(vec![Instrument {
            tradingsymbol: "SHANKARA".into(),
            kind: "ofs".into(),
            series: "EQ".into(),
            start_at: "2026-09-07 08:00:00".into(),
            end_at: "2026-09-21 13:00:00".into(),
            min_price: 150.0,
            orders: Some(vec![
                BidOrder {
                    status: "cancelled".into(),
                    ..Default::default()
                },
                BidOrder {
                    status: "PLACED".into(),
                    ..Default::default()
                },
                BidOrder {
                    status: "PLACED".into(),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        }]);
        let rows = rendered(&app);
        assert!(rows.contains("placed (2)"), "{rows}");
        assert!(!rows.contains("cancelled"), "{rows}");
    }
}
