use super::tile;
use crate::tui::app::App;
use crate::tui::format;
use crate::tui::ui;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

const ACCOUNT_SEP: &str = "  ";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct AccountCounts {
    positions: Option<(usize, usize)>,
    orders: Option<(usize, usize)>,
    holdings: Option<usize>,
    gtts: Option<usize>,
}

impl AccountCounts {
    fn of(app: &App) -> Self {
        Self {
            positions: app.positions.as_ref().map(|p| {
                (
                    p.net.len(),
                    p.net.iter().filter(|pos| pos.quantity != 0).count(),
                )
            }),
            orders: app.orders.as_ref().map(|orders| {
                (
                    orders.len(),
                    orders
                        .iter()
                        .filter(|o| crate::tui::data::order_is_open(o))
                        .count(),
                )
            }),
            holdings: app.holdings.as_ref().map(Vec::len),
            gtts: app.gtts.as_ref().map(Vec::len),
        }
    }

    fn segments(self, compact: bool) -> [String; 4] {
        let pair = |full: &str, short: &str, counts: Option<(usize, usize)>| match counts {
            None => format!("{short} ?"),
            Some((total, open)) if compact => format!("{short} {open}/{total}"),
            Some((total, open)) => format!("{full} {total} ({open} open)"),
        };
        let single = |full: &str, short: &str, count: Option<usize>| match count {
            None => format!("{short} ?"),
            Some(n) if compact => format!("{short} {n}"),
            Some(n) => format!("{full} {n}"),
        };
        [
            pair("Positions", "Pos", self.positions),
            pair("Orders", "Ord", self.orders),
            single("Holdings", "Hold", self.holdings),
            single("GTT", "GTT", self.gtts),
        ]
    }
}

fn account_line(counts: AccountCounts, width: u16) -> String {
    let budget = width.saturating_sub(1);
    let full = counts.segments(false);
    let joined = full.join(ACCOUNT_SEP);
    if joined.chars().count() <= budget as usize {
        return format!(" {joined}");
    }
    let compact = counts.segments(true);
    let refs: Vec<&str> = compact.iter().map(String::as_str).collect();
    format!(" {}", ui::fit_hints(budget, ACCOUNT_SEP, &refs))
}

pub(in crate::tui::ui) fn draw_account(frame: &mut Frame, area: Rect, app: &App, compact: bool) {
    if area.is_empty() {
        return;
    }
    let inner = if compact {
        area
    } else {
        let block = tile("Account");
        let inner = block.inner(area);
        frame.render_widget(block, area);
        inner
    };
    if inner.is_empty() {
        return;
    }
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            account_line(AccountCounts::of(app), inner.width),
            format::dim(),
        ))),
        inner,
    );
}

#[cfg(test)]
mod tests {
    use super::{AccountCounts, account_line};

    fn loaded() -> AccountCounts {
        AccountCounts {
            positions: Some((12, 5)),
            orders: Some((3, 1)),
            holdings: Some(40),
            gtts: Some(2),
        }
    }

    #[test]
    fn account_line_uses_full_labels_when_they_fit() {
        assert_eq!(
            account_line(loaded(), 80),
            " Positions 12 (5 open)  Orders 3 (1 open)  Holdings 40  GTT 2"
        );
    }

    #[test]
    fn account_line_compacts_labels_before_it_drops_a_segment() {
        let compact = account_line(loaded(), 40);
        assert_eq!(compact, " Pos 5/12  Ord 1/3  Hold 40  GTT 2");
        assert!(compact.chars().count() <= 40);
    }

    #[test]
    fn account_line_drops_whole_segments_from_the_right() {
        assert_eq!(account_line(loaded(), 30), " Pos 5/12  Ord 1/3  Hold 40");
        assert_eq!(account_line(loaded(), 20), " Pos 5/12  Ord 1/3");
        assert_eq!(account_line(loaded(), 11), " Pos 5/12");
        assert_eq!(account_line(loaded(), 5), " ");
    }

    #[test]
    fn account_line_separates_an_unloaded_source_from_an_empty_one() {
        let unknown = AccountCounts::default();
        assert_eq!(account_line(unknown, 80), " Pos ?  Ord ?  Hold ?  GTT ?");

        let empty = AccountCounts {
            positions: Some((0, 0)),
            orders: Some((0, 0)),
            holdings: Some(0),
            gtts: Some(0),
        };
        assert_eq!(
            account_line(empty, 80),
            " Positions 0 (0 open)  Orders 0 (0 open)  Holdings 0  GTT 0"
        );
    }

    #[test]
    fn account_line_never_renders_a_partial_segment() {
        let big = AccountCounts {
            positions: Some((123_456, 654)),
            orders: Some((9_999, 12)),
            holdings: Some(87_654),
            gtts: Some(321),
        };
        for counts in [loaded(), AccountCounts::default(), big] {
            for width in 0..=200u16 {
                let line = account_line(counts, width);
                assert!(
                    line.chars().count() <= width.max(1) as usize,
                    "width {width}: {line:?}"
                );
                for segment in line.trim_start().split("  ").filter(|s| !s.is_empty()) {
                    let whole = counts
                        .segments(false)
                        .into_iter()
                        .chain(counts.segments(true))
                        .any(|form| form == segment);
                    assert!(whole, "width {width}: partial segment {segment:?}");
                }
            }
        }
    }
}
