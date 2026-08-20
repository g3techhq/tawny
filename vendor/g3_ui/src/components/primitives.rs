//! Compact mobile primitive components.

use super::primitives_styles as s;
use crate::theme::merge_classes;
use dioxus::prelude::*;

/// Shared semantic color for status-oriented primitives.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum StatusColor {
    #[default]
    Neutral,
    Accent,
    Success,
    Warning,
    Danger,
}

impl StatusColor {
    fn badge_class(self) -> &'static str {
        match self {
            StatusColor::Neutral => s::BADGE_NEUTRAL,
            StatusColor::Accent => s::BADGE_ACCENT,
            StatusColor::Success => s::BADGE_SUCCESS,
            StatusColor::Warning => s::BADGE_WARNING,
            StatusColor::Danger => s::BADGE_DANGER,
        }
    }
}

/// Avatar size token.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum AvatarSize {
    Sm,
    #[default]
    Md,
    Lg,
}

impl AvatarSize {
    fn class(self) -> &'static str {
        match self {
            AvatarSize::Sm => s::AVATAR_SM,
            AvatarSize::Md => s::AVATAR_MD,
            AvatarSize::Lg => s::AVATAR_LG,
        }
    }
}

/// Skeleton placeholder shape.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum SkeletonShape {
    #[default]
    Text,
    Block,
    Avatar,
    Row,
}

impl SkeletonShape {
    fn class(self) -> &'static str {
        match self {
            SkeletonShape::Text => s::SKELETON_TEXT,
            SkeletonShape::Block => s::SKELETON_BLOCK,
            SkeletonShape::Avatar => s::SKELETON_AVATAR,
            SkeletonShape::Row => s::SKELETON_ROW,
        }
    }
}

/// Compact status badge.
#[component]
pub fn Badge(color: Option<StatusColor>, class: Option<String>, children: Element) -> Element {
    let color = color.unwrap_or_default();
    let cls = merge_classes(
        format!("{} {}", s::BADGE, color.badge_class()),
        class.as_deref(),
    );

    rsx! {
        span { class: cls, {children} }
    }
}

/// Circular avatar with image or fallback text.
#[component]
pub fn Avatar(
    src: Option<String>,
    alt: Option<String>,
    fallback: Option<String>,
    size: Option<AvatarSize>,
    class: Option<String>,
) -> Element {
    let size = size.unwrap_or_default();
    let label = alt
        .or_else(|| fallback.clone())
        .unwrap_or_else(|| "Avatar".to_string());
    let image_alt = label.clone();
    let fallback = fallback.unwrap_or_default();
    let cls = merge_classes(format!("{} {}", s::AVATAR, size.class()), class.as_deref());

    rsx! {
        span { class: cls, role: "img", aria_label: label,
            if let Some(src) = src {
                img { src, alt: image_alt }
            } else {
                span { class: s::AVATAR_FALLBACK, aria_hidden: "true", "{fallback}" }
            }
        }
    }
}

/// Tappable compact chip with optional start/end slots.
#[component]
pub fn Chip(
    selected: Option<bool>,
    disabled: Option<bool>,
    start: Option<Element>,
    end: Option<Element>,
    class: Option<String>,
    onclick: Option<Callback<Event<MouseData>>>,
    children: Element,
) -> Element {
    let is_selected = selected.unwrap_or(false);
    let is_disabled = disabled.unwrap_or(false);
    let selected_cls = if is_selected { s::CHIP_SELECTED } else { "" };
    let disabled_cls = if is_disabled { s::CHIP_DISABLED } else { "" };
    let cls = merge_classes(
        format!("{} {selected_cls} {disabled_cls}", s::CHIP),
        class.as_deref(),
    );

    rsx! {
        button {
            class: cls,
            r#type: "button",
            disabled: is_disabled,
            aria_pressed: is_selected.to_string(),
            onclick: move |event| {
                if is_disabled {
                    return;
                }
                if let Some(ref onclick) = onclick {
                    onclick.call(event);
                }
            },
            if let Some(start) = start {
                span { class: s::CHIP_START, {start} }
            }
            span { class: s::CHIP_LABEL, {children} }
            if let Some(end) = end {
                span { class: s::CHIP_END, {end} }
            }
        }
    }
}

/// Linear progress indicator. `None` value renders an indeterminate bar.
#[component]
pub fn Progress(value: Option<f64>, max: Option<f64>, class: Option<String>) -> Element {
    let max = max.unwrap_or(100.0).max(f64::EPSILON);
    let clamped_value = value.map(|value| value.clamp(0.0, max));
    let ratio = clamped_value.map(|value| value / max).unwrap_or(0.0);
    let progress_value = format!("{}%", ratio * 100.0);
    let indeterminate_cls = if clamped_value.is_none() {
        s::PROGRESS_INDETERMINATE
    } else {
        ""
    };
    let cls = merge_classes(
        format!("{} {indeterminate_cls}", s::PROGRESS),
        class.as_deref(),
    );

    if let Some(value) = clamped_value {
        rsx! {
            div {
                class: cls,
                role: "progressbar",
                aria_valuemin: "0",
                aria_valuemax: max.to_string(),
                aria_valuenow: value.to_string(),
                style: "--g3-progress-value: {progress_value};",
                div { class: s::PROGRESS_TRACK,
                    div { class: s::PROGRESS_FILL }
                }
            }
        }
    } else {
        rsx! {
            div {
                class: cls,
                role: "progressbar",
                aria_valuemin: "0",
                aria_valuemax: max.to_string(),
                style: "--g3-progress-value: {progress_value};",
                div { class: s::PROGRESS_TRACK,
                    div { class: s::PROGRESS_FILL }
                }
            }
        }
    }
}

/// Non-interactive loading placeholder.
#[component]
pub fn Skeleton(shape: Option<SkeletonShape>, class: Option<String>) -> Element {
    let shape = shape.unwrap_or_default();
    let cls = merge_classes(
        format!("{} {}", s::SKELETON, shape.class()),
        class.as_deref(),
    );

    rsx! {
        span { class: cls, aria_hidden: "true" }
    }
}

#[cfg(feature = "playground")]
#[component]
pub fn PrimitivesPlaygroundDemo() -> Element {
    let mut selected = use_signal(|| true);
    let progress_value = use_signal(|| "62".to_string());
    let progress = progress_value()
        .parse::<f64>()
        .map(|value| value.clamp(0.0, 100.0) / 100.0)
        .unwrap_or(0.0);

    rsx! {
        crate::PlaygroundDemoFrame {
            center: false,
            controls: rsx! {
                crate::Checkbox { checked: selected, label: "Selected chip".to_string() }
                crate::Field {
                    label: "Progress".to_string(),
                    value: progress_value,
                    r#type: "range",
                    min: 0,
                    max: 100,
                }
            },
            div { class: "g3-primitives-demo-stack",
                div { class: "g3-primitives-demo-row",
                    Badge { color: StatusColor::Accent, "Live" }
                    Badge { color: StatusColor::Success, "Ready" }
                    Badge { color: StatusColor::Warning, "Hold" }
                    Badge { color: StatusColor::Danger, "Late" }
                }
                div { class: "g3-primitives-demo-row",
                    Avatar {
                        fallback: "MP",
                        alt: "Matthew Player",
                        size: AvatarSize::Lg,
                    }
                    Chip {
                        selected: selected(),
                        onclick: move |_| selected.toggle(),
                        start: rsx! {
                            span { "#" }
                        },
                        "Front nine"
                    }
                    Chip { disabled: true, "Locked" }
                }
                Progress { value: progress, max: 1.0 }
                Progress { max: 1.0 }
                div { class: "g3-primitives-demo-skeletons",
                    Skeleton { shape: SkeletonShape::Row }
                    Skeleton { shape: SkeletonShape::Text }
                    Skeleton { shape: SkeletonShape::Block }
                }
            }
        }
    }
}

crate::g3_playground! {
    name: "Primitives",
    description: "Compact mobile badges, avatars, chips, progress, and skeletons.",
    demo: PrimitivesPlaygroundDemo,
    source: "src/components/primitives.rs",
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(app: fn() -> Element) {
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
    }

    #[component]
    fn PrimitiveComponentsSmokeApp() -> Element {
        rsx! {
            Badge { color: StatusColor::Success, "Ready" }
            Avatar { fallback: "GP", alt: "Greenside player" }
            Chip { selected: true, onclick: |_| {}, "Walking" }
            Progress { value: 0.5, max: 1.0 }
            Progress {}
            Skeleton { shape: SkeletonShape::Row }
        }
    }

    #[test]
    fn mobile_primitives_render() {
        render(PrimitiveComponentsSmokeApp);
    }
}
