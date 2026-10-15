use super::table::{TableRows, right, selectable_table};
use super::{bid_kind_cell, bid_status_cell, fit_amount};
use crate::tui::bids::{self, Section as BidsSection};
use crate::tui::format;
use crate::tui::ui;
use crate::tui::ui::columns::{
    self, FitCol, col, fit_constraints, fit_header, fit_row, table_budget,
};
use crate::tui::ui::pane::{PaneHits, PlainList};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Cell, Table};

const GSEC_COLS: [FitCol; 6] = [
    col(9, 3),  // type
    col(16, 0), // instrument
    col(7, 1),  // yield
    col(7, 2),  // ends
    col(13, 4), // order value
    col(9, 5),  // status
];
const GSEC_LABELS: [(&str, bool); 6] = [
    ("TYPE", false),
    ("INSTRUMENT", false),
    ("YIELD", true),
    ("ENDS", true),
    ("ORDER ₹", true),
    ("STATUS", true),
];
const GSEC_NAME_MAX: u16 = 32;

pub(super) fn draw_gsec_table(
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

    let fit = columns::fit_columns(table_budget(area.width), 1, &GSEC_COLS);
    let layout = columns::spread(
        &fit,
        &GSEC_COLS,
        &[columns::text(
            1,
            columns::widest(rows.iter().map(|row| {
                format::display_width(if row.name.is_empty() {
                    row.label()
                } else {
                    &row.name
                })
            })),
            GSEC_NAME_MAX,
        )],
    );
    let name_width = layout.width(1);
    let value_width = layout.width(4) as usize;
    let table_rows = rows
        .iter()
        .map(|row| {
            let name = if row.name.is_empty() {
                row.label()
            } else {
                &row.name
            };
            let order = row.order.as_ref();
            let status = order.map(|order| order.status.as_str()).unwrap_or_default();
            fit_row(
                &fit,
                &[
                    &|| bid_kind_cell(&row.kind),
                    &|| Cell::from(format::pad_or_clip(name, name_width as usize)),
                    &|| {
                        right(if row.indicative_yield > 0.0 {
                            format!("{:.2}%", row.indicative_yield)
                        } else {
                            ".".to_string()
                        })
                    },
                    &|| right(format::day_month(&row.end_at)),
                    &|| {
                        right(order.filter(|order| order.quantity > 0).map_or_else(
                            || ".".to_string(),
                            |order| {
                                #[expect(
                                    clippy::cast_precision_loss,
                                    reason = "minimum amount is display-only f64 arithmetic"
                                )]
                                let value = order.price * order.quantity as f64;
                                fit_amount(value, format::price(value), value_width)
                            },
                        ))
                    },
                    &|| {
                        bid_status_cell(
                            status,
                            order.is_some_and(crate::tui::bids::BidOrder::is_live),
                        )
                    },
                ],
            )
        })
        .collect::<Vec<_>>();
    let table = Table::new(table_rows, fit_constraints(&fit, &layout))
        .header(fit_header(&fit, &GSEC_LABELS))
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
mod gsec_render_tests {
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
        rendered_at(app, 100, 24)
    }

    fn rendered_at(app: &App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
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
    fn gsec_table_keeps_yield_at_narrow_width() {
        let mut app = app();
        app.bids_pane.set_section(Section::Government);
        app.bids.government = Some(vec![Instrument {
            symbol: "657GS2033".into(),
            name: "6.57% GS 2033".into(),
            kind: "gsec".into(),
            end_at: "2026-09-10 17:00:00".into(),
            indicative_yield: 7.25,
            order: Some(BidOrder {
                status: "PLACED".into(),
                price: 101.5,
                quantity: 100,
            }),
            ..Default::default()
        }]);
        let wide = rendered(&app);
        assert!(wide.contains("GSEC"), "{wide}");
        assert!(wide.contains("6.57% GS 2033"), "{wide}");
        assert!(wide.contains("10,150.00"), "{wide}");
        assert!(wide.contains("placed"), "{wide}");
        let narrow = rendered_at(&app, 48, 10);
        assert!(narrow.contains("7.25%"), "{narrow}");
        assert!(narrow.contains("10 Sep"), "{narrow}");
        assert!(!narrow.contains("10,150"), "{narrow}");
    }

    #[test]
    fn a_large_gsec_order_value_shortens_rather_than_lose_leading_digits() {
        let mut app = app();
        app.bids_pane.set_section(Section::Government);
        app.bids.government = Some(vec![Instrument {
            symbol: "657GS2033".into(),
            name: "6.57% GS 2033".into(),
            kind: "gsec".into(),
            end_at: "2026-09-10 17:00:00".into(),
            indicative_yield: 7.25,
            order: Some(BidOrder {
                status: "PLACED".into(),
                price: 100.0,
                quantity: 100_000,
            }),
            ..Default::default()
        }]);
        let rows = rendered(&app);
        assert!(rows.contains("1.00Cr"), "{rows}");
        assert!(!rows.contains(",00,00,000.00"), "{rows}");
    }
}
