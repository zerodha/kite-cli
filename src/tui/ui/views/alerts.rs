use super::alert_history::{draw_alert_history, history_height};
use super::book_state::state_panel;
use super::headers::{alerts_header_sort, alerts_sort_targets};
use super::table::{TableRows, record_header_hits, right_styled, selectable_table, sorted_title};
use crate::tui::app::{AlertsSort, App};
use crate::tui::data::Alert;
use crate::tui::format;
use crate::tui::ui;
use crate::tui::ui::columns::{self, FitCol, col, fit_constraints, fit_row, table_budget};
use crate::tui::ui::pane::{BookList, PaneHits};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, Wrap};

const ALERT_NAME_MIN: u16 = 12;
const ALERT_NAME_MAX: u16 = 36;
const ALERT_SYM_MIN: u16 = 16;
const ALERT_SYM_MAX: u16 = 24;
const ALERT_STATUS_W: u16 = 8;
const ALERT_CREATED_W: u16 = 10;
const ALERT_COND_MIN: u16 = 11;
const ALERT_COND_MAX: u16 = 40;
const ALERT_REASON_MAX: u16 = 24;
const ALERT_TYPE_W: u16 = 6;

fn alert_cols(alerts: &[Alert], area_width: u16) -> [FitCol; 8] {
    let widest = |width: fn(&Alert) -> usize| -> u16 {
        u16::try_from(alerts.iter().map(width).max().unwrap_or(0)).unwrap_or(u16::MAX)
    };
    let condition_w =
        widest(|alert| format::display_width(&crate::tui::data::alert_condition(alert)))
            .clamp(ALERT_COND_MIN, ALERT_COND_MAX.max(ALERT_COND_MIN));
    let natural_reason_w = match widest(|alert| format::display_width(alert_reason(alert))) {
        0 => 0,
        width => width.clamp(6, ALERT_REASON_MAX),
    };
    let reason_w = natural_reason_w.min(
        table_budget(area_width)
            .saturating_sub(ALERT_NAME_MIN + 1)
            .max(6),
    );
    let triggered_w = widest(|alert| alert.alert_count.to_string().chars().count()).max(9);
    [
        col(ALERT_NAME_MIN, 0),  // NAME, flexible, never dropped
        col(ALERT_SYM_MIN, 3),   // SYMBOL, the watched instrument
        col(condition_w, 2),     // CONDITION
        col(ALERT_STATUS_W, 4),  // STATUS
        col(reason_w, 1),        // REASON, why the service switched it off
        col(triggered_w, 5),     // TRIGGERED, alert_count
        col(ALERT_TYPE_W, 6),    // TYPE, SIMPLE / ATO with its order count
        col(ALERT_CREATED_W, 7), // CREATED
    ]
}

fn alert_disabled_reason(alert: &Alert) -> String {
    let reason = alert_reason(alert);
    if reason.is_empty() {
        return String::new();
    }
    format!("Trigger is disabled due to: '{reason}'. Modify and enable the alert.")
}

fn alert_reason(alert: &Alert) -> &str {
    alert.disabled_reason.as_deref().unwrap_or("").trim()
}

pub(super) const ALERT_LABELS: [(&str, bool); 8] = [
    ("NAME", false),
    ("SYMBOL", false),
    ("CONDITION", false),
    ("STATUS", false),
    ("REASON", false),
    ("TRIGGERED", true),
    ("TYPE", false),
    ("CREATED", false),
];

fn alerts_placeholder(alerts: Option<&[Alert]>) -> Option<(&'static str, Option<&'static str>)> {
    match alerts {
        None => Some(("Alerts not available yet", None)),
        Some([]) => Some(("No alerts", Some("a Set an alert"))),
        Some(_) => None,
    }
}

fn selected_alert_with_detail<'a>(
    alerts: &'a [Alert],
    list: &BookList<'_, AlertsSort>,
) -> Option<&'a Alert> {
    list.display
        .get(list.selected)
        .and_then(|&natural| alerts.get(natural))
        .filter(|alert| {
            !alert_reason(alert).is_empty()
                || crate::tui::data::alert_basket_len(alert).is_some_and(|len| len > 0)
        })
}

fn alert_detail_height(area_width: u16, alert: &Alert) -> u16 {
    let width = area_width.saturating_sub(2);
    let mut rows = 2u16;
    if !alert_reason(alert).is_empty() {
        let reason = format!("Reason: {}", alert_disabled_reason(alert));
        rows = rows.saturating_add(super::super::modals::wrapped_rows(&reason, width));
    }
    if let Some(basket) = &alert.basket {
        rows = rows.saturating_add(
            u16::try_from(basket.items.len().min(ALERT_BASKET_ROWS)).unwrap_or(u16::MAX) + 1,
        );
    }
    rows
}

const ALERT_BASKET_ROWS: usize = 5;

fn draw_alert_detail(frame: &mut Frame, area: Rect, alert: &Alert) {
    let block = ui::panel(format!(" Selected alert · {} ", alert.name));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let mut lines = Vec::new();
    if !alert_reason(alert).is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Reason: ", format::bold()),
            Span::raw(alert_disabled_reason(alert)),
        ]));
    }
    if let Some(basket) = &alert.basket
        && !basket.items.is_empty()
    {
        lines.push(Line::from(vec![
            Span::styled("Places on trigger: ", format::bold()),
            Span::styled(
                format!("{} order(s)", basket.items.len()),
                Style::default().fg(format::warning()),
            ),
        ]));
        for item in basket.items.iter().take(ALERT_BASKET_ROWS) {
            lines.push(Line::from(Span::raw(format!(
                "  {}  {}",
                if item.exchange.is_empty() {
                    item.tradingsymbol.clone()
                } else {
                    format!("{}:{}", item.exchange, item.tradingsymbol)
                },
                basket_item_summary(&item.params),
            ))));
        }
        if basket.items.len() > ALERT_BASKET_ROWS {
            lines.push(Line::from(Span::styled(
                format!("  … {} more, in e", basket.items.len() - ALERT_BASKET_ROWS),
                format::dim(),
            )));
        }
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn basket_item_summary(params: &serde_json::Value) -> String {
    let read = |key: &str| match params.get(key) {
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(value) if !value.is_null() => value.to_string(),
        _ => String::new(),
    };
    let mut parts: Vec<String> = ["transaction_type", "quantity", "product", "order_type"]
        .into_iter()
        .map(read)
        .filter(|value| !value.is_empty())
        .collect();
    let price = read("price");
    if read("order_type") != "MARKET" && !price.is_empty() && price != "0" {
        parts.push(format!("@ {price}"));
    }
    let trigger = read("trigger_price");
    if !trigger.is_empty() && trigger != "0" {
        parts.push(format!("trigger {trigger}"));
    }
    parts.join(" · ")
}

fn alert_row(
    alert: &crate::tui::data::Alert,
    fit: &columns::ColumnFit,
    layout: &columns::TableLayout,
) -> Row<'static> {
    let name_w = layout.width(0);
    let sym_w = layout.width(1);
    let enabled = crate::tui::data::alert_is_enabled(alert);
    let base = if enabled {
        Style::default()
    } else {
        format::dim()
    };
    let status_style = if enabled {
        Style::default().fg(format::up())
    } else {
        format::dim()
    };
    let warn = Style::default().fg(format::warning());
    fit_row(
        fit,
        &[
            &|| {
                Cell::from(Span::styled(
                    format::pad_or_clip(&alert.name, name_w as usize),
                    base,
                ))
            },
            &|| {
                Cell::from(Span::styled(
                    format::pad_or_clip(&crate::tui::data::alert_label(alert), sym_w as usize),
                    base,
                ))
            },
            &|| Cell::from(Span::styled(crate::tui::data::alert_condition(alert), base)),
            &|| {
                Cell::from(Span::styled(
                    if enabled { "enabled" } else { "disabled" },
                    status_style,
                ))
            },
            &|| {
                let reason = alert_reason(alert);
                Cell::from(Span::styled(
                    format::pad_or_clip(reason, layout.width(4) as usize),
                    warn,
                ))
            },
            &|| {
                right_styled(
                    alert.alert_count.to_string(),
                    if alert.alert_count > 0 {
                        base
                    } else {
                        format::dim()
                    },
                )
            },
            &|| {
                let (label, style) = match crate::tui::data::alert_basket_len(alert) {
                    None if crate::tui::data::alert_is_ato(alert) => ("ATO ?".to_string(), warn),
                    None => ("SIMPLE".to_string(), base),
                    Some(0) => ("ATO 0".to_string(), warn),
                    Some(count) => (
                        format!("ATO {count}"),
                        if crate::tui::data::alert_is_enabled(alert) {
                            Style::default().fg(format::accent())
                        } else {
                            format::dim()
                        },
                    ),
                };
                Cell::from(Span::styled(
                    format::pad_or_clip(&label, layout.width(6) as usize),
                    style,
                ))
            },
            &|| {
                Cell::from(Span::styled(
                    alert.created_at.format("%Y-%m-%d").to_string(),
                    format::dim(),
                ))
            },
        ],
    )
}

pub fn draw_alerts_pane(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    list: &BookList<'_, AlertsSort>,
    hits: &PaneHits<'_>,
) {
    if let Some((headline, hint)) = alerts_placeholder(app.alerts_fetched()) {
        state_panel(
            frame,
            area,
            " Alerts ",
            ui::MessageTone::Normal,
            headline,
            None,
            hint,
        );
        return;
    }
    let alerts = app.alert_list();

    let detail_alert = selected_alert_with_detail(alerts, list);
    let history = app.alert_history();
    let mut constraints = vec![Constraint::Min(4)];
    if let Some(alert) = detail_alert {
        constraints.push(Constraint::Length(alert_detail_height(area.width, alert)));
    }
    if let Some(state) = history {
        constraints.push(Constraint::Length(history_height(area.height, state)));
    }
    let areas = Layout::vertical(constraints).split(area);
    let table_area = areas[0];
    let mut next_area = 1;
    let detail_area = detail_alert.map(|_| {
        let area = areas[next_area];
        next_area += 1;
        area
    });
    let history_area = history.map(|state| {
        let area = areas[next_area];
        (area, state)
    });

    let targets = alerts_sort_targets();
    let cols = columns::sort_floors(
        &alert_cols(alerts, table_area.width),
        &ALERT_LABELS,
        &targets.map(|(idx, _)| idx),
    );
    let fit = columns::fit_columns(table_budget(table_area.width), 1, &cols);
    let layout = columns::spread(
        &fit,
        &cols,
        &[
            columns::text(
                0,
                columns::widest(
                    alerts
                        .iter()
                        .map(|alert| format::display_width(&alert.name)),
                ),
                ALERT_NAME_MAX,
            ),
            columns::text(
                1,
                columns::widest(
                    alerts
                        .iter()
                        .map(|alert| format::display_width(&crate::tui::data::alert_label(alert))),
                ),
                ALERT_SYM_MAX,
            ),
        ],
    );

    let display = list.display;
    let rows: Vec<Row> = display
        .iter()
        .map(|&i| alert_row(&alerts[i], &fit, &layout))
        .collect();

    let (header, marked) =
        columns::fit_header_sorted(&fit, &layout, &ALERT_LABELS, alerts_header_sort(list.sort));
    let title = sorted_title("Alerts · Kite", list.sort.label(), marked);
    let table = Table::new(rows, fit_constraints(&fit, &layout))
        .header(header)
        .column_spacing(layout.spacing)
        .block(ui::panel(list.title(&title)));

    record_header_hits(hits, table_area, &fit, &layout, &cols, &targets);
    selectable_table(
        frame,
        table_area,
        table,
        &layout,
        TableRows {
            selected: list.selected,
            count: alerts.len(),
            viewport: Some(list.viewport),
        },
        hits,
    );

    if let (Some(detail_area), Some(alert)) = (detail_area, detail_alert) {
        draw_alert_detail(frame, detail_area, alert);
    }
    if let Some((history_area, state)) = history_area {
        draw_alert_history(frame, history_area, state);
    }
}

#[cfg(test)]
mod alerts_tests {

    use super::super::draw_alerts;
    use super::{
        ALERT_CREATED_W, ALERT_LABELS, ALERT_NAME_MAX, ALERT_SYM_MAX, ALERT_SYM_MIN, ALERT_TYPE_W,
        alert_cols, alert_detail_height, alerts_placeholder, table_budget,
    };
    use crate::tui::app::App;
    use crate::tui::data::Alert;
    use crate::tui::notes::Pad;
    use crate::tui::settings::Settings;
    use crate::tui::ui::columns;
    use kiteconnect::KiteConnect;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    const WIDTHS: [u16; 5] = [60, 70, 80, 100, 200];

    fn test_app() -> App {
        let client = KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("tok").unwrap())
            .build()
            .expect("dummy client builds");
        let (events_tx, _events_rx) = tokio::sync::mpsc::channel(8);
        App::new(
            client,
            events_tx,
            crate::tui::marketwatch::State::test_loaded(Vec::new()),
            Pad::default(),
            false,
            &Settings::default(),
        )
    }

    fn disabled_alert() -> Alert {
        {
            let mut row = crate::tui::data::test_models::alert();
            row.alert_type =
                serde_json::from_value(serde_json::json!("simple")).expect("alert_type");
            row.uuid = "u1".into();
            row.name = "NIFTY upper".into();
            row.status = serde_json::from_value(serde_json::json!("disabled")).expect("status");
            row.disabled_reason = Some("Corporate action".into());
            row.lhs_attribute = "LastTradedPrice".into();
            row.lhs_exchange = "INDICES".into();
            row.lhs_tradingsymbol = "NIFTY 50".into();
            row.operator = ">=".into();
            row.rhs_type = "constant".into();
            row.rhs_constant = 27_000.0;
            row.alert_count = 3;
            row.created_at = "2025-02-17T08:24:10".parse().expect("timestamp");
            row
        }
    }

    fn healthy_alert() -> Alert {
        {
            let mut row = disabled_alert();
            row.uuid = "u2".into();
            row.name = "gold dip".into();
            row.status = serde_json::from_value(serde_json::json!("enabled")).expect("status");
            row.disabled_reason = Some(String::new());
            row.alert_count = 0;
            row
        }
    }

    fn alerts_app() -> App {
        let mut app = test_app();
        app.set_alerts(vec![disabled_alert()]);
        app
    }

    fn rows_at(width: u16, app: &App) -> Vec<String> {
        rows_of(width, 8, |frame, area| draw_alerts(frame, area, app))
    }

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

    fn alert_cells() -> Vec<(&'static str, String)> {
        vec![
            ("NAME", "NIFTY upper".to_string()),
            ("SYMBOL", "INDICES:NIFTY 50".to_string()),
            ("CONDITION", "Last price >= 27000".to_string()),
            ("STATUS", "disabled".to_string()),
            ("REASON", "Corporate action".to_string()),
            ("TRIGGERED", "3".to_string()),
            ("TYPE", "SIMPLE".to_string()),
            ("CREATED", "2025-02-17".to_string()),
        ]
    }

    #[test]
    fn alert_columns_always_fit_at_full_width() {
        let alerts = [disabled_alert(), healthy_alert()];
        for area_width in 10..=200u16 {
            let cols = alert_cols(&alerts, area_width);
            let fit = columns::fit_columns(table_budget(area_width), 1, &cols);
            let layout = columns::spread(
                &fit,
                &cols,
                &[
                    columns::text(0, ALERT_NAME_MAX, ALERT_NAME_MAX),
                    columns::text(1, ALERT_SYM_MAX, ALERT_SYM_MAX),
                ],
            );
            let mut used = layout.inset;
            let mut count = 0u16;
            for idx in 0..cols.len() {
                if fit.shows(idx) {
                    used += layout.width(idx);
                    count += 1;
                }
            }
            used += count.saturating_sub(1) * layout.spacing;
            assert!(
                used <= table_budget(area_width) || count == 1,
                "width {area_width}: columns render {used} in {}",
                table_budget(area_width)
            );
        }
    }

    #[test]
    fn alert_cells_are_whole_or_absent_at_every_width() {
        let app = alerts_app();
        for width in 30..=200u16 {
            let rows = rows_at(width, &app);
            for (label, value) in alert_cells() {
                if value.chars().count() < 4 {
                    continue; // too short for a partial render to be detectable
                }
                let tail: String = value.chars().skip(2).collect();
                for (y, row) in rows.iter().enumerate().take(3).skip(1) {
                    assert!(
                        row.contains(&value) || !row.contains(&tail) || row.contains('…'),
                        "width {width} row {y}: {label} rendered only {tail:?}: {row:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn alert_header_and_data_agree_at_every_width() {
        let app = alerts_app();
        for width in WIDTHS {
            let cols = alert_cols(&[disabled_alert()], width);
            let rows = rows_at(width, &app);
            let header = &rows[1];
            let data = &rows[2];
            for (idx, (label, value)) in alert_cells().into_iter().enumerate() {
                let shown = cols[idx].width > 0 && fit_shows(&cols, width, idx);
                assert_eq!(
                    header.contains(label),
                    shown,
                    "width {width}: header/{label} disagrees with the fit: {header:?}"
                );
                if idx == 0 {
                    continue; // the name ellipsises rather than disappearing
                }
                if shown {
                    let present = if idx == 4 {
                        data.contains(&value)
                            || (data.contains("Trigger is disabled due to:") && data.contains('…'))
                    } else {
                        data.contains(&value)
                    };
                    assert!(
                        present,
                        "width {width}: data/{label} missing despite the fit: {data:?}"
                    );
                } else if idx != 3 {
                    assert!(
                        !data.contains(&value),
                        "width {width}: data/{label} present despite the fit: {data:?}"
                    );
                }
            }
        }
        assert_eq!(
            ALERT_LABELS.len(),
            8,
            "labels and columns must describe the same table"
        );
    }

    fn fit_shows(cols: &[super::FitCol; 8], width: u16, idx: usize) -> bool {
        columns::fit_columns(table_budget(width), 1, cols).shows(idx)
    }

    #[test]
    fn a_disabled_reason_is_reachable_at_every_tested_width() {
        let app = alerts_app();
        for width in WIDTHS {
            let rows = rows_at(width, &app);
            let data = &rows[2];
            assert!(
                data.contains("Corporate action") || data.contains("Corporate"),
                "width {width} lost the disabled reason entirely: {data:?}"
            );
            assert!(
                rows[1].contains("REASON"),
                "width {width} drew a reason under no REASON header: {:?}",
                rows[1]
            );
        }
    }

    #[test]
    fn a_wide_alert_table_shows_kites_disabled_reason_in_full() {
        let reason = "The instrument used in this expression is not valid.";
        let mut alert = disabled_alert();
        alert.disabled_reason = Some(reason.to_string());
        let mut app = test_app();
        app.set_alerts(vec![alert]);

        let rendered = rows_at(280, &app).join("\n");
        assert!(
            rendered.contains(&format!(
                "Trigger is disabled due to: '{reason}'. Modify and enable the alert."
            )),
            "{rendered}"
        );
    }

    #[test]
    fn alert_detail_height_counts_word_wrap_at_the_actual_width() {
        let mut alert = disabled_alert();
        alert.disabled_reason = Some("one two three four five six seven".to_string());
        let width = 30;
        let reason = format!(
            "Reason: Trigger is disabled due to: '{}'. Modify and enable the alert.",
            alert.disabled_reason.as_deref().unwrap_or("")
        );
        assert_eq!(
            alert_detail_height(width, &alert),
            2 + super::super::super::modals::wrapped_rows(&reason, width - 2)
        );
    }

    #[test]
    fn alert_detail_reaches_the_final_sentence_after_word_wrap() {
        let mut alert = disabled_alert();
        alert.disabled_reason =
            Some("alpha beta gamma delta epsilon zeta eta theta iota kappa lambda".to_string());
        let mut app = test_app();
        app.set_alerts(vec![alert]);
        let rows = rows_of(60, 30, |frame, area| draw_alerts(frame, area, &app));
        let rendered = rows.join("\n");
        assert!(
            rendered.contains("Modify") && rendered.contains("enable the alert."),
            "detail sentence was clipped:\n{rendered}"
        );
    }

    #[test]
    fn alerts_table_labels_trigger_count_and_both_alert_types() {
        let mut ato = healthy_alert();
        ato.alert_type = kiteconnect::AlertType::Ato;
        let mut app = test_app();
        app.set_alerts(vec![disabled_alert(), ato]);

        let rows = rows_at(200, &app);
        assert!(rows[1].contains("TRIGGERED"), "{:?}", rows[1]);
        assert!(!rows[1].contains("FIRED"), "{:?}", rows[1]);
        assert!(rows[1].contains("TYPE"), "{:?}", rows[1]);
        let rendered = rows.join("\n");
        assert!(rendered.contains("SIMPLE"), "{rendered}");
        assert!(rendered.contains("ATO"), "{rendered}");
    }

    #[test]
    fn alert_columns_no_row_can_fill_cost_nothing() {
        let healthy = [healthy_alert()];
        let cols = alert_cols(&healthy, 200);
        assert_eq!(cols[4].width, 0, "nothing is disabled, so no reason column");
        assert_eq!(cols[6].width, ALERT_TYPE_W);

        let sick = [disabled_alert(), {
            let mut row = healthy_alert();
            row.alert_type = serde_json::from_value(serde_json::json!("ato")).expect("alert_type");
            row
        }];
        let cols = alert_cols(&sick, 200);
        assert!(cols[4].width > 0);
        assert_eq!(cols[6].width, ALERT_TYPE_W);
    }

    #[test]
    fn alert_column_priorities_are_unique_and_rooted_at_zero() {
        let cols = alert_cols(&[disabled_alert()], 200);
        let mut priorities: Vec<u8> = cols.iter().map(|c| c.priority).collect();
        priorities.sort_unstable();
        assert_eq!(
            priorities,
            (0..u8::try_from(cols.len()).unwrap()).collect::<Vec<u8>>()
        );
        assert_eq!(ALERT_LABELS.len(), cols.len());
    }

    #[test]
    fn alert_columns_drop_in_priority_order() {
        let alerts = [disabled_alert()];
        let shown = |width: u16| -> Vec<&'static str> {
            let cols = alert_cols(&alerts, width);
            let fit = columns::fit_columns(table_budget(width), 1, &cols);
            ALERT_LABELS
                .iter()
                .enumerate()
                .filter(|(idx, _)| fit.shows(*idx) && cols[*idx].width > 0)
                .map(|(_, (label, _))| *label)
                .collect()
        };
        let mut previous = shown(200);
        assert!(previous.contains(&"CREATED"), "all columns at 200");
        let mut dropped: Vec<&str> = Vec::new();
        for width in (10..200).rev() {
            let now = shown(width);
            for label in &now {
                assert!(
                    previous.contains(label),
                    "{label} reappeared at width {width}"
                );
            }
            for label in previous.iter().filter(|l| !now.contains(l)) {
                dropped.push(label);
            }
            previous = now;
        }
        assert_eq!(
            dropped,
            vec![
                "CREATED",
                "TYPE",
                "TRIGGERED",
                "STATUS",
                "SYMBOL",
                "CONDITION",
                "REASON"
            ]
        );
        assert_eq!(previous, vec!["NAME"], "the name always survives");
    }

    #[test]
    fn the_alerts_placeholder_separates_unknown_from_empty() {
        assert_eq!(
            alerts_placeholder(None),
            Some(("Alerts not available yet", None))
        );
        let empty: [Alert; 0] = [];
        assert_eq!(
            alerts_placeholder(Some(&empty)),
            Some(("No alerts", Some("a Set an alert")))
        );
        assert_eq!(alerts_placeholder(Some(&[healthy_alert()])), None);
    }

    #[test]
    fn long_names_and_symbols_clip_with_an_ellipsis() {
        let mut app = test_app();
        app.set_alerts(vec![{
            let mut row = healthy_alert();
            row.name = "a very long label the user typed on kite web".into();
            row.lhs_tradingsymbol = "NIFTY NEXT 50".into();
            row
        }]);
        for width in 40..=200u16 {
            let data = rows_at(width, &app)[2].clone();
            assert!(
                data.contains("a very long label the user typed on kite web") || data.contains('…'),
                "width {width} cut a cell with no ellipsis: {data:?}"
            );
        }
        assert_eq!(ALERT_SYM_MIN, 16);
        assert_eq!(ALERT_SYM_MAX, 24);
        assert_eq!(ALERT_CREATED_W, 10);
    }
}
