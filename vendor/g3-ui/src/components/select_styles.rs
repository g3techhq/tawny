//! Style constants for Select component.
#![allow(dead_code)]
pub const SELECT_BTN: &str = "g3-select-btn";
pub const SELECT_BTN_IOS: &str = "g3-select-btn-ios";
pub const SELECT_BTN_MD: &str = "g3-select-btn-md";
pub const SELECT_VALUE: &str = "g3-select-value";
pub const SELECT_ICON: &str = "g3-select-icon";
pub const SELECT_SHEET: &str = "g3-select-sheet";
pub const OPTION: &str = "g3-select-option";
pub const OPTION_SELECTED: &str = "g3-select-option-selected";
pub const SEPARATOR: &str = "g3-select-separator";
pub fn catalog() -> Vec<(&'static str, &'static str)> {
    vec![
        ("SELECT_BTN", SELECT_BTN),
        ("SELECT_BTN_IOS", SELECT_BTN_IOS),
        ("SELECT_BTN_MD", SELECT_BTN_MD),
        ("SELECT_VALUE", SELECT_VALUE),
        ("SELECT_ICON", SELECT_ICON),
        ("SELECT_SHEET", SELECT_SHEET),
        ("OPTION", OPTION),
        ("OPTION_SELECTED", OPTION_SELECTED),
        ("SEPARATOR", SEPARATOR),
    ]
}
