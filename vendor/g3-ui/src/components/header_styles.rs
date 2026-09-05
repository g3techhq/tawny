//! Style constants for Header component.
#![allow(dead_code)]
pub const HEADER_BASE: &str = "g3-header shadow-lg sticky top-0 z-10";
pub const HEADER_IOS: &str = "g3-header-ios";
pub const HEADER_MD: &str = "g3-header-md";
pub const HEADER_WITH_TOOLBAR: &str = "g3-header-with-toolbar";
pub const HEADER_ROW: &str = "g3-header-row";
pub const HEADER_TITLE: &str = "g3-header-title";
pub const HEADER_TITLE_TEXT: &str = "g3-header-title-text";
pub const HEADER_SLOT: &str = "g3-header-slot";
pub const HEADER_START_SLOT: &str = "g3-header-slot g3-header-start-slot";
pub const HEADER_END_SLOT: &str = "g3-header-slot g3-header-end-slot";
pub const TOOLBAR: &str = "g3-header-toolbar";
pub fn catalog() -> Vec<(&'static str, &'static str)> {
    vec![
        ("HEADER_BASE", HEADER_BASE),
        ("HEADER_IOS", HEADER_IOS),
        ("HEADER_MD", HEADER_MD),
        ("HEADER_WITH_TOOLBAR", HEADER_WITH_TOOLBAR),
        ("HEADER_ROW", HEADER_ROW),
        ("HEADER_TITLE", HEADER_TITLE),
        ("HEADER_TITLE_TEXT", HEADER_TITLE_TEXT),
        ("HEADER_START_SLOT", HEADER_START_SLOT),
        ("HEADER_END_SLOT", HEADER_END_SLOT),
        ("TOOLBAR", TOOLBAR),
    ]
}
