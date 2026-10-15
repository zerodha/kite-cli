use super::content::HelpRow;
use crate::tui::format;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

pub(super) const KEY_INDENT: usize = 2;
const KEY_GAP: usize = 2;
const MIN_DESC: usize = 12;
const DESC_TARGET: usize = 54;

pub(super) fn max_key_width(rows: &[HelpRow]) -> usize {
    rows.iter()
        .filter_map(|row| match row {
            HelpRow::Entry { key, .. } => Some(key.chars().count()),
            _ => None,
        })
        .max()
        .unwrap_or(0)
}

pub(super) fn key_width(max_key: usize, inner_w: usize) -> usize {
    max_key.min(inner_w.saturating_sub(KEY_INDENT + KEY_GAP + MIN_DESC))
}

pub(super) fn desc_col(key_w: usize) -> usize {
    KEY_INDENT + key_w + KEY_GAP
}

pub(super) fn wrap_words(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut rows: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_len = 0usize;
    for word in text.split_whitespace() {
        let word_len = word.chars().count();
        if word_len > width {
            if current_len > 0 {
                rows.push(std::mem::take(&mut current));
            }
            let chars: Vec<char> = word.chars().collect();
            let mut cut = 0;
            while chars.len() - cut > width {
                rows.push(chars[cut..cut + width].iter().collect());
                cut += width;
            }
            current = chars[cut..].iter().collect();
            current_len = chars.len() - cut;
            continue;
        }
        let needed = if current_len == 0 {
            word_len
        } else {
            current_len + 1 + word_len
        };
        if needed > width {
            rows.push(std::mem::take(&mut current));
            current.push_str(word);
            current_len = word_len;
        } else {
            if current_len > 0 {
                current.push(' ');
            }
            current.push_str(word);
            current_len = needed;
        }
    }
    if !current.is_empty() || rows.is_empty() {
        rows.push(current);
    }
    rows
}

fn entry_rows(key: &str, desc: &str, key_w: usize, inner_w: usize) -> Vec<String> {
    let col = desc_col(key_w);
    let wrapped = wrap_words(desc, inner_w.saturating_sub(col));
    let indent = " ".repeat(col);
    let mut rows = Vec::with_capacity(wrapped.len() + 1);
    let mut tail = &wrapped[..];
    if key.chars().count() > key_w {
        for part in wrap_words(key, inner_w.saturating_sub(KEY_INDENT)) {
            rows.push(format!("{}{part}", " ".repeat(KEY_INDENT)));
        }
    } else {
        rows.push(format!(
            "{}{}{}{}",
            " ".repeat(KEY_INDENT),
            format::pad_or_clip_right(key, key_w),
            " ".repeat(KEY_GAP),
            wrapped[0]
        ));
        tail = &wrapped[1..];
    }
    for part in tail {
        rows.push(format!("{indent}{part}"));
    }
    rows
}

pub(super) fn help_lines(rows: &[HelpRow], inner_w: u16) -> Vec<Line<'static>> {
    let inner_w = inner_w as usize;
    let key_w = key_width(max_key_width(rows), inner_w);
    let col = desc_col(key_w);
    let mut lines = Vec::new();
    for row in rows {
        match row {
            HelpRow::Blank => lines.push(Line::from("")),
            HelpRow::Section(title) => lines.push(Line::from(Span::styled(
                format::pad_or_clip(&format!(" {title} "), inner_w),
                format::title_style(),
            ))),
            HelpRow::Group(title) => lines.push(Line::from(Span::styled(
                format::pad_or_clip(&format!("  {title}"), inner_w),
                Style::default().fg(format::accent()),
            ))),
            HelpRow::Note(text) => {
                for part in wrap_words(text, inner_w.saturating_sub(1)) {
                    lines.push(Line::from(Span::styled(format!(" {part}"), format::dim())));
                }
            }
            HelpRow::Entry { key, desc } => {
                for text in entry_rows(key, desc, key_w, inner_w) {
                    let head: String = text.chars().take(col).collect();
                    let tail: String = text.chars().skip(col).collect();
                    lines.push(Line::from(vec![
                        Span::styled(head, format::bold()),
                        Span::raw(tail),
                    ]));
                }
            }
        }
    }
    lines
}

pub(super) fn box_width(rows: &[HelpRow], frame_w: u16) -> u16 {
    let widest_desc = rows
        .iter()
        .filter_map(|row| match row {
            HelpRow::Entry { desc, .. } => Some(desc.chars().count()),
            HelpRow::Note(text) => Some(text.chars().count() + 1),
            _ => None,
        })
        .max()
        .unwrap_or(DESC_TARGET);
    let inner = desc_col(max_key_width(rows)) + widest_desc.min(DESC_TARGET);
    u16::try_from(inner + 2).unwrap_or(u16::MAX).min(frame_w)
}

#[cfg(test)]
mod tests {
    use super::{
        HelpRow, KEY_GAP, KEY_INDENT, MIN_DESC, box_width, desc_col, entry_rows, help_lines,
        key_width, max_key_width, wrap_words,
    };
    use crate::tui::ui::help::content::{every_pane, help_rows};
    use crate::tui::workspace::PaneId;

    fn first_col(row: &str) -> Option<usize> {
        row.chars().position(|c| c != ' ')
    }

    #[test]
    fn a_wrapped_description_continues_under_the_description_column() {
        let key_w = 8;
        let col = desc_col(key_w);
        let rows = entry_rows(
            "n",
            "notes, a new note attaches to the selected row",
            key_w,
            40,
        );
        assert!(rows.len() > 1, "this description must wrap at 40: {rows:?}");
        assert_eq!(first_col(&rows[0]), Some(KEY_INDENT + key_w - 1), "the key");
        for cont in &rows[1..] {
            assert_eq!(
                first_col(cont),
                Some(col),
                "a continuation must start in the description column: {cont:?}"
            );
        }
        assert!(
            rows.iter().all(|row| row.chars().count() <= 40),
            "no row may overflow its box: {rows:?}"
        );
    }

    #[test]
    fn a_description_that_fits_is_left_alone() {
        let rows = entry_rows("r", "refresh account data", 8, 60);
        assert_eq!(rows, vec!["         r  refresh account data".to_string()]);
    }

    #[test]
    fn the_widest_key_sets_the_description_column_for_every_entry() {
        let rows = help_rows(PaneId::TradingWatch);
        let key_w = key_width(max_key_width(&rows), 80);
        let widest = rows
            .iter()
            .filter_map(|row| match row {
                HelpRow::Entry { key, .. } => Some(key.clone()),
                _ => None,
            })
            .max_by_key(|key| key.chars().count())
            .expect("the Watch pane documents keys");
        assert_eq!(key_w, widest.chars().count(), "widest key is {widest:?}");
        let col = desc_col(key_w);
        for row in &rows {
            if let HelpRow::Entry { key, desc } = row {
                let first = entry_rows(key, desc, key_w, 80).remove(0);
                let after_key: String = first.chars().skip(col).collect();
                assert_eq!(
                    first_col(&after_key),
                    Some(0),
                    "description must start at column {col}: {first:?}"
                );
            }
        }
    }

    #[test]
    fn a_narrow_box_shrinks_the_key_column_before_the_description() {
        assert_eq!(key_width(17, 80), 17, "a wide box pays for the widest key");
        assert_eq!(key_width(17, 26), 26 - KEY_INDENT - KEY_GAP - MIN_DESC);
        assert_eq!(key_width(17, 10), 0, "nothing left to give");
    }

    #[test]
    fn an_oversized_key_takes_a_row_of_its_own() {
        let rows = entry_rows(
            "Shift-J / Shift-K",
            "move the selected row down / up",
            6,
            30,
        );
        assert_eq!(rows[0], "  Shift-J / Shift-K");
        let col = desc_col(6);
        for cont in &rows[1..] {
            assert_eq!(first_col(cont), Some(col), "{cont:?}");
        }
    }

    #[test]
    fn wrap_words_breaks_on_spaces_and_splits_an_unbreakable_word() {
        assert_eq!(wrap_words("one two three", 8), vec!["one two", "three"]);
        assert_eq!(wrap_words("aaaaaaaaaa", 4), vec!["aaaa", "aaaa", "aa"]);
        assert_eq!(wrap_words("", 10), vec![""]);
        assert_eq!(wrap_words("   ", 10), vec![""]);
        assert_ne!(wrap_words("hi there", 0).len(), 0);
    }

    #[test]
    fn no_line_ever_exceeds_the_inner_width_at_any_width() {
        for pane in every_pane() {
            let rows = help_rows(pane);
            for width in 20..=140u16 {
                let inner = box_width(&rows, width).saturating_sub(2);
                for line in help_lines(&rows, inner) {
                    let drawn: usize = line
                        .spans
                        .iter()
                        .map(|span| span.content.chars().count())
                        .sum();
                    assert!(
                        drawn <= inner as usize,
                        "{pane:?} at {width}: line of {drawn} cells in {inner}: {line:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_box_never_asks_for_more_than_the_frame() {
        let rows = help_rows(PaneId::TradingWatch);
        for width in [20u16, 60, 80, 100, 120, 200] {
            assert!(box_width(&rows, width) <= width, "at {width}");
        }
        assert!(box_width(&rows, 200) < 120, "the modal stays a modal");
    }
}
