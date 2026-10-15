use super::layout::{draw_select_list, modal_block};
use crate::instruments::Instrument;
use crate::tui::app::{App, SearchPurpose, SearchState};
use crate::tui::format;
use crate::tui::ui::HitList;
use crate::tui::ui::centered;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{ListItem, Paragraph, Wrap};

pub(super) fn draw_search(frame: &mut Frame, state: &SearchState, app: &App) {
    let area = centered(frame.area(), 78, 20);
    let title = match state.purpose {
        SearchPurpose::AddToWatch => " Add instrument ",
        SearchPurpose::ChainUnderlying => " Chain underlying ",
        SearchPurpose::NewAlert => " Alert on instrument ",
        SearchPurpose::AtoBasketItem => " Basket order instrument ",
    };
    let inner = modal_block(
        frame,
        app,
        area,
        title.to_string(),
        Style::default().fg(format::accent()),
    );
    let [input_area, results_area] =
        Layout::vertical([Constraint::Length(2), Constraint::Min(0)]).areas(inner);

    let input_line = Line::from(vec![
        Span::styled("❯ ", Style::default().fg(format::accent())),
        Span::raw(state.input.clone()),
        Span::styled("▏", Style::default().fg(format::accent())),
    ]);
    frame.render_widget(Paragraph::new(input_line), input_area);

    if app.instruments.is_none() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                app.instruments_error.as_ref().map_or_else(
                    || "loading instrument master… (first run downloads ~90 MB)".to_string(),
                    |error| {
                        format!(
                            "instrument master unavailable: {error}. Ctrl-R retries the download"
                        )
                    },
                ),
                format::dim(),
            ))
            .wrap(Wrap { trim: true }),
            results_area,
        );
        return;
    }
    if state.input.trim().is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "type a symbol or name, e.g. INFY, NIFTY, NFO:INFY",
                format::dim(),
            )),
            results_area,
        );
        return;
    }
    if state.results.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled("no matches", format::dim())),
            results_area,
        );
        return;
    }

    let items: Vec<ListItem> = state
        .results
        .iter()
        .map(|inst| ListItem::new(Line::from(instrument_row_spans(inst, true))))
        .collect();

    draw_select_list(
        frame,
        app,
        results_area,
        items,
        Some(state.selected.min(state.results.len() - 1)),
        HitList::Search,
        "▶ ",
    );
}

const INSTRUMENT_LABEL_W: usize = 28;
const INSTRUMENT_TYPE_W: usize = 6;

pub(super) fn instrument_row_spans(inst: &Instrument, with_expiry: bool) -> Vec<Span<'static>> {
    let mut spans = vec![
        Span::styled(
            format::pad_or_clip(&inst.label(), INSTRUMENT_LABEL_W),
            format::bold(),
        ),
        Span::styled(
            format::pad_or_clip(inst.display_type(), INSTRUMENT_TYPE_W),
            format::dim(),
        ),
    ];
    if !inst.name.is_empty() {
        spans.push(Span::raw(inst.name.clone()));
    }
    if with_expiry && let Some(expiry) = inst.expiry.as_deref().filter(|expiry| !expiry.is_empty())
    {
        spans.push(Span::styled(format!("  {expiry}"), format::dim()));
    }
    spans
}
