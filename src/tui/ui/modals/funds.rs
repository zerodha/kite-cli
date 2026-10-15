use super::layout::modal_block;
use crate::tui::app::App;
use crate::tui::format;
use crate::tui::ui::centered;
use ratatui::Frame;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

const FUNDS_LABEL_W: u16 = 20;
const FUNDS_VAL_W: u16 = 16;

#[expect(
    clippy::too_many_lines,
    reason = "declarative fund rows keep segment display order together"
)]
pub(super) fn funds_lines(app: &App) -> Vec<Line<'static>> {
    use crate::tui::data::FundsSegment;

    let label_width = usize::from(FUNDS_LABEL_W);
    let value_width = usize::from(FUNDS_VAL_W);

    let header_row = |left: &str, equity: &str, commodity: &str, style: Style| {
        Line::from(vec![
            Span::styled(format!("{left:<label_width$}"), style),
            Span::styled(format!("{equity:>value_width$}"), style),
            Span::styled("  ", style),
            Span::styled(format!("{commodity:>value_width$}"), style),
        ])
    };

    let money_row = |label: &str, equity: f64, commodity: f64| {
        let cell = |value: f64| {
            let style = if value == 0.0 {
                format::dim()
            } else {
                Style::default()
            };
            Span::styled(format!("{:>value_width$}", format::inr(value)), style)
        };
        Line::from(vec![
            Span::styled(format!("{label:<label_width$}"), format::dim()),
            cell(equity),
            Span::raw("  "),
            cell(commodity),
        ])
    };

    let eq = app.funds.as_ref().map(|f| &f.equity);
    let co = app.funds.as_ref().and_then(|f| f.commodity.as_ref());
    let default_seg = FundsSegment::default();
    let eq = eq.unwrap_or(&default_seg);
    let co = co.unwrap_or(&default_seg);

    let mut lines: Vec<Line> = Vec::new();

    if app.funds.is_none() {
        lines.push(Line::from(Span::styled("loading funds…", format::dim())));
    }

    lines.push(header_row(
        "",
        "Equity",
        "Commodity",
        format::bold().add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::default());

    lines.push(Line::from(Span::styled("Available", format::title_style())));
    lines.push(money_row("Cash", eq.available.cash, co.available.cash));
    lines.push(money_row(
        "Opening balance",
        eq.available.opening_balance,
        co.available.opening_balance,
    ));
    lines.push(money_row(
        "Collateral",
        eq.available.collateral,
        co.available.collateral,
    ));
    lines.push(money_row(
        "Intraday payin",
        eq.available.intraday_payin,
        co.available.intraday_payin,
    ));
    lines.push(money_row(
        "Live balance",
        eq.available.live_balance.unwrap_or(0.0),
        co.available.live_balance.unwrap_or(0.0),
    ));

    lines.push(Line::default());
    lines.push(Line::from(Span::styled("Utilised", format::title_style())));
    lines.push(money_row("Debits", eq.utilised.debits, co.utilised.debits));
    lines.push(money_row(
        "Collateral (Liquid)",
        eq.utilised.liquid_collateral,
        co.utilised.liquid_collateral,
    ));
    lines.push(money_row(
        "Collateral (Equity)",
        eq.utilised.stock_collateral,
        co.utilised.stock_collateral,
    ));
    lines.push(money_row("SPAN", eq.utilised.span, co.utilised.span));
    lines.push(money_row(
        "Exposure",
        eq.utilised.exposure,
        co.utilised.exposure,
    ));
    lines.push(money_row(
        "Option premium",
        eq.utilised.option_premium,
        co.utilised.option_premium,
    ));
    lines.push(money_row(
        "M2M realised",
        eq.utilised.m2m_realised,
        co.utilised.m2m_realised,
    ));
    lines.push(money_row(
        "M2M unrealised",
        eq.utilised.m2m_unrealised,
        co.utilised.m2m_unrealised,
    ));
    lines.push(money_row(
        "Delivery",
        eq.utilised.delivery,
        co.utilised.delivery,
    ));
    lines.push(money_row("Payout", eq.utilised.payout, co.utilised.payout));

    lines.push(Line::default());
    let net_cell = |value: f64| {
        let style = if value == 0.0 {
            format::dim()
        } else {
            format::bold()
        };
        Span::styled(format!("{:>value_width$}", format::inr(value)), style)
    };
    lines.push(Line::from(vec![
        Span::styled(format!("{:<label_width$}", "Net"), format::bold()),
        net_cell(eq.net),
        Span::raw("  "),
        net_cell(co.net),
    ]));

    lines
}

pub(super) fn draw_funds(frame: &mut Frame, app: &App) {
    let lines = funds_lines(app);
    let box_width: u16 = FUNDS_LABEL_W + FUNDS_VAL_W + 2 + FUNDS_VAL_W + 2;
    let height = (u16::try_from(lines.len()).unwrap_or(u16::MAX) + 2).min(frame.area().height);
    let area = centered(frame.area(), box_width, height);
    let inner = modal_block(
        frame,
        app,
        area,
        " Funds ".to_string(),
        Style::default().fg(format::accent()),
    );
    frame.render_widget(Paragraph::new(lines), inner);
}

#[cfg(test)]
mod render_tests {
    use super::super::render_tests::{rendered_text, test_app};
    use super::draw_funds;

    #[test]
    fn funds_modal_renders_liquid_and_equity_collateral_components() {
        let mut app = test_app();
        let mut funds = crate::tui::data::test_models::funds();
        funds.equity.utilised.liquid_collateral = 123.0;
        funds.equity.utilised.stock_collateral = 679.0;
        app.funds = Some(funds);

        let text = rendered_text(100, 30, |frame| draw_funds(frame, &app));
        assert!(text.contains("Collateral (Liquid)"), "{text}");
        assert!(text.contains("Collateral (Equity)"), "{text}");
        assert!(text.contains("₹123"), "{text}");
        assert!(text.contains("₹679"), "{text}");
    }
}
