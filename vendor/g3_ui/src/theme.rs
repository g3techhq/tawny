//! Global mode and theme system for g3_ui.
//!
//! Modes control platform styling (iOS vs Material Design). Themes control CSS
//! custom-property tokens and are fully configurable by consumer crates.

use cfg_if::cfg_if;
use dioxus::prelude::*;
use std::cell::Cell;

/// Platform styling mode - mirrors Ionic's `mode` attribute.
#[derive(Clone, Copy, PartialEq, Default)]
pub enum ComponentMode {
    /// Material Design: sharper corners, flat elevation, ease-out transitions
    #[default]
    Md,
    /// iOS styling: pill shapes, spring animations, subtle shadows
    Ios,
}

/// CSS custom-property theme tokens for g3_ui components.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    pub focused: String,
    pub label_primary: String,
    pub label_secondary: String,
    pub card_border: String,
    pub bg: String,
    pub bg_secondary: String,
    pub card: String,
    pub card_inset: String,
    pub surface: String,
    pub control: String,
    pub text: String,
    pub text_secondary: String,
    pub shadow: String,
    pub success: String,
    pub warning: String,
    pub danger: String,
    pub color_scheme: String,
}

impl Default for Theme {
    fn default() -> Self {
        Self::default_light()
    }
}

impl Theme {
    pub fn default_light() -> Self {
        Self {
            focused: "#007aff".into(),
            label_primary: "#000000".into(),
            label_secondary: "#8f8f8f".into(),
            card_border: "#c7c7c7".into(),
            bg: "#f8f8f8".into(),
            bg_secondary: "#efeff4".into(),
            card: "#ffffff".into(),
            card_inset: "#fafafa".into(),
            surface: "#ffffff".into(),
            control: "#ffffff".into(),
            text: "#000000".into(),
            text_secondary: "#6b6b6b".into(),
            shadow: "rgba(0, 0, 0, 0.1)".into(),
            success: "#37c964".into(),
            warning: "#ff9f0a".into(),
            danger: "#ff3b30".into(),
            color_scheme: "light".into(),
        }
    }

    pub fn default_dark() -> Self {
        Self {
            focused: "#0a84ff".into(),
            label_primary: "#ffffff".into(),
            label_secondary: "#a1a1aa".into(),
            card_border: "rgba(255, 255, 255, 0.14)".into(),
            bg: "#111827".into(),
            bg_secondary: "#0f172a".into(),
            card: "#1f2937".into(),
            card_inset: "#111827".into(),
            surface: "#1f2937".into(),
            control: "rgba(255, 255, 255, 0.12)".into(),
            text: "#ffffff".into(),
            text_secondary: "#d1d5db".into(),
            shadow: "rgba(0, 0, 0, 0.35)".into(),
            success: "#30d158".into(),
            warning: "#ffd60a".into(),
            danger: "#ff453a".into(),
            color_scheme: "dark".into(),
        }
    }

    pub fn with_focused(mut self, focused: impl Into<String>) -> Self {
        self.focused = focused.into();
        self
    }

    pub fn to_style_attr(&self) -> String {
        format!(
            "--color-focused: {}; --color-label-primary: {}; --color-label-secondary: {}; --color-card-border: {}; --color-bg: {}; --color-bg-secondary: {}; --color-card: {}; --color-card-inset: {}; --color-surface: {}; --color-control: {}; --color-text: {}; --color-text-secondary: {}; --color-shadow: {}; --color-success: {}; --color-warning: {}; --color-danger: {}; color-scheme: {};",
            self.focused,
            self.label_primary,
            self.label_secondary,
            self.card_border,
            self.bg,
            self.bg_secondary,
            self.card,
            self.card_inset,
            self.surface,
            self.control,
            self.text,
            self.text_secondary,
            self.shadow,
            self.success,
            self.warning,
            self.danger,
            self.color_scheme,
        )
    }
}

/// Runtime mode context for g3_ui components.
#[derive(Clone, Copy, PartialEq)]
pub struct G3Mode {
    pub mode: ComponentMode,
}

thread_local! {
    static GLOBAL_MODE: Cell<Option<ComponentMode>> = const { Cell::new(None) };
}

/// Set the global styling mode. Typically called once at app init
/// based on `cfg(target_os)`.
pub fn set_mode(mode: ComponentMode) {
    GLOBAL_MODE.with(|m| m.set(Some(mode)));
}

/// Get the current global mode. Used by components when no per-component
/// `mode` prop is provided.
pub fn get_mode() -> ComponentMode {
    GLOBAL_MODE
        .with(|m| m.get())
        .unwrap_or_else(detect_platform_mode)
}

/// Detect the best platform styling mode for the current build/runtime.
pub fn detect_platform_mode() -> ComponentMode {
    cfg_if! {
        if #[cfg(target_os = "ios")] {
            ComponentMode::Ios
        } else if #[cfg(target_os = "android")] {
            ComponentMode::Md
        } else if #[cfg(target_arch = "wasm32")] {
            detect_web_mode()
        } else {
            ComponentMode::Md
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn detect_web_mode() -> ComponentMode {
    let Some(window) = web_sys::window() else {
        return ComponentMode::Md;
    };

    let navigator = window.navigator();
    let user_agent = navigator.user_agent().unwrap_or_default().to_lowercase();
    let platform = navigator.platform().unwrap_or_default().to_lowercase();
    let max_touch_points = navigator.max_touch_points();

    let is_iphone_or_ipod = user_agent.contains("iphone") || user_agent.contains("ipod");
    let is_ipad = user_agent.contains("ipad")
        || (platform.contains("mac") && max_touch_points > 1 && user_agent.contains("safari"));

    if is_iphone_or_ipod || is_ipad {
        ComponentMode::Ios
    } else {
        ComponentMode::Md
    }
}

/// Merge a required component class list with an optional per-instance override.
pub fn merge_classes(base: impl AsRef<str>, class: Option<&str>) -> String {
    let base = base.as_ref().trim();
    let class = class.unwrap_or_default().trim();

    match (base.is_empty(), class.is_empty()) {
        (true, true) => String::new(),
        (true, false) => class.to_string(),
        (false, true) => base.to_string(),
        (false, false) => format!("{base} {class}"),
    }
}

/// Resolve the current component mode from an explicit prop, mode context, or global mode.
pub fn use_component_mode(mode: Option<ComponentMode>) -> ComponentMode {
    let mode_context = try_use_context::<G3Mode>();
    mode.or_else(|| mode_context.map(|context| context.mode))
        .unwrap_or_else(get_mode)
}

#[component]
pub fn G3ThemeProvider(
    mode: Option<ComponentMode>,
    theme: Option<Theme>,
    children: Element,
) -> Element {
    let mode = mode.unwrap_or_else(get_mode);
    provide_context(G3Mode { mode });
    provide_context(theme.unwrap_or_default());

    rsx! {
        {children}
    }
}

/// Initialize the global mode based on compile-time platform detection.
pub fn init_auto_mode() {
    set_mode(detect_platform_mode());
}

/// Class applied to a root element while g3_ui's stylesheet is still loading.
/// Internal to `AppWrapper` - pair with [`use_css_preload_guard`] and
/// [`G3PreloadStyle`], adding this class to the root's class list while the
/// returned signal is `true`.
pub(crate) const CSS_PRELOAD_CLASS: &str = "g3-preload";

const CSS_PRELOAD_POLL_SCRIPT: &str = r#"
let tries = 0;
const ready = () => getComputedStyle(document.documentElement)
    .getPropertyValue("--g3-css-loaded").trim() === "1";
const tick = () => {
    if (ready() || tries++ > 300) {
        requestAnimationFrame(() => dioxus.send(true));
    } else {
        requestAnimationFrame(tick);
    }
};
tick();
"#;

/// Tracks whether g3_ui.css (attached at runtime via `document::Link`) has
/// finished loading. Internal to `AppWrapper`, which is the only supported
/// way consumers should attach g3_ui's stylesheet and get this protection -
/// not something consumers need to call themselves.
pub(crate) fn use_css_preload_guard() -> Signal<bool> {
    let mut preloading = use_signal(|| true);
    use_effect(move || {
        spawn(async move {
            let mut eval = document::eval(CSS_PRELOAD_POLL_SCRIPT);
            let _ = eval.recv::<bool>().await;
            preloading.set(false);
        });
    });
    preloading
}

/// Inline (network-free) style that hides [`CSS_PRELOAD_CLASS`] roots and
/// kills their transitions. Must be inline rather than living in g3_ui.css
/// itself, since that external stylesheet is exactly what hasn't loaded yet
/// during the window this guard needs to cover. Rendered once by `AppWrapper`.
#[component]
pub(crate) fn G3PreloadStyle() -> Element {
    rsx! {
        document::Style {
            r#".g3-preload, .g3-preload * {{ transition: none !important; }} .g3-preload {{ visibility: hidden !important; }}"#
        }
    }
}

impl ComponentMode {
    pub fn as_str(self) -> &'static str {
        match self {
            ComponentMode::Md => "md",
            ComponentMode::Ios => "ios",
        }
    }
}

/// Backward-compatible alias for earlier g3_ui naming.
pub type G3Theme = Theme;
