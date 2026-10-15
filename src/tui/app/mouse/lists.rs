use super::super::{App, Modal, ui};
use crate::tui::form::FormViewport;
use crate::tui::workspace::PaneId;
use crossterm::event::KeyCode;

impl App {
    pub(in crate::tui::app) fn scroll_pane(&mut self, pane: PaneId, delta: isize) {
        match pane {
            PaneId::PortfolioFunds => {
                self.focus_pane(pane);
                self.funds_scroll
                    .set(self.funds_scroll.get().saturating_add_signed(delta));
            }
            PaneId::TradingChart | PaneId::TradingDepth | PaneId::DerivativesChart => {}
            _ => {
                self.focus_pane(pane);
                self.move_selection_in(pane, delta);
            }
        }
    }

    pub(in crate::tui::app) fn select_in_list(&mut self, list: ui::HitList, index: usize) {
        match list {
            ui::HitList::Pane(pane) => {
                self.focus_pane(pane);
                self.set_selection_in(pane, index);
            }
            ui::HitList::Chain { owner, band } => {
                self.focus_pane(owner);
                match band {
                    ui::ChainBand::Call => self.set_chain_call_focus(owner, true),
                    ui::ChainBand::Put => self.set_chain_call_focus(owner, false),
                    ui::ChainBand::Strike => {}
                }
                self.set_selection_in(owner, index);
            }
            ui::HitList::BidsApplications(owner) => {
                self.focus_pane(owner);
                self.bids_pane.focus_applications();
                self.set_selection_in(owner, index);
            }
            ui::HitList::Search => {
                if let Some(Modal::Search(state)) = self.modals.last_mut() {
                    state.selected = index.min(state.results.len().saturating_sub(1));
                }
            }

            ui::HitList::Palette => {
                if let Some(Modal::Palette(state)) = self.modals.last_mut()
                    && index < state.results.len()
                {
                    state.selected = Some(index);
                    state.chosen = true;

                    Self::sync_preview(state);
                }
            }
            ui::HitList::ThemePicker => {
                if let Some(Modal::ThemePicker(state)) = self.modals.last_mut() {
                    state.selected = index.min(state.results.len().saturating_sub(1));

                    Self::sync_theme_preview(state);
                }
            }

            ui::HitList::WorkspacePicker => {
                if let Some(Modal::WorkspacePicker(state)) = self.modals.last_mut() {
                    state.selected = index.min(crate::tui::workspace::Preset::COUNT - 1);
                }
            }
            ui::HitList::EventLog => {
                let max_row = self.toasts.len().saturating_sub(1);
                if let Some(Modal::EventLog(state)) = self.modals.last_mut() {
                    let index = index.min(max_row);
                    if state.selected != index {
                        state.selected = index;

                        state.detail = FormViewport::default();
                    }
                }
            }
            ui::HitList::Notes => {
                let max_row = self.notes.ids_newest_first().len().saturating_sub(1);
                if let Some(Modal::Notes(state)) = self.modals.last_mut() {
                    state.selected = index.min(max_row);
                }
            }
        }
    }

    pub(in crate::tui::app) fn scroll_list(&mut self, list: ui::HitList, delta: isize) {
        match list {
            ui::HitList::Pane(pane) | ui::HitList::Chain { owner: pane, .. } => {
                self.focus_pane(pane);
                self.move_selection_in(pane, delta);
            }
            ui::HitList::BidsApplications(owner) => {
                self.focus_pane(owner);
                self.bids_pane.focus_applications();
                self.move_selection_in(owner, delta);
            }

            ui::HitList::Search
            | ui::HitList::Palette
            | ui::HitList::ThemePicker
            | ui::HitList::WorkspacePicker
            | ui::HitList::EventLog
            | ui::HitList::Notes => self.press(if delta < 0 {
                KeyCode::Up
            } else {
                KeyCode::Down
            }),
        }
    }
}

#[cfg(test)]
pub(in crate::tui::app) mod tests {
    use super::super::super::test_support::{
        H, HEADER_ROW, W, char_key, click, holding, holdings_app, ltp_tick, master_inst, position,
        press_at, region_of, render, row_offset, sample_order, scroll, test_app, text_at,
    };
    use super::super::super::{
        App, HoldingsSort, KeyCode, KeyEvent, KeyModifiers, Modal, MouseButton, MouseEvent,
        MouseEventKind, OrdersSort, Side, SortTarget, bids, ui,
    };

    use crate::tui::instruments::InstrumentStore;
    use crate::tui::workspace::{PaneId, Preset};
    use std::sync::Arc;

    #[test]
    fn overview_mouse_click_focuses_pane_and_selects_scrolled_row() {
        let mut app = test_app();
        app.positions = Some({
            let mut row = crate::tui::data::test_models::positions();
            row.net = (0..5)
                .map(|i| position(&format!("P{i}"), i, 10, 0.0, 100.0, 100.0))
                .collect();
            row
        });

        app.holdings = Some(
            (0u32..6)
                .map(|i| {
                    holding(
                        &format!("H{i}"),
                        100 + i,
                        100.0,
                        100.0 * f64::from(i + 1),
                        0.0,
                        1,
                    )
                })
                .collect(),
        );

        let rect = ratatui::layout::Rect {
            x: 0,
            y: 5,
            width: 20,
            height: 3,
        };

        let surface = ui::Surface::of(&app);
        let mut hit = app.hit.borrow_mut();
        hit.surface = Some(surface);
        hit.push(
            rect,
            ui::HitAction::Rows {
                list: ui::HitList::Pane(PaneId::OverviewHoldings),
                offset: 3,
            },
        );
        hit.record_row_identities(&app);
        drop(hit);

        app.on_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 2,
            row: 6,
            modifiers: KeyModifiers::NONE,
        });

        assert_eq!(app.focused_pane(), PaneId::OverviewHoldings);
        assert_eq!(app.pane_row(app.focused_pane()), 4);
    }

    #[test]
    fn view_table_row_click_selects_it() {
        let mut app = test_app();
        app.set_pane(PaneId::PortfolioHoldings);
        app.holdings = Some(
            (0u32..5)
                .map(|i| holding(&format!("H{i}"), 100 + i, 100.0, 110.0, 100.0, 1))
                .collect(),
        );

        click(&mut app, W, H, 4, HEADER_ROW + 3);
        assert_eq!(app.pane_row(PaneId::PortfolioHoldings), 2);
    }

    #[test]
    fn a_click_below_the_last_table_record_selects_nothing() {
        let mut app = test_app();
        app.set_workspace(Preset::Portfolio);
        app.holdings = Some(
            (0u32..2)
                .map(|i| holding(&format!("H{i}"), 100 + i, 100.0, 110.0, 100.0, 1))
                .collect(),
        );
        let pane = PaneId::PortfolioHoldings;
        render(&app, W, H);
        let rows = region_of(&app, |action| {
            action
                == ui::HitAction::Rows {
                    list: ui::HitList::Pane(pane),
                    offset: 0,
                }
        });
        app.focus_pane_for_test(app.workspace().default_focus());
        click(&mut app, W, H, rows.rect.x + 4, rows.rect.y + 1 + 4);
        assert_eq!(app.pane_row(pane), 0, "blank space is not a record");
        assert_eq!(app.focused_pane(), pane, "but it focuses the pane");
    }

    #[test]
    fn a_hidden_tables_headers_cannot_be_clicked_through_a_maximized_chart() {
        let mut app = holdings_app();
        render(&app, W, H);

        let header = app
            .hit
            .borrow()
            .regions
            .iter()
            .rev()
            .find(|region| matches!(region.action, ui::HitAction::Header { .. }))
            .copied()
            .expect("the table records its headers");
        let unsorted = app.books.portfolio_holdings.sort();
        click(&mut app, W, H, header.rect.x, header.rect.y);
        let sort = app.books.portfolio_holdings.sort();
        assert_ne!(sort, unsorted, "the point is a live header while drawn");

        app.open_chart_panel_for_test(PaneId::PortfolioHoldings);
        app.chart_maximized = true;
        click(&mut app, W, H, header.rect.x, header.rect.y);
        assert_eq!(
            app.books.portfolio_holdings.sort(),
            sort,
            "nothing re-sorted"
        );
        assert!(
            !app.hit
                .borrow()
                .regions
                .iter()
                .any(|region| matches!(region.action, ui::HitAction::Header { .. })),
            "an undrawn header records no target"
        );
    }

    #[test]
    fn scroll_over_a_view_table_moves_its_selection() {
        let mut app = test_app();
        app.set_pane(PaneId::PortfolioHoldings);
        app.holdings = Some(
            (0u32..5)
                .map(|i| holding(&format!("H{i}"), 100 + i, 100.0, 110.0, 100.0, 1))
                .collect(),
        );
        scroll(&mut app, W, H, 4, 4, true);
        assert_eq!(app.pane_row(PaneId::PortfolioHoldings), 1);
        scroll(&mut app, W, H, 4, 4, false);
        assert_eq!(app.pane_row(PaneId::PortfolioHoldings), 0);
    }

    #[test]
    fn overview_scroll_over_the_holdings_panel_focuses_it() {
        let mut app = test_app();
        app.set_workspace(Preset::Overview);
        app.positions = Some({
            let mut row = crate::tui::data::test_models::positions();
            row.net = (0..2)
                .map(|i| position(&format!("P{i}"), i, 10, 0.0, 100.0, 100.0))
                .collect();
            row
        });
        app.holdings = Some(
            (0u32..6)
                .map(|i| {
                    holding(
                        &format!("H{i}"),
                        100 + i,
                        100.0,
                        100.0 * f64::from(i + 1),
                        0.0,
                        1,
                    )
                })
                .collect(),
        );
        render(&app, W, H);
        let panel = region_of(&app, |action| {
            matches!(
                action,
                ui::HitAction::Rows {
                    list: ui::HitList::Pane(PaneId::OverviewHoldings),
                    ..
                }
            )
        });

        assert_eq!(app.focused_pane(), PaneId::OverviewPositions);
        scroll(&mut app, W, H, panel.rect.x + 4, panel.rect.y, true);
        assert_eq!(app.focused_pane(), PaneId::OverviewHoldings);
        assert_eq!(app.pane_row(app.focused_pane()), 1);
    }

    #[test]
    fn bids_section_label_click_switches_to_it() {
        let mut app = test_app();
        app.set_pane(PaneId::InvestBids);
        app.bids_pane.focus_applications();
        assert_eq!(app.bids_pane.section(), bids::Section::Ipo);
        let x = text_at(&app, W, H, HEADER_ROW, "[Auctions]");
        click(&mut app, W, H, x, HEADER_ROW);
        assert_eq!(app.bids_pane.section(), bids::Section::Auctions);
        assert!(
            !app.bids_pane.application_focus(),
            "switching section drops the applications focus, as `[`/`]` do"
        );
    }

    #[test]
    fn bids_applications_row_click_focuses_that_pane() {
        let mut app = test_app();
        app.set_pane(PaneId::InvestBids);
        app.bids.ipo = Some(vec![
            bids::Instrument::default(),
            bids::Instrument::default(),
        ]);
        app.bids.applications = Some(vec![
            bids::Application::default(),
            bids::Application::default(),
            bids::Application::default(),
        ]);
        render(&app, W, H);
        let rows = region_of(&app, |action| {
            matches!(
                action,
                ui::HitAction::Rows {
                    list: ui::HitList::BidsApplications(_),
                    ..
                }
            )
        });

        assert!(!app.bids_pane.application_focus());
        click(&mut app, W, H, rows.rect.x + 2, rows.rect.y + 1);
        assert!(app.bids_pane.application_focus());
        assert_eq!(app.pane_row(PaneId::InvestBids), 1);
    }

    #[test]
    fn a_wheel_over_the_issues_table_hidden_by_a_click_scrolls_nothing() {
        let mut app = test_app();
        app.set_pane(PaneId::InvestBids);
        app.bids.ipo = Some(vec![
            bids::Instrument::default(),
            bids::Instrument::default(),
        ]);
        app.bids.applications = Some(vec![
            bids::Application::default(),
            bids::Application::default(),
            bids::Application::default(),
        ]);
        render(&app, W, H);
        let row_target = |app: &App, applications: bool| {
            region_of(app, |action| match action {
                ui::HitAction::Rows { list, .. } => {
                    matches!(list, ui::HitList::BidsApplications(_)) == applications
                }
                _ => false,
            })
        };
        let issues = row_target(&app, false);
        let applications = row_target(&app, true);

        press_at(&mut app, applications.rect.x + 2, applications.rect.y + 1);
        assert!(app.bids_pane.application_focus());
        assert_eq!(app.pane_row(PaneId::InvestBids), 1);
        app.on_mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: issues.rect.x + 2,
            row: issues.rect.y,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(
            app.pane_row(PaneId::InvestBids),
            1,
            "the notch was aimed at a table that is no longer drawn"
        );
    }

    pub(in crate::tui::app) fn overview_app() -> App {
        let mut app = test_app();
        app.set_workspace(Preset::Overview);
        app.positions = Some({
            let mut row = crate::tui::data::test_models::positions();
            row.net = (0..2)
                .map(|i| position(&format!("P{i}"), i, 10, 0.0, 100.0, 100.0))
                .collect();
            row
        });
        app.holdings = Some(
            (0u32..3)
                .map(|i| holding(&format!("H{i}"), 100 + i, 100.0, 110.0, 100.0, 1))
                .collect(),
        );
        app
    }

    fn book_region(app: &App, pane: PaneId) -> ui::HitRegion {
        region_of(app, |action| action == ui::HitAction::Pane(pane))
    }

    fn book_rows(app: &App, pane: PaneId) -> Option<ui::HitRegion> {
        let hit = app.hit.borrow();
        hit.regions
            .iter()
            .find(|region| {
                matches!(
                    region.action,
                    ui::HitAction::Rows {
                        list: ui::HitList::Pane(p),
                        ..
                    } if p == pane
                )
            })
            .copied()
    }

    #[test]
    fn overview_click_on_an_empty_books_title_focuses_it() {
        let mut app = overview_app();
        app.positions = Some({
            let mut row = crate::tui::data::test_models::positions();
            row.net = Vec::new();
            row
        });
        app.holdings = Some(Vec::new());
        render(&app, W, H);

        assert!(
            book_rows(&app, PaneId::OverviewHoldings).is_none(),
            "an empty book draws no records, so it records no row target"
        );
        let book = book_region(&app, PaneId::OverviewHoldings);
        assert_eq!(app.focused_pane(), PaneId::OverviewPositions);

        click(&mut app, W, H, book.rect.x + 4, book.rect.y);
        assert_eq!(app.focused_pane(), PaneId::OverviewHoldings);
        assert_eq!(
            app.workspace(),
            Preset::Overview,
            "a book click opens nothing"
        );
        assert!(app.modals.is_empty());
    }

    #[test]
    fn overview_click_on_a_loading_books_message_focuses_it() {
        let mut app = test_app();
        app.set_workspace(Preset::Overview);
        assert!(app.holdings.is_none(), "nothing has loaded yet");
        render(&app, W, H);

        let book = book_region(&app, PaneId::OverviewHoldings);

        click(
            &mut app,
            W,
            H,
            book.rect.x + book.rect.width / 2,
            book.rect.y + 1,
        );
        assert_eq!(app.focused_pane(), PaneId::OverviewHoldings);
        assert_eq!(app.pane_row(app.focused_pane()), 0);
    }

    #[test]
    fn overview_click_below_the_last_record_keeps_the_selection() {
        let mut app = overview_app();
        app.workspaces
            .focus(Preset::Overview, PaneId::OverviewHoldings);
        app.set_selection(2);
        render(&app, W, H);

        let book = book_region(&app, PaneId::OverviewHoldings);
        let rows = book_rows(&app, PaneId::OverviewHoldings).expect("records drew");
        let blank = rows.rect.y + rows.rect.height;
        assert!(
            blank < book.rect.y + book.rect.height - 1,
            "the blank row under the last record is inside the book body"
        );

        click(&mut app, W, H, rows.rect.x + 4, blank);
        assert_eq!(app.focused_pane(), PaneId::OverviewHoldings);
        assert_eq!(
            app.pane_row(app.focused_pane()),
            2,
            "clicking past the last record must not move the selection"
        );
    }

    #[test]
    fn overview_click_on_the_positions_total_row_keeps_the_selection() {
        let mut app = overview_app();
        app.set_selection(1);
        render(&app, W, H);

        let book = book_region(&app, PaneId::OverviewPositions);

        click(
            &mut app,
            W,
            H,
            book.rect.x + 4,
            book.rect.y + book.rect.height - 2,
        );
        assert_eq!(app.focused_pane(), PaneId::OverviewPositions);
        assert_eq!(app.pane_row(app.focused_pane()), 1);
    }

    #[test]
    fn overview_click_on_the_other_books_blank_body_focuses_only() {
        let mut app = overview_app();
        app.set_selection(1);
        render(&app, W, H);

        let rows = book_rows(&app, PaneId::OverviewHoldings).expect("records drew");
        let blank = rows.rect.y + rows.rect.height;
        click(&mut app, W, H, rows.rect.x + 4, blank);
        assert_eq!(app.focused_pane(), PaneId::OverviewHoldings);
        assert_eq!(
            app.pane_row(app.focused_pane()),
            0,
            "the Holdings instance has its own untouched cursor, still on its first row"
        );

        app.workspaces
            .focus(Preset::Overview, PaneId::OverviewPositions);
        assert_eq!(app.pane_row(app.focused_pane()), 1);
    }

    #[test]
    fn overview_click_on_a_record_still_selects_it() {
        let mut app = overview_app();
        render(&app, W, H);

        let rows = book_rows(&app, PaneId::OverviewHoldings).expect("records drew");
        click(&mut app, W, H, rows.rect.x + 4, rows.rect.y + 2);
        assert_eq!(app.focused_pane(), PaneId::OverviewHoldings);
        assert_eq!(app.pane_row(app.focused_pane()), 2);
    }

    #[test]
    fn overview_clicks_land_on_the_right_record_in_the_compact_layout() {
        const CW: u16 = 80;
        const CH: u16 = 24;
        for (pane, index) in [
            (PaneId::OverviewPositions, 1usize),
            (PaneId::OverviewHoldings, 2),
        ] {
            let mut app = overview_app();
            render(&app, CW, CH);
            let rows = book_rows(&app, pane).expect("records drew in the compact layout");
            let offset = row_offset(rows);
            assert!(
                rows.rect.height as usize >= index + 1 - offset,
                "{pane:?} drew too few rows to click index {index}"
            );

            click(
                &mut app,
                CW,
                CH,
                rows.rect.x + 4,
                rows.rect.y + u16::try_from(index - offset).expect("visible row fits u16"),
            );
            assert_eq!(app.focused_pane(), pane);
            assert_eq!(app.pane_row(app.focused_pane()), index);
        }
    }

    #[test]
    fn overview_book_regions_stay_inside_the_compact_main_band() {
        const CW: u16 = 80;
        const CH: u16 = 24;
        let app = overview_app();
        render(&app, CW, CH);
        let positions = book_region(&app, PaneId::OverviewPositions);
        let holdings = book_region(&app, PaneId::OverviewHoldings);

        for book in [positions, holdings] {
            assert!(book.rect.y >= 4, "{book:?} overlaps the KPI band");
            assert!(
                book.rect.y + book.rect.height <= 22,
                "{book:?} overlaps the account row"
            );
        }

        assert_eq!(positions.rect.y + positions.rect.height, holdings.rect.y);
    }

    #[test]
    fn overview_fallback_records_no_hit_for_the_hidden_book() {
        const FW: u16 = 80;
        const FH: u16 = 17;
        let mut app = overview_app();
        render(&app, FW, FH);

        let positions = book_region(&app, PaneId::OverviewPositions);
        assert!(
            !app.hit
                .borrow()
                .regions
                .iter()
                .any(|region| region.action == ui::HitAction::Pane(PaneId::OverviewHoldings)),
            "the hidden book must record no rectangle"
        );
        assert!(
            book_rows(&app, PaneId::OverviewHoldings).is_none(),
            "the hidden book draws no records either"
        );

        assert_eq!(positions.rect.width, FW);

        click(&mut app, FW, FH, positions.rect.x + 4, positions.rect.y);
        assert_eq!(app.focused_pane(), PaneId::OverviewPositions);

        app.on_key(char_key(']'));
        render(&app, FW, FH);
        assert_eq!(app.focused_pane(), PaneId::OverviewHoldings);
        let holdings = book_region(&app, PaneId::OverviewHoldings);
        assert_eq!(holdings.rect, positions.rect, "the band did not move");
        assert!(
            !app.hit
                .borrow()
                .regions
                .iter()
                .any(|region| region.action == ui::HitAction::Pane(PaneId::OverviewPositions)),
            "the book that is now hidden records nothing"
        );

        let rows = book_rows(&app, PaneId::OverviewHoldings).expect("records drew");
        click(&mut app, FW, FH, rows.rect.x + 4, rows.rect.y + 1);
        assert_eq!(app.pane_row(app.focused_pane()), 1 + row_offset(rows));
    }

    #[test]
    fn overview_indices_and_movers_take_focus_but_no_selection() {
        const W: u16 = 120;
        const H: u16 = 35;
        let mut app = overview_app();
        app.set_selection(1);
        render(&app, W, H);

        for pane in [PaneId::OverviewIndices, PaneId::OverviewMovers] {
            let region = book_region(&app, pane);
            click(&mut app, W, H, region.rect.x + 2, region.rect.y + 1);
            assert_eq!(app.focused_pane(), pane);
            assert_eq!(app.pane_row(PaneId::OverviewPositions), 1);
        }
    }

    #[test]
    fn a_book_click_does_not_reach_through_an_open_modal() {
        let mut app = overview_app();
        app.set_selection(1);
        render(&app, W, H);
        let book = book_region(&app, PaneId::OverviewHoldings);

        app.open_palette();
        render(&app, W, H);
        assert!(
            book_rows(&app, PaneId::OverviewPositions).is_none()
                && !app
                    .hit
                    .borrow()
                    .regions
                    .iter()
                    .any(|region| matches!(region.action, ui::HitAction::Pane(_))),
            "a modal owns the screen, so the books record nothing"
        );

        click(&mut app, W, H, book.rect.x + 4, book.rect.y);
        assert_eq!(app.focused_pane(), PaneId::OverviewPositions);
        assert_eq!(app.pane_row(app.focused_pane()), 1);
        assert!(matches!(app.modals.last(), Some(Modal::Palette(_))));
    }

    #[test]
    fn a_row_target_from_the_view_before_a_switch_selects_nothing() {
        let mut app = overview_app();
        app.set_pane(PaneId::PortfolioHoldings);
        render(&app, W, H);
        let rows = region_of(&app, |action| {
            matches!(
                action,
                ui::HitAction::Rows {
                    list: ui::HitList::Pane(_),
                    ..
                }
            )
        });

        app.show_pane(PaneId::PortfolioPositions);
        press_at(&mut app, rows.rect.x + 2, rows.rect.y + 1);
        assert_eq!(
            app.pane_row(PaneId::PortfolioPositions),
            0,
            "the click was aimed at a table that is no longer drawn"
        );
    }

    #[tokio::test]
    async fn a_refresh_that_replaces_a_drawn_record_retires_its_row_target() {
        use crate::tui::data::DataEvent;
        use crate::tui::event::AppEvent;
        let book = |first: &str| {
            vec![
                holding(first, 1, 100.0, 110.0, 100.0, 1),
                holding("ZZZ", 2, 100.0, 110.0, 100.0, 1),
            ]
        };
        let mut app = test_app();
        app.set_pane(PaneId::PortfolioHoldings);
        app.holdings = Some(book("AAA"));
        app.set_selection(1);
        render(&app, W, H);
        let rows = region_of(&app, |action| {
            action
                == ui::HitAction::Rows {
                    list: ui::HitList::Pane(PaneId::PortfolioHoldings),
                    offset: 0,
                }
        });

        app.on_event(AppEvent::Data(DataEvent::Holdings(Ok(book("BBB")))));
        let selected = app.pane_row(PaneId::PortfolioHoldings);
        press_at(&mut app, rows.rect.x + 4, rows.rect.y);
        assert_eq!(
            app.pane_row(PaneId::PortfolioHoldings),
            selected,
            "the click was aimed at AAA, which is gone"
        );

        render(&app, W, H);
        let mut repriced = book("BBB");
        repriced[0].last_price = 120.0;
        app.on_event(AppEvent::Data(DataEvent::Holdings(Ok(repriced))));
        press_at(&mut app, rows.rect.x + 4, rows.rect.y);
        assert_eq!(app.pane_row(PaneId::PortfolioHoldings), 0);
    }

    #[test]
    fn a_rerank_in_a_hidden_list_keeps_the_orders_row_target_live() {
        use kiteconnect::ticker::TickerEvent;
        let mut app = test_app();
        app.set_pane(PaneId::OrdersBook);
        let mut second = sample_order("OPEN");
        second.order_id = "OID2".into();
        app.orders = Some(vec![sample_order("OPEN"), second]);
        app.holdings = Some(vec![
            holding("FIRST", 100, 100.0, 120.0, 100.0, 1),
            holding("SECOND", 101, 100.0, 110.0, 100.0, 1),
        ]);
        let rank_of = |app: &App, id: &str| {
            let orders = app.orders.as_deref().unwrap_or_default();
            app.orders_order_in(PaneId::OrdersBook)
                .iter()
                .position(|&i| orders[i].order_id == id)
                .expect("the order is drawn")
        };
        app.set_selection(rank_of(&app, "OID1"));
        render(&app, W, H);
        let rows = region_of(&app, |action| {
            action
                == ui::HitAction::Rows {
                    list: ui::HitList::Pane(PaneId::OrdersBook),
                    offset: 0,
                }
        });

        let before = app.holdings_order_in(PaneId::PortfolioHoldings);
        app.on_ws(TickerEvent::Ticks(vec![ltp_tick(101, 150.0)]));
        assert_ne!(
            app.holdings_order_in(PaneId::PortfolioHoldings),
            before,
            "the hidden book re-ranked"
        );
        let row = u16::try_from(rank_of(&app, "OID2")).expect("a small row");
        press_at(&mut app, rows.rect.x + 4, rows.rect.y + row);
        let selected = app
            .selected_order_in(PaneId::OrdersBook)
            .expect("an order is selected");
        assert_eq!(selected.order_id, "OID2");
    }

    #[test]
    fn palette_row_click_selects_that_command() {
        let mut app = test_app();
        app.on_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL));
        render(&app, W, H);
        let rows: Vec<(usize, ratatui::layout::Rect)> = {
            let hit = app.hit.borrow();
            hit.regions
                .iter()
                .filter_map(|region| match region.action {
                    ui::HitAction::Row {
                        list: ui::HitList::Palette,
                        index,
                    } => Some((index, region.rect)),
                    _ => None,
                })
                .collect()
        };
        assert!(rows.len() > 2, "the menu lists several commands");
        let (index, rect) = rows[2];
        click(&mut app, W, H, rect.x + 2, rect.y);
        let Some(Modal::Palette(state)) = app.modals.last() else {
            panic!("the palette is still open");
        };
        assert_eq!(state.selected, Some(index));
        assert!(
            state.selected_item().is_some(),
            "a recorded palette row is always an item, never a section header"
        );
    }

    #[test]
    fn a_retyped_palette_retires_its_row_targets() {
        let mut app = test_app();
        app.on_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL));
        render(&app, W, H);
        let (index, rect) = {
            let hit = app.hit.borrow();
            hit.regions
                .iter()
                .filter_map(|region| match region.action {
                    ui::HitAction::Row {
                        list: ui::HitList::Palette,
                        index,
                    } => Some((index, region.rect)),
                    _ => None,
                })
                .nth(2)
                .expect("the menu lists several commands")
        };

        app.on_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
        let selected = |app: &App| match app.modals.last() {
            Some(Modal::Palette(state)) => (state.results.len(), state.selected),
            _ => panic!("the palette is still open"),
        };
        let (count, before) = selected(&app);
        assert!(count > index, "the new results still reach the old row");
        assert_ne!(before, Some(index));
        press_at(&mut app, rect.x + 2, rect.y);
        assert_eq!(
            selected(&app).1,
            before,
            "the click was aimed at a row of the unfiltered menu"
        );
    }

    #[test]
    fn palette_scroll_moves_its_selection() {
        let mut app = test_app();
        app.on_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL));
        let selected = |app: &App| match app.modals.last() {
            Some(Modal::Palette(state)) => state.selected,
            _ => panic!("the palette is open"),
        };
        let before = selected(&app);
        render(&app, W, H);
        let row = region_of(&app, |action| {
            matches!(
                action,
                ui::HitAction::Row {
                    list: ui::HitList::Palette,
                    ..
                }
            )
        });
        scroll(&mut app, W, H, row.rect.x + 2, row.rect.y, true);
        assert!(
            selected(&app) > before,
            "a notch down moves to the next command"
        );
    }

    #[test]
    fn palette_clicks_only_ever_select() {
        let mut app = test_app();
        app.instruments = Some(Arc::new(InstrumentStore::new(vec![
            master_inst("NSE", "NSE", "INFY", 408_065),
            master_inst("BSE", "BSE", "INFY", 136_248),
        ])));
        app.on_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL));
        for c in "buy inf".chars() {
            app.on_key(char_key(c));
        }
        let before_view = app.workspace();
        let rows: Vec<(usize, ratatui::layout::Rect)> = {
            render(&app, W, H);
            let hit = app.hit.borrow();
            hit.regions
                .iter()
                .filter_map(|region| match region.action {
                    ui::HitAction::Row {
                        list: ui::HitList::Palette,
                        index,
                    } => Some((index, region.rect)),
                    _ => None,
                })
                .collect()
        };
        assert!(rows.len() > 1, "INFY resolves on two exchanges");
        let (index, rect) = rows[1];

        click(&mut app, W, H, rect.x, rect.y);
        let Some(Modal::Palette(state)) = app.modals.last() else {
            panic!("the palette is still open");
        };
        assert_eq!(state.selected, Some(index));
        assert!(state.chosen, "a click states which row the user means");
        assert_eq!(state.input, "buy inf", "a click never edits the input");

        click(&mut app, W, H, rect.x + 2, rect.y);
        assert!(matches!(app.modals.last(), Some(Modal::Palette(_))));
        assert_eq!(app.modals.len(), 1, "no ticket opened");
        assert_eq!(app.workspace(), before_view);
    }

    #[test]
    fn palette_clicks_off_a_row_change_nothing() {
        let mut app = test_app();
        app.on_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL));
        render(&app, W, H);
        let selected = match app.modals.last() {
            Some(Modal::Palette(state)) => state.selected,
            _ => panic!("the palette is open"),
        };
        let before_view = app.workspace();
        let before_selection = app.pane_row(app.focused_pane());

        let rows: Vec<ratatui::layout::Rect> = {
            let hit = app.hit.borrow();
            hit.regions
                .iter()
                .filter_map(|region| match region.action {
                    ui::HitAction::Row {
                        list: ui::HitList::Palette,
                        ..
                    } => Some(region.rect),
                    _ => None,
                })
                .collect()
        };
        let first = rows[0];

        for (column, row) in [(first.x, first.y - 1), (0, 0), (W - 1, H - 1)] {
            click(&mut app, W, H, column, row);
            let Some(Modal::Palette(state)) = app.modals.last() else {
                panic!("a stray click at ({column},{row}) must not dismiss the palette");
            };
            assert_eq!(
                state.selected, selected,
                "({column},{row}) moved the selection"
            );
            assert!(!state.chosen, "({column},{row}) is not a choice");
        }
        assert_eq!(app.workspace(), before_view, "no click reached the tab bar");
        assert_eq!(app.pane_row(app.focused_pane()), before_selection);
    }

    #[test]
    fn palette_wheel_moves_inside_the_results_only() {
        let mut app = test_app();
        app.on_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL));
        let selected = |app: &App| match app.modals.last() {
            Some(Modal::Palette(state)) => state.selected,
            _ => panic!("the palette is open"),
        };
        let before = selected(&app);
        let before_selection = app.pane_row(app.focused_pane());
        render(&app, W, H);
        let row = region_of(&app, |action| {
            matches!(
                action,
                ui::HitAction::Row {
                    list: ui::HitList::Palette,
                    ..
                }
            )
        });
        scroll(&mut app, W, H, row.rect.x + 2, row.rect.y, true);
        assert!(selected(&app) > before, "a notch down moves one item");
        let after = selected(&app);
        match app.modals.last() {
            Some(Modal::Palette(state)) => assert!(state.chosen),
            _ => panic!("the palette is open"),
        }

        scroll(&mut app, W, H, 0, 0, true);
        assert_eq!(
            selected(&app),
            after,
            "a notch outside the results is inert"
        );
        assert_eq!(
            app.pane_row(app.focused_pane()),
            before_selection,
            "the background view must not scroll under a modal"
        );
    }

    #[test]
    fn a_palette_under_a_ticket_gets_no_mouse_input() {
        let mut app = test_app();
        app.on_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL));
        render(&app, W, H);
        let row = region_of(&app, |action| {
            matches!(
                action,
                ui::HitAction::Row {
                    list: ui::HitList::Palette,
                    ..
                }
            )
        });
        let selected = match app.modals.first() {
            Some(Modal::Palette(state)) => state.selected,
            _ => panic!("the palette is open"),
        };
        app.open_ticket(
            Side::Buy,
            "NSE".to_string(),
            "INFY".to_string(),
            408_065,
            1,
            None,
        );
        assert!(matches!(app.modals.last(), Some(Modal::Ticket(_))));

        click(&mut app, W, H, row.rect.x + 2, row.rect.y);
        match app.modals.first() {
            Some(Modal::Palette(state)) => assert_eq!(state.selected, selected),
            _ => panic!("the palette is still beneath the ticket"),
        }
    }

    fn header_regions(app: &App) -> Vec<(ratatui::layout::Rect, SortTarget)> {
        let hit = app.hit.borrow();
        hit.regions
            .iter()
            .filter_map(|region| match region.action {
                ui::HitAction::Header { target, .. } => Some((region.rect, target)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn header_click_sorts_by_that_column() {
        let mut app = holdings_app();
        assert_eq!(app.books.portfolio_holdings.sort(), HoldingsSort::PnlPct);
        let x = text_at(&app, W, H, HEADER_ROW, "VALUE");
        click(&mut app, W, H, x, HEADER_ROW);
        assert_eq!(app.books.portfolio_holdings.sort(), HoldingsSort::Value);
        let x = text_at(&app, W, H, HEADER_ROW, "SYMBOL");
        click(&mut app, W, H, x, HEADER_ROW);
        assert_eq!(app.books.portfolio_holdings.sort(), HoldingsSort::Symbol);
    }

    #[test]
    fn a_header_click_lands_on_its_label_at_every_width() {
        for width in [80u16, 120, 200] {
            let mut app = holdings_app();
            app.books.portfolio_holdings.set_sort(HoldingsSort::PnlPct);
            let x = text_at(&app, width, H, HEADER_ROW, "VALUE");
            click(&mut app, width, H, x, HEADER_ROW);
            assert_eq!(
                app.books.portfolio_holdings.sort(),
                HoldingsSort::Value,
                "width {width}: a click on VALUE sorted by something else"
            );
        }
    }

    #[test]
    fn clicking_the_active_header_advances_the_cycle() {
        let mut app = holdings_app();
        app.books.portfolio_holdings.set_sort(HoldingsSort::Value);
        let x = text_at(&app, W, H, HEADER_ROW, "VALUE");
        click(&mut app, W, H, x, HEADER_ROW);
        assert_eq!(
            app.books.portfolio_holdings.sort(),
            HoldingsSort::Value.next()
        );
    }

    #[test]
    fn header_click_keeps_the_selected_record() {
        let mut app = holdings_app();

        app.books.portfolio_holdings.set_sort(HoldingsSort::Symbol);
        app.set_selection(3);
        let selected = |app: &App| -> String {
            let order = app.holdings_order_in(PaneId::PortfolioHoldings);
            let idx = order[app.pane_row(PaneId::PortfolioHoldings)];
            app.holdings.as_ref().expect("holdings")[idx]
                .tradingsymbol
                .clone()
        };
        assert_eq!(selected(&app), "H3");
        let x = text_at(&app, W, H, HEADER_ROW, "VALUE");
        click(&mut app, W, H, x, HEADER_ROW);
        assert_eq!(app.books.portfolio_holdings.sort(), HoldingsSort::Value);
        assert_eq!(
            selected(&app),
            "H3",
            "the click moved the selection to another holding"
        );
    }

    #[test]
    fn a_row_click_queued_behind_a_header_click_selects_nothing() {
        let mut app = holdings_app();
        app.books.portfolio_holdings.set_sort(HoldingsSort::Symbol);
        app.set_selection(0);
        let selected = |app: &App| -> String {
            let order = app.holdings_order_in(PaneId::PortfolioHoldings);
            let idx = order[app.pane_row(PaneId::PortfolioHoldings)];
            app.holdings.as_ref().expect("holdings")[idx]
                .tradingsymbol
                .clone()
        };
        assert_eq!(selected(&app), "H0");
        render(&app, W, H);
        let rows = region_of(&app, |action| {
            matches!(
                action,
                ui::HitAction::Rows {
                    list: ui::HitList::Pane(_),
                    ..
                }
            )
        });

        let x = text_at(&app, W, H, HEADER_ROW, "VALUE");
        click(&mut app, W, H, x, HEADER_ROW);
        assert_eq!(app.books.portfolio_holdings.sort(), HoldingsSort::Value);
        assert_ne!(
            app.pane_row(PaneId::PortfolioHoldings),
            0,
            "the reorder moved the selected holding off row 0"
        );

        press_at(&mut app, rows.rect.x + 2, rows.rect.y);
        assert_eq!(
            selected(&app),
            "H0",
            "the stale row target selected the record that took its place"
        );
    }

    #[test]
    fn header_rects_cover_their_own_columns_and_not_the_gaps() {
        let app = holdings_app();
        render(&app, W, H);
        let regions = header_regions(&app);
        assert_eq!(
            regions.len(),
            HoldingsSort::ALL.len(),
            "one target per sortable Holdings column at a wide terminal"
        );
        for (rect, _) in &regions {
            assert_eq!(rect.height, 1, "a header is one row");
            assert_eq!(
                rect.y, HEADER_ROW,
                "the header sits below the block's top border"
            );
        }

        let mut sorted = regions.clone();
        sorted.sort_by_key(|(rect, _)| rect.x);
        for pair in sorted.windows(2) {
            let (left, right) = (pair[0].0, pair[1].0);
            assert!(
                left.right() < right.x,
                "header rects {left:?} and {right:?} touch, so a gap click sorts"
            );
        }

        let value_x = text_at(&app, W, H, HEADER_ROW, "VALUE");
        let (rect, target) = regions
            .iter()
            .find(|(rect, _)| rect.x <= value_x && value_x < rect.right())
            .expect("a target covers the VALUE label");
        assert_eq!(*target, SortTarget::Holdings(HoldingsSort::Value));
        assert!(rect.x <= value_x, "{rect:?} vs {value_x}");
    }

    #[test]
    fn a_dropped_column_records_no_header_target() {
        let mut app = test_app();
        app.set_pane(PaneId::OrdersBook);
        app.orders = Some(vec![sample_order("COMPLETE")]);
        render(&app, 79, 24);
        let targets: Vec<SortTarget> = header_regions(&app)
            .into_iter()
            .map(|(_, target)| target)
            .collect();
        assert!(
            !targets.contains(&SortTarget::Orders(OrdersSort::Time)),
            "TIME is dropped at 79 columns but still clickable: {targets:?}"
        );
        assert!(
            targets.contains(&SortTarget::Orders(OrdersSort::Symbol)),
            "SYMBOL survives and must stay clickable: {targets:?}"
        );
    }

    #[test]
    fn a_chart_split_moves_the_header_rects() {
        const SPLIT_W: u16 = 150;
        let mut app = test_app();
        app.set_pane(PaneId::OrdersBook);
        app.orders = Some(vec![sample_order("COMPLETE")]);
        render(&app, SPLIT_W, H);
        let unsplit = header_regions(&app);
        app.open_chart_panel_for_test(PaneId::OrdersBook);
        render(&app, SPLIT_W, H);
        let split = header_regions(&app);
        assert!(!split.is_empty(), "the split table recorded no headers");
        assert_ne!(
            split, unsplit,
            "the split table recorded the unsplit table's rects"
        );

        let table_right = SPLIT_W - (SPLIT_W / 3).max(52);
        for (rect, _) in &split {
            assert_eq!(rect.y, HEADER_ROW, "{rect:?}");
            assert!(
                rect.right() <= table_right,
                "{rect:?} runs into the chart panel"
            );
        }

        let x = text_at(&app, SPLIT_W, H, HEADER_ROW, "STATUS");
        click(&mut app, SPLIT_W, H, x, HEADER_ROW);
        assert_eq!(app.books.orders.list.sort(), OrdersSort::Status);
    }

    #[test]
    fn clicks_that_miss_a_sortable_header_do_nothing() {
        let mut app = holdings_app();
        let before = app.books.portfolio_holdings.sort();

        let avg = text_at(&app, W, H, HEADER_ROW, "AVG");
        click(&mut app, W, H, avg, HEADER_ROW);
        assert_eq!(
            app.books.portfolio_holdings.sort(),
            before,
            "AVG is not a sortable column"
        );

        click(&mut app, W, H, 0, HEADER_ROW);
        assert_eq!(app.books.portfolio_holdings.sort(), before);

        render(&app, W, H);
        let regions = header_regions(&app);
        let leftmost = regions
            .iter()
            .map(|(rect, _)| rect.x)
            .min()
            .expect("a target exists");
        if leftmost > 0 {
            click(&mut app, W, H, leftmost - 1, 2);
            assert_eq!(
                app.books.portfolio_holdings.sort(),
                before,
                "the gap before a header sorts"
            );
        }
    }

    #[test]
    fn a_modal_swallows_a_click_on_the_header_beneath_it() {
        let mut app = holdings_app();
        let before = app.books.portfolio_holdings.sort();
        let x = text_at(&app, W, H, HEADER_ROW, "VALUE");
        app.modals.push(Modal::Funds);
        click(&mut app, W, H, x, HEADER_ROW);
        assert_eq!(
            app.books.portfolio_holdings.sort(),
            before,
            "a click reached under the modal"
        );
        assert_eq!(app.modals.len(), 1, "the modal is still open");
    }

    #[test]
    fn a_header_click_leaves_marks_and_the_chart_target_alone() {
        let mut app = test_app();
        app.set_pane(PaneId::OrdersBook);
        let mut first = sample_order("OPEN");
        first.order_id = "OID1".into();
        first.order_timestamp = "2024-01-01T10:00:00".parse().expect("timestamp");
        first.tradingsymbol = "ZZZ".into();
        let mut second = sample_order("OPEN");
        second.order_id = "OID2".into();
        second.order_timestamp = "2024-01-01T09:00:00".parse().expect("timestamp");
        second.tradingsymbol = "AAA".into();
        app.orders = Some(vec![first, second]);

        app.set_workspace(PaneId::OrdersBook.preset());
        app.focus_pane(PaneId::OrdersBook);
        app.on_key(char_key(' '));
        assert!(app.is_order_marked_in(PaneId::OrdersBook, "OID1"));
        app.open_chart_panel_for_test(PaneId::OrdersBook);
        let chart = app.chart_pane.target().cloned();

        let x = text_at(&app, W, H, HEADER_ROW, "STATUS");
        click(&mut app, W, H, x, HEADER_ROW);

        assert_eq!(app.books.orders.list.sort(), OrdersSort::Status);
        assert!(
            app.is_order_marked_in(PaneId::OrdersBook, "OID1"),
            "the click dropped the mark"
        );
        assert!(
            !app.is_order_marked_in(PaneId::OrdersBook, "OID2"),
            "the click marked another order"
        );
        assert_eq!(
            app.chart_pane
                .target()
                .as_ref()
                .map(|target| target.row_identity.clone()),
            chart.as_ref().map(|target| target.row_identity.clone()),
            "the click moved the chart to another instrument"
        );
        assert!(app.modals.is_empty(), "the click opened a modal");
    }
}
