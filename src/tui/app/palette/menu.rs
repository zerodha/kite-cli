use super::super::{App, ExecItem, PaletteAction, PaletteRow};
use crate::tui::palette::Jump;
use crate::tui::pane_state::ChartBinding;
use crate::tui::workspace::{PaneId, Preset};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    ToggleChart,
    CycleInterval,
    ToggleVolume,
    CycleIndicator,
    CycleMoversUniverse,

    NewAlert,

    AlertHistory,
    NewSearch,
    ManageWatchlists,
    SwitchTheme,
    RefreshAll,
    Help,
    Notes,
    SettingsPath,
    Logout,
    Quit,
}

impl MenuAction {
    pub fn label(self) -> &'static str {
        match self {
            MenuAction::ToggleChart => "Toggle chart",
            MenuAction::CycleInterval => "Cycle chart interval",
            MenuAction::ToggleVolume => "Toggle chart volume",
            MenuAction::CycleIndicator => "Cycle chart indicator",
            MenuAction::CycleMoversUniverse => "Cycle movers universe",
            MenuAction::NewAlert => "New alert…",
            MenuAction::AlertHistory => "Alert trigger history…",
            MenuAction::NewSearch => "Search instrument…",
            MenuAction::ManageWatchlists => "Create or switch watchlist…",
            MenuAction::SwitchTheme => "Switch theme…",
            MenuAction::RefreshAll => "Refresh",
            MenuAction::Help => "Help",
            MenuAction::Notes => "Notes…",
            MenuAction::SettingsPath => "Settings file path",
            MenuAction::Logout => "Logout",
            MenuAction::Quit => "Quit",
        }
    }
}

impl App {
    pub(in crate::tui::app) fn menu_rows(pane: PaneId, hidden: &[PaneId]) -> Vec<PaletteRow> {
        const KEYS: [&str; Preset::COUNT] = ["1", "2", "3", "4", "5", "6"];
        let item = |item, key| PaletteRow::Item {
            action: PaletteAction::Execute {
                item: Box::new(item),

                completion: None,
            },
            key,
        };

        let watch = pane == PaneId::TradingWatch;
        let chartable = ChartBinding::of_book(pane).is_some();
        let chart_toggle_key = (chartable && pane != PaneId::PortfolioPositions).then_some("c");
        let mut rows = vec![
            PaletteRow::Header("Suggested"),
            item(ExecItem::Action(MenuAction::RefreshAll), Some("r")),
            item(
                ExecItem::Action(MenuAction::NewSearch),
                watch.then_some("/"),
            ),
            item(
                ExecItem::Action(MenuAction::NewAlert),
                (pane == PaneId::OrdersAlerts).then_some("a"),
            ),
            item(ExecItem::Action(MenuAction::SwitchTheme), Some("T")),
            item(ExecItem::Action(MenuAction::Notes), Some("n")),
            item(ExecItem::Action(MenuAction::Help), Some("?")),
            PaletteRow::Header("Switch workspace"),
        ];

        for (preset, key) in Preset::ALL.into_iter().zip(KEYS) {
            rows.push(item(ExecItem::Jump(Jump::Workspace(preset)), Some(key)));
        }

        rows.push(PaletteRow::Header("Panes"));
        for pane in pane.preset().panes().iter().copied() {
            rows.push(item(
                ExecItem::FocusPane(pane),
                hidden.contains(&pane).then_some("hidden"),
            ));
        }
        rows.push(PaletteRow::Header("View"));
        rows.push(item(
            ExecItem::Action(MenuAction::ToggleChart),
            chart_toggle_key,
        ));
        let charted = chartable || matches!(pane, PaneId::TradingChart | PaneId::DerivativesChart);
        rows.push(item(
            ExecItem::Action(MenuAction::CycleInterval),
            charted.then_some("i"),
        ));
        rows.push(item(
            ExecItem::Action(MenuAction::ToggleVolume),
            charted.then_some("v"),
        ));

        if charted {
            rows.push(item(
                ExecItem::Action(MenuAction::CycleIndicator),
                Some("I"),
            ));
        }
        if pane.preset() == Preset::Overview {
            rows.push(item(
                ExecItem::Action(MenuAction::CycleMoversUniverse),
                Some("u"),
            ));
        }

        if pane == PaneId::OrdersAlerts {
            rows.push(item(
                ExecItem::Action(MenuAction::AlertHistory),
                Some("Enter"),
            ));
        }
        if watch {
            rows.push(item(
                ExecItem::Action(MenuAction::ManageWatchlists),
                Some("w"),
            ));
        }
        rows.push(PaletteRow::Header("App"));

        rows.push(item(ExecItem::Action(MenuAction::SettingsPath), None));
        rows.push(item(ExecItem::Action(MenuAction::Logout), None));
        rows.push(item(ExecItem::Action(MenuAction::Quit), Some("q")));
        rows
    }

    pub(in crate::tui::app) fn menu_exec_items(pane: PaneId, hidden: &[PaneId]) -> Vec<ExecItem> {
        Self::menu_rows(pane, hidden)
            .into_iter()
            .filter_map(|row| row.item().cloned())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::{App, ExecItem, MenuAction, PaletteRow};
    use crate::tui::marketwatch::State as WatchState;
    use crate::tui::notes::Pad;
    use crate::tui::palette::Jump;
    use crate::tui::settings::Settings;
    use crate::tui::workspace::{PaneId, Preset};
    use kiteconnect::KiteConnect;

    fn exec_items(app: &App, input: &str) -> Vec<ExecItem> {
        app.palette_build(input)
            .rows
            .iter()
            .filter_map(|row| row.item().cloned())
            .collect()
    }

    #[test]
    fn menu_rows_are_grouped_and_navigable() {
        let rows = App::menu_rows(PaneId::TradingWatch, &[]);
        assert!(matches!(
            rows.first(),
            Some(PaletteRow::Header("Suggested"))
        ));
        assert!(
            !rows
                .iter()
                .any(|row| matches!(row.item(), Some(ExecItem::Theme(_)))),
            "themes must not appear in the palette menu"
        );

        let at = rows
            .iter()
            .position(|row| matches!(row, PaletteRow::Header("Switch workspace")))
            .expect("the menu has a Switch workspace group");
        let jumps: Vec<(Preset, Option<&'static str>)> = rows[at + 1..]
            .iter()
            .map_while(|row| match (row.item(), row) {
                (Some(ExecItem::Jump(Jump::Workspace(preset))), PaletteRow::Item { key, .. }) => {
                    Some((*preset, *key))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            jumps,
            [
                (Preset::Overview, Some("1")),
                (Preset::Trading, Some("2")),
                (Preset::Portfolio, Some("3")),
                (Preset::Derivatives, Some("4")),
                (Preset::Orders, Some("5")),
                (Preset::Invest, Some("6")),
            ]
        );

        let trading = App::menu_rows(PaneId::TradingWatch, &[PaneId::TradingDepth]);
        let panes: Vec<&'static str> = trading
            .iter()
            .filter_map(|row| match row.item() {
                Some(ExecItem::FocusPane(pane)) => Some(pane.title()),
                _ => None,
            })
            .collect();
        assert_eq!(panes, ["Watch", "Chart", "Depth", "Orders"]);

        let hidden = trading.iter().find_map(|row| match (row.item(), row) {
            (Some(ExecItem::FocusPane(PaneId::TradingDepth)), PaletteRow::Item { key, .. }) => {
                Some(*key)
            }
            _ => None,
        });
        assert_eq!(hidden, Some(Some("hidden")));

        for (i, row) in rows.iter().enumerate() {
            if matches!(row, PaletteRow::Header(_)) {
                assert!(
                    matches!(rows.get(i + 1), Some(PaletteRow::Item { .. })),
                    "header at {i} has no item after it"
                );
            }
        }
    }

    #[test]
    fn menu_rows_show_chart_keys_only_on_chartable_panes() {
        fn cycle_interval_key(rows: &[PaletteRow]) -> Option<&'static str> {
            rows.iter()
                .find_map(|row| match row {
                    PaletteRow::Item { key, .. }
                        if matches!(
                            row.item(),
                            Some(ExecItem::Action(MenuAction::CycleInterval))
                        ) =>
                    {
                        Some(*key)
                    }
                    _ => None,
                })
                .flatten()
        }
        for pane in [
            PaneId::TradingChart,
            PaneId::DerivativesChart,
            PaneId::PortfolioPositions,
            PaneId::OrdersBook,
        ] {
            assert_eq!(
                cycle_interval_key(&App::menu_rows(pane, &[])),
                Some("i"),
                "{pane:?}"
            );
        }
        assert_eq!(
            cycle_interval_key(&App::menu_rows(PaneId::InvestMf, &[])),
            None
        );
    }

    #[test]
    fn watch_menu_offers_watchlist_management() {
        let watch_rows = App::menu_rows(PaneId::TradingWatch, &[]);
        let manage = |row: &PaletteRow| {
            matches!(
                row.item(),
                Some(ExecItem::Action(MenuAction::ManageWatchlists))
            )
        };
        assert!(
            watch_rows
                .iter()
                .any(|row| manage(row) && matches!(row, PaletteRow::Item { key: Some("w"), .. }))
        );
        for pane in Preset::ALL
            .into_iter()
            .flat_map(Preset::panes)
            .copied()
            .filter(|pane| *pane != PaneId::TradingWatch)
        {
            assert!(!App::menu_rows(pane, &[]).iter().any(manage), "{pane:?}");
        }
    }

    #[test]
    fn the_indicator_row_is_offered_only_where_a_chart_can_show_it() {
        #[expect(
            clippy::option_option,
            reason = "distinguishes an absent row from a row without a shortcut"
        )]
        fn indicator_key(rows: &[PaletteRow]) -> Option<Option<&'static str>> {
            rows.iter().find_map(|row| match row {
                PaletteRow::Item { key, .. }
                    if matches!(
                        row.item(),
                        Some(ExecItem::Action(MenuAction::CycleIndicator))
                    ) =>
                {
                    Some(*key)
                }
                _ => None,
            })
        }
        for pane in [
            PaneId::TradingChart,
            PaneId::DerivativesChart,
            PaneId::TradingOrders,
            PaneId::OrdersBook,
            PaneId::PortfolioPositions,
            PaneId::PortfolioHoldings,
        ] {
            assert_eq!(
                indicator_key(&App::menu_rows(pane, &[])),
                Some(Some("I")),
                "{pane:?} must advertise I"
            );
        }
        for pane in [
            PaneId::OverviewPositions,
            PaneId::TradingWatch,
            PaneId::InvestMf,
            PaneId::OrdersGtt,
            PaneId::DerivativesChain,
            PaneId::InvestBids,
        ] {
            assert_eq!(
                indicator_key(&App::menu_rows(pane, &[])),
                None,
                "{pane:?} has no chart, so the row must not be offered at all"
            );
        }
    }

    #[test]
    fn the_movers_universe_row_is_offered_only_on_the_overview() {
        #[expect(
            clippy::option_option,
            reason = "distinguishes an absent row from a row without a shortcut"
        )]
        fn movers_key(rows: &[PaletteRow]) -> Option<Option<&'static str>> {
            rows.iter().find_map(|row| match row {
                PaletteRow::Item { key, .. }
                    if matches!(
                        row.item(),
                        Some(ExecItem::Action(MenuAction::CycleMoversUniverse))
                    ) =>
                {
                    Some(*key)
                }
                _ => None,
            })
        }
        for pane in Preset::ALL.into_iter().flat_map(Preset::panes).copied() {
            let expected = (pane.preset() == Preset::Overview).then_some(Some("u"));
            assert_eq!(movers_key(&App::menu_rows(pane, &[])), expected, "{pane:?}");
        }
    }

    #[test]
    fn typing_a_gated_action_name_does_not_offer_it_from_the_wrong_view() {
        let (events_tx, _events_rx) = tokio::sync::mpsc::channel(8);
        let client = KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("token").unwrap())
            .build()
            .expect("dummy client builds");
        let mut app = App::new(
            client,
            events_tx,
            WatchState::test_loaded(Vec::new()),
            Pad::default(),
            true,
            &Settings::default(),
        );

        app.set_pane(PaneId::TradingChart);
        assert!(
            exec_items(&app, "indicator")
                .iter()
                .any(|item| matches!(item, ExecItem::Action(MenuAction::CycleIndicator))),
            "the Chart pane has a chart, so the indicator action is offered"
        );
        app.set_pane(PaneId::InvestMf);
        assert!(
            !exec_items(&app, "indicator")
                .iter()
                .any(|item| matches!(item, ExecItem::Action(MenuAction::CycleIndicator))),
            "no chart here, so typing its name must not offer it either"
        );

        app.set_workspace(Preset::Overview);
        assert!(
            exec_items(&app, "movers")
                .iter()
                .any(|item| matches!(item, ExecItem::Action(MenuAction::CycleMoversUniverse))),
        );
        app.set_pane(PaneId::OrdersBook);
        assert!(
            !exec_items(&app, "movers")
                .iter()
                .any(|item| matches!(item, ExecItem::Action(MenuAction::CycleMoversUniverse))),
            "no movers panel here, so typing its name must not offer it either"
        );
    }
}
