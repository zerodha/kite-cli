pub(super) mod chart;
pub(super) mod dividers;
pub(super) mod lists;
pub(super) mod modals;

use super::{App, SplitDrag, ui};
use crate::tui::workspace::PaneId;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

impl App {
    pub(in crate::tui::app) fn on_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::ScrollUp => self.on_scroll(mouse.column, mouse.row, -1),
            MouseEventKind::ScrollDown => self.on_scroll(mouse.column, mouse.row, 1),
            MouseEventKind::Down(MouseButton::Left) => {
                self.end_split_drag();
                self.on_click(mouse.column, mouse.row);
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if !self.drag_divider(mouse.column, mouse.row) {
                    self.on_chart_hover(mouse.column, mouse.row);
                }
            }
            MouseEventKind::Up(MouseButton::Left) => self.end_split_drag(),

            MouseEventKind::Moved => {
                self.end_split_drag();
                self.on_chart_hover(mouse.column, mouse.row);
            }
            _ => {}
        }
    }

    pub(in crate::tui::app) fn hit_map_is_current(&self) -> bool {
        self.hit.borrow().surface.as_ref() == Some(&ui::Surface::of(self))
    }

    fn on_click(&mut self, column: u16, row: u16) {
        if !self.hit_map_is_current() {
            return;
        }

        if let Some((id, hover)) = self.chart_pointer(column, row) {
            let owner = self
                .workspace()
                .panes()
                .iter()
                .copied()
                .find(|pane| self.chart_of(*pane).is_some_and(|chart| chart.id() == id));
            if let Some(owner) = owner {
                self.focus_pane(owner);
            }
            self.clear_chart_hovers();
            let offset = self
                .chart_by_id(id)
                .and_then(|chart| chart.candle_offset(&hover.ts, &self.candles));
            if let Some(chart) = self.chart_by_id_mut(id) {
                chart.set_cursor(offset);
            }
            return;
        }

        let Some(region) = self.hit.borrow().at(column, row) else {
            return;
        };

        if let ui::HitAction::Rows { list, .. } | ui::HitAction::Row { list, .. } = region.action
            && !self.hit.borrow().rows_are_current(self, list)
        {
            return;
        }
        match region.action {
            ui::HitAction::Rows { list, offset } => {
                if let Some(index) = ui::table_row_hit(region.rect, offset, column, row) {
                    self.select_in_list(list, index);
                }
            }
            ui::HitAction::Row { list, index } => self.select_in_list(list, index),

            ui::HitAction::Header { owner, target } => {
                self.focus_pane(owner);
                self.sort_by_header(owner, target);
            }
            ui::HitAction::NoteCaret { scroll } => {
                self.place_note_caret(region.rect, scroll, column, row);
            }
            ui::HitAction::Pane(pane) => self.focus_pane(pane),
            ui::HitAction::Workspace(preset) => self.switch_workspace(preset),
            ui::HitAction::ModalControl { field, intent } => self.modal_control(field, intent),
            ui::HitAction::Key(key) => self.on_key(key),

            ui::HitAction::BidsSection(section) => {
                self.focus_pane(PaneId::InvestBids);
                self.set_bids_section(section);
            }
            ui::HitAction::Divider { split, span } => {
                self.drag_split = Some(SplitDrag {
                    split,
                    span,
                    origin: split.axis_pos(column, row),
                    moved: false,
                });
            }

            ui::HitAction::Text(_) | ui::HitAction::Capture => {}
        }
    }

    fn on_scroll(&mut self, column: u16, row: u16, delta: isize) {
        if !self.hit_map_is_current() {
            return;
        }

        if self.modals.is_empty() && self.hit.borrow().chart_under(column, row).is_some() {
            return;
        }
        let action = self
            .hit
            .borrow()
            .at(column, row)
            .map(|region| region.action);
        match action {
            Some(ui::HitAction::Rows { list, .. } | ui::HitAction::Row { list, .. }) => {
                self.scroll_list(list, delta);
            }
            Some(ui::HitAction::Text(text)) => self.scroll_text(text, delta),

            Some(ui::HitAction::ModalControl { .. }) => {
                self.scroll_text(ui::HitText::ModalBody, delta);
            }

            Some(ui::HitAction::Pane(pane)) => self.scroll_pane(pane, delta),

            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{
        H, W, click, edit_ticket, hint_of, open_ticket_state, press_at, region_of, render,
        test_app, text_at, ticket_app, ticket_option, ticket_row,
    };
    use super::super::{KeyCode, KeyEvent, KeyModifiers, Modal, PositionsSort, TicketField, ui};
    use crate::tui::workspace::{PaneId, Preset};
    use kiteconnect::OrderType;

    #[test]
    fn the_orders_tab_click_switches_to_the_orders_workspace() {
        let mut app = test_app();
        app.set_workspace(Preset::Trading);
        let x = text_at(&app, W, H, 0, "5 Orders");
        click(&mut app, W, H, x + 2, 0);
        assert_eq!(app.focused_pane(), PaneId::OrdersBook);
        assert_eq!(app.workspace(), Preset::Orders);
    }

    #[tokio::test]
    async fn a_strip_click_switches_workspace() {
        let mut app = test_app();
        app.set_pane(PaneId::PortfolioHoldings);
        let x = text_at(&app, W, H, 0, "Trading");
        click(&mut app, W, H, x + 1, 0);
        assert_eq!(app.workspace(), Preset::Trading);
    }

    #[tokio::test]
    async fn a_click_on_each_tab_selects_its_preset_at_every_width() {
        for (width, height) in [(160u16, 45u16), (120, 35), (80, 24), (40, 24)] {
            let mut app = test_app();
            for start in Preset::ALL {
                app.set_workspace(start);
                render(&app, width, height);
                let tabs: Vec<(Preset, ratatui::layout::Rect)> = app
                    .hit
                    .borrow()
                    .regions
                    .iter()
                    .filter_map(|region| match region.action {
                        ui::HitAction::Workspace(preset) => Some((preset, region.rect)),
                        _ => None,
                    })
                    .collect();
                if width >= 80 {
                    let targets: Vec<Preset> = tabs.iter().map(|(preset, _)| *preset).collect();
                    assert_eq!(targets, Preset::ALL, "{width}x{height}");
                }
                for (target, rect) in tabs {
                    for column in [rect.x, rect.right() - 1] {
                        app.set_workspace(start);
                        click(&mut app, width, height, column, 0);
                        assert_eq!(
                            app.workspace(),
                            target,
                            "{width}x{height}: from {start:?}, column {column}"
                        );
                    }
                }
            }
        }
    }

    fn pane_outer(app: &super::super::App, pane: PaneId) -> ratatui::layout::Rect {
        use crate::tui::workspace::{Request, resolve};
        let preset = app.workspace();
        let body = ratatui::layout::Rect::new(0, 1, W, H - 2);
        let resolved = resolve(
            body,
            &Request {
                preset,
                focused: app.focused_pane(),
                maximized: app.workspaces.maximized(preset),
                ratios: &app.splits,
                overview: None,
            },
        );
        resolved
            .rect(pane)
            .unwrap_or_else(|| panic!("{pane:?} is drawn at {W}x{H}"))
    }

    #[test]
    fn a_click_on_any_composite_pane_title_or_blank_body_focuses_it() {
        for (preset, pane, other) in [
            (Preset::Trading, PaneId::TradingWatch, PaneId::TradingOrders),
            (Preset::Trading, PaneId::TradingChart, PaneId::TradingWatch),
            (Preset::Trading, PaneId::TradingDepth, PaneId::TradingWatch),
            (Preset::Trading, PaneId::TradingOrders, PaneId::TradingWatch),
            (
                Preset::Portfolio,
                PaneId::PortfolioPositions,
                PaneId::PortfolioFunds,
            ),
            (
                Preset::Portfolio,
                PaneId::PortfolioHoldings,
                PaneId::PortfolioFunds,
            ),
            (
                Preset::Portfolio,
                PaneId::PortfolioFunds,
                PaneId::PortfolioPositions,
            ),
            (
                Preset::Derivatives,
                PaneId::DerivativesChain,
                PaneId::DerivativesChart,
            ),
            (
                Preset::Derivatives,
                PaneId::DerivativesChart,
                PaneId::DerivativesChain,
            ),
        ] {
            let mut app = test_app();
            app.set_workspace(preset);
            let outer = pane_outer(&app, pane);
            for (column, row) in [(outer.x + 2, outer.y), (outer.x + 1, outer.bottom() - 2)] {
                app.focus_pane_for_test(other);
                click(&mut app, W, H, column, row);
                assert_eq!(app.focused_pane(), pane, "{pane:?} at ({column}, {row})");
            }
        }
    }

    #[test]
    fn a_bids_section_click_selects_that_section() {
        let issue = |symbol: &str| crate::tui::bids::Instrument {
            symbol: symbol.into(),
            ..Default::default()
        };
        let mut app = test_app();
        app.set_workspace(Preset::Invest);
        app.bids.set_instruments(
            crate::tui::bids::Section::Government,
            Ok(vec![issue("GS2030"), issue("GS2035")]),
        );
        render(&app, W, H);
        let label = region_of(&app, |action| {
            action == ui::HitAction::BidsSection(crate::tui::bids::Section::Government)
        });
        click(&mut app, W, H, label.rect.x + 1, label.rect.y);
        assert_eq!(
            app.bids_pane.section(),
            crate::tui::bids::Section::Government
        );
        assert_eq!(app.focused_pane(), PaneId::InvestBids);
        app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(app.pane_row(PaneId::InvestBids), 1, "j moves the Bids rows");
    }

    #[test]
    fn every_in_pane_target_focuses_the_pane_that_drew_it() {
        let mut checked = 0;
        for preset in Preset::ALL {
            let mut app = test_app();
            app.set_workspace(preset);
            render(&app, W, H);
            let targets: Vec<(u16, u16)> = app
                .hit
                .borrow()
                .regions
                .iter()
                .filter(|region| {
                    matches!(
                        region.action,
                        ui::HitAction::Rows { .. }
                            | ui::HitAction::Row { .. }
                            | ui::HitAction::Header { .. }
                            | ui::HitAction::BidsSection(_)
                            | ui::HitAction::Pane(_)
                    )
                })
                .map(|region| (region.rect.x + region.rect.width / 2, region.rect.y))
                .collect();
            for (column, row) in targets {
                let owner = preset
                    .panes()
                    .iter()
                    .copied()
                    .find(|pane| {
                        pane_outer(&app, *pane)
                            .contains(ratatui::layout::Position::new(column, row))
                    })
                    .unwrap_or_else(|| panic!("{preset:?}: ({column}, {row}) is in a pane"));
                for other in preset.panes().iter().copied().filter(|pane| *pane != owner) {
                    let mut app = test_app();
                    app.set_workspace(preset);
                    app.focus_pane_for_test(other);
                    click(&mut app, W, H, column, row);
                    assert_eq!(
                        app.focused_pane(),
                        owner,
                        "{preset:?}: ({column}, {row}) clicked from {other:?}"
                    );
                    checked += 1;
                }
            }
        }
        assert!(checked > 0, "the workspaces record in-pane targets");
    }

    #[test]
    fn footer_hint_click_presses_the_key_it_advertises() {
        let mut app = test_app();
        app.set_pane(PaneId::PortfolioPositions);

        let x = text_at(&app, W, H, H - 1, "S:sort");
        assert_eq!(
            app.books.portfolio_positions.sort(),
            PositionsSort::default()
        );
        click(&mut app, W, H, x, H - 1);
        assert_eq!(
            app.books.portfolio_positions.sort(),
            PositionsSort::default().next()
        );
    }

    #[test]
    fn footer_group_divider_is_dimmer_than_the_hints() {
        let app = test_app();
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(W, H)).expect("terminal");
        terminal
            .draw(|frame| ui::draw(frame, &app))
            .expect("frame draws");
        let buffer = terminal.backend().buffer().clone();
        let row = H - 1;
        let line: String = (0..W).map(|x| buffer[(x, row)].symbol()).collect();
        let divider = u16::try_from(line.find('│').expect("the footer groups its hints"))
            .expect("fixture column fits u16");
        let hint = u16::try_from(line.find("?:help").expect("the footer offers help"))
            .expect("fixture column fits u16");
        assert!(
            buffer[(divider, row)]
                .modifier
                .contains(ratatui::style::Modifier::DIM),
            "the divider must be dimmer than the hints"
        );
        assert!(
            !buffer[(hint, row)]
                .modifier
                .contains(ratatui::style::Modifier::DIM),
            "a hint must not be dimmed to divider weight"
        );
        assert_eq!(
            buffer[(divider, row)].fg,
            buffer[(hint, row)].fg,
            "the divider shares the hint colour; only its weight differs"
        );
    }

    #[test]
    fn footer_group_divider_click_does_nothing() {
        let mut app = test_app();
        app.set_pane(PaneId::PortfolioPositions);
        let x = text_at(&app, W, H, H - 1, "│");
        click(&mut app, W, H, x, H - 1);
        assert_eq!(
            app.books.portfolio_positions.sort(),
            PositionsSort::default()
        );
        assert!(app.modals.is_empty(), "a divider is not a key");
    }

    #[test]
    fn footer_hint_click_past_the_last_hint_does_nothing() {
        let mut app = test_app();
        app.set_pane(PaneId::PortfolioPositions);
        click(&mut app, W, H, W - 1, H - 1);
        assert_eq!(
            app.books.portfolio_positions.sort(),
            PositionsSort::default()
        );
    }

    #[test]
    fn footer_hint_click_reaches_the_open_modals_own_keys() {
        let mut app = test_app();
        app.modals.push(Modal::Funds);
        let x = text_at(&app, W, H, H - 1, "Esc:close");
        click(&mut app, W, H, x, H - 1);
        assert!(app.modals.is_empty(), "`Esc:close` closes the Funds modal");
    }

    #[test]
    fn a_stale_rectangle_from_before_a_mode_change_reaches_nothing() {
        let mut app = ticket_app();
        render(&app, W, H);

        let price = ticket_row(&app, TicketField::Price);
        let order_type = ticket_option(&app, TicketField::OrderType, 0);
        click(&mut app, W, H, order_type.rect.x + 1, order_type.rect.y);
        assert_eq!(open_ticket_state(&app).order_type, OrderType::Market);

        render(&app, W, H);
        assert!(
            app.hit.borrow().regions.iter().all(|region| !matches!(
                region.action,
                ui::HitAction::ModalControl {
                    field: ui::ModalField::Ticket(TicketField::Price),
                    ..
                }
            )),
            "the hidden price row leaves no target"
        );
        click(&mut app, W, H, price.rect.x + 2, price.rect.y);
        let state = open_ticket_state(&app);
        assert_eq!(state.focused(), TicketField::Product);
        assert_eq!(state.price.len(), 0);
    }

    #[tokio::test]
    async fn a_second_queued_review_press_cannot_confirm_an_undrawn_order() {
        let enter = |action: ui::HitAction| {
            action == ui::HitAction::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        };
        let mut app = ticket_app();
        edit_ticket(&mut app, |state| {
            state.qty = "10".to_string();
            state.price = "1523.40".to_string();
        });
        render(&app, W, H);
        let button = region_of(&app, enter);
        let hint = hint_of(&app, enter);

        press_at(&mut app, button.rect.x + 1, button.rect.y);
        assert!(
            matches!(app.modals.last(), Some(Modal::Confirm(_))),
            "the first press opens the review"
        );

        press_at(&mut app, button.rect.x + 1, button.rect.y);
        press_at(&mut app, hint.rect.x, hint.rect.y);
        assert!(
            matches!(app.modals.last(), Some(Modal::Confirm(_))),
            "the review is still on screen, so nothing was dispatched"
        );
        assert_eq!(app.modals.len(), 2, "and the ticket is still under it");
    }
}
