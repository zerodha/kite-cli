use super::{HitAction, HitText, hint_key};
use crate::tui::app::App;
use crate::tui::form::{ControlIntent, FormViewport, ModalField};
use crate::tui::format;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

const MARKER_WIDTH: u16 = 2;
pub const LABEL_WIDTH: u16 = 12;
const GAP_WIDTH: u16 = 2;
pub const CONTROL_COLUMN: u16 = MARKER_WIDTH + LABEL_WIDTH + GAP_WIDTH;
const MIN_CONTENT_WIDTH: u16 = 4;
pub const WELL_WIDTH: u16 = 16;
const CHROME_ROWS: u16 = 7;
const NARROW_WIDTH: u16 = 72;
const WIDE_WIDTH: u16 = 88;
pub const WIDE_TERMINAL: u16 = 100;
const STACK_TERMINAL: u16 = 48;

#[derive(Debug, Clone, Copy)]
pub struct ModalLayout {
    pub outer: Rect,
    pub subtitle: Rect,
    pub body: Rect,
    pub help: Rect,
    pub actions: Rect,
    pub content: u16,
}

pub fn preferred_width(area: Rect) -> u16 {
    let preferred = if area.width < WIDE_TERMINAL {
        NARROW_WIDTH
    } else {
        WIDE_WIDTH
    };
    preferred.min(area.width.saturating_sub(2)).min(area.width)
}

pub fn content_width(area: Rect) -> u16 {
    preferred_width(area).saturating_sub(4)
}

pub fn title_fits(area: Rect, title: &str) -> bool {
    format::display_width(title) + 4 <= usize::from(preferred_width(area))
}

pub fn stacked(area: Rect) -> bool {
    area.width < STACK_TERMINAL
}

impl ModalLayout {
    pub fn measure(area: Rect, body_rows: u16) -> Option<ModalLayout> {
        let width = preferred_width(area);
        if width < 4 || area.height < 4 {
            return None;
        }
        let cap = area.height.saturating_sub(2).max(4);
        let height = body_rows
            .saturating_add(CHROME_ROWS)
            .max(12)
            .min(cap)
            .min(area.height);
        if height < 4 {
            return None;
        }
        let outer = Rect {
            x: area.x + (area.width - width) / 2,
            y: area.y + (area.height - height) / 2,
            width,
            height,
        };
        let content = width.saturating_sub(4);
        let inner_x = outer.x + 2;
        let row = |y: u16| Rect {
            x: inner_x,
            y: outer.y + y,
            width: content,
            height: 1,
        };
        if height < CHROME_ROWS + 1 {
            let body = Rect {
                x: inner_x,
                y: outer.y + 1,
                width: content,
                height: height.saturating_sub(2),
            };
            let empty = Rect {
                width: 0,
                height: 0,
                ..body
            };
            return Some(ModalLayout {
                outer,
                subtitle: empty,
                body,
                help: empty,
                actions: empty,
                content,
            });
        }
        Some(ModalLayout {
            outer,
            subtitle: row(1),
            body: Rect {
                height: height - CHROME_ROWS,
                ..row(3)
            },
            help: row(height - 3),
            actions: row(height - 2),
            content,
        })
    }

    pub fn full(&self) -> bool {
        self.actions.height > 0 && self.body.height > 0 && self.content >= MIN_CONTENT_WIDTH
    }
}

pub fn contract_fits(area: Rect) -> bool {
    ModalLayout::measure(area, 0).is_some_and(|layout| layout.full())
}

pub fn draw_frame(
    frame: &mut Frame,
    app: &App,
    layout: Option<ModalLayout>,
    title: &str,
    border: Style,
    subtitle: &str,
    too_small: &str,
) -> Option<ModalLayout> {
    let layout = layout?;
    super::modals::modal_block(
        frame,
        app,
        layout.outer,
        Line::from(Span::styled(
            format!("─ {title} "),
            border.add_modifier(Modifier::BOLD),
        )),
        border,
    );
    if !layout.full() {
        draw_too_small(frame, &layout, too_small);
        return None;
    }
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format::pad_or_clip(subtitle, layout.content as usize),
            format::dim(),
        ))),
        layout.subtitle,
    );
    for y in [layout.outer.y + 2, layout.help.y - 1] {
        rule(frame, layout.outer, y, border);
    }
    Some(layout)
}

pub fn draw_too_small(frame: &mut Frame, layout: &ModalLayout, reason: &str) {
    let lines = vec![
        Line::from(Span::styled("Resize to edit", format::dim())),
        Line::from(Span::styled(reason, format::dim())),
    ];
    frame.render_widget(Paragraph::new(lines), layout.body);
}

fn rule(frame: &mut Frame, outer: Rect, y: u16, border: Style) {
    if y >= outer.bottom() || outer.width < 2 {
        return;
    }
    let mut text = String::from("├");
    text.push_str(&"─".repeat(usize::from(outer.width - 2)));
    text.push('┤');
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(text, border))),
        Rect {
            x: outer.x,
            y,
            width: outer.width,
            height: 1,
        },
    );
}

#[derive(Debug, Clone)]
pub struct Segment {
    pub label: String,
    pub selected: bool,
    pub enabled: bool,
}

impl Segment {
    pub fn new(label: impl Into<String>, selected: bool) -> Self {
        Self {
            label: label.into(),
            selected,
            enabled: true,
        }
    }

    pub fn disabled(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            selected: false,
            enabled: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Control {
    Well { value: String, width: u16 },
    Segments(Vec<Segment>),
    Carousel(String),
    Fixed(String),
}

#[derive(Debug, Clone)]
pub struct FormRow {
    pub field: Option<ModalField>,
    pub label: &'static str,
    pub control: Control,
    pub focused: bool,
    pub issue: Option<String>,
}

impl FormRow {
    pub fn well(field: ModalField, label: &'static str, value: &str, focused: bool) -> Self {
        Self {
            field: Some(field),
            label,
            control: Control::Well {
                value: value.to_string(),
                width: WELL_WIDTH,
            },
            focused,
            issue: None,
        }
    }

    pub fn segments(
        field: ModalField,
        label: &'static str,
        options: Vec<Segment>,
        focused: bool,
    ) -> Self {
        Self {
            field: Some(field),
            label,
            control: Control::Segments(options),
            focused,
            issue: None,
        }
    }

    pub fn carousel(field: ModalField, label: &'static str, value: &str, focused: bool) -> Self {
        Self {
            field: Some(field),
            label,
            control: Control::Carousel(value.to_string()),
            focused,
            issue: None,
        }
    }

    pub fn fixed(label: &'static str, value: impl Into<String>, reason: &str) -> Self {
        Self {
            field: None,
            label,
            control: Control::Fixed(format!("{} ({reason})", value.into())),
            focused: false,
            issue: None,
        }
    }

    pub fn issue(mut self, issue: Option<String>) -> Self {
        self.issue = issue;
        self
    }
}

#[derive(Debug, Clone)]
pub struct ListRow {
    pub field: Option<ModalField>,
    pub index: usize,
    pub selected: bool,
    pub label: String,
    pub value: String,
    pub value_style: Style,
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
pub enum FormItem {
    Section(String),
    Row(FormRow),
    List(ListRow),
    Note(String),
    Warning(String),
    Error(String),
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "field enums are passed by value by form callers"
)]
pub fn issue_for<F: PartialEq>(
    issue: Option<&crate::tui::form::FormIssue<F>>,
    field: F,
) -> Option<String> {
    issue
        .filter(|issue| issue.field.as_ref() == Some(&field))
        .map(|issue| issue.message.clone())
}

pub fn general_issue<F>(items: &mut Vec<FormItem>, issue: Option<&crate::tui::form::FormIssue<F>>) {
    if let Some(issue) = issue.filter(|issue| issue.field.is_none()) {
        items.push(FormItem::Error(issue.message.clone()));
    }
}

#[derive(Debug)]
struct PlacedRow {
    line: Line<'static>,
    targets: Vec<(u16, u16, HitAction)>,
}

#[derive(Debug)]
pub struct BodyPlan {
    rows: Vec<PlacedRow>,
    focus: Option<(u16, u16)>,
}

impl BodyPlan {
    pub fn new(items: &[FormItem], content: u16, stacked: bool) -> Self {
        let mut rows: Vec<PlacedRow> = Vec::new();
        let mut focus = None;
        for item in items {
            let start = u16::try_from(rows.len()).unwrap_or(u16::MAX);
            match item {
                FormItem::Section(label) => rows.push(plain(
                    format::pad_or_clip(label, content as usize),
                    format::bold(),
                )),
                FormItem::Note(text) => wrap_into(&mut rows, text, 0, content, format::dim()),
                FormItem::Warning(text) => wrap_into(
                    &mut rows,
                    text,
                    0,
                    content,
                    Style::default().fg(format::warning()),
                ),
                FormItem::Error(text) => wrap_into(
                    &mut rows,
                    text,
                    0,
                    content,
                    Style::default().fg(format::error()),
                ),
                FormItem::Row(row) => {
                    place_row(&mut rows, row, content, stacked);
                    if row.focused {
                        let end = u16::try_from(rows.len().saturating_sub(1)).unwrap_or(u16::MAX);
                        focus = Some((start, end));
                    }
                }
                FormItem::List(row) => {
                    place_list_row(&mut rows, row, content);
                    if row.selected {
                        let end = u16::try_from(rows.len().saturating_sub(1)).unwrap_or(u16::MAX);
                        focus = Some((start, end));
                    }
                }
            }
        }
        Self { rows, focus }
    }

    pub fn rows(&self) -> u16 {
        u16::try_from(self.rows.len()).unwrap_or(u16::MAX)
    }
}

pub fn query_row(label: &str, value: &str, content: u16) -> Line<'static> {
    let focus = format::title_style();
    let mut spans = vec![
        Span::styled("> ", focus),
        Span::styled(
            format::pad_or_clip_right(label, usize::from(LABEL_WIDTH)),
            focus,
        ),
        Span::raw("  "),
    ];
    spans.extend(well_spans(
        value,
        content.saturating_sub(CONTROL_COLUMN),
        true,
        false,
    ));
    Line::from(spans)
}

fn plain(text: String, style: Style) -> PlacedRow {
    PlacedRow {
        line: Line::from(Span::styled(text, style)),
        targets: Vec::new(),
    }
}

fn wrap_into(rows: &mut Vec<PlacedRow>, text: &str, indent: u16, content: u16, style: Style) {
    let width = usize::from(content.saturating_sub(indent)).max(1);
    let pad = " ".repeat(usize::from(indent));
    for chunk in wrap_cells(text, width) {
        rows.push(plain(format!("{pad}{chunk}"), style));
    }
}

pub fn wrap_cells(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut out: Vec<String> = Vec::new();
    let mut line = String::new();
    let mut line_width = 0usize;
    let push_line = |line: &mut String, line_width: &mut usize, out: &mut Vec<String>| {
        out.push(std::mem::take(line));
        *line_width = 0;
    };
    for word in text.split_whitespace() {
        let word_width = format::display_width(word);
        if line_width > 0 && line_width + 1 + word_width > width {
            push_line(&mut line, &mut line_width, &mut out);
        }
        if word_width > width {
            for grapheme in word.chars() {
                let cell = format::display_width(grapheme.encode_utf8(&mut [0; 4]));
                if line_width > 0 && line_width + cell > width {
                    push_line(&mut line, &mut line_width, &mut out);
                }
                line.push(grapheme);
                line_width += cell;
            }
            continue;
        }
        if line_width > 0 {
            line.push(' ');
            line_width += 1;
        }
        line.push_str(word);
        line_width += word_width;
    }
    if !line.is_empty() || out.is_empty() {
        out.push(line);
    }
    out
}

fn place_row(rows: &mut Vec<PlacedRow>, row: &FormRow, content: u16, stacked: bool) {
    let marker = if row.focused { "> " } else { "  " };
    let focus_style = if row.focused {
        format::title_style()
    } else {
        format::dim()
    };
    let (control_column, mut spans) = if stacked {
        rows.push(PlacedRow {
            line: Line::from(vec![
                Span::styled(marker.to_string(), focus_style),
                Span::styled(
                    format::pad_or_clip(
                        row.label,
                        usize::from(content.saturating_sub(MARKER_WIDTH)),
                    ),
                    focus_style,
                ),
            ]),
            targets: row
                .field
                .map(|field| {
                    vec![(
                        0,
                        content,
                        HitAction::ModalControl {
                            field,
                            intent: ControlIntent::Focus,
                        },
                    )]
                })
                .unwrap_or_default(),
        });
        (MARKER_WIDTH, vec![Span::raw("  ")])
    } else {
        (
            CONTROL_COLUMN,
            vec![
                Span::styled(marker.to_string(), focus_style),
                Span::styled(
                    format::pad_or_clip_right(row.label, usize::from(LABEL_WIDTH)),
                    focus_style,
                ),
                Span::raw("  "),
            ],
        )
    };

    let mut targets: Vec<(u16, u16, HitAction)> = Vec::new();
    if let Some(field) = row.field {
        targets.push((
            0,
            content,
            HitAction::ModalControl {
                field,
                intent: ControlIntent::Focus,
            },
        ));
    }
    place_control(row, content, control_column, &mut spans, &mut targets);
    rows.push(PlacedRow {
        line: Line::from(spans),
        targets,
    });
    if let Some(message) = &row.issue {
        wrap_into(
            rows,
            &format!("! {message}"),
            control_column,
            content,
            Style::default().fg(format::error()),
        );
    }
}

fn place_list_row(rows: &mut Vec<PlacedRow>, row: &ListRow, content: u16) {
    let gutter = if row.field.is_some() { MARKER_WIDTH } else { 0 };
    let mut spans: Vec<Span<'static>> = Vec::new();
    if gutter > 0 {
        spans.push(Span::styled(
            if row.selected { "> " } else { "  " }.to_string(),
            format::title_style(),
        ));
    }
    let selection = if row.selected {
        format::selection_style()
    } else {
        Style::default()
    };
    let available = usize::from(content.saturating_sub(gutter));
    let label_width = format::display_width(&row.label).min(available);
    spans.push(Span::styled(
        format::pad_or_clip(&row.label, label_width),
        selection,
    ));
    spans.push(Span::styled(
        format::pad_or_clip(&row.value, available - label_width),
        row.value_style.patch(selection),
    ));
    let mut targets = Vec::new();
    if let Some(field) = row.field {
        targets.push((
            0,
            content,
            HitAction::ModalControl {
                field,
                intent: ControlIntent::Option(row.index),
            },
        ));
    }
    rows.push(PlacedRow {
        line: Line::from(spans),
        targets,
    });
    if let Some(note) = &row.note {
        wrap_into(
            rows,
            &format!("! {note}"),
            gutter + 2,
            content,
            Style::default().fg(format::error()),
        );
    }
}

fn well_spans(value: &str, width: u16, focused: bool, failed: bool) -> Vec<Span<'static>> {
    let bracket = if failed {
        Style::default().fg(format::error())
    } else if focused {
        Style::default().fg(format::accent())
    } else {
        format::border_style()
    };
    let inner = usize::from(width.saturating_sub(2)).max(1);
    let reserved = usize::from(focused);
    let room = inner.saturating_sub(reserved).max(1);
    let shown = clip_tail(value, room);
    let pad = inner.saturating_sub(format::display_width(&shown) + reserved);
    let mut spans = vec![Span::styled("[", bracket), Span::raw(shown)];
    if focused {
        spans.push(Span::styled(" ", format::filled_style(format::accent())));
    }
    spans.push(Span::raw(" ".repeat(pad)));
    spans.push(Span::styled("]", bracket));
    spans
}

fn clip_tail(text: &str, width: usize) -> String {
    if format::display_width(text) <= width {
        return text.to_string();
    }
    let mut out: Vec<char> = Vec::new();
    let mut used = 0usize;
    for ch in text.chars().rev() {
        let cell = format::display_width(ch.encode_utf8(&mut [0; 4]));
        if used + cell > width {
            break;
        }
        out.push(ch);
        used += cell;
    }
    out.reverse();
    out.into_iter().collect()
}

fn segment_spans(
    options: &[Segment],
    focused: bool,
) -> (Vec<Span<'static>>, Vec<(u16, u16, usize)>) {
    let frame_style = if focused {
        Style::default().fg(format::accent())
    } else {
        format::border_style()
    };
    let mut spans = vec![Span::styled("[", frame_style)];
    let mut hits = Vec::new();
    let mut offset: u16 = 1;
    for (index, option) in options.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled("|", frame_style));
            offset += 1;
        }
        let marker = if !option.enabled {
            '-'
        } else if option.selected {
            '*'
        } else {
            ' '
        };
        let text = format!("{marker}{} ", option.label);
        let width = u16::try_from(format::display_width(&text)).unwrap_or(u16::MAX);
        let style = if !option.enabled {
            Style::default().fg(format::text_muted())
        } else if option.selected && focused {
            format::filled_style(format::accent()).add_modifier(Modifier::BOLD)
        } else if option.selected {
            format::selection_style().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        spans.push(Span::styled(text, style));
        if option.enabled {
            hits.push((offset, width, index));
        }
        offset += width;
    }
    spans.push(Span::styled("]", frame_style));
    (spans, hits)
}

fn carousel_spans(value: &str, focused: bool) -> (Vec<Span<'static>>, Vec<(u16, ControlIntent)>) {
    let frame_style = if focused {
        Style::default().fg(format::accent())
    } else {
        format::border_style()
    };
    let chevron = format::title_style();
    let value_style = if focused {
        format::filled_style(format::accent()).add_modifier(Modifier::BOLD)
    } else {
        format::selection_style().add_modifier(Modifier::BOLD)
    };
    let spans = vec![
        Span::styled("[", frame_style),
        Span::styled("<", chevron),
        Span::styled(format!(" {value} "), value_style),
        Span::styled(">", chevron),
        Span::styled("]", frame_style),
    ];
    let next = 2 + u16::try_from(format::display_width(value)).unwrap_or(u16::MAX) + 1;
    (
        spans,
        vec![(1, ControlIntent::Previous), (next, ControlIntent::Next)],
    )
}

pub fn draw_body(
    frame: &mut Frame,
    app: &App,
    layout: &ModalLayout,
    plan: &BodyPlan,
    viewport: &FormViewport,
) {
    if layout.body.height == 0 || layout.body.width == 0 {
        return;
    }
    let scroll = viewport.resolve(plan.rows(), layout.body.height, plan.focus);
    app.hit
        .borrow_mut()
        .push(layout.body, HitAction::Text(HitText::ModalBody));
    for offset in 0..layout.body.height {
        let Some(row) = plan.rows.get(usize::from(scroll + offset)) else {
            break;
        };
        let rect = Rect {
            y: layout.body.y + offset,
            height: 1,
            ..layout.body
        };
        frame.render_widget(&row.line, rect);
        for (x, width, action) in &row.targets {
            let x = rect.x.saturating_add(*x);
            if x >= rect.right() {
                continue;
            }
            app.hit.borrow_mut().push(
                Rect {
                    x,
                    y: rect.y,
                    width: (*width).min(rect.right() - x),
                    height: 1,
                },
                *action,
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    Primary,
    Secondary,
    Destructive,
    Disabled,
    Note,
}

#[derive(Debug, Clone, Copy)]
pub struct FormAction {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: ActionKind,
}

#[derive(Debug, Clone, Copy)]
pub struct FormFooter {
    pub help: &'static [&'static str],
    pub actions: &'static [FormAction],
    pub hints: &'static [super::HintGroup],
}

fn action_text(action: &FormAction) -> String {
    match action.kind {
        ActionKind::Note => action.label.to_string(),
        _ => format!("[{} {}]", action.key, action.label),
    }
}

fn actions_width(actions: &[FormAction]) -> u16 {
    let separators = 2 * u16::try_from(actions.len().saturating_sub(1)).unwrap_or(u16::MAX);
    actions
        .iter()
        .map(|action| {
            u16::try_from(format::display_width(&action_text(action))).unwrap_or(u16::MAX)
        })
        .sum::<u16>()
        .saturating_add(separators)
}

pub fn draw_footer(frame: &mut Frame, app: &App, layout: &ModalLayout, footer: &FormFooter) {
    if !layout.full() {
        return;
    }
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format::pad_or_clip(&footer.help.join("  "), usize::from(layout.content)),
            format::dim(),
        ))),
        layout.help,
    );

    let mut actions = footer.actions;
    while !actions.is_empty() && actions_width(actions) > layout.content {
        actions = &actions[1..];
    }

    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut offset: u16 = 0;
    for action in actions {
        if offset > 0 {
            spans.push(Span::raw("  "));
            offset += 2;
        }
        let text = action_text(action);
        let width = u16::try_from(format::display_width(&text)).unwrap_or(u16::MAX);
        match action.kind {
            ActionKind::Primary => spans.push(Span::styled(
                text,
                format::filled_style(format::accent()).add_modifier(Modifier::BOLD),
            )),
            ActionKind::Destructive => spans.push(Span::styled(
                text,
                format::filled_style(format::error()).add_modifier(Modifier::BOLD),
            )),
            ActionKind::Note => spans.push(Span::styled(text, format::dim())),
            ActionKind::Secondary => {
                spans.push(Span::raw("["));
                spans.push(Span::styled(action.key.to_string(), format::title_style()));
                spans.push(Span::raw(format!(" {}]", action.label)));
            }
            ActionKind::Disabled => spans.push(Span::styled(
                text,
                Style::default().fg(format::text_muted()),
            )),
        }
        if matches!(
            action.kind,
            ActionKind::Primary | ActionKind::Secondary | ActionKind::Destructive
        ) && let Some(key) = hint_key(action.key)
            && offset < layout.content
        {
            app.hit.borrow_mut().push(
                Rect {
                    x: layout.actions.x + offset,
                    y: layout.actions.y,
                    width: width.min(layout.content - offset),
                    height: 1,
                },
                HitAction::Key(key),
            );
        }
        offset += width;
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), layout.actions);
}

fn place_control(
    row: &FormRow,
    content: u16,
    control_column: u16,
    spans: &mut Vec<Span<'static>>,
    targets: &mut Vec<(u16, u16, HitAction)>,
) {
    let available = content.saturating_sub(control_column);
    match &row.control {
        Control::Well { value, width } => {
            let width = (*width).min(available.max(2));
            spans.extend(well_spans(value, width, row.focused, row.issue.is_some()));
        }
        Control::Segments(options) => {
            let (segment_spans, hits) = segment_spans(options, row.focused);
            for (offset, width, index) in hits {
                if let Some(field) = row.field {
                    targets.push((
                        control_column + offset,
                        width,
                        HitAction::ModalControl {
                            field,
                            intent: ControlIntent::Option(index),
                        },
                    ));
                }
            }
            spans.extend(segment_spans);
        }
        Control::Carousel(value) => {
            let (carousel, chevrons) = carousel_spans(value, row.focused);
            for (offset, intent) in chevrons {
                if let Some(field) = row.field {
                    targets.push((
                        control_column + offset,
                        1,
                        HitAction::ModalControl { field, intent },
                    ));
                }
            }
            spans.extend(carousel);
        }
        Control::Fixed(value) => spans.push(Span::styled(
            format::pad_or_clip(value, usize::from(available)),
            Style::default().fg(format::text_muted()),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{ModalLayout, WELL_WIDTH, preferred_width, segment_spans, well_spans, wrap_cells};
    use crate::tui::ui::forms::Segment;
    use ratatui::layout::Rect;

    fn area(width: u16, height: u16) -> Rect {
        Rect {
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    #[test]
    fn preferred_width_widens_at_a_hundred_columns() {
        assert_eq!(preferred_width(area(80, 24)), 72);
        assert_eq!(preferred_width(area(99, 24)), 72);
        assert_eq!(preferred_width(area(120, 35)), 88);
        assert_eq!(preferred_width(area(40, 24)), 38);
        assert_eq!(preferred_width(area(1, 1)), 0);
    }

    #[test]
    fn a_tiny_area_produces_no_frame() {
        assert!(ModalLayout::measure(area(1, 1), 4).is_none());
        assert!(ModalLayout::measure(area(0, 0), 4).is_none());
        assert!(ModalLayout::measure(area(80, 3), 4).is_none());
    }

    #[test]
    fn a_short_frame_drops_to_the_resize_notice() {
        let layout = ModalLayout::measure(area(32, 10), 10).expect("a frame fits");
        assert!(layout.full());
        assert_eq!((layout.outer.height, layout.body.height), (8, 1));
        let layout = ModalLayout::measure(area(32, 9), 10).expect("a frame fits");
        assert!(!layout.full(), "seven rows cannot hold the contract");
        assert_eq!(layout.actions.height, 0);
    }

    #[test]
    fn a_frame_with_no_body_width_drops_to_the_resize_notice() {
        let layout = ModalLayout::measure(area(6, 24), 10).expect("a frame fits");
        assert_eq!((layout.actions.height, layout.content), (1, 0));
        assert!(!layout.full(), "a zero-width body cannot hold the contract");
        assert!(!super::contract_fits(area(6, 24)));
        assert!(super::contract_fits(area(10, 24)));
        assert!(!super::contract_fits(area(9, 24)));
    }

    #[test]
    fn the_body_is_the_height_less_seven_chrome_rows() {
        let layout = ModalLayout::measure(area(80, 24), 10).expect("a frame fits");
        assert_eq!((layout.outer.x, layout.outer.y), (4, 3));
        assert_eq!((layout.outer.width, layout.outer.height), (72, 17));
        assert_eq!(layout.body.height, 10);
        assert_eq!(layout.content, 68);
        assert_eq!(layout.subtitle.y, layout.outer.y + 1);
        assert_eq!(layout.actions.y, layout.outer.bottom() - 2);
    }

    #[test]
    fn a_body_taller_than_the_terminal_is_capped() {
        let layout = ModalLayout::measure(area(80, 24), 40).expect("a frame fits");
        assert_eq!(layout.outer.height, 22, "terminal height less two");
        assert_eq!(layout.body.height, 15);
    }

    fn well_text(value: &str, focused: bool) -> String {
        well_spans(value, WELL_WIDTH, focused, false)
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    #[test]
    fn a_focused_well_reserves_one_cell_for_its_caret() {
        assert_eq!(well_text("10", true), "[10            ]");
        assert_eq!(well_text("10", false), "[10            ]");
        assert_eq!(well_text("", true), "[              ]");
    }

    #[test]
    fn a_long_value_scrolls_its_tail_and_keeps_the_caret_inside() {
        let text = well_text("123456789012345678", true);
        assert_eq!(text, "[6789012345678 ]");
        assert_eq!(crate::tui::format::display_width(&text), 16);
    }

    #[test]
    fn segments_render_markers_and_target_only_enabled_options() {
        let options = vec![
            Segment::new("MARKET", false),
            Segment::new("LIMIT", true),
            Segment::disabled("SL"),
        ];
        let (spans, hits) = segment_spans(&options, true);
        let text: String = spans.iter().map(|span| span.content.as_ref()).collect();
        assert_eq!(text, "[ MARKET |*LIMIT |-SL ]");
        assert_eq!(hits.len(), 2, "the disabled option has no target");
        assert_eq!(hits[0], (1, 8, 0));
        assert_eq!(hits[1], (10, 7, 1));
    }

    #[test]
    fn wrapping_holds_wide_and_combining_text_inside_the_field() {
        assert_eq!(wrap_cells("東京", 1), ["東", "京"]);
        let combining = "e\u{301}e\u{301}e\u{301}e\u{301}";
        for row in wrap_cells(combining, 3) {
            assert!(crate::tui::format::display_width(&row) <= 3, "{row:?}");
        }
    }

    #[test]
    fn wrapping_measures_display_cells_and_keeps_every_word() {
        assert_eq!(wrap_cells("one two three", 7), ["one two", "three"]);
        assert_eq!(wrap_cells("", 10), [""]);
        assert_eq!(wrap_cells("abcdefghij", 4), ["abcd", "efgh", "ij"]);
        assert_eq!(wrap_cells("東京証券", 4), ["東京", "証券"]);
    }
}
