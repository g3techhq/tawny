//! Style constants for Button component.
#![allow(dead_code)]

// ── Base ──────────────────────────────────────────────
pub const BASE: &str = "g3-btn";

// ── iOS ───────────────────────────────────────────────
pub const IOS: &str = "g3-btn-ios";

// ── Android (Material) ────────────────────────────────
pub const MD: &str = "g3-btn-md";

// ── Variants ──────────────────────────────────────────
pub const SOLID: &str = "g3-btn-solid";
pub const OUTLINE: &str = "g3-btn-outline";
pub const CLEAR: &str = "g3-btn-clear";
pub const NEUTRAL: &str = "g3-btn-neutral";
pub const DANGER: &str = "g3-btn-danger";

// ── Sizes ─────────────────────────────────────────────
pub const SM: &str = "g3-btn-sm";
pub const MD_SIZE: &str = "g3-btn-md-size";
pub const LG: &str = "g3-btn-lg";

// ── Badge overlay ─────────────────────────────────────
pub const BADGE: &str = "g3-btn-badge";

pub fn catalog() -> Vec<(&'static str, &'static str)> {
    vec![
        ("BASE", BASE),
        ("IOS", IOS),
        ("MD", MD),
        ("SOLID", SOLID),
        ("OUTLINE", OUTLINE),
        ("CLEAR", CLEAR),
        ("NEUTRAL", NEUTRAL),
        ("DANGER", DANGER),
        ("SM", SM),
        ("MD_SIZE", MD_SIZE),
        ("LG", LG),
        ("BADGE", BADGE),
    ]
}
