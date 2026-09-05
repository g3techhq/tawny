//! Style constants for ConfirmModal component.
#![allow(dead_code)]
pub const MODAL: &str = "g3-modal";
pub const MODAL_IOS: &str = "g3-modal-ios";
pub const MODAL_MD: &str = "g3-modal-md";
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
