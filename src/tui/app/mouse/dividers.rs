use super::super::{App, ToastLevel, settings, ui};

impl App {
    pub(in crate::tui::app) fn drag_divider(&mut self, column: u16, row: u16) -> bool {
        let Some(mut drag) = self.drag_split else {
            return false;
        };
        let (split, span) = (drag.split, drag.span);

        let seam = ui::HitAction::Divider { split, span };
        let on_screen = self.hit_map_is_current()
            && self
                .hit
                .borrow()
                .regions
                .iter()
                .any(|region| region.action == seam);
        if !on_screen {
            self.end_split_drag();
            return false;
        }

        if !drag.moved && split.axis_pos(column, row) == drag.origin {
            return true;
        }
        if let Some(ratio) = split.ratio_at(span, column, row) {
            drag.moved = true;
            self.drag_split = Some(drag);
            self.splits.set(split, ratio);
        }
        true
    }

    pub(in crate::tui::app) fn end_split_drag(&mut self) {
        let Some(drag) = self.drag_split.take() else {
            return;
        };
        if !drag.moved {
            return;
        }
        let split = drag.split;
        let Some(ratio) = self.splits.get(split) else {
            return;
        };
        let Some(path) = self.settings_file() else {
            return;
        };
        if let Err(err) = settings::save_split_ratio(&path, split, ratio) {
            self.toast(
                ToastLevel::Error,
                format!("divider moved but not saved: {err}"),
            );
        }
    }

    pub(in crate::tui::app) fn reset_splits(&mut self) {
        if self.splits.is_default() {
            self.toast(ToastLevel::Info, "dividers already at defaults");
            return;
        }
        self.splits.clear();
        self.drag_split = None;
        if let Some(path) = self.settings_file()
            && let Err(err) = settings::clear_split_ratios(&path)
        {
            self.toast(
                ToastLevel::Error,
                format!("dividers reset but not saved: {err}"),
            );
            return;
        }
        self.toast(ToastLevel::Info, "dividers reset");
    }
}

#[cfg(test)]
mod divider_drag_tests {

    use super::super::super::test_support::{char_key, render, test_app};
    use super::super::super::{App, settings, ui};
    use crate::tui::split::Split;
    use crate::tui::workspace::{PaneId, Preset};
    use crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use ratatui::layout::Rect;
    use std::path::PathBuf;

    const W: u16 = 160;
    const H: u16 = 40;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kite-splits-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir.join("tui.json")
    }

    fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn divider(app: &App, split: Split) -> (Rect, Rect) {
        let hit = app.hit.borrow();
        hit.regions
            .iter()
            .find_map(|region| match region.action {
                ui::HitAction::Divider { split: got, span } if got == split => {
                    Some((region.rect, span))
                }
                _ => None,
            })
            .expect("the split recorded its divider")
    }

    fn drag_to(app: &mut App, split: Split, column: u16, row: u16) {
        render(app, W, H);
        let (strip, _) = divider(app, split);
        app.on_mouse(mouse(
            MouseEventKind::Down(MouseButton::Left),
            strip.x,
            strip.y,
        ));
        app.on_mouse(mouse(MouseEventKind::Drag(MouseButton::Left), column, row));
        app.on_mouse(mouse(MouseEventKind::Up(MouseButton::Left), column, row));
    }

    fn overview_app() -> App {
        use super::super::super::test_support::{holding, position};
        let mut app = test_app();
        app.set_workspace(Preset::Overview);
        app.positions = Some({
            let mut row = crate::tui::data::test_models::positions();
            row.net = (0..3)
                .map(|i| position(&format!("P{i}"), i, 10, 0.0, 100.0, 100.0))
                .collect();
            row
        });
        app.holdings = Some(
            (0u32..4)
                .map(|i| holding(&format!("H{i}"), 100 + i, 100.0, 100.0, 0.0, 1))
                .collect(),
        );
        app
    }

    #[tokio::test]
    async fn a_drag_ends_when_its_seam_leaves_the_screen() {
        use crate::tui::workspace::Preset;

        for (name, key) in [("modal", 'F'), ("workspace", '1'), ("maximize", 'z')] {
            let mut app = test_app();
            app.set_workspace(Preset::Trading);
            render(&app, W, H);
            let (strip, _) = divider(&app, Split::TradingRoot);
            app.on_mouse(mouse(
                MouseEventKind::Down(MouseButton::Left),
                strip.x,
                strip.y,
            ));
            app.on_mouse(mouse(
                MouseEventKind::Drag(MouseButton::Left),
                strip.x + 10,
                strip.y,
            ));
            let held = app.splits.get(Split::TradingRoot);
            assert!(held.is_some(), "{name}: the first move is a resize");

            app.on_key(char_key(key));
            render(&app, W, H);
            app.on_mouse(mouse(
                MouseEventKind::Drag(MouseButton::Left),
                strip.x + 30,
                strip.y,
            ));
            assert_eq!(
                app.splits.get(Split::TradingRoot),
                held,
                "{name}: the drag must not reach through"
            );
        }
    }

    #[test]
    fn an_out_and_back_drag_returns_the_seam_to_the_press_column() {
        let mut app = trading_app();
        render(&app, W, H);
        let (strip, _) = divider(&app, Split::TradingRoot);
        app.on_mouse(mouse(
            MouseEventKind::Down(MouseButton::Left),
            strip.x,
            strip.y,
        ));
        for column in [strip.x - 12, strip.x] {
            app.on_mouse(mouse(
                MouseEventKind::Drag(MouseButton::Left),
                column,
                strip.y,
            ));
        }
        render(&app, W, H);
        let (back, _) = divider(&app, Split::TradingRoot);
        assert_eq!(back.x, strip.x, "the seam is back under the pointer");
    }

    #[test]
    fn a_drag_moves_the_trading_root_seam_to_the_pointer() {
        let mut app = trading_app();
        render(&app, W, H);
        let (strip, span) = divider(&app, Split::TradingRoot);
        assert_eq!(strip.width, 1, "the seam is one column");
        assert_eq!(span.width, W, "the split covers the whole body width");
        assert_eq!(
            app.splits.get(Split::TradingRoot),
            None,
            "nothing is stored until a drag"
        );

        drag_to(&mut app, Split::TradingRoot, span.x + 70, 10);

        assert_eq!(
            app.splits.length(Split::TradingRoot, W, 104),
            71,
            "the list is as wide as the pointer asked"
        );

        render(&app, W, H);
        let (moved, _) = divider(&app, Split::TradingRoot);
        assert_eq!(moved.x, span.x + 70, "the seam followed the pointer");
    }

    #[test]
    fn a_drag_past_either_minimum_clamps_instead_of_starving_a_panel() {
        let (min_list, min_chart) = Split::TradingRoot.minimums();
        let mut app = trading_app();
        render(&app, W, H);
        let (_, span) = divider(&app, Split::TradingRoot);

        drag_to(&mut app, Split::TradingRoot, 0, 10);
        assert_eq!(app.splits.length(Split::TradingRoot, W, 104), min_list);
        render(&app, W, H);
        let (left, _) = divider(&app, Split::TradingRoot);
        assert_eq!(left.x, span.x + min_list - 1);

        drag_to(&mut app, Split::TradingRoot, W - 1, 10);
        assert_eq!(app.splits.length(Split::TradingRoot, W, 104), W - min_chart);
    }

    #[test]
    fn a_drag_moves_the_overview_books_seam_by_rows() {
        let mut app = overview_app();
        render(&app, W, H);
        let (strip, span) = divider(&app, Split::OverviewBooks);
        assert_eq!(strip.height, 1, "the seam is one row");
        assert!(span.height > 0);

        let (min_positions, min_holdings) = Split::OverviewBooks.minimums();
        drag_to(&mut app, Split::OverviewBooks, 4, span.y + span.height - 1);
        assert_eq!(
            app.splits
                .length(Split::OverviewBooks, span.height, span.height),
            span.height - min_holdings,
            "the holdings book keeps its floor"
        );
        drag_to(&mut app, Split::OverviewBooks, 4, span.y);
        assert_eq!(
            app.splits
                .length(Split::OverviewBooks, span.height, span.height),
            min_positions,
            "and so does the positions book"
        );
    }

    #[test]
    fn a_drag_moves_the_chart_split() {
        use super::super::super::test_support::position;
        let mut app = test_app();
        app.set_pane(PaneId::PortfolioPositions);
        app.positions = Some({
            let mut row = crate::tui::data::test_models::positions();
            row.net = vec![position("INFY", 408_065, 10, 0.0, 100.0, 100.0)];
            row
        });
        app.open_chart_panel_for_test(PaneId::PortfolioPositions);
        render(&app, W, H);

        let (_, span) = divider(&app, Split::ChartRight);
        let (min_table, min_chart) = Split::ChartRight.minimums();

        drag_to(&mut app, Split::ChartRight, span.x + 90, 10);
        assert_eq!(app.splits.length(Split::ChartRight, span.width, 0), 91);
        drag_to(&mut app, Split::ChartRight, span.x + span.width, 10);
        assert_eq!(
            app.splits.length(Split::ChartRight, span.width, 0),
            span.width - min_chart
        );
        drag_to(&mut app, Split::ChartRight, span.x, 10);
        assert_eq!(
            app.splits.length(Split::ChartRight, span.width, 0),
            min_table
        );
    }

    #[test]
    fn the_release_writes_the_ratio_and_the_reset_key_clears_it() {
        let path = scratch("persist");
        let mut app = trading_app();
        app.settings_path = Some(path.clone());
        render(&app, W, H);
        let (_, span) = divider(&app, Split::TradingRoot);

        drag_to(&mut app, Split::TradingRoot, span.x + 80, 10);
        let saved = settings::from_json(&std::fs::read_to_string(&path).expect("written"));
        assert_eq!(
            saved.split_ratios().get(Split::TradingRoot),
            app.splits.get(Split::TradingRoot),
            "the file holds the ratio the app is drawing"
        );
        assert_eq!(saved.split_ratios().length(Split::TradingRoot, W, 104), 81);

        app.on_key(KeyEvent::new(KeyCode::Char('='), KeyModifiers::NONE));
        assert!(app.splits.is_default());
        let cleared = settings::from_json(&std::fs::read_to_string(&path).expect("written"));
        assert!(cleared.split_ratios().is_default());
        assert!(cleared.splits.is_empty(), "the `splits` object is emptied");
    }

    #[test]
    fn the_drag_itself_writes_nothing() {
        let path = scratch("no-write-mid-drag");
        let mut app = trading_app();
        app.settings_path = Some(path.clone());
        render(&app, W, H);
        let (strip, span) = divider(&app, Split::TradingRoot);

        app.on_mouse(mouse(
            MouseEventKind::Down(MouseButton::Left),
            strip.x,
            strip.y,
        ));
        for column in (60..80).rev() {
            app.on_mouse(mouse(
                MouseEventKind::Drag(MouseButton::Left),
                span.x + column,
                10,
            ));
            assert!(!path.exists(), "column {column} wrote the file");
        }
        app.on_mouse(mouse(
            MouseEventKind::Up(MouseButton::Left),
            span.x + 60,
            10,
        ));
        assert!(path.exists(), "the release writes once");
    }

    #[test]
    fn a_stored_ratio_is_the_extent_the_next_session_draws() {
        let settings = settings::from_json(r#"{"splits":{"trading_root":0.4}}"#);
        assert_eq!(
            settings.split_ratios().length(Split::TradingRoot, W, 104),
            64
        );

        let absurd = settings::from_json(r#"{"splits":{"trading_root":9.0}}"#);
        assert_eq!(
            absurd.split_ratios().length(Split::TradingRoot, W, 104),
            W - Split::TradingRoot.minimums().1
        );

        let broken = settings::from_json(r#"{"splits":{"trading_root":"wide"}}"#);
        assert!(broken.split_ratios().is_default());
    }

    #[test]
    fn a_drag_across_the_other_axis_alone_moves_nothing() {
        let path = scratch("off-axis");
        let mut app = trading_app();
        app.settings_path = Some(path.clone());
        render(&app, W, H);
        let (strip, _) = divider(&app, Split::TradingRoot);

        app.on_mouse(mouse(
            MouseEventKind::Down(MouseButton::Left),
            strip.x,
            strip.y,
        ));
        for row in strip.y..strip.y + 5 {
            app.on_mouse(mouse(MouseEventKind::Drag(MouseButton::Left), strip.x, row));
        }
        app.on_mouse(mouse(
            MouseEventKind::Up(MouseButton::Left),
            strip.x,
            strip.y + 4,
        ));

        assert!(app.splits.is_default(), "no ratio was asked for");
        assert!(!path.exists(), "and nothing was written");
        render(&app, W, H);
        let (after, _) = divider(&app, Split::TradingRoot);
        assert_eq!(after.x, strip.x, "the seam did not budge");
    }

    #[test]
    fn a_press_and_release_without_travel_writes_nothing() {
        let path = scratch("no-travel");
        let mut app = trading_app();
        app.settings_path = Some(path.clone());
        render(&app, W, H);
        let (strip, span) = divider(&app, Split::TradingRoot);

        drag_to(&mut app, Split::TradingRoot, span.x + 80, 10);
        assert!(path.exists(), "the real drag wrote");
        let written = std::fs::read_to_string(&path).expect("written");

        app.on_mouse(mouse(
            MouseEventKind::Down(MouseButton::Left),
            strip.x,
            strip.y,
        ));
        app.on_mouse(mouse(
            MouseEventKind::Up(MouseButton::Left),
            strip.x,
            strip.y,
        ));
        assert_eq!(
            std::fs::read_to_string(&path).expect("still there"),
            written,
            "a bare click on the seam rewrote the file"
        );
    }

    #[test]
    fn a_resize_drops_the_old_seam_and_the_drag_held_through_it() {
        use crate::tui::event::AppEvent;
        let mut app = trading_app();
        render(&app, 160, 45);
        let (strip, span) = divider(&app, Split::TradingRoot);

        app.on_mouse(mouse(
            MouseEventKind::Down(MouseButton::Left),
            strip.x,
            strip.y,
        ));
        app.on_event(AppEvent::Resize);
        app.on_mouse(mouse(
            MouseEventKind::Drag(MouseButton::Left),
            span.x + 70,
            10,
        ));
        app.on_mouse(mouse(
            MouseEventKind::Up(MouseButton::Left),
            span.x + 70,
            10,
        ));
        assert_eq!(
            app.splits.get(Split::TradingRoot),
            None,
            "a drag held through a resize moved the seam"
        );

        render(&app, 160, 45);
        app.on_event(AppEvent::Resize);
        app.on_mouse(mouse(
            MouseEventKind::Down(MouseButton::Left),
            strip.x,
            strip.y,
        ));
        app.on_mouse(mouse(
            MouseEventKind::Drag(MouseButton::Left),
            span.x + 70,
            10,
        ));
        assert_eq!(
            app.splits.get(Split::TradingRoot),
            None,
            "a queued press found the old terminal's seam"
        );
    }

    #[test]
    fn a_press_away_from_a_seam_starts_no_drag() {
        let mut app = trading_app();
        render(&app, W, H);
        app.on_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 4, 4));
        app.on_mouse(mouse(MouseEventKind::Drag(MouseButton::Left), 4, 20));
        assert!(app.splits.is_default(), "nothing moved");
    }

    #[test]
    fn the_orders_and_portfolio_seams_drag_and_are_remembered() {
        use crate::tui::workspace::Preset;
        let path = scratch("orders-portfolio");
        for (preset, split) in [
            (Preset::Orders, Split::OrdersRoot),
            (Preset::Orders, Split::OrdersRight),
            (Preset::Portfolio, Split::PortfolioRoot),
            (Preset::Portfolio, Split::PortfolioLeft),
            (Preset::Portfolio, Split::PortfolioRight),
        ] {
            let mut app = test_app();
            app.settings_path = Some(path.clone());
            app.set_workspace(preset);
            render(&app, W, H);
            let (strip, span) = divider(&app, split);
            let (column, row) = match split.axis() {
                crate::tui::split::Axis::Horizontal => (span.x + span.width / 3, strip.y),
                crate::tui::split::Axis::Vertical => (strip.x, span.y + span.height / 3),
            };
            drag_to(&mut app, split, column, row);
            let stored = app.splits.get(split);
            assert!(stored.is_some(), "{split:?} moved");
            let saved = settings::from_json(&std::fs::read_to_string(&path).expect("written"));
            assert_eq!(saved.split_ratios().get(split), stored, "{split:?} on disk");
        }
    }

    fn trading_app() -> App {
        use super::super::super::test_support::sample_order;
        use crate::tui::marketwatch::WatchItem;
        use crate::tui::workspace::Preset;
        let mut app = test_app();
        app.set_workspace(Preset::Trading);
        app.orders = Some(vec![sample_order("OPEN")]);
        app.set_watch_for_test(crate::tui::marketwatch::State::test_loaded(vec![
            WatchItem {
                exchange: "NSE".into(),
                tradingsymbol: "INFY".into(),
                instrument_token: 408_065,
                ..WatchItem::default()
            },
        ]));
        app
    }

    #[test]
    fn every_composite_seam_is_draggable_and_remembered() {
        let path = scratch("composite");
        let mut app = trading_app();
        app.settings_path = Some(path.clone());
        render(&app, W, H);

        let (root, root_span) = divider(&app, Split::TradingRoot);
        assert_eq!(root.width, 1, "a vertical seam is one column");
        let (right, right_span) = divider(&app, Split::TradingRight);
        assert_eq!(right.height, 1, "a horizontal seam is one row");
        let (_, bottom_span) = divider(&app, Split::TradingBottom);
        assert!(
            bottom_span.width < root_span.width,
            "the innermost seam divides only the right column"
        );

        assert_eq!(root.x, root_span.x + 63);
        drag_to(&mut app, Split::TradingRoot, root_span.x + 49, 10);
        render(&app, W, H);
        assert_eq!(
            divider(&app, Split::TradingRoot).0.x,
            root_span.x + 49,
            "the watch list is as wide as the pointer asked"
        );

        drag_to(
            &mut app,
            Split::TradingRight,
            right_span.x + 4,
            right_span.y + 12,
        );
        render(&app, W, H);
        assert_eq!(
            divider(&app, Split::TradingRight).0.y,
            right_span.y + 12,
            "the chart is as tall as the pointer asked"
        );

        let saved = settings::from_json(&std::fs::read_to_string(&path).expect("read"));
        let ratios = saved.split_ratios();
        assert!(ratios.get(Split::TradingRoot).is_some());
        assert!(ratios.get(Split::TradingRight).is_some());
        assert_eq!(
            ratios.get(Split::TradingBottom),
            None,
            "an undragged seam stores nothing"
        );
    }

    #[test]
    fn a_composite_seam_stops_at_its_panes_minimums() {
        let (watch_min, chart_min) = Split::TradingRoot.minimums();
        let mut app = trading_app();
        render(&app, W, H);
        let (_, span) = divider(&app, Split::TradingRoot);

        drag_to(&mut app, Split::TradingRoot, 0, 10);
        render(&app, W, H);
        assert_eq!(
            divider(&app, Split::TradingRoot).0.x,
            span.x + watch_min - 1
        );

        drag_to(&mut app, Split::TradingRoot, W - 1, 10);
        render(&app, W, H);
        assert_eq!(
            divider(&app, Split::TradingRoot).0.x,
            span.x + span.width - chart_min - 1
        );
    }

    #[test]
    fn the_reset_key_is_a_no_op_when_no_divider_has_moved() {
        let path = scratch("reset-noop");
        let mut app = trading_app();
        app.settings_path = Some(path.clone());
        app.on_key(KeyEvent::new(KeyCode::Char('='), KeyModifiers::NONE));
        assert!(!path.exists());
        assert!(app.splits.is_default());
    }
}
