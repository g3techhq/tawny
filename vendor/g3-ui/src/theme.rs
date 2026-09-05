//! Global mode and theme system for g3-ui.
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
/// CSS custom-property theme tokens for g3-ui components.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    /// Accent color: focus rings, selected segments, active toggles, links.
    /// Tints elsewhere are derived from this with `color-mix`, so it is the
    /// one token that most changes an app's character.
    pub focused: String,
    /// Primary label text inside list rows and fields.
    pub label_primary: String,
    /// Secondary label text - helper copy, row detail, placeholder values.
    pub label_secondary: String,
    /// Hairline border around cards and inset groups.
    pub card_border: String,
    /// The app's base background, behind everything else.
    pub bg: String,
    /// A recessed background for grouped content, matching the iOS grouped
    /// table style where rows sit on a slightly darker ground.
    pub bg_secondary: String,
    /// Card fill.
    pub card: String,
    /// Fill for a card nested inside another card.
    pub card_inset: String,
    /// Fill for raised surfaces that float above the page: sheets, modals,
    /// popovers, toasts.
    pub surface: String,
    /// Fill for interactive controls - inputs, selects, unselected segments.
    pub control: String,
    /// Body text.
    pub text: String,
    /// De-emphasized body text: captions, timestamps, disabled labels.
    pub text_secondary: String,
    /// Shadow color, including its alpha. Elevation is expressed by varying
    /// blur and offset against this single color.
    pub shadow: String,
    /// Semantic color for success states and confirmations.
    pub success: String,
    /// Semantic color for warnings and states needing attention.
    pub warning: String,
    /// Semantic color for errors and destructive actions.
    pub danger: String,
    /// The CSS `color-scheme` value (`"light"` or `"dark"`). Drives native
    /// form controls, scrollbars, and the browser's own default surfaces so
    /// they match the rest of the theme.
    pub color_scheme: String,
}
impl Default for Theme {
    fn default() -> Self {
        Self::default_light()
    }
}
impl Theme {
    /// The built-in light theme: iOS system colors on a near-white ground.
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
    /// The built-in dark theme. Not a mechanical inversion of
    /// [`Theme::default_light`] - contrast and accent brightness are tuned
    /// separately for a dark ground.
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
    /// Return this theme with a different accent color. The common way to
    /// brand an app is to start from a default theme and override this one
    /// token.
    pub fn with_focused(mut self, focused: impl Into<String>) -> Self {
        self.focused = focused.into();
        self
    }
    /// Render every token as a CSS custom-property declaration, suitable for
    /// an inline `style` attribute. `AppWrapper` applies this to the shell
    /// root, which is how the tokens reach the stylesheet.
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
/// Runtime mode context for g3-ui components.
///
/// Carries a signal rather than a plain `ComponentMode` so that switching mode
/// at runtime actually reaches components that are already on screen. A plain
/// value in context is read once and never re-read: Dioxus skips re-rendering a
/// component whose props are unchanged, and a context read is not a
/// subscription. Reading the signal in [`use_component_mode`] subscribes the
/// reading component, so a mode change re-renders it even though nothing was
/// passed to it.
#[derive(Clone, Copy, PartialEq)]
pub struct G3Mode {
    /// The platform styling mode in effect for this subtree.
    pub mode: Signal<ComponentMode>,
}
/// Keep one signal per provider scope and push the latest value into it.
///
/// `peek` deliberately avoids subscribing the provider to its own signal, which
/// would otherwise re-render it every time it published a change.
pub(crate) fn use_context_signal<T: PartialEq + Clone + 'static>(value: T) -> Signal<T> {
    let mut signal = use_signal(|| value.clone());
    if *signal.peek() != value {
        signal.set(value);
    }
    signal
}
/// Read the ambient [`Theme`], if a provider or `AppWrapper` set one.
///
/// Subscribes the calling component to theme changes, for the same reason
/// [`G3Mode`] holds a signal. Only for components that do not themselves
/// provide a theme - see [`use_ancestor_context`] for the ones that do.
pub fn use_ambient_theme() -> Option<Theme> {
    try_consume_context::<Signal<Theme>>().map(|theme| theme())
}
/// Resolve a context value from an *ancestor*, ignoring whatever this component
/// provides itself.
///
/// `try_consume_context` searches the current scope first. A component that both
/// consumes and re-provides - `AppWrapper` inheriting an outer theme and then
/// publishing it to its own subtree - would therefore read its own value back on
/// every render after the first, pinning it to whatever was ambient at mount.
/// Resolving the ancestor once via `use_hook` sidesteps that: signal identity
/// upstream is stable, so holding the handle is safe, and reading through it
/// each render still subscribes to changes.
pub(crate) fn use_ancestor_context<T: Clone + 'static>() -> Option<T> {
    use_hook(try_consume_context::<T>)
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
        if #[cfg(target_os = "ios")] { ComponentMode::Ios } else if #[cfg(target_os =
        "android")] { ComponentMode::Md } else if #[cfg(target_arch = "wasm32")] {
        detect_web_mode() } else { ComponentMode::Md }
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
///
/// Uses `try_consume_context` rather than `try_use_context`: the latter is a
/// hook that caches on first render, which would pin a component to whichever
/// mode was in effect when it mounted.
pub fn use_component_mode(mode: Option<ComponentMode>) -> ComponentMode {
    let mode_context = try_consume_context::<G3Mode>().map(|context| (context.mode)());
    mode.or(mode_context).unwrap_or_else(get_mode)
}
#[component]
pub fn G3ThemeProvider(
    mode: Option<ComponentMode>,
    theme: Option<Theme>,
    children: Element,
) -> Element {
    let mode = use_context_signal(mode.unwrap_or_else(get_mode));
    let theme = use_context_signal(theme.unwrap_or_default());
    provide_context(G3Mode { mode });
    provide_context(theme);
    rsx! {
        {children}
    }
}
/// Initialize the global mode based on compile-time platform detection.
pub fn init_auto_mode() {
    set_mode(detect_platform_mode());
}
impl ComponentMode {
    /// The `data-g3-mode` attribute value the stylesheet keys on.
    pub fn as_str(self) -> &'static str {
        match self {
            ComponentMode::Md => "md",
            ComponentMode::Ios => "ios",
        }
    }
}
/// Backward-compatible alias for earlier g3-ui naming.
pub type G3Theme = Theme;
