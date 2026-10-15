use crate::tui::format;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageTone {
    Normal,
    Error,
}

pub fn state_message(
    frame: &mut Frame,
    inner: Rect,
    tone: MessageTone,
    headline: &str,
    detail: Option<&str>,
    hint: Option<&str>,
) {
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let inset = u16::from(inner.width >= 4);
    let text_width = (inner.width - inset * 2) as usize;
    let headline_style = match tone {
        MessageTone::Normal => format::header_style(),
        MessageTone::Error => Style::default()
            .fg(format::error())
            .add_modifier(Modifier::BOLD),
    };
    let mut lines: Vec<Line<'static>> = wrap_prose(headline, text_width)
        .into_iter()
        .map(|row| Line::from(Span::styled(row, headline_style)))
        .collect();
    for prose in [detail, hint].into_iter().flatten() {
        lines.extend(
            wrap_prose(prose, text_width)
                .into_iter()
                .map(|row| Line::from(Span::styled(row, format::dim()))),
        );
    }

    let top = u16::from(lines.len() < inner.height as usize);
    let height = (inner.height - top) as usize;
    if lines.len() > height {
        lines.truncate(height);
        if let Some(last) = lines.last_mut() {
            let style = last.spans.first().map_or(format::dim(), |span| span.style);
            let row: String = last
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect();
            *last = Line::from(Span::styled(mark_clipped(&row, text_width), style));
        }
    }
    if lines.is_empty() {
        return;
    }
    let area = Rect {
        x: inner.x + inset,
        y: inner.y + top,
        width: u16::try_from(text_width).unwrap_or(u16::MAX),
        height: u16::try_from(lines.len()).unwrap_or(u16::MAX),
    };
    frame.render_widget(Paragraph::new(lines), area);
}

pub(super) fn wrap_prose(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut rows = Vec::new();
    for logical in text.split('\n') {
        let logical = logical.strip_suffix('\r').unwrap_or(logical);
        let mut current = String::new();
        let mut used = 0usize;
        for word in logical.split_whitespace() {
            let word_width = format::display_width(word);
            if word_width > width {
                if used > 0 {
                    rows.push(std::mem::take(&mut current));
                    used = 0;
                }
                let span = Span::raw(word);
                for grapheme in span.styled_graphemes(Style::default()) {
                    let cells = format::grapheme_width(grapheme.symbol);
                    if used > 0 && used + cells > width {
                        rows.push(std::mem::take(&mut current));
                        used = 0;
                    }
                    current.push_str(grapheme.symbol);
                    used += cells;
                }
                continue;
            }
            let needed = if used == 0 {
                word_width
            } else {
                used + 1 + word_width
            };
            if needed > width {
                rows.push(std::mem::take(&mut current));
                current.push_str(word);
                used = word_width;
            } else {
                if used > 0 {
                    current.push(' ');
                    used += 1;
                }
                current.push_str(word);
                used += word_width;
            }
        }
        rows.push(current);
    }
    rows
}

pub(super) fn mark_clipped(row: &str, width: usize) -> String {
    const MARK: &str = "...";
    if width == 0 {
        return String::new();
    }
    if width <= MARK.len() {
        return MARK[..width].to_string();
    }
    let budget = width - MARK.len();
    let span = Span::raw(row);
    let mut kept = String::new();
    let mut used = 0usize;
    for grapheme in span.styled_graphemes(Style::default()) {
        let cells = format::grapheme_width(grapheme.symbol);
        if used + cells > budget {
            break;
        }
        kept.push_str(grapheme.symbol);
        used += cells;
    }
    kept.push_str(&" ".repeat(budget - used));
    kept.push_str(MARK);
    kept
}

#[cfg(test)]
mod tests {
    use super::{MessageTone, mark_clipped, state_message, wrap_prose};
    use crate::tui::format;
    use crate::tui::settings::{self, Theme};
    use crate::tui::ui::panel;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::style::Modifier;

    fn row(buffer: &Buffer, y: u16) -> String {
        let mut out = String::new();
        let mut skip = 0u16;
        for x in 0..buffer.area.width {
            if skip > 0 {
                skip -= 1;
                continue;
            }
            let symbol = buffer[(x, y)].symbol();
            let cells = format::display_width(symbol);
            skip = u16::try_from(cells.saturating_sub(1)).unwrap();
            out.push_str(symbol);
        }
        out
    }

    fn rows(buffer: &Buffer) -> Vec<String> {
        (0..buffer.area.height).map(|y| row(buffer, y)).collect()
    }

    #[test]
    fn wrap_prose_measures_terminal_cells() {
        assert_eq!(wrap_prose("₹1,00,000 short", 9), vec!["₹1,00,000", "short"]);
        assert_eq!(wrap_prose("株株株株", 6), vec!["株株株", "株"]);
        assert_eq!(
            wrap_prose("e\u{301}e\u{301} x", 2),
            vec!["e\u{301}e\u{301}", "x"]
        );
    }

    #[test]
    fn wrap_prose_splits_an_overlong_word_on_grapheme_boundaries() {
        assert_eq!(wrap_prose("abcdefgh", 3), vec!["abc", "def", "gh"]);
        assert_eq!(wrap_prose("株株株", 3), vec!["株", "株", "株"]);
        assert_eq!(wrap_prose("株", 1), vec!["株"]);
    }

    #[test]
    fn wrap_prose_preserves_explicit_line_breaks() {
        assert_eq!(wrap_prose("a\n\nb", 10), vec!["a", "", "b"]);
        assert_eq!(wrap_prose("a\r\nb", 10), vec!["a", "b"]);
        assert_eq!(wrap_prose("", 10), vec![""]);
    }

    #[test]
    fn mark_clipped_puts_its_marker_on_the_right_edge() {
        assert_eq!(mark_clipped("abcdef", 10), "abcdef ...");
        assert_eq!(mark_clipped("abcdefghij", 6), "abc...");
        assert_eq!(mark_clipped("abc", 2), "..");
        assert_eq!(mark_clipped("abc", 0), "");
        assert_eq!(mark_clipped("株株", 6), "株 ...");
    }

    fn state_panel(
        width: u16,
        height: u16,
        title: &str,
        tone: MessageTone,
        headline: &str,
        detail: Option<&str>,
        hint: Option<&str>,
    ) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| {
                let block = panel(title.to_string());
                let inner = block.inner(frame.area());
                frame.render_widget(block, frame.area());
                state_message(frame, inner, tone, headline, detail, hint);
            })
            .unwrap();
        terminal.backend().buffer().clone()
    }

    #[test]
    fn the_state_message_styles_headline_detail_and_hint() {
        let _guard = settings::theme_guard(settings::preset("kite").expect("preset exists"));
        let buffer = state_panel(
            40,
            8,
            " Movers ",
            MessageTone::Error,
            "Market movers unavailable",
            Some("Connection timed out"),
            Some("r Retry"),
        );
        let headline = &buffer[(2, 2)];
        assert_eq!(headline.symbol(), "M");
        assert_eq!(headline.fg, format::error());
        assert!(headline.modifier.contains(Modifier::BOLD));
        for y in [3u16, 4] {
            let cell = &buffer[(2, y)];
            assert_eq!(cell.fg, format::text_muted(), "row {y} is not muted");
            assert!(!cell.modifier.contains(Modifier::BOLD), "row {y} is bold");
        }

        let normal = state_panel(
            40,
            8,
            " Watchlist ",
            MessageTone::Normal,
            "Watchlist is empty",
            None,
            Some("/ Search and add instruments"),
        );
        let cell = &normal[(2, 2)];
        assert_eq!(Some(cell.fg), format::text());
        assert!(cell.modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn a_state_message_keeps_every_word_of_a_ligating_script() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let buffer = state_panel(
            9,
            8,
            " Watch ",
            MessageTone::Normal,
            "x",
            Some("لا لا لا"),
            None,
        );
        let painted: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert_eq!(painted.matches('ل').count(), 3, "{painted}");
        assert_eq!(painted.matches('ا').count(), 3, "{painted}");
    }

    #[test]
    fn the_top_inset_yields_to_the_message() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let tight = state_panel(
            40,
            5,
            " Movers ",
            MessageTone::Normal,
            "Market movers unavailable",
            Some("Connection timed out"),
            Some("r Retry"),
        );
        assert_eq!(row(&tight, 1), "│ Market movers unavailable            │");

        let roomy = state_panel(
            40,
            6,
            " Movers ",
            MessageTone::Normal,
            "Market movers unavailable",
            Some("Connection timed out"),
            Some("r Retry"),
        );
        assert_eq!(row(&roomy, 1), "│                                      │");
        assert_eq!(row(&roomy, 2), "│ Market movers unavailable            │");
    }

    #[test]
    fn state_prose_that_outruns_its_panel_is_marked() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let buffer = state_panel(
            20,
            4,
            " Movers ",
            MessageTone::Normal,
            "Market movers are unavailable right now",
            Some("Connection timed out"),
            None,
        );
        assert_eq!(row(&buffer, 1), "│ Market movers    │");
        assert_eq!(row(&buffer, 2), "│ are unavailab... │");
    }

    #[test]
    fn a_very_narrow_state_panel_drops_its_inset() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        for width in 1..=6u16 {
            let mut terminal = Terminal::new(TestBackend::new(width, 3)).unwrap();
            terminal
                .draw(|frame| {
                    let area = frame.area();
                    state_message(frame, area, MessageTone::Normal, "empty", None, None);
                })
                .unwrap();
            let rendered = rows(terminal.backend().buffer());
            let inset = usize::from(width >= 4);
            assert!(
                rendered.iter().any(|line| line[inset..].starts_with('e')),
                "width {width} lost the message: {rendered:?}"
            );
        }
    }
}
