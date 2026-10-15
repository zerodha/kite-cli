use crate::tui::app::AlertHistoryState;
use crate::tui::data::AlertTrigger;

use crate::tui::format;
use crate::tui::ui;
use crate::tui::ui::columns::{self, FitCol, col};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

pub(super) fn history_height(area_h: u16, state: &AlertHistoryState) -> u16 {
    let triggers = match &state.triggers {
        Some(Ok(triggers)) => u16::try_from(triggers.len().clamp(1, 3)).unwrap_or(u16::MAX),
        _ => 1,
    };
    let wanted = 2 + triggers * TRIGGER_LINES;
    wanted
        .min((area_h / 2).max(MIN_HISTORY_H))
        .max(MIN_HISTORY_H)
}

const TRIGGER_LINES: u16 = 3;
const MIN_HISTORY_H: u16 = 3;
const TRIGGER_INDENT: &str = "  ";

pub(super) fn draw_alert_history(frame: &mut Frame, area: Rect, state: &AlertHistoryState) {
    let count = match &state.triggers {
        Some(Ok(triggers)) => format!(" · {} ", plural_triggers(triggers.len())),
        _ => " ".to_string(),
    };
    let kind = match state.basket_len {
        Some(len) => format!(" · ATO {len} order(s)"),
        None => String::new(),
    };
    let block = ui::panel(format!(
        " Trigger history · {}{kind}{count}",
        state.label.trim()
    ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = match &state.triggers {
        None => vec![prose("loading trigger history…")],
        Some(Err(err)) => vec![prose(&format!(
            "history unavailable, Enter to retry ({err})"
        ))],
        Some(Ok(triggers)) if triggers.is_empty() => {
            vec![prose(
                "not triggered yet, no triggers recorded for this alert",
            )]
        }
        Some(Ok(triggers)) => {
            let room = (inner.height / TRIGGER_LINES).max(1) as usize;
            triggers
                .iter()
                .take(room)
                .flat_map(|trigger| trigger_lines(trigger, inner.width))
                .collect()
        }
    };
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn prose(text: &str) -> Line<'static> {
    Line::from(Span::styled(format!(" {text}"), format::dim()))
}

fn plural_triggers(count: usize) -> String {
    if count == 1 {
        "1 trigger".to_string()
    } else {
        format!("{count} triggers")
    }
}

fn trigger_lines(trigger: &AlertTrigger, width: u16) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(vec![
            Span::styled("▸ ", format::dim()),
            Span::styled(trigger.created_at.to_string(), format::bold()),
        ]),
        Line::from(vec![
            Span::raw(TRIGGER_INDENT),
            Span::styled(trigger.condition.clone(), Style::default()),
        ]),
    ];
    let budget = width.saturating_sub(u16::try_from(TRIGGER_INDENT.len()).unwrap_or(u16::MAX));
    let mut spans = vec![Span::raw(TRIGGER_INDENT)];
    spans.extend(trigger_quote_spans(trigger, budget));
    lines.push(Line::from(spans));
    lines
}

const TRIGGER_GAP: u16 = 2;

fn trigger_quote_spans(trigger: &AlertTrigger, budget: u16) -> Vec<Span<'static>> {
    let Some(quote) = crate::tui::data::trigger_quote(trigger) else {
        return vec![Span::styled("no quote recorded", format::dim())];
    };
    let mut segments: Vec<(String, Style)> =
        vec![(format::price(quote.last_price), format::bold())];
    if crate::tui::data::trigger_placed_orders(trigger) {
        segments.push((
            "orders placed".to_string(),
            Style::default().fg(format::warning()),
        ));
    }
    segments.push((
        format::signed_price(quote.net_change),
        format::change_style(quote.net_change),
    ));
    segments.push((
        format!(
            "O {} H {} L {} C {}",
            format::price(quote.ohlc.open),
            format::price(quote.ohlc.high),
            format::price(quote.ohlc.low),
            format::price(quote.ohlc.close),
        ),
        format::dim(),
    ));
    if quote.volume > 0 {
        segments.push((
            format!(
                "vol {}",
                format::compact_qty(u64::try_from(quote.volume).unwrap())
            ),
            format::dim(),
        ));
    }
    fit_segments(budget, TRIGGER_GAP, &segments)
}

fn fit_segments(budget: u16, gap: u16, segments: &[(String, Style)]) -> Vec<Span<'static>> {
    let cols: Vec<FitCol> = segments
        .iter()
        .enumerate()
        .map(|(idx, (text, _))| {
            col(
                u16::try_from(text.chars().count()).unwrap_or(u16::MAX),
                u8::try_from(idx).unwrap_or(u8::MAX),
            )
        })
        .collect();
    let fit = columns::fit_columns(budget, gap, &cols);
    let kept: Vec<&(String, Style)> = segments
        .iter()
        .enumerate()
        .filter(|(idx, _)| fit.shows(*idx))
        .map(|(_, segment)| segment)
        .collect();
    let width: usize = kept
        .iter()
        .map(|(text, _)| text.chars().count())
        .sum::<usize>()
        + gap as usize * kept.len().saturating_sub(1);
    if width > budget as usize {
        return Vec::new();
    }
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (idx, (text, style)) in kept.into_iter().enumerate() {
        if idx > 0 {
            spans.push(Span::raw(" ".repeat(gap as usize)));
        }
        spans.push(Span::styled(text.clone(), *style));
    }
    spans
}

#[cfg(test)]
mod alert_history_tests {
    use super::{draw_alert_history, trigger_quote_spans};
    use crate::tui::app::AlertHistoryState;
    use crate::tui::data::AlertTrigger;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn rows_of(
        width: u16,
        height: u16,
        draw: impl FnOnce(&mut ratatui::Frame, ratatui::layout::Rect),
    ) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw(frame, frame.area())).unwrap();
        let buffer = terminal.backend().buffer().clone();
        (0..height)
            .map(|y| (0..width).map(|x| buffer[(x, y)].symbol()).collect())
            .collect()
    }

    fn trigger(condition: &str, price: f64, ato: bool) -> AlertTrigger {
        {
            let mut row = crate::tui::data::test_models::history();
            row.condition = condition.to_string();
            row.created_at = "2025-02-17T09:16:46".parse().expect("timestamp");
            row.meta = vec![{
                let mut row = crate::tui::data::test_models::trigger_quote();
                row.last_price = price;
                row.net_change = -1_269.9;
                row.ohlc = {
                    let mut row = crate::tui::data::test_models::ohlc();
                    row.open = 59_221.95;
                    row.high = 59_241.8;
                    row.low = 58_283.7;
                    row.close = 59_557.95;
                    row
                };
                row.volume = 0;
                row
            }];
            row.order_meta = ato.then(|| serde_json::json!({"orders": []}));
            row
        }
    }

    fn history_rows(state: &AlertHistoryState, width: u16, height: u16) -> Vec<String> {
        rows_of(width, height, |frame, area| {
            draw_alert_history(frame, area, state);
        })
    }

    fn state(triggers: Option<Result<Vec<AlertTrigger>, String>>) -> AlertHistoryState {
        AlertHistoryState {
            uuid: "u1".to_string(),
            request: crate::tui::data::AlertRequestId(1),
            label: "NIFTY upper".to_string(),
            triggers,
            basket_len: None,
        }
    }

    #[test]
    fn a_trigger_shows_when_it_fired_the_condition_and_the_price() {
        let condition = "LastTradedPrice(\"INDICES:NIFTY NEXT 50\") <= 58290.35";
        let state = state(Some(Ok(vec![trigger(condition, 58_288.05, false)])));
        let rendered = history_rows(&state, 100, 6).join("\n");
        assert!(rendered.contains("2025-02-17 09:16:46"), "{rendered}");
        assert!(rendered.contains(condition), "{rendered}");
        assert!(rendered.contains("58,288.05"), "{rendered}");
        assert!(rendered.contains("NIFTY upper"), "{rendered}");
        assert!(rendered.contains("1 trigger"), "{rendered}");
    }

    #[test]
    fn an_alert_that_never_fired_shows_the_empty_state_not_the_loading_state() {
        let empty = history_rows(&state(Some(Ok(Vec::new()))), 80, 4).join("\n");
        assert!(empty.contains("not triggered yet"), "{empty}");
        assert!(!empty.contains("loading"), "{empty}");

        let loading = history_rows(&state(None), 80, 4).join("\n");
        assert!(loading.contains("loading"), "{loading}");
        assert!(!loading.contains("not triggered yet"), "{loading}");

        let failed = history_rows(&state(Some(Err("offline".into()))), 80, 4).join("\n");
        assert!(
            failed.contains("offline") && failed.contains("Enter"),
            "{failed}"
        );
    }

    #[test]
    fn an_ato_trigger_says_orders_were_placed() {
        let ato = state(Some(Ok(vec![trigger("x <= 1", 71.8, true)])));
        let rendered = history_rows(&ato, 100, 6).join("\n");
        assert!(rendered.contains("orders placed"), "{rendered}");
        let simple = state(Some(Ok(vec![trigger("x <= 1", 71.8, false)])));
        assert!(
            !history_rows(&simple, 100, 6)
                .join("\n")
                .contains("orders placed")
        );
    }

    #[test]
    fn trigger_quote_segments_drop_from_the_right_and_never_clip_a_price() {
        let trigger = trigger("x", 58_288.05, false);
        let text = |budget: u16| -> String {
            trigger_quote_spans(&trigger, budget)
                .iter()
                .map(|span| span.content.to_string())
                .collect()
        };
        let wide = text(120);
        assert!(wide.contains("58,288.05") && wide.contains("-1,269.90"));
        assert!(wide.contains("O 59,221.95"));
        let narrow = text(24);
        assert!(narrow.contains("58,288.05"), "{narrow:?}");
        assert!(!narrow.contains("O 59,221.95"), "{narrow:?}");
        for budget in 0..=120u16 {
            let line = text(budget);
            assert!(
                line.chars().count() <= budget as usize,
                "budget {budget} renders {} cells: {line:?}",
                line.chars().count()
            );
            assert!(
                line.is_empty() || line.contains("58,288.05"),
                "budget {budget} drew something other than the whole price: {line:?}"
            );
        }
    }

    #[test]
    fn a_zero_volume_snapshot_draws_no_volume_segment() {
        let mut trigger = trigger("x", 100.0, false);
        let text = |trigger: &AlertTrigger| -> String {
            trigger_quote_spans(trigger, 200)
                .iter()
                .map(|span| span.content.to_string())
                .collect()
        };
        assert!(!text(&trigger).contains("vol"));
        trigger.meta[0].volume = 4_823_910;
        assert!(text(&trigger).contains("vol 48.24L"));
    }

    #[test]
    fn a_trigger_without_a_quote_says_so() {
        let mut trigger = trigger("x <= 1", 100.0, false);
        trigger.meta.clear();
        let state = state(Some(Ok(vec![trigger])));
        let rendered = history_rows(&state, 80, 6).join("\n");
        assert!(rendered.contains("no quote recorded"), "{rendered}");
    }

    #[test]
    fn the_history_panel_never_takes_more_than_half_the_body() {
        let many: Vec<AlertTrigger> = (0..9).map(|_| trigger("x <= 1", 1.0, false)).collect();
        let state = state(Some(Ok(many)));
        for height in 6..=40u16 {
            let asked = super::history_height(height, &state);
            assert!(
                asked <= (height / 2).max(super::MIN_HISTORY_H),
                "height {height}: panel asked for {asked}"
            );
        }
    }
}
