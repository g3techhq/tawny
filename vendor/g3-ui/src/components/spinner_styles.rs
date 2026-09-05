//! Style constants for Spinner component.
#![allow(dead_code)]
pub const SPINNER: &str = "g3-spinner";
pub const WRAPPER: &str = "g3-spinner-wrapper";
pub const CENTERED: &str = "g3-spinner-centered";
pub fn catalog() -> Vec<(&'static str, &'static str)> {
    vec![
        ("SPINNER", SPINNER),
        ("WRAPPER", WRAPPER),
        ("CENTERED", CENTERED),
    ]
}
