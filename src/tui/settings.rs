use crate::tui::app::PRODUCTS;
use crate::tui::data::{ChartInterval, MoversUniverse};
use crate::tui::indicators::ChartIndicator;
use crate::tui::split::{Split, SplitRatios};
use ratatui::style::Color;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::Duration;

mod workspace;

pub use workspace::{save_workspace_focus, save_workspace_preferences};

const MIN_POLL: u64 = 2;
const MAX_POLL: u64 = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub background: Option<Color>,
    pub surface: Option<Color>,
    pub text: Option<Color>,
    pub text_muted: Color,
    pub border: Color,
    pub selection: Option<Color>,
    pub accent: Color,
    pub up: Color,
    pub down: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,
}

impl Theme {
    pub const DEFAULT: Theme = Theme {
        background: None,
        surface: None,
        text: None,
        text_muted: Color::DarkGray,
        border: Color::Reset,
        selection: None,
        accent: Color::Cyan,
        up: Color::Green,
        down: Color::Red,
        warning: Color::Yellow,
        error: Color::Red,
        info: Color::Cyan,
    };
}

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

pub const PRESETS: &[(&str, Theme)] = &[
    (
        "kite",
        Theme {
            background: Some(rgb(0x0e, 0x14, 0x18)),
            surface: Some(rgb(0x18, 0x22, 0x2a)),
            text: Some(rgb(0xe3, 0xe9, 0xec)),
            text_muted: rgb(0x98, 0xa4, 0xa9),
            border: rgb(0x24, 0x34, 0x3c),
            selection: Some(rgb(0x22, 0x3b, 0x40)),
            accent: rgb(0x59, 0xa6, 0xf1),
            up: rgb(0x2f, 0xd0, 0x70),
            down: rgb(0xf4, 0x7e, 0x7e),
            warning: rgb(0xe8, 0xa3, 0x3d),
            error: rgb(0xff, 0x79, 0x71),
            info: rgb(0x38, 0xb6, 0xc9),
        },
    ),
    (
        "ayu",
        Theme {
            background: Some(rgb(0x0b, 0x0e, 0x14)),
            surface: Some(rgb(0x11, 0x15, 0x1c)),
            text: Some(rgb(0xbf, 0xbd, 0xb6)),
            text_muted: rgb(0x98, 0x9b, 0xa2),
            border: rgb(0x1e, 0x23, 0x2b),
            selection: Some(rgb(0x25, 0x33, 0x40)),
            accent: rgb(0xe6, 0xb4, 0x50),
            up: rgb(0xaa, 0xd9, 0x4c),
            down: rgb(0xf0, 0x71, 0x78),
            warning: rgb(0xff, 0xb4, 0x54),
            error: rgb(0xf0, 0x71, 0x78),
            info: rgb(0x59, 0xc2, 0xff),
        },
    ),
    (
        "catppuccin",
        Theme {
            background: Some(rgb(0x1e, 0x1e, 0x2e)),
            surface: Some(rgb(0x31, 0x32, 0x44)),
            text: Some(rgb(0xcd, 0xd6, 0xf4)),
            text_muted: rgb(0xb6, 0xb8, 0xc2),
            border: rgb(0x45, 0x47, 0x5a),
            selection: Some(rgb(0x45, 0x47, 0x5a)),
            accent: rgb(0xcd, 0xa9, 0xf7),
            up: rgb(0xa6, 0xe3, 0xa1),
            down: rgb(0xf5, 0xa0, 0xb8),
            warning: rgb(0xf9, 0xe2, 0xaf),
            error: rgb(0xf5, 0xa0, 0xb8),
            info: rgb(0x92, 0xba, 0xfa),
        },
    ),
    (
        "catppuccin-frappe",
        Theme {
            background: Some(rgb(0x30, 0x34, 0x46)),
            surface: Some(rgb(0x41, 0x45, 0x59)),
            text: Some(rgb(0xc6, 0xd0, 0xf5)),
            text_muted: rgb(0xcd, 0xcf, 0xd8),
            border: rgb(0x51, 0x57, 0x6d),
            selection: Some(rgb(0x51, 0x57, 0x6d)),
            accent: rgb(0xe0, 0xc6, 0xf0),
            up: rgb(0xb7, 0xda, 0x9f),
            down: rgb(0xf4, 0xc4, 0xc5),
            warning: rgb(0xe7, 0xcd, 0x9a),
            error: rgb(0xf4, 0xc4, 0xc5),
            info: rgb(0xc0, 0xd0, 0xf6),
        },
    ),
    (
        "catppuccin-macchiato",
        Theme {
            background: Some(rgb(0x24, 0x27, 0x3a)),
            surface: Some(rgb(0x36, 0x3a, 0x4f)),
            text: Some(rgb(0xca, 0xd3, 0xf5)),
            text_muted: rgb(0xbf, 0xc1, 0xcd),
            border: rgb(0x49, 0x4d, 0x64),
            selection: Some(rgb(0x49, 0x4d, 0x64)),
            accent: rgb(0xd3, 0xb5, 0xf8),
            up: rgb(0xa6, 0xda, 0x95),
            down: rgb(0xf3, 0xaf, 0xb9),
            warning: rgb(0xee, 0xd4, 0x9f),
            error: rgb(0xf3, 0xaf, 0xb9),
            info: rgb(0xa7, 0xc2, 0xf7),
        },
    ),
    (
        "cobalt2",
        Theme {
            background: Some(rgb(0x19, 0x35, 0x49)),
            surface: Some(rgb(0x12, 0x27, 0x38)),
            text: Some(rgb(0xff, 0xff, 0xff)),
            text_muted: rgb(0xae, 0xbe, 0xcd),
            border: rgb(0x23, 0x4e, 0x6e),
            selection: Some(rgb(0x23, 0x4e, 0x6e)),
            accent: rgb(0xff, 0xc6, 0x00),
            up: rgb(0x3a, 0xd9, 0x00),
            down: rgb(0xff, 0xa4, 0x94),
            warning: rgb(0xff, 0xaa, 0x21),
            error: rgb(0xff, 0xa4, 0x94),
            info: rgb(0x7d, 0xc2, 0xff),
        },
    ),
    (
        "dracula",
        Theme {
            background: Some(rgb(0x28, 0x2a, 0x36)),
            surface: Some(rgb(0x34, 0x37, 0x46)),
            text: Some(rgb(0xf8, 0xf8, 0xf2)),
            text_muted: rgb(0xb0, 0xb8, 0xd2),
            border: rgb(0x44, 0x47, 0x5a),
            selection: Some(rgb(0x44, 0x47, 0x5a)),
            accent: rgb(0xca, 0xa9, 0xfa),
            up: rgb(0x50, 0xfa, 0x7b),
            down: rgb(0xff, 0x9e, 0x9e),
            warning: rgb(0xf1, 0xfa, 0x8c),
            error: rgb(0xff, 0x9e, 0x9e),
            info: rgb(0x8b, 0xe9, 0xfd),
        },
    ),
    (
        "everforest",
        Theme {
            background: Some(rgb(0x2d, 0x35, 0x3b)),
            surface: Some(rgb(0x34, 0x3f, 0x44)),
            text: Some(rgb(0xd3, 0xc6, 0xaa)),
            text_muted: rgb(0xa8, 0xb2, 0xab),
            border: rgb(0x4a, 0x55, 0x5b),
            selection: Some(rgb(0x54, 0x3a, 0x48)),
            accent: rgb(0x7f, 0xbb, 0xb3),
            up: rgb(0xa7, 0xc0, 0x80),
            down: rgb(0xeb, 0x99, 0x9b),
            warning: rgb(0xdb, 0xbc, 0x7f),
            error: rgb(0xeb, 0x99, 0x9b),
            info: rgb(0x83, 0xc0, 0x92),
        },
    ),
    (
        "github-dark",
        Theme {
            background: Some(rgb(0x0d, 0x11, 0x17)),
            surface: Some(rgb(0x16, 0x1b, 0x22)),
            text: Some(rgb(0xc9, 0xd1, 0xd9)),
            text_muted: rgb(0x98, 0xa0, 0xa9),
            border: rgb(0x30, 0x36, 0x3d),
            selection: Some(rgb(0x30, 0x36, 0x3d)),
            accent: rgb(0x58, 0xa6, 0xff),
            up: rgb(0x3f, 0xb9, 0x50),
            down: rgb(0xfa, 0x77, 0x71),
            warning: rgb(0xd2, 0x99, 0x22),
            error: rgb(0xfa, 0x77, 0x71),
            info: rgb(0x79, 0xc0, 0xff),
        },
    ),
    (
        "gruvbox",
        Theme {
            background: Some(rgb(0x28, 0x28, 0x28)),
            surface: Some(rgb(0x3c, 0x38, 0x36)),
            text: Some(rgb(0xeb, 0xdb, 0xb2)),
            text_muted: rgb(0xc2, 0xba, 0xb1),
            border: rgb(0x50, 0x49, 0x45),
            selection: Some(rgb(0x50, 0x49, 0x45)),
            accent: rgb(0xa9, 0xc1, 0xb8),
            up: rgb(0xbf, 0xc2, 0x3c),
            down: rgb(0xfd, 0xa4, 0x9a),
            warning: rgb(0xfa, 0xbd, 0x2f),
            error: rgb(0xfd, 0xa4, 0x9a),
            info: rgb(0xa9, 0xc1, 0xb8),
        },
    ),
    (
        "kanagawa",
        Theme {
            background: Some(rgb(0x1f, 0x1f, 0x28)),
            surface: Some(rgb(0x2a, 0x2a, 0x37)),
            text: Some(rgb(0xdc, 0xd7, 0xba)),
            text_muted: rgb(0xbe, 0xbe, 0xba),
            border: rgb(0x54, 0x54, 0x6d),
            selection: Some(rgb(0x2d, 0x4f, 0x67)),
            accent: rgb(0xaa, 0xbe, 0xe5),
            up: rgb(0xaa, 0xc7, 0x85),
            down: rgb(0xf0, 0xac, 0xb4),
            warning: rgb(0xe6, 0xc3, 0x84),
            error: rgb(0xf6, 0xaa, 0xaa),
            info: rgb(0x9a, 0xc4, 0xd5),
        },
    ),
    (
        "material",
        Theme {
            background: Some(rgb(0x26, 0x32, 0x38)),
            surface: Some(rgb(0x31, 0x40, 0x48)),
            text: Some(rgb(0xee, 0xff, 0xff)),
            text_muted: rgb(0xaa, 0xb6, 0xbc),
            border: rgb(0x37, 0x47, 0x4f),
            selection: Some(rgb(0x37, 0x47, 0x4f)),
            accent: rgb(0xcf, 0xa2, 0xed),
            up: rgb(0xc3, 0xe8, 0x8d),
            down: rgb(0xf4, 0x9c, 0xa0),
            warning: rgb(0xff, 0xcb, 0x6b),
            error: rgb(0xf4, 0x9c, 0xa0),
            info: rgb(0x89, 0xdd, 0xff),
        },
    ),
    (
        "matrix",
        Theme {
            background: Some(rgb(0x00, 0x00, 0x00)),
            surface: Some(rgb(0x01, 0x14, 0x01)),
            text: Some(rgb(0x00, 0xff, 0x41)),
            text_muted: rgb(0x4a, 0xaf, 0x56),
            border: rgb(0x00, 0x55, 0x00),
            selection: Some(rgb(0x00, 0x3b, 0x00)),
            accent: rgb(0x00, 0xff, 0x41),
            up: rgb(0x00, 0xff, 0x41),
            down: rgb(0xff, 0x6a, 0x6a),
            warning: rgb(0xff, 0xb0, 0x00),
            error: rgb(0xff, 0x6a, 0x6a),
            info: rgb(0x00, 0xcc, 0x66),
        },
    ),
    (
        "monokai",
        Theme {
            background: Some(rgb(0x27, 0x28, 0x22)),
            surface: Some(rgb(0x3e, 0x3d, 0x32)),
            text: Some(rgb(0xf8, 0xf8, 0xf2)),
            text_muted: rgb(0xba, 0xb8, 0xae),
            border: rgb(0x49, 0x48, 0x3e),
            selection: Some(rgb(0x49, 0x48, 0x3e)),
            accent: rgb(0xc8, 0xa9, 0xff),
            up: rgb(0xa6, 0xe2, 0x2e),
            down: rgb(0xfc, 0x9b, 0xbe),
            warning: rgb(0xfd, 0xa5, 0x3c),
            error: rgb(0xfc, 0x9b, 0xbe),
            info: rgb(0x66, 0xd9, 0xef),
        },
    ),
    (
        "nightowl",
        Theme {
            background: Some(rgb(0x01, 0x16, 0x27)),
            surface: Some(rgb(0x0b, 0x29, 0x42)),
            text: Some(rgb(0xd6, 0xde, 0xeb)),
            text_muted: rgb(0x9a, 0xa7, 0xa7),
            border: rgb(0x1d, 0x3b, 0x53),
            selection: Some(rgb(0x1d, 0x3b, 0x53)),
            accent: rgb(0x82, 0xaa, 0xff),
            up: rgb(0x22, 0xda, 0x6e),
            down: rgb(0xf3, 0x83, 0x81),
            warning: rgb(0xec, 0xc4, 0x8d),
            error: rgb(0xf3, 0x83, 0x81),
            info: rgb(0x21, 0xc7, 0xa8),
        },
    ),
    (
        "nord",
        Theme {
            background: Some(rgb(0x2e, 0x34, 0x40)),
            surface: Some(rgb(0x3b, 0x42, 0x52)),
            text: Some(rgb(0xec, 0xef, 0xf4)),
            text_muted: rgb(0xb8, 0xbe, 0xc9),
            border: rgb(0x4c, 0x56, 0x6a),
            selection: Some(rgb(0x43, 0x4c, 0x5e)),
            accent: rgb(0x93, 0xc6, 0xd4),
            up: rgb(0xad, 0xc5, 0x99),
            down: rgb(0xe0, 0xb2, 0xb8),
            warning: rgb(0xeb, 0xcb, 0x8b),
            error: rgb(0xe0, 0xb2, 0xb8),
            info: rgb(0xab, 0xc0, 0xd5),
        },
    ),
    (
        "one-dark",
        Theme {
            background: Some(rgb(0x28, 0x2c, 0x34)),
            surface: Some(rgb(0x2c, 0x31, 0x3a)),
            text: Some(rgb(0xab, 0xb2, 0xbf)),
            text_muted: rgb(0xaf, 0xb3, 0xb9),
            border: rgb(0x3b, 0x40, 0x48),
            selection: Some(rgb(0x3e, 0x44, 0x51)),
            accent: rgb(0x74, 0xb9, 0xf1),
            up: rgb(0x98, 0xc3, 0x79),
            down: rgb(0xeb, 0x9e, 0xa4),
            warning: rgb(0xe5, 0xc0, 0x7b),
            error: rgb(0xeb, 0x9e, 0xa4),
            info: rgb(0x6a, 0xbf, 0xc9),
        },
    ),
    (
        "rose-pine",
        Theme {
            background: Some(rgb(0x19, 0x17, 0x24)),
            surface: Some(rgb(0x1f, 0x1d, 0x2e)),
            text: Some(rgb(0xe0, 0xde, 0xf4)),
            text_muted: rgb(0xae, 0xac, 0xbb),
            border: rgb(0x40, 0x3d, 0x52),
            selection: Some(rgb(0x40, 0x3d, 0x52)),
            accent: rgb(0xc4, 0xa7, 0xe7),
            up: rgb(0x9c, 0xe0, 0x8f),
            down: rgb(0xf0, 0x90, 0xab),
            warning: rgb(0xf6, 0xc1, 0x77),
            error: rgb(0xf0, 0x90, 0xab),
            info: rgb(0x9c, 0xcf, 0xd8),
        },
    ),
    (
        "solarized",
        Theme {
            background: Some(rgb(0x00, 0x2b, 0x36)),
            surface: Some(rgb(0x07, 0x36, 0x42)),
            text: Some(rgb(0x93, 0xa1, 0xa1)),
            text_muted: rgb(0x8f, 0x9e, 0xa3),
            border: rgb(0x07, 0x36, 0x42),
            selection: Some(rgb(0x07, 0x36, 0x42)),
            accent: rgb(0x4f, 0xa1, 0xdb),
            up: rgb(0x91, 0xa3, 0x1a),
            down: rgb(0xe8, 0x7a, 0x78),
            warning: rgb(0xbc, 0x95, 0x1a),
            error: rgb(0xe8, 0x7a, 0x78),
            info: rgb(0x3f, 0xaa, 0xa2),
        },
    ),
    (
        "solarized-light",
        Theme {
            background: Some(rgb(0xfd, 0xf6, 0xe3)),
            surface: Some(rgb(0xee, 0xe8, 0xd5)),
            text: Some(rgb(0x57, 0x6a, 0x71)),
            text_muted: rgb(0x60, 0x69, 0x69),
            border: rgb(0xd3, 0xcb, 0xb7),
            selection: Some(rgb(0xee, 0xe8, 0xd5)),
            accent: rgb(0x1d, 0x6b, 0xa2),
            up: rgb(0x5e, 0x6d, 0x00),
            down: rgb(0xc2, 0x2c, 0x29),
            warning: rgb(0x81, 0x61, 0x00),
            error: rgb(0xc2, 0x2c, 0x29),
            info: rgb(0x1e, 0x72, 0x6c),
        },
    ),
    ("system", Theme::DEFAULT),
    (
        "tokyo-night",
        Theme {
            background: Some(rgb(0x1a, 0x1b, 0x26)),
            surface: Some(rgb(0x24, 0x28, 0x3b)),
            text: Some(rgb(0xc0, 0xca, 0xf5)),
            text_muted: rgb(0x9a, 0x9f, 0xb8),
            border: rgb(0x3b, 0x42, 0x61),
            selection: Some(rgb(0x28, 0x34, 0x57)),
            accent: rgb(0x7a, 0xa2, 0xf7),
            up: rgb(0x9e, 0xce, 0x6a),
            down: rgb(0xf7, 0x76, 0x8e),
            warning: rgb(0xe0, 0xaf, 0x68),
            error: rgb(0xf7, 0x76, 0x8e),
            info: rgb(0x7d, 0xcf, 0xff),
        },
    ),
];

fn normalize_preset(name: &str) -> impl Iterator<Item = char> + '_ {
    name.trim()
        .chars()
        .filter(|c| !matches!(c, '-' | '_'))
        .map(|c| c.to_ascii_lowercase())
}

pub fn preset(name: &str) -> Option<Theme> {
    PRESETS
        .iter()
        .find(|(candidate, _)| normalize_preset(candidate).eq(normalize_preset(name)))
        .map(|(_, theme)| *theme)
}

pub fn preset_names() -> impl Iterator<Item = &'static str> {
    PRESETS.iter().map(|(name, _)| *name)
}

static THEME: RwLock<Theme> = RwLock::new(Theme::DEFAULT);

pub fn set_theme(theme: Theme) {
    if let Ok(mut guard) = THEME.write() {
        *guard = theme;
    }
}

pub fn theme() -> Theme {
    THEME.read().map_or(Theme::DEFAULT, |guard| *guard)
}

#[cfg(test)]
static THEME_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
pub struct ThemeGuard {
    previous: Theme,
    _lock: std::sync::MutexGuard<'static, ()>,
}

#[cfg(test)]
impl Drop for ThemeGuard {
    fn drop(&mut self) {
        set_theme(self.previous);
    }
}

#[cfg(test)]
pub fn theme_guard(theme: Theme) -> ThemeGuard {
    let lock = THEME_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let previous = self::theme();
    set_theme(theme);
    ThemeGuard {
        previous,
        _lock: lock,
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ThemeConfig {
    pub background: Option<String>,
    pub surface: Option<String>,
    pub text: Option<String>,
    pub text_muted: Option<String>,
    pub border: Option<String>,
    pub selection: Option<String>,
    pub accent: Option<String>,
    pub up: Option<String>,
    pub down: Option<String>,
    pub warning: Option<String>,
    pub error: Option<String>,
    pub info: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ThemeSetting {
    Preset(String),
    Custom(Box<ThemeConfig>),
}

impl Default for ThemeSetting {
    fn default() -> Self {
        let apple_terminal = std::env::var("TERM_PROGRAM").as_deref() == Ok("Apple_Terminal");
        let truecolor = std::env::var("COLORTERM").is_ok_and(|value| {
            matches!(value.to_ascii_lowercase().as_str(), "truecolor" | "24bit")
        });
        let system = apple_terminal && !truecolor || std::env::var("TERM").as_deref() == Ok("dumb");
        ThemeSetting::Preset(if system { "system" } else { "kite" }.to_string())
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PollConfig {
    pub orders: Option<u64>,
    pub positions: Option<u64>,
    pub holdings: Option<u64>,
    pub margins: Option<u64>,
    pub gtt: Option<u64>,
    pub alerts: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PollIntervals {
    pub orders: Duration,
    pub positions: Duration,
    pub holdings: Duration,
    pub margins: Duration,
    pub gtt: Duration,
    pub alerts: Duration,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: ThemeSetting,
    pub default_product: Option<String>,
    pub chart_interval: Option<String>,
    pub chart_volume: Option<bool>,
    pub chart_indicator: Option<String>,
    pub movers_universe: Option<String>,
    pub poll_secs: PollConfig,
    pub selected_watch: HashMap<String, String>,
    #[serde(default, deserialize_with = "split_entries")]
    pub splits: HashMap<String, f32>,
    pub workspaces: Option<serde_json::Value>,
}

impl Settings {
    pub fn theme(&self) -> Theme {
        match &self.theme {
            ThemeSetting::Preset(name) => {
                preset(name).unwrap_or_else(|| Settings::default().theme())
            }
            ThemeSetting::Custom(config) => {
                let default = Theme::DEFAULT;
                let solid = |raw: &Option<String>, fallback: Color| {
                    raw.as_deref().and_then(parse_color).unwrap_or(fallback)
                };
                let opt = |raw: &Option<String>, fallback: Option<Color>| {
                    raw.as_deref().and_then(parse_color).or(fallback)
                };
                Theme {
                    background: opt(&config.background, default.background),
                    surface: opt(&config.surface, default.surface),
                    text: opt(&config.text, default.text),
                    text_muted: solid(&config.text_muted, default.text_muted),
                    border: solid(&config.border, default.border),
                    selection: opt(&config.selection, default.selection),
                    accent: solid(&config.accent, default.accent),
                    up: solid(&config.up, default.up),
                    down: solid(&config.down, default.down),
                    warning: solid(&config.warning, default.warning),
                    error: solid(&config.error, default.error),
                    info: solid(&config.info, default.info),
                }
            }
        }
    }

    pub fn chart_interval(&self) -> Option<ChartInterval> {
        self.chart_interval.as_deref().and_then(parse_interval)
    }

    pub fn chart_indicator(&self) -> Option<ChartIndicator> {
        self.chart_indicator
            .as_deref()
            .and_then(parse_chart_indicator)
    }

    pub fn movers_universe(&self) -> Option<MoversUniverse> {
        self.movers_universe
            .as_deref()
            .and_then(parse_movers_universe)
            .filter(|universe| *universe != MoversUniverse::All)
    }

    pub fn selected_watch(&self, user_id: &str) -> Option<&str> {
        self.selected_watch
            .get(user_id)
            .map(String::as_str)
            .filter(|id| !id.is_empty())
    }

    pub fn split_ratios(&self) -> SplitRatios {
        let mut ratios = SplitRatios::default();
        for split in Split::ALL {
            if let Some(ratio) = self.splits.get(split.config_token()) {
                ratios.set(split, *ratio);
            }
        }
        ratios
    }

    pub fn default_product_index(&self) -> Option<usize> {
        let want = self.default_product.as_ref()?;
        PRODUCTS.iter().position(|p| p.eq_ignore_ascii_case(want))
    }

    pub fn poll_intervals(&self) -> PollIntervals {
        let secs = |value: Option<u64>, default: u64| {
            Duration::from_secs(value.unwrap_or(default).clamp(MIN_POLL, MAX_POLL))
        };
        PollIntervals {
            orders: secs(self.poll_secs.orders, 5),
            positions: secs(self.poll_secs.positions, 5),
            holdings: secs(self.poll_secs.holdings, 30),
            margins: secs(self.poll_secs.margins, 30),
            gtt: secs(self.poll_secs.gtt, 60),
            alerts: secs(self.poll_secs.alerts, 60),
        }
    }
}

fn parse_color(name: &str) -> Option<Color> {
    let name = name.trim();
    if let Some(hex) = name.strip_prefix('#') {
        if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let rgb = u32::from_str_radix(hex, 16).ok()?;
        let [_, red, green, blue] = rgb.to_be_bytes();
        return Some(Color::Rgb(red, green, blue));
    }
    match name.to_ascii_lowercase().as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "gray" | "grey" => Some(Color::Gray),
        "darkgray" | "darkgrey" => Some(Color::DarkGray),
        "white" => Some(Color::White),
        "lightred" => Some(Color::LightRed),
        "lightgreen" => Some(Color::LightGreen),
        "lightyellow" => Some(Color::LightYellow),
        "lightblue" => Some(Color::LightBlue),
        "lightmagenta" => Some(Color::LightMagenta),
        "lightcyan" => Some(Color::LightCyan),
        _ => None,
    }
}

fn parse_interval(token: &str) -> Option<ChartInterval> {
    match token.trim().to_ascii_lowercase().as_str() {
        "1m" => Some(ChartInterval::M1),
        "5m" => Some(ChartInterval::M5),
        "15m" => Some(ChartInterval::M15),
        "1h" | "60m" | "hour" => Some(ChartInterval::H1),
        "1d" => Some(ChartInterval::Day),
        _ => None,
    }
}

fn parse_chart_indicator(token: &str) -> Option<ChartIndicator> {
    match token.trim().to_ascii_lowercase().as_str() {
        "none" | "off" => Some(ChartIndicator::None),
        "sma" => Some(ChartIndicator::Sma),
        "ema" => Some(ChartIndicator::Ema),
        "bollinger" | "bb" => Some(ChartIndicator::Bollinger),
        "rsi" => Some(ChartIndicator::Rsi),
        "macd" => Some(ChartIndicator::Macd),
        _ => None,
    }
}

fn parse_movers_universe(token: &str) -> Option<MoversUniverse> {
    let normalized: String = token
        .to_ascii_lowercase()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect();
    match normalized.as_str() {
        "nifty50" => Some(MoversUniverse::Nifty50),
        "nifty100" => Some(MoversUniverse::Nifty100),
        "nifty500" => Some(MoversUniverse::Nifty500),
        "niftyfno" | "niftyfostocks" | "fno" => Some(MoversUniverse::NiftyFno),
        "all" => Some(MoversUniverse::All),
        _ => None,
    }
}

fn split_entries<'de, D>(deserializer: D) -> Result<HashMap<String, f32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<HashMap<String, serde_json::Value>>::deserialize(deserializer)?;
    Ok(raw
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(key, value)| {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "saved split ratios retain existing f32 rounding"
            )]
            value.as_f64().map(|ratio| (key, ratio as f32))
        })
        .collect())
}

pub fn from_json(raw: &str) -> Settings {
    serde_json::from_str::<serde_json::Value>(raw.trim_start_matches('\u{feff}'))
        .ok()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}

pub fn config_file_path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("kite").join("tui.json"))
}

fn read_document(path: &Path) -> Result<serde_json::Value, String> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(serde_json::json!({}));
        }
        Err(err) => return Err(format!("cannot read {}: {err}", path.display())),
    };
    let raw = raw.trim_start_matches('\u{feff}');
    if raw.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }
    let document: serde_json::Value = serde_json::from_str(raw)
        .map_err(|err| format!("{} is not valid JSON: {err}", path.display()))?;
    if !document.is_object() {
        return Err(format!("{} is not a JSON object", path.display()));
    }
    Ok(document)
}

fn set_paths(document: &mut serde_json::Value, writes: Vec<(&[&str], serde_json::Value)>) {
    for (keys, value) in writes {
        let Some((last, parents)) = keys.split_last() else {
            continue;
        };
        let mut at = &mut *document;
        for key in parents {
            if !at[*key].is_object() {
                at[*key] = serde_json::json!({});
            }
            at = &mut at[*key];
        }
        at[*last] = value;
    }
}

fn save_deep_settings_at(
    path: &Path,
    writes: Vec<(&[&str], serde_json::Value)>,
) -> Result<(), String> {
    let mut document = read_document(path)?;
    set_paths(&mut document, writes);
    crate::util::save_json_atomic(path, &document)
}

pub fn save_selected_watch(path: &Path, user_id: &str, watch_id: &str) -> Result<(), String> {
    save_deep_settings_at(
        path,
        vec![(&["selected_watch", user_id], serde_json::json!(watch_id))],
    )
}

pub fn save_theme_preset(path: &Path, name: &str) -> Result<(), String> {
    let document = read_document(path)?;
    let mut writes = vec![(&["theme"][..], serde_json::json!(name))];
    if document["theme"].is_object() {
        writes.push((&["saved_custom_theme"][..], document["theme"].clone()));
    }
    save_deep_settings_at(path, writes)
}

pub fn save_movers_universe(path: &Path, universe: MoversUniverse) -> Result<(), String> {
    save_deep_settings_at(
        path,
        vec![(
            &["movers_universe"],
            serde_json::json!(universe.config_token()),
        )],
    )
}

pub fn save_chart_indicator(path: &Path, indicator: ChartIndicator) -> Result<(), String> {
    save_deep_settings_at(
        path,
        vec![(
            &["chart_indicator"],
            serde_json::json!(indicator.config_token()),
        )],
    )
}

pub fn save_split_ratio(path: &Path, split: Split, ratio: f32) -> Result<(), String> {
    save_deep_settings_at(
        path,
        vec![(&["splits", split.config_token()], serde_json::json!(ratio))],
    )
}

pub fn clear_split_ratios(path: &Path) -> Result<(), String> {
    save_deep_settings_at(path, vec![(&["splits"], serde_json::json!({}))])
}

pub fn load() -> Settings {
    let Some(path) = config_file_path() else {
        return Settings::default();
    };
    std::fs::read_to_string(path)
        .ok()
        .map(|raw| from_json(&raw))
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "settings/tests.rs"]
mod tests;
