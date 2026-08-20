//! Style constants for Field component.
#![allow(dead_code)]

// ── Base ──────────────────────────────────────────────
pub const FIELD: &str = "g3-field";

// ── iOS ───────────────────────────────────────────────
pub const FIELD_IOS: &str = "g3-field-ios";

// ── Android (Material) ────────────────────────────────
pub const FIELD_MD: &str = "g3-field-md";

pub fn catalog() -> Vec<(&'static str, &'static str)> {
    vec![
        ("FIELD", FIELD),
        ("FIELD_IOS", FIELD_IOS),
        ("FIELD_MD", FIELD_MD),
    ]
}
