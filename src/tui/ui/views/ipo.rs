use super::table::{TableRows, right, selectable_table};
use super::{bid_status_cell, bid_symbol_cell, fit_amount};
use crate::tui::app::App;
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

const IPO_COLS: [FitCol; 7] = [
    col(14, 0), // instrument
    col(26, 6), // name
    col(13, 2), // dates
    col(11, 1), // price band
    col(10, 3), // min amount
    col(6, 5),  // lot qty
    col(9, 4),  // status
];
const IPO_LABELS: [(&str, bool); 7] = [
    ("INSTRUMENT", false),
    ("NAME", false),
    ("DATES", true),
    ("PRICE", true),
    ("MIN AMT", true),
    ("QTY", true),
    ("STATUS", true),
];
const IPO_SYM_MAX: u16 = 24;
const IPO_NAME_MAX: u16 = 40;

fn ipo_row(
    row: &bids::Instrument,
    status: &bids::BidStatus,
    fit: &columns::ColumnFit,
    layout: &columns::TableLayout,
) -> ratatui::widgets::Row<'static> {
    let band_width = usize::from(layout.width(3));
    let open = row.is_open();
    let cells = fit_row(
        fit,
        &[
            &|| bid_symbol_cell(row.label(), row.is_sme().then_some("SME")),
            &|| Cell::from(format::pad_or_clip(&row.name, layout.width(1) as usize)),
            &|| right(format::date_span(&row.start_at, &row.end_at)),
            &|| {
                right(match row.price_band() {
                    Some((low, high)) if low > 0.0 && (low - high).abs() > f64::EPSILON => {
                        format::fit_cell(
                            &format!("{}–{}", format::whole_price(low), format::whole_price(high)),
                            &format!(
                                "{}–{}",
                                format::compact_amount(low),
                                format::compact_amount(high)
                            ),
                            band_width,
                        )
                    }
                    Some((_, high)) => fit_amount(high, format::whole_price(high), band_width),
                    None => ".".to_string(),
                })
            },
            &|| {
                right(
                    row.min_amount()
                        .map(|amount| {
                            fit_amount(
                                amount,
                                format::whole_price(amount),
                                usize::from(layout.width(4)),
                            )
                        })
                        .unwrap_or_default(),
                )
            },
            &|| {
                right(if row.min_qty > 0 {
                    row.min_qty.to_string()
                } else {
                    String::new()
                })
            },
            &|| bid_status_cell(status),
        ],
    );
    if open || row.status.is_empty() {
        cells
    } else {
        cells.style(format::dim())
    }
}

pub(super) fn draw_ipo_table(
    frame: &mut Frame,
    area: Rect,
    app: &App,
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

    let fit = columns::fit_columns(table_budget(area.width), 1, &IPO_COLS);
    let layout = columns::spread(
        &fit,
        &IPO_COLS,
        &[
            columns::text(
                0,
                columns::widest(rows.iter().map(|row| {
                    format::display_width(row.label()) + if row.is_sme() { 4 } else { 0 }
                })),
                IPO_SYM_MAX,
            ),
            columns::text(
                1,
                columns::widest(rows.iter().map(|row| format::display_width(&row.name))),
                IPO_NAME_MAX,
            ),
        ],
    );
    let status = |row: &bids::Instrument| -> bids::BidStatus {
        if section == BidsSection::Sse {
            return row.sse_status();
        }
        let applied = app
            .bids
            .applications
            .as_deref()
            .is_some_and(|applications| {
                applications
                    .iter()
                    .any(|item| item.symbol == row.label() && item.is_live())
            });
        if applied {
            bids::BidStatus::Live("applied".to_string())
        } else {
            bids::BidStatus::of(row.status.to_ascii_lowercase(), row.is_open())
        }
    };
    let table_rows = rows
        .iter()
        .map(|row| ipo_row(row, &status(row), &fit, &layout))
        .collect::<Vec<_>>();
    let open = rows.iter().filter(|row| row.is_open()).count();
    let table = Table::new(table_rows, fit_constraints(&fit, &layout))
        .header(fit_header(&fit, &IPO_LABELS))
        .column_spacing(layout.spacing)
        .block(ui::focus_panel(
            format!("{} · {open} open", section.title()),
            focused,
        ));
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

const APPLICATION_COLS: [FitCol; 5] = [
    col(18, 0), // instrument
    col(16, 2), // investor type
    col(13, 3), // created
    col(12, 1), // allotment
    col(16, 4), // status
];
const APPLICATION_LABELS: [(&str, bool); 5] = [
    ("INSTRUMENT", false),
    ("INVESTOR TYPE", false),
    ("CREATED", true),
    ("ALLOTMENT", true),
    ("STATUS", true),
];
const APPLICATION_SYM_MAX: u16 = 38;

pub(super) fn draw_ipo_applications(
    frame: &mut Frame,
    area: Rect,
    applications: &[bids::Application],
    list: &PlainList<'_>,
    hits: &PaneHits<'_>,
) {
    let PlainList {
        selected,
        focused,
        viewport,
    } = *list;

    let fit = columns::fit_columns(table_budget(area.width), 1, &APPLICATION_COLS);
    let layout = columns::spread(
        &fit,
        &APPLICATION_COLS,
        &[columns::text(
            0,
            columns::widest(
                applications
                    .iter()
                    .map(|application| format::display_width(&application.symbol)),
            ),
            APPLICATION_SYM_MAX,
        )],
    );
    let symbol_width = layout.width(0);
    let rows = applications
        .iter()
        .map(|application| {
            fit_row(
                &fit,
                &[
                    &|| {
                        Cell::from(format::pad_or_clip(
                            &application.symbol,
                            symbol_width as usize,
                        ))
                    },
                    &|| Cell::from(application.investor_type.clone()),
                    &|| right(format::day_month(&application.created_at)),
                    &|| {
                        right(if application.allotment_quantity > 0 {
                            application.allotment_quantity.to_string()
                        } else {
                            "N/A".into()
                        })
                    },
                    &|| {
                        bid_status_cell(&bids::BidStatus::of(
                            application.status.to_ascii_lowercase(),
                            application.is_live(),
                        ))
                    },
                ],
            )
        })
        .collect::<Vec<_>>();
    let table = Table::new(rows, fit_constraints(&fit, &layout))
        .header(fit_header(&fit, &APPLICATION_LABELS))
        .column_spacing(layout.spacing)
        .block(ui::focus_panel("IPO · applications", focused));
    selectable_table(
        frame,
        area,
        table,
        &layout,
        TableRows {
            selected,
            count: applications.len(),
            viewport,
        },
        &hits.under(ui::HitList::BidsApplications(hits.owner())),
    );
}

#[cfg(test)]
mod ipo_render_tests {
    use super::super::draw_bids;
    use crate::tui::app::App;
    use crate::tui::bids::{BidOrder, Instrument, InvestorType, Section};
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
    fn ipo_band_and_minimum_shorten_rather_than_lose_leading_digits() {
        let mut app = app();
        app.bids.ipo = Some(vec![Instrument {
            symbol: "BIGISSUE".into(),
            name: "Big Issue Limited".into(),
            status: "ongoing".into(),
            start_at: "2026-09-07 10:00:00".into(),
            end_at: "2026-09-09 17:00:00".into(),
            min_qty: 1_000,
            investor_types: vec![InvestorType {
                min_price: 10_000.0,
                max_price: 12_000.0,
            }],
            ..Default::default()
        }]);
        let rows = rendered(&app);
        assert!(rows.contains("0.10L–0.12L"), "{rows}");
        assert!(!rows.contains(",000–12,000"), "{rows}");
        assert!(rows.contains("1.20Cr"), "{rows}");
        assert!(!rows.contains(",20,00,000"), "{rows}");
    }

    #[test]
    fn sse_rows_use_kite_web_labels_and_ignore_the_order_field() {
        let mut app = app();
        app.bids_pane.set_section(Section::Sse);
        let issue = |symbol: &str, status: &str, orders: Vec<BidOrder>| Instrument {
            symbol: symbol.into(),
            name: format!("{symbol} Foundation"),
            status: status.into(),
            start_at: "2026-11-11 10:00:00".into(),
            end_at: "2026-11-20 17:00:00".into(),
            min_qty: 1_000,
            investor_types: vec![InvestorType {
                min_price: 1.0,
                max_price: 1.0,
            }],
            order: Some(BidOrder::default()),
            orders: Some(orders),
            ..Default::default()
        };
        let order = |status: &str| BidOrder {
            status: status.into(),
            ..Default::default()
        };
        app.bids.sse = Some(vec![issue(
            "FLZCZP",
            "ongoing",
            vec![order("cancelled"), order("failed")],
        )]);
        let rows = rendered(&app);
        assert!(rows.contains("Give back"), "{rows}");
        assert!(!rows.contains("Applied"), "{rows}");

        app.bids.sse = Some(vec![
            issue("LIVE", "ongoing", vec![order("submitted")]),
            issue("SOON", "preapply", Vec::new()),
            issue("DONE", "closed", Vec::new()),
        ]);
        let rows = rendered(&app);
        assert!(rows.contains("Applied"), "{rows}");
        assert!(rows.contains("Pre-apply"), "{rows}");
        assert!(rows.contains("CLOSED"), "{rows}");
    }
}
