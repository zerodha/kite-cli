use crate::tui::format;
use kiteconnect::ticker::Tick;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

const BAR_BLOCKS: [char; 8] = ['▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];

const DEPTH_BAR_W: usize = 3;
const DEPTH_QTY_W: usize = 7;
const DEPTH_PRICE_W: usize = 9;
const DEPTH_GAP: &str = "  ";

#[derive(Debug, Clone, Copy)]
struct Fit {
    bars: bool,
    qty: bool,
    bid_w: usize,
    ask_w: usize,
    bid_qty_w: usize,
    ask_qty_w: usize,
}

impl Fit {
    fn for_width(width: u16, [bid_qty_w, bid_w, ask_w, ask_qty_w]: [usize; 4]) -> Self {
        let width = usize::from(width);
        let prices = bid_w + DEPTH_GAP.len() + ask_w;
        if width < prices {
            let cell = width.saturating_sub(DEPTH_GAP.len()) / 2;
            return Self {
                bars: false,
                qty: false,
                bid_w: cell,
                ask_w: cell,
                bid_qty_w,
                ask_qty_w,
            };
        }
        let with_qty = prices + bid_qty_w + ask_qty_w + 2;
        Self {
            bars: width >= with_qty + 2 * DEPTH_BAR_W,
            qty: width >= with_qty,
            bid_w,
            ask_w,
            bid_qty_w,
            ask_qty_w,
        }
    }

    fn bid_qty_cell(self, text: &str) -> String {
        let width = self.bid_qty_w;
        format!("{text:>width$}")
    }

    fn ask_qty_cell(self, text: &str) -> String {
        let width = self.ask_qty_w;
        format!("{text:<width$}")
    }

    fn bid_cell(self, text: &str) -> String {
        let width = self.bid_w;
        format!("{:>width$}", whole_or_marker(text, width))
    }

    fn ask_cell(self, text: &str) -> String {
        let width = self.ask_w;
        format!("{:<width$}", whole_or_marker(text, width))
    }

    fn line(self, [bid_bar, bid_qty, bid, ask, ask_qty, ask_bar]: [Span<'_>; 6]) -> Line<'_> {
        let mut spans = Vec::with_capacity(9);
        if self.bars {
            spans.push(bid_bar);
        }
        if self.qty {
            spans.extend([bid_qty, Span::raw(" ")]);
        }
        spans.extend([bid, Span::raw(DEPTH_GAP), ask]);
        if self.qty {
            spans.extend([Span::raw(" "), ask_qty]);
        }
        if self.bars {
            spans.push(ask_bar);
        }
        Line::from(spans)
    }
}

fn whole_or_marker(text: &str, width: usize) -> &str {
    if text.chars().count() <= width {
        text
    } else {
        "…"
    }
}

fn price_width(prices: &[kiteconnect::ticker::DepthEntry]) -> usize {
    prices
        .iter()
        .take(5)
        .map(|entry| format::price(entry.price).chars().count())
        .fold(DEPTH_PRICE_W, usize::max)
}

fn qty_width(levels: &[kiteconnect::ticker::DepthEntry], total: Option<&str>) -> usize {
    levels
        .iter()
        .take(5)
        .map(|entry| {
            format::group_indian(u64::from(entry.quantity))
                .chars()
                .count()
        })
        .chain(total.map(|total| total.chars().count()))
        .fold(DEPTH_QTY_W, usize::max)
}

fn depth_bar(value: u64, max: u64, width: usize, grow_right: bool) -> String {
    if width == 0 || max == 0 || value == 0 {
        return " ".repeat(width);
    }
    let total_eighths = width * 8;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "depth ratio rounds to clamped eighth-cell glyphs"
    )]
    let filled = (((value as f64 / max as f64) * total_eighths as f64).round() as usize)
        .clamp(1, total_eighths);
    let full = filled / 8;
    let rem = filled % 8;

    let mut bar = String::with_capacity(width);
    if grow_right {
        bar.extend(std::iter::repeat_n('█', full));
        if rem > 0 {
            bar.push(BAR_BLOCKS[rem - 1]);
        }
        bar.extend(std::iter::repeat_n(
            ' ',
            width - full - usize::from(rem > 0),
        ));
    } else {
        let tip = match rem {
            0 => None,
            1..=3 => Some('▕'),
            _ => Some('▐'),
        };
        let cells = full + usize::from(tip.is_some());
        bar.extend(std::iter::repeat_n(' ', width - cells));
        if let Some(tip) = tip {
            bar.push(tip);
        }
        bar.extend(std::iter::repeat_n('█', full));
    }
    bar
}

pub(super) fn draw_depth(
    frame: &mut Frame,
    area: Rect,
    depth: &kiteconnect::ticker::MarketDepth,
    tick: &Tick,
    circuit: Option<(f64, f64)>,
) {
    let bid_style = Style::default().fg(format::up());
    let ask_style = Style::default().fg(format::down());
    let header_style = format::header_style();

    let max_qty = depth
        .buy
        .iter()
        .chain(depth.sell.iter())
        .map(|e| u64::from(e.quantity))
        .max()
        .unwrap_or(0);

    let totals = tick
        .buy_quantity
        .zip(tick.sell_quantity)
        .map(|(buy, sell)| (format::compact_qty(buy), format::compact_qty(sell)));
    let fit = Fit::for_width(
        area.width,
        [
            qty_width(&depth.buy, totals.as_ref().map(|(buy, _)| buy.as_str())),
            price_width(&depth.buy),
            price_width(&depth.sell),
            qty_width(&depth.sell, totals.as_ref().map(|(_, sell)| sell.as_str())),
        ],
    );
    let mut lines: Vec<Line> = Vec::with_capacity(8);
    lines.push(fit.line([
        Span::styled(" ".repeat(DEPTH_BAR_W), header_style),
        Span::styled(fit.bid_qty_cell("QTY"), header_style),
        Span::styled(fit.bid_cell("BID"), header_style),
        Span::styled(fit.ask_cell("ASK"), header_style),
        Span::styled(fit.ask_qty_cell("QTY"), header_style),
        Span::raw(""),
    ]));

    for level in 0..5 {
        let bid = depth.buy.get(level);
        let ask = depth.sell.get(level);
        let bid_qty = bid.map_or(0, |e| u64::from(e.quantity));
        let ask_qty = ask.map_or(0, |e| u64::from(e.quantity));
        let bid_qty_text = bid
            .map(|_| format::group_indian(bid_qty))
            .unwrap_or_default();
        let ask_qty_text = ask
            .map(|_| format::group_indian(ask_qty))
            .unwrap_or_default();
        lines.push(fit.line([
            Span::styled(depth_bar(bid_qty, max_qty, DEPTH_BAR_W, false), bid_style),
            Span::raw(fit.bid_qty_cell(&bid_qty_text)),
            Span::styled(
                fit.bid_cell(&bid.map(|e| format::price(e.price)).unwrap_or_default()),
                bid_style,
            ),
            Span::styled(
                fit.ask_cell(&ask.map(|e| format::price(e.price)).unwrap_or_default()),
                ask_style,
            ),
            Span::raw(fit.ask_qty_cell(&ask_qty_text)),
            Span::styled(depth_bar(ask_qty, max_qty, DEPTH_BAR_W, true), ask_style),
        ]));
    }

    if let (Some(buy), Some(sell), Some((buy_total, sell_total))) =
        (tick.buy_quantity, tick.sell_quantity, totals)
    {
        lines.push(Line::from(""));
        lines.push(depth_totals_line(
            &fit,
            buy,
            sell,
            &buy_total,
            &sell_total,
            bid_style,
            ask_style,
        ));
    }

    if let Some((lower, upper)) = circuit {
        let (lower, upper) = (format::price(lower), format::price(upper));
        let (bid_cell, ask_cell) = if fit.qty {
            (fit.bid_cell(&lower), fit.ask_cell(&upper))
        } else {
            (
                fit.bid_cell(&format!("LC {lower}")),
                fit.ask_cell(&format!("{upper} UC")),
            )
        };
        lines.push(fit.line([
            Span::raw(" ".repeat(DEPTH_BAR_W)),
            Span::styled(fit.bid_qty_cell("LC"), format::dim()),
            Span::raw(bid_cell),
            Span::raw(ask_cell),
            Span::styled(fit.ask_qty_cell("UC"), format::dim()),
            Span::raw(" ".repeat(DEPTH_BAR_W)),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), area);
}

fn depth_totals_line(
    fit: &Fit,
    buy: u64,
    sell: u64,
    buy_total: &str,
    sell_total: &str,
    bid_style: Style,
    ask_style: Style,
) -> Line<'static> {
    let total_max = buy.max(sell);
    let (bid_cell, ask_cell) = if fit.qty {
        (
            Span::styled(fit.bid_cell("total"), format::dim()),
            Span::styled(fit.ask_cell("total"), format::dim()),
        )
    } else {
        (
            Span::styled(fit.bid_cell(buy_total), bid_style),
            Span::styled(fit.ask_cell(sell_total), ask_style),
        )
    };
    fit.line([
        Span::styled(depth_bar(buy, total_max, DEPTH_BAR_W, false), bid_style),
        Span::styled(fit.bid_qty_cell(buy_total), bid_style),
        bid_cell,
        ask_cell,
        Span::styled(fit.ask_qty_cell(sell_total), ask_style),
        Span::styled(depth_bar(sell, total_max, DEPTH_BAR_W, true), ask_style),
    ])
}

#[cfg(test)]
mod tests {
    use super::depth_bar;

    #[test]
    fn depth_bar_scales_proportionally() {
        assert_eq!(depth_bar(0, 100, 5, true), "     ");
        assert_eq!(depth_bar(50, 0, 5, true), "     ");

        assert_eq!(depth_bar(100, 100, 5, true), "█████");

        assert_eq!(depth_bar(50, 100, 5, true), "██▌  ");

        assert_eq!(depth_bar(50, 100, 5, false), "  ▐██");
        assert_eq!(depth_bar(100, 100, 5, false), "█████");

        assert_eq!(depth_bar(1, 1_000_000, 5, true), "▏    ");
        assert_eq!(depth_bar(1, 1_000_000, 5, false), "    ▕");

        assert_eq!(depth_bar(50, 100, 0, true), "");
    }
}

#[cfg(test)]
mod depth_render_tests {
    use super::{DEPTH_BAR_W, DEPTH_GAP, DEPTH_PRICE_W, DEPTH_QTY_W, draw_depth};
    use crate::tui::data::test_models::depth_entry;
    use crate::tui::format;
    use kiteconnect::ticker::{MarketDepth, Tick};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;

    #[test]
    fn depth_ladder_separates_qty_from_price_at_high_prices() {
        let mut depth = MarketDepth::default();
        depth.buy = vec![depth_entry(1_234, 12_345.67, 1)];
        depth.sell = vec![depth_entry(5_678, 23_456.78, 1)];
        let tick = Tick::default();

        let mut terminal = Terminal::new(TestBackend::new(42, 8)).unwrap();
        terminal
            .draw(|frame| draw_depth(frame, Rect::new(0, 0, 42, 8), &depth, &tick, None))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();

        let cells: Vec<char> = (0..42)
            .map(|x| buffer[(x, 1)].symbol().chars().next().unwrap_or(' '))
            .collect();
        let bid_sep = DEPTH_BAR_W + DEPTH_QTY_W;
        let ask_sep = bid_sep + 1 + DEPTH_PRICE_W + DEPTH_GAP.len() + DEPTH_PRICE_W;

        assert_eq!(
            cells[bid_sep], ' ',
            "qty/bid-price separator missing: {cells:?}"
        );
        assert_eq!(
            cells[ask_sep], ' ',
            "ask-price/qty separator missing: {cells:?}"
        );

        let bid_price: String = cells[bid_sep + 1..bid_sep + 1 + DEPTH_PRICE_W]
            .iter()
            .collect();
        let ask_price: String = cells[ask_sep - DEPTH_PRICE_W..ask_sep].iter().collect();
        assert_eq!(bid_price, "12,345.67");
        assert_eq!(ask_price, "23,456.78");
    }

    fn first_level_row(bid: f64, ask: f64, width: u16) -> String {
        let mut depth = MarketDepth::default();
        depth.buy = vec![depth_entry(10, bid, 1)];
        depth.sell = vec![depth_entry(10, ask, 1)];
        let mut terminal = Terminal::new(TestBackend::new(width, 8)).unwrap();
        terminal
            .draw(|frame| draw_depth(frame, frame.area(), &depth, &Tick::default(), None))
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..width).map(|x| buffer[(x, 1)].symbol()).collect()
    }

    #[test]
    fn a_ladder_too_narrow_for_both_prices_draws_markers_not_digits() {
        for (bid, ask, fits, narrow) in [
            (12_345.67, 23_456.78, 20, 18),
            (123_456.78, 234_567.89, 24, 22),
        ] {
            let row = first_level_row(bid, ask, fits);
            assert!(
                row.contains(&format::price(bid)) && row.contains(&format::price(ask)),
                "{fits} cells hold both prices whole: {row:?}"
            );
            let row = first_level_row(bid, ask, narrow);
            assert!(
                !row.chars().any(|c| c.is_ascii_digit()),
                "{narrow} cells drew a partial price: {row:?}"
            );
            assert_eq!(row.matches('…').count(), 2, "{row:?}");
        }
    }

    fn ladder_rows(depth: &MarketDepth, tick: &Tick, width: u16) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(width, 8)).unwrap();
        terminal
            .draw(|frame| draw_depth(frame, frame.area(), depth, tick, None))
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..8)
            .map(|y| (0..width).map(|x| buffer[(x, y)].symbol()).collect())
            .collect()
    }

    #[test]
    fn a_wide_quantity_draws_whole_or_gives_way_to_the_prices() {
        let mut depth = MarketDepth::default();
        depth.buy = vec![depth_entry(1_234_567, 1_234.50, 3)];
        depth.sell = vec![depth_entry(2_345_678, 1_235.50, 2)];
        let mut tick = Tick::default();
        tick.buy_quantity = Some(12_000);
        tick.sell_quantity = Some(9_000);
        let whole = [
            "12,34,567",
            "23,45,678",
            "1,234.50",
            "1,235.50",
            "12,000",
            "9,000",
            "QTY",
            "BID",
            "ASK",
            "total",
            "…",
        ];
        for width in [78, 36, 26, 16] {
            let rows = ladder_rows(&depth, &tick, width);
            for row in &rows {
                for token in row.split(|c: char| !c.is_ascii_graphic() && c != '…') {
                    assert!(
                        token.is_empty() || whole.contains(&token),
                        "{width} cells drew a partial number {token:?}:\n{}",
                        rows.join("\n")
                    );
                }
            }
            let prices = rows[1].contains("1,234.50") && rows[1].contains("1,235.50");
            let markers = rows[1].matches('…').count() == 2;
            assert!(
                if width > 16 { prices } else { markers },
                "{width} cells lost a price:\n{}",
                rows.join("\n")
            );
        }

        let rows = ladder_rows(&depth, &tick, 78);
        let cells: Vec<Vec<char>> = rows.iter().map(|row| row.chars().collect()).collect();
        let column = |row: usize, at: usize| -> String { cells[row][at..at + 9].iter().collect() };
        let bid_at = DEPTH_BAR_W;
        let ask_at = bid_at + 9 + 1 + DEPTH_PRICE_W + DEPTH_GAP.len() + DEPTH_PRICE_W + 1;
        assert_eq!(column(0, bid_at), "      QTY", "{}", rows.join("\n"));
        assert_eq!(column(1, bid_at), "12,34,567", "{}", rows.join("\n"));
        assert_eq!(column(0, ask_at), "QTY      ", "{}", rows.join("\n"));
        assert_eq!(column(1, ask_at), "23,45,678", "{}", rows.join("\n"));
    }

    #[test]
    fn depth_header_uses_header_style_not_dim() {
        use crate::tui::format;
        let _guard = crate::tui::settings::theme_guard(crate::tui::settings::Theme::DEFAULT);

        let mut depth = MarketDepth::default();
        depth.buy = vec![depth_entry(100, 10.0, 1)];
        depth.sell = vec![depth_entry(100, 20.0, 1)];
        let tick = Tick::default();

        let mut terminal = Terminal::new(TestBackend::new(42, 8)).unwrap();
        terminal
            .draw(|frame| draw_depth(frame, Rect::new(0, 0, 42, 8), &depth, &tick, None))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();

        let qty_header_x = u16::try_from(DEPTH_BAR_W).unwrap();
        let cell = &buffer[(qty_header_x, 0)];
        assert_eq!(cell.fg, format::header_style().fg.unwrap_or(cell.fg));
        assert_ne!(
            cell.fg,
            format::text_muted(),
            "depth header must not render in the dim/muted colour"
        );
    }

    fn circuit_row(width: u16) -> String {
        let mut depth = MarketDepth::default();
        depth.buy = vec![depth_entry(100, 150.0, 1)];
        depth.sell = vec![depth_entry(100, 150.5, 1)];
        let mut tick = Tick::default();
        tick.buy_quantity = Some(1_000);
        tick.sell_quantity = Some(2_000);
        let mut terminal = Terminal::new(TestBackend::new(width, 9)).unwrap();
        terminal
            .draw(|frame| draw_depth(frame, frame.area(), &depth, &tick, Some((135.0, 165.0))))
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..width).map(|x| buffer[(x, 8)].symbol()).collect()
    }

    #[test]
    fn depth_ladder_shows_circuit_limits_under_their_side() {
        let row = circuit_row(42);
        let bid_price_x = DEPTH_BAR_W + DEPTH_QTY_W + 1;
        let ask_price_x = bid_price_x + DEPTH_PRICE_W + DEPTH_GAP.len();
        assert_eq!(row[DEPTH_BAR_W..bid_price_x - 1].trim(), "LC", "{row:?}");
        assert_eq!(
            row[bid_price_x..bid_price_x + DEPTH_PRICE_W].trim(),
            "135.00"
        );
        assert_eq!(
            row[ask_price_x..ask_price_x + DEPTH_PRICE_W].trim(),
            "165.00"
        );
        assert_eq!(row[ask_price_x + DEPTH_PRICE_W + 1..].trim(), "UC");
    }

    #[test]
    fn a_narrow_ladder_keeps_circuit_labels_beside_the_prices() {
        let row = circuit_row(20);
        assert!(row.contains("LC 135.00"), "{row:?}");
        assert!(row.contains("165.00 UC"), "{row:?}");
    }
}
