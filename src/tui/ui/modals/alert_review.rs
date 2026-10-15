use crate::tui::app::AlertFormState;
use crate::tui::format;
use crate::tui::ui::forms;
use ratatui::style::Style;

fn review_orders(
    heading: String,
    numbered: bool,
    orders: &[&crate::tui::alerts::DraftItem],
    value_style: Style,
) -> Vec<forms::FormItem> {
    let prefixes: Vec<String> = orders
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let marker = if numbered {
                (index + 1).to_string()
            } else {
                "!".to_string()
            };
            format!("{marker} {}", item.label())
        })
        .collect();
    let width = prefixes
        .iter()
        .map(|prefix| format::display_width(prefix))
        .max()
        .unwrap_or(0)
        + 2;
    let mut items = vec![forms::FormItem::Section(heading)];
    for (prefix, item) in prefixes.iter().zip(orders) {
        items.push(forms::FormItem::List(forms::ListRow {
            field: None,
            index: 0,
            selected: false,
            label: format::pad_or_clip(prefix, width),
            value: item.summary(),
            value_style,
            note: None,
        }));
    }
    items
}

pub(super) fn alert_review_lines(state: &AlertFormState, wide: bool) -> Vec<forms::FormItem> {
    let mut items = Vec::new();
    if state.drops_the_basket() {
        items.push(forms::FormItem::Error(
            "Saving as Simple DELETES the order basket on your Kite account.".to_string(),
        ));
    }
    if state.ato && state.draft.baseline_unavailable() {
        items.push(forms::FormItem::Section("Orders".to_string()));
        items.push(forms::FormItem::Warning(
            "This alert's saved orders did not load. They are unchanged by this save and are not listed here."
                .to_string(),
        ));
    } else if state.ato {
        let orders: Vec<&crate::tui::alerts::DraftItem> = state.draft.items().iter().collect();
        let heading = if state.saves_enabled() {
            "Orders armed"
        } else {
            "Orders saved"
        };
        items.extend(review_orders(
            format!("{heading} ({})", orders.len()),
            true,
            &orders,
            Style::default(),
        ));
        let removed = state.draft.removed();
        if !removed.is_empty() {
            items.extend(review_orders(
                format!("Orders removed ({})", removed.len()),
                false,
                &removed,
                Style::default().fg(format::error()),
            ));
        }
    }
    items.push(forms::FormItem::Section("Impact".to_string()));
    if state.ato {
        items.push(forms::FormItem::Note(
            "MARKET basket orders use price-protected LIMIT orders.".to_string(),
        ));
    }
    items.push(forms::FormItem::Warning(
        match (state.ato, state.saves_enabled()) {
            (true, true) => "Saving arms this alert on your Kite account.",
            (true, false) => "Saving stores these orders; the alert stays disabled until re-armed.",
            (false, _) => "Saving updates this alert on your Kite account.",
        }
        .to_string(),
    ));
    if state.saves_enabled() {
        items.push(forms::FormItem::Note(
            "It can fire while the terminal is closed.".to_string(),
        ));
    }
    if wide {
        items.push(forms::FormItem::Note(String::new()));
        items.push(forms::FormItem::Note(
            "Opaque items keep their saved settings.".to_string(),
        ));
    }
    items
}

#[cfg(test)]
mod render_tests {
    use super::super::draw_alert;
    use super::super::render_tests::{
        assert_mockup, mockup_app, mockup_ato, modal_ascii, modal_rect, rect_tuple, rendered_text,
    };
    use crate::tui::app::{AlertFormStage, AlertFormState};
    use crate::tui::ui::HitAction;

    fn fetched_order(
        id: i64,
        symbol: &str,
        token: u32,
        order_type: &str,
        quantity: u32,
        price: f64,
    ) -> crate::tui::data::FetchedBasketItem {
        {
            let mut row = crate::tui::data::test_models::basket_item();
            row.id = Some(id);
            row.exchange = "NSE".into();
            row.tradingsymbol = symbol.into();
            row.instrument_token = Some(token);
            row.weight = id;
            row.params = serde_json::json!({
                "variety": "regular",
                "transaction_type": "BUY",
                "product": "CNC",
                "order_type": order_type,
                "validity": "DAY",
                "validity_ttl": 0,
                "quantity": quantity,
                "disclosed_quantity": 0,
                "price": price,
                "trigger_price": 0,
                "squareoff": 0,
                "stoploss": 0,
                "trailing_stoploss": 0,
                "gtt": {"target": 0, "stoploss": 0},
                "tags": []
            });
            row
        }
    }

    fn mockup_ato_review() -> AlertFormState {
        let items = vec![
            fetched_order(1, "INFY", 408_065, "LIMIT", 10, 1505.0),
            fetched_order(2, "TCS", 2_953_217, "MARKET", 1, 0.0),
            fetched_order(3, "WIPRO", 969_473, "LIMIT", 1, 500.0),
        ];
        let mut draft = crate::tui::alerts::AtoBasketDraft::from_fetched(&items);
        draft.remove(2);
        AlertFormState {
            stage: AlertFormStage::Review,
            draft,
            ..mockup_ato()
        }
    }

    #[test]
    fn ato_review_matches_the_spec_mockup() {
        let app = mockup_app();
        let state = mockup_ato_review();
        assert_mockup(
            &modal_ascii(80, 24, |frame| draw_alert(frame, &state, &app)),
            "
+- Review ATO / NSE:INFY ----------------------------------------------+
| Last price >= 1510.00 | status enabled | edited alert                |
+----------------------------------------------------------------------+
| Orders armed (2)                                                     |
| 1 NSE:INFY  BUY 10 CNC LIMIT 1505.00 DAY                             |
| 2 NSE:TCS   BUY 1 CNC MARKET DAY                                     |
| Orders removed (1)                                                   |
| ! NSE:WIPRO  BUY 1 CNC LIMIT 500.00 DAY                              |
| Impact                                                               |
| MARKET basket orders use price-protected LIMIT orders.               |
| Saving arms this alert on your Kite account.                         |
| It can fire while the terminal is closed.                            |
+----------------------------------------------------------------------+
| Review all armed and removed orders.  PgUp/PgDn Scroll               |
| [y/Enter Save to Kite]  [Esc Back to form]                           |
+----------------------------------------------------------------------+
",
        );
        assert_eq!(rect_tuple(modal_rect(&app)), (4, 4, 72, 16));

        assert_mockup(
            &modal_ascii(120, 35, |frame| draw_alert(frame, &state, &app)),
            "
+- Review ATO / NSE:INFY --------------------------------------------------------------+
| Last price >= 1510.00 | status enabled | edited alert                                |
+--------------------------------------------------------------------------------------+
| Orders armed (2)                                                                     |
| 1 NSE:INFY  BUY 10 CNC LIMIT 1505.00 DAY                                             |
| 2 NSE:TCS   BUY 1 CNC MARKET DAY                                                     |
| Orders removed (1)                                                                   |
| ! NSE:WIPRO  BUY 1 CNC LIMIT 500.00 DAY                                              |
| Impact                                                                               |
| MARKET basket orders use price-protected LIMIT orders.                               |
| Saving arms this alert on your Kite account.                                         |
| It can fire while the terminal is closed.                                            |
|                                                                                      |
| Opaque items keep their saved settings.                                              |
+--------------------------------------------------------------------------------------+
| Review all armed and removed orders.  PgUp/PgDn Scroll                               |
| [y/Enter Save to Kite]  [Esc Back to form]                                           |
+--------------------------------------------------------------------------------------+
",
        );
        assert_eq!(rect_tuple(modal_rect(&app)), (16, 8, 88, 18));
    }

    #[test]
    fn choosing_simple_on_a_stored_ato_warns_that_the_basket_is_deleted() {
        let app = mockup_app();
        let state = AlertFormState {
            ato: false,
            stage: AlertFormStage::Review,
            ..mockup_ato()
        };
        let text = rendered_text(100, 30, |frame| draw_alert(frame, &state, &app));
        assert!(
            text.contains("Saving as Simple DELETES the order basket"),
            "dropping a basket must be stated:\n{text}"
        );
    }

    #[test]
    fn a_submitting_ato_form_says_it_is_sending() {
        let app = mockup_app();
        let state = AlertFormState {
            stage: AlertFormStage::Submitting(crate::tui::data::AlertRequestId(1)),
            ..mockup_ato()
        };
        let text = rendered_text(100, 30, |frame| draw_alert(frame, &state, &app));
        assert!(text.contains("Orders armed"), "the review stays:\n{text}");
        assert!(text.contains("Sending to Kite..."), "{text}");
        assert!(text.contains("Esc Close (request continues)"), "{text}");
        assert!(text.contains("[y/Enter Save to Kite]"), "{text}");
        let sends = app
            .hit
            .borrow()
            .regions
            .iter()
            .filter(|region| {
                region.action
                    == HitAction::Key(crossterm::event::KeyEvent::new(
                        crossterm::event::KeyCode::Char('y'),
                        crossterm::event::KeyModifiers::NONE,
                    ))
            })
            .count();
        assert_eq!(sends, 0, "a request in flight offers no second send");
    }

    #[test]
    fn a_disabled_ato_review_does_not_claim_to_arm_its_orders() {
        let app = mockup_app();
        let disabled = AlertFormState {
            stage: AlertFormStage::Review,
            status: kiteconnect::AlertStatus::Disabled,
            ..mockup_ato()
        };
        let text = rendered_text(80, 40, |frame| draw_alert(frame, &disabled, &app));
        assert!(text.contains("Orders saved (2)"), "{text}");
        assert!(!text.contains("Orders armed"), "{text}");
        assert!(!text.contains("Saving arms this alert"), "{text}");
        assert!(
            !text.contains("It can fire while the terminal is closed"),
            "a muted alert cannot fire at all:\n{text}"
        );
        assert!(text.contains("stays disabled until re-armed"), "{text}");
        assert!(text.contains("Saving keeps it muted"), "{text}");

        let enabled = AlertFormState {
            status: kiteconnect::AlertStatus::Enabled,
            ..disabled
        };
        let text = rendered_text(80, 40, |frame| draw_alert(frame, &enabled, &app));
        assert!(text.contains("Orders armed (2)"), "{text}");
        assert!(text.contains("Saving arms this alert"), "{text}");
        assert!(
            text.contains("It can fire while the terminal is closed"),
            "{text}"
        );
    }
}
