//! Header component with start/end buttons and an optional toolbar.
use super::header_styles as s;
use crate::theme::{ComponentMode, merge_classes, use_component_mode};
use dioxus::prelude::*;
#[derive(Clone, Copy)]
pub(crate) struct HeaderToolbarContext;
#[component]
fn HeaderToolbarContextProvider(children: Element) -> Element {
    provide_context(HeaderToolbarContext);
    rsx! {
        {children}
    }
}
#[component]
pub fn Header(
    title: String,
    /// Small element (e.g. an icon) rendered inline right after the title
    /// text — for a status indicator that belongs with the title itself
    /// rather than off in the end-button slot.
    title_icon: Option<Element>,
    start_button: Option<Element>,
    end_button: Option<Element>,
    toolbar: Option<Element>,
    class: Option<String>,
    mode: Option<ComponentMode>,
) -> Element {
    let mode = use_component_mode(mode);
    let has_toolbar = toolbar.is_some();
    let header_cls = match mode {
        ComponentMode::Ios => format!("{} {}", s::HEADER_BASE, s::HEADER_IOS),
        ComponentMode::Md => format!("{} {}", s::HEADER_BASE, s::HEADER_MD),
    };
    let header_cls = merge_classes(header_cls, has_toolbar.then_some(s::HEADER_WITH_TOOLBAR));
    rsx! {
        header { class: merge_classes(format!("{header_cls} relative"), class
                    .as_deref()),
            div { class: s::HEADER_ROW,
                div { class: s::HEADER_START_SLOT,
                    if let Some(start) = start_button {
                        {start}
                    }
                }
                h1 { class: s::HEADER_TITLE,
                    span { class: s::HEADER_TITLE_TEXT, "{title}" }
                    if let Some(icon) = title_icon {
                        {icon}
                    }
                }
                div { class: s::HEADER_END_SLOT,
                    if let Some(end) = end_button {
                        {end}
                    }
                }
            }
            if let Some(t) = toolbar {
                div { class: s::TOOLBAR,
                    HeaderToolbarContextProvider { {t} }
                }
            }
        }
    }
}
#[cfg(feature = "playground")]
#[component]
pub fn HeaderPlaygroundDemo() -> Element {
    let active = use_signal(|| 0_usize);
    let title = use_signal(|| "Pending Game".to_string());
    let toolbar = use_signal(|| true);
    let start_button = use_signal(|| true);
    let end_button = use_signal(|| true);
    let start_text = use_signal(|| "Close".to_string());
    let end_text = use_signal(|| "Create".to_string());
    let playground_mode = crate::use_component_mode(None);
    let toolbar_slot = toolbar().then(|| {
        rsx! {
            crate::SegmentGroup { active,
                crate::SegmentButton { index: 0, "Players" }
                crate::SegmentButton { index: 1, "Bet" }
            }
        }
    });
    rsx! {
        crate::PlaygroundDemoFrame {
            app: false,
            controls: rsx! {
                crate::Field { label: "Title".to_string(), value: title }
                crate::Checkbox { checked: start_button, label: "Start button".to_string() }
                crate::Field { label: "Start text".to_string(), value: start_text }
                crate::Checkbox { checked: end_button, label: "End button".to_string() }
                crate::Field { label: "End text"
                            .to_string(), value: end_text }
                crate::Checkbox { checked: toolbar, label: "Toolbar".to_string() }
            },
            crate::AppWrapper { mode: playground_mode, class: "g3-playground-device-app",
                Header {
                    title: title(),
                    start_button: start_button().then(|| rsx! {
                        crate::Button { style: crate::ButtonStyle::Clear, onclick: |_| {}, "{start_text()}" }
                    }),
                    end_button: end_button().then(|| rsx! {
                        crate::Button { style: crate::ButtonStyle::Outline, onclick: |_| {}, "{end_text()}" }
                    }),
                    toolbar: toolbar_slot,
                }
                crate::Body { has_footer_space: false,
                    crate::Card { title: "Preview", "Toolbar stays attached to the header." }
                }
            }
        }
    }
}
crate::g3_playground! {
    name: "Header",
    description: "App header with start, title, end, and toolbar slots.",
    demo: HeaderPlaygroundDemo,
    source: "src/components/header.rs",
}
