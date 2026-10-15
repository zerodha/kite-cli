use super::{Candle, ChartHit, ChartRequest, draw_chart};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect};

pub(super) fn render(
    width: u16,
    height: u16,
    area: Rect,
    request: ChartRequest<'_>,
) -> (Buffer, Option<ChartHit>) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut hit = None;
    terminal
        .draw(|frame| hit = draw_chart(frame, area, request))
        .unwrap();
    (terminal.backend().buffer().clone(), hit)
}

pub(super) fn candle(
    timestamp: &str,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: u64,
) -> Candle {
    crate::tui::data::test_models::candle_at(timestamp, open, high, low, close, volume)
}
