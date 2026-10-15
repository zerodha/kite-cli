use super::book_state::{BookState, focus_book_state};
use super::table::right_styled;
use super::{
    draw_auctions, draw_corporate_action_table, draw_gsec_table, draw_ipo_applications,
    draw_ipo_table,
};
use crate::tui::app::App;
use crate::tui::bids::{self, Section as BidsSection};
use crate::tui::format;
use crate::tui::ui;
use crate::tui::ui::pane::{BidsInput, PaneHits, PlainList};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph};

fn draw_bid_section_header(
    frame: &mut Frame,
    area: Rect,
    active: BidsSection,
    hits: &PaneHits<'_>,
) {
    let block = ui::panel(" Bids · [ ] section ");
    let inner = block.inner(area);

    let mut x = inner.x;
    let mut tabs = Vec::with_capacity(BidsSection::ALL.len());
    for section in BidsSection::ALL {
        let label = format!(" [{}] ", section.title());
        let width = u16::try_from(label.chars().count())
            .unwrap_or(u16::MAX)
            .min(inner.right().saturating_sub(x));
        if width > 0 {
            hits.push(
                Rect {
                    x,
                    y: inner.y,
                    width,
                    height: 1,
                },
                ui::HitAction::BidsSection(section),
            );
        }
        x = x.saturating_add(u16::try_from(label.chars().count()).unwrap_or(u16::MAX));
        let style = if section == active {
            format::selection_style().add_modifier(Modifier::UNDERLINED)
        } else {
            format::dim()
        };
        tabs.push(Span::styled(label, style));
    }
    frame.render_widget(Paragraph::new(Line::from(tabs)).block(block), area);
}

pub(super) fn bid_kind_cell(kind: &str) -> Cell<'static> {
    Cell::from(Span::styled(
        kind.to_ascii_uppercase(),
        Style::default().fg(format::info()),
    ))
}

pub(super) fn bid_symbol_cell(symbol: &str, tag: Option<&str>) -> Cell<'static> {
    let mut spans = vec![Span::raw(symbol.to_string())];
    if let Some(tag) = tag {
        spans.push(Span::styled(
            format!(" {tag}"),
            Style::default().fg(format::info()),
        ));
    }
    Cell::from(Line::from(spans))
}

pub(super) fn bid_status_cell(status: &str, live: bool) -> Cell<'static> {
    let style = if live {
        Style::default().fg(format::up())
    } else {
        format::dim()
    };
    right_styled(status.to_ascii_lowercase(), style)
}

pub(super) fn offer_price(value: f64, width: usize) -> String {
    if value > 0.0 {
        fit_amount(value, format::price(value), width)
    } else {
        ".".to_string()
    }
}

pub(super) fn fit_amount(value: f64, full: String, width: usize) -> String {
    if format::display_width(&full) <= width {
        full
    } else {
        format::fit_cell(&full, &format::compact_amount(value), width)
    }
}

fn draw_bid_instrument_table(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    rows: &[bids::Instrument],
    section: BidsSection,
    list: &PlainList<'_>,
    hits: &PaneHits<'_>,
) {
    match section {
        BidsSection::Ipo | BidsSection::Sse => {
            draw_ipo_table(frame, area, app, rows, section, list, hits);
        }
        BidsSection::Government => draw_gsec_table(frame, area, rows, section, list, hits),
        BidsSection::CorporateActions => {
            draw_corporate_action_table(frame, area, rows, section, list, hits);
        }
        BidsSection::Auctions => draw_auctions(frame, area, app, list, hits),
    }
}

pub fn draw_bids_pane(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    input: &BidsInput<'_>,
    hits: &PaneHits<'_>,
) {
    let section = input.section;
    let selected = input.selected;
    let [header, body] = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(area);
    draw_bid_section_header(frame, header, section, hits);
    let focused = input.focused;
    let focused_list = || PlainList {
        selected,
        focused,
        viewport: Some(input.viewport),
    };
    if section == BidsSection::Auctions {
        draw_auctions(frame, body, app, &focused_list(), hits);
        return;
    }
    if input.application_focus {
        draw_bid_applications_pane(frame, body, app, &focused_list(), hits);
        return;
    }
    if let Some(error) = app.bids.error(section) {
        focus_book_state(
            frame,
            body,
            section.title(),
            BookState::Failed(error),
            focused,
        );
        return;
    }
    let Some(rows) = app.bids.instruments(section) else {
        focus_book_state(frame, body, section.title(), BookState::Unknown, focused);
        return;
    };
    if rows.is_empty()
        && (section != BidsSection::Ipo
            || app
                .bids
                .applications
                .as_deref()
                .is_none_or(<[bids::Application]>::is_empty))
    {
        focus_book_state(
            frame,
            body,
            section.title(),
            BookState::Empty("No open bids or eligible instruments"),
            focused,
        );
        return;
    }
    if section == BidsSection::Ipo {
        let [instruments, applications] = if body.height >= 18 {
            Layout::vertical([Constraint::Min(9), Constraint::Length(9)]).areas(body)
        } else {
            Layout::vertical([Constraint::Min(0), Constraint::Length(0)]).areas(body)
        };
        if rows.is_empty() {
            focus_book_state(
                frame,
                instruments,
                "IPO instruments",
                BookState::Empty("No active IPOs"),
                focused,
            );
        } else {
            draw_bid_instrument_table(
                frame,
                instruments,
                app,
                rows,
                section,
                &focused_list(),
                hits,
            );
        }
        if applications.height > 0 {
            draw_bid_applications_pane(
                frame,
                applications,
                app,
                &PlainList {
                    selected,
                    focused: false,
                    viewport: None,
                },
                hits,
            );
        }
    } else {
        draw_bid_instrument_table(frame, body, app, rows, section, &focused_list(), hits);
    }
}

fn draw_bid_applications_pane(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    list: &PlainList<'_>,
    hits: &PaneHits<'_>,
) {
    const TITLE: &str = "IPO applications";
    let focused = list.focused;
    if let Some(error) = app.bids.applications_error.as_deref() {
        focus_book_state(frame, area, TITLE, BookState::Failed(error), focused);
        return;
    }
    match app.bids.applications.as_deref() {
        Some(applications) if !applications.is_empty() => {
            draw_ipo_applications(frame, area, applications, list, hits);
        }
        Some(_) => focus_book_state(
            frame,
            area,
            TITLE,
            BookState::Empty("No IPO applications"),
            focused,
        ),
        None => focus_book_state(frame, area, TITLE, BookState::Unknown, focused),
    }
}

#[cfg(test)]
mod bids_render_tests {
    use super::super::draw_bids;
    use crate::tui::app::App;
    use crate::tui::bids::{Application, Instrument, InvestorType};
    use crate::tui::notes::Pad;
    use crate::tui::settings::Settings;
    use crate::tui::workspace::PaneId;
    use kiteconnect::KiteConnect;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use tokio::sync::mpsc;

    fn app() -> App {
        let client = KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("tok").unwrap())
            .build()
            .unwrap();
        let (events_tx, _events_rx) = mpsc::channel(8);
        let mut app = App::new(
            client,
            events_tx,
            crate::tui::marketwatch::State::test_loaded(Vec::new()),
            Pad::default(),
            false,
            &Settings::default(),
        );
        app.set_pane(PaneId::InvestBids);
        app
    }

    fn rendered(app: &App) -> String {
        rendered_at(app, 100, 24)
    }

    fn rendered_at(app: &App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| draw_bids(frame, frame.area(), app))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect()
    }

    #[test]
    fn bids_render_loading_and_ipo_application_rows() {
        let unknown = rendered(&app());
        assert!(unknown.contains("IPO not available yet"), "{unknown}");
        assert!(unknown.contains("r Refresh"), "{unknown}");
        assert!(unknown.contains("Bids · [ ] section"), "{unknown}");

        let mut app = app();
        app.bids.ipo = Some(vec![
            Instrument {
                symbol: "ACME".into(),
                name: "Acme Limited".into(),
                status: "ongoing".into(),
                sub_type: "SME".into(),
                start_at: "2026-09-07 10:00:00".into(),
                end_at: "2026-09-09 17:00:00".into(),
                min_qty: 120,
                investor_types: vec![InvestorType {
                    min_price: 118.0,
                    max_price: 124.0,
                }],
                ..Default::default()
            },
            Instrument {
                symbol: "OLDIPO".into(),
                name: "Old Issue".into(),
                status: "closed".into(),
                end_at: "2026-08-27 17:00:00".into(),
                ..Default::default()
            },
        ]);
        app.bids.applications = Some(vec![Application {
            symbol: "OLDIPO".into(),
            status: "allotted".into(),
            created_at: "2026-08-25 11:00:00".into(),
            ..Default::default()
        }]);
        let rows = rendered(&app);
        assert!(rows.contains("IPO · 1 open"), "{rows}");
        assert!(rows.contains("ACME SME"), "{rows}");
        assert!(rows.contains("7–9 Sep"), "{rows}");
        assert!(rows.contains("118–124"), "{rows}");
        assert!(rows.contains("14,880"), "{rows}");
        assert!(rows.contains("ongoing"), "{rows}");
        assert!(rows.contains("applied"), "{rows}");
        assert!(rows.contains("25 Aug"), "{rows}");
        assert!(!rows.contains("Kite Web"), "{rows}");
    }
}
