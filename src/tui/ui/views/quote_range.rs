use crate::tui::format;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

fn circuit_marker_color(low: f64, value: f64, high: f64) -> ratatui::style::Color {
    let frac = if high <= low {
        0.5
    } else {
        ((value - low) / (high - low)).clamp(0.0, 1.0)
    };
    if (0.05..=0.95).contains(&frac) {
        format::text_muted()
    } else {
        format::warning()
    }
}

const RANGE_BAR_W: usize = 12;

fn range_bar(low: f64, value: f64, high: f64, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let last = width - 1;
    let idx = if high <= low {
        last / 2
    } else {
        let frac = ((value - low) / (high - low)).clamp(0.0, 1.0);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "clamped range ratio rounds to a terminal column"
        )]
        let column = (frac * last as f64).round() as usize;
        column
    }
    .min(last);
    (0..width)
        .map(|i| if i == idx { '●' } else { '─' })
        .collect()
}

pub(super) fn range_line(label: &str, low: f64, value: f64, high: f64) -> Line<'static> {
    let frac = if high <= low {
        0.5
    } else {
        ((value - low) / (high - low)).clamp(0.0, 1.0)
    };
    let marker_color = if frac >= 2.0 / 3.0 {
        format::up()
    } else if frac <= 1.0 / 3.0 {
        format::down()
    } else {
        format::text_muted()
    };
    range_line_colored(label, low, value, high, marker_color)
}

fn range_bar_spans(low: f64, value: f64, high: f64, marker_color: Color) -> Vec<Span<'static>> {
    let bar = range_bar(low, value, high, RANGE_BAR_W);
    let mut spans: Vec<Span<'static>> = vec![
        Span::styled(format!("{:>9} ", format::price(low)), format::dim()),
        Span::styled("├".to_string(), format::dim()),
    ];
    if let Some(pos) = bar.find('●') {
        let (before, rest) = bar.split_at(pos);
        let after = &rest['●'.len_utf8()..];
        if !before.is_empty() {
            spans.push(Span::styled(before.to_string(), format::dim()));
        }
        spans.push(Span::styled(
            "●".to_string(),
            Style::default().fg(marker_color),
        ));
        if !after.is_empty() {
            spans.push(Span::styled(after.to_string(), format::dim()));
        }
    } else {
        spans.push(Span::styled(bar, format::dim()));
    }
    spans.push(Span::styled("┤".to_string(), format::dim()));
    spans.push(Span::styled(
        format!(" {}", format::price(high)),
        format::dim(),
    ));
    spans
}

fn range_line_colored(
    label: &str,
    low: f64,
    value: f64,
    high: f64,
    marker_color: Color,
) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = vec![Span::styled(format!("{label:<4} "), format::dim())];
    spans.extend(range_bar_spans(low, value, high, marker_color));
    Line::from(spans)
}

pub(super) fn circuit_line(lower: f64, ltp: f64, upper: f64) -> Line<'static> {
    let marker_color = circuit_marker_color(lower, ltp, upper);
    let mut spans: Vec<Span<'static>> = vec![Span::styled("LC ", format::dim())];
    spans.extend(range_bar_spans(lower, ltp, upper, marker_color));
    spans.push(Span::styled(" UC", format::dim()));
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::range_bar;

    fn marker_index(bar: &str) -> usize {
        bar.chars()
            .position(|c| c == '●')
            .expect("bar has a marker")
    }

    #[test]
    fn range_bar_places_marker_at_ends_and_middle() {
        assert_eq!(range_bar(100.0, 100.0, 200.0, 5).chars().count(), 5);
        assert_eq!(range_bar(100.0, 100.0, 200.0, 5).matches('●').count(), 1);

        assert_eq!(marker_index(&range_bar(100.0, 100.0, 200.0, 5)), 0);
        assert_eq!(marker_index(&range_bar(100.0, 200.0, 200.0, 5)), 4);
        assert_eq!(marker_index(&range_bar(100.0, 150.0, 200.0, 5)), 2);
    }

    #[test]
    fn range_bar_centres_on_zero_span() {
        assert_eq!(marker_index(&range_bar(100.0, 100.0, 100.0, 5)), 2);
        assert_eq!(marker_index(&range_bar(200.0, 150.0, 100.0, 5)), 2);
    }

    #[test]
    fn range_bar_clamps_out_of_range_values() {
        assert_eq!(marker_index(&range_bar(100.0, 50.0, 200.0, 5)), 0);
        assert_eq!(marker_index(&range_bar(100.0, 500.0, 200.0, 5)), 4);
    }

    #[test]
    fn range_bar_zero_width_is_empty() {
        assert_eq!(range_bar(100.0, 150.0, 200.0, 0), "");
    }
}
