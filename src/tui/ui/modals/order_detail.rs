use super::layout::{modal_block, wrapped_rows};
use crate::tui::app::{App, OrderDetailState};

use crate::tui::format;
use crate::tui::ui::centered;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, Wrap};

pub(super) fn draw_order_detail(frame: &mut Frame, state: &OrderDetailState, app: &App) {
    let order = &state.order;
    let history_rows = match &state.history {
        Some(Ok(history)) => u16::try_from(history.len().min(6)).unwrap_or(u16::MAX) + 2,
        _ => 2,
    };

    let mut rows: Vec<(String, String)> = vec![
        ("Time".into(), order.order_timestamp.to_string()),
        (
            "Symbol".into(),
            format!("{}:{}", order.exchange, order.tradingsymbol),
        ),
        (
            "Side".into(),
            format!(
                "{} {} {}",
                order.transaction_type, order.order_type, order.product
            ),
        ),
        (
            "Qty".into(),
            format!(
                "{} filled / {} total ({} pending)",
                order.filled_quantity, order.quantity, order.pending_quantity
            ),
        ),
        ("Price".into(), format::price(order.price)),
        ("Trigger".into(), format::price(order.trigger_price)),
        ("Avg fill".into(), format::price(order.average_price)),
        (
            "Validity".into(),
            crate::tui::data::order_validity_label(order),
        ),
        ("Variety".into(), order.variety.clone()),
        ("Status".into(), order.status.clone()),
    ];
    if !crate::tui::data::order_kind_label(order).is_empty() {
        rows.insert(
            9,
            (
                "Kind".into(),
                crate::tui::data::order_kind_label(order).to_string(),
            ),
        );
    }

    let field_count = u16::try_from(rows.len()).unwrap_or(u16::MAX);
    let table = Table::new(
        rows.into_iter().map(|(key, value)| {
            Row::new(vec![
                Cell::from(Span::styled(key, format::dim())),
                Cell::from(value),
            ])
        }),
        [Constraint::Length(10), Constraint::Min(0)],
    );

    let box_width: u16 = 58.min(frame.area().width);
    let inner_width = box_width.saturating_sub(2);
    let reason = order
        .status_message
        .as_deref()
        .map(str::trim)
        .filter(|msg| !msg.is_empty());
    let reason_rows: u16 = match reason {
        Some(msg) => 2 + wrapped_rows(msg, inner_width),
        None => 0,
    };

    let height = (2 + field_count + reason_rows + history_rows).min(frame.area().height);
    let area = centered(frame.area(), box_width, height);
    let inner = modal_block(
        frame,
        app,
        area,
        format!(" Order {} ", order.order_id),
        Style::default().fg(format::accent()),
    );

    let [fields_area, reason_area, history_area] = Layout::vertical([
        Constraint::Length(field_count),
        Constraint::Length(reason_rows),
        Constraint::Min(0),
    ])
    .areas(inner);
    frame.render_widget(table, fields_area);

    if let Some(msg) = reason {
        let reason_lines = vec![
            Line::default(),
            Line::from(Span::styled("Message", format::dim())),
            Line::from(Span::styled(msg, Style::default().fg(format::error()))),
        ];
        frame.render_widget(
            Paragraph::new(reason_lines).wrap(Wrap { trim: false }),
            reason_area,
        );
    }

    let lines = order_history_lines(state);
    frame.render_widget(Paragraph::new(lines), history_area);
}

fn order_history_lines(state: &OrderDetailState) -> Vec<Line<'static>> {
    let mut lines = vec![Line::default()];
    match &state.history {
        None => lines.push(Line::from(Span::styled("loading history…", format::dim()))),
        Some(Err(err)) => lines.push(Line::from(Span::styled(
            format!("history unavailable: {err}"),
            Style::default().fg(format::error()),
        ))),
        Some(Ok(history)) => {
            lines.push(Line::from(Span::styled("HISTORY", format::dim())));
            for entry in history.iter().rev().take(6).rev() {
                let time = entry.order_timestamp.format("%H:%M:%S").to_string();
                lines.push(Line::from(vec![
                    Span::styled(format!("{time:<10}"), format::dim()),
                    Span::raw(format!(
                        "{:<18} {}/{}",
                        entry.status, entry.filled_quantity, entry.quantity
                    )),
                ]));
            }
        }
    }
    lines
}

#[cfg(test)]
mod render_tests {
    use super::super::render_tests::{rendered_text, test_app};
    use super::draw_order_detail;
    use crate::tui::app::OrderDetailState;
    use crate::tui::data::Order;

    #[test]
    fn a_narrow_rejection_keeps_the_whole_reason() {
        let mut order: Order = crate::tui::data::test_models::order();
        order.order_id = "251009000000001".into();
        order.order_timestamp = "2026-09-09T10:15:00".parse().unwrap();
        order.status = "REJECTED".into();
        order.status_message = Some("RMS: Margin exceeds, required 1,23,456.00, available 12,345.00 for entity account-AB1234 across exchange segment NSE equity END".into());
        order.exchange = "NSE".into();
        order.tradingsymbol = "INFY".into();
        order.transaction_type = "BUY".into();
        order.order_type = "LIMIT".into();
        order.product = "CNC".into();
        order.quantity = 10.0;
        let state = OrderDetailState {
            request: crate::tui::data::OrderHistoryRequest {
                order_id: order.order_id.clone(),
                seq: 1,
            },
            order,
            history: Some(Ok(Vec::new())),
        };
        let app = test_app();
        for (w, h) in [(40, 24), (80, 24), (120, 35), (160, 45)] {
            let text = rendered_text(w, h, |frame| draw_order_detail(frame, &state, &app));
            assert!(
                text.contains("END"),
                "{w}x{h}: the reason tail is lost:\n{text}"
            );
        }
    }
}
