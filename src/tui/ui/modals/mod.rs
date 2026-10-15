mod alert;
mod alert_review;
mod ato;
mod confirm;
mod conversion;
mod events;
mod funds;
mod layout;
mod note_editor;
mod notes;
mod order_detail;
mod palette;
mod picker;
mod search;
mod theme;
mod ticket;

use crate::tui::app::{App, Modal, Side};
use crate::tui::format;
use crate::tui::ui::forms;
pub(super) use alert::alert_footer;
use alert::draw_alert;
pub(super) use ato::ato_item_footer;
use ato::draw_ato_item;
pub(super) use confirm::confirm_footer;
use confirm::draw_confirm;
pub(super) use conversion::conversion_footer;
use conversion::draw_position_convert;
use events::draw_event_log;
pub(super) use events::event_log_footer;
use funds::{draw_funds, funds_lines};
pub(super) use layout::{modal_block, wrapped_rows};
pub(super) use note_editor::note_editor_footer;
use notes::draw_notes;
pub(super) use notes::notes_footer;
use order_detail::draw_order_detail;
use palette::draw_palette;
use picker::draw_workspace_picker;
pub(super) use picker::workspace_picker_footer;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::Paragraph;
use search::draw_search;
use theme::draw_theme_picker;
pub(super) use theme::theme_picker_footer;
use ticket::draw_ticket;
pub(super) use ticket::ticket_footer;

pub fn draw_modal(frame: &mut Frame, modal: &Modal, app: &App) {
    match modal {
        Modal::Help(scroll) => super::help::draw_help(frame, app, *scroll),
        Modal::Search(state) => draw_search(frame, state, app),
        Modal::Palette(state) => draw_palette(frame, state, app),
        Modal::ThemePicker(state) => draw_theme_picker(frame, state, app),
        Modal::WorkspacePicker(state) => draw_workspace_picker(frame, *state, app),
        Modal::Ticket(state) => draw_ticket(frame, state, app),
        Modal::PositionConvert(state) => draw_position_convert(frame, state, app),
        Modal::Confirm(state) => draw_confirm(frame, state, app),
        Modal::OrderDetail(state) => draw_order_detail(frame, state, app),
        Modal::Alert(state) => draw_alert(frame, state, app),
        Modal::AtoItem(state) => draw_ato_item(frame, state, app),
        Modal::EventLog(state) => draw_event_log(frame, state, app),
        Modal::Notes(state) => draw_notes(frame, state, app),
        Modal::Funds => draw_funds(frame, app),
    }
}

pub(super) fn side_color(side: Side) -> Color {
    match side {
        Side::Buy => format::up(),
        Side::Sell => format::down(),
    }
}

const FIELD_HELP: &[&str] = &["Tab/Shift-Tab Field", "Up/Down Step", "Left/Right Choice"];

const CANCEL: forms::FormAction = forms::FormAction {
    key: "Esc",
    label: "Cancel",
    kind: forms::ActionKind::Secondary,
};

const TOO_SMALL: &str = "Esc closes without sending.";

pub(super) fn draw_funds_pane(frame: &mut Frame, area: Rect, app: &App, focused: bool) {
    let block = super::focus_panel("Funds", focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.is_empty() {
        return;
    }
    let lines = funds_lines(app);
    let max = lines.len().saturating_sub(usize::from(inner.height));
    let scroll = app.funds_scroll.get().min(max);
    app.funds_scroll.set(scroll);
    frame.render_widget(
        Paragraph::new(lines).scroll((u16::try_from(scroll).unwrap_or(u16::MAX), 0)),
        inner,
    );
}

#[cfg(test)]
mod render_tests {
    use super::{draw_event_log, draw_theme_picker, forms, notes_footer, theme_picker_footer};
    use crate::tui::alerts::{Attribute, Operator};
    use crate::tui::app::{
        AlertFormFocus, AlertFormStage, AlertFormState, App, AtoItemFocus, AtoItemFormState,
        EventLogState, Side, ThemePickerState, ToastLevel,
    };
    use crate::tui::form::{FormIssue, FormViewport};
    use crate::tui::notes::{NoteEditor, NotesState, Pad};
    use crate::tui::settings;
    use crate::tui::settings::Settings;
    use kiteconnect::KiteConnect;
    use kiteconnect::ticker::Tick;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    pub(super) fn rendered_text<F: FnOnce(&mut ratatui::Frame)>(w: u16, h: u16, draw: F) -> String {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(draw).unwrap();
        let buffer = terminal.backend().buffer().clone();
        let mut out = String::new();
        for y in 0..h {
            for x in 0..w {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    pub(super) fn modal_ascii<F: FnOnce(&mut ratatui::Frame)>(w: u16, h: u16, draw: F) -> String {
        let text = rendered_text(w, h, draw);
        let rows: Vec<&str> = text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect();
        let indent = rows
            .iter()
            .map(|line| line.len() - line.trim_start().len())
            .min()
            .unwrap_or(0);
        rows.iter()
            .map(|line| {
                line[indent..]
                    .trim_end()
                    .replace(['┌', '┐', '└', '┘', '├', '┤'], "+")
                    .replace('─', "-")
                    .replace('│', "|")
                    .replace('█', "#")
            })
            .collect::<Vec<String>>()
            .join("\n")
    }

    pub(super) fn assert_mockup(actual: &str, expected: &str) {
        let expected: String = expected
            .trim_matches('\n')
            .lines()
            .map(|line| line.trim_end().replace('_', " "))
            .collect::<Vec<String>>()
            .join("\n");
        assert_eq!(actual, expected);
    }

    pub(super) fn test_app() -> App {
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

    pub(super) fn mockup_app() -> App {
        let mut app = test_app();
        app.ticks.insert(408_065, {
            let mut value = Tick::default();
            value.last_price = Some(1500.0);
            value
        });
        app
    }

    pub(super) fn modal_rect(app: &App) -> ratatui::layout::Rect {
        app.hit
            .borrow()
            .regions
            .iter()
            .rev()
            .find(|region| region.action == super::super::HitAction::Capture)
            .expect("a modal captures its own box")
            .rect
    }

    pub(super) fn rect_tuple(rect: ratatui::layout::Rect) -> (u16, u16, u16, u16) {
        (rect.x, rect.y, rect.width, rect.height)
    }

    pub(super) fn mockup_alert() -> AlertFormState {
        AlertFormState {
            exchange: "NSE".into(),
            tradingsymbol: "INFY".into(),
            name: "INFY".into(),
            instrument_token: 408_065,
            attribute: Attribute::LastTradedPrice,
            level: String::new(),
            level_edited: true,
            operator: Operator::GreaterThanOrEqual,
            focus: AlertFormFocus::Level,
            tick_size: 0.05,
            issue: Some(FormIssue::at(
                AlertFormFocus::Level,
                "Enter a level to alert on.",
            )),
            uuid: None,
            status: kiteconnect::AlertStatus::Enabled,
            ato: false,
            was_ato: false,
            draft: crate::tui::alerts::AtoBasketDraft::new(),
            basket_selected: 0,
            viewport: FormViewport::default(),
            stage: AlertFormStage::Edit,
        }
    }

    pub(super) fn mockup_ato() -> AlertFormState {
        let items = vec![
            {
                let mut row = crate::tui::data::test_models::basket_item();
                row.id = Some(1);
                row.exchange = "NSE".into();
                row.tradingsymbol = "INFY".into();
                row.instrument_token = Some(408_065);
                row.weight = 1;
                row.params = serde_json::json!({
                    "variety": "regular",
                    "transaction_type": "BUY",
                    "product": "CNC",
                    "order_type": "LIMIT",
                    "validity": "DAY",
                    "validity_ttl": 0,
                    "quantity": 10,
                    "disclosed_quantity": 0,
                    "price": 1505.0,
                    "trigger_price": 0,
                    "squareoff": 0,
                    "stoploss": 0,
                    "trailing_stoploss": 0,
                    "gtt": {"target": 0, "stoploss": 0},
                    "tags": []
                });
                row
            },
            {
                let mut row = crate::tui::data::test_models::basket_item();
                row.id = Some(2);
                row.exchange = "NSE".into();
                row.tradingsymbol = "TCS".into();
                row.instrument_token = Some(2_953_217);
                row.weight = 2;
                row.params = serde_json::json!({
                    "order_type": "LIMIT",
                    "quantity": 1,
                    "price": 3200.0,
                    "disclosed_quantity": 1
                });
                row
            },
        ];
        AlertFormState {
            level: "1510.00".into(),
            ato: true,
            was_ato: true,
            uuid: Some("ato-uuid".into()),
            focus: AlertFormFocus::Basket,
            issue: None,
            draft: crate::tui::alerts::AtoBasketDraft::from_fetched(&items),
            ..mockup_alert()
        }
    }

    pub(super) fn mockup_ato_item() -> AtoItemFormState {
        AtoItemFormState {
            exchange: "NSE".into(),
            tradingsymbol: "INFY".into(),
            instrument_token: 408_065,
            editing: None,
            side: Side::Buy,
            product: 0,
            order_type: 1,
            validity: 2,
            quantity: "10".into(),
            price: "1505.00".into(),
            trigger: String::new(),
            ttl: "5".into(),
            focus: AtoItemFocus::Quantity,
            lot_size: 1,
            tick_size: 0.05,
            freeze_quantity: None,
            issue: None,
            viewport: FormViewport::default(),
        }
    }

    fn theme_results(count: usize) -> Vec<&'static str> {
        [
            "kite",
            "ayu",
            "catppuccin",
            "catppuccin-frappe",
            "catppuccin-macchiato",
            "dracula",
            "gruvbox",
            "nord",
            "solarized-light",
            "system",
        ]
        .iter()
        .take(count)
        .copied()
        .collect()
    }

    pub(super) fn theme_picker(count: usize) -> ThemePickerState {
        ThemePickerState {
            input: String::new(),
            selected: 0,
            results: theme_results(count),
            preview_base: settings::preset("kite"),
        }
    }

    pub(super) fn event_log_app() -> App {
        let mut app = test_app();
        app.push_event(14400, ToastLevel::Info, "Connected to Kite");
        app.push_event(14480, ToastLevel::Success, "Order update: INFY complete");
        app.push_event(
            14530,
            ToastLevel::Error,
            "Order rejected: insufficient margin",
        );
        app
    }

    pub(super) fn notes_app() -> App {
        let mut app = test_app();
        app.notes = Pad::loaded(crate::tui::notes::Notes::from_vec(vec![
            crate::tui::notes::Note {
                id: 1,
                body: "Watch the opening range.".to_string(),
                created: 1_789_012_800,
                updated: 1_789_012_860,
                instrument: Some(crate::tui::notes::NoteInstrument {
                    exchange: "NSE".to_string(),
                    tradingsymbol: "INFY".to_string(),
                }),
            },
            crate::tui::notes::Note {
                id: 2,
                body: "Review execution notes.".to_string(),
                created: 1_788_947_100,
                updated: 1_788_947_100,
                instrument: None,
            },
        ]));
        app
    }

    pub(super) fn notes_list_state() -> NotesState {
        NotesState {
            attach: Some(crate::tui::notes::NoteInstrument {
                exchange: "NSE".to_string(),
                tradingsymbol: "INFY".to_string(),
            }),
            ..NotesState::default()
        }
    }

    #[test]
    fn the_utility_modals_survive_a_terminal_too_small_for_them() {
        let app = event_log_app();
        let log = EventLogState::default();
        let themes = theme_picker(8);
        let notes = notes_list_state();
        let editor = NotesState {
            editor: Some(NoteEditor::draft(None)),
            ..NotesState::default()
        };
        for (w, h) in [(48, 18), (32, 10), (32, 9), (20, 4), (1, 1), (0, 0)] {
            rendered_text(w.max(1), h.max(1), |frame| {
                draw_event_log(frame, &log, &app);
            });
            rendered_text(w.max(1), h.max(1), |frame| {
                draw_theme_picker(frame, &themes, &app);
            });
            rendered_text(w.max(1), h.max(1), |frame| {
                super::draw_notes(frame, &notes, &app);
            });
            rendered_text(w.max(1), h.max(1), |frame| {
                super::draw_notes(frame, &editor, &app);
            });
        }
        let text = rendered_text(32, 9, |frame| draw_event_log(frame, &log, &app));
        assert!(text.contains("Resize to edit"), "{text}");
        assert!(!text.contains("Close"), "{text}");
    }

    #[test]
    fn the_utility_footers_and_hints_advertise_the_same_keys() {
        let footers = [
            theme_picker_footer(true),
            theme_picker_footer(false),
            super::event_log_footer(),
            notes_footer(false, true),
            notes_footer(false, false),
            notes_footer(true, true),
            super::note_editor_footer(false),
            super::note_editor_footer(true),
        ];
        for footer in footers {
            let hints: String = footer
                .hints
                .iter()
                .flat_map(|group| group.hints.iter())
                .copied()
                .collect::<Vec<&str>>()
                .join(" ");
            for action in footer.actions {
                if action.kind != forms::ActionKind::Primary
                    && action.kind != forms::ActionKind::Secondary
                {
                    continue;
                }
                assert!(
                    hints.contains(&format!("{}:", action.key)),
                    "{} is on a button but not in the hints: {hints}",
                    action.key
                );
            }
        }
    }
}
