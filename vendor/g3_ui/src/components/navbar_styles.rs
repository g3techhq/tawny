//! Style constants for Navbar component.
#![allow(dead_code)]

pub const NAVBAR_BASE: &str = "g3-navbar flex min-h-0 flex-1 flex-col overflow-hidden";
pub const NAVBAR_IOS: &str = "g3-navbar-ios";
pub const NAVBAR_MD: &str = "g3-navbar-md";
pub const TAB_BAR: &str = "g3-navbar-tab-bar";
pub const TAB: &str = "g3-navbar-tab";
pub const TAB_SELECTED: &str = "g3-navbar-tab-selected";
pub const TAB_DISABLED: &str = "g3-navbar-tab-disabled";
pub const TAB_ICON: &str = "g3-navbar-tab-icon";
pub const TAB_LABEL: &str = "g3-navbar-tab-label";

pub fn catalog() -> Vec<(&'static str, &'static str)> {
    vec![
        ("NAVBAR_BASE", NAVBAR_BASE),
        ("NAVBAR_IOS", NAVBAR_IOS),
        ("NAVBAR_MD", NAVBAR_MD),
        ("TAB_BAR", TAB_BAR),
        ("TAB", TAB),
        ("TAB_SELECTED", TAB_SELECTED),
        ("TAB_DISABLED", TAB_DISABLED),
        ("TAB_ICON", TAB_ICON),
        ("TAB_LABEL", TAB_LABEL),
    ]
}
