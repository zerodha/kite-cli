use crate::tui::format;
use ratatui::style::Style;
use ratatui::text::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WrapRow {
    pub line: usize,
    pub start: usize,
    pub end: usize,
}

fn wrap_line(line: usize, text: &str, width: usize, rows: &mut Vec<WrapRow>) {
    let (mut start, mut end, mut used) = (0, 0, 0);
    for grapheme in Span::raw(text).styled_graphemes(Style::default()) {
        let cells = format::grapheme_width(grapheme.symbol);
        if used > 0 && used + cells > width {
            rows.push(WrapRow { line, start, end });
            (start, used) = (end, 0);
        }
        used += cells;
        end += grapheme.symbol.chars().count();
    }
    rows.push(WrapRow { line, start, end });
    if used >= width {
        rows.push(WrapRow {
            line,
            start: end,
            end,
        });
    }
}

pub fn wrap(lines: &[String], width: usize) -> Vec<WrapRow> {
    let width = width.max(1);
    let mut rows = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        wrap_line(idx, line, width, &mut rows);
    }
    rows
}

pub fn cursor_row(lines: &[String], width: usize, line: usize, col: usize) -> (usize, usize) {
    let rows = wrap(lines, width);
    let mut last = (0, 0);
    for (index, row) in rows.iter().enumerate().filter(|(_, row)| row.line == line) {
        last = (index, col - row.start);
        if col < row.end {
            return last;
        }
    }
    last
}

pub fn scroll_for(cursor: usize, height: usize) -> usize {
    cursor.saturating_sub(height.max(1) - 1)
}

pub fn editor_scroll(buffer: &NoteBuffer, width: u16, height: u16) -> usize {
    let (line, col) = buffer.cursor();
    let (caret_row, _) = cursor_row(buffer.lines(), width.max(1) as usize, line, col);
    scroll_for(caret_row, height.max(1) as usize)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteBuffer {
    lines: Vec<String>,
    line: usize,
    col: usize,
    desired: usize,
}

impl Default for NoteBuffer {
    fn default() -> Self {
        Self::from_text("")
    }
}

fn byte_of(line: &str, col: usize) -> usize {
    line.char_indices()
        .nth(col)
        .map_or(line.len(), |(idx, _)| idx)
}

fn previous_grapheme_column(line: &str, column: usize) -> usize {
    let mut position = 0;
    for grapheme in Span::raw(line).styled_graphemes(Style::default()) {
        let next = position + grapheme.symbol.chars().count();
        if next >= column {
            return position;
        }
        position = next;
    }
    position
}

fn next_grapheme_column(line: &str, column: usize) -> usize {
    let mut position = 0;
    for grapheme in Span::raw(line).styled_graphemes(Style::default()) {
        position += grapheme.symbol.chars().count();
        if position > column {
            return position;
        }
    }
    position
}

fn char_len(line: &str) -> usize {
    line.chars().count()
}

impl NoteBuffer {
    pub fn from_text(text: &str) -> Self {
        let expanded = text.replace('\t', "    ");
        let lines: Vec<String> = expanded.split('\n').map(str::to_string).collect();
        let line = lines.len() - 1;
        let col = char_len(&lines[line]);
        Self {
            lines,
            line,
            col,
            desired: col,
        }
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    pub fn cursor(&self) -> (usize, usize) {
        (self.line, self.col)
    }

    fn len(&self) -> usize {
        char_len(&self.lines[self.line])
    }

    pub fn insert(&mut self, ch: char) {
        if ch == '\t' {
            for _ in 0..4 {
                self.insert(' ');
            }
            return;
        }
        if ch == '\n' {
            self.newline();
            return;
        }
        let at = byte_of(&self.lines[self.line], self.col);
        self.lines[self.line].insert(at, ch);
        self.col = next_grapheme_column(&self.lines[self.line], self.col);
        self.desired = self.col;
    }

    pub fn newline(&mut self) {
        let at = byte_of(&self.lines[self.line], self.col);
        let tail = self.lines[self.line].split_off(at);
        self.lines.insert(self.line + 1, tail);
        self.line += 1;
        self.col = 0;
        self.desired = 0;
    }

    pub fn backspace(&mut self) {
        if self.col > 0 {
            let previous = previous_grapheme_column(&self.lines[self.line], self.col);
            let start = byte_of(&self.lines[self.line], previous);
            let end = byte_of(&self.lines[self.line], self.col);
            self.lines[self.line].drain(start..end);
            self.col = previous;
        } else if self.line > 0 {
            let tail = self.lines.remove(self.line);
            self.line -= 1;
            self.col = self.len();
            self.lines[self.line].push_str(&tail);
            self.col = previous_grapheme_column(&self.lines[self.line], self.col + 1);
        }
        self.desired = self.col;
    }

    pub fn left(&mut self) {
        if self.col > 0 {
            self.col = previous_grapheme_column(&self.lines[self.line], self.col);
        } else if self.line > 0 {
            self.line -= 1;
            self.col = self.len();
        }
        self.desired = self.col;
    }

    pub fn right(&mut self) {
        if self.col < self.len() {
            self.col = next_grapheme_column(&self.lines[self.line], self.col);
        } else if self.line + 1 < self.lines.len() {
            self.line += 1;
            self.col = 0;
        }
        self.desired = self.col;
    }

    pub fn home(&mut self) {
        self.col = 0;
        self.desired = 0;
    }

    pub fn end(&mut self) {
        self.col = self.len();
        self.desired = self.col;
    }

    pub fn up(&mut self) {
        if self.line > 0 {
            self.line -= 1;
            self.col =
                previous_grapheme_column(&self.lines[self.line], self.desired.min(self.len()) + 1);
        }
    }

    pub fn down(&mut self) {
        if self.line + 1 < self.lines.len() {
            self.line += 1;
            self.col =
                previous_grapheme_column(&self.lines[self.line], self.desired.min(self.len()) + 1);
        }
    }

    pub fn click(&mut self, width: usize, row: usize, cell: usize) {
        let rows = wrap(&self.lines, width);
        let Some(target) = rows.get(row).or_else(|| rows.last()).copied() else {
            return;
        };
        let shown: String = self.lines[target.line]
            .chars()
            .skip(target.start)
            .take(target.end - target.start)
            .collect();
        self.line = target.line;
        self.col = target.start + col_at_cell(&shown, cell);
        self.desired = self.col;
    }
}

fn col_at_cell(text: &str, cell: usize) -> usize {
    let span = Span::raw(text);
    let mut used = 0usize;
    let mut chars = 0usize;
    for grapheme in span.styled_graphemes(Style::default()) {
        let cells = format::grapheme_width(grapheme.symbol);
        if cells > 0 && cell < used + cells {
            return chars;
        }
        used += cells;
        chars += grapheme.symbol.chars().count();
    }
    char_len(text)
}

#[cfg(test)]
mod tests {
    use super::{NoteBuffer, WrapRow, cursor_row, scroll_for, wrap};

    #[test]
    fn wrap_of_an_empty_buffer_is_one_empty_row() {
        assert_eq!(
            wrap(&[String::new()], 10),
            vec![WrapRow {
                line: 0,
                start: 0,
                end: 0
            }]
        );
    }

    #[test]
    fn wrap_splits_long_lines_and_leaves_a_row_for_an_end_cursor() {
        let lines = vec!["a".repeat(25), "short".to_string()];
        let rows = wrap(&lines, 10);
        assert_eq!(
            rows,
            vec![
                WrapRow {
                    line: 0,
                    start: 0,
                    end: 10
                },
                WrapRow {
                    line: 0,
                    start: 10,
                    end: 20
                },
                WrapRow {
                    line: 0,
                    start: 20,
                    end: 25
                },
                WrapRow {
                    line: 1,
                    start: 0,
                    end: 5
                },
            ]
        );
        let exact = wrap(&["b".repeat(10)], 10);
        assert_eq!(exact.len(), 2);
        assert_eq!(
            exact[1],
            WrapRow {
                line: 0,
                start: 10,
                end: 10
            }
        );
    }

    #[test]
    fn cursor_row_agrees_with_wrap_at_every_position() {
        let lines = vec!["a".repeat(25), String::new(), "hello".to_string()];
        let rows = wrap(&lines, 10);
        for (line_idx, line) in lines.iter().enumerate() {
            for col in 0..=line.chars().count() {
                let (row, col_in_row) = cursor_row(&lines, 10, line_idx, col);
                let wrapped = rows.get(row).unwrap_or_else(|| {
                    panic!("cursor at ({line_idx},{col}) points past the wrapped rows")
                });
                assert_eq!(wrapped.line, line_idx, "at ({line_idx},{col})");
                assert_eq!(wrapped.start + col_in_row, col, "at ({line_idx},{col})");
                assert!(col_in_row < 10, "caret column must stay inside the row");
            }
        }
    }

    #[test]
    fn wrap_packs_wide_glyphs_by_cells() {
        let lines = [format!("{}TAIL", "漢".repeat(6))];
        let rows = wrap(&lines, 10);
        let ranges: Vec<(usize, usize)> = rows.iter().map(|row| (row.start, row.end)).collect();
        assert_eq!(ranges, vec![(0, 5), (5, 10)]);
        assert_eq!(cursor_row(&lines, 10, 0, 10), (1, 5));
        assert_eq!(cursor_row(&lines, 10, 0, 5), (1, 0));
    }

    #[test]
    fn scroll_for_pins_short_content_and_follows_a_deep_cursor() {
        assert_eq!(scroll_for(0, 5), 0);
        assert_eq!(scroll_for(4, 5), 0); // last visible row, no scroll yet
        assert_eq!(scroll_for(5, 5), 1);
        assert_eq!(scroll_for(40, 5), 36);
        assert_eq!(scroll_for(3, 0), 3);
    }

    fn state(buffer: &NoteBuffer) -> (String, usize, usize) {
        let (line, col) = buffer.cursor();
        (buffer.text(), line, col)
    }

    #[test]
    fn new_buffer_is_one_blank_line_with_the_cursor_at_the_start() {
        let buffer = NoteBuffer::default();
        assert_eq!(state(&buffer), (String::new(), 0, 0));
        assert_eq!(buffer.lines().len(), 1);
    }

    #[test]
    fn from_text_puts_the_cursor_at_the_very_end() {
        let buffer = NoteBuffer::from_text("first\nsecond");
        assert_eq!(state(&buffer), ("first\nsecond".to_string(), 1, 6));
    }

    #[test]
    fn insert_types_at_the_cursor() {
        let mut buffer = NoteBuffer::from_text("");
        for ch in "buy".chars() {
            buffer.insert(ch);
        }
        assert_eq!(state(&buffer), ("buy".to_string(), 0, 3));
        buffer.home();
        buffer.insert('!');
        assert_eq!(state(&buffer), ("!buy".to_string(), 0, 1));
    }

    #[test]
    fn backspace_at_the_very_start_of_the_buffer_is_a_noop() {
        let mut buffer = NoteBuffer::from_text("abc");
        buffer.home();
        buffer.backspace();
        assert_eq!(state(&buffer), ("abc".to_string(), 0, 0));
    }

    #[test]
    fn backspace_joins_two_lines_at_the_join_point() {
        let mut buffer = NoteBuffer::from_text("first\nsecond");
        buffer.home(); // start of "second"
        buffer.backspace();
        assert_eq!(state(&buffer), ("firstsecond".to_string(), 0, 5));
        buffer.insert('-');
        assert_eq!(state(&buffer), ("first-second".to_string(), 0, 6));
    }

    #[test]
    fn backspace_removing_a_lone_newline_leaves_one_line() {
        let mut buffer = NoteBuffer::from_text("a\n");
        assert_eq!(buffer.cursor(), (1, 0));
        buffer.backspace();
        assert_eq!(state(&buffer), ("a".to_string(), 0, 1));
        assert_eq!(buffer.lines().len(), 1);
    }

    #[test]
    fn enter_splits_a_line_mid_word() {
        let mut buffer = NoteBuffer::from_text("firstsecond");
        buffer.home();
        for _ in 0..5 {
            buffer.right();
        }
        buffer.newline();
        assert_eq!(state(&buffer), ("first\nsecond".to_string(), 1, 0));
    }

    #[test]
    fn enter_at_end_of_buffer_opens_a_fresh_line() {
        let mut buffer = NoteBuffer::from_text("done");
        buffer.newline();
        assert_eq!(state(&buffer), ("done\n".to_string(), 1, 0));
        assert_eq!(buffer.lines().len(), 2);
    }

    #[test]
    fn left_and_right_wrap_across_line_boundaries_and_clamp_at_the_ends() {
        let mut buffer = NoteBuffer::from_text("ab\ncd");
        buffer.home(); // (1, 0)
        buffer.left();
        assert_eq!(
            buffer.cursor(),
            (0, 2),
            "left at col 0 goes to the line above"
        );
        buffer.left();
        buffer.left();
        assert_eq!(buffer.cursor(), (0, 0));
        buffer.left();
        assert_eq!(buffer.cursor(), (0, 0), "left at buffer start is a noop");
        for _ in 0..5 {
            buffer.right();
        }
        assert_eq!(buffer.cursor(), (1, 2), "right at buffer end is a noop");
    }

    #[test]
    fn home_and_end_move_within_the_current_line_only() {
        let mut buffer = NoteBuffer::from_text("long line here\nshort");
        buffer.up();
        buffer.home();
        assert_eq!(buffer.cursor(), (0, 0));
        buffer.end();
        assert_eq!(buffer.cursor(), (0, 14));
    }

    #[test]
    fn up_down_preserve_the_desired_column_across_a_short_line() {
        let mut buffer = NoteBuffer::from_text("0123456789abc\nxy\n0123456789abc");
        buffer.up();
        buffer.up();
        buffer.home();
        for _ in 0..10 {
            buffer.right();
        }
        assert_eq!(buffer.cursor(), (0, 10));
        buffer.down();
        assert_eq!(buffer.cursor(), (1, 2), "clamped to the short line's end");
        buffer.down();
        assert_eq!(buffer.cursor(), (2, 10), "desired column restored");
        buffer.up();
        assert_eq!(buffer.cursor(), (1, 2));
        buffer.up();
        assert_eq!(buffer.cursor(), (0, 10));
    }

    #[test]
    fn horizontal_motion_resets_the_desired_column() {
        let mut buffer = NoteBuffer::from_text("0123456789\nxy\n0123456789");
        buffer.up();
        buffer.up();
        buffer.end(); // col 10, desired 10
        buffer.down();
        assert_eq!(buffer.cursor(), (1, 2));
        buffer.left();
        buffer.down();
        assert_eq!(buffer.cursor(), (2, 1));
    }

    #[test]
    fn up_and_down_clamp_at_the_first_and_last_line() {
        let mut buffer = NoteBuffer::from_text("one\ntwo");
        buffer.down();
        assert_eq!(buffer.cursor(), (1, 3), "down on the last line is a noop");
        buffer.up();
        buffer.up();
        assert_eq!(buffer.cursor(), (0, 3), "up on the first line is a noop");
    }

    #[test]
    fn multibyte_input_inserts_and_deletes_whole_graphemes() {
        let mut buffer = NoteBuffer::from_text("");
        for ch in "₹1520, निफ्टी".chars() {
            buffer.insert(ch);
        }
        let chars = "₹1520, निफ्टी".chars().count();
        assert_eq!(buffer.cursor(), (0, chars));
        assert_eq!(buffer.text(), "₹1520, निफ्टी");
        buffer.backspace();
        let expected = "₹1520, नि";
        assert_eq!(
            state(&buffer),
            (expected.to_string(), 0, expected.chars().count())
        );
    }

    #[test]
    fn multibyte_cursor_motion_and_mid_string_edits_stay_on_char_boundaries() {
        let mut buffer = NoteBuffer::from_text("₹₹₹");
        buffer.home();
        buffer.right();
        buffer.insert('x');
        assert_eq!(state(&buffer), ("₹x₹₹".to_string(), 0, 2));
        buffer.left();
        buffer.backspace();
        assert_eq!(state(&buffer), ("x₹₹".to_string(), 0, 0));
        buffer.end();
        assert_eq!(buffer.cursor(), (0, 3));
    }

    #[test]
    fn multibyte_newline_splits_on_a_char_boundary() {
        let mut buffer = NoteBuffer::from_text("₹1520₹");
        buffer.home();
        buffer.right();
        buffer.newline();
        assert_eq!(state(&buffer), ("₹\n1520₹".to_string(), 1, 0));
    }

    #[test]
    fn inserting_a_newline_char_splits_rather_than_corrupting_the_line() {
        let mut buffer = NoteBuffer::from_text("ab");
        buffer.insert('\n');
        assert_eq!(state(&buffer), ("ab\n".to_string(), 1, 0));
        assert_eq!(buffer.lines().len(), 2);
    }

    #[test]
    fn a_click_puts_the_cursor_where_it_landed() {
        let mut buffer = NoteBuffer::from_text("hello world\nsecond line");
        buffer.click(20, 0, 6);
        assert_eq!(buffer.cursor(), (0, 6)); // the 'w' of "world"
        buffer.click(20, 1, 0);
        assert_eq!(buffer.cursor(), (1, 0));
        buffer.click(20, 1, 7);
        assert_eq!(buffer.cursor(), (1, 7)); // the 'l' of "line"
    }

    #[test]
    fn a_click_past_the_text_clamps_to_it() {
        let mut buffer = NoteBuffer::from_text("short\nlonger line");
        buffer.click(20, 0, 40);
        assert_eq!(buffer.cursor(), (0, 5));
        buffer.click(20, 99, 3);
        assert_eq!(buffer.cursor(), (1, 3));
        let mut buffer = NoteBuffer::from_text("\nx");
        buffer.click(20, 0, 12);
        assert_eq!(buffer.cursor(), (0, 0));
    }

    #[test]
    fn a_click_on_a_wrapped_row_maps_into_the_right_line() {
        let mut buffer = NoteBuffer::from_text(&"abcdefghij".repeat(3));
        buffer.click(10, 1, 2);
        assert_eq!(buffer.cursor(), (0, 12));
        buffer.click(10, 2, 9);
        assert_eq!(buffer.cursor(), (0, 29));
        buffer.click(10, 3, 0);
        assert_eq!(buffer.cursor(), (0, 30));
    }

    #[test]
    fn a_click_counts_terminal_cells_not_characters() {
        let mut buffer = NoteBuffer::from_text("測試ab");
        buffer.click(20, 0, 0);
        assert_eq!(buffer.cursor(), (0, 0)); // 測
        buffer.click(20, 0, 1);
        assert_eq!(buffer.cursor(), (0, 0)); // still 測: it owns two cells
        buffer.click(20, 0, 2);
        assert_eq!(buffer.cursor(), (0, 1)); // 試
        buffer.click(20, 0, 4);
        assert_eq!(buffer.cursor(), (0, 2)); // a
        buffer.click(20, 0, 5);
        assert_eq!(buffer.cursor(), (0, 3)); // b
        buffer.click(20, 0, 19);
        assert_eq!(buffer.cursor(), (0, 4));
        let mut buffer = NoteBuffer::from_text("₹120 target");
        buffer.click(20, 0, 1);
        assert_eq!(buffer.cursor(), (0, 1));
        buffer.click(20, 0, 5);
        assert_eq!(buffer.cursor(), (0, 5));
    }

    #[test]
    fn a_click_lands_on_a_base_character_not_its_combining_mark() {
        let mut buffer = NoteBuffer::from_text("e\u{301}x");
        assert_eq!(buffer.lines()[0].chars().count(), 3);
        buffer.click(20, 0, 0);
        assert_eq!(buffer.cursor(), (0, 0)); // the 'e', not its accent
        buffer.click(20, 0, 1);
        assert_eq!(buffer.cursor(), (0, 2)); // the 'x'
    }

    #[test]
    fn a_click_after_a_joined_emoji_lands_after_the_whole_emoji() {
        let mut buffer = NoteBuffer::from_text("👨\u{200d}👩\u{200d}👧X");
        assert_eq!(buffer.lines()[0].chars().count(), 6);
        buffer.click(20, 0, 2);
        assert_eq!(buffer.cursor(), (0, 5)); // the 'X', past the whole family
        buffer.insert('Z');
        assert_eq!(buffer.text(), "👨\u{200d}👩\u{200d}👧ZX");
    }

    #[test]
    fn a_click_resets_the_sticky_column() {
        let mut buffer = NoteBuffer::from_text("first line here\nsecond line here");
        buffer.click(20, 1, 12);
        assert_eq!(buffer.cursor(), (1, 12));
        buffer.up();
        assert_eq!(buffer.cursor(), (0, 12));
    }

    #[test]
    fn a_click_in_a_one_cell_box_is_answered() {
        let mut buffer = NoteBuffer::from_text("abc");
        buffer.click(1, 0, 0);
        assert_eq!(buffer.cursor(), (0, 0));
        buffer.click(1, 2, 0);
        assert_eq!(buffer.cursor(), (0, 2));
        buffer.click(0, 1, 0);
        assert_eq!(buffer.cursor(), (0, 1));
    }

    #[test]
    fn the_editor_scroll_follows_the_caret_down() {
        let buffer = NoteBuffer::from_text("a\nb\nc\nd\ne");
        assert_eq!(super::editor_scroll(&buffer, 20, 3), 2);
        assert_eq!(super::editor_scroll(&buffer, 20, 10), 0);
        assert_eq!(super::editor_scroll(&buffer, 0, 0), 9);
    }
    #[test]
    fn cursor_and_deletion_keep_emoji_and_combining_graphemes_whole() {
        for text in ["👩🏽‍💻", "🇮🇳", "e\u{301}", "👍🏽"] {
            let mut buffer = NoteBuffer::from_text(text);
            buffer.end();
            buffer.left();
            assert_eq!(buffer.cursor(), (0, 0));
            buffer.right();
            assert_eq!(buffer.cursor(), (0, text.chars().count()));
            buffer.backspace();
            assert_eq!(buffer.text(), "");
            assert_eq!(buffer.cursor(), (0, 0));
        }
    }
}
