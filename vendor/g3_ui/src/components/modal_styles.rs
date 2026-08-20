//! Style constants for ConfirmModal component.
#![allow(dead_code)]

pub const MODAL: &str = "g3-modal";
pub const MODAL_IOS: &str = "g3-modal-ios";
pub const MODAL_MD: &str = "g3-modal-md";
// Plain custom class, not Tailwind utilities: this backdrop's positioning
// and dimming live in g3_ui.css's own `.g3-modal-overlay` rule so Modal
// works standalone, regardless of whether a consuming app's Tailwind
// `@source` scan covers this crate's source (most don't, since it lives
// outside the app's own `src/`).
pub const OVERLAY: &str = "g3-modal-overlay";
pub const ACTIONS: &str = "g3-modal-actions";
pub const TITLE: &str = "g3-modal-title";

pub fn catalog() -> Vec<(&'static str, &'static str)> {
    vec![
        ("MODAL", MODAL),
        ("MODAL_IOS", MODAL_IOS),
        ("MODAL_MD", MODAL_MD),
    ]
}
