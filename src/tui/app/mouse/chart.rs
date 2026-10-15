use super::super::{App, ChartHover};
use crate::tui::pane_state::ChartId;

impl App {
    pub(in crate::tui::app) fn chart_pointer(
        &self,
        column: u16,
        row: u16,
    ) -> Option<(ChartId, ChartHover)> {
        if !self.modals.is_empty() || !self.hit_map_is_current() {
            return None;
        }
        let map = self.hit.borrow();
        let entry = map.chart_under(column, row)?;
        let chart = self.chart_by_id(entry.chart)?;
        let hit = &entry.hit;
        if Some(hit.token) != chart.token() || hit.interval != chart.interval() {
            return None;
        }
        let index = hit.candle_at(column, row)?;
        Some((
            entry.chart,
            ChartHover {
                token: hit.token,
                interval: hit.interval,
                ts: hit.visible_ts.get(index - hit.start)?.clone(),
                window_ts: hit.visible_ts.first()?.clone(),
                visible_n: hit.visible_ts.len(),
            },
        ))
    }

    pub(in crate::tui::app) fn on_chart_hover(&mut self, column: u16, row: u16) {
        let Some((id, hover)) = self.chart_pointer(column, row) else {
            self.clear_chart_hovers();
            return;
        };

        let anchored = self.chart_by_id(id).map(|chart| {
            chart
                .hover()
                .filter(|previous| {
                    previous.token == hover.token
                        && previous.interval == hover.interval
                        && chart
                            .candle_offset(&previous.window_ts, &self.candles)
                            .is_some()
                })
                .map(|previous| (previous.window_ts.clone(), previous.visible_n))
        });
        if let Some(chart) = self.chart_by_id_mut(id) {
            chart.inspect(hover, anchored.flatten());
        }
    }
}

#[cfg(test)]
mod chart_pointer_tests {
    use super::super::super::test_support::{char_key, holding, test_app};
    use super::super::super::{App, AppEvent, ChartInterval, Modal, SeriesKey, ui};
    use crate::tui::data::Candle;
    use crate::tui::workspace::PaneId;
    use crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    const INFY: u32 = 408_065;
    const RELIANCE: u32 = 738_561;

    fn candles(n: usize) -> Vec<Candle> {
        (0..n)
            .map(|index| {
                let mut row = crate::tui::data::test_models::candle();
                row.date = format!(
                    "{}+05:30",
                    (format!("2026-09-09 09:{:02}:00", 15 + index)).replace(' ', "T")
                )
                .parse()
                .expect("date");
                let price_offset =
                    f64::from(u32::try_from(index).expect("fixture candle index fits u32"));
                row.open = 100.0 + price_offset;
                row.high = 101.0 + price_offset;
                row.low = 99.0 + price_offset;
                row.close = 100.5 + price_offset;
                row.volume = 1_000 + index as u64;
                row
            })
            .collect()
    }

    fn app_with_chart() -> App {
        let mut app = test_app();
        app.holdings = Some(vec![
            holding("INFY", INFY, 100.0, 110.0, 105.0, 10),
            holding("RELIANCE", RELIANCE, 100.0, 105.0, 100.0, 5),
        ]);
        app.set_pane(PaneId::PortfolioHoldings);

        for token in [INFY, RELIANCE] {
            for interval in [ChartInterval::M5, ChartInterval::M15] {
                app.candles
                    .insert(SeriesKey::new(token, interval), candles(40));
            }
        }
        app.on_key(char_key('c'));
        app
    }

    fn draw(app: &App) {
        let mut terminal = Terminal::new(TestBackend::new(170, 40)).unwrap();
        terminal.draw(|frame| ui::draw(frame, app)).unwrap();
    }

    fn chart_hit(app: &App) -> ui::ChartHit {
        app.hit
            .borrow()
            .charts
            .last()
            .map(|entry| entry.hit.clone())
            .expect("the drawn chart records a hit")
    }

    fn slot_point(hit: &ui::ChartHit, slot: usize) -> (u16, u16) {
        (
            hit.plot.x
                + u16::try_from(hit.pad + slot * 2 + 1).expect("visible slot column fits u16"),
            hit.plot.y + hit.plot.height / 2,
        )
    }

    fn moved(column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Moved,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn clicked(column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn scrolled(column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn a_plot_recorded_by_another_instance_inspects_nothing_here() {
        let mut app = app_with_chart();
        draw(&app);
        let hit = chart_hit(&app);
        let (column, row) = slot_point(&hit, 3);

        app.hit.borrow_mut().charts[0].chart = super::ChartId::new(41);

        app.on_mouse(moved(column, row));
        assert_eq!(
            app.chart_pane.hover(),
            None,
            "the plot under the pointer is not this chart's"
        );

        app.on_mouse(clicked(column, row));
        assert_eq!(
            app.chart_pane.cursor(),
            None,
            "and a click on it commits nothing here"
        );
    }

    #[test]
    fn moving_over_a_candle_inspects_it_without_touching_the_keyboard_scan() {
        let mut app = app_with_chart();
        draw(&app);
        let hit = chart_hit(&app);
        let (column, row) = slot_point(&hit, 3);
        app.on_mouse(moved(column, row));

        let hover = app
            .chart_pane
            .hover()
            .cloned()
            .expect("the pointer inspects a bar");
        assert_eq!(hover.token, INFY);
        assert_eq!(hover.interval, ChartInterval::M5);
        assert_eq!(hover.ts, hit.visible_ts[3]);
        assert_eq!(hover.window_ts, hit.visible_ts[0]);
        assert_eq!(
            app.chart_pane.cursor(),
            None,
            "hover must not write the scan"
        );
    }

    #[test]
    fn repeated_movement_over_one_slot_cannot_recentre_the_chart() {
        let mut app = app_with_chart();
        draw(&app);
        let first = chart_hit(&app);
        let (column, row) = slot_point(&first, 2);

        for _ in 0..20 {
            app.on_mouse(moved(column, row));
            draw(&app);
            let hit = chart_hit(&app);
            assert_eq!(hit.start, first.start, "the window moved under the pointer");
            assert_eq!(hit.visible_ts, first.visible_ts);
        }
        assert_eq!(
            app.chart_pane.hover().map(|hover| hover.ts.clone()),
            Some(first.visible_ts[2].clone())
        );
    }

    #[test]
    fn leaving_the_candle_area_restores_the_keyboard_state() {
        let mut app = app_with_chart();
        app.chart_pane.set_cursor(Some(4));
        draw(&app);
        let hit = chart_hit(&app);
        let (column, row) = slot_point(&hit, 1);
        app.on_mouse(moved(column, row));
        assert!(app.chart_pane.hover().is_some());

        app.on_mouse(moved(hit.outer.x + hit.outer.width - 1, row));
        assert_eq!(app.chart_pane.hover(), None);
        assert_eq!(
            app.chart_pane.cursor(),
            Some(4),
            "the scan survives untouched"
        );
    }

    #[test]
    fn the_left_padding_and_the_other_panes_inspect_nothing() {
        let mut app = app_with_chart();

        app.candles
            .insert(SeriesKey::new(INFY, ChartInterval::M5), candles(5));
        draw(&app);
        let hit = chart_hit(&app);
        assert!(hit.pad > 0);
        let row = hit.plot.y + hit.plot.height / 2;
        for column in 0..u16::try_from(hit.pad).expect("plot padding fits u16") {
            app.on_mouse(moved(hit.plot.x + column, row));
            assert_eq!(
                app.chart_pane.hover(),
                None,
                "padding column {column} inspected"
            );
        }

        for row in [hit.plot.y - 1, hit.plot.y + hit.plot.height] {
            app.on_mouse(moved(
                hit.plot.x + u16::try_from(hit.pad).expect("plot padding fits u16") + 1,
                row,
            ));
            assert_eq!(app.chart_pane.hover(), None, "row {row} inspected");
        }
    }

    #[test]
    fn a_click_commits_the_pointed_candle_to_the_keyboard_cursor_once() {
        let mut app = app_with_chart();
        draw(&app);
        let hit = chart_hit(&app);
        let (column, row) = slot_point(&hit, 5);
        let clicked_ts = hit.visible_ts[5].clone();
        app.on_mouse(moved(column, row));
        app.on_mouse(clicked(column, row));

        assert_eq!(app.chart_pane.hover(), None, "a commit ends the inspection");
        let cached = app
            .candle_store()
            .candles(SeriesKey::new(INFY, ChartInterval::M5))
            .unwrap();
        let index = cached
            .iter()
            .position(|candle| crate::tui::data::candle_timestamp(candle) == clicked_ts)
            .unwrap();
        assert_eq!(app.chart_pane.cursor(), Some(cached.len() - 1 - index));

        draw(&app);
        let after = chart_hit(&app);
        draw(&app);
        assert_eq!(chart_hit(&app).start, after.start);
        assert_eq!(app.chart_pane.cursor(), Some(cached.len() - 1 - index));
    }

    #[tokio::test]
    async fn a_candle_click_focuses_the_chart_pane_that_drew_it() {
        use crate::tui::marketwatch::{State, WatchItem};
        use crate::tui::workspace::{PaneId, Preset};
        let mut app = test_app();
        app.set_watch_for_test(State::test_loaded(vec![WatchItem {
            tradingsymbol: "INFY".into(),
            exchange: "NSE".into(),
            instrument_token: INFY,
            ..Default::default()
        }]));
        for interval in [ChartInterval::M5, ChartInterval::M15] {
            app.candles
                .insert(SeriesKey::new(INFY, interval), candles(40));
        }
        app.set_workspace(Preset::Trading);
        app.focus_pane_for_test(PaneId::TradingWatch);
        let mut terminal = Terminal::new(TestBackend::new(160, 45)).unwrap();
        terminal.draw(|frame| ui::draw(frame, &app)).unwrap();
        let hit = app
            .hit
            .borrow()
            .charts
            .iter()
            .find(|entry| entry.chart == app.trading_chart.id())
            .map(|entry| entry.hit.clone())
            .expect("the Trading chart records its plot");
        let (column, row) = slot_point(&hit, 3);

        app.on_mouse(clicked(column, row));
        assert_eq!(app.focused_pane(), PaneId::TradingChart);
        assert!(
            app.trading_chart.cursor().is_some(),
            "the candle is committed"
        );
        terminal.draw(|frame| ui::draw(frame, &app)).unwrap();
        assert!(
            app.hit
                .borrow()
                .hints
                .iter()
                .any(|hint| hint.action == ui::HitAction::Key(char_key('i'))),
            "the footer advertises the chart's own keys"
        );
    }

    #[test]
    fn h_and_l_continue_from_the_pointed_candle_and_reach_live() {
        let mut app = app_with_chart();
        draw(&app);
        let hit = chart_hit(&app);
        let cached_len = app
            .candle_store()
            .candles(SeriesKey::new(INFY, ChartInterval::M5))
            .unwrap()
            .len();

        let newest = hit.visible_ts.len() - 1;
        let (column, row) = slot_point(&hit, newest - 2);
        app.on_mouse(moved(column, row));

        app.on_key(char_key('h'));
        assert_eq!(
            app.chart_pane.hover(),
            None,
            "a keypress ends the inspection"
        );
        assert_eq!(
            app.chart_pane.cursor(),
            Some(3),
            "h steps one older than the pointed bar"
        );
        assert!(cached_len > 4);

        for _ in 0..3 {
            app.on_key(char_key('l'));
        }
        assert_eq!(app.chart_pane.cursor(), Some(0));
        app.on_key(char_key('l'));
        assert_eq!(
            app.chart_pane.cursor(),
            None,
            "stepping past the newest is LIVE"
        );
    }

    #[test]
    fn every_other_key_just_ends_the_inspection() {
        for key in ['C', 'i', 'I', 'v', 'c', 'j'] {
            let mut app = app_with_chart();
            draw(&app);
            let hit = chart_hit(&app);
            let (column, row) = slot_point(&hit, 4);
            app.on_mouse(moved(column, row));
            assert!(app.chart_pane.hover().is_some());
            app.on_key(char_key(key));
            assert_eq!(
                app.chart_pane.hover(),
                None,
                "{key} must end the inspection"
            );
        }
    }

    #[test]
    fn a_resize_ends_the_inspection() {
        let mut app = app_with_chart();
        draw(&app);
        let hit = chart_hit(&app);
        let (column, row) = slot_point(&hit, 4);
        app.on_mouse(moved(column, row));
        app.on_event(AppEvent::Resize);
        assert_eq!(app.chart_pane.hover(), None);
    }

    #[test]
    fn a_hit_from_another_instrument_or_interval_is_rejected() {
        let mut app = app_with_chart();
        draw(&app);
        let hit = chart_hit(&app);
        let (column, row) = slot_point(&hit, 4);

        app.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        app.on_mouse(moved(column, row));
        assert_eq!(
            app.chart_pane.hover(),
            None,
            "a stale token must not inspect"
        );
        assert_eq!(app.chart_pane.cursor(), None);
        app.on_mouse(clicked(column, row));
        assert_eq!(
            app.chart_pane.cursor(),
            None,
            "nor must a stale token commit"
        );

        app.on_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        draw(&app);
        app.on_key(char_key('i'));
        assert_eq!(app.chart_pane.interval(), ChartInterval::M15);
        app.on_mouse(moved(column, row));
        assert_eq!(
            app.chart_pane.hover(),
            None,
            "a stale interval must not inspect"
        );
    }

    #[test]
    fn a_modal_blocks_both_inspection_and_commit() {
        let mut app = app_with_chart();
        draw(&app);
        let hit = chart_hit(&app);
        let (column, row) = slot_point(&hit, 4);
        app.on_mouse(moved(column, row));
        assert!(app.chart_pane.hover().is_some());

        app.modals.push(Modal::Funds);
        app.on_mouse(moved(column, row));
        assert_eq!(app.chart_pane.hover(), None);
        app.on_mouse(clicked(column, row));
        assert_eq!(app.chart_pane.cursor(), None);

        draw(&app);
        assert!(app.hit.borrow().charts.is_empty());
    }

    #[test]
    fn a_wheel_over_the_chart_leaves_the_selection_alone() {
        let mut app = app_with_chart();
        draw(&app);
        let hit = chart_hit(&app);
        assert_eq!(app.pane_row(PaneId::PortfolioHoldings), 0);
        for (column, row) in [
            slot_point(&hit, 4),
            (hit.outer.x, hit.outer.y),
            (
                hit.outer.x + hit.outer.width - 1,
                hit.outer.y + hit.outer.height - 1,
            ),
        ] {
            app.on_mouse(scrolled(column, row));
            assert_eq!(
                app.pane_row(PaneId::PortfolioHoldings),
                0,
                "a notch at ({column}, {row}) moved the trade target"
            );
        }

        app.on_mouse(scrolled(0, hit.outer.y + 4));
        assert_eq!(app.pane_row(PaneId::PortfolioHoldings), 1);
    }

    #[test]
    fn a_hovered_bar_survives_a_current_bucket_update_and_an_appended_one() {
        let mut app = app_with_chart();
        draw(&app);
        let hit = chart_hit(&app);
        let (column, row) = slot_point(&hit, 6);
        app.on_mouse(moved(column, row));
        let inspected = app.chart_pane.hover().cloned().unwrap();

        let key = SeriesKey::new(INFY, ChartInterval::M5);
        let series = app.candles.candles_mut(key).unwrap();
        let last = series.len() - 1;
        series[last].close += 5.0;
        series.push(crate::tui::data::test_models::candle_at(
            "2026-09-09 09:55:00",
            140.0,
            141.0,
            139.0,
            140.5,
            7,
        ));
        app.candles.touch(SeriesKey::new(INFY, ChartInterval::M5));

        draw(&app);
        let after = chart_hit(&app);
        app.on_mouse(moved(column, row));
        assert_eq!(
            app.chart_pane.hover().map(|hover| hover.ts.clone()),
            Some(inspected.ts.clone()),
            "the pointer still names the same bar"
        );
        assert_eq!(
            after.start, hit.start,
            "the anchored window does not slide when a bar appends"
        );
    }

    #[test]
    fn appending_to_a_short_history_leaves_the_inspected_bar_under_the_pointer() {
        let mut app = app_with_chart();
        app.candles
            .insert(SeriesKey::new(INFY, ChartInterval::M5), candles(3));
        app.candles.touch(SeriesKey::new(INFY, ChartInterval::M5));
        draw(&app);
        let hit = chart_hit(&app);
        assert!(hit.pad > 0, "a short history is padded on the left");
        let (column, row) = slot_point(&hit, 1);
        app.on_mouse(moved(column, row));
        let inspected = app
            .chart_pane
            .hover()
            .cloned()
            .expect("the pointer is on a bar");
        assert_eq!(inspected.ts, hit.visible_ts[1]);

        app.candles
            .insert(SeriesKey::new(INFY, ChartInterval::M5), candles(4));
        app.candles.touch(SeriesKey::new(INFY, ChartInterval::M5));
        draw(&app);
        let after = chart_hit(&app);
        assert_eq!(after.pad, hit.pad, "the padding is held for the inspection");
        assert_eq!(after.visible_ts, hit.visible_ts, "and so is the window");

        app.on_mouse(moved(column, row));
        assert_eq!(
            app.chart_pane.hover().map(|hover| hover.ts.clone()),
            Some(inspected.ts),
            "the glyph did not move out from under the pointer"
        );
    }

    #[test]
    fn an_evicted_anchor_ends_the_inspection_rather_than_naming_a_neighbour() {
        let mut app = app_with_chart();
        draw(&app);
        let hit = chart_hit(&app);
        let (column, row) = slot_point(&hit, 6);
        app.on_mouse(moved(column, row));
        let inspected = app.chart_pane.hover().cloned().unwrap();

        app.candles.insert(
            SeriesKey::new(INFY, ChartInterval::M5),
            (0..40)
                .map(|index| {
                    let mut row = crate::tui::data::test_models::candle();
                    row.date = format!(
                        "{}+05:30",
                        (format!("2026-09-10 09:{:02}:00", 15 + index)).replace(' ', "T")
                    )
                    .parse()
                    .expect("date");
                    row.open = 200.0;
                    row.high = 201.0;
                    row.low = 199.0;
                    row.close = 200.5;
                    row.volume = 1;
                    row
                })
                .collect(),
        );
        app.candles.touch(SeriesKey::new(INFY, ChartInterval::M5));
        draw(&app);

        let after = chart_hit(&app);
        assert!(!after.visible_ts.contains(&inspected.ts));
        app.on_mouse(moved(column, row));
        assert_eq!(
            app.chart_pane.hover().map(|hover| hover.ts.clone()),
            Some(after.visible_ts[6].clone()),
            "the next move inspects the bar that is actually drawn there"
        );

        let resumed_anchor = app.chart_pane.hover().cloned().expect("still inspecting");
        assert_eq!(
            resumed_anchor.window_ts, after.visible_ts[0],
            "the anchor names a bar the current series holds"
        );
        draw(&app);
        let resumed = chart_hit(&app);
        assert_eq!(
            resumed.visible_ts, after.visible_ts,
            "the anchored window is honoured rather than recentred"
        );
        let (column, row) = slot_point(&resumed, 8);
        app.on_mouse(moved(column, row));
        assert_eq!(
            app.chart_pane.hover().map(|hover| hover.ts.clone()),
            Some(resumed.visible_ts[8].clone()),
            "and the readout keeps following the pointer"
        );
    }

    #[test]
    fn no_candles_means_no_hit_and_no_inspection() {
        let mut app = app_with_chart();
        app.candles.remove(SeriesKey::new(INFY, ChartInterval::M5));
        draw(&app);
        assert!(app.hit.borrow().charts.is_empty());
        app.on_mouse(moved(120, 20));
        assert_eq!(app.chart_pane.hover(), None);
    }
}
