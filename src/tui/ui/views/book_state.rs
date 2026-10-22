use crate::tui::ui;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Block;

pub(super) fn state_panel(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    tone: ui::MessageTone,
    headline: &str,
    detail: Option<&str>,
    hint: Option<&str>,
) {
    state_body(
        frame,
        area,
        ui::panel(title.to_string()),
        tone,
        headline,
        detail,
        hint,
    );
}

pub(super) fn state_body(
    frame: &mut Frame,
    area: Rect,
    block: Block<'static>,
    tone: ui::MessageTone,
    headline: &str,
    detail: Option<&str>,
    hint: Option<&str>,
) {
    let inner = block.inner(area);
    frame.render_widget(block, area);
    ui::state_message(frame, inner, tone, headline, detail, hint);
}

#[derive(Clone, Copy)]
pub(super) enum BookState<'a> {
    Unknown,
    Empty(&'a str),
    Failed(&'a str),
}

fn book_state_parts<'a>(
    title: &str,
    state: BookState<'a>,
) -> (
    ui::MessageTone,
    String,
    Option<&'a str>,
    Option<&'static str>,
) {
    match state {
        BookState::Unknown => (
            ui::MessageTone::Normal,
            format!("{title} not available yet"),
            None,
            Some("r Refresh"),
        ),
        BookState::Empty(headline) => (ui::MessageTone::Normal, headline.to_string(), None, None),
        BookState::Failed(error) => (
            ui::MessageTone::Error,
            format!("{title} unavailable"),
            Some(error),
            Some("r Retry"),
        ),
    }
}

pub(super) fn book_state(frame: &mut Frame, area: Rect, title: &str, state: BookState<'_>) {
    let (tone, headline, detail, hint) = book_state_parts(title, state);
    state_panel(
        frame,
        area,
        &format!(" {title} "),
        tone,
        &headline,
        detail,
        hint,
    );
}

pub(super) fn loaded<'a, T>(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    rows: Option<&'a [T]>,
    empty: &str,
) -> Option<&'a [T]> {
    match rows {
        Some(rows) if !rows.is_empty() => Some(rows),
        _ => {
            book_state(
                frame,
                area,
                title,
                if rows.is_some() {
                    BookState::Empty(empty)
                } else {
                    BookState::Unknown
                },
            );
            None
        }
    }
}

pub(super) fn focus_book_state(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    state: BookState<'_>,
    focused: bool,
) {
    let (tone, headline, detail, hint) = book_state_parts(title, state);
    state_body(
        frame,
        area,
        ui::focus_panel(title, focused),
        tone,
        &headline,
        detail,
        hint,
    );
}

#[cfg(test)]
mod view_state_tests {

    use super::super::draw_watch;
    use super::{BookState, book_state};
    use crate::tui::app::App;
    use crate::tui::format;
    use crate::tui::marketwatch::State;
    use crate::tui::notes::Pad;
    use crate::tui::settings::{self, Settings, Theme};
    use kiteconnect::KiteConnect;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::style::Modifier;

    fn test_app() -> App {
        let client = KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("tok").unwrap())
            .build()
            .expect("dummy client builds");
        let (events_tx, _events_rx) = tokio::sync::mpsc::channel(8);
        App::new(
            client,
            events_tx,
            State::test_loaded(Vec::new()),
            Pad::default(),
            true,
            &Settings::default(),
        )
    }

    fn row(buffer: &Buffer, y: u16) -> String {
        (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol())
            .collect()
    }

    #[test]
    fn an_empty_book_is_one_bold_headline_and_no_hint() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let mut terminal = Terminal::new(TestBackend::new(30, 6)).unwrap();
        terminal
            .draw(|frame| {
                book_state(
                    frame,
                    frame.area(),
                    "Orders",
                    BookState::Empty("No orders today"),
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();

        assert!(
            row(&buffer, 0).starts_with("┌ Orders "),
            "{:?}",
            row(&buffer, 0)
        );
        assert_eq!(
            row(&buffer, 1).trim_end(),
            format!("│{}│", " ".repeat(28)),
            "a panel with room to spare takes the blank first row"
        );
        assert!(
            row(&buffer, 2).starts_with("│ No orders today"),
            "{:?}",
            row(&buffer, 2)
        );
        assert!(buffer[(2, 2)].modifier.contains(Modifier::BOLD));
        assert_eq!(
            row(&buffer, 3).trim_end(),
            format!("│{}│", " ".repeat(28)),
            "no hint row"
        );
    }

    #[test]
    fn a_state_message_gives_up_its_blank_row_before_its_words() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let mut terminal = Terminal::new(TestBackend::new(40, 4)).unwrap();
        let app = test_app();
        terminal
            .draw(|frame| draw_watch(frame, frame.area(), &app))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        assert!(
            row(&buffer, 1).starts_with("│ Watchlist is empty"),
            "{:?}",
            row(&buffer, 1)
        );
        assert!(
            row(&buffer, 2).starts_with("│ / Search and add instruments"),
            "{:?}",
            row(&buffer, 2)
        );
        assert_eq!(buffer[(2, 2)].fg, format::text_muted(), "the hint is muted");
        assert!(!buffer[(2, 2)].modifier.contains(Modifier::BOLD));
    }
}

#[cfg(test)]
mod view_tone_tests {

    use super::super::{
        PlainList, draw_alerts, draw_auctions, draw_chart_panel, draw_gtts, draw_holdings, draw_mf,
        draw_orders, draw_positions,
    };
    use crate::tui::app::App;

    use crate::tui::format;
    use crate::tui::marketwatch::State;
    use crate::tui::notes::Pad;
    use crate::tui::settings::{self, Settings};
    use crate::tui::ui::pane::PaneHits;
    use crate::tui::workspace::PaneId;
    use kiteconnect::KiteConnect;
    use ratatui::Frame;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::style::Modifier;

    fn app() -> App {
        let client = KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("tok").unwrap())
            .build()
            .expect("dummy client builds");
        let (events_tx, _events_rx) = tokio::sync::mpsc::channel(8);
        App::new(
            client,
            events_tx,
            State::test_loaded(Vec::new()),
            Pad::default(),
            true,
            &Settings::default(),
        )
    }

    fn bids_ipo_app() -> App {
        use crate::tui::bids::{Instrument, Section};
        let mut app = app();
        app.set_pane(PaneId::InvestBids);
        app.bids_pane.set_section(Section::Ipo);
        app.bids.ipo = Some(vec![Instrument {
            symbol: "ACME".into(),
            status: "ongoing".into(),
            ..Default::default()
        }]);
        app.bids_pane.focus_applications();
        app
    }

    fn auctions(frame: &mut Frame, area: Rect, app: &App) {
        draw_auctions(
            frame,
            area,
            app,
            &PlainList {
                selected: app.pane_row(PaneId::InvestBids),
                focused: true,
                viewport: Some(app.bids_pane.viewport()),
            },
            &PaneHits::new(&app.hit, PaneId::PortfolioPositions),
        );
    }

    fn chart_panel(frame: &mut Frame, area: Rect, app: &App) {
        draw_chart_panel(
            frame,
            area,
            app,
            app.selected_chart_target().as_ref(),
            &app.chart_pane,
            &PaneHits::new(&app.hit, PaneId::PortfolioPositions),
        );
    }

    struct Panel {
        text: String,
        headline: ratatui::buffer::Cell,
    }

    impl Panel {
        fn says(&self, needle: &str) -> bool {
            self.text.contains(needle)
        }
    }

    fn panel(app: &App, draw: fn(&mut Frame, Rect, &App)) -> Panel {
        let mut terminal = ratatui::Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| draw(frame, frame.area(), app))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let text = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        Panel {
            text,
            headline: buffer[(2, 2)].clone(),
        }
    }

    fn tone_theme() -> settings::ThemeGuard {
        settings::theme_guard(settings::preset("kite").expect("the kite preset exists"))
    }

    fn normal_tone(panel: &Panel, at: &str) {
        assert_eq!(
            panel.headline.fg,
            format::header_style()
                .fg
                .expect("the kite preset names a text colour"),
            "{at}: not the ordinary tone\n{}",
            panel.text
        );
        assert_ne!(panel.headline.fg, format::error(), "{at}");
        assert!(panel.headline.modifier.contains(Modifier::BOLD), "{at}");
    }

    fn error_tone(panel: &Panel, at: &str) {
        assert_eq!(
            panel.headline.fg,
            format::error(),
            "{at}: not the failure tone\n{}",
            panel.text
        );
        assert!(panel.headline.modifier.contains(Modifier::BOLD), "{at}");
    }

    #[test]
    fn the_option_backed_books_separate_unknown_from_empty() {
        struct Case {
            view: PaneId,
            draw: fn(&mut Frame, Rect, &App),
            unknown: &'static str,
            empty: &'static str,
            fill: fn(&mut App),
        }
        let _guard = tone_theme();

        let cases = [
            Case {
                view: PaneId::PortfolioPositions,
                draw: draw_positions,
                unknown: "Positions not available yet",
                empty: "No positions today",
                fill: |app| {
                    app.positions = Some({
                        let mut row = crate::tui::data::test_models::positions();
                        row.net = Vec::new();
                        row
                    });
                },
            },
            Case {
                view: PaneId::OrdersBook,
                draw: draw_orders,
                unknown: "Orders not available yet",
                empty: "No orders today",
                fill: |app| app.orders = Some(Vec::new()),
            },
            Case {
                view: PaneId::PortfolioHoldings,
                draw: draw_holdings,
                unknown: "Holdings not available yet",
                empty: "No holdings",
                fill: |app| app.holdings = Some(Vec::new()),
            },
            Case {
                view: PaneId::OrdersGtt,
                draw: draw_gtts,
                unknown: "GTT not available yet",
                empty: "No GTT triggers",
                fill: |app| app.gtts = Some(Vec::new()),
            },
            Case {
                view: PaneId::PortfolioMf,
                draw: draw_mf,
                unknown: "MF not available yet",
                empty: "No mutual fund holdings",
                fill: |app| app.mf_holdings = Some(Vec::new()),
            },
        ];
        for case in cases {
            let mut app = app();
            app.set_pane(case.view);
            let unknown = panel(&app, case.draw);
            assert!(
                unknown.says(case.unknown),
                "{:?} unknown:\n{}",
                case.view,
                unknown.text
            );
            assert!(
                unknown.says("r Refresh"),
                "{:?}: `r` re-polls this view, so the unknown state names it:\n{}",
                case.view,
                unknown.text
            );
            normal_tone(&unknown, &format!("{:?} unknown", case.view));

            (case.fill)(&mut app);
            let empty = panel(&app, case.draw);
            assert!(
                empty.says(case.empty),
                "{:?} empty:\n{}",
                case.view,
                empty.text
            );
            assert!(
                !empty.says("r Refresh"),
                "{:?}: an arrived-and-empty book has nothing to retry:\n{}",
                case.view,
                empty.text
            );
            normal_tone(&empty, &format!("{:?} empty", case.view));
        }
    }

    #[test]
    fn the_empty_mf_book_still_names_coin() {
        let _guard = tone_theme();
        let mut app = app();
        app.set_pane(PaneId::PortfolioMf);
        app.mf_holdings = Some(Vec::new());
        let empty = panel(&app, draw_mf);
        assert!(
            empty.says("Invest in direct MFs at coin.zerodha.com"),
            "{}",
            empty.text
        );
    }

    #[test]
    fn the_chart_panel_states_are_ordinary_and_name_the_close_key() {
        use crate::tui::app::ChartTarget;
        let _guard = tone_theme();
        let mut app = app();
        app.set_pane(PaneId::PortfolioPositions);
        app.positions = Some({
            let mut row = crate::tui::data::test_models::positions();
            row.net = Vec::new();
            row
        });
        app.open_chart_panel_for_test(PaneId::PortfolioPositions);

        let none = panel(&app, chart_panel);
        assert!(none.says("Chart unavailable for this row"), "{}", none.text);
        assert!(none.says("c Close the chart"), "{}", none.text);
        normal_tone(&none, "chart with no target");

        app.chart_pane
            .resolve(Some(ChartTarget::untokened_for_test("NSE", "WEIRD")));
        let untokened = panel(&app, chart_panel);
        assert!(
            untokened.says("Historical chart unavailable"),
            "{}",
            untokened.text
        );
        assert!(
            untokened.says("This row carries no instrument token."),
            "{}",
            untokened.text
        );
        normal_tone(&untokened, "chart with an untokened target");
    }

    #[test]
    fn the_auctions_book_has_all_three_states() {
        let _guard = tone_theme();
        let mut app = app();
        app.set_pane(PaneId::InvestBids);

        let unknown = panel(&app, auctions);
        assert!(
            unknown.says("Auction not available yet"),
            "{}",
            unknown.text
        );
        normal_tone(&unknown, "auctions unknown");

        app.auctions = Some(Vec::new());
        let empty = panel(&app, auctions);
        assert!(empty.says("No exchange auction holdings"), "{}", empty.text);
        normal_tone(&empty, "auctions empty");

        app.bids.auctions_error = Some("503 from the exchange".to_string());
        let failed = panel(&app, auctions);
        assert!(
            failed.says("Auction holdings unavailable"),
            "{}",
            failed.text
        );
        assert!(
            failed.says("503 from the exchange"),
            "the server's own words are the detail:\n{}",
            failed.text
        );
        assert!(failed.says("r Retry"), "{}", failed.text);
        error_tone(&failed, "auctions failed");
    }

    #[test]
    fn the_alerts_book_names_a_rather_than_r() {
        let _guard = tone_theme();
        let mut app = app();
        app.set_pane(PaneId::OrdersAlerts);

        let unknown = panel(&app, draw_alerts);
        assert!(unknown.says("Alerts not available yet"), "{}", unknown.text);
        assert!(!unknown.says("r Ref"), "{}", unknown.text);
        assert!(!unknown.says("r Retry"), "{}", unknown.text);
        normal_tone(&unknown, "alerts unknown");

        app.set_alerts(Vec::new());
        let empty = panel(&app, draw_alerts);
        assert!(empty.says("No alerts"), "{}", empty.text);
        assert!(empty.says("a Set an alert"), "{}", empty.text);
        normal_tone(&empty, "alerts empty");
    }

    #[test]
    fn the_bids_sections_have_all_three_states() {
        use crate::tui::bids::Section;
        let _guard = tone_theme();
        let mut app = app();
        app.set_pane(PaneId::InvestBids);
        app.bids_pane.set_section(Section::Government);

        let unknown = panel(&app, super::super::draw_bids);
        assert!(
            unknown.says("Govt. securities not available yet"),
            "{}",
            unknown.text
        );

        app.bids.government = Some(Vec::new());
        let empty = panel(&app, super::super::draw_bids);
        assert!(
            empty.says("No open bids or eligible instruments"),
            "{}",
            empty.text
        );

        app.bids.government_error = Some("gateway timeout".to_string());
        let failed = panel(&app, super::super::draw_bids);
        assert!(
            failed.says("Govt. securities unavailable"),
            "{}",
            failed.text
        );
        assert!(failed.says("gateway timeout"), "{}", failed.text);
        assert!(failed.says("r Retry"), "{}", failed.text);

        let mut ipo = bids_ipo_app();
        assert!(
            panel(&ipo, super::super::draw_bids).says("IPO applications not available yet"),
            "{}",
            panel(&ipo, super::super::draw_bids).text
        );
        ipo.bids.applications = Some(Vec::new());
        assert!(
            panel(&ipo, super::super::draw_bids).says("No IPO applications"),
            "{}",
            panel(&ipo, super::super::draw_bids).text
        );
        ipo.bids.applications_error = Some("no session".to_string());
        let failed = panel(&ipo, super::super::draw_bids);
        assert!(
            failed.says("IPO applications unavailable"),
            "{}",
            failed.text
        );
        assert!(failed.says("no session"), "{}", failed.text);
    }
}
