//! Sheet component - bottom and side sheet with drag-to-dismiss and platform styling.
use super::overlay_scroll::use_lock_body_scroll;
use super::sheet_styles as s;
use crate::theme::{ComponentMode, merge_classes, use_component_mode};
use dioxus::prelude::*;
#[cfg(feature = "playground")]
use dioxus_icons::lucide::Menu;
use std::sync::atomic::{AtomicU64, Ordering};
const SHEET_DISMISS_DISTANCE: f64 = 96.0;
static SHEET_INSTANCE_ID: AtomicU64 = AtomicU64::new(0);
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
/// How a side sheet interacts with the app content beside it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SideSheetType {
    /// Slide above the app content without moving it.
    #[default]
    Overlay,
    /// Slide in while moving the app content by the same distance.
    Push,
    /// Stay beneath the app content while the content moves away to reveal it.
    Reveal,
    /// Reserve space beside the complete app page as persistent navigation.
    Menu,
}
/// Which edge a sheet uses and, for side sheets, how it affects app content.
///
/// `Push`, `Reveal`, and `Menu` side sheets should be direct children of
/// `AppWrapper`, beside one root content element, matching Ionic's
/// menu/content structure.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum SheetPlacement {
    /// Rises from the bottom - the standard mobile action sheet.
    #[default]
    Bottom,
    /// Slides in from the leading edge, as a navigation drawer does.
    Left(SideSheetType),
    /// Slides in from the trailing edge, for inspectors and filters.
    Right(SideSheetType),
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
    let (placement_cls, side_type_cls) = match placement {
        SheetPlacement::Bottom => (s::SHEET_BOTTOM, ""),
        SheetPlacement::Left(side_type) => (s::SHEET_LEFT, side_sheet_type_class(side_type)),
        SheetPlacement::Right(side_type) => (s::SHEET_RIGHT, side_sheet_type_class(side_type)),
    };
    let side_cls = (!side_type_cls.is_empty())
        .then_some(s::SHEET_SIDE)
        .unwrap_or_default();
    let is_menu = matches!(
        placement,
        SheetPlacement::Left(SideSheetType::Menu) | SheetPlacement::Right(SideSheetType::Menu)
    );
    let sheet_cls = format!(
        "{} {mode_cls} {placement_cls} {side_cls} {side_type_cls}",
        s::SHEET,
    );
    let has_handle = matches!(placement, SheetPlacement::Bottom);
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
        "{} {} {placement_cls} {side_cls} {side_type_cls}",
        s::BACKDROP,
        if visual_open {
            "g3-sheet-backdrop-open"
        } else {
            "g3-sheet-backdrop-closed"
        },
    );
    let state_cls = if visual_open {
        s::STATE_OPEN
    } else {
        s::STATE_CLOSED
    };
    rsx! {
        if !is_menu {
            button {
                r#type: "button",
                aria_label: "Close sheet",
                aria_hidden: (!is_open_now).to_string(),
                tabindex: if is_open_now { "0" } else { "-1" },
                class: backdrop_cls,
                onpointerdown: move |_| is_open.set(false),
                onclick: move |_| is_open.set(false),
            }
        }
        div {
            id: dialog_id,
            role: if is_menu { "navigation" } else { "dialog" },
            aria_modal: (!is_menu).to_string(),
            aria_label: if is_menu { "Menu" } else { "Sheet" },
            aria_hidden: (!
                    is_open_now).to_string(),
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
fn side_sheet_type_class(side_type: SideSheetType) -> &'static str {
    match side_type {
        SideSheetType::Overlay => s::SHEET_OVERLAY,
        SideSheetType::Push => s::SHEET_PUSH,
        SideSheetType::Reveal => s::SHEET_REVEAL,
        SideSheetType::Menu => s::SHEET_MENU,
    }
}
#[cfg(feature = "playground")]
#[component]
pub fn SheetPlaygroundDemo() -> Element {
    let mut open = use_signal(|| false);
    let placement_index = use_signal(|| 0_usize);
    let side_type_index = use_signal(|| 0_usize);
    let side_type = match side_type_index() {
        1 => SideSheetType::Push,
        2 => SideSheetType::Reveal,
        3 => SideSheetType::Menu,
        _ => SideSheetType::Overlay,
    };
    let placement = match placement_index() {
        1 => SheetPlacement::Left(side_type),
        2 => SheetPlacement::Right(side_type),
        _ => SheetPlacement::Bottom,
    };
    rsx! {
        crate::PlaygroundDemoFrame {
            app: false,
            controls: rsx! {
                div {
                    span { "Placement" }
                    crate::SegmentGroup { active: placement_index,
                        crate::SegmentButton { index: 0, "Bottom" }
                        crate::SegmentButton { index: 1, "Left" }
                        crate::SegmentButton { index: 2, "Right" }
                    }
                }
                if placement_index() != 0 {
                    div {
                        span { "Side type" }
                        crate::SegmentGroup { active: side_type_index,
                            crate::SegmentButton { index: 0, "Overlay" }
                            crate::SegmentButton { index: 1, "Push" }
                            crate::SegmentButton { index: 2, "Reveal" }
                            crate::SegmentButton { index: 3, "Menu" }
                        }
                    }
                }
                crate::Checkbox { checked: open, label: "Open"
                            .to_string() }
            },
            crate::AppWrapper { class: "g3-playground-device-app",
                div { class: "g3-sheet-demo-content-root",
                    crate::Header {
                        title: "Side sheets",
                        start_button: rsx! {
                            crate::Button {
                                style: crate::ButtonStyle::Clear,
                                size: crate::ButtonSize::Sm,
                                aria_label: "Toggle menu".to_string(),
                                onclick: move |_| open.toggle(),
                                Menu { size: 22, class: "fill-none" }
                            }
                        },
                    }
                    crate::Body { has_footer_space: false,
                        crate::Card { title: "Round settings",
                            "Push moves this page. Menu preserves its complete layout beside a desktop rail."
                        }
                        crate::Button { onclick: move |_| open.set(true), "Open sheet" }
                    }
                }
                Sheet {
                    is_open: open,
                    placement,
                    class: "g3-sheet-demo-surface",
                    div { class: "g3-sheet-demo-menu",
                        div { class: "g3-sheet-demo-menu-header",
                            span { class: "g3-sheet-demo-menu-eyebrow", "Fairway" }
                            strong { "Round menu" }
                            span { "Choose a destination. Wide menus close from the header toggle." }
                        }
                        crate::List { lines: crate::ListLines::Full,
                            crate::Item { label: "Scorecard", metadata: "12 / 18" }
                            crate::Item { label: "Players", metadata: "4" }
                            crate::Item { label: "Round settings" }
                        }
                    }
                }
            }
        }
    }
}
crate::g3_playground! {
    name: "Sheet",
    description: "Bottom sheet plus overlay, push, reveal, and persistent menu side sheets.",
    demo: SheetPlaygroundDemo,
    source: "src/components/sheet.rs",
}
