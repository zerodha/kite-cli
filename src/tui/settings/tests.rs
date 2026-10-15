use super::{Settings, from_json, parse_color};
use crate::tui::data::ChartInterval;
use ratatui::style::Color;
use serde_json::json;
use std::time::Duration;

#[test]
fn selected_watch_is_keyed_by_account() {
    let settings = from_json(r#"{"selected_watch":{"AB1234":"W7","CD5678":""}}"#);
    assert_eq!(settings.selected_watch("AB1234"), Some("W7"));
    assert_eq!(settings.selected_watch("CD5678"), None, "blank is unset");
    assert_eq!(settings.selected_watch("ZZ0000"), None);
    assert_eq!(from_json("{}").selected_watch("AB1234"), None);
}

#[test]
fn empty_and_garbage_yield_defaults() {
    for raw in ["{}", "not json at all", ""] {
        let settings = from_json(raw);
        assert_eq!(settings.theme(), super::preset("kite").unwrap());
        assert_eq!(settings.chart_interval(), None);
        assert_eq!(settings.default_product_index(), None);
        let polls = settings.poll_intervals();
        assert_eq!(polls.orders, Duration::from_secs(5));
        assert_eq!(polls.holdings, Duration::from_secs(30));
        assert_eq!(polls.gtt, Duration::from_secs(60));
        assert_eq!(polls.alerts, Duration::from_secs(60));
    }
}

#[test]
fn an_invalid_split_entry_drops_only_itself() {
    use crate::tui::split::Split;
    let settings = from_json(
        r#"{"default_product":"MIS","theme":"dracula",
            "splits":{"trading_root":"bad","chart_right":0.4}}"#,
    );
    assert_eq!(settings.theme(), super::preset("dracula").unwrap());
    assert_eq!(
        settings.default_product_index(),
        Some(super::PRODUCTS.iter().position(|p| *p == "MIS").unwrap()),
        "a bad layout value must not change an order default"
    );
    let ratios = settings.split_ratios();
    assert_eq!(ratios.get(Split::ChartRight), Some(0.4));
    assert_eq!(
        ratios.get(Split::TradingRoot),
        None,
        "the entry that is not a number is the only thing dropped"
    );

    assert!(from_json(r#"{"splits":null}"#).split_ratios().is_default());
    assert!(
        from_json(r#"{"splits":{"trading_root":[]}}"#)
            .split_ratios()
            .is_default()
    );
}

pub(super) fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("kite-settings-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir.join("tui.json")
}

#[test]
fn a_malformed_settings_file_is_reported_not_replaced() {
    let path = scratch("malformed");
    let original = r#"{"theme":"dracula","default_product":"MIS",}"#;
    std::fs::write(&path, original).expect("write");

    let err = super::save_selected_watch(&path, "AB1234", "W7")
        .expect_err("a document that cannot be read cannot be written back");
    assert!(err.contains("not valid JSON"), "{err}");
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        original,
        "the file is left exactly as the user wrote it"
    );

    std::fs::write(&path, "[1,2,3]").expect("write");
    let err = super::save_theme_preset(&path, "kite")
        .expect_err("a non-object document is not a settings file");
    assert!(err.contains("not a JSON object"), "{err}");
    assert_eq!(std::fs::read_to_string(&path).expect("read"), "[1,2,3]");
}

#[test]
fn a_missing_or_empty_settings_file_is_created() {
    let path = scratch("missing");
    super::save_selected_watch(&path, "AB1234", "W7").expect("a missing file is created");
    let saved = from_json(&std::fs::read_to_string(&path).expect("read"));
    assert_eq!(saved.selected_watch("AB1234"), Some("W7"));

    std::fs::write(&path, "   \n").expect("write");
    super::save_theme_preset(&path, "dracula").expect("an empty file is filled");
    let saved = from_json(&std::fs::read_to_string(&path).expect("read"));
    assert_eq!(saved.theme(), super::preset("dracula").unwrap());

    super::save_selected_watch(&path, "AB1234", "W7").expect("save");
    let saved = from_json(&std::fs::read_to_string(&path).expect("read"));
    assert_eq!(saved.theme(), super::preset("dracula").unwrap());
    assert_eq!(saved.selected_watch("AB1234"), Some("W7"));
}

#[test]
fn settings_writers_update_only_the_explicit_file() {
    let path = scratch("explicit-writers");
    std::fs::write(&path, r#"{"unknown":{"keep":7}}"#).unwrap();
    super::save_selected_watch(&path, "AB1234", "W7").unwrap();
    super::save_theme_preset(&path, "dracula").unwrap();
    super::save_movers_universe(&path, super::MoversUniverse::Nifty100).unwrap();
    super::save_chart_indicator(&path, super::ChartIndicator::Sma).unwrap();
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(
        saved,
        json!({
            "unknown": {"keep": 7}, "selected_watch": {"AB1234": "W7"},
            "theme": "dracula", "movers_universe": "nifty100", "chart_indicator": "sma"
        })
    );
}

#[test]
fn parses_named_colors_case_insensitively() {
    assert_eq!(parse_color("cyan"), Some(Color::Cyan));
    assert_eq!(parse_color("GREEN"), Some(Color::Green));
    assert_eq!(parse_color("  Red  "), Some(Color::Red));
    assert_eq!(parse_color("gray"), Some(Color::Gray));
    assert_eq!(parse_color("grey"), Some(Color::Gray));
    assert_eq!(parse_color("darkgray"), Some(Color::DarkGray));
    assert_eq!(parse_color("lightblue"), Some(Color::LightBlue));
}

#[test]
fn parses_hex_colors() {
    assert_eq!(parse_color("#ff8800"), Some(Color::Rgb(255, 136, 0)));
    assert_eq!(parse_color("#000000"), Some(Color::Rgb(0, 0, 0)));
    assert_eq!(parse_color("#FFFFFF"), Some(Color::Rgb(255, 255, 255)));
}

#[test]
fn bad_colors_return_none() {
    assert_eq!(parse_color("chartreuse"), None);
    assert_eq!(parse_color("#fff"), None); // too short
    assert_eq!(parse_color("#gggggg"), None); // non-hex digits
    assert_eq!(parse_color("#a€bc"), None);
    assert_eq!(parse_color("#+fffff"), None);
    assert_eq!(parse_color(""), None);
}

#[test]
fn theme_falls_back_per_field() {
    let raw = r##"{ "theme": { "accent": "#112233", "up": "bogus" } }"##;
    let settings = from_json(raw);
    let theme = settings.theme();
    assert_eq!(theme.accent, Color::Rgb(0x11, 0x22, 0x33));
    assert_eq!(theme.up, Color::Green); // "bogus" ignored -> default
    assert_eq!(theme.down, Color::Red); // absent -> default
}

#[test]
fn parses_chart_interval() {
    let interval = |s: &str| from_json(&format!(r#"{{"chart_interval":"{s}"}}"#)).chart_interval();
    assert_eq!(interval("1m"), Some(ChartInterval::M1));
    assert_eq!(interval("5m"), Some(ChartInterval::M5));
    assert_eq!(interval("15m"), Some(ChartInterval::M15));
    assert_eq!(interval("1h"), Some(ChartInterval::H1));
    assert_eq!(interval("60m"), Some(ChartInterval::H1));
    assert_eq!(interval("HOUR"), Some(ChartInterval::H1));
    assert_eq!(interval("1D"), Some(ChartInterval::Day));
    assert_eq!(interval("1d"), Some(ChartInterval::Day));
    assert_eq!(interval("weekly"), None);
}

#[test]
fn parses_chart_volume_toggle() {
    assert_eq!(
        from_json(r#"{"chart_volume":false}"#).chart_volume,
        Some(false)
    );
    assert_eq!(
        from_json(r#"{"chart_volume":true}"#).chart_volume,
        Some(true)
    );
    assert_eq!(from_json("{}").chart_volume, None);
}

#[test]
fn parses_chart_indicator_tokens_and_aliases() {
    use crate::tui::indicators::ChartIndicator;
    let indicator =
        |s: &str| from_json(&format!(r#"{{"chart_indicator":"{s}"}}"#)).chart_indicator();

    for expected in [
        ChartIndicator::None,
        ChartIndicator::Sma,
        ChartIndicator::Ema,
        ChartIndicator::Bollinger,
        ChartIndicator::Rsi,
        ChartIndicator::Macd,
    ] {
        assert_eq!(
            indicator(expected.config_token()),
            Some(expected),
            "token {:?} must round-trip",
            expected.config_token()
        );
    }

    assert_eq!(indicator("EMA"), Some(ChartIndicator::Ema));
    assert_eq!(indicator(" macd "), Some(ChartIndicator::Macd));
    assert_eq!(indicator("bb"), Some(ChartIndicator::Bollinger));
    assert_eq!(indicator("off"), Some(ChartIndicator::None));
    assert_eq!(indicator("stochastic"), None);
    assert_eq!(indicator(""), None);
    assert_eq!(from_json("{}").chart_indicator(), None);
}

#[test]
fn parses_movers_universe_tokens_and_aliases() {
    use crate::tui::data::MoversUniverse;
    let universe =
        |s: &str| from_json(&format!(r#"{{"movers_universe":"{s}"}}"#)).movers_universe();

    for expected in [
        MoversUniverse::Nifty50,
        MoversUniverse::Nifty100,
        MoversUniverse::Nifty500,
        MoversUniverse::NiftyFno,
    ] {
        assert_eq!(
            universe(expected.config_token()),
            Some(expected),
            "config token {} did not round-trip",
            expected.config_token()
        );
    }

    assert_eq!(universe("Nifty 50"), Some(MoversUniverse::Nifty50));
    assert_eq!(universe("nifty-500"), Some(MoversUniverse::Nifty500));
    assert_eq!(universe("fno"), Some(MoversUniverse::NiftyFno));
    assert_eq!(universe("all"), None);
    assert_eq!(universe("ALL"), None);
    assert_eq!(
        super::parse_movers_universe("all"),
        Some(MoversUniverse::All),
        "the token must still parse, it is the read that drops it"
    );

    assert_eq!(universe("nifty 1000"), None);
    assert_eq!(universe(""), None);
    assert_eq!(from_json("{}").movers_universe(), None);
}

#[test]
fn validates_default_product() {
    let idx =
        |s: &str| from_json(&format!(r#"{{"default_product":"{s}"}}"#)).default_product_index();
    assert_eq!(idx("CNC"), Some(0));
    assert_eq!(idx("mis"), Some(1)); // case-insensitive
    assert_eq!(idx("NRML"), Some(2));
    assert_eq!(idx("MTF"), Some(3));
    assert_eq!(idx("BOGUS"), None);
}

#[test]
fn clamps_poll_intervals() {
    let raw = r#"{ "poll_secs": { "orders": 0, "positions": 999, "holdings": 45 } }"#;
    let polls = from_json(raw).poll_intervals();
    assert_eq!(polls.orders, Duration::from_secs(2)); // 0 clamped up to MIN
    assert_eq!(polls.positions, Duration::from_secs(600)); // 999 clamped to MAX
    assert_eq!(polls.holdings, Duration::from_secs(45)); // in range, kept
    assert_eq!(polls.margins, Duration::from_secs(30)); // absent -> default
}

#[test]
fn unset_theme_resolves_to_kite() {
    let kite = super::preset("kite").unwrap();
    assert_eq!(from_json("{}").theme(), kite);
    assert_eq!(Settings::default().theme(), kite);
    assert!(kite.background.is_some());
    assert_ne!(kite.up, kite.down);
}

#[test]
fn system_preset_is_terminal_native() {
    let system = super::preset("system").unwrap();
    assert_eq!(system, super::Theme::DEFAULT);
    assert!(system.background.is_none());
    assert!(system.surface.is_none());
    assert!(system.text.is_none());
    assert!(system.selection.is_none());
    assert_eq!(system.border, Color::Reset);
    assert_eq!(from_json(r#"{ "theme": "system" }"#).theme(), system);
}

#[test]
fn every_rgb_preset_meets_text_contrast() {
    use super::{PRESETS, Theme};
    use crate::tui::format::contrast_ratio;

    const BODY_TEXT_MIN: f64 = 4.5;

    fn text_roles(theme: &Theme) -> [(&'static str, Option<Color>); 8] {
        [
            ("text", theme.text),
            ("text_muted", Some(theme.text_muted)),
            ("accent", Some(theme.accent)),
            ("up", Some(theme.up)),
            ("down", Some(theme.down)),
            ("warning", Some(theme.warning)),
            ("error", Some(theme.error)),
            ("info", Some(theme.info)),
        ]
    }

    fn surfaces(theme: &Theme) -> [(&'static str, Option<Color>); 3] {
        [
            ("background", theme.background),
            ("surface", theme.surface),
            ("selection", theme.selection),
        ]
    }

    let rgb = |color: Option<Color>| match color {
        Some(Color::Rgb(r, g, b)) => Some((r, g, b)),
        _ => None,
    };

    let mut failures = Vec::new();
    let mut checked = 0usize;
    for (name, theme) in PRESETS {
        if *theme == Theme::DEFAULT {
            continue;
        }
        for (role, fg) in text_roles(theme) {
            let Some(fg) = rgb(fg) else {
                failures.push(format!("{name}.{role}: not an RGB colour"));
                continue;
            };
            for (surface_name, bg) in surfaces(theme) {
                let Some(bg) = rgb(bg) else {
                    failures.push(format!("{name}.{surface_name}: not an RGB colour"));
                    continue;
                };
                checked += 1;
                let ratio = contrast_ratio(fg, bg);
                if ratio < BODY_TEXT_MIN {
                    failures.push(format!(
                        "{name}.{role} on {surface_name}: {ratio:.2} < {BODY_TEXT_MIN}"
                    ));
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {checked} text/surface pairs fail rule T:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
    assert_eq!(checked, 504, "unexpected number of contrast checks");
}

#[test]
fn every_rgb_preset_fill_takes_a_readable_label() {
    use super::{PRESETS, Theme};
    use crate::tui::format::{contrast_ratio, filled_style};

    for (name, theme) in PRESETS {
        if *theme == Theme::DEFAULT {
            continue;
        }
        for (role, fill) in [
            ("accent", theme.accent),
            ("text_muted", theme.text_muted),
            ("up", theme.up),
            ("down", theme.down),
        ] {
            let Color::Rgb(r, g, b) = fill else {
                panic!("{name}.{role}: expected an RGB fill, got {fill:?}");
            };
            let Some(Color::Rgb(lr, lg, lb)) = filled_style(fill).fg else {
                panic!("{name}.{role}: an RGB fill must yield an RGB label");
            };
            let ratio = contrast_ratio((lr, lg, lb), (r, g, b));
            assert!(ratio >= 4.5, "{name}.{role} label contrast {ratio:.2}");
        }
    }
}

#[test]
fn every_preset_is_sound() {
    use super::{PRESETS, preset, preset_names};
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    for name in preset_names() {
        assert!(seen.insert(name), "duplicate preset name: {name}");
    }
    for (name, theme) in PRESETS {
        assert_eq!(preset(name), Some(*theme), "{name} did not resolve");
        assert_ne!(theme.up, theme.down, "{name}: up == down");
        if *name == "system" {
            assert!(theme.background.is_none(), "system: background");
        } else {
            assert!(theme.background.is_some(), "{name}: background");
            assert!(theme.surface.is_some(), "{name}: surface");
            assert!(theme.text.is_some(), "{name}: text");
            assert!(theme.selection.is_some(), "{name}: selection");
            assert_ne!(theme.border, Color::Reset, "{name}: border");
        }
    }
}

#[test]
fn preset_lookup_is_case_and_separator_insensitive() {
    use super::preset;
    assert!(preset("dracula").is_some());
    assert!(preset("DRACULA").is_some());
    assert_eq!(preset("tokyo-night"), preset("tokyonight"));
    assert_eq!(preset("tokyo-night"), preset("TOKYO_NIGHT"));
    assert!(preset("tokyo-night").is_some());
    assert_eq!(preset("not-a-theme"), None);
    assert_eq!(preset(""), None);
}

#[test]
fn theme_setting_parses_preset_string() {
    let settings = from_json(r#"{ "theme": "dracula" }"#);
    assert_eq!(settings.theme(), super::preset("dracula").unwrap());
    assert_eq!(settings.theme().accent, Color::Rgb(0xca, 0xa9, 0xfa));
}

#[test]
fn theme_setting_unknown_preset_falls_back_to_default() {
    let settings = from_json(r#"{ "theme": "not-a-theme" }"#);
    assert_eq!(settings.theme(), super::Settings::default().theme());
}

#[test]
fn custom_object_falls_back_to_default_per_key() {
    use super::Theme;
    let settings = from_json(r##"{ "theme": { "background": "#123456" } }"##);
    let theme = settings.theme();
    let default = Theme::DEFAULT;
    assert_eq!(theme.background, Some(Color::Rgb(0x12, 0x34, 0x56)));
    assert_eq!(theme.surface, default.surface);
    assert_eq!(theme.text, default.text);
    assert_eq!(theme.selection, default.selection);
    assert_eq!(theme.text_muted, default.text_muted);
    assert_eq!(theme.border, default.border);
    assert_eq!(theme.accent, default.accent);
    assert_eq!(theme.up, default.up);
    assert_eq!(theme.down, default.down);
    assert_eq!(theme.warning, default.warning);
    assert_eq!(theme.error, default.error);
    assert_eq!(theme.info, default.info);
}

#[test]
fn custom_object_parses_full_surface_keys() {
    let settings = from_json(
        r##"{ "theme": { "surface": "#010203", "text_muted": "gray", "selection": "#0a0b0c", "warning": "yellow", "info": "#111111" } }"##,
    );
    let theme = settings.theme();
    assert_eq!(theme.surface, Some(Color::Rgb(1, 2, 3)));
    assert_eq!(theme.text_muted, Color::Gray);
    assert_eq!(theme.selection, Some(Color::Rgb(0x0a, 0x0b, 0x0c)));
    assert_eq!(theme.warning, Color::Yellow);
    assert_eq!(theme.info, Color::Rgb(0x11, 0x11, 0x11));
}

#[test]
fn theme_setting_garbage_yields_default() {
    let settings = from_json(r#"{ "theme": 42 }"#);
    assert_eq!(settings.theme(), super::preset("kite").unwrap());
}
#[test]
fn bom_and_duplicate_theme_keys_use_the_last_value() {
    let settings = from_json("\u{feff}{\"theme\":\"nord\",\"theme\":\"dracula \"}");
    assert_eq!(settings.theme(), super::preset("dracula").unwrap());
}
#[test]
fn selecting_a_preset_preserves_the_custom_palette() {
    let path = scratch("custom-backup");
    let custom = serde_json::json!({"background":"#123456","accent":"cyan"});
    std::fs::write(&path, serde_json::json!({"theme":custom}).to_string()).unwrap();
    super::save_theme_preset(&path, "dracula").unwrap();
    super::save_theme_preset(&path, "nord").unwrap();
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved["theme"], "nord");
    assert_eq!(saved["saved_custom_theme"], custom);
    std::fs::remove_file(path).unwrap();
}
