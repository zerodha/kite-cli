use crate::tui::settings;
use ratatui::buffer::CellWidth;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

pub fn instrument_label(exchange: &str, symbol: &str) -> String {
    if exchange.is_empty() {
        symbol.to_string()
    } else {
        format!("{exchange}:{symbol}")
    }
}

pub fn group_indian(value: u64) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 2);
    for (index, digit) in digits.chars().enumerate() {
        let left = digits.len() - index;
        if index > 0 && left >= 3 && left % 2 == 1 {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

pub fn price(value: f64) -> String {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "rounded absolute cents use saturating float conversion"
    )]
    let cents = (value.abs() * 100.0).round() as u64;
    let negative = value < 0.0 && cents != 0;
    format!(
        "{}{}.{:02}",
        if negative { "-" } else { "" },
        group_indian(cents / 100),
        cents % 100
    )
}

pub fn signed_price(value: f64) -> String {
    let text = price(value);
    if text.starts_with('-') {
        text
    } else {
        format!("+{text}")
    }
}

pub fn percent(value: f64) -> String {
    format!("{value:+.2}%")
}

pub fn whole_price(value: f64) -> String {
    if value >= 0.0 && value.fract() == 0.0 {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "nonnegative whole-price display preserves saturating conversion"
        )]
        group_indian(value as u64)
    } else {
        price(value)
    }
}

pub fn inr(value: f64) -> String {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "rounded absolute rupees use saturating float conversion"
    )]
    let rounded = value.abs().round() as u64;
    let negative = value < 0.0 && rounded != 0;
    format!(
        "{}₹{}",
        if negative { "-" } else { "" },
        group_indian(rounded)
    )
}

pub fn fit_money(value: f64, full: &str, width: usize) -> String {
    if display_width(full) <= width {
        return full.to_string();
    }
    let compact = compact_amount(value);
    if value < 0.0 {
        format!("-{compact}")
    } else {
        format!("+{compact}")
    }
}

pub fn compact_amount(value: f64) -> String {
    let magnitude = value.abs();
    if magnitude < 10_000_000.0 {
        format!("{:.2}L", magnitude / 100_000.0)
    } else {
        format!("{:.2}Cr", magnitude / 10_000_000.0)
    }
}

pub fn fit_cell(full: &str, compact: &str, width: usize) -> String {
    if display_width(full) <= width {
        full.to_string()
    } else if display_width(compact) <= width {
        compact.to_string()
    } else {
        "…".to_string()
    }
}

pub fn display_width(text: &str) -> usize {
    if is_printable_ascii(text) {
        return text.len();
    }
    let span = Span::raw(text);
    span.styled_graphemes(Style::default())
        .map(|grapheme| grapheme_width(grapheme.symbol))
        .sum()
}

pub fn grapheme_width(symbol: &str) -> usize {
    symbol.cell_width() as usize
}

fn is_printable_ascii(text: &str) -> bool {
    text.bytes().all(|byte| matches!(byte, b' '..=b'~'))
}

fn clip_to(text: &str, width: usize, right: bool) -> String {
    if width == 0 {
        return String::new();
    }
    if is_printable_ascii(text) {
        return match (text.len() <= width, right) {
            (true, false) => format!("{text:<width$}"),
            (true, true) => format!("{text:>width$}"),
            (false, _) => format!("{}…", &text[..width - 1]),
        };
    }
    let span = Span::raw(text);
    let graphemes = || span.styled_graphemes(Style::default()).map(|g| g.symbol);
    let full: usize = graphemes().map(grapheme_width).sum();
    if full <= width {
        let body: String = graphemes().collect();
        let pad = " ".repeat(width - full);
        return if right {
            format!("{pad}{body}")
        } else {
            format!("{body}{pad}")
        };
    }
    let budget = width - 1;
    let mut kept = String::new();
    let mut used = 0usize;
    for symbol in graphemes() {
        let cells = grapheme_width(symbol);
        if used + cells > budget {
            break;
        }
        kept.push_str(symbol);
        used += cells;
    }
    let gap = " ".repeat(budget - used);
    if right {
        format!("{gap}{kept}…")
    } else {
        format!("{kept}…{gap}")
    }
}

pub fn pad_or_clip(text: &str, width: usize) -> String {
    clip_to(text, width, false)
}

pub fn pad_or_clip_right(text: &str, width: usize) -> String {
    clip_to(text, width, true)
}

pub fn compact_qty(value: u64) -> String {
    match value {
        0..=99_999 => group_indian(value),
        #[expect(
            clippy::cast_precision_loss,
            reason = "this quantity range is exactly representable in f64"
        )]
        100_000..=9_999_999 => format!("{:.2}L", value as f64 / 100_000.0),
        #[expect(
            clippy::cast_precision_loss,
            reason = "compact quantity is an approximate two-decimal display"
        )]
        _ => format!("{:.2}Cr", value as f64 / 10_000_000.0),
    }
}

pub(crate) const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WallDate {
    pub year: u32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
}

impl WallDate {
    pub fn parse(text: &str) -> Option<Self> {
        let field = |range: std::ops::Range<usize>| text.get(range)?.parse::<u32>().ok();
        let year = field(0..4)?;
        let month = field(5..7)?;
        let day = field(8..10)?;
        if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            return None;
        }
        let hour = field(11..13).unwrap_or(0);
        let minute = field(14..16).unwrap_or(0);
        Some(Self {
            year,
            month,
            day,
            hour,
            minute,
        })
    }

    pub(crate) fn month_name(self) -> &'static str {
        MONTHS[(self.month - 1) as usize]
    }

    pub(crate) fn epoch_day(self) -> i64 {
        let (year, month, day) = (
            i64::from(self.year),
            i64::from(self.month),
            i64::from(self.day),
        );
        let y = if month <= 2 { year - 1 } else { year };
        let era = y.div_euclid(400);
        let yoe = y - era * 400; // [0, 399]
        let mp = if month > 2 { month - 3 } else { month + 9 }; // [0, 11]
        let doy = (153 * mp + 2) / 5 + day - 1; // [0, 365]
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
        era * 146_097 + doe - 719_468
    }

    pub(crate) fn weekday_name(self) -> &'static str {
        WEEKDAYS[usize::try_from((self.epoch_day() + 4).rem_euclid(7)).expect("weekday is in 0..7")]
    }
}

pub fn day_month(text: &str) -> String {
    match WallDate::parse(text) {
        Some(date) => format!("{} {}", date.day, date.month_name()),
        None => text.to_string(),
    }
}

pub fn day_month_time(text: &str) -> String {
    match WallDate::parse(text) {
        Some(date) => format!(
            "{} {} {:02}:{:02}",
            date.day,
            date.month_name(),
            date.hour,
            date.minute
        ),
        None => text.to_string(),
    }
}

pub fn date_span(start: &str, end: &str) -> String {
    match (WallDate::parse(start), WallDate::parse(end)) {
        (Some(from), Some(to)) if from.month == to.month && from.year == to.year => {
            if from.day == to.day {
                day_month(end)
            } else {
                format!("{}–{} {}", from.day, to.day, to.month_name())
            }
        }
        (Some(_), Some(_)) => format!("{}–{}", day_month(start), day_month(end)),
        (None, Some(_)) => day_month(end),
        (Some(_), None) => day_month(start),
        (None, None) => String::new(),
    }
}

pub fn time_ist(unix: u32) -> String {
    clock_ist(u64::from(unix))
}

pub fn clock_ist(unix_secs: u64) -> String {
    let secs_of_day = (unix_secs + 19_800) % 86_400;
    format!(
        "{:02}:{:02}:{:02}",
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60
    )
}

pub fn uptime(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("up {h}h {m:02}m")
    } else if m > 0 {
        format!("up {m}m {s:02}s")
    } else {
        format!("up {s}s")
    }
}

pub fn date_ist(unix_secs: u64) -> (u64, u64, u64) {
    let days = (unix_secs + 19_800) / 86_400;
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097; // day of era, [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year (Mar-based), [0, 365]
    let mp = (5 * doy + 2) / 153; // Mar-based month, [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = if m <= 2 { y + 1 } else { y };
    (year, m, d)
}

pub fn datetime_ist_string(unix_secs: u64) -> String {
    let (y, m, d) = date_ist(unix_secs);
    let sod = (unix_secs + 19_800) % 86_400;
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}",
        sod / 3600,
        (sod % 3600) / 60,
        sod % 60
    )
}

pub fn weekday_ist(unix_secs: u64) -> u64 {
    let days = (unix_secs + 19_800) / 86_400;
    (days + 4) % 7
}

pub fn market_phase(unix_secs: u64) -> &'static str {
    const PRE_OPEN: u64 = 9 * 3600; // 09:00:00
    const OPEN: u64 = 9 * 3600 + 15 * 60; // 09:15:00
    const CLOSE: u64 = 15 * 3600 + 30 * 60; // 15:30:00
    let dow = weekday_ist(unix_secs);
    if dow == 0 || dow == 6 {
        return "closed";
    }
    let sod = (unix_secs + 19_800) % 86_400;
    if (PRE_OPEN..OPEN).contains(&sod) {
        "pre-open"
    } else if (OPEN..CLOSE).contains(&sod) {
        "open"
    } else {
        "closed"
    }
}

pub fn exchange_session_open(exchange: &str, unix_secs: u64) -> bool {
    let dow = weekday_ist(unix_secs);
    if dow == 0 || dow == 6 {
        return false;
    }
    let sod = (unix_secs + 19_800) % 86_400;
    let (open, close) = match exchange {
        "NSE" | "BSE" | "NFO" | "BFO" => (9 * 3600 + 15 * 60, 15 * 3600 + 30 * 60),
        "CDS" | "BCD" => (9 * 3600, 17 * 3600),
        "MCX" | "NCO" => (9 * 3600, 23 * 3600 + 30 * 60),
        _ => return false,
    };
    (open..close).contains(&sod)
}

pub fn change_style(value: f64) -> Style {
    if value > 0.0 {
        Style::default().fg(up())
    } else if value < 0.0 {
        Style::default().fg(down())
    } else {
        Style::default().fg(text_muted())
    }
}

pub fn accent() -> Color {
    settings::theme().accent
}

pub fn up() -> Color {
    settings::theme().up
}

pub fn down() -> Color {
    settings::theme().down
}

pub fn warning() -> Color {
    settings::theme().warning
}

pub fn error() -> Color {
    settings::theme().error
}

pub fn info() -> Color {
    settings::theme().info
}

pub fn text_muted() -> Color {
    settings::theme().text_muted
}

pub fn text() -> Option<Color> {
    settings::theme().text
}

pub fn border_style() -> Style {
    Style::default().fg(settings::theme().border)
}

pub fn selection_style() -> Style {
    match settings::theme().selection {
        Some(bg) => Style::default().bg(bg).add_modifier(Modifier::BOLD),
        None => Style::default().add_modifier(Modifier::REVERSED),
    }
}

pub fn relative_luminance((r, g, b): (u8, u8, u8)) -> f64 {
    fn channel(value: u8) -> f64 {
        let value = f64::from(value) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

pub fn contrast_ratio(a: (u8, u8, u8), b: (u8, u8, u8)) -> f64 {
    let (a, b) = (relative_luminance(a), relative_luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

pub fn filled_style(fill: Color) -> Style {
    const BLACK: (u8, u8, u8) = (0, 0, 0);
    const WHITE: (u8, u8, u8) = (255, 255, 255);
    let fg = match fill {
        Color::Rgb(r, g, b) => {
            if contrast_ratio(WHITE, (r, g, b)) > contrast_ratio(BLACK, (r, g, b)) {
                Color::Rgb(WHITE.0, WHITE.1, WHITE.2)
            } else {
                Color::Rgb(BLACK.0, BLACK.1, BLACK.2)
            }
        }
        _ => Color::Black,
    };
    Style::default().fg(fg).bg(fill)
}

pub fn dim() -> Style {
    Style::default().fg(text_muted())
}

pub fn bold() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

pub fn title_style() -> Style {
    Style::default().fg(accent()).add_modifier(Modifier::BOLD)
}

pub fn header_style() -> Style {
    let mut style = Style::default().add_modifier(Modifier::BOLD);
    if let Some(fg) = text() {
        style = style.fg(fg);
    }
    style
}

#[cfg(test)]
mod tests {
    use super::WallDate;
    use super::{
        accent, border_style, compact_amount, compact_qty, contrast_ratio, date_ist,
        datetime_ist_string, display_width, exchange_session_open, filled_style, fit_cell,
        fit_money, group_indian, header_style, inr, market_phase, pad_or_clip, pad_or_clip_right,
        price, relative_luminance, signed_price, time_ist, title_style, uptime, weekday_ist,
    };
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Modifier, Style};

    #[test]
    fn luminance_and_contrast_match_wcag() {
        assert!((relative_luminance((0, 0, 0)) - 0.0).abs() < 1e-12);
        assert!((relative_luminance((255, 255, 255)) - 1.0).abs() < 1e-12);
        assert!((contrast_ratio((0, 0, 0), (255, 255, 255)) - 21.0).abs() < 1e-9);
        assert!((contrast_ratio((128, 128, 128), (128, 128, 128)) - 1.0).abs() < 1e-12);
        let (a, b) = ((0x1d, 0x6b, 0xa2), (0xee, 0xe8, 0xd5));
        assert!((contrast_ratio(a, b) - contrast_ratio(b, a)).abs() < 1e-12);
        assert!(relative_luminance((10, 10, 10)) < relative_luminance((11, 11, 11)));
    }

    #[test]
    fn filled_style_picks_the_readable_rgb_label() {
        const BLACK: Color = Color::Rgb(0, 0, 0);
        const WHITE: Color = Color::Rgb(255, 255, 255);
        for (fill, expected) in [
            (Color::Rgb(0x1d, 0x6b, 0xa2), WHITE),
            (Color::Rgb(0x60, 0x69, 0x69), WHITE),
            (Color::Rgb(0x59, 0xa6, 0xf1), BLACK),
            (Color::Rgb(0xff, 0xff, 0xff), BLACK),
            (Color::Rgb(0x80, 0x80, 0x80), BLACK),
            (Color::Rgb(0, 0, 0), WHITE),
        ] {
            let style = filled_style(fill);
            assert_eq!(style.bg, Some(fill), "{fill:?}: background");
            assert_eq!(style.fg, Some(expected), "{fill:?}: foreground");
            assert_eq!(style.add_modifier, Modifier::empty(), "{fill:?}: modifier");
            let Color::Rgb(r, g, b) = fill else {
                unreachable!("test fills are all RGB")
            };
            let Some(Color::Rgb(fr, fg_, fb)) = style.fg else {
                unreachable!("an RGB fill yields an RGB label")
            };
            let ratio = contrast_ratio((fr, fg_, fb), (r, g, b));
            assert!(ratio >= 4.5, "{fill:?}: label contrast {ratio:.2}");
        }
    }

    #[test]
    fn filled_style_switches_label_at_the_contrast_crossover() {
        let grey = |v: u8| Color::Rgb(v, v, v);
        let crossover = (0..=255u8)
            .find(|v| {
                contrast_ratio((0, 0, 0), (*v, *v, *v))
                    >= contrast_ratio((255, 255, 255), (*v, *v, *v))
            })
            .expect("greys cross over below pure white");
        assert_eq!(filled_style(grey(crossover)).fg, Some(Color::Rgb(0, 0, 0)));
        assert_eq!(
            filled_style(grey(crossover - 1)).fg,
            Some(Color::Rgb(255, 255, 255))
        );
        for v in 0..=255u8 {
            let black = contrast_ratio((0, 0, 0), (v, v, v));
            let white = contrast_ratio((255, 255, 255), (v, v, v));
            let expected = if white > black {
                Color::Rgb(255, 255, 255)
            } else {
                Color::Rgb(0, 0, 0)
            };
            assert_eq!(filled_style(grey(v)).fg, Some(expected), "grey {v}");
        }
    }

    #[test]
    fn filled_style_keeps_native_fallback_for_non_rgb_fills() {
        for fill in [
            Color::Cyan,
            Color::DarkGray,
            Color::Reset,
            Color::Indexed(240),
        ] {
            let style = filled_style(fill);
            assert_eq!(style.fg, Some(Color::Black), "{fill:?}: foreground");
            assert_eq!(style.bg, Some(fill), "{fill:?}: background");
            assert_eq!(style.add_modifier, Modifier::empty(), "{fill:?}: modifier");
        }
    }

    #[test]
    fn selection_style_leaves_cell_foregrounds_alone() {
        use crate::tui::settings::{self, Theme};

        let rgb = settings::preset("kite").expect("kite preset");
        let _guard = settings::theme_guard(rgb);
        let selected = super::selection_style();
        assert_eq!(selected.fg, None);
        assert_eq!(selected.bg, rgb.selection);
        assert!(selected.add_modifier.contains(Modifier::BOLD));
        assert!(!selected.add_modifier.contains(Modifier::REVERSED));

        settings::set_theme(Theme::DEFAULT);
        let native = super::selection_style();
        assert_eq!(native.fg, None);
        assert_eq!(native.bg, None);
        assert_eq!(native.add_modifier, Modifier::REVERSED);
    }

    #[test]
    fn title_style_uses_accent_not_border() {
        let _guard = crate::tui::settings::theme_guard(crate::tui::settings::Theme::DEFAULT);
        assert_eq!(title_style().fg, Some(accent()));
        assert_ne!(title_style().fg, border_style().fg);
    }

    #[test]
    fn header_style_is_not_border_colour() {
        let _guard = crate::tui::settings::theme_guard(crate::tui::settings::Theme::DEFAULT);
        let border = border_style().fg;
        assert_ne!(header_style().fg, border);
    }

    #[test]
    fn formats_uptime() {
        assert_eq!(uptime(5), "up 5s");
        assert_eq!(uptime(59), "up 59s");
        assert_eq!(uptime(60), "up 1m 00s");
        assert_eq!(uptime(847), "up 14m 07s");
        assert_eq!(uptime(3600), "up 1h 00m");
        assert_eq!(uptime(11100), "up 3h 05m");
    }

    fn utc(year: u64, month: u64, day: u64, hour: u64, minute: u64, second: u64) -> u64 {
        let mut days = 0u64;
        for current_year in 1970..year {
            days += if leap(current_year) { 366 } else { 365 };
        }
        let months = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        for (idx, len) in months.iter().enumerate() {
            let current_month = idx as u64 + 1;
            if current_month >= month {
                break;
            }
            days += len + u64::from(current_month == 2 && leap(year));
        }
        days += day - 1;
        ((days * 24 + hour) * 60 + minute) * 60 + second
    }

    fn leap(y: u64) -> bool {
        (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
    }

    fn ist(y: u64, mo: u64, d: u64, h: u64, mi: u64, s: u64) -> u64 {
        utc(y, mo, d, h, mi, s) - 19_800
    }

    #[test]
    fn groups_digits_indian_style() {
        assert_eq!(group_indian(0), "0");
        assert_eq!(group_indian(999), "999");
        assert_eq!(group_indian(1_000), "1,000");
        assert_eq!(group_indian(123_456), "1,23,456");
        assert_eq!(group_indian(12_345_678), "1,23,45,678");
    }

    #[test]
    fn formats_prices() {
        assert_eq!(price(1523.4), "1,523.40");
        assert_eq!(price(-0.5), "-0.50");
        assert_eq!(price(152_340.505), "1,52,340.51");
    }

    #[test]
    fn price_drops_sign_on_tiny_negatives_that_round_to_zero() {
        assert_eq!(price(-0.001), "0.00");
        assert_eq!(price(-0.004), "0.00");
        assert_eq!(price(0.0), "0.00");
        assert_eq!(price(-0.0), "0.00");
        assert_eq!(price(-0.005), "-0.01");
    }

    #[test]
    fn signed_price_never_shows_negative_zero() {
        assert_eq!(signed_price(-0.001), "+0.00");
        assert_eq!(signed_price(0.0), "+0.00");
        assert_eq!(signed_price(5.0), "+5.00");
        assert_eq!(signed_price(-5.0), "-5.00");
    }

    #[test]
    fn inr_drops_sign_on_tiny_negatives_that_round_to_zero() {
        assert_eq!(inr(-0.49), "₹0");
        assert_eq!(inr(0.0), "₹0");
        assert_eq!(inr(-0.51), "-₹1");
    }

    #[test]
    fn pad_or_clip_pads_short_and_clips_long_to_exact_width() {
        assert_eq!(pad_or_clip("NSE:INFY", 12), "NSE:INFY    ");
        assert_eq!(pad_or_clip("NSE:INFY", 8), "NSE:INFY");
        let clipped = pad_or_clip("NFO:MIDCPNIFTY26AUG13400CE", 12);
        assert_eq!(clipped, "NFO:MIDCPNI…");
        assert_eq!(display_width(&clipped), 12);
        assert_eq!(pad_or_clip("anything", 0), "");
    }

    #[test]
    fn pad_or_clip_budgets_terminal_cells_not_scalars() {
        assert_eq!(pad_or_clip("", 4), "    ");
        assert_eq!(pad_or_clip("", 0), "");
        assert_eq!(pad_or_clip("a", 1), "a");
        assert_eq!(pad_or_clip("ab", 1), "…");
        assert_eq!(pad_or_clip_right("ab", 1), "…");
        assert_eq!(pad_or_clip("₹1,234", 8), "₹1,234  ");
        let combining = "e\u{301}e\u{301}e\u{301}";
        assert_eq!(display_width(combining), 3);
        assert_eq!(pad_or_clip(combining, 5), format!("{combining}  "));
        let clipped = pad_or_clip(combining, 2);
        assert_eq!(clipped, "e\u{301}…");
        assert_eq!(display_width(&clipped), 2);
        assert_eq!(display_width("測試測試"), 8);
        assert_eq!(pad_or_clip("測試測試", 10), "測試測試  ");
        let straddled = pad_or_clip("測試測", 4);
        assert_eq!(straddled, "測… ");
        assert_eq!(display_width(&straddled), 4);
        let straddled_right = pad_or_clip_right("測試測", 4);
        assert_eq!(straddled_right, " 測…");
        assert_eq!(display_width(&straddled_right), 4);
        let family = "👨‍👩‍👧";
        assert_eq!(display_width(&format!("🇮🇳{family}")), 4);
        let kept = pad_or_clip(&format!("🇮🇳{family}"), 3);
        assert_eq!(display_width(&kept), 3);
        assert_eq!(kept, "🇮🇳…");
        assert_eq!(display_width(&pad_or_clip("नमस्ते जगत", 5)), 5);
    }

    #[test]
    fn pad_or_clip_normalizes_single_line_text_at_every_width() {
        assert_eq!(pad_or_clip("a\tb", 4), "ab  ");
        assert_eq!(pad_or_clip("a\nb\rc", 5), "abc  ");
        assert_eq!(pad_or_clip_right("a\tb", 4), "  ab");
        assert_eq!(pad_or_clip("a\tbcde", 4), "abc…");
    }

    #[test]
    fn pad_or_clip_right_pads_on_the_left_and_marks_an_overflow() {
        assert_eq!(pad_or_clip_right("+1.23%", 9), "   +1.23%");
        assert_eq!(pad_or_clip_right("+1.23%", 6), "+1.23%");
        let clipped = pad_or_clip_right("+2781.3612%", 9);
        assert_eq!(clipped, "+2781.36…");
        assert_eq!(display_width(&clipped), 9);
        assert_eq!(pad_or_clip_right("anything", 0), "");
        assert_eq!(display_width(&pad_or_clip_right("₹1,234", 8)), 8);
        assert_eq!(display_width(&pad_or_clip_right("नमस्ते जगत", 5)), 5);
    }

    #[test]
    fn display_width_counts_the_cells_the_renderer_paints() {
        assert_eq!(display_width("لا"), 2);
        assert_eq!(display_width("لا لا لا"), 8);
        assert_eq!(display_width("₹1,234"), 6);
        assert_eq!(display_width("株株"), 4);
        assert_eq!(display_width("e\u{301}"), 1);
    }

    #[test]
    fn pad_or_clip_lays_out_printable_ascii() {
        assert_eq!(display_width("NSE:INFY 1,523.40"), 17);
        assert_eq!(display_width(" ~"), 2);
        assert_eq!(pad_or_clip("INFY", 7), "INFY   ");
        assert_eq!(pad_or_clip_right("INFY", 7), "   INFY");
        assert_eq!(pad_or_clip("NIFTY", 5), "NIFTY");
        assert_eq!(pad_or_clip_right("NIFTY", 5), "NIFTY");
        assert_eq!(pad_or_clip("BANKNIFTY", 5), "BANK…");
        assert_eq!(pad_or_clip_right("BANKNIFTY", 5), "BANK…");
        assert_eq!(pad_or_clip(" a ~", 6), " a ~  ");
        assert_eq!(pad_or_clip_right(" a ~", 6), "   a ~");
        assert_eq!(pad_or_clip("~~~~", 3), "~~…");
        assert_eq!(pad_or_clip("INFY", 0), "");
        assert_eq!(pad_or_clip_right("", 0), "");
        assert_eq!(pad_or_clip_right("", 3), "   ");
    }

    #[test]
    fn pad_or_clip_falls_back_outside_printable_ascii() {
        assert_eq!(display_width("a\u{7f}b"), 2);
        assert_eq!(pad_or_clip("a\u{7f}b", 4), "ab  ");
        assert_eq!(pad_or_clip_right("a\u{1f}b", 3), " ab");
        assert_eq!(pad_or_clip("ab\u{7f}cde", 4), "abc…");
        assert_eq!(display_width("NSE:株"), 6);
        assert_eq!(pad_or_clip("NSE:株", 7), "NSE:株 ");
        assert_eq!(pad_or_clip("NSE:株式", 7), "NSE:株…");
        assert_eq!(pad_or_clip("ABCD₹E", 5), "ABCD…");
        assert_eq!(pad_or_clip_right("₹1,234", 7), " ₹1,234");
        assert_eq!(pad_or_clip("cafe\u{301}s", 5), "cafe\u{301}s");
    }

    #[test]
    fn widths_and_clips_match_the_cells_ratatui_paints() {
        let painted = |text: &str| {
            let mut buffer = Buffer::empty(Rect::new(0, 0, 32, 1));
            buffer.set_stringn(0, 0, text, 32, Style::default()).0 as usize
        };
        let probes = (0u8..0x80).map(char::from).chain(['₹', '株', '\u{301}']);
        for probe in probes {
            for text in [
                format!("{probe}"),
                format!("a{probe}"),
                format!("ab{probe}cd"),
            ] {
                assert_eq!(display_width(&text), painted(&text), "{text:?}");
                for width in 0..8 {
                    assert_eq!(
                        painted(&pad_or_clip(&text, width)),
                        width,
                        "{text:?} {width}"
                    );
                    assert_eq!(
                        painted(&pad_or_clip_right(&text, width)),
                        width,
                        "{text:?} {width}"
                    );
                }
            }
        }
    }

    #[test]
    fn fit_money_keeps_full_form_when_it_fits() {
        let full = signed_price(999_999.99); // "+9,99,999.99" = 12 chars
        assert_eq!(full.chars().count(), 12);
        assert_eq!(fit_money(999_999.99, &full, 12), full);
    }

    #[test]
    fn fit_money_boundary_exact_width_uses_full_form() {
        let full = signed_price(9_999_999.99); // "+99,99,999.99" = 13 chars
        assert_eq!(full.chars().count(), 13);
        assert_eq!(fit_money(9_999_999.99, &full, 13), full);
    }

    #[test]
    fn fit_money_falls_back_to_lakh_when_it_overflows() {
        let full = signed_price(1_000_000.0); // "+10,00,000.00" = 13 chars
        assert_eq!(fit_money(1_000_000.0, &full, 12), "+10.00L");
    }

    #[test]
    fn fit_cell_never_returns_a_partial_figure() {
        let full = price(1_234_567.89); // "12,34,567.89" = 12 cells
        assert_eq!(display_width(&full), 12);
        assert_eq!(fit_cell(&full, &compact_amount(1_234_567.89), 12), full);
        assert_eq!(fit_cell(&full, &compact_amount(1_234_567.89), 11), "12.35L");
        assert_eq!(fit_cell(&full, &compact_amount(1_234_567.89), 5), "…");
    }

    #[test]
    fn fit_money_preserves_negative_sign_in_compact_form() {
        let full = signed_price(-1_000_000.0);
        assert_eq!(fit_money(-1_000_000.0, &full, 12), "-10.00L");
    }

    #[test]
    fn fit_money_falls_back_to_crore_scale() {
        let full = signed_price(150_000_000.0);
        assert_eq!(fit_money(150_000_000.0, &full, 13), "+15.00Cr");
    }

    #[test]
    fn formats_compact_quantities() {
        assert_eq!(compact_qty(99_999), "99,999");
        assert_eq!(compact_qty(123_456), "1.23L");
        assert_eq!(compact_qty(45_600_000), "4.56Cr");
    }

    #[test]
    fn formats_ist_time() {
        assert_eq!(time_ist(1_719_978_300), "09:15:00");
    }

    #[test]
    fn whole_price_drops_paise_only_for_whole_rupees() {
        assert_eq!(super::whole_price(124.0), "124");
        assert_eq!(super::whole_price(249_600.0), "2,49,600");
        assert_eq!(super::whole_price(101.5), "101.50");
    }

    #[test]
    fn wall_date_helpers_render_kite_timestamps() {
        assert_eq!(super::day_month("2026-09-10 17:00:00"), "10 Sep");
        assert_eq!(super::day_month("2026-09-10T17:00:00+05:30"), "10 Sep");
        assert_eq!(super::day_month("garbage"), "garbage");
        assert_eq!(super::day_month_time("2026-09-07 08:00:00"), "7 Sep 08:00");
        assert_eq!(
            super::date_span("2026-09-07 10:00:00", "2026-09-09 17:00:00"),
            "7–9 Sep"
        );
        assert_eq!(
            super::date_span("2026-08-28 10:00:00", "2026-09-02 17:00:00"),
            "28 Aug–2 Sep"
        );
        assert_eq!(super::date_span("", "2026-09-09 17:00:00"), "9 Sep");
        assert_eq!(super::date_span("2026-09-09", "2026-09-09"), "9 Sep");
        assert!(super::WallDate::parse("2026-13-01").is_none());
    }

    #[test]
    fn weekday_math_for_known_date() {
        assert_eq!(weekday_ist(ist(2026, 7, 7, 12, 0, 0)), 2);
        assert_eq!(weekday_ist(0), 4);
    }

    #[test]
    fn epoch_day_round_trips_civil_from_days() {
        for date in [
            (1970, 1, 1),
            (1999, 12, 31),
            (2000, 3, 1),
            (2024, 2, 29),
            (2026, 9, 10),
        ] {
            let (year, month, day) = date;
            let stamp = format!("{year:04}-{month:02}-{day:02} 09:15:00");
            let parsed = WallDate::parse(&stamp).expect("well-formed stamp");
            let secs = u64::try_from(parsed.epoch_day()).expect("fixture date is after the epoch")
                * 86_400
                + 12 * 3_600
                - 19_800;
            assert_eq!(date_ist(secs), (year, month, day), "{stamp}");
        }
    }

    #[test]
    fn weekday_names_match_the_epoch_weekday() {
        let name = |stamp: &str| WallDate::parse(stamp).expect("stamp").weekday_name();
        assert_eq!(name("1970-01-01 00:00:00"), "Thu");
        assert_eq!(name("2026-08-19 09:15:00"), "Wed");
        assert_eq!(name("2026-08-20 09:15:00"), "Thu");
        assert_eq!(name("2026-09-07 09:15:00"), "Mon");
    }

    #[test]
    fn date_ist_known_dates() {
        assert_eq!(date_ist(ist(2026, 7, 7, 9, 15, 0)), (2026, 7, 7));
        assert_eq!(date_ist(ist(2024, 2, 29, 12, 0, 0)), (2024, 2, 29));
        assert_eq!(date_ist(0), (1970, 1, 1));
    }

    #[test]
    fn datetime_ist_string_formats() {
        assert_eq!(
            datetime_ist_string(ist(2026, 7, 7, 9, 15, 0)),
            "2026-07-07 09:15:00"
        );
        assert_eq!(
            datetime_ist_string(ist(2024, 2, 29, 23, 59, 59)),
            "2024-02-29 23:59:59"
        );
        assert_eq!(datetime_ist_string(0), "1970-01-01 05:30:00");
    }

    #[test]
    fn market_phase_boundaries() {
        assert_eq!(market_phase(ist(2026, 7, 7, 8, 59, 59)), "closed");
        assert_eq!(market_phase(ist(2026, 7, 7, 9, 0, 0)), "pre-open");
        assert_eq!(market_phase(ist(2026, 7, 7, 9, 14, 59)), "pre-open");
        assert_eq!(market_phase(ist(2026, 7, 7, 9, 15, 0)), "open");
        assert_eq!(market_phase(ist(2026, 7, 7, 15, 29, 59)), "open");
        assert_eq!(market_phase(ist(2026, 7, 7, 15, 30, 0)), "closed");
        assert_eq!(market_phase(ist(2026, 7, 11, 12, 0, 0)), "closed");
    }

    #[test]
    fn exchange_sessions_exclude_premarket_ticks() {
        let before_equity_open = ist(2026, 7, 7, 9, 14, 59);
        assert!(!exchange_session_open("NSE", before_equity_open));
        assert!(exchange_session_open("NSE", ist(2026, 7, 7, 9, 15, 0)));
        assert!(exchange_session_open("CDS", ist(2026, 7, 7, 9, 0, 0)));
        assert!(!exchange_session_open("CDS", ist(2026, 7, 7, 17, 0, 0)));
        assert!(exchange_session_open("MCX", ist(2026, 7, 7, 23, 29, 59)));
        assert!(!exchange_session_open("MCX", ist(2026, 7, 7, 23, 30, 0)));
        assert!(!exchange_session_open("NSE", ist(2026, 7, 11, 10, 0, 0)));
    }
}
