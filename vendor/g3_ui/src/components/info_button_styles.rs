//! InfoButton style constants.
#![allow(dead_code)]

/// Base class for info button.
pub const BASE: &str = "info-btn";

/// iOS variant - subtle circular background with spring transition.
pub const IOS: &str = "info-btn-ios";

/// Android variant - flat, no background.
pub const MD: &str = "info-btn-md";

/// Catalog of all style constants.
pub fn catalog() -> Vec<(&'static str, &'static str)> {
    vec![("base", BASE), ("ios", IOS), ("md", MD)]
}
