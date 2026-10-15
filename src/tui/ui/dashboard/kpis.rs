use super::{signed_inr, tile};
use crate::tui::app::App;
use crate::tui::data::Holding;

use crate::tui::format;
use crate::tui::ui;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct HoldingsKpis {
    current_value: f64,
    invested: f64,
    day_pnl: f64,
    total_pnl: f64,
    prev_value: f64,
}

impl HoldingsKpis {
    fn day_pct(self) -> Option<f64> {
        (self.prev_value > 0.0).then_some(self.day_pnl / self.prev_value * 100.0)
    }
}

fn holdings_kpis(holdings: &[Holding], ltp: impl Fn(u32) -> Option<f64>) -> HoldingsKpis {
    let mut totals = HoldingsKpis::default();
    for holding in holdings {
        let live_ltp = ltp(holding.instrument_token);
        if !crate::tui::data::holding_has_pnl(holding, live_ltp) {
            continue;
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "overview totals use display-only f64 arithmetic"
        )]
        let quantity = crate::tui::data::holding_total_quantity(holding) as f64;
        let mark = crate::tui::data::holding_mark(holding, live_ltp);
        totals.current_value += mark * quantity;
        totals.invested += holding.average_price * quantity;
        totals.total_pnl += crate::tui::data::holding_live_pnl(holding, live_ltp);
        totals.day_pnl += crate::tui::data::holding_live_day_pnl(holding, live_ltp);
        totals.prev_value += holding.close_price * quantity;
    }
    totals
}

pub(in crate::tui::ui) fn draw_kpis(frame: &mut Frame, area: Rect, app: &App) {
    if area.is_empty() {
        return;
    }
    let [portfolio, day, total, funds] =
        Layout::horizontal([Constraint::Percentage(25); 4]).areas(area);

    let has_holdings = app.holdings.is_some();
    let holdings = app.holdings.as_deref().unwrap_or(&[]);
    let ltp = |token: u32| app.live_ltp(token);
    let totals = holdings_kpis(holdings, ltp);

    let headline_w = portfolio.width.saturating_sub(2) as usize;

    let (pv_value, pv_sub) = if has_holdings {
        let total = totals.current_value;
        let value = money_value(
            total,
            &format::inr(total),
            headline_w,
            format::header_style(),
        );
        let sub = (!holdings.is_empty()).then(|| {
            Line::from(Span::styled(
                format!("invested {}", format::inr(totals.invested)),
                format::dim(),
            ))
        });
        (value, sub)
    } else {
        (unknown_value(), None)
    };
    kpi_tile(frame, portfolio, "Portfolio Value", pv_value, pv_sub);

    let (dp_value, dp_sub) = if has_holdings {
        let total = totals.day_pnl;
        let value = money_value(
            total,
            &signed_inr(total),
            headline_w,
            format::change_style(total).add_modifier(Modifier::BOLD),
        );
        let pct_text = totals
            .day_pct()
            .map(|pct| format!("{} today", format::percent(pct)));
        let sub = pct_text.map(|text| Line::from(Span::styled(text, format::change_style(total))));
        (value, sub)
    } else {
        (unknown_value(), None)
    };
    kpi_tile(frame, day, "Day P&L", dp_value, dp_sub);

    let (tp_value, tp_sub) = if has_holdings {
        let total = totals.total_pnl;
        let value = money_value(
            total,
            &signed_inr(total),
            headline_w,
            format::change_style(total).add_modifier(Modifier::BOLD),
        );
        (value, None)
    } else {
        (unknown_value(), None)
    };
    kpi_tile(frame, total, "Total P&L", tp_value, tp_sub);

    let funds_value = match app.margin_available {
        Some(available) => money_value(
            available,
            &format::inr(available),
            headline_w,
            format::header_style(),
        ),
        None => unknown_value(),
    };
    kpi_tile(frame, funds, "Available Funds", funds_value, None);
}

const OVERFLOW: &str = "overflow";

const UNKNOWN: &str = "?";

fn money_value(value: f64, full: &str, width: usize, style: Style) -> Line<'static> {
    let fitted = format::fit_money(value, full, width);
    if format::display_width(&fitted) <= width {
        return Line::from(Span::styled(fitted, style));
    }
    if format::display_width(OVERFLOW) <= width {
        return Line::from(Span::styled(OVERFLOW, format::dim()));
    }
    unknown_value()
}

fn unknown_value() -> Line<'static> {
    Line::from(Span::styled(UNKNOWN, format::dim()))
}

fn kpi_tile(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    value: Line<'static>,
    subtitle: Option<Line<'static>>,
) {
    if area.is_empty() {
        return;
    }
    let block = tile(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.is_empty() {
        return;
    }

    let fits = |line: &Line| line.width() <= inner.width as usize;
    let value = value.alignment(Alignment::Right);
    let subtitle = subtitle
        .filter(fits)
        .unwrap_or_default()
        .alignment(Alignment::Right);
    let lines = match inner.height {
        1 => vec![value],
        2 => vec![value, subtitle],
        _ => vec![value, subtitle, Line::default()],
    };
    frame.render_widget(Paragraph::new(lines).style(ui::bar_style()), inner);
}

#[cfg(test)]
mod tests {
    use super::{HoldingsKpis, holdings_kpis, money_value};
    use crate::tui::app::App;
    use crate::tui::data::Holding;
    use crate::tui::format;
    use crate::tui::ui::dashboard::fixtures::{
        buffer_rows, find_once, pledged_holding, position, test_app,
    };
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn holding(
        token: u32,
        qty: i64,
        last_price: f64,
        close_price: f64,
        average_price: f64,
    ) -> Holding {
        let mut row = crate::tui::data::test_models::holding();
        row.tradingsymbol = "SYM".into();
        row.exchange = "NSE".into();
        row.instrument_token = token;
        row.quantity = qty;
        row.t1_quantity = 0;
        row.collateral_quantity = 0;
        row.mtf = None;
        row.average_price = average_price;
        row.last_price = last_price;
        row.close_price = close_price;
        row.day_change = None;
        row.discrepancy = false;
        row
    }

    #[test]
    fn holdings_kpis_match_kite_webs_holdings_aggregate() {
        let holdings = [
            holding(1, 5, 0.0, 200.0, 100.0),
            holding(2, 2, 50.0, 40.0, 30.0),
        ];
        let totals = holdings_kpis(&holdings, |token| (token == 1).then_some(210.0));
        assert_eq!(
            totals,
            HoldingsKpis {
                current_value: 1_150.0,
                invested: 560.0,
                day_pnl: 70.0,
                total_pnl: 590.0,
                prev_value: 1_080.0,
            }
        );
        assert_eq!(totals.day_pct(), Some(70.0 / 1_080.0 * 100.0));
    }

    #[test]
    fn holdings_kpis_exclude_discrepant_and_unpriced_rows_from_every_total() {
        let valid = holding(1, 5, 110.0, 100.0, 90.0);
        let mut discrepant = holding(2, 50, 1_000.0, 900.0, 800.0);
        discrepant.discrepancy = true;
        let unpriced = holding(3, 50, 0.0, 900.0, 800.0);
        let totals = holdings_kpis(&[valid], |_| None);
        assert_eq!(
            holdings_kpis(
                &[holding(1, 5, 110.0, 100.0, 90.0), discrepant, unpriced],
                |_| None
            ),
            totals
        );
    }

    #[test]
    fn holdings_kpis_are_zero_without_holdings() {
        assert_eq!(holdings_kpis(&[], |_| None), HoldingsKpis::default());
        assert_eq!(HoldingsKpis::default().day_pct(), None);
    }

    #[test]
    fn a_money_headline_degrades_to_overflow_then_unknown_but_never_clips() {
        let text = |line: ratatui::text::Line| -> String {
            line.spans.iter().map(|s| s.content.as_ref()).collect()
        };
        let plain = ratatui::style::Style::default();
        let big = 1_500_000_000.0;

        assert_eq!(
            text(money_value(840_500.0, &format::inr(840_500.0), 18, plain)),
            "₹8,40,500"
        );
        assert_eq!(
            text(money_value(big, &format::inr(big), 10, plain)),
            "+150.00Cr"
        );
        assert_eq!(
            text(money_value(big, &format::inr(big), 8, plain)),
            "overflow"
        );
        assert_eq!(text(money_value(big, &format::inr(big), 7, plain)), "?");
    }

    #[test]
    fn holdings_kpis_include_fully_pledged_holding() {
        let mut pledged = holding(1, 0, 100.0, 0.0, 0.0);
        pledged.collateral_quantity = 50; // free_quantity() == 0
        let totals = holdings_kpis(&[pledged], |_| None);
        assert_eq!(totals.current_value.to_bits(), 5000.0_f64.to_bits());
    }

    #[test]
    fn every_holdings_kpi_uses_free_pledged_and_mtf_quantity() {
        let mut mixed = holding(1, 5, 150.0, 140.0, 100.0);
        mixed.collateral_quantity = 20;
        mixed.mtf = Some({
            let mut mtf = crate::tui::data::test_models::mtf();
            mtf.quantity = 3;
            mtf
        });
        let totals = holdings_kpis(&[mixed], |_| None);
        assert_eq!(
            totals,
            HoldingsKpis {
                current_value: 4_200.0,
                invested: 2_800.0,
                day_pnl: 280.0,
                total_pnl: 1_400.0,
                prev_value: 3_920.0,
            }
        );
    }

    #[test]
    fn overview_kpis_do_not_include_open_positions() {
        let mut app = test_app();
        let mut holding = pledged_holding("INFY", 10, 0);
        holding.last_price = 110.0;
        holding.close_price = 90.0;
        app.holdings = Some(vec![holding]);
        app.positions = Some({
            let mut row = crate::tui::data::test_models::positions();
            row.net = vec![position("NIFTYFUT")];
            row
        });

        let mut terminal = Terminal::new(TestBackend::new(160, 5)).unwrap();
        terminal
            .draw(|frame| super::draw_kpis(frame, frame.area(), &app))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let rendered = (0..5)
            .map(|y| {
                (0..160)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("₹1,100"), "{rendered}");
        assert!(rendered.contains("invested ₹1,000"), "{rendered}");
        assert!(rendered.contains("+₹200"), "{rendered}");
        assert!(rendered.contains("+₹100"), "{rendered}");

        assert!(!rendered.contains("₹2,150"), "{rendered}");
        assert!(!rendered.contains("+₹150"), "{rendered}");
    }

    #[test]
    fn kpi_subtitle_is_dropped_when_it_cannot_fit() {
        let mut app = test_app();
        let mut holding = pledged_holding("BIG", 10_000, 0);
        holding.average_price = 1_000.0; // invested ₹1,00,00,000
        holding.last_price = 1_000.0;
        app.holdings = Some(vec![holding]);

        let rendered = |width: u16| -> String {
            let mut terminal = Terminal::new(TestBackend::new(width, 5)).unwrap();
            terminal
                .draw(|frame| super::draw_kpis(frame, frame.area(), &app))
                .unwrap();
            let buffer = terminal.backend().buffer().clone();
            (0..5)
                .map(|y| {
                    (0..width)
                        .map(|x| buffer[(x, y)].symbol())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        let wide = rendered(120);
        assert!(wide.contains("invested ₹1,00,00,000"), "{wide}");
        let narrow = rendered(80);
        assert!(!narrow.contains("invested"), "{narrow}");
        assert!(
            narrow.contains("₹1,00,00,000"),
            "headline must stay: {narrow}"
        );
    }

    fn kpi_app() -> App {
        let mut app = test_app();
        let mut holding = pledged_holding("KPI", 100, 0);
        holding.average_price = 8_500.0;
        holding.last_price = 8_405.0;
        holding.close_price = 8_383.5;
        app.holdings = Some(vec![holding]);
        app.margin_available = Some(50_000.0);
        app
    }

    fn kpi_rows(app: &App, width: u16, height: u16) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| super::draw_kpis(frame, frame.area(), app))
            .unwrap();
        buffer_rows(terminal.backend().buffer())
    }

    #[test]
    fn the_kpi_band_right_aligns_every_headline_at_eighty_columns() {
        let rows = kpi_rows(&kpi_app(), 80, 3);
        assert_eq!(
            rows[1],
            "│         ₹8,40,500││           +₹2,150││           -₹9,500││           ₹50,000│"
        );
    }

    #[test]
    fn the_kpi_band_stacks_headline_subtitle_and_a_blank_row_at_one_twenty() {
        let rows = kpi_rows(&kpi_app(), 120, 5);
        assert_eq!(
            rows[1],
            "│                   ₹8,40,500││                     +₹2,150│\
│                     -₹9,500││                     ₹50,000│"
        );
        assert_eq!(
            rows[2],
            "│          invested ₹8,50,000││                +0.26% today│\
│                            ││                            │"
        );
        assert_eq!(
            rows[3],
            "│                            ││                            │\
│                            ││                            │"
        );
    }

    #[test]
    fn a_short_tile_drops_the_blank_row_before_it_drops_the_subtitle() {
        let two = kpi_rows(&kpi_app(), 120, 4);
        assert!(two[1].contains("₹8,40,500"), "{:?}", two[1]);
        assert!(two[2].contains("invested ₹8,50,000"), "{:?}", two[2]);

        let one = kpi_rows(&kpi_app(), 120, 3);
        assert!(one[1].contains("₹8,40,500"), "{:?}", one[1]);
        assert!(!one.join("\n").contains("invested"), "{one:#?}");
    }

    #[test]
    fn an_unknown_kpi_says_so_rather_than_inventing_a_zero() {
        let rows = kpi_rows(&test_app(), 80, 3);
        assert_eq!(
            rows[1],
            "│                 ?││                 ?││                 ?││                 ?│"
        );
    }

    #[test]
    fn a_known_empty_book_reports_zero_while_absent_funds_stay_unknown() {
        let mut app = test_app();
        app.holdings = Some(Vec::new());
        let rows = kpi_rows(&app, 80, 3);
        assert_eq!(
            rows[1],
            "│                ₹0││               +₹0││               +₹0││                 ?│"
        );
    }

    #[test]
    fn the_funds_headline_is_neutral_while_pnl_keeps_its_semantic_colour() {
        let _guard = crate::tui::settings::theme_guard(crate::tui::settings::Theme::DEFAULT);
        let mut terminal = Terminal::new(TestBackend::new(80, 3)).unwrap();
        let app = kpi_app();
        terminal
            .draw(|frame| super::draw_kpis(frame, frame.area(), &app))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let rows = buffer_rows(&buffer);

        for (needle, fg) in [("+₹2,150", format::up()), ("-₹9,500", format::down())] {
            let (y, x) = find_once(&rows, needle);
            assert_eq!(buffer[(x, y)].fg, fg, "{needle} colour");
            assert!(
                buffer[(x, y)]
                    .modifier
                    .contains(ratatui::style::Modifier::BOLD),
                "{needle} weight"
            );
        }

        let (y, x) = find_once(&rows, "₹50,000");
        assert_ne!(
            buffer[(x, y)].fg,
            format::accent(),
            "funds must read as a value, not as chrome"
        );
        assert!(
            buffer[(x, y)]
                .modifier
                .contains(ratatui::style::Modifier::BOLD)
        );
    }
}
