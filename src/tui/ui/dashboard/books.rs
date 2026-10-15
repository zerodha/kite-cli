use super::signed_inr;
use crate::tui::app::App;
use crate::tui::data::Position;

use crate::tui::format;
use crate::tui::ui::columns::{
    self, FitCol, col, fit_constraints, fit_header, fit_row, right_styled, table_budget,
};
use crate::tui::ui::{self, HitAction, HitList};
use crate::tui::workspace::PaneId;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Cell, HighlightSpacing, Row, Table, TableState};

fn book_rows_hit(body: Rect, count: usize, offset: usize) -> Option<Rect> {
    let visible = u16::try_from(count.saturating_sub(offset)).unwrap_or(u16::MAX);
    let height = body.height.min(visible);
    (height > 0 && body.width > 0).then_some(Rect { height, ..body })
}

const PANEL_SYM_MIN: u16 = 8;
const PANEL_SYM_MAX: u16 = 16;
const PANEL_PNL_W: u16 = 13;

const PANEL_POS_COLS: [FitCol; 6] = [
    col(PANEL_SYM_MIN, 0), // SYMBOL
    col(7, 3),             // QTY
    col(11, 5),            // AVG
    col(11, 2),            // LTP
    col(PANEL_PNL_W, 1),   // P&L
    col(9, 4),             // CHG%
];

const PANEL_POS_LABELS: [(&str, bool); 6] = [
    ("SYMBOL", false),
    ("QTY", true),
    ("AVG", true),
    ("LTP", true),
    ("P&L", true),
    ("CHG%", true),
];

fn position_panel_row(
    app: &App,
    position: &Position,
    fit: &columns::ColumnFit,
    sym_w: u16,
) -> (Row<'static>, f64) {
    let live = app.live_ltp(position.instrument_token);
    let ltp = live.unwrap_or(position.last_price);
    let pnl = crate::tui::data::position_live_pnl(position, live);
    let day_pct = (position.close_price > 0.0)
        .then(|| (ltp - position.close_price) / position.close_price * 100.0);
    let qty_style = if position.quantity == 0 {
        format::dim()
    } else {
        Style::default()
    };
    let row = fit_row(
        fit,
        &[
            &|| {
                Cell::from(format::pad_or_clip(
                    &position.tradingsymbol,
                    usize::from(sym_w),
                ))
            },
            &|| right_styled(position.quantity.to_string(), qty_style),
            &|| right_styled(format::price(position.average_price), format::dim()),
            &|| right_styled(format::price(ltp), Style::default()),
            &|| {
                right_styled(
                    format::fit_money(pnl, &format::signed_price(pnl), PANEL_PNL_W as usize),
                    format::change_style(pnl),
                )
            },
            &|| {
                right_styled(
                    day_pct.map_or_else(|| ".".into(), format::percent),
                    day_pct.map(format::change_style).unwrap_or_default(),
                )
            },
        ],
    );
    (row, pnl)
}

pub(in crate::tui::ui) fn draw_positions_panel(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    focused: bool,
) {
    let positions = app.positions.as_ref().map(|p| p.net.as_slice());
    let count = positions.map_or(0, <[Position]>::len);
    let block = ui::focus_panel(format!("Positions ({count})"), focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(positions) = positions.filter(|p| !p.is_empty()) else {
        let headline = if app.positions.is_none() {
            "Positions not available yet"
        } else {
            "No positions"
        };
        ui::state_message(frame, inner, ui::MessageTone::Normal, headline, None, None);
        return;
    };

    let fit = columns::fit_columns(table_budget(area.width), 1, &PANEL_POS_COLS);
    let layout = columns::spread(
        &fit,
        &PANEL_POS_COLS,
        &[columns::text(
            0,
            columns::widest(
                positions
                    .iter()
                    .map(|position| format::display_width(&position.tradingsymbol)),
            ),
            PANEL_SYM_MAX,
        )],
    );
    let mut total_pnl = 0.0;
    let rows: Vec<Row> = positions
        .iter()
        .map(|position| {
            let (row, pnl) = position_panel_row(app, position, &fit, layout.width(0));
            total_pnl += pnl;
            row
        })
        .collect();

    let footer = fit_row(
        &fit,
        &[
            &|| {
                Cell::from(Span::styled(
                    "TOTAL",
                    format::dim().add_modifier(Modifier::BOLD),
                ))
            },
            &Cell::default,
            &Cell::default,
            &Cell::default,
            &|| {
                right_styled(
                    format::fit_money(total_pnl, &signed_inr(total_pnl), PANEL_PNL_W as usize),
                    format::change_style(total_pnl).add_modifier(Modifier::BOLD),
                )
            },
            &Cell::default,
        ],
    )
    .top_margin(1);
    let table = Table::new(rows, fit_constraints(&fit, &layout))
        .header(fit_header(&fit, &PANEL_POS_LABELS))
        .footer(footer)
        .column_spacing(layout.spacing)
        .row_highlight_style(format::selection_style())
        .highlight_symbol(layout.highlight_symbol())
        .highlight_spacing(HighlightSpacing::Always);

    let viewport = app.books.overview_positions.viewport();
    let mut state = TableState::default().with_offset(viewport.get());
    if focused && count > 0 {
        state.select(Some(app.overview_positions_row().min(count - 1)));
    }
    frame.render_stateful_widget(table, inner, &mut state);
    viewport.set(state.offset());

    let body = Rect {
        x: inner.x,
        y: inner.y.saturating_add(1),
        width: inner.width,
        height: inner.height.saturating_sub(3),
    };
    if let Some(rows) = book_rows_hit(body, count, state.offset()) {
        app.hit.borrow_mut().push(
            rows,
            HitAction::Rows {
                list: HitList::Pane(PaneId::OverviewPositions),
                offset: state.offset(),
            },
        );
    }
}

const PANEL_HOLD_COLS: [FitCol; 6] = [
    col(PANEL_SYM_MIN, 0), // SYMBOL
    col(7, 3),             // QTY
    col(11, 2),            // LTP
    col(13, 4),            // VALUE
    col(PANEL_PNL_W, 1),   // P&L
    col(9, 5),             // P&L%
];

const PANEL_HOLD_LABELS: [(&str, bool); 6] = [
    ("SYMBOL", false),
    ("QTY", true),
    ("LTP", true),
    ("VALUE", true),
    ("P&L", true),
    ("P&L%", true),
];

fn holding_row(
    app: &App,
    h: &crate::tui::data::Holding,
    fit: &columns::ColumnFit,
    sym_w: u16,
) -> Row<'static> {
    let ltp = app.live_ltp(h.instrument_token).unwrap_or(h.last_price);
    let qty = crate::tui::data::holding_total_quantity(h);
    #[expect(
        clippy::cast_precision_loss,
        reason = "holding value is display-only f64 arithmetic"
    )]
    let value = ltp * qty as f64;
    let pnl = crate::tui::data::holding_live_pnl(h, Some(ltp));
    let pnl_pct =
        (h.average_price > 0.0).then(|| (ltp - h.average_price) / h.average_price * 100.0);
    fit_row(
        fit,
        &[
            &|| Cell::from(format::pad_or_clip(&h.tradingsymbol, sym_w as usize)),
            &|| right_styled(qty.to_string(), format::dim()),
            &|| right_styled(format::price(ltp), Style::default()),
            &|| {
                right_styled(
                    format::fit_money(
                        value,
                        &format::inr(value),
                        PANEL_HOLD_COLS[3].width as usize,
                    ),
                    Style::default(),
                )
            },
            &|| {
                right_styled(
                    format::fit_money(pnl, &format::signed_price(pnl), PANEL_PNL_W as usize),
                    format::change_style(pnl),
                )
            },
            &|| {
                right_styled(
                    pnl_pct.map_or_else(|| ".".into(), format::percent),
                    pnl_pct.map(format::change_style).unwrap_or_default(),
                )
            },
        ],
    )
}

pub(in crate::tui::ui) fn draw_holdings_panel(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    focused: bool,
) {
    let holdings = app.holdings.as_deref();
    let order = app.overview_holdings_display_order();
    let count = order.len();
    let block = ui::focus_panel(format!("Holdings ({count})"), focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(holdings) = holdings.filter(|_| count > 0) else {
        let headline = if app.holdings.is_none() {
            "Holdings not available yet"
        } else {
            "No holdings"
        };
        ui::state_message(frame, inner, ui::MessageTone::Normal, headline, None, None);
        return;
    };

    let fit = columns::fit_columns(table_budget(area.width), 1, &PANEL_HOLD_COLS);
    let layout = columns::spread(
        &fit,
        &PANEL_HOLD_COLS,
        &[columns::text(
            0,
            columns::widest(
                order
                    .iter()
                    .map(|&i| format::display_width(&holdings[i].tradingsymbol)),
            ),
            PANEL_SYM_MAX,
        )],
    );
    let sym_w = layout.width(0);
    let rows: Vec<Row> = order
        .iter()
        .map(|&i| holding_row(app, &holdings[i], &fit, sym_w))
        .collect();

    let table = Table::new(rows, fit_constraints(&fit, &layout))
        .header(fit_header(&fit, &PANEL_HOLD_LABELS))
        .column_spacing(layout.spacing)
        .row_highlight_style(format::selection_style())
        .highlight_symbol(layout.highlight_symbol())
        .highlight_spacing(HighlightSpacing::Always);

    let viewport = app.books.overview_holdings.viewport();
    let mut state = TableState::default().with_offset(viewport.get());
    if focused {
        state.select(Some(app.overview_holdings_row(&order).min(count - 1)));
    }
    frame.render_stateful_widget(table, inner, &mut state);
    viewport.set(state.offset());

    let body = Rect {
        x: inner.x,
        y: inner.y.saturating_add(1),
        width: inner.width,
        height: inner.height.saturating_sub(1),
    };
    if let Some(rows) = book_rows_hit(body, count, state.offset()) {
        app.hit.borrow_mut().push(
            rows,
            HitAction::Rows {
                list: HitList::Pane(PaneId::OverviewHoldings),
                offset: state.offset(),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{draw_holdings_panel, draw_positions_panel};
    use crate::tui::app::App;
    use crate::tui::data::Holding;
    use crate::tui::format;
    use crate::tui::ui::HitAction;
    use crate::tui::ui::dashboard::fixtures::{
        book_app, buffer_rows, find_all, find_once, pledged_holding, position, root_buffer,
        test_app,
    };
    use crate::tui::workspace::PaneId;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn symbol_start_x(focused: bool) -> u16 {
        let mut app = test_app();
        app.positions = Some({
            let mut row = crate::tui::data::test_models::positions();
            row.net = vec![position("INFY")];
            row
        });
        let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
        terminal
            .draw(|frame| draw_positions_panel(frame, frame.area(), &app, focused))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let row: String = (0..40).map(|x| buffer[(x, 2)].symbol()).collect();
        row.find("INFY")
            .map(|byte_idx| u16::try_from(row[..byte_idx].chars().count()).unwrap())
            .expect("row has INFY content")
    }

    #[test]
    fn focus_toggle_does_not_shift_columns() {
        assert_eq!(symbol_start_x(true), symbol_start_x(false));
    }

    #[test]
    fn holdings_panel_title_reports_full_row_count() {
        let mut app = test_app();
        app.holdings = Some(vec![pledged_holding("MIXED", 5, 3)]);

        let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
        terminal
            .draw(|frame| draw_holdings_panel(frame, frame.area(), &app, true))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let title: String = (0..40).map(|x| buffer[(x, 0)].symbol()).collect();

        assert!(
            title.contains("Holdings (1)"),
            "unexpected title: {title:?}"
        );
        assert!(!title.contains("free qty only"));
    }

    const SIZES: [(u16, u16); 2] = [(80, 24), (120, 35)];

    #[test]
    fn the_focused_book_marks_its_title_and_the_other_does_not() {
        for (width, height) in SIZES {
            for (pane, focused_title, inactive_title) in [
                (
                    PaneId::OverviewPositions,
                    "> Positions (2)",
                    "   Holdings (3)",
                ),
                (
                    PaneId::OverviewHoldings,
                    "> Holdings (3)",
                    "   Positions (2)",
                ),
            ] {
                let app = book_app(pane);
                let rows = buffer_rows(&root_buffer(&app, width, height));
                find_once(&rows, focused_title);
                find_once(&rows, inactive_title);
                let markers: usize = rows.iter().map(|row| row.matches("> ").count()).sum();
                assert_eq!(markers, 1, "{pane:?} at {width}x{height}: {rows:#?}");
            }
        }
    }

    #[test]
    fn the_focused_book_takes_the_accent_border_and_the_other_the_theme_border() {
        let _guard = crate::tui::settings::theme_guard(crate::tui::settings::Theme::DEFAULT);
        let inactive = format::border_style()
            .fg
            .expect("the border style names a colour");
        for (width, height) in SIZES {
            for pane in [PaneId::OverviewPositions, PaneId::OverviewHoldings] {
                let buffer = root_buffer(&book_app(pane), width, height);
                let rows = buffer_rows(&buffer);
                let (focused_y, _) = find_once(&rows, "> ");
                let (inactive_y, _) = find_once(
                    &rows,
                    if pane == PaneId::OverviewHoldings {
                        "   Positions"
                    } else {
                        "   Holdings"
                    },
                );
                assert_eq!(
                    buffer[(0, focused_y)].fg,
                    format::accent(),
                    "{pane:?} at {width}x{height}: focused border"
                );
                assert_eq!(
                    buffer[(0, inactive_y)].fg,
                    inactive,
                    "{pane:?} at {width}x{height}: inactive border"
                );
            }
        }
    }

    #[test]
    fn the_focus_marker_does_not_move_the_titles_or_the_columns() {
        for (width, height) in SIZES {
            let with_positions = buffer_rows(&root_buffer(
                &book_app(PaneId::OverviewPositions),
                width,
                height,
            ));
            let with_holdings = buffer_rows(&root_buffer(
                &book_app(PaneId::OverviewHoldings),
                width,
                height,
            ));
            for label in ["Positions (2)", "Holdings (3)", "SYMBOL", "P&L"] {
                let a = find_all(&with_positions, label);
                let b = find_all(&with_holdings, label);
                assert!(!a.is_empty(), "no {label} at {width}x{height}");
                assert_eq!(a, b, "{label} moved at {width}x{height}");
            }
        }
    }

    #[test]
    fn an_empty_or_loading_book_keeps_the_focus_marker() {
        for (positions, holdings, message) in [
            (
                Some({
                    let mut row = crate::tui::data::test_models::positions();
                    row.net = Vec::new();
                    row
                }),
                Some(Vec::new()),
                "No holdings",
            ),
            (None, None, "Holdings not available yet"),
        ] {
            for (width, height) in SIZES {
                let mut app = book_app(PaneId::OverviewHoldings);
                app.positions = positions.clone();
                app.holdings = holdings.clone();
                let rows = buffer_rows(&root_buffer(&app, width, height));
                find_once(&rows, "> Holdings (0)");
                find_once(&rows, message);
                let markers: usize = rows.iter().map(|row| row.matches("> ").count()).sum();
                assert_eq!(markers, 1, "at {width}x{height}: {rows:#?}");
            }
        }
    }

    #[test]
    fn focus_does_not_change_a_records_pnl_colour() {
        let _guard = crate::tui::settings::theme_guard(crate::tui::settings::Theme::DEFAULT);
        for pane in [PaneId::OverviewPositions, PaneId::OverviewHoldings] {
            let buffer = root_buffer(&book_app(pane), 80, 24);
            let rows = buffer_rows(&buffer);
            let (y, x) = find_all(&rows, "+10.00")[0];
            assert_eq!(
                buffer[(x, y)].fg,
                format::up(),
                "{pane:?}: a gain stays green whichever book is focused"
            );
        }
    }

    #[test]
    fn every_preset_separates_the_focus_border_from_the_inactive_one() {
        for (name, theme) in crate::tui::settings::PRESETS {
            assert_ne!(theme.accent, theme.border, "{name}: focus border");
            assert_ne!(Some(theme.accent), theme.background, "{name}: title");
        }
    }

    fn reported_holding() -> Holding {
        let mut holding = pledged_holding("DIVISLAB", 15, 0);
        holding.average_price = 6_077.93;
        holding.last_price = 30_000.0;
        holding.close_price = 25_000.0;
        holding
    }

    #[test]
    fn overview_panels_never_render_a_partial_money_cell() {
        type DrawPanel = fn(&mut ratatui::Frame, ratatui::layout::Rect, &App, bool);
        let mut app = test_app();
        app.holdings = Some(vec![reported_holding()]);
        app.positions = Some({
            let mut row = crate::tui::data::test_models::positions();
            row.net = vec![{
                let mut row = position("DIVISLAB");
                row.pnl = 358_831.05;
                row.last_price = 30_000.0;
                row.close_price = 25_000.0;
                row.average_price = 6_077.93;
                row.quantity = 15;
                row
            }];
            row
        });

        let panels: [(&str, DrawPanel); 2] = [
            ("positions", draw_positions_panel),
            ("holdings", draw_holdings_panel),
        ];
        for (name, draw) in panels {
            for width in 20..=200u16 {
                let mut terminal = Terminal::new(TestBackend::new(width, 8)).unwrap();
                terminal
                    .draw(|frame| draw(frame, frame.area(), &app, true))
                    .unwrap();
                let buffer = terminal.backend().buffer().clone();
                for y in 0..8 {
                    let row: String = (0..width).map(|x| buffer[(x, y)].symbol()).collect();
                    for (full, tail) in [
                        ("+3,58,831.05", "58,831.05"),
                        ("30,000.00", ",000.00"),
                        ("6,077.93", "077.93"),
                        ("₹4,50,000", "50,000"),
                    ] {
                        assert!(
                            row.contains(full) || !row.contains(tail),
                            "{name} at {width}, row {y}: partial {full} ({tail}): {row:?}"
                        );
                    }
                }
            }
        }
    }

    fn panel_rows(
        app: &App,
        width: u16,
        height: u16,
        draw: fn(&mut ratatui::Frame, ratatui::layout::Rect, &App, bool),
    ) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| draw(frame, frame.area(), app, true))
            .unwrap();
        buffer_rows(terminal.backend().buffer())
    }

    #[test]
    fn a_book_separates_an_unknown_source_from_an_empty_one() {
        let mut app = book_app(PaneId::OverviewPositions);

        app.positions = None;
        app.holdings = None;
        let positions = panel_rows(&app, 60, 8, draw_positions_panel);
        assert_eq!(positions[1].trim_matches('│').trim(), "");
        assert!(
            positions[2].starts_with("│ Positions not available yet"),
            "{:?}",
            positions[2]
        );
        let holdings = panel_rows(&app, 60, 8, draw_holdings_panel);
        assert!(
            holdings[2].starts_with("│ Holdings not available yet"),
            "{:?}",
            holdings[2]
        );

        app.positions = Some({
            let mut row = crate::tui::data::test_models::positions();
            row.net = Vec::new();
            row
        });
        app.holdings = Some(Vec::new());
        let positions = panel_rows(&app, 60, 8, draw_positions_panel);
        assert!(
            positions[2].starts_with("│ No positions"),
            "{:?}",
            positions[2]
        );
        let holdings = panel_rows(&app, 60, 8, draw_holdings_panel);
        assert!(
            holdings[2].starts_with("│ No holdings"),
            "{:?}",
            holdings[2]
        );
    }

    #[test]
    fn an_empty_book_keeps_its_focus_target_and_records_no_rows() {
        let mut app = book_app(PaneId::OverviewHoldings);
        app.positions = Some({
            let mut row = crate::tui::data::test_models::positions();
            row.net = Vec::new();
            row
        });
        root_buffer(&app, 120, 35);

        let hit = app.hit.borrow();
        let book = hit
            .regions
            .iter()
            .find(|region| region.action == HitAction::Pane(PaneId::OverviewPositions))
            .expect("the empty book records its pane");
        let body = (book.rect.x + 2, book.rect.y + 2);
        assert_eq!(
            hit.at(body.0, body.1).map(|region| region.action),
            Some(HitAction::Pane(PaneId::OverviewPositions))
        );
        assert!(
            !hit.regions.iter().any(|region| matches!(
                region.action,
                HitAction::Rows {
                    list: crate::tui::ui::HitList::Pane(PaneId::OverviewPositions),
                    ..
                }
            )),
            "an empty book has no row to click: {:#?}",
            hit.regions
        );
    }
}
