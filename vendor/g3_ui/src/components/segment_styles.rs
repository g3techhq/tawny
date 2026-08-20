//! Style constants for Segment component.
#![allow(dead_code)]

pub const SEGMENT_IOS: &str = "g3-segment-ios";
pub const SEGMENT_BTN_IOS: &str = "g3-segment-btn-ios";
pub const SEGMENT_MD: &str = "g3-segment-md";
pub const SEGMENT_BTN_MD: &str = "g3-segment-btn-md";
pub const TOOLBAR: &str = "g3-segment-toolbar";
pub const STANDALONE: &str = "g3-segment-standalone";
pub const ROUTE_WRAPPER: &str = "g3-segment-route-wrapper";
pub const SCROLLABLE: &str = "overflow-x-auto";

pub fn catalog() -> Vec<(&'static str, &'static str)> {
    vec![
        ("SEGMENT_IOS", SEGMENT_IOS),
        ("SEGMENT_BTN_IOS", SEGMENT_BTN_IOS),
        ("SEGMENT_MD", SEGMENT_MD),
        ("SEGMENT_BTN_MD", SEGMENT_BTN_MD),
        ("TOOLBAR", TOOLBAR),
        ("STANDALONE", STANDALONE),
        ("ROUTE_WRAPPER", ROUTE_WRAPPER),
    ]
}
