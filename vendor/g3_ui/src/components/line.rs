//! Line (divider/separator) component.

use super::line_styles as s;
use crate::theme::merge_classes;
use dioxus::prelude::*;

/// Orientation of the line separator.
#[derive(Clone, Copy, PartialEq, Default)]
pub enum LineOrientation {
    /// A rule running left to right, separating stacked content.
    #[default]
    Horizontal,
    /// A rule running top to bottom, separating side-by-side content.
    Vertical,
}

#[component]
pub fn Line(
    orientation: Option<LineOrientation>,
    /// Add symmetric spacing around the line. On a horizontal line this is
    /// vertical (block) margin; on a vertical line it is horizontal (inline)
    /// margin, so a divider does not sit flush against its neighbours.
    margins: Option<bool>,
    class: Option<String>,
) -> Element {
    let orient = orientation.unwrap_or_default();
    let base_cls = match orient {
        LineOrientation::Horizontal => s::LINE_H,
        LineOrientation::Vertical => s::LINE_V,
    };
    let margins_cls = margins.unwrap_or(false).then_some(s::LINE_MARGINS);

    let cls = merge_classes(
        merge_classes(format!("{} {base_cls}", s::LINE), margins_cls),
        class.as_deref(),
    );
    let aria_orientation = match orient {
        LineOrientation::Horizontal => "horizontal",
        LineOrientation::Vertical => "vertical",
    };

    rsx! {
        hr { class: cls, role: "separator", aria_orientation }
    }
}
#[cfg(feature = "playground")]
#[component]
pub fn LinePlaygroundDemo() -> Element {
    let vertical = use_signal(|| false);
    let margins = use_signal(|| false);
    rsx! {
        crate::PlaygroundDemoFrame {
            controls: rsx! {
                crate::Checkbox { checked: vertical, label: "Vertical".to_string() }
                crate::Checkbox { checked: margins, label: "Margins".to_string() }
            },
            div { class: if vertical() { "g3-line-demo-surface g3-line-demo-surface-vertical" } else { "g3-line-demo-surface g3-line-demo-surface-horizontal" },
                Line {
                    orientation: if vertical() { LineOrientation::Vertical } else { LineOrientation::Horizontal },
                    margins: margins(),
                }
            }
        }
    }
}
crate::g3_playground! {
    name: "Line",
    description: "Horizontal or vertical separator.",
    demo: LinePlaygroundDemo,
    source: "src/components/line.rs",
}
