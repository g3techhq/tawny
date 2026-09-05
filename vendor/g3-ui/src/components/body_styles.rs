//! Style constants for Body component.
#![allow(dead_code)]
pub const BODY_BASE: &str = "g3-body";
pub const BODY_IOS: &str = "g3-body-ios";
pub const BODY_MD: &str = "g3-body-md";
pub const BODY_CONTENT: &str = "g3-body-content";
pub const BODY_CONTENT_NO_PADDING: &str = "g3-body-content-no-padding";
pub const FOOTER_SPACER: &str = "g3-body-footer-spacer";
pub fn catalog() -> Vec<(&'static str, &'static str)> {
    vec![
        ("BODY_BASE", BODY_BASE),
        ("BODY_IOS", BODY_IOS),
        ("BODY_MD", BODY_MD),
    ]
}
