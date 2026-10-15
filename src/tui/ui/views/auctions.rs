use super::book_state::{loaded, state_panel};
use super::table::{TableRows, right, right_styled, selectable_table, symbol_cell, symbol_cells};
use crate::tui::app::App;
use crate::tui::format;
use crate::tui::ui;
use crate::tui::ui::columns::{
    self, FitCol, col, fit_constraints, fit_header, fit_row, table_budget,
};
use crate::tui::ui::pane::{PaneHits, PlainList};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::{Cell, Row, Table};

const AUCTION_SYM_MIN: u16 = 12;
const AUCTION_SYM_MAX: u16 = 24;
const AUCTION_PNL_W: u16 = 12;
const AUCTION_COLS: [FitCol; 6] = [
    col(12, 0),              // AUCTION
    col(AUCTION_SYM_MIN, 1), // SYMBOL
    col(7, 2),               // QTY
    col(11, 5),              // AVG
    col(11, 3),              // LTP
    col(AUCTION_PNL_W, 4),   // P&L
];
const AUCTION_LABELS: [(&str, bool); 6] = [
    ("AUCTION", false),
    ("SYMBOL", false),
    ("QTY", true),
    ("AVG", true),
    ("LTP", true),
    ("P&L", true),
];

fn auction_row(
    app: &App,
    auction: &crate::tui::data::AuctionHolding,
    fit: &columns::ColumnFit,
    symbol_width: u16,
) -> ratatui::widgets::Row<'static> {
    let ltp = app
        .live_ltp(auction.instrument_token)
        .unwrap_or(auction.last_price);
    #[expect(
        clippy::cast_precision_loss,
        reason = "auction value is display-only f64 arithmetic"
    )]
    let pnl = (ltp - auction.average_price)
        * (auction.quantity + auction.t1_quantity + auction.collateral_quantity) as f64;
    let has_pnl = !auction.discrepancy && ltp > 0.0;
    fit_row(
        fit,
        &[
            &|| {
                Cell::from(Span::styled(
                    auction.auction_number.clone(),
                    format::warning(),
                ))
            },
            &|| symbol_cell(&auction.exchange, &auction.tradingsymbol, symbol_width),
            &|| right(auction.quantity.to_string()),
            &|| right(format::price(auction.average_price)),
            &|| right(format::price(ltp)),
            &|| {
                right_styled(
                    if has_pnl {
                        format::fit_money(pnl, &format::signed_price(pnl), AUCTION_PNL_W as usize)
                    } else {
                        "N/A".to_string()
                    },
                    if has_pnl {
                        format::change_style(pnl)
                    } else {
                        Style::default()
                    },
                )
            },
        ],
    )
}

pub fn draw_auctions(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    list: &PlainList<'_>,
    hits: &PaneHits<'_>,
) {
    let PlainList {
        selected, viewport, ..
    } = *list;
    if let Some(error) = app.bids.auctions_error.as_deref() {
        state_panel(
            frame,
            area,
            " Auction ",
            ui::MessageTone::Error,
            "Auction holdings unavailable",
            Some(error),
            Some("r Retry"),
        );
        return;
    }
    let Some(auctions) = loaded(
        frame,
        area,
        "Auction",
        app.auctions.as_deref(),
        "No exchange auction holdings",
    ) else {
        return;
    };

    let fit = columns::fit_columns(table_budget(area.width), 1, &AUCTION_COLS);
    let layout = columns::spread(
        &fit,
        &AUCTION_COLS,
        &[columns::text(
            1,
            columns::widest(
                auctions
                    .iter()
                    .map(|auction| symbol_cells(&auction.exchange, &auction.tradingsymbol)),
            ),
            AUCTION_SYM_MAX,
        )],
    );
    let symbol_width = layout.width(1);
    let rows: Vec<Row> = auctions
        .iter()
        .map(|auction| auction_row(app, auction, &fit, symbol_width))
        .collect();
    let table = Table::new(rows, fit_constraints(&fit, &layout))
        .header(fit_header(&fit, &AUCTION_LABELS))
        .column_spacing(layout.spacing)
        .block(ui::panel(" Auction holdings · s:sell "));
    selectable_table(
        frame,
        area,
        table,
        &layout,
        TableRows {
            selected,
            count: auctions.len(),
            viewport,
        },
        hits,
    );
}
