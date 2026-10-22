use super::layout::{SELECT_MARKER, draw_body_line, draw_select_list, section_line};
use crate::tui::app::{App, WorkspacePickerState};
use crate::tui::format;
use crate::tui::ui::forms;
use crate::tui::ui::{HitList, always, nav};
use crate::tui::workspace::{PaneId, Preset};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::ListItem;

#[expect(
    clippy::cast_possible_truncation,
    reason = "fixed preset count is far below u16::MAX"
)]
const PICKER_BODY_ROWS: u16 = 1 + Preset::COUNT as u16;

const PICKER_TOO_SMALL: &str = "Esc closes · 1-6 still switch.";

pub(in crate::tui::ui) fn workspace_picker_footer() -> &'static forms::FormFooter {
    const {
        &forms::FormFooter {
            help: &["j/k or Up/Down Select", "1-6 Switch"],
            actions: &[
                forms::FormAction {
                    key: "Enter",
                    label: "Switch",
                    kind: forms::ActionKind::Primary,
                },
                forms::FormAction {
                    key: "Esc",
                    label: "Close",
                    kind: forms::ActionKind::Secondary,
                },
            ],
            hints: &[
                nav(&["j/k:select", "1-6:switch"]),
                always(&["Enter:switch", "Esc:close"]),
            ],
        }
    }
}

fn picker_row(preset: Preset, content: u16) -> Line<'static> {
    let panes: Vec<&str> = preset.panes().iter().copied().map(PaneId::title).collect();
    let lead = format!("{}  {:<13}", preset.key(), preset.title());
    let room = usize::from(content)
        .saturating_sub(SELECT_MARKER.len())
        .saturating_sub(format::display_width(&lead));
    Line::from(vec![
        Span::styled(lead, format::bold()),
        Span::styled(format::pad_or_clip(&panes.join(" · "), room), format::dim()),
    ])
}

pub(super) fn draw_workspace_picker(frame: &mut Frame, state: WorkspacePickerState, app: &App) {
    let layout = forms::ModalLayout::measure(frame.area(), PICKER_BODY_ROWS);
    let Some(layout) = forms::draw_frame(
        frame,
        app,
        layout,
        "Workspaces",
        Style::default().fg(format::accent()),
        &format!("on screen: {}", app.workspace().title()),
        PICKER_TOO_SMALL,
    ) else {
        return;
    };
    let body = layout.body;
    draw_body_line(
        frame,
        body,
        0,
        section_line("Switch workspace", layout.content),
    );
    let shown = body.height.saturating_sub(1).min(PICKER_BODY_ROWS - 1);
    if shown > 0 {
        let items: Vec<ListItem> = Preset::ALL
            .into_iter()
            .map(|preset| ListItem::new(picker_row(preset, layout.content)))
            .collect();
        let list_area = Rect {
            y: body.y + 1,
            height: shown,
            ..body
        };
        draw_select_list(
            frame,
            app,
            list_area,
            items,
            Some(state.selected.min(Preset::COUNT - 1)),
            HitList::WorkspacePicker,
            SELECT_MARKER,
        );
    }
    forms::draw_footer(frame, app, &layout, workspace_picker_footer());
}
