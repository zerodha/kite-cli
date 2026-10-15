use crate::tui::format;
use crate::tui::ui;
use ratatui::widgets::Block;

mod account;
mod books;
#[cfg(test)]
mod fixtures;
mod indices;
mod kpis;
mod movers;

pub(super) use account::draw_account;
pub(super) use books::{draw_holdings_panel, draw_positions_panel};
pub(super) use indices::{index_lines, render_index_panel};
pub(super) use kpis::draw_kpis;
pub(super) use movers::draw_movers;

fn signed_inr(value: f64) -> String {
    if value >= 0.0 {
        format!("+{}", format::inr(value))
    } else {
        format::inr(value)
    }
}

fn tile(title: &str) -> Block<'static> {
    ui::panel(format!(" {title} "))
}

#[cfg(test)]
mod tests {
    use super::fixtures::{book_app, buffer_rows, find_all, find_once, root_buffer};
    use crate::tui::app::App;

    use crate::tui::workspace::PaneId;

    const FRAMES: [(u16, u16, bool); 3] = [(80, 24, true), (120, 35, false), (160, 45, false)];

    fn overview_frame_app(pane: PaneId) -> App {
        let mut app = book_app(pane);
        app.orders = Some(Vec::new());
        app.gtts = Some(Vec::new());
        app
    }

    #[test]
    fn the_overview_draws_both_books_and_the_account_counts_at_every_size() {
        for (width, height, _) in FRAMES {
            let app = overview_frame_app(PaneId::OverviewPositions);
            let rows = buffer_rows(&root_buffer(&app, width, height));
            let frame = rows.join("\n");
            find_once(&rows, "> Positions (2)");
            find_once(&rows, "   Holdings (3)");
            assert!(
                frame.contains("Positions 2 (2 open)") && frame.contains("Holdings 3"),
                "account counts at {width}x{height}:\n{frame}"
            );
            assert!(frame.contains("P0") && frame.contains("H0"), "{frame}");
            let account_y = rows
                .iter()
                .position(|row| row.contains("Positions 2 (2 open)"))
                .expect("the account band drew");
            for stale in ["IST", "open\u{a0}"] {
                assert!(
                    !rows[account_y].contains(stale),
                    "the account band repeats {stale:?} at {width}x{height}: {:?}",
                    rows[account_y]
                );
            }
        }
    }

    #[test]
    fn the_compact_bands_drop_their_borders_and_give_main_the_rows() {
        let app = overview_frame_app(PaneId::OverviewPositions);
        let compact = buffer_rows(&root_buffer(&app, 80, 24));
        let account = &compact[compact.len() - 2];
        assert!(account.contains("Positions 2 (2 open)"), "{account:?}");
        assert!(
            !compact.iter().any(|row| row.contains("Account")),
            "the compact band drops its panel title:\n{}",
            compact.join("\n")
        );

        let normal = buffer_rows(&root_buffer(&app, 80, 30));
        assert!(
            normal.iter().any(|row| row.contains("Account")),
            "the normal band keeps its panel:\n{}",
            normal.join("\n")
        );
    }

    #[test]
    fn the_overview_keeps_both_books_at_eighty_by_twentyfour_under_a_toast() {
        for text in [
            "",
            "order rejected: insufficient funds for this margin block, and then some more detail that wraps across three whole rows of an eighty column terminal",
        ] {
            let mut app = overview_frame_app(PaneId::OverviewPositions);
            if !text.is_empty() {
                app.toast(crate::tui::app::ToastLevel::Error, text.to_string());
            }
            let rows = buffer_rows(&root_buffer(&app, 80, 24));
            let frame = rows.join("\n");
            find_once(&rows, "> Positions (2)");
            find_once(&rows, "   Holdings (3)");
            assert!(
                frame.contains("P0") && frame.contains("H0"),
                "toast {:?} starved a book of records:\n{frame}",
                &text[..text.len().min(20)]
            );
        }
    }

    #[test]
    fn the_compact_kpi_band_shows_every_headline_and_no_subtitle() {
        let mut app = overview_frame_app(PaneId::OverviewPositions);
        app.margin_available = Some(450_000.0);
        let rows = buffer_rows(&root_buffer(&app, 120, 24));
        let kpi = rows[1..4].join("\n");
        for headline in ["\u{20b9}330", "+\u{20b9}30", "\u{20b9}4,50,000"] {
            assert!(kpi.contains(headline), "{headline} missing from\n{kpi}");
        }
        assert!(!kpi.contains("invested"), "no room for a subtitle:\n{kpi}");

        let tall = buffer_rows(&root_buffer(&app, 120, 35));
        assert!(tall[1..6].join("\n").contains("invested"));
    }

    #[test]
    fn the_fallback_draws_only_the_focused_book() {
        for (width, height) in [(80u16, 17u16), (60, 35)] {
            for (pane, shown, hidden) in [
                (PaneId::OverviewPositions, "> Positions (2)", "Holdings ("),
                (PaneId::OverviewHoldings, "> Holdings (3)", "Positions ("),
            ] {
                let app = overview_frame_app(pane);
                let rows = buffer_rows(&root_buffer(&app, width, height));
                find_once(&rows, shown);
                assert!(
                    find_all(&rows, hidden).is_empty(),
                    "{hidden} drew at {width}x{height}:\n{}",
                    rows.join("\n")
                );
                let frame = rows.join("\n");
                assert!(!frame.contains("Movers"), "{frame}");
                assert!(!frame.contains("Indices"), "{frame}");
                assert!(
                    frame.contains("Pos ") || frame.contains("Positions 2"),
                    "{frame}"
                );
            }
        }
    }

    #[test]
    fn the_overview_survives_a_frame_too_small_to_be_useful() {
        for (width, height) in [(1u16, 1u16), (2, 3), (10, 5), (40, 8), (69, 12)] {
            for pane in [PaneId::OverviewPositions, PaneId::OverviewHoldings] {
                let buffer = root_buffer(&overview_frame_app(pane), width, height);
                assert_eq!(buffer.area.width, width);
                assert_eq!(buffer.area.height, height);
            }
        }
    }

    #[test]
    fn the_compact_overview_renders_in_a_light_and_a_dark_preset() {
        for name in ["kite", "solarized-light"] {
            let theme = crate::tui::settings::preset(name).expect("preset exists");
            let _guard = crate::tui::settings::theme_guard(theme);
            for (width, height, _) in FRAMES {
                let buffer =
                    root_buffer(&overview_frame_app(PaneId::OverviewHoldings), width, height);
                let rows = buffer_rows(&buffer);
                let (focused_y, _) = find_once(&rows, "> Holdings (3)");
                let (inactive_y, _) = find_once(&rows, "   Positions (2)");
                assert_eq!(
                    buffer[(0, focused_y)].fg,
                    theme.accent,
                    "{name} at {width}x{height}: focused border"
                );
                assert_eq!(
                    buffer[(0, inactive_y)].fg,
                    theme.border,
                    "{name} at {width}x{height}: inactive border"
                );
                assert!(
                    rows.join("\n").contains("Holdings 3"),
                    "{name} at {width}x{height}: account counts"
                );
            }
        }
    }

    #[test]
    fn empty_books_render_at_every_size_and_preset() {
        for name in ["kite", "solarized-light"] {
            let _guard =
                crate::tui::settings::theme_guard(crate::tui::settings::preset(name).unwrap());
            for (width, height, _) in FRAMES {
                let mut app = overview_frame_app(PaneId::OverviewPositions);
                app.positions = Some({
                    let mut row = crate::tui::data::test_models::positions();
                    row.net = Vec::new();
                    row
                });
                app.holdings = Some(Vec::new());
                let rows = buffer_rows(&root_buffer(&app, width, height));
                let frame = rows.join("\n");
                find_once(&rows, "> Positions (0)");
                find_once(&rows, "   Holdings (0)");
                assert!(frame.contains("No positions"), "{name}:\n{frame}");
                assert!(frame.contains("No holdings"), "{name}:\n{frame}");
                assert!(
                    frame.contains("Positions 0 (0 open)") || frame.contains("Pos 0/0"),
                    "{name} at {width}x{height}:\n{frame}"
                );
            }
        }
    }
}
