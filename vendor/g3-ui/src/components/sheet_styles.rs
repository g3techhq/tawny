//! Style constants for Sheet component.
#![allow(dead_code)]
pub const SHEET: &str = "g3-sheet";
pub const SHEET_IOS: &str = "g3-sheet-ios";
pub const SHEET_MD: &str = "g3-sheet-md";
pub const SHEET_BOTTOM: &str = "g3-sheet-bottom";
pub const SHEET_LEFT: &str = "g3-sheet-left";
pub const SHEET_RIGHT: &str = "g3-sheet-right";
pub const SHEET_SIDE: &str = "g3-sheet-side";
pub const SHEET_OVERLAY: &str = "g3-sheet-overlay";
pub const SHEET_PUSH: &str = "g3-sheet-push";
pub const SHEET_REVEAL: &str = "g3-sheet-reveal";
pub const SHEET_MENU: &str = "g3-sheet-menu";
pub const HANDLE_WRAP_IOS: &str = "g3-sheet-handle-wrap-ios flex justify-center cursor-grab active:cursor-grabbing touch-none select-none";
pub const HANDLE_IOS: &str = "g3-sheet-handle-ios";
pub const BACKDROP: &str = "g3-sheet-backdrop";
pub const STATE_OPEN: &str = "g3-sheet-open";
pub const STATE_CLOSED: &str = "g3-sheet-closed";
pub const CONTENT: &str = "g3-sheet-content";
pub fn catalog() -> Vec<(&'static str, &'static str)> {
    vec![
        ("SHEET", SHEET),
        ("SHEET_IOS", SHEET_IOS),
        ("SHEET_MD", SHEET_MD),
        ("SHEET_BOTTOM", SHEET_BOTTOM),
        ("SHEET_LEFT", SHEET_LEFT),
        ("SHEET_RIGHT", SHEET_RIGHT),
        ("SHEET_SIDE", SHEET_SIDE),
        ("SHEET_OVERLAY", SHEET_OVERLAY),
        ("SHEET_PUSH", SHEET_PUSH),
        ("SHEET_REVEAL", SHEET_REVEAL),
        ("SHEET_MENU", SHEET_MENU),
        ("HANDLE_WRAP_IOS", HANDLE_WRAP_IOS),
        ("HANDLE_IOS", HANDLE_IOS),
        ("BACKDROP", BACKDROP),
        ("STATE_OPEN", STATE_OPEN),
        ("STATE_CLOSED", STATE_CLOSED),
        ("CONTENT", CONTENT),
    ]
}
