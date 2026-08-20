//! Style constants for Card component.
#![allow(dead_code)]

pub const CARD: &str = "g3-card";
pub const CARD_IOS: &str = "g3-card-ios";
pub const CARD_MD: &str = "g3-card-md";
pub const INSET: &str = "g3-card-inset";
pub const SELECTED: &str = "selected";
pub const INTERACTIVE: &str = "g3-card-interactive";
pub const CONTROL: &str = "g3-card-control";
pub const HEADER: &str = "g3-card-header";
pub const TITLE: &str = "g3-card-title";
pub const BODY: &str = "g3-card-body";
pub const RIGHT_TEXT: &str = "card-right-text text-sm whitespace-nowrap";

pub fn catalog() -> Vec<(&'static str, &'static str)> {
    vec![
        ("CARD", CARD),
        ("CARD_IOS", CARD_IOS),
        ("CARD_MD", CARD_MD),
        ("INSET", INSET),
        ("SELECTED", SELECTED),
        ("INTERACTIVE", INTERACTIVE),
        ("CONTROL", CONTROL),
    ]
}
