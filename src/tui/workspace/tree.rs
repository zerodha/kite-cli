use super::{PaneId, PaneRect, Seam};
use crate::tui::split::{Axis, Split, SplitRatios};
use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Extent {
    Ratio {
        bps: u16,
        minimums: (u16, u16),
    },
    Fraction {
        numerator: u16,
        denominator: u16,
        minimums: (u16, u16),
    },
    Content {
        length: u16,
        reserve: u16,
    },
}

impl Extent {
    pub(super) fn length(self, total: u16) -> u16 {
        match self {
            Extent::Ratio {
                bps,
                minimums: (first, second),
            } => {
                let want = u16::try_from((u32::from(total) * u32::from(bps) + 5_000) / 10_000)
                    .unwrap_or(u16::MAX);
                if total < first.saturating_add(second) {
                    want.min(total)
                } else {
                    want.clamp(first, total - second)
                }
            }
            Extent::Fraction {
                numerator,
                denominator,
                minimums: (first, second),
            } => {
                let want = u16::try_from(
                    u32::from(total) * u32::from(numerator) / u32::from(denominator.max(1)),
                )
                .unwrap_or(u16::MAX);
                if total < first.saturating_add(second) {
                    want.min(total)
                } else {
                    want.clamp(first, total - second)
                }
            }
            Extent::Content { length, reserve } => length.min(total.saturating_sub(reserve)),
        }
    }
}

pub(super) enum Node {
    Leaf(PaneId),
    Split {
        split: Option<Split>,
        axis: Axis,
        first: Extent,
        children: (Box<Node>, Box<Node>),
    },
}

pub(super) fn resolve_split(
    node: &Node,
    area: Rect,
    ratios: &SplitRatios,
    panes: &mut Vec<PaneRect>,
    seams: &mut Vec<Seam>,
) {
    match node {
        Node::Leaf(id) => panes.push(PaneRect {
            id: *id,
            outer: area,
        }),
        Node::Split {
            split,
            axis,
            first,
            children,
        } => {
            let total = match axis {
                Axis::Horizontal => area.width,
                Axis::Vertical => area.height,
            };
            let fallback = first.length(total);
            let length = match split {
                Some(split) => ratios.length(*split, total, fallback),
                None => fallback,
            }
            .min(total);
            let (head, tail) = divide(area, *axis, length);
            resolve_split(&children.0, head, ratios, panes, seams);
            resolve_split(&children.1, tail, ratios, panes, seams);
            if let Some(split) = split {
                seams.push(Seam {
                    split: *split,
                    span: area,
                    first: head,
                    second: tail,
                });
            }
        }
    }
}

fn divide(area: Rect, axis: Axis, first: u16) -> (Rect, Rect) {
    match axis {
        Axis::Horizontal => (
            Rect {
                width: first,
                ..area
            },
            Rect {
                x: area.x.saturating_add(first),
                width: area.width - first,
                ..area
            },
        ),
        Axis::Vertical => (
            Rect {
                height: first,
                ..area
            },
            Rect {
                y: area.y.saturating_add(first),
                height: area.height - first,
                ..area
            },
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::Extent;

    #[test]
    fn a_ratio_clamps_off_both_minimums_and_gives_up_when_neither_fits() {
        let extent = Extent::Ratio {
            bps: 5_000,
            minimums: (40, 24),
        };
        assert_eq!(extent.length(128), 64);
        assert_eq!(extent.length(70), 40);
        assert_eq!(extent.length(60), 30);
        assert_eq!(extent.length(0), 0);
    }

    #[test]
    fn a_content_extent_is_capped_by_its_reserve_and_never_raised() {
        let extent = Extent::Content {
            length: 7,
            reserve: 6,
        };
        assert_eq!(extent.length(25), 7);
        assert_eq!(extent.length(12), 6);
        assert_eq!(extent.length(4), 0, "a short axis shrinks the first child");
        let empty = Extent::Content {
            length: 5,
            reserve: 6,
        };
        assert_eq!(empty.length(25), 5, "a five-row book is not padded to six");
    }
}
