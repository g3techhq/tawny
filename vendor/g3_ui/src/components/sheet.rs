//! Sheet component - bottom and side sheet with drag-to-dismiss and platform styling.

use super::overlay_scroll::use_lock_body_scroll;
use super::sheet_styles as s;
use crate::theme::{ComponentMode, merge_classes, use_component_mode};
use dioxus::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

const SHEET_DISMISS_DISTANCE: f64 = 96.0;

static SHEET_INSTANCE_ID: AtomicU64 = AtomicU64::new(0);

// Drag tracking runs entirely in JS (attached directly to the handle/dialog
// DOM nodes) so per-frame pointer movement never has to round-trip through
// Dioxus re-renders. Only the final dismiss/no-dismiss decision is sent back
// to Rust. This also sidesteps a dioxus-interpreter-js quirk where clearing
// the `style` attribute back to "" restores previously-set inline properties
// instead of removing them, which left `--g3-sheet-drag-y` stuck after a
// drag-to-dismiss and made the next open animate in short of fully open.
const SHEET_PRESENT_SCRIPT: &str = r#"
requestAnimationFrame(() => dioxus.send(true));
"#;

const SHEET_DRAG_SCRIPT: &str = r#"
const dialog = document.getElementById("__DIALOG_ID__");
const handle = document.getElementById("__HANDLE_ID__");
if (dialog && handle && handle.dataset.g3SheetDragBound !== "true") {
    handle.dataset.g3SheetDragBound = "true";

    let dragging = false;
    let startY = 0;
    let deltaY = 0;
    const DISMISS_DISTANCE = __DISMISS_DISTANCE__;

    const onDown = (e) => {
        dragging = true;
        startY = e.clientY;
        deltaY = 0;
        dialog.style.setProperty("transition", "none");
        dialog.style.setProperty("touch-action", "none");
        try { handle.setPointerCapture(e.pointerId); } catch (err) {}
        e.preventDefault();
    };

    const onMove = (e) => {
        if (!dragging) return;
        deltaY = Math.max(0, e.clientY - startY);
        dialog.style.setProperty("--g3-sheet-drag-y", deltaY + "px");
        e.preventDefault();
    };

    const onEnd = () => {
        if (!dragging) return;
        dragging = false;
        dialog.style.removeProperty("transition");
        dialog.style.removeProperty("touch-action");
        dialog.style.setProperty("--g3-sheet-drag-y", "0px");
        if (deltaY > DISMISS_DISTANCE) {
            dioxus.send(true);
        }
    };

    handle.addEventListener("pointerdown", onDown);
    handle.addEventListener("pointermove", onMove);
    handle.addEventListener("pointerup", onEnd);
    handle.addEventListener("pointercancel", onEnd);
}
"#;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum SheetPlacement {
    #[default]
    Bottom,
    Left,
    Right,
}

#[component]
pub fn Sheet(
    mut is_open: Signal<bool>,
    placement: Option<SheetPlacement>,
    draggable: Option<bool>,
    class: Option<String>,
    mode: Option<ComponentMode>,
    children: Element,
) -> Element {
    let mode = use_component_mode(mode);
    let placement = placement.unwrap_or_default();
    let is_draggable = draggable.unwrap_or(true);
    let instance_id = use_hook(|| SHEET_INSTANCE_ID.fetch_add(1, Ordering::Relaxed));

    let mode_cls = match mode {
        ComponentMode::Ios => s::SHEET_IOS,
        ComponentMode::Md => s::SHEET_MD,
    };
    let placement_cls = match placement {
        SheetPlacement::Bottom => s::SHEET_BOTTOM,
        SheetPlacement::Left => s::SHEET_LEFT,
        SheetPlacement::Right => s::SHEET_RIGHT,
    };

    let sheet_cls = format!("{} {mode_cls} {placement_cls}", s::SHEET);
    let has_handle = placement == SheetPlacement::Bottom;
    let dialog_id = format!("g3-sheet-{instance_id}");
    let handle_id = format!("g3-sheet-handle-{instance_id}");
    let mut ever_opened = use_signal(|| false);
    let mut presented_open = use_signal(|| false);

    use_effect(move || {
        if !is_open() {
            presented_open.set(false);
            return;
        }
        ever_opened.set(true);
        if presented_open() {
            return;
        }
        spawn(async move {
            let mut eval = document::eval(SHEET_PRESENT_SCRIPT);
            let _ = eval.recv::<bool>().await;
            if is_open() {
                presented_open.set(true);
            }
        });
    });

    {
        let dialog_id = dialog_id.clone();
        let handle_id = handle_id.clone();
        use_effect(move || {
            if !(is_open() && is_draggable && has_handle) {
                return;
            }
            let script = SHEET_DRAG_SCRIPT
                .replace("__DIALOG_ID__", &dialog_id)
                .replace("__HANDLE_ID__", &handle_id)
                .replace("__DISMISS_DISTANCE__", &SHEET_DISMISS_DISTANCE.to_string());
            spawn(async move {
                let mut eval = document::eval(&script);
                while let Ok(true) = eval.recv::<bool>().await {
                    is_open.set(false);
                }
            });
        });
    }

    use_lock_body_scroll(is_open);
    let is_open_now = is_open();
    if !is_open_now && !ever_opened() {
        return rsx! {};
    }
    let visual_open = is_open_now && presented_open();

    let backdrop_cls = format!(
        "{} {}",
        s::BACKDROP,
        if visual_open {
            "g3-sheet-backdrop-open"
        } else {
            "g3-sheet-backdrop-closed pointer-events-none"
        }
    );
    let state_cls = if visual_open {
        s::STATE_OPEN
    } else {
        s::STATE_CLOSED
    };

    rsx! {
        button {
            r#type: "button",
            aria_label: "Sheet backdrop",
            class: format!("{backdrop_cls} appearance-none border-0 p-0"),
            onclick: move |_| is_open.set(false),
        }
        div {
            id: dialog_id,
            role: "dialog",
            aria_modal: "true",
            aria_label: "Sheet",
            aria_hidden: (!is_open_now).to_string(),
            inert: (!is_open_now).then(|| "".to_string()),
            class: merge_classes(format!("{sheet_cls} {state_cls}"), class.as_deref()),
            if is_draggable && has_handle {
                button {
                    id: handle_id,
                    r#type: "button",
                    class: s::HANDLE_WRAP_IOS,
                    aria_label: "Sheet handle",
                    div { class: s::HANDLE_IOS }
                }
            }
            div { class: s::CONTENT, {children} }
        }
    }
}
#[cfg(feature = "playground")]
#[component]
pub fn SheetPlaygroundDemo() -> Element {
    let mut open = use_signal(|| false);
    let placement_index = use_signal(|| 0_usize);
    let placement = match placement_index() {
        1 => SheetPlacement::Left,
        2 => SheetPlacement::Right,
        _ => SheetPlacement::Bottom,
    };
    rsx! {
        crate::PlaygroundDemoFrame {
            controls: rsx! {
                div {
                    span { "Placement" }
                    crate::SegmentGroup { active: placement_index,
                        crate::SegmentButton { index: 0, "Bottom" }
                        crate::SegmentButton { index: 1, "Left" }
                        crate::SegmentButton { index: 2, "Right" }
                    }
                }
                crate::Checkbox { checked: open, label: "Open".to_string() }
            },
            crate::Button { onclick: move |_| open.set(true), "Open sheet" }
            Sheet { is_open: open, placement,
                crate::List { inset: true, lines: crate::ListLines::None,
                    crate::Item {
                        label: "Round settings",
                        description: "Use the handle or backdrop to close.",
                    }
                    crate::Item { label: "Tee time", metadata: "9:40" }
                    crate::Item { label: "Players", metadata: "4" }
                }
            }
        }
    }
}
crate::g3_playground! {
    name: "Sheet",
    description: "Bottom and side sheet with backdrop dismissal.",
    demo: SheetPlaygroundDemo,
    source: "src/components/sheet.rs",
}
