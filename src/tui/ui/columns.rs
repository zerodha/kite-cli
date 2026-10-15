use crate::tui::format;
use ratatui::layout::Constraint;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Row};

pub const SELECT_GUTTER: u16 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FitCol {
    pub width: u16,
    pub priority: u8,
}

pub const fn col(width: u16, priority: u8) -> FitCol {
    FitCol { width, priority }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnFit {
    keep: Vec<bool>,
    slack: u16,
    budget: u16,
}

impl ColumnFit {
    pub fn shows(&self, idx: usize) -> bool {
        self.keep.get(idx).copied().unwrap_or(false)
    }

    pub fn flex(&self, min: u16, max: u16) -> u16 {
        min.saturating_add(self.slack).min(max.max(min))
    }
}

fn rendered_width(columns: &[FitCol], keep: &[bool], spacing: u16) -> u16 {
    let mut total: u16 = 0;
    let mut count: u16 = 0;
    for (column, kept) in columns.iter().zip(keep) {
        if *kept {
            total = total.saturating_add(column.width);
            count = count.saturating_add(1);
        }
    }
    total.saturating_add(spacing.saturating_mul(count.saturating_sub(1)))
}

fn victim(columns: &[FitCol], keep: &[bool]) -> Option<usize> {
    if keep.iter().filter(|kept| **kept).count() <= 1 {
        return None;
    }
    columns
        .iter()
        .enumerate()
        .filter(|(idx, _)| keep[*idx])
        .max_by_key(|(idx, column)| (column.priority, *idx))
        .map(|(idx, _)| idx)
}

pub fn fit_columns(budget: u16, spacing: u16, columns: &[FitCol]) -> ColumnFit {
    let mut keep: Vec<bool> = columns.iter().map(|column| column.width > 0).collect();
    loop {
        let used = rendered_width(columns, &keep, spacing);
        if used <= budget {
            return ColumnFit {
                keep,
                slack: budget - used,
                budget,
            };
        }
        match victim(columns, &keep) {
            Some(idx) => keep[idx] = false,
            None => {
                return ColumnFit {
                    keep,
                    slack: 0,
                    budget,
                };
            }
        }
    }
}

const MIN_SPACING: u16 = 1;
pub const MAX_SPACING: u16 = 3;
const LEFT_INSET: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextCol {
    pub idx: usize,
    pub natural: u16,
    pub max: u16,
}

pub const fn text(idx: usize, natural: u16, max: u16) -> TextCol {
    TextCol { idx, natural, max }
}

pub fn widest(values: impl IntoIterator<Item = usize>) -> u16 {
    u16::try_from(values.into_iter().max().unwrap_or(0)).unwrap_or(u16::MAX)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableLayout {
    pub inset: u16,
    pub spacing: u16,
    widths: Vec<u16>,
}

impl TableLayout {
    pub fn width(&self, idx: usize) -> u16 {
        self.widths.get(idx).copied().unwrap_or(0)
    }

    pub fn highlight_symbol(&self) -> &'static str {
        if self.inset > 0 { " ▶ " } else { "▶ " }
    }
}

fn grow_to(widths: &mut [u16], slack: &mut u16, idx: usize, target: u16) {
    let Some(width) = widths.get_mut(idx).filter(|width| **width > 0) else {
        return;
    };
    let take = target.saturating_sub(*width).min(*slack);
    *width = width.saturating_add(take);
    *slack -= take;
}

pub fn spread(fit: &ColumnFit, columns: &[FitCol], grow: &[TextCol]) -> TableLayout {
    let mut widths: Vec<u16> = columns
        .iter()
        .enumerate()
        .map(|(idx, column)| if fit.shows(idx) { column.width } else { 0 })
        .collect();
    let mut slack = fit.slack;

    for column in grow {
        grow_to(
            &mut widths,
            &mut slack,
            column.idx,
            column.natural.min(column.max),
        );
    }

    let kept = widths.iter().filter(|width| **width > 0).count();
    let gaps = u16::try_from(kept.saturating_sub(1)).unwrap_or(u16::MAX);
    let mut spacing = MIN_SPACING;
    while gaps > 0 && spacing < MAX_SPACING && slack >= gaps {
        spacing += 1;
        slack -= gaps;
    }

    if let Some(column) = grow.first() {
        grow_to(&mut widths, &mut slack, column.idx, column.max);
    }

    TableLayout {
        inset: LEFT_INSET.min(slack),
        spacing,
        widths,
    }
}

pub fn table_budget(area_width: u16) -> u16 {
    area_width.saturating_sub(2 + SELECT_GUTTER)
}

pub(in crate::tui::ui) fn right(text: impl Into<Line<'static>>) -> Cell<'static> {
    Cell::from(text.into().right_aligned())
}

pub(in crate::tui::ui) fn right_styled(
    text: impl Into<std::borrow::Cow<'static, str>>,
    style: Style,
) -> Cell<'static> {
    Cell::from(Line::from(Span::styled(text, style)).right_aligned())
}

pub fn fit_header<'a>(fit: &ColumnFit, labels: &[(&'a str, bool)]) -> Row<'a> {
    let cells: Vec<Cell> = labels
        .iter()
        .enumerate()
        .filter(|(idx, _)| fit.shows(*idx))
        .map(|(_, (label, right))| {
            let line = Line::from(*label);
            Cell::from(if *right { line.right_aligned() } else { line })
        })
        .collect();
    Row::new(cells).style(format::header_style())
}

pub fn fit_constraints(fit: &ColumnFit, layout: &TableLayout) -> Vec<Constraint> {
    (0..layout.widths.len())
        .filter(|idx| fit.shows(*idx))
        .map(|idx| Constraint::Length(layout.width(idx)))
        .collect()
}

pub fn fit_row(fit: &ColumnFit, cells: &[&dyn Fn() -> Cell<'static>]) -> Row<'static> {
    let kept: Vec<Cell<'static>> = cells
        .iter()
        .enumerate()
        .filter(|(idx, _)| fit.shows(*idx))
        .map(|(_, build)| build())
        .collect();
    Row::new(kept)
}

const SORT_ARROW_CELLS: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    const fn arrow(self) -> &'static str {
        match self {
            SortDirection::Ascending => "▲",
            SortDirection::Descending => "▼",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeaderSort {
    pub column: usize,
    pub direction: SortDirection,
}

pub const fn asc(column: usize) -> HeaderSort {
    HeaderSort {
        column,
        direction: SortDirection::Ascending,
    }
}

pub const fn desc(column: usize) -> HeaderSort {
    HeaderSort {
        column,
        direction: SortDirection::Descending,
    }
}

pub fn sort_floors(columns: &[FitCol], labels: &[(&str, bool)], sortable: &[usize]) -> Vec<FitCol> {
    let mut floored = columns.to_vec();
    for &idx in sortable {
        let Some((label, _)) = labels.get(idx) else {
            continue;
        };
        let floor =
            u16::try_from(format::display_width(label) + SORT_ARROW_CELLS).unwrap_or(u16::MAX);
        if let Some(column) = floored.get_mut(idx) {
            column.width = column.width.max(floor);
        }
    }
    floored
}

pub fn fit_header_sorted<'a>(
    fit: &ColumnFit,
    layout: &TableLayout,
    labels: &[(&'a str, bool)],
    sort: HeaderSort,
) -> (Row<'a>, bool) {
    let column_cells = layout.width(sort.column).min(fit.budget) as usize;
    let marked = fit.shows(sort.column)
        && labels.get(sort.column).is_some_and(|(label, _)| {
            format::display_width(label) + SORT_ARROW_CELLS <= column_cells
        });
    let cells: Vec<Cell> = labels
        .iter()
        .enumerate()
        .filter(|(idx, _)| fit.shows(*idx))
        .map(|(idx, (label, right))| {
            let line = if marked && idx == sort.column {
                Line::from(vec![
                    Span::raw(*label),
                    Span::styled(
                        format!(" {}", sort.direction.arrow()),
                        Style::default().fg(format::accent()),
                    ),
                ])
            } else {
                Line::from(*label)
            };
            Cell::from(if *right { line.right_aligned() } else { line })
        })
        .collect();
    (Row::new(cells).style(format::header_style()), marked)
}

#[cfg(test)]
mod tests {
    use super::{col, fit_columns};

    fn kept(budget: u16, spacing: u16, columns: &[super::FitCol]) -> Vec<usize> {
        let fit = fit_columns(budget, spacing, columns);
        (0..columns.len()).filter(|idx| fit.shows(*idx)).collect()
    }

    fn count(fit: &super::ColumnFit) -> usize {
        fit.keep.iter().filter(|kept| **kept).count()
    }

    #[test]
    fn exact_fit_keeps_every_column_with_no_slack() {
        let columns = [col(10, 0), col(12, 1), col(8, 2)];
        let fit = fit_columns(32, 1, &columns);
        assert_eq!(kept(32, 1, &columns), vec![0, 1, 2]);
        assert_eq!(fit.slack, 0);
        assert_eq!(count(&fit), 3);
    }

    #[test]
    fn one_column_too_narrow_drops_exactly_one() {
        let columns = [col(10, 0), col(12, 1), col(8, 2)];
        assert_eq!(kept(31, 1, &columns), vec![0, 1]);
        assert_eq!(fit_columns(31, 1, &columns).slack, 8);
    }

    #[test]
    fn priority_order_is_honoured_not_display_order() {
        let columns = [col(10, 0), col(9, 5), col(9, 1)];
        assert_eq!(kept(200, 1, &columns), vec![0, 1, 2]);
        assert_eq!(kept(29, 1, &columns), vec![0, 2]); // 10+9+1 gap = 20 fits
        assert_eq!(kept(19, 1, &columns), vec![0]);
    }

    #[test]
    fn ties_drop_right_to_left() {
        let columns = [col(10, 0), col(9, 3), col(9, 3)];
        assert_eq!(kept(20, 1, &columns), vec![0, 1]);
    }

    #[test]
    fn absurdly_narrow_keeps_only_the_identifying_column() {
        let columns = [col(10, 0), col(12, 1), col(8, 2)];
        let fit = fit_columns(1, 1, &columns);
        assert_eq!(kept(1, 1, &columns), vec![0]);
        assert_eq!(fit.slack, 0);
        assert_eq!(kept(0, 1, &columns), vec![0]);
    }

    #[test]
    fn kept_set_never_exceeds_the_budget() {
        let columns = [
            col(10, 0),
            col(12, 1),
            col(11, 2),
            col(7, 3),
            col(8, 4),
            col(11, 5),
        ];
        for budget in 0..=140u16 {
            let fit = fit_columns(budget, 1, &columns);
            let width = super::rendered_width(&columns, &fit.keep, 1);
            assert!(
                width <= budget || count(&fit) == 1,
                "budget {budget} renders {width} across {} columns",
                count(&fit)
            );
            assert!(count(&fit) >= 1, "budget {budget} rendered no column");
            if width <= budget {
                assert_eq!(fit.slack + width, budget, "budget {budget}");
            } else {
                assert_eq!(fit.slack, 0, "budget {budget}");
            }
        }
    }

    #[test]
    fn zero_spacing_packs_columns_edge_to_edge() {
        let columns = [col(10, 0), col(12, 1), col(8, 2)];
        assert_eq!(kept(30, 0, &columns), vec![0, 1, 2]);
        assert_eq!(fit_columns(30, 0, &columns).slack, 0);
    }

    #[test]
    fn flex_grows_a_text_column_into_the_slack_and_caps_it() {
        let columns = [col(10, 0), col(12, 1)];
        let fit = fit_columns(40, 1, &columns);
        assert_eq!(fit.slack, 17);
        assert_eq!(fit.flex(10, 24), 24); // capped
        assert_eq!(fit.flex(10, 20), 20);
        let fit = fit_columns(23, 1, &columns);
        assert_eq!(fit.flex(10, 24), 10);
    }

    #[test]
    fn shows_is_false_past_the_end() {
        let fit = fit_columns(100, 1, &[col(10, 0)]);
        assert!(fit.shows(0));
        assert!(!fit.shows(1));
    }

    #[test]
    fn empty_column_list_is_all_slack() {
        let fit = fit_columns(42, 1, &[]);
        assert_eq!(count(&fit), 0);
        assert_eq!(fit.slack, 42);
    }

    use super::{MAX_SPACING, MIN_SPACING, widest};
    use crate::tui::format;

    #[test]
    fn widest_measures_terminal_cells_not_bytes() {
        assert_eq!(widest([4usize, 11, 7]), 11);
        assert_eq!(widest(Vec::<usize>::new()), 0);
        assert_eq!(
            widest(["NSE:INFY", "INDICES:NIFTY NEXT 50"].map(format::display_width)),
            21
        );
    }

    #[test]
    fn an_exactly_full_budget_is_left_alone() {
        let columns = [col(10, 0), col(12, 1), col(8, 2)];
        let fit = fit_columns(32, 1, &columns); // 30 + 2 gaps
        let layout = spread(&fit, &columns, &[text(0, 24, 24)]);
        assert_eq!(layout.spacing, MIN_SPACING);
        assert_eq!(layout.inset, 0);
        assert_eq!(
            (0..3).map(|idx| layout.width(idx)).collect::<Vec<_>>(),
            vec![10, 12, 8]
        );
    }

    #[test]
    fn the_natural_width_of_a_text_column_is_bought_before_the_gap() {
        let columns = [col(10, 0), col(12, 1)];
        let fit = fit_columns(27, 1, &columns);
        assert_eq!(fit.slack, 4);
        let layout = spread(&fit, &columns, &[text(0, 14, 24)]);
        assert_eq!(layout.width(0), 14);
        assert_eq!(layout.spacing, MIN_SPACING);
        assert_eq!(layout.inset, 0);
    }

    #[test]
    fn the_gap_is_bought_before_the_flex_columns_remainder() {
        let columns = [col(10, 0), col(12, 1)];
        let fit = fit_columns(29, 1, &columns);
        assert_eq!(fit.slack, 6);
        let layout = spread(&fit, &columns, &[text(0, 10, 24)]);
        assert_eq!(layout.spacing, MAX_SPACING);
        assert_eq!(layout.width(0), 14);
        assert_eq!(layout.inset, 0);
    }

    #[test]
    fn the_inset_is_the_last_cell_spent() {
        let columns = [col(10, 0), col(12, 1)];
        for budget in 23..=60u16 {
            let fit = fit_columns(budget, 1, &columns);
            let layout = spread(&fit, &columns, &[text(0, 12, 16)]);
            let spread_full = layout.spacing == MAX_SPACING && layout.width(0) == 16;
            assert_eq!(
                layout.inset > 0,
                spread_full && budget > 16 + 12 + MAX_SPACING,
                "budget {budget}: {layout:?}"
            );
            assert!(layout.inset <= 1);
        }
    }

    #[test]
    fn numeric_columns_keep_their_declared_width_at_every_budget() {
        let columns = [col(10, 0), col(12, 1), col(8, 2), col(11, 3)];
        for budget in 0..=200u16 {
            let fit = fit_columns(budget, 1, &columns);
            let layout = spread(&fit, &columns, &[text(0, 40, 40)]);
            for (idx, column) in columns.iter().enumerate().skip(1) {
                let width = layout.width(idx);
                assert!(
                    width == 0 || width == column.width,
                    "budget {budget}: numeric column {idx} rendered at {width}"
                );
            }
        }
    }

    #[test]
    fn the_spread_layout_never_exceeds_the_budget() {
        let columns = [col(12, 0), col(11, 1), col(8, 2), col(10, 3), col(9, 4)];
        for budget in 0..=200u16 {
            let fit = fit_columns(budget, 1, &columns);
            let grow = [text(0, 30, 30), text(2, 12, 12)];
            let layout = spread(&fit, &columns, &grow);
            let kept: Vec<u16> = (0..columns.len())
                .filter(|idx| fit.shows(*idx))
                .map(|idx| layout.width(idx))
                .collect();
            let used: u16 = kept.iter().sum::<u16>()
                + layout.inset
                + u16::try_from(kept.len().saturating_sub(1)).unwrap() * layout.spacing;
            assert!(
                used <= budget || kept.len() == 1,
                "budget {budget}: layout claims {used} ({layout:?})"
            );
        }
    }

    #[test]
    fn one_text_column_matches_the_old_flex_rule_while_the_gap_stays_narrow() {
        let columns = [col(10, 0), col(12, 1), col(8, 2), col(11, 3), col(9, 4)];
        for budget in 0..=200u16 {
            let fit = fit_columns(budget, 1, &columns);
            let layout = spread(&fit, &columns, &[text(0, 24, 24)]);
            if layout.spacing != MIN_SPACING || !fit.shows(0) {
                continue;
            }
            assert_eq!(
                layout.width(0),
                fit.flex(10, 24),
                "budget {budget} moved a packed table's text column"
            );
        }
    }

    #[test]
    fn a_zero_width_column_is_not_rendered() {
        let columns = [col(10, 0), col(0, 1), col(8, 2)];
        let fit = fit_columns(100, 1, &columns);
        assert!(fit.shows(0) && fit.shows(2));
        assert!(!fit.shows(1), "an empty column claimed a gap");
        let layout = spread(&fit, &columns, &[]);
        assert_eq!(layout.width(1), 0);
        assert_eq!(fit_constraints(&fit, &layout).len(), 2);
    }

    #[test]
    fn the_inset_widens_the_selection_marker() {
        let columns = [col(10, 0)];
        assert_eq!(
            spread(&fit_columns(10, 1, &columns), &columns, &[]).inset,
            0
        );
        assert_eq!(
            spread(&fit_columns(10, 1, &columns), &columns, &[]).highlight_symbol(),
            "▶ "
        );
        assert_eq!(
            spread(&fit_columns(11, 1, &columns), &columns, &[]).highlight_symbol(),
            " ▶ "
        );
    }

    #[test]
    fn a_natural_width_below_the_declared_one_changes_nothing() {
        let columns = [col(14, 0), col(12, 1)];
        let fit = fit_columns(40, 1, &columns);
        let layout = spread(&fit, &columns, &[text(0, 4, 4)]);
        assert_eq!(layout.width(0), 14);
    }

    use super::{fit_constraints, fit_row, spread, table_budget, text};

    fn packed(fit: &super::ColumnFit, columns: &[super::FitCol]) -> super::TableLayout {
        spread(fit, columns, &[])
    }
    use ratatui::backend::TestBackend;
    use ratatui::layout::Constraint;
    use ratatui::widgets::{Cell, Table};
    use ratatui::{Terminal, widgets::Row};
    use std::cell::Cell as CellFlag;

    fn rendered(
        header: Row<'static>,
        row: Row<'static>,
        constraints: Vec<Constraint>,
    ) -> Vec<String> {
        let width: u16 = 40;
        let table = Table::new(vec![row], constraints)
            .header(header)
            .column_spacing(1);
        let mut terminal = Terminal::new(TestBackend::new(width, 2)).expect("test backend");
        terminal
            .draw(|frame| frame.render_widget(table, frame.area()))
            .expect("draw");
        let buffer = terminal.backend().buffer().clone();
        (0..2)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol().to_string())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn table_budget_leaves_the_borders_and_the_selection_gutter_alone() {
        assert_eq!(table_budget(80), 80 - 2 - super::SELECT_GUTTER);
        assert_eq!(table_budget(2), 0);
        assert_eq!(table_budget(0), 0);
    }

    #[test]
    fn header_constraints_and_row_agree_on_which_columns_exist() {
        let columns = [col(10, 0), col(12, 1), col(8, 2)];
        let fit = fit_columns(23, 1, &columns); // room for two of the three
        assert_eq!(kept(23, 1, &columns), vec![0, 1]);

        let header = super::fit_header(&fit, &[("SYMBOL", false), ("LTP", true), ("VOL", true)]);
        let constraints = fit_constraints(&fit, &packed(&fit, &columns));
        assert_eq!(
            constraints,
            vec![Constraint::Length(10), Constraint::Length(12)]
        );
        let cell = |text: &'static str, right: bool| {
            let line = ratatui::text::Line::from(text);
            Cell::from(if right { line.right_aligned() } else { line })
        };
        let row = fit_row(
            &fit,
            &[&|| cell("INFY", false), &|| cell("1,520.30", true), &|| {
                cell("1.23L", true)
            }],
        );

        let lines = rendered(header, row, constraints);
        assert_eq!(lines[0], "SYMBOL              LTP");
        assert_eq!(lines[1], "INFY           1,520.30");
    }

    #[test]
    fn only_a_text_column_departs_from_its_declared_width() {
        let columns = [col(10, 0), col(12, 1)];
        let fit = fit_columns(40, 1, &columns);
        let layout = spread(&fit, &columns, &[text(0, 10, 24)]);
        assert_eq!(
            fit_constraints(&fit, &layout),
            vec![Constraint::Length(24), Constraint::Length(12)]
        );
        let layout = spread(&fit, &columns, &[text(1, 30, 30)]);
        assert_eq!(
            fit_constraints(&fit, &layout),
            vec![Constraint::Length(10), Constraint::Length(29)]
        );
        let layout = spread(&fit, &columns, &[text(9, 30, 30)]);
        assert_eq!(
            fit_constraints(&fit, &layout),
            vec![Constraint::Length(10), Constraint::Length(12)]
        );
    }

    #[test]
    fn a_dropped_columns_cell_is_never_built() {
        let columns = [col(10, 0), col(12, 1), col(8, 2)];
        let fit = fit_columns(23, 1, &columns); // the third column is dropped
        let built = [
            CellFlag::new(false),
            CellFlag::new(false),
            CellFlag::new(false),
        ];
        let make = |idx: usize| {
            built[idx].set(true);
            Cell::default()
        };
        let _ = fit_row(&fit, &[&|| make(0), &|| make(1), &|| make(2)]);
        assert!(built[0].get() && built[1].get());
        assert!(
            !built[2].get(),
            "a cell was formatted for a column that is not rendered"
        );
    }

    use super::{HeaderSort, SortDirection, asc, desc, fit_header_sorted, sort_floors};

    fn header_text(header: Row<'static>, constraints: Vec<Constraint>) -> String {
        let width: u16 = 60;
        let table = Table::new(vec![Row::new(Vec::<Cell>::new())], constraints)
            .header(header)
            .column_spacing(1);
        let mut terminal = Terminal::new(TestBackend::new(width, 1)).expect("test backend");
        terminal
            .draw(|frame| frame.render_widget(table, frame.area()))
            .expect("draw");
        let buffer = terminal.backend().buffer().clone();
        (0..width)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    #[test]
    fn a_sorted_header_marks_exactly_one_label() {
        let columns = [col(10, 0), col(12, 1), col(8, 2)];
        let fit = fit_columns(60, 1, &columns);
        let labels = [("SYMBOL", false), ("LTP", true), ("VOL", true)];
        let layout = packed(&fit, &columns);
        let (header, marked) = fit_header_sorted(&fit, &layout, &labels, desc(1));
        assert!(marked);
        let line = header_text(header, fit_constraints(&fit, &layout));
        assert!(line.contains("LTP ▼"), "{line:?}");
        assert_eq!(line.matches('▼').count(), 1, "{line:?}");
        assert!(!line.contains('▲'), "{line:?}");
        let (header, _) = fit_header_sorted(&fit, &layout, &labels, asc(0));
        let line = header_text(header, fit_constraints(&fit, &layout));
        assert!(line.starts_with("SYMBOL ▲"), "{line:?}");
    }

    #[test]
    fn a_dropped_sorted_column_is_reported_unmarked() {
        let columns = [col(10, 0), col(12, 1), col(8, 2)];
        let fit = fit_columns(23, 1, &columns); // room for two of three
        let labels = [("SYMBOL", false), ("LTP", true), ("VOL", true)];
        let layout = packed(&fit, &columns);
        let (header, marked) = fit_header_sorted(&fit, &layout, &labels, desc(2));
        assert!(!marked);
        let line = header_text(header, fit_constraints(&fit, &layout));
        assert!(!line.contains('▼'), "{line:?}");
        assert!(!fit_header_sorted(&fit, &layout, &labels, desc(9)).1);
    }

    #[test]
    fn a_squeezed_column_is_left_plain() {
        let columns = [col(10, 0), col(12, 1)];
        let labels = [("SYMBOL", false), ("LTP", true)];
        let fit = fit_columns(8, 1, &columns);
        assert!(fit_header_sorted(&fit, &packed(&fit, &columns), &labels, asc(0)).1);
        let fit = fit_columns(7, 1, &columns);
        assert!(fit.shows(0));
        let narrow = [col(7, 0), col(12, 1)];
        assert!(!fit_header_sorted(&fit, &packed(&fit, &narrow), &labels, asc(0)).1);
        let columns = [col(7, 0), col(12, 1)];
        let fit = fit_columns(40, 1, &columns);
        let layout = spread(&fit, &columns, &[text(0, 8, 8)]);
        assert!(fit_header_sorted(&fit, &layout, &labels, asc(0)).1);
        let layout = spread(&fit, &columns, &[text(0, 7, 7)]);
        assert!(!fit_header_sorted(&fit, &layout, &labels, asc(0)).1);
    }

    #[test]
    fn sort_floors_widens_only_the_sortable_columns() {
        let columns = [col(4, 0), col(9, 1), col(20, 2)];
        let labels = [("SYMBOL", false), ("TRIGGERED", true), ("STATUS", false)];
        let floored = sort_floors(&columns, &labels, &[0, 1]);
        assert_eq!(floored[0].width, 8); // "SYMBOL" + arrow
        assert_eq!(floored[1].width, 11); // "TRIGGERED" + arrow
        assert_eq!(floored[2].width, 20); // not sortable, untouched
        assert_eq!(
            floored.iter().map(|c| c.priority).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert_eq!(sort_floors(&columns, &labels, &[2])[2].width, 20);
        assert_eq!(sort_floors(&columns, &labels, &[9]), columns.to_vec());
        let wide = [("測試", true)];
        assert_eq!(sort_floors(&[col(1, 0)], &wide, &[0])[0].width, 6);
    }

    #[test]
    fn header_sort_constructors_name_their_direction() {
        assert_eq!(
            asc(3),
            HeaderSort {
                column: 3,
                direction: SortDirection::Ascending
            }
        );
        assert_eq!(
            desc(3),
            HeaderSort {
                column: 3,
                direction: SortDirection::Descending
            }
        );
    }
}
