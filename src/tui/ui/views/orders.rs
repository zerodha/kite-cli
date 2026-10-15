use super::book_state::loaded;
use super::headers::{orders_header_sort, orders_sort_targets};
use super::inline_chart::{chart_split, draw_chart_panel};
use super::table::{
    TableRows, record_header_hits, right, selectable_table, sorted_title, symbol_cell, symbol_cells,
};
use crate::tui::app::{App, OrdersSort};
use crate::tui::data::Order;

use crate::tui::format;
use crate::tui::ui;
use crate::tui::ui::columns::{self, FitCol, col, fit_constraints, fit_row, table_budget};
use crate::tui::ui::pane::{BookList, InlineChartInput, PaneHits};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::{Cell, Row, Table};

const ORD_SYM_MIN: u16 = 10;
const ORD_SYM_MAX: u16 = 28;

pub(super) const ORD_COLS: [FitCol; 11] = [
    col(3, 2),           // SEL
    col(8, 7),           // TIME
    col(ORD_SYM_MIN, 0), // SYMBOL
    col(4, 3),           // SIDE
    col(6, 8),           // TYPE
    col(9, 5),           // KIND
    col(5, 10),          // PROD
    col(9, 4),           // QTY
    col(11, 6),          // PRICE
    col(9, 9),           // TRIGGER
    col(15, 1),          // STATUS
];

pub(super) const ORD_LABELS: [(&str, bool); 11] = [
    ("SEL", false),
    ("TIME", false),
    ("SYMBOL", false),
    ("SIDE", false),
    ("TYPE", false),
    ("KIND", false),
    ("PROD", false),
    ("QTY", true),
    ("PRICE", true),
    ("TRIGGER", true),
    ("STATUS", false),
];

const ORD_QTY: usize = 7;

fn quantity_pair(order: &Order) -> String {
    format!("{}/{}", order.filled_quantity, order.quantity)
}

fn order_checkbox(marked: bool) -> &'static str {
    if marked { "[x]" } else { "[ ]" }
}

fn order_row(
    app: &App,
    order: &Order,
    fit: &columns::ColumnFit,
    sym_w: u16,
    hits: &PaneHits<'_>,
) -> Row<'static> {
    let time = || order.order_timestamp.format("%H:%M:%S").to_string();
    let side_style = if order.transaction_type == "BUY" {
        Style::default().fg(format::up())
    } else {
        Style::default().fg(format::down())
    };
    let status_style = match order.status.as_str() {
        "COMPLETE" => Style::default().fg(format::up()),
        "REJECTED" | "CANCELLED" => Style::default().fg(format::down()),
        _ if crate::tui::data::order_is_open(order) => Style::default().fg(format::warning()),
        _ => Style::default(),
    };
    let price = || {
        if order.average_price > 0.0 {
            format::price(order.average_price)
        } else if order.price > 0.0 {
            format::price(order.price)
        } else {
            "MKT".to_string()
        }
    };
    fit_row(
        fit,
        &[
            &|| {
                let marked = app.is_order_marked_in(hits.owner(), &order.order_id);
                let style = if marked {
                    Style::default().fg(format::up())
                } else if !crate::tui::data::order_is_open(order) {
                    format::dim()
                } else {
                    Style::default()
                };
                Cell::from(Span::styled(order_checkbox(marked), style))
            },
            &|| Cell::from(Span::styled(time(), format::dim())),
            &|| symbol_cell(&order.exchange, &order.tradingsymbol, sym_w),
            &|| Cell::from(Span::styled(order.transaction_type.clone(), side_style)),
            &|| Cell::from(order.order_type.clone()),
            &|| {
                Cell::from(Span::styled(
                    crate::tui::data::order_kind_label(order),
                    format::warning(),
                ))
            },
            &|| Cell::from(Span::styled(order.product.clone(), format::dim())),
            &|| right(quantity_pair(order)),
            &|| right(price()),
            &|| {
                right(if order.trigger_price > 0.0 {
                    format::price(order.trigger_price)
                } else {
                    ".".to_string()
                })
            },
            &|| Cell::from(Span::styled(order.status.clone(), status_style)),
        ],
    )
}

pub fn draw_orders_pane(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    list: &BookList<'_, OrdersSort>,
    chart: &InlineChartInput<'_>,
    hits: &PaneHits<'_>,
) {
    let Some(orders) = loaded(
        frame,
        area,
        "Orders",
        app.orders.as_deref(),
        "No orders today",
    ) else {
        return;
    };

    let split = chart_split(area, &app.splits, chart.open);
    let (content_area, chart_area) = (split.table, split.chart);
    let targets = orders_sort_targets();
    let mut cols = columns::sort_floors(&ORD_COLS, &ORD_LABELS, &targets.map(|(idx, _)| idx));
    cols[ORD_QTY].width = cols[ORD_QTY].width.max(columns::widest(
        orders
            .iter()
            .map(|order| format::display_width(&quantity_pair(order))),
    ));
    let fit = columns::fit_columns(table_budget(content_area.width), 1, &cols);
    let layout = columns::spread(
        &fit,
        &cols,
        &[columns::text(
            2,
            columns::widest(
                orders
                    .iter()
                    .map(|order| symbol_cells(&order.exchange, &order.tradingsymbol)),
            ),
            ORD_SYM_MAX,
        )],
    );
    let sym_w = layout.width(2);
    let display = list.display;
    let rows: Vec<Row> = display
        .iter()
        .map(|&i| order_row(app, &orders[i], &fit, sym_w, hits))
        .collect();

    let (header, marked) =
        columns::fit_header_sorted(&fit, &layout, &ORD_LABELS, orders_header_sort(list.sort));
    let title = sorted_title("Orders", list.sort.label(), marked);
    let table = Table::new(rows, fit_constraints(&fit, &layout))
        .header(header)
        .column_spacing(layout.spacing)
        .block(ui::panel(list.title(&title)));

    record_header_hits(hits, content_area, &fit, &layout, &cols, &targets);
    selectable_table(
        frame,
        content_area,
        table,
        &layout,
        TableRows {
            selected: list.selected,
            count: orders.len(),
            viewport: Some(list.viewport),
        },
        hits,
    );
    if let Some(chart_area) = chart_area {
        draw_chart_panel(frame, chart_area, app, chart.target, chart.chart, hits);
    }
    split.push_divider(app, area);
}

#[cfg(test)]
mod orders_render_tests {
    use super::order_checkbox;
    use crate::tui::app::App;
    use crate::tui::data::Order;
    use crate::tui::notes::Pad;
    use crate::tui::settings::Settings;
    use crate::tui::workspace::PaneId;
    use kiteconnect::KiteConnect;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn orders_app(orders: Vec<Order>) -> App {
        let client = KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("tok").unwrap())
            .build()
            .expect("dummy client builds");
        let (events_tx, _events_rx) = tokio::sync::mpsc::channel(8);
        let mut app = App::new(
            client,
            events_tx,
            crate::tui::marketwatch::State::test_loaded(Vec::new()),
            Pad::default(),
            false,
            &Settings::default(),
        );
        app.set_pane(PaneId::OrdersBook);
        app.orders = Some(orders);
        app
    }

    #[test]
    fn a_large_quantity_pair_is_drawn_whole() {
        let mut order: Order = crate::tui::data::test_models::order();
        order.order_id = "251009000000001".into();
        order.order_timestamp = "2026-09-09T10:15:00".parse().unwrap();
        order.status = "COMPLETE".into();
        order.exchange = "NSE".into();
        order.tradingsymbol = "IDEA".into();
        order.instrument_token = 1;
        order.transaction_type = "BUY".into();
        order.order_type = "LIMIT".into();
        order.product = "CNC".into();
        order.price = 7.5;
        order.average_price = 7.45;
        order.quantity = 100_000.0;
        order.filled_quantity = 100_000.0;
        let app = orders_app(vec![order]);
        for (w, h) in [(80, 24), (120, 35), (160, 24), (160, 45)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal
                .draw(|frame| crate::tui::ui::draw(frame, &app))
                .unwrap();
            let buffer = terminal.backend().buffer().clone();
            let text: Vec<String> = (0..h)
                .map(|y| (0..w).map(|x| buffer[(x, y)].symbol()).collect())
                .collect();
            let text = text.join("\n");
            assert!(text.contains("IDEA"), "{w}x{h}:\n{text}");
            if text.contains("QTY") {
                assert!(
                    text.contains("100000/100000"),
                    "{w}x{h}: the pair is clipped:\n{text}"
                );
            }
        }
    }

    #[test]
    fn marked_order_checkbox_renders_checked() {
        assert_eq!(order_checkbox(false), "[ ]");
        assert_eq!(order_checkbox(true), "[x]");
    }
}
