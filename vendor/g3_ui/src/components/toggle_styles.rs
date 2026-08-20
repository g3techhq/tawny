//! Style constants for Toggle (switch) component.
#![allow(dead_code)]

pub const SWITCH: &str = "g3-switch";
pub const THUMB: &str = "g3-switch-thumb";
pub const SM: &str = "g3-switch-sm";

pub const SWITCH_IOS: &str = "g3-switch-ios";
pub const THUMB_IOS: &str = "g3-switch-thumb-ios";

pub const SWITCH_MD: &str = "g3-switch-md";
pub const THUMB_MD: &str = "g3-switch-thumb-md";

pub fn catalog() -> Vec<(&'static str, &'static str)> {
    vec![
        ("SWITCH", SWITCH),
        ("THUMB", THUMB),
        ("SM", SM),
        ("SWITCH_IOS", SWITCH_IOS),
        ("THUMB_IOS", THUMB_IOS),
        ("SWITCH_MD", SWITCH_MD),
        ("THUMB_MD", THUMB_MD),
    ]
}
