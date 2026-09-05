//! Style constants for Field component.
#![allow(dead_code)]
pub const FIELD: &str = "g3-field";
pub const FIELD_IOS: &str = "g3-field-ios";
pub const FIELD_MD: &str = "g3-field-md";
pub fn catalog() -> Vec<(&'static str, &'static str)> {
    vec![
        ("FIELD", FIELD),
        ("FIELD_IOS", FIELD_IOS),
        ("FIELD_MD", FIELD_MD),
    ]
}
