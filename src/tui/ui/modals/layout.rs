use crate::tui::app::App;
use crate::tui::format;
use crate::tui::ui::{HitAction, HitList};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, ListState, Paragraph};

pub(in crate::tui::ui) fn modal_block(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    title: impl Into<Line<'static>>,
    border: Style,
) -> Rect {
    app.hit.borrow_mut().push(area, HitAction::Capture);
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .title(title.into())
        .style(crate::tui::ui::bar_style())
        .border_style(border);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    inner
}

pub(super) fn record_list_hits(app: &App, area: Rect, state: &ListState, list: HitList) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    app.hit.borrow_mut().push(
        area,
        HitAction::Rows {
            list,
            offset: state.offset(),
        },
    );
}

pub(super) fn draw_select_list(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    items: Vec<ratatui::widgets::ListItem<'static>>,
    selected: Option<usize>,
    list: HitList,
    marker: &'static str,
) {
    let mut state = ListState::default();
    state.select(selected);
    frame.render_stateful_widget(
        ratatui::widgets::List::new(items)
            .highlight_style(format::selection_style())
            .highlight_symbol(marker),
        area,
        &mut state,
    );
    record_list_hits(app, area, &state, list);
}

pub(super) const UTILITY_BODY_ROWS: u16 = 22;

pub(super) const UTILITY_TOO_SMALL: &str = "Esc closes.";

pub(super) const SELECT_MARKER: &str = "> ";

pub(super) fn body_row(body: Rect, offset: u16) -> Option<Rect> {
    (offset < body.height).then(|| Rect {
        y: body.y + offset,
        height: 1,
        ..body
    })
}

pub(super) fn draw_body_line(frame: &mut Frame, body: Rect, offset: u16, line: Line<'static>) {
    if let Some(rect) = body_row(body, offset) {
        frame.render_widget(Paragraph::new(line), rect);
    }
}

pub(super) fn section_line(label: &str, content: u16) -> Line<'static> {
    Line::from(Span::styled(
        format::pad_or_clip(label, usize::from(content)),
        format::bold(),
    ))
}

pub(super) fn draw_detail(
    frame: &mut Frame,
    body: Rect,
    content: u16,
    label: &str,
    rows: &[String],
) {
    let top = body.height.saturating_sub(DETAIL_ROWS);
    draw_body_line(frame, body, top, section_line(label, content));
    for (offset, text) in rows.iter().take(usize::from(DETAIL_ROWS - 1)).enumerate() {
        draw_body_line(
            frame,
            body,
            top + 1 + u16::try_from(offset).unwrap_or(u16::MAX),
            Line::from(Span::raw(text.clone())),
        );
    }
}

pub(super) const DETAIL_ROWS: u16 = 3;

pub(super) fn list_rows(body: Rect) -> (u16, u16) {
    let detail = if body.height >= DETAIL_ROWS + 3 {
        DETAIL_ROWS
    } else {
        0
    };
    (body.height.saturating_sub(1 + detail), detail)
}

pub(in crate::tui::ui) fn wrapped_rows(text: &str, width: u16) -> u16 {
    let width = width.max(1) as usize;
    let mut rows: usize = 1;
    let mut current: usize = text.len() - text.trim_start().len();
    for word in text.split_whitespace() {
        let word_len = word.chars().count();
        let needed = if current == 0 {
            word_len
        } else {
            current + 1 + word_len
        };
        if needed > width && current != 0 {
            rows += 1;
            current = 0;
        }
        if word_len > width {
            let extra = word_len.div_ceil(width) - 1;
            rows += extra;
            current = word_len - extra * width;
        } else if current == 0 {
            current = word_len;
        } else {
            current += 1 + word_len;
        }
    }
    u16::try_from(rows).unwrap_or(u16::MAX)
}

#[cfg(test)]
mod wrap_tests {
    use super::wrapped_rows;

    #[test]
    fn word_wrap_beats_char_density() {
        assert_eq!(wrapped_rows("12345 12345 12345", 10), 3);
    }

    #[test]
    fn long_word_spans_ceil_len_over_width() {
        assert_eq!(wrapped_rows(&"a".repeat(25), 10), 3);
        assert_eq!(wrapped_rows(&"b".repeat(10), 10), 1);
        assert_eq!(wrapped_rows(&"c".repeat(11), 10), 2);
    }

    #[test]
    fn realistic_reason_never_under_counts() {
        let reason = "Insufficient funds. Your available margin is 1,234.50 but \
                      this CNC order needs 5,678.90, reduce the quantity or add funds.";
        let width: u16 = 56;
        let char_density = u16::try_from(reason.chars().count())
            .unwrap()
            .div_ceil(width);
        assert!(
            wrapped_rows(reason, width) >= char_density,
            "word-wrap must never under-count vs the char-density estimate"
        );
    }

    #[test]
    fn blank_line_is_one_row() {
        assert_eq!(wrapped_rows("", 10), 1);
        assert_eq!(wrapped_rows("   ", 10), 1);
    }
}
