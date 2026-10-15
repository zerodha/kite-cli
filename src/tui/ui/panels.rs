use super::HitAction;
use crate::tui::app::App;
use crate::tui::format;
use crate::tui::split::{Axis as SplitAxis, Split};
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType};

pub fn panel(title: impl Into<Line<'static>>) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Plain)
        .border_style(format::border_style())
        .title_style(format::title_style())
        .title(title.into())
}

pub fn focus_marker(focused: bool) -> &'static str {
    if focused { "> " } else { "  " }
}

pub fn focus_title(title: impl std::fmt::Display, focused: bool) -> String {
    format!(" {}{title} ", focus_marker(focused))
}

pub fn focus_panel(title: impl std::fmt::Display, focused: bool) -> Block<'static> {
    let block = panel(focus_title(title, focused));
    if focused {
        block.border_style(Style::default().fg(format::accent()))
    } else {
        block
    }
}

fn divider_strip(split: Split, first: Rect, second: Rect) -> Option<Rect> {
    if first.is_empty() || second.is_empty() {
        return None;
    }
    match split.axis() {
        SplitAxis::Horizontal if second.x == first.right() => Some(Rect {
            x: first.right() - 1,
            y: first.y,
            width: 1,
            height: first.height,
        }),
        SplitAxis::Vertical if second.y == first.bottom() => Some(Rect {
            x: first.x,
            y: first.bottom() - 1,
            width: first.width,
            height: 1,
        }),
        _ => None,
    }
}

pub fn push_divider(app: &App, split: Split, span: Rect, first: Rect, second: Rect) {
    if let Some(strip) = divider_strip(split, first, second) {
        app.hit
            .borrow_mut()
            .push(strip, HitAction::Divider { split, span });
    }
}

pub fn table_row_hit(table: Rect, offset: usize, column: u16, row: u16) -> Option<usize> {
    if table.contains(Position { x: column, y: row }) {
        Some(offset + (row - table.y) as usize)
    } else {
        None
    }
}

pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::table_row_hit;
    use ratatui::layout::Rect;

    #[test]
    fn table_row_hit_maps_offset_and_bounds() {
        let rect = Rect {
            x: 0,
            y: 2,
            width: 20,
            height: 5,
        };
        assert_eq!(table_row_hit(rect, 0, 5, 2), Some(0)); // first row
        assert_eq!(table_row_hit(rect, 0, 5, 6), Some(4)); // last row
        assert_eq!(table_row_hit(rect, 20, 5, 2), Some(20)); // scrolled first row
        assert_eq!(table_row_hit(rect, 0, 5, 1), None); // above (header/border)
        assert_eq!(table_row_hit(rect, 0, 5, 7), None); // below the body
        assert_eq!(table_row_hit(rect, 0, 25, 3), None); // right of the table
    }

    #[test]
    fn a_divider_strip_needs_two_panels_that_touch() {
        use crate::tui::split::Split;
        use ratatui::layout::Rect;

        let list = Rect::new(4, 2, 60, 20);
        let detail = Rect::new(64, 2, 40, 20);
        assert_eq!(
            super::divider_strip(Split::TradingRoot, list, detail),
            Some(Rect::new(63, 2, 1, 20)),
            "one column, the list's own right border"
        );

        let upper = Rect::new(4, 2, 60, 12);
        let lower = Rect::new(4, 14, 60, 8);
        assert_eq!(
            super::divider_strip(Split::OverviewBooks, upper, lower),
            Some(Rect::new(4, 13, 60, 1)),
            "one row, the upper book's own bottom border"
        );

        assert_eq!(
            super::divider_strip(Split::TradingRoot, list, Rect::new(70, 2, 40, 20)),
            None
        );
        assert_eq!(
            super::divider_strip(Split::TradingRoot, list, Rect::new(50, 2, 40, 20)),
            None
        );
        assert_eq!(
            super::divider_strip(Split::TradingRoot, list, Rect::new(64, 2, 0, 20)),
            None
        );
        assert_eq!(
            super::divider_strip(Split::TradingRoot, Rect::new(4, 2, 0, 20), detail),
            None
        );
        assert_eq!(
            super::divider_strip(Split::OverviewBooks, upper, Rect::new(4, 20, 60, 8)),
            None
        );
    }

    #[test]
    fn panel_title_uses_accent_not_border() {
        use crate::tui::format;
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        use ratatui::style::Color;

        let _guard = crate::tui::settings::theme_guard(crate::tui::settings::Theme::DEFAULT);
        let block = super::panel("Test");
        let mut terminal = Terminal::new(TestBackend::new(20, 3)).unwrap();
        terminal
            .draw(|frame| frame.render_widget(block, frame.area()))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let title_cell = &buffer[(1, 0)];
        assert_eq!(title_cell.symbol(), "T");
        assert_eq!(title_cell.fg, format::accent());
        assert_ne!(
            title_cell.fg,
            format::border_style().fg.unwrap_or(Color::Reset)
        );
    }
}
