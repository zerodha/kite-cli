use super::send_unless_dry_run;
use crate::app::output::Output;
use crate::app::render::{self, Cell, Col, Doc, Table};
use crate::app::runtime::Runtime;
use crate::cli::PortfolioCommands;
use crate::util::wire_value;
use anyhow::Result;

pub async fn handle(rt: &Runtime, command: PortfolioCommands) -> Result<Output> {
    let renderer = match &command {
        PortfolioCommands::Holdings => holdings,
        PortfolioCommands::Positions => render::positions,
        PortfolioCommands::Auctions | PortfolioCommands::Convert(_) => auctions,
    };
    let value = match command {
        PortfolioCommands::Holdings => rt.with_session(|api| api.holdings().raw()).await?,
        PortfolioCommands::Positions => rt.with_session(|api| api.positions().raw()).await?,
        PortfolioCommands::Auctions => {
            rt.with_session(|api| api.auction_instruments().raw())
                .await?
        }
        PortfolioCommands::Convert(args) => {
            let params = kiteconnect::ConvertPositionParams::new(
                wire_value(&args.exchange)?,
                args.tradingsymbol,
                wire_value(&args.old_product)?,
                wire_value(&args.new_product)?,
                wire_value(&args.position_type)?,
                wire_value(&args.transaction_type)?,
                args.quantity,
            );
            return send_unless_dry_run(
                rt,
                &args.confirm,
                |api| api.convert_position(params.clone()),
                "convert the position",
                "Position conversion submitted",
            )
            .await;
        }
    };
    Ok(Output::view(value, renderer))
}

fn auctions(value: &serde_json::Value, doc: &mut Doc) {
    render::records(
        value,
        &[
            "auction_number",
            "exchange",
            "tradingsymbol",
            "quantity",
            "average_price",
            "last_price",
            "pnl",
        ],
        doc,
    );
}

#[expect(
    clippy::cast_precision_loss,
    reason = "holding calculations use f64 quantities as the TUI does"
)]
fn holdings(value: &serde_json::Value, doc: &mut Doc) {
    use crate::tui::data::{
        holding_has_pnl, holding_live_day_pnl, holding_live_pnl, holding_total_quantity,
    };
    let Some(rows) = value.as_array().filter(|rows| !rows.is_empty()) else {
        doc.empty("No holdings.");
        return;
    };
    let mut table = Table::new(vec![
        Col::text("INSTRUMENT"),
        Col::number("QTY"),
        Col::decimal("AVG").drop(1),
        Col::decimal("LTP"),
        Col::decimal("CUR VALUE"),
        Col::decimal("P&L"),
        Col::decimal("NET CHG").drop(3),
        Col::decimal("DAY CHG").drop(2),
    ]);
    let (mut invested, mut current, mut day_pnl, mut previous) = (0.0, 0.0, 0.0, 0.0);
    let mut excluded = 0;
    for row in rows {
        let holding = serde_json::from_value::<kiteconnect::Holding>(row.clone()).ok();
        let quantity = holding.as_ref().map(holding_total_quantity);
        let values = holding
            .as_ref()
            .filter(|holding| holding_has_pnl(holding, None))
            .map(|holding| {
                let quantity = holding_total_quantity(holding) as f64;
                let cost = holding.average_price * quantity;
                let value = holding.last_price * quantity;
                let pnl = holding_live_pnl(holding, None);
                let day = holding_live_day_pnl(holding, None);
                invested += cost;
                current += value;
                day_pnl += day;
                previous += value - day;
                (cost, value, pnl)
            });
        if values.is_none() {
            excluded += 1;
        }
        table.row(vec![
            render::instrument(row).into(),
            quantity.map_or_else(|| "–".into(), render::qty),
            render::price_field(row, "average_price"),
            render::price_field(row, "last_price"),
            values.map_or_else(|| "–".into(), |(_, value, _)| render::price(value, "")),
            values.map_or_else(|| "–".into(), |(_, _, pnl)| render::change(pnl)),
            values.filter(|(cost, _, _)| *cost > 0.0).map_or_else(
                || "–".into(),
                |(cost, _, pnl)| render::change_pct(pnl / cost * 100.0),
            ),
            row["day_change_percentage"]
                .as_f64()
                .map_or_else(|| "–".into(), render::change_pct),
        ]);
    }
    table.total(vec![
        "Total".into(),
        "".into(),
        "".into(),
        "".into(),
        render::price(current, ""),
        render::change(current - invested),
        if invested > 0.0 {
            render::change_pct((current - invested) / invested * 100.0)
        } else {
            "–".into()
        },
        if previous > 0.0 {
            render::change_pct(day_pnl / previous * 100.0)
        } else {
            "–".into()
        },
    ]);
    doc.table(table).blank();
    holdings_summary(doc, invested, current, day_pnl);
    if excluded > 0 {
        doc.warn(&format!(
            "excluded {excluded} incomplete, discrepant or unpriced holdings from totals"
        ));
    }
}

fn holdings_summary(doc: &mut Doc, invested: f64, current: f64, day_pnl: f64) {
    let mut day = render::change(day_pnl);
    day.text
        .insert(usize::from(day.text.starts_with(['+', '-'])), '₹');
    doc.line(vec![
        Cell::from(format!(
            "Invested {} · Current {} · Day's P&L",
            render::rupees(invested),
            render::rupees(current)
        )),
        day,
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mockup_totals_use_unrounded_costs() {
        let mut fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../mock_responses/holdings.json")).unwrap();
        fixture["data"][0]["mtf"]["quantity"] = serde_json::json!(0);
        let mut doc = Doc::default();
        holdings(&fixture["data"], &mut doc);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        doc.write(
            &mut stdout,
            render::Term::default(),
            &mut stderr,
            render::Term::default(),
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(stdout).unwrap(),
            "INSTRUMENT  QTY     AVG     LTP  CUR VALUE      P&L   NET CHG  DAY CHG\nNSE:AARON     1  161.00  352.95     352.95  +191.95  +119.22%   +0.17%\nBSE:SBIN     16  801.78  762.45  12,199.20  -629.30    -4.91%   -0.52%\n──────────────────────────────────────────────────────────────────────\nTotal                            12,552.15  -437.35    -3.37%   -0.50%\n\nInvested ₹12,989.50 · Current ₹12,552.15 · Day's P&L -₹62.60\n"
        );
        assert_eq!(stderr, b"");
    }
}
