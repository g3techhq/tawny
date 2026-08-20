//! Style constants for AppWrapper (shell) component.
#![allow(dead_code)]

pub const SHELL_BASE: &str = "g3-app-shell flex flex-col overflow-hidden h-dvh";
pub const SHELL_IOS: &str = "g3-shell-ios";
pub const SHELL_MD: &str = "g3-shell-md";
pub const SHELL_NO_SELECT: &str = "g3-no-select";

pub fn catalog() -> Vec<(&'static str, &'static str)> {
    vec![
        ("SHELL_BASE", SHELL_BASE),
        ("SHELL_IOS", SHELL_IOS),
        ("SHELL_MD", SHELL_MD),
        ("SHELL_NO_SELECT", SHELL_NO_SELECT),
    ]
}
