use super::quote::QUOTE_GAP;
use crate::tui::format;
use crate::tui::ticks;
use kiteconnect::ticker::Tick;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

pub(super) const MISSING: &str = "?";
const OVERFLOW: &str = "overflow";

pub(super) fn fit_value(value: &str, width: usize) -> String {
    if format::display_width(value) <= width {
        value.to_string()
    } else if format::display_width(OVERFLOW) <= width {
        OVERFLOW.to_string()
    } else if width > 0 {
        MISSING.to_string()
    } else {
        String::new()
    }
}

fn right_row(text: &str, style: Style, width: usize) -> Line<'static> {
    Line::from(Span::styled(
        format::pad_or_clip_right(&fit_value(text, width), width),
        style,
    ))
}

pub(super) fn ltp_rows(tick: &Tick, width: usize) -> Vec<Line<'static>> {
    const LTP_LABEL: &str = "LTP ";
    let value = tick
        .last_price
        .map_or_else(|| MISSING.into(), format::price);
    let (label, value) =
        if format::display_width(LTP_LABEL) + format::display_width(&value) <= width {
            (LTP_LABEL, value)
        } else {
            ("", fit_value(&value, width))
        };
    let left_w = format::display_width(label) + format::display_width(&value);
    let mut left: Vec<Span<'static>> = Vec::new();
    if !label.is_empty() {
        left.push(Span::styled(label, format::dim()));
    }
    left.push(Span::styled(value, format::header_style()));

    let style = ticks::change(tick)
        .map(format::change_style)
        .unwrap_or_default();
    let change = ticks::change(tick).map(format::signed_price);
    let percent = ticks::change_percent(tick).map(format::percent);
    let group = match (&change, &percent) {
        (Some(change), Some(percent)) => Some(format!("{change} ({percent})")),
        (Some(only), None) | (None, Some(only)) => Some(only.clone()),
        (None, None) => None,
    };
    let Some(group) = group else {
        return vec![Line::from(left)];
    };

    let gap = format::display_width(QUOTE_GAP);
    let group_w = format::display_width(&group);
    if left_w + gap + group_w <= width {
        left.push(Span::raw(" ".repeat(width - left_w - group_w)));
        left.push(Span::styled(group, style));
        return vec![Line::from(left)];
    }
    let mut rows = vec![Line::from(left)];
    match (change, percent) {
        (Some(change), Some(percent)) if group_w > width => {
            rows.push(right_row(&change, style, width));
            rows.push(right_row(&percent, style, width));
        }
        _ => rows.push(right_row(&group, style, width)),
    }
    rows
}
