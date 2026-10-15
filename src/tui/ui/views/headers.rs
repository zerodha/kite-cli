use crate::tui::app::{
    AlertsSort, GttSort, HoldingsSort, MfSort, OrdersSort, PositionsSort, SortTarget,
};
use crate::tui::ui::columns::{HeaderSort, asc, desc};

pub(super) fn positions_header_sort(mode: PositionsSort) -> HeaderSort {
    match mode {
        PositionsSort::Symbol => asc(0),
        PositionsSort::Qty => desc(2),
        PositionsSort::Pnl => desc(5),
        PositionsSort::DayPct => desc(6),
    }
}

pub(super) fn positions_sort_targets() -> [(usize, SortTarget); 4] {
    PositionsSort::ALL.map(|mode| {
        (
            positions_header_sort(mode).column,
            SortTarget::Positions(mode),
        )
    })
}

pub(super) fn orders_header_sort(mode: OrdersSort) -> HeaderSort {
    match mode {
        OrdersSort::Time => desc(1),
        OrdersSort::Symbol => asc(2),
        OrdersSort::Status => asc(10),
    }
}

pub(super) fn orders_sort_targets() -> [(usize, SortTarget); 3] {
    OrdersSort::ALL.map(|mode| (orders_header_sort(mode).column, SortTarget::Orders(mode)))
}

pub(super) fn gtt_header_sort(mode: GttSort) -> HeaderSort {
    match mode {
        GttSort::Symbol => asc(1),
        GttSort::Type => asc(2),
        GttSort::Status => asc(5),
        GttSort::Created => desc(7),
    }
}

pub(super) fn gtt_sort_targets() -> [(usize, SortTarget); 4] {
    GttSort::ALL.map(|mode| (gtt_header_sort(mode).column, SortTarget::Gtt(mode)))
}

pub(super) fn alerts_header_sort(mode: AlertsSort) -> HeaderSort {
    match mode {
        AlertsSort::Name => asc(0),
        AlertsSort::Symbol => asc(1),
        AlertsSort::Status => asc(3),
        AlertsSort::Fired => desc(5),
        AlertsSort::Created => desc(7),
    }
}

pub(super) fn alerts_sort_targets() -> [(usize, SortTarget); 5] {
    AlertsSort::ALL.map(|mode| (alerts_header_sort(mode).column, SortTarget::Alerts(mode)))
}

pub(super) fn holdings_header_sort(mode: HoldingsSort) -> HeaderSort {
    match mode {
        HoldingsSort::Symbol => asc(0),
        HoldingsSort::Value => desc(4),
        HoldingsSort::PnlAbs => desc(5),
        HoldingsSort::PnlPct => desc(6),
        HoldingsSort::DayPct => desc(7),
    }
}

pub(super) fn holdings_sort_targets() -> [(usize, SortTarget); 5] {
    HoldingsSort::ALL.map(|mode| {
        (
            holdings_header_sort(mode).column,
            SortTarget::Holdings(mode),
        )
    })
}

pub(super) fn mf_header_sort(mode: MfSort) -> HeaderSort {
    match mode {
        MfSort::Name => asc(0),
        MfSort::Value => desc(5),
        MfSort::Pnl => desc(6),
        MfSort::DayChange => desc(8),
    }
}

pub(super) fn mf_sort_targets() -> [(usize, SortTarget); 4] {
    MfSort::ALL.map(|mode| (mf_header_sort(mode).column, SortTarget::Mf(mode)))
}

#[cfg(test)]
mod sort_header_tests {

    use super::super::{
        ALERT_LABELS, GTT_COLS, GTT_LABELS, HOLDINGS_COLS, HOLDINGS_LABELS, MF_COLS, MF_LABELS,
        ORD_COLS, ORD_LABELS, POS_COLS, POS_LABELS, draw_holdings, draw_orders, draw_positions,
        table_budget,
    };
    use super::{
        alerts_header_sort, alerts_sort_targets, gtt_header_sort, gtt_sort_targets,
        holdings_header_sort, holdings_sort_targets, mf_header_sort, mf_sort_targets,
        orders_header_sort, orders_sort_targets, positions_header_sort, positions_sort_targets,
    };
    use crate::tui::app::SortTarget;
    use crate::tui::app::{
        AlertsSort, App, GttSort, HoldingsSort, MfSort, OrdersSort, PositionsSort,
    };
    use crate::tui::data::{Holding, Order, Position};
    use crate::tui::event::AppEvent;
    use crate::tui::notes::Pad;
    use crate::tui::settings::Settings;
    use crate::tui::ui::columns::{self, FitCol, SortDirection, asc, desc};
    use crate::tui::workspace::PaneId;
    use crossterm::event::{KeyCode, KeyEvent};
    use kiteconnect::KiteConnect;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn test_app() -> App {
        let client = KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("tok").unwrap())
            .build()
            .unwrap();
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

    fn holding() -> Holding {
        {
            let mut row = crate::tui::data::test_models::holding();
            row.tradingsymbol = "DIVISLAB".into();
            row.exchange = "NSE".into();
            row.instrument_token = 1;
            row.quantity = 15;
            row.t1_quantity = 0;
            row.collateral_quantity = 0;
            row.mtf = None;
            row.average_price = 6_077.93;
            row.last_price = 30_000.0;
            row.close_price = 25_000.0;
            row.day_change = None;
            row.discrepancy = false;
            row
        }
    }

    fn position() -> Position {
        {
            let mut row = crate::tui::data::test_models::position();
            row.tradingsymbol = "DIVISLAB".into();
            row.exchange = "NSE".into();
            row.instrument_token = 1;
            row.product = "CNC".into();
            row.overnight_quantity = 0;
            row.day_buy_quantity = 0;
            row.day_sell_quantity = 0;
            row.quantity = 15;
            row.average_price = 6_077.93;
            row.last_price = 30_000.0;
            row.close_price = 25_000.0;
            row.pnl = 358_831.05;
            row.realised = 0.0;
            row.multiplier = 1.0;
            row
        }
    }

    fn order() -> Order {
        {
            let mut row = crate::tui::data::test_models::order();
            row.order_id = "251009000000001".into();
            row.order_timestamp = "2026-09-09T10:15:00".parse().expect("timestamp");
            row.status = "COMPLETE".into();
            row.status_message = None;
            row.exchange = "NSE".into();
            row.tradingsymbol = "DIVISLAB".into();
            row.instrument_token = 1;
            row.transaction_type = "BUY".into();
            row.order_type = "LIMIT".into();
            row.product = "CNC".into();
            row.validity = "DAY".into();
            row.validity_ttl = None;
            row.variety = "regular".into();
            row.tags = Vec::new();
            row.price = 6_100.0;
            row.trigger_price = 0.0;
            row.average_price = 6_077.93;
            row.quantity = f64::from(15);
            row.filled_quantity = f64::from(15);
            row.pending_quantity = f64::from(0);
            row
        }
    }

    fn loaded_app() -> App {
        let mut app = test_app();
        app.positions = Some({
            let mut row = crate::tui::data::test_models::positions();
            row.net = vec![position()];
            row
        });
        app.orders = Some(vec![order()]);
        app.holdings = Some(vec![holding()]);
        app
    }

    fn rows_at(
        width: u16,
        height: u16,
        app: &App,
        draw: fn(&mut ratatui::Frame, ratatui::layout::Rect, &App),
    ) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| draw(frame, frame.area(), app))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        (0..height)
            .map(|y| (0..width).map(|x| buffer[(x, y)].symbol()).collect())
            .collect()
    }

    fn header_row(rows: &[String], identifying: &str) -> String {
        rows.iter()
            .find(|row| row.contains(identifying))
            .cloned()
            .unwrap_or_default()
    }

    fn title_row(rows: &[String]) -> String {
        rows.iter()
            .find(|row| row.starts_with('┌'))
            .cloned()
            .unwrap_or_default()
    }

    fn sortable_columns(targets: &[(usize, SortTarget)]) -> Vec<usize> {
        targets.iter().map(|(idx, _)| *idx).collect()
    }

    fn header_shape(header: &str) -> String {
        header
            .replace(['▼', '▲', '│'], "")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn each_sort_mode_marks_the_column_it_orders_by() {
        assert_eq!(positions_header_sort(PositionsSort::Pnl), desc(5));
        assert_eq!(positions_header_sort(PositionsSort::Qty), desc(2));
        assert_eq!(positions_header_sort(PositionsSort::DayPct), desc(6));
        assert_eq!(positions_header_sort(PositionsSort::Symbol), asc(0));
        assert_eq!(orders_header_sort(OrdersSort::Time), desc(1));
        assert_eq!(orders_header_sort(OrdersSort::Status), asc(10));
        assert_eq!(orders_header_sort(OrdersSort::Symbol), asc(2));
        assert_eq!(holdings_header_sort(HoldingsSort::PnlPct), desc(6));
        assert_eq!(holdings_header_sort(HoldingsSort::PnlAbs), desc(5));
        assert_eq!(holdings_header_sort(HoldingsSort::Value), desc(4));
        assert_eq!(holdings_header_sort(HoldingsSort::DayPct), desc(7));
        assert_eq!(holdings_header_sort(HoldingsSort::Symbol), asc(0));
        assert_eq!(mf_header_sort(MfSort::Name), asc(0));
        assert_eq!(mf_header_sort(MfSort::Value), desc(5));
        assert_eq!(mf_header_sort(MfSort::Pnl), desc(6));
        assert_eq!(mf_header_sort(MfSort::DayChange), desc(8));
        assert_eq!(gtt_header_sort(GttSort::Created), desc(7));
        assert_eq!(gtt_header_sort(GttSort::Status), asc(5));
        assert_eq!(gtt_header_sort(GttSort::Symbol), asc(1));
        assert_eq!(gtt_header_sort(GttSort::Type), asc(2));
        assert_eq!(alerts_header_sort(AlertsSort::Name), asc(0));
        assert_eq!(alerts_header_sort(AlertsSort::Symbol), asc(1));
        assert_eq!(alerts_header_sort(AlertsSort::Status), asc(3));
        assert_eq!(alerts_header_sort(AlertsSort::Fired), desc(5));
        assert_eq!(alerts_header_sort(AlertsSort::Created), desc(7));
    }

    #[test]
    fn every_marked_column_is_the_labelled_one() {
        let cases: [(&str, usize, &str); 21] = [
            ("positions", 0, "SYMBOL"),
            ("positions", 2, "QTY"),
            ("positions", 5, "P&L"),
            ("positions", 6, "DAY%"),
            ("orders", 1, "TIME"),
            ("orders", 2, "SYMBOL"),
            ("orders", 10, "STATUS"),
            ("holdings", 0, "SYMBOL"),
            ("holdings", 4, "VALUE"),
            ("holdings", 5, "P&L"),
            ("holdings", 6, "P&L%"),
            ("holdings", 7, "DAY%"),
            ("mf", 0, "FUND"),
            ("mf", 5, "CUR.VAL"),
            ("mf", 6, "P&L"),
            ("mf", 8, "DAY%"),
            ("gtt", 1, "SYMBOL"),
            ("gtt", 2, "TYPE"),
            ("gtt", 5, "STATUS"),
            ("gtt", 7, "CREATED"),
            ("alerts", 5, "TRIGGERED"),
        ];
        for (table, idx, label) in cases {
            let labels: &[(&str, bool)] = match table {
                "positions" => &POS_LABELS,
                "orders" => &ORD_LABELS,
                "holdings" => &HOLDINGS_LABELS,
                "mf" => &MF_LABELS,
                "gtt" => &GTT_LABELS,
                _ => &ALERT_LABELS,
            };
            assert_eq!(labels[idx].0, label, "{table} column {idx}");
        }
    }

    #[test]
    fn every_sortable_column_reserves_its_arrow() {
        type SortableTable<'a> = (&'a str, &'a [FitCol], &'a [(&'a str, bool)], &'a [usize]);
        let (pos, ord, hold, mf, gtt) = (
            sortable_columns(&positions_sort_targets()),
            sortable_columns(&orders_sort_targets()),
            sortable_columns(&holdings_sort_targets()),
            sortable_columns(&mf_sort_targets()),
            sortable_columns(&gtt_sort_targets()),
        );
        let tables: [SortableTable; 5] = [
            ("positions", &POS_COLS, &POS_LABELS, &pos),
            ("orders", &ORD_COLS, &ORD_LABELS, &ord),
            ("holdings", &HOLDINGS_COLS, &HOLDINGS_LABELS, &hold),
            ("mf", &MF_COLS, &MF_LABELS, &mf),
            ("gtt", &GTT_COLS, &GTT_LABELS, &gtt),
        ];
        for (name, cols, labels, sortable) in tables {
            let floored = columns::sort_floors(cols, labels, sortable);
            for &idx in sortable {
                let needed = crate::tui::format::display_width(labels[idx].0) + 2;
                assert!(
                    floored[idx].width as usize >= needed,
                    "{name} column {idx} ({}) has no room for an arrow",
                    labels[idx].0
                );
                assert!(floored[idx].width >= cols[idx].width);
            }
            for (idx, (before, after)) in cols.iter().zip(&floored).enumerate() {
                if !sortable.contains(&idx) {
                    assert_eq!(
                        before.width, after.width,
                        "{name} widened non-sortable column {idx}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_alerts_triggered_column_is_widened_for_its_arrow() {
        let declared = [
            columns::col(12, 0),
            columns::col(16, 3),
            columns::col(11, 2),
            columns::col(8, 4),
            columns::col(0, 1),
            columns::col(9, 5),
            columns::col(6, 6),
            columns::col(10, 7),
        ];
        let floored = columns::sort_floors(
            &declared,
            &ALERT_LABELS,
            &sortable_columns(&alerts_sort_targets()),
        );
        assert_eq!(declared[5].width, 9);
        assert_eq!(floored[5].width, 11);
        assert_eq!(floored[3].width, 8);
    }

    #[test]
    fn the_sorted_header_carries_the_arrow_and_the_title_goes_plain() {
        let app = loaded_app();
        for (width, height) in [(80u16, 24u16), (120, 35)] {
            let rows = rows_at(width, height, &app, draw_positions);
            let header = header_row(&rows, "SYMBOL");
            assert!(
                header.contains("P&L ▼"),
                "{width}x{height} positions header has no P&L arrow: {header:?}"
            );
            assert!(
                !header.contains("QTY ▼") && !header.contains("SYMBOL ▲"),
                "{width}x{height} marked more than one positions column: {header:?}"
            );
            let title = title_row(&rows);
            assert!(title.contains("Positions"), "{title:?}");
            assert!(
                !title.contains("P&L ▼"),
                "{width}x{height} title still repeats the sort: {title:?}"
            );

            let rows = rows_at(width, height, &app, draw_holdings);
            let header = header_row(&rows, "SYMBOL");
            assert!(
                header.contains("P&L% ▼"),
                "{width}x{height} holdings header has no P&L% arrow: {header:?}"
            );
            assert!(
                !header.contains("P&L ▼"),
                "{width}x{height} marked the wrong holdings column: {header:?}"
            );

            let rows = rows_at(width, height, &app, draw_orders);
            let header = header_row(&rows, "SYMBOL");
            assert!(
                header.contains("TIME ▼"),
                "{width}x{height} orders header has no TIME arrow: {header:?}"
            );
        }
    }

    #[test]
    fn cycling_the_sort_moves_the_arrow_but_not_the_columns() {
        let mut app = loaded_app();
        app.set_pane(PaneId::PortfolioPositions);
        let baseline = header_shape(&header_row(
            &rows_at(80, 24, &app, draw_positions),
            "SYMBOL",
        ));
        let mut marked = Vec::new();
        for _ in 0..4 {
            app.on_event(AppEvent::Key(KeyEvent::from(KeyCode::Char('S'))));
            let rows = rows_at(80, 24, &app, draw_positions);
            let header = header_row(&rows, "SYMBOL");
            assert_eq!(
                header_shape(&header),
                baseline,
                "sorting reshuffled the columns: {header:?}"
            );
            assert_eq!(
                header.matches('▼').count() + header.matches('▲').count(),
                1,
                "exactly one column is marked: {header:?}"
            );
            marked.push(header);
        }
        assert!(marked[0].contains("QTY ▼"), "{:?}", marked[0]);
        assert!(marked[1].contains("DAY% ▼"), "{:?}", marked[1]);
        assert!(marked[2].contains("SYMBOL ▲"), "{:?}", marked[2]);
        assert!(marked[3].contains("P&L ▼"), "{:?}", marked[3]);
    }

    #[test]
    fn the_eighty_column_kept_sets_are_what_the_budget_says() {
        fn kept<'a>(
            cols: &[FitCol],
            labels: &[(&'a str, bool)],
            sortable: &[usize],
            budget: u16,
        ) -> Vec<&'a str> {
            let floored = columns::sort_floors(cols, labels, sortable);
            let fit = columns::fit_columns(budget, 1, &floored);
            (0..labels.len())
                .filter(|idx| fit.shows(*idx))
                .map(|idx| labels[idx].0)
                .collect()
        }
        assert_eq!(table_budget(80), 76);

        assert_eq!(
            kept(
                &POS_COLS,
                &POS_LABELS,
                &sortable_columns(&positions_sort_targets()),
                76
            ),
            vec!["SYMBOL", "PROD", "QTY", "AVG", "LTP", "P&L", "DAY%"]
        );
        assert_eq!(
            kept(
                &HOLDINGS_COLS,
                &HOLDINGS_LABELS,
                &sortable_columns(&holdings_sort_targets()),
                76
            ),
            vec!["SYMBOL", "QTY", "LTP", "VALUE", "P&L", "P&L%", "DAY%"]
        );
        assert_eq!(
            kept(
                &ORD_COLS,
                &ORD_LABELS,
                &sortable_columns(&orders_sort_targets()),
                76
            ),
            vec![
                "SEL", "TIME", "SYMBOL", "SIDE", "KIND", "QTY", "PRICE", "STATUS"
            ]
        );
        assert!(
            !kept(
                &ORD_COLS,
                &ORD_LABELS,
                &sortable_columns(&orders_sort_targets()),
                75
            )
            .contains(&"TIME")
        );
    }

    #[test]
    fn a_dropped_sorted_column_falls_back_to_the_title() {
        let app = loaded_app();
        let rows = rows_at(79, 24, &app, draw_orders);
        let header = header_row(&rows, "SYMBOL");
        assert!(!header.contains("TIME"), "TIME survived 79: {header:?}");
        assert!(
            !header.contains('▼') && !header.contains('▲'),
            "orphan arrow on an unsorted column: {header:?}"
        );
        assert!(
            title_row(&rows).contains("time ▼"),
            "the dropped sort is named nowhere: {:?}",
            title_row(&rows)
        );
    }

    #[test]
    fn an_absurdly_narrow_panel_shows_no_orphan_arrow() {
        let mut app = loaded_app();
        app.books.portfolio_holdings.set_sort(HoldingsSort::Symbol);
        for width in 8..=18u16 {
            let rows = rows_at(width, 8, &app, draw_holdings);
            let body = rows.join("\n");
            let arrows = body.matches('▲').count();
            assert!(
                title_row(&rows).contains("Hol"),
                "width {width} lost the panel title: {:?}",
                title_row(&rows)
            );
            if arrows == 0 {
                continue;
            }
            assert_eq!(arrows, 1, "width {width} marked twice: {body:?}");
            assert!(
                body.contains("SYMBOL ▲"),
                "width {width} clipped the label off its own arrow: {body:?}"
            );
        }
        let marked = |width: u16| rows_at(width, 8, &app, draw_holdings).join("\n");
        assert!(marked(12).contains("SYMBOL ▲"), "{:?}", marked(12));
        assert!(!marked(11).contains('▲'), "{:?}", marked(11));
    }

    #[test]
    fn the_arrow_takes_the_accent_colour() {
        let _guard = crate::tui::settings::theme_guard(crate::tui::settings::Theme::DEFAULT);
        let app = loaded_app();
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| draw_positions(frame, frame.area(), &app))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let arrow = (0..24)
            .flat_map(|y| (0..80).map(move |x| (x, y)))
            .find(|&(x, y)| buffer[(x, y)].symbol() == "▼")
            .expect("the P&L arrow is drawn");
        assert_eq!(buffer[arrow].fg, crate::tui::format::accent());
        assert_eq!(
            SortDirection::Descending,
            positions_header_sort(PositionsSort::Pnl).direction
        );
    }
}
