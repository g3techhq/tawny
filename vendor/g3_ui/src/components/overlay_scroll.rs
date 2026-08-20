//! Shared overlay scroll-lock helper.

use dioxus::prelude::Signal;

#[cfg(target_arch = "wasm32")]
use dioxus::prelude::{use_drop, use_effect};

pub(crate) fn use_lock_body_scroll(locked: Signal<bool>) {
    #[cfg(target_arch = "wasm32")]
    {
        use_effect(move || set_body_scroll_locked(locked()));
        use_drop(|| set_body_scroll_locked(false));
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = locked;
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
