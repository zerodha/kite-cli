use super::state_panel;
use crate::tui::app::{App, ChartTarget};
use crate::tui::indicators;
use crate::tui::pane_state::ChartPane;
use crate::tui::split::{Split, SplitRatios};
use crate::tui::ui::pane::{InlineChart, PaneHits};
use crate::tui::ui::{self, chart};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};

pub(super) struct ChartSplit {
    pub(super) table: Rect,
    pub(super) chart: Option<Rect>,
    divider: Option<Split>,
}

impl ChartSplit {
    pub(super) fn push_divider(&self, app: &App, area: Rect) {
        if let (Some(split), Some(chart)) = (self.divider, self.chart) {
            ui::push_divider(app, split, area, self.table, chart);
        }
    }
}

pub(super) fn chart_split(area: Rect, splits: &SplitRatios, chart: InlineChart) -> ChartSplit {
    let InlineChart::Open { maximized } = chart else {
        return ChartSplit {
            table: area,
            chart: None,
            divider: None,
        };
    };
    if maximized {
        return ChartSplit {
            table: Rect { height: 0, ..area },
            chart: Some(area),
            divider: None,
        };
    }
    if area.width >= 150 {
        let natural = area.width - (area.width / 3).max(52);
        let table_w = splits.length(Split::ChartRight, area.width, natural);
        let [table, chart] =
            Layout::horizontal([Constraint::Length(table_w), Constraint::Min(0)]).areas(area);
        ChartSplit {
            table,
            chart: Some(chart),
            divider: Some(Split::ChartRight),
        }
    } else if area.height >= 18 {
        let natural = area.height - (area.height / 2).max(9);
        let table_h = splits.length(Split::ChartBottom, area.height, natural);
        let [table, chart] =
            Layout::vertical([Constraint::Length(table_h), Constraint::Min(0)]).areas(area);
        ChartSplit {
            table,
            chart: Some(chart),
            divider: Some(Split::ChartBottom),
        }
    } else {
        ChartSplit {
            table: Rect { height: 0, ..area },
            chart: Some(area),
            divider: None,
        }
    }
}

pub(super) fn draw_chart_panel(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    target: Option<&ChartTarget>,
    chart: &ChartPane,
    hits: &PaneHits<'_>,
) {
    let Some(target) = target else {
        state_panel(
            frame,
            area,
            " Chart · c:close ",
            ui::MessageTone::Normal,
            "Chart unavailable for this row",
            None,
            Some("c Close the chart"),
        );
        return;
    };
    if target.instrument_token == 0 {
        state_panel(
            frame,
            area,
            &format!(" {} · c:close ", target.label()),
            ui::MessageTone::Normal,
            "Historical chart unavailable",
            Some("This row carries no instrument token."),
            Some("c Close the chart"),
        );
        return;
    }
    let block = ui::panel(format!(
        " {} · {} · c:close ",
        target.label(),
        chart.interval().label()
    ));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let token = target.instrument_token;
    let series = chart.indicator_series(token, app.candle_store());
    if let Some(hit) = chart::draw_chart(frame, inner, chart_request(app, chart, token, &series)) {
        hits.push_chart(chart.id(), hit);
    }
}

pub(super) fn chart_request<'a>(
    app: &'a App,
    chart: &'a ChartPane,
    token: u32,
    series: &'a indicators::Series,
) -> chart::ChartRequest<'a> {
    let key = chart.key_for(token);
    chart::ChartRequest {
        token,
        candles: app.candle_store().candles(key),
        cursor: chart.cursor(),
        hover: chart.hover_for(token).map(|hover| chart::HoverAnchor {
            selected_ts: &hover.ts,
            window_ts: &hover.window_ts,
            visible_n: hover.visible_n,
        }),
        interval: chart.interval(),
        loading: app.candles_loading(key),
        ltp: app.live_ltp(token),
        show_volume: chart.volume(),
        indicator: chart.indicator(),
        series,
    }
}

#[cfg(test)]
mod split_geometry_tests {

    use ratatui::layout::{Constraint, Layout, Rect};

    fn area(width: u16, height: u16) -> Rect {
        Rect {
            x: 3,
            y: 2,
            width,
            height,
        }
    }

    #[test]
    fn the_undragged_chart_split_is_where_the_min_and_length_put_it() {
        for width in [150u16, 156, 160, 200, 240] {
            let area = area(width, 30);
            let chart_width = (width / 3).max(52);
            let [table, _] =
                Layout::horizontal([Constraint::Min(80), Constraint::Length(chart_width)])
                    .areas(area);
            assert_eq!(
                width - chart_width,
                table.width,
                "the table moved at width {width}"
            );
        }
        for height in [18u16, 19, 20, 24, 30, 35, 45] {
            let area = area(120, height);
            let chart_height = (height / 2).max(9);
            let [table, _] =
                Layout::vertical([Constraint::Min(8), Constraint::Length(chart_height)])
                    .areas(area);
            assert_eq!(
                height - chart_height,
                table.height,
                "the table moved at height {height}"
            );
        }
    }
}
