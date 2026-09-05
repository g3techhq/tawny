//! Shared overlay scroll-lock helper.
use dioxus::prelude::{Signal, provide_context, try_consume_context};
#[cfg(target_arch = "wasm32")]
use dioxus::prelude::{use_drop, use_effect};
/// Playground previews contain overlays inside a device frame, so those
/// overlays must not lock the real document surrounding the frame.
#[derive(Clone, Copy)]
struct DisableBodyScrollLock;
pub(crate) fn disable_body_scroll_lock_for_subtree() {
    provide_context(DisableBodyScrollLock);
}
pub(crate) fn use_lock_body_scroll(locked: Signal<bool>) {
    let disabled = try_consume_context::<DisableBodyScrollLock>().is_some();
    #[cfg(target_arch = "wasm32")]
    {
        use_effect(move || {
            if !disabled {
                set_body_scroll_locked(locked());
            }
        });
        use_drop(move || {
            if !disabled {
                set_body_scroll_locked(false);
            }
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (locked, disabled);
    }
}
#[cfg(target_arch = "wasm32")]
fn set_body_scroll_locked(locked: bool) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(body) = document.body() else {
        return;
    };
    let _ = body
        .class_list()
        .toggle_with_force("g3-overlay-scroll-locked", locked);
}
