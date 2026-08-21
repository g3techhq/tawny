//! Component metadata for playgrounds and documentation.

#[cfg(feature = "playground")]
use dioxus::prelude::*;

/// Name and one-line summary for a component, used by the playground and
/// by generated documentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComponentDescriptor {
    /// The component's public name, e.g. `"G3Button"`.
    pub name: &'static str,
    /// One sentence describing what the component is for.
    pub description: &'static str,
}

/// A runnable playground entry: a descriptor plus the demo that renders it
/// and the source shown alongside.
#[cfg(feature = "playground")]
#[derive(Clone, Copy)]
pub struct ComponentPlaygroundDemo {
    /// Which component this demo is for.
    pub descriptor: ComponentDescriptor,
    /// Renders the live demo.
    pub render: fn() -> dioxus::prelude::Element,
    /// The demo's source, displayed next to it so the example can be copied.
    pub source: &'static str,
}

#[cfg(feature = "playground")]
impl PartialEq for ComponentPlaygroundDemo {
    fn eq(&self, other: &Self) -> bool {
        self.descriptor == other.descriptor
    }
}

/// Every component in the library, with its description.
pub fn component_descriptors() -> Vec<ComponentDescriptor> {
    crate::components::component_descriptors()
}

/// Every playground demo in the library. Requires the `playground` feature.
#[cfg(feature = "playground")]
pub fn component_playground_demos() -> Vec<ComponentPlaygroundDemo> {
    crate::components::component_playground_demos()
}

#[cfg(feature = "playground")]
#[component]
pub fn PlaygroundDemoFrame(
    children: Element,
    controls: Option<Element>,
    app: Option<bool>,
    center: Option<bool>,
    class: Option<String>,
) -> Element {
    let preview_cls = crate::theme::merge_classes("g3-playground-preview", class.as_deref());
    let mode = crate::theme::use_component_mode(None);
    let body_cls = if center.unwrap_or(true) {
        "g3-playground-body-center"
    } else {
        "g3-playground-body-flow"
    };

    rsx! {
        div { class: "g3-playground-demo-stack",
            div { class: preview_cls,
                if app.unwrap_or(true) {
                    crate::components::AppWrapper { mode, class: "g3-playground-device-app",
                        crate::components::Body { class: body_cls, has_footer_space: false, {children} }
                    }
                } else {
                    div { class: "g3-playground-raw-surface", {children} }
                }
            }
            if let Some(controls) = controls {
                div { class: "playground-controls-pane", {controls} }
            }
        }
    }
}

/// Declare a component's playground entry: its `DESCRIPTOR` constant and the
/// demo wiring the playground collects.
///
/// Expands to items in the calling module, so invoke it once per component.
#[macro_export]
macro_rules! g3_playground {
    (
        name: $name:literal,
        description: $description:literal,
        demo: $demo:ident,
        source: $source:literal $(,)?
    ) => {
        pub const DESCRIPTOR: $crate::ComponentDescriptor = $crate::ComponentDescriptor {
            name: $name,
            description: $description,
        };

        #[cfg(feature = "playground")]
        pub const PLAYGROUND: $crate::ComponentPlaygroundDemo = $crate::ComponentPlaygroundDemo {
            descriptor: DESCRIPTOR,
            render: __g3_playground_render,
            source: include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $source)),
        };

        #[cfg(feature = "playground")]
        fn __g3_playground_render() -> dioxus::prelude::Element {
            rsx! { $demo {} }
        }
    };
}
