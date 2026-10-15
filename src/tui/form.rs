use crate::tui::app::{AlertFormFocus, AtoItemFocus};
use crate::tui::ticket::{ConversionField, TicketField};
use std::cell::Cell;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormIssue<F> {
    pub field: Option<F>,
    pub message: String,
}

impl<F> FormIssue<F> {
    pub fn at(field: F, message: impl Into<String>) -> Self {
        Self {
            field: Some(field),
            message: message.into(),
        }
    }

    pub fn general(message: impl Into<String>) -> Self {
        Self {
            field: None,
            message: message.into(),
        }
    }
}

#[derive(Debug, Default)]
pub struct FormViewport {
    scroll: Cell<u16>,
    height: Cell<u16>,
    content: Cell<u16>,
    reveal: Cell<bool>,
    drawn: Cell<bool>,
}

impl FormViewport {
    pub fn set_drawn(&self, drawn: bool) {
        self.drawn.set(drawn);
    }

    pub fn drawn(&self) -> bool {
        self.drawn.get()
    }

    pub fn height(&self) -> u16 {
        self.height.get().max(1)
    }

    pub fn request_reveal(&self) {
        self.reveal.set(true);
    }

    pub fn scroll_by(&self, delta: isize) {
        let max = self.max_scroll();
        let next = (i64::from(self.scroll.get()) + delta as i64).clamp(0, i64::from(max));
        self.scroll
            .set(u16::try_from(next).expect("bounded TUI value fits u16"));
        self.reveal.set(false);
    }

    pub fn page(&self, dir: i8) {
        let step =
            isize::try_from(self.height().saturating_sub(1).max(1)).expect("TUI index fits isize");
        self.scroll_by(step * isize::from(dir));
    }

    fn max_scroll(&self) -> u16 {
        self.content.get().saturating_sub(self.height())
    }

    pub fn resolve(&self, content: u16, height: u16, focus: Option<(u16, u16)>) -> u16 {
        self.content.set(content);
        self.height.set(height);
        let height = height.max(1);
        let max = content.saturating_sub(height);
        let mut scroll = self.scroll.get().min(max);
        if self.reveal.replace(false)
            && let Some((top, bottom)) = focus
        {
            if top < scroll {
                scroll = top;
            } else if bottom >= scroll + height {
                scroll = bottom.saturating_sub(height - 1);
            }
            scroll = scroll.min(max);
        }
        self.scroll.set(scroll);
        scroll
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlIntent {
    Focus,
    Option(usize),
    Previous,
    Next,
}

impl ControlIntent {
    pub fn pick(self, current: usize, count: usize) -> Option<usize> {
        if count == 0 {
            return None;
        }
        let next = match self {
            ControlIntent::Option(index) if index < count => index,
            ControlIntent::Focus | ControlIntent::Option(_) => return None,
            ControlIntent::Previous => {
                (current.cast_signed() - 1).rem_euclid(count.cast_signed()) as usize
            }
            ControlIntent::Next => {
                (current.cast_signed() + 1).rem_euclid(count.cast_signed()) as usize
            }
        };
        (next != current).then_some(next)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalField {
    Ticket(TicketField),
    Conversion(ConversionField),
    Alert(AlertFormFocus),
    AtoItem(AtoItemFocus),
}

#[cfg(test)]
mod tests {
    use super::{FormIssue, FormViewport};
    use crate::tui::ticket::TicketField;

    #[test]
    fn an_issue_carries_its_field_or_none() {
        let field = FormIssue::at(TicketField::Quantity, "quantity must be positive");
        assert_eq!(field.field, Some(TicketField::Quantity));
        let general: FormIssue<TicketField> = FormIssue::general("session expired");
        assert_eq!(general.field, None);
        assert_eq!(general.message, "session expired");
    }

    #[test]
    fn viewport_clamps_scroll_to_the_measured_body() {
        let viewport = FormViewport::default();
        assert_eq!(viewport.resolve(20, 10, None), 0);
        viewport.scroll_by(50);
        assert_eq!(
            viewport.resolve(20, 10, None),
            10,
            "clamped to content minus height"
        );
        viewport.scroll_by(-50);
        assert_eq!(viewport.resolve(20, 10, None), 0);
    }

    #[test]
    fn a_reveal_pulls_the_focused_row_into_view_once() {
        let viewport = FormViewport::default();
        viewport.resolve(30, 10, None);
        viewport.request_reveal();
        assert_eq!(viewport.resolve(30, 10, Some((25, 26))), 17);
        viewport.scroll_by(-5);
        assert_eq!(viewport.resolve(30, 10, Some((25, 26))), 12);
    }

    #[test]
    fn a_shrinking_body_pulls_the_offset_back() {
        let viewport = FormViewport::default();
        viewport.resolve(40, 10, None);
        viewport.scroll_by(30);
        assert_eq!(viewport.resolve(12, 10, None), 2);
    }

    #[test]
    fn a_body_shorter_than_its_frame_never_scrolls() {
        let viewport = FormViewport::default();
        viewport.scroll_by(5);
        assert_eq!(viewport.resolve(4, 10, None), 0);
    }
}
