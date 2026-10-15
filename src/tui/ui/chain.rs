use crate::tui::app::App;
use crate::tui::format;
use crate::tui::instruments::OptionChain;
use crate::tui::ticks;
use crate::tui::ui;
use crate::tui::ui::columns::{self, FitCol, col, fit_constraints, fit_row, table_budget};
use crate::tui::ui::columns::{right, right_styled};
use crate::tui::ui::pane::{ChainInput, PaneHits};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, TableState};

fn fmt_expiry(iso: &str) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let mut parts = iso.split('-');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(_y), Some(m), Some(d)) => match (m.parse::<usize>(), d.parse::<u32>()) {
            (Ok(m), Ok(d)) if (1..=12).contains(&m) => format!("{d} {}", MONTHS[m - 1]),
            _ => iso.to_string(),
        },
        _ => iso.to_string(),
    }
}

fn cap_chg_pct(value: f64) -> String {
    if value >= 999.995 {
        ">999%".to_string()
    } else if value <= -999.995 {
        "<-999%".to_string()
    } else {
        format::percent(value)
    }
}

fn fmt_strike(strike: f64) -> String {
    if strike.fract().abs() < f64::EPSILON && strike >= 0.0 {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "nonnegative whole strike uses integer display formatting"
        )]
        let whole = strike as u64;
        format::group_indian(whole)
    } else {
        format::price(strike)
    }
}

pub fn draw_chain_leaf(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    pane: crate::tui::workspace::PaneId,
) {
    let Some(instance) = app.chain_pane_of(pane) else {
        return;
    };
    let input = ChainInput {
        chain: instance.chain(),
        selected: instance.display_row(),
        call_focus: instance.call_focus(),
        spot_token: instance.spot_token(),
        atm_strike: app.chain_atm_strike_of(pane),
        underlying: instance.underlying(),
    };
    draw_chain_pane(frame, area, app, &input, &PaneHits::new(&app.hit, pane));
}

fn draw_chain_pane(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    input: &ChainInput<'_>,
    hits: &PaneHits<'_>,
) {
    let Some(chain) = input.chain else {
        let (headline, hint) = if app.instruments.is_none() {
            ("Instruments not available yet", None)
        } else {
            ("No options available", Some("u Change underlying"))
        };
        draw_state(frame, area, headline, hint);
        return;
    };
    if chain.rows.is_empty() {
        draw_state(
            frame,
            area,
            &format!("No options for {}", input.underlying),
            Some("u Change underlying"),
        );
        return;
    }

    let [header_area, table_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
    draw_header(frame, header_area, app, chain, input);
    draw_table(frame, table_area, app, chain, input, hits);
}

fn draw_state(frame: &mut Frame, area: Rect, headline: &str, hint: Option<&str>) {
    let block = ui::panel(" Option chain ");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    ui::state_message(frame, inner, ui::MessageTone::Normal, headline, None, hint);
}

fn draw_header(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    chain: &OptionChain,
    input: &ChainInput<'_>,
) {
    let sep = || Span::styled(" · ", format::dim());
    let mut spans = vec![Span::styled(
        chain.underlying.clone(),
        format::bold().fg(format::accent()),
    )];

    spans.push(sep());
    match input.spot_token.and_then(|token| app.live_ltp(token)) {
        Some(spot) => {
            let change = input
                .spot_token
                .and_then(|token| app.tick(token))
                .and_then(ticks::change);
            let style = change.map(format::change_style).unwrap_or_default();
            spans.push(Span::styled(format::price(spot), style));
            spans.push(Span::styled(" spot", format::dim()));
        }
        None => spans.push(Span::styled(", spot", format::dim())),
    }

    spans.push(sep());
    match &chain.expiry {
        Some(exp) => spans.push(Span::raw(fmt_expiry(exp))),
        None => spans.push(Span::styled(".", format::dim())),
    }
    spans.push(Span::styled(" exp ", format::dim()));
    spans.push(Span::styled("([ ] change)", format::dim()));

    spans.push(sep());
    spans.push(Span::styled("u:cycle · /:underlying", format::dim()));

    if let Some(atm) = input.atm_strike {
        spans.push(sep());
        spans.push(Span::styled("ATM ", format::dim()));
        spans.push(Span::raw(fmt_strike(atm)));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

const CHAIN_COLS: [FitCol; 7] = [
    col(9, 3),  // CALL OI
    col(11, 1), // CALL LTP
    col(8, 5),  // CALL CHG%
    col(11, 0), // STRIKE
    col(8, 6),  // PUT CHG%
    col(11, 2), // PUT LTP
    col(9, 4),  // PUT OI
];

const STRIKE_COL: usize = 3;

fn draw_table(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    chain: &OptionChain,
    input: &ChainInput<'_>,
    hits: &PaneHits<'_>,
) {
    let selected = input.selected.min(chain.rows.len() - 1);
    let atm_row = input.atm_strike;
    let call_focus = input.call_focus;

    let fit = columns::fit_columns(table_budget(area.width), 1, &CHAIN_COLS);
    let layout = columns::spread(&fit, &CHAIN_COLS, &[]);

    let rows = chain_rows(app, chain, call_focus, atm_row, &fit);
    let title = format!(
        " {} option chain · {} strikes · {}CALLS {}PUTS ",
        chain.underlying,
        chain.rows.len(),
        ui::focus_marker(call_focus),
        ui::focus_marker(!call_focus),
    );
    let call_hdr = if call_focus { "LTP*" } else { "LTP" };
    let put_hdr = if call_focus { "LTP" } else { "LTP*" };
    let table = Table::new(rows, fit_constraints(&fit, &layout))
        .header(
            fit_row(
                &fit,
                &[
                    &|| right("CALL OI"),
                    &|| right(call_hdr.to_string()),
                    &|| right("CHG%"),
                    &|| center_styled("STRIKE".to_string(), format::header_style()),
                    &|| right("CHG%"),
                    &|| right(put_hdr.to_string()),
                    &|| right("PUT OI"),
                ],
            )
            .style(format::header_style()),
        )
        .column_spacing(layout.spacing)
        .block(ui::panel(title));

    let visible = area.height.saturating_sub(3) as usize; // borders + header row
    let mut state = TableState::default().with_selected(Some(selected));
    if visible > 0 && chain.rows.len() > visible {
        let half = visible / 2;
        let max_offset = chain.rows.len() - visible;
        *state.offset_mut() = selected.saturating_sub(half).min(max_offset);
    }

    frame.render_stateful_widget(
        table
            .row_highlight_style(format::selection_style())
            .highlight_symbol(layout.highlight_symbol()),
        area,
        &mut state,
    );
    record_row_hits(hits, area, &fit, &layout, state.offset());
}

fn record_row_hits(
    hits: &PaneHits<'_>,
    area: Rect,
    fit: &columns::ColumnFit,
    layout: &columns::TableLayout,
    offset: usize,
) {
    let rows = Rect {
        x: area.x,
        y: area.y.saturating_add(2),
        width: area.width,
        height: area.height.saturating_sub(3),
    };
    if rows.width == 0 || rows.height == 0 {
        return;
    }
    let mut x = area
        .x
        .saturating_add(1 + layout.inset + columns::SELECT_GUTTER);
    let mut strike = None;
    for idx in 0..CHAIN_COLS.len() {
        if !fit.shows(idx) {
            continue;
        }
        if idx == STRIKE_COL {
            strike = Some((x, layout.width(idx)));
        }
        x = x.saturating_add(layout.width(idx) + layout.spacing);
    }

    let band = |x: u16, width: u16, side: ui::ChainBand| {
        let x = x.max(rows.x).min(rows.right());
        let width = width.min(rows.right().saturating_sub(x));
        if width == 0 {
            return;
        }
        hits.under(ui::HitList::Chain {
            owner: hits.owner(),
            band: side,
        })
        .push_rows(
            Rect {
                x,
                y: rows.y,
                width,
                height: rows.height,
            },
            offset,
        );
    };

    let Some((strike_x, strike_w)) = strike else {
        band(rows.x, rows.width, ui::ChainBand::Strike);
        return;
    };
    band(rows.x, strike_x.saturating_sub(rows.x), ui::ChainBand::Call);
    band(strike_x, strike_w, ui::ChainBand::Strike);
    let put_x = strike_x.saturating_add(strike_w);
    band(
        put_x,
        rows.right().saturating_sub(put_x),
        ui::ChainBand::Put,
    );
}

fn center_styled(text: String, style: Style) -> Cell<'static> {
    Cell::from(Line::from(Span::styled(text, style)).alignment(Alignment::Center))
}

fn chain_rows(
    app: &App,
    chain: &OptionChain,
    call_focus: bool,
    atm_row: Option<f64>,
    fit: &columns::ColumnFit,
) -> Vec<Row<'static>> {
    let dash = || ".".to_string();
    let fmt_price = |value: Option<f64>| value.map_or_else(dash, format::price);
    let fmt_oi = |value: Option<u64>| {
        value
            .filter(|oi| *oi > 0)
            .map_or_else(dash, format::compact_qty)
    };
    let fmt_pct = |value: Option<f64>| value.map_or_else(dash, cap_chg_pct);

    chain
        .rows
        .iter()
        .map(|row| {
            let call_tick = row
                .call
                .as_ref()
                .and_then(|leg| app.tick(leg.instrument_token));
            let put_tick = row
                .put
                .as_ref()
                .and_then(|leg| app.tick(leg.instrument_token));
            let call_change = call_tick.and_then(ticks::change);
            let put_change = put_tick.and_then(ticks::change);
            let call_style = call_change.map(format::change_style).unwrap_or_default();
            let put_style = put_change.map(format::change_style).unwrap_or_default();

            let call_ltp_style = if call_focus {
                call_style.add_modifier(Modifier::UNDERLINED)
            } else {
                call_style
            };
            let put_ltp_style = if call_focus {
                put_style
            } else {
                put_style.add_modifier(Modifier::UNDERLINED)
            };

            let is_atm = atm_row.is_some_and(|atm| (atm - row.strike).abs() < f64::EPSILON);
            let strike_style = if is_atm {
                format::bold().fg(format::accent())
            } else {
                format::bold()
            };

            let mut widget_row = fit_row(
                fit,
                &[
                    &|| right(fmt_oi(call_tick.and_then(|t| t.oi))),
                    &|| {
                        right_styled(
                            fmt_price(call_tick.and_then(|t| t.last_price)),
                            call_ltp_style,
                        )
                    },
                    &|| {
                        right_styled(
                            fmt_pct(call_tick.and_then(ticks::change_percent)),
                            call_style,
                        )
                    },
                    &|| center_styled(fmt_strike(row.strike), strike_style),
                    &|| right_styled(fmt_pct(put_tick.and_then(ticks::change_percent)), put_style),
                    &|| {
                        right_styled(
                            fmt_price(put_tick.and_then(|t| t.last_price)),
                            put_ltp_style,
                        )
                    },
                    &|| right(fmt_oi(put_tick.and_then(|t| t.oi))),
                ],
            );
            if is_atm {
                widget_row = widget_row.style(Style::default().add_modifier(Modifier::BOLD));
            }
            widget_row
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{CHAIN_COLS, STRIKE_COL, cap_chg_pct, fmt_expiry, fmt_strike, table_budget};
    use crate::tui::instruments::OptionChain;
    use crate::tui::ui::columns;
    use crate::tui::workspace::PaneId;

    fn bare_app() -> crate::tui::app::App {
        use crate::tui::app::App;
        use crate::tui::notes::Pad;
        use crate::tui::settings::Settings;
        use kiteconnect::KiteConnect;

        let client = KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("tok").unwrap())
            .build()
            .expect("dummy client builds");
        let (events_tx, _events_rx) = tokio::sync::mpsc::channel(8);
        let mut app = App::new(
            client,
            events_tx,
            crate::tui::marketwatch::State::test_loaded(Vec::new()),
            Pad::default(),
            false,
            &Settings::default(),
        );
        app.set_pane(PaneId::DerivativesChain);
        app
    }

    fn chain_app() -> crate::tui::app::App {
        use crate::instruments::{OptionLeg, OptionRow};

        let mut app = bare_app();
        let leg = |token: u32| OptionLeg {
            instrument_token: token,
            tradingsymbol: "NIFTY24000CE".into(),
            exchange: "NFO".into(),
            lot_size: 50,
            tick_size: 0.05,
        };
        app.set_chain_for_test(OptionChain {
            underlying: "NIFTY".into(),
            expiries: vec!["2026-09-24".into()],
            expiry: Some("2026-09-24".into()),
            rows: (0..6)
                .map(|i| OptionRow {
                    strike: 24000.0 + f64::from(i) * 100.0,
                    call: Some(leg(1000 + i)),
                    put: Some(leg(2000 + i)),
                })
                .collect(),
        });
        app
    }

    fn rows(app: &crate::tui::app::App, width: u16, height: u16) -> Vec<String> {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| {
                super::draw_chain_leaf(frame, frame.area(), app, PaneId::DerivativesChain);
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        (0..height)
            .map(|y| (0..width).map(|x| buffer[(x, y)].symbol()).collect())
            .collect()
    }

    fn find_all(rows: &[String], needle: &str) -> Vec<(usize, usize)> {
        rows.iter()
            .enumerate()
            .filter_map(|(y, row)| {
                row.find(needle)
                    .map(|byte| (y, row[..byte].chars().count()))
            })
            .collect()
    }

    fn find_once(rows: &[String], needle: &str) -> (usize, usize) {
        let found = find_all(rows, needle);
        assert_eq!(
            found.len(),
            1,
            "{needle:?} is not on exactly one row of\n{}",
            rows.join("\n")
        );
        found[0]
    }

    fn press(app: &mut crate::tui::app::App, key: char) {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        app.on_event(crate::tui::event::AppEvent::Key(KeyEvent::new(
            KeyCode::Char(key),
            KeyModifiers::NONE,
        )));
    }

    #[test]
    fn focused_ltp_underlines_digits_without_padding() {
        use kiteconnect::ticker::Tick;
        use ratatui::{Terminal, backend::TestBackend, style::Modifier};

        for width in [80, 120] {
            let mut app = chain_app();
            for (token, price) in [(1000, 12.34), (2000, 56.78)] {
                let mut tick = Tick::default();
                tick.instrument_token = token;
                tick.last_price = Some(price);
                app.ticks.insert(token, tick);
            }
            for (key, price) in [('h', "12.34"), ('l', "56.78")] {
                press(&mut app, key);
                let mut terminal = Terminal::new(TestBackend::new(width, 20)).unwrap();
                terminal
                    .draw(|frame| {
                        super::draw_chain_leaf(frame, frame.area(), &app, PaneId::DerivativesChain);
                    })
                    .unwrap();
                let buffer = terminal.backend().buffer();
                let rendered: Vec<String> = (0..20)
                    .map(|y| (0..width).map(|x| buffer[(x, y)].symbol()).collect())
                    .collect();
                let (y, x) = find_once(&rendered, price);
                for column in x..x + 5 {
                    assert!(
                        buffer[(u16::try_from(column).unwrap(), u16::try_from(y).unwrap())]
                            .modifier
                            .contains(Modifier::UNDERLINED)
                    );
                }
                for column in x - 6..x {
                    let cell = &buffer[(u16::try_from(column).unwrap(), u16::try_from(y).unwrap())];
                    assert_eq!(cell.symbol(), " ");
                    assert!(
                        !cell.modifier.contains(Modifier::UNDERLINED),
                        "{price} padding at ({column}, {y}) has {:?} at width {width}",
                        cell.modifier
                    );
                }
            }
        }
    }

    #[test]
    fn the_chain_title_marks_the_leg_h_and_l_chose() {
        for (width, height) in [(80u16, 24u16), (120, 35)] {
            let mut app = chain_app();
            assert!(
                app.derivatives_chain.call_focus(),
                "calls are the initial leg"
            );
            let calls = rows(&app, width, height);
            find_once(&calls, "> CALLS   PUTS ");

            press(&mut app, 'l');
            assert!(!app.derivatives_chain.call_focus());
            let puts = rows(&app, width, height);
            find_once(&puts, "  CALLS > PUTS ");

            press(&mut app, 'h');
            assert!(app.derivatives_chain.call_focus());
            assert_eq!(rows(&app, width, height), calls, "h returns to the calls");

            for label in ["CALLS", "PUTS", "STRIKE"] {
                assert_eq!(
                    find_all(&calls, label),
                    find_all(&puts, label),
                    "{label} moved at {width}x{height}"
                );
            }
            let markers: usize = calls.iter().map(|row| row.matches("> ").count()).sum();
            assert_eq!(markers, 1, "one marker at {width}x{height}: {calls:#?}");
        }
    }

    #[test]
    fn the_chain_states_are_told_apart_by_the_instrument_master() {
        let mut app = bare_app();
        let unknown = rows(&app, 80, 24).join("\n");
        assert!(
            unknown.contains("Instruments not available yet"),
            "{unknown}"
        );
        assert!(
            !unknown.contains("r Refresh"),
            "`r` does not re-poll the chain: {unknown}"
        );

        app.instruments = Some(std::sync::Arc::new(
            crate::instruments::InstrumentStore::new(Vec::new()),
        ));
        let empty = rows(&app, 80, 24).join("\n");
        assert!(empty.contains("No options available"), "{empty}");
        assert!(empty.contains("u Change underlying"), "{empty}");

        let mut no_strikes = chain_app();
        no_strikes.set_chain_for_test(OptionChain {
            underlying: "NIFTY".into(),
            expiries: Vec::new(),
            expiry: None,
            rows: Vec::new(),
        });
        let none = rows(&no_strikes, 80, 24).join("\n");
        assert!(none.contains("No options for"), "{none}");
        assert!(none.contains("u Change underlying"), "{none}");
    }

    #[test]
    fn the_recorded_bands_line_up_with_the_rendered_strike_column() {
        use crate::tui::ui::{ChainBand, HitAction, HitList};

        for width in [80u16, 100, 140] {
            let app = chain_app();
            let mut terminal =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, 20)).unwrap();
            terminal
                .draw(|frame| crate::tui::ui::draw(frame, &app))
                .unwrap();
            let buffer = terminal.backend().buffer().clone();

            let band = |want: ChainBand| {
                let hit = app.hit.borrow();
                hit.regions
                    .iter()
                    .find(|region| {
                        matches!(
                            region.action,
                            HitAction::Rows {
                                list: HitList::Chain { band: got, .. },
                                ..
                            } if got == want
                        )
                    })
                    .expect("chain records every band")
                    .rect
            };
            let calls = band(ChainBand::Call);
            let strike = band(ChainBand::Strike);
            let puts = band(ChainBand::Put);
            assert_eq!(calls.right(), strike.x, "width {width}");
            assert_eq!(strike.right(), puts.x, "width {width}");
            assert_eq!(strike.width, CHAIN_COLS[STRIKE_COL].width, "width {width}");

            let header: String = (0..width)
                .map(|x| buffer[(x, strike.y - 1)].symbol())
                .collect();
            let label = header.find("STRIKE").unwrap_or_else(|| {
                panic!("width {width}: no STRIKE header in {header:?}");
            });
            let label = u16::try_from(header[..label].chars().count()).unwrap();
            assert!(
                label >= strike.x && label + 6 <= strike.right(),
                "width {width}: STRIKE at {label} is outside the recorded band {strike:?}"
            );
        }
    }

    #[test]
    fn chain_columns_fit_at_full_width_at_every_terminal_width() {
        for area_width in 10..=200u16 {
            let budget = table_budget(area_width);
            let fit = columns::fit_columns(budget, 1, &CHAIN_COLS);
            let mut used = 0u16;
            let mut count = 0u16;
            for (idx, column) in CHAIN_COLS.iter().enumerate() {
                if fit.shows(idx) {
                    used += column.width;
                    count += 1;
                }
            }
            used += count.saturating_sub(1); // 1-cell column spacing
            assert!(
                used <= budget || count == 1,
                "width {area_width}: columns render {used} in {budget}"
            );
            assert!(fit.shows(3), "width {area_width} dropped the STRIKE column");
        }
    }

    #[test]
    fn chain_sheds_columns_from_the_outside_in() {
        let shown = |area_width: u16| -> Vec<usize> {
            let fit = columns::fit_columns(table_budget(area_width), 1, &CHAIN_COLS);
            (0..CHAIN_COLS.len())
                .filter(|idx| fit.shows(*idx))
                .collect()
        };
        assert_eq!(shown(80), vec![0, 1, 2, 3, 4, 5, 6]);
        assert_eq!(shown(70), vec![0, 1, 2, 3, 5, 6], "PUT CHG% goes first");
        assert_eq!(shown(60), vec![0, 1, 3, 5, 6], "then CALL CHG%");
        assert_eq!(shown(50), vec![0, 1, 3, 5], "then PUT OI");
        assert_eq!(shown(30), vec![1, 3], "a strike and its call LTP survive");
    }

    #[test]
    fn caps_huge_chg_pct_within_column_width() {
        assert_eq!(cap_chg_pct(9900.0), ">999%");
        assert_eq!(cap_chg_pct(1000.0), ">999%");
        assert_eq!(cap_chg_pct(-9900.0), "<-999%");
        assert_eq!(cap_chg_pct(-1000.0), "<-999%");
        for value in [9900.0, 1000.0, -9900.0, -1000.0] {
            assert!(cap_chg_pct(value).chars().count() <= 8);
        }
        assert_eq!(cap_chg_pct(12.34), "+12.34%");
        assert_eq!(cap_chg_pct(-5.0), "-5.00%");
        assert_eq!(cap_chg_pct(999.99), "+999.99%");
        assert_eq!(cap_chg_pct(999.996), ">999%");
        assert_eq!(cap_chg_pct(-999.996), "<-999%");
        assert_eq!(cap_chg_pct(999.994), "+999.99%");
    }

    #[test]
    fn formats_expiry_compactly() {
        assert_eq!(fmt_expiry("2026-07-14"), "14 Jul");
        assert_eq!(fmt_expiry("2026-01-01"), "1 Jan");
        assert_eq!(fmt_expiry("2026-12-31"), "31 Dec");
        assert_eq!(fmt_expiry("garbage"), "garbage");
        assert_eq!(fmt_expiry("2026-13-01"), "2026-13-01");
    }

    #[test]
    fn formats_strike() {
        assert_eq!(fmt_strike(24400.0), "24,400");
        assert_eq!(fmt_strike(24400.5), "24,400.50");
    }
}
