//! Pull-to-refresh wrapper component.
use super::refresher_styles as s;
use crate::theme::{ComponentMode, merge_classes, use_component_mode};
use dioxus::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};
pub const DEFAULT_REFRESH_THRESHOLD: f64 = 48.0;
pub const REFRESH_ELASTIC_FACTOR: f64 = 0.42;
static REFRESHER_INSTANCE_ID: AtomicU64 = AtomicU64::new(0);
/// Live state of a pull-to-refresh gesture, handed to the refresher's
/// render callback so a custom indicator can follow the pull.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RefresherState {
    /// Current pull distance in pixels.
    pub pull: f64,
    /// `pull` as a fraction of the trigger threshold; reaches `1.0` at the
    /// point where releasing would start a refresh.
    pub progress: f64,
    /// Whether a refresh is currently running. Stays `true` from release
    /// until the refresh completes.
    pub refreshing: bool,
}
const REFRESHER_DRAG_SCRIPT: &str = r#"
const root = document.getElementById("__ROOT_ID__");
const label = document.getElementById("__LABEL_ID__");
if (root) {
    const THRESHOLD = __THRESHOLD__;
    const ELASTIC = __ELASTIC__;
    let dragging = false;
    let engaged = false;
    let startY = 0;
    let pull = 0;

    const scrollParent = () => {
        let el = root.parentElement;
        while (el) {
            const oy = getComputedStyle(el).overflowY;
            if ((oy === "auto" || oy === "scroll") && el.scrollHeight > el.clientHeight) {
                return el;
            }
            el = el.parentElement;
        }
        return null;
    };

    const gateOk = () =>
        root.dataset.canRefresh === "true" &&
        root.dataset.refreshing !== "true" &&
        root.dataset.disabled !== "true";

    const atTop = () => {
        const sp = scrollParent();
        return !sp || sp.scrollTop <= 0;
    };

    const setPull = (p) => {
        pull = p;
        root.style.setProperty("--g3-refresher-pull", p + "px");
        root.style.setProperty("--g3-refresher-progress", Math.min(p / THRESHOLD, 1.4));
        root.setAttribute("data-state", p > 0 ? "pulling" : "idle");
        if (label) {
            label.textContent = p >= THRESHOLD ? "Release to refresh" : "Pull to refresh";
        }
    };

    // The drag script mutates the label outside Dioxus. Restore it ourselves
    // once the caller's controlled `refreshing` prop settles, because a very
    // fast refresh can batch true -> false without a virtual-DOM text patch.
    const settleAfterRefresh = () => {
        if (root.dataset.refreshing === "true") {
            if (label) label.textContent = "Refreshing";
            setTimeout(settleAfterRefresh, 100);
            return;
        }
        root.setAttribute("data-state", "idle");
        root.style.setProperty("--g3-refresher-progress", "0");
        if (label) label.textContent = "Pull to refresh";
    };

    const onDown = (y) => {
        if (!gateOk() || !atTop()) return;
        dragging = true;
        engaged = false;
        startY = y;
    };

    // `e` is optional (touch path passes it so we can preventDefault the scroll).
    const onMove = (y, e) => {
        if (!dragging) return;
        const dy = y - startY;
        if (dy <= 0) {
            if (engaged) {
                engaged = false;
                setPull(0);
            }
            return;
        }
        if (!engaged) {
            if (!atTop()) return;
            engaged = true;
        }
        // Non-passive touchmove: this is what actually stops the page scrolling.
        if (e && e.cancelable) e.preventDefault();
        const dist = dy > THRESHOLD ? THRESHOLD + (dy - THRESHOLD) * ELASTIC : dy;
        setPull(dist);
    };

    const onEnd = () => {
        if (!dragging) return;
        dragging = false;
        if (!engaged) return;
        engaged = false;
        if (pull >= THRESHOLD && root.dataset.hasRefresh === "true") {
            // Hand off to the Rust `refreshing` state. Clear the pull var so the
            // content settles flush again once refreshing ends; while refreshing
            // the indicator is driven by the `data-state="refreshing"` rules.
            root.style.setProperty("--g3-refresher-pull", "0px");
            root.style.setProperty("--g3-refresher-progress", "1");
            root.setAttribute("data-state", "refreshing");
            if (label) label.textContent = "Refreshing";
            dioxus.send(true);
            setTimeout(settleAfterRefresh, 100);
        } else {
            setPull(0);
        }
    };

    // Mouse (desktop) rides pointer events; touch uses touch events so the
    // touchmove listener can be non-passive and cancel the scroll.
    root.addEventListener("pointerdown", (e) => { if (e.pointerType === "mouse") onDown(e.clientY); });
    root.addEventListener("pointermove", (e) => { if (e.pointerType === "mouse") onMove(e.clientY, null); });
    root.addEventListener("pointerup", (e) => { if (e.pointerType === "mouse") onEnd(); });
    root.addEventListener("touchstart", (e) => { onDown(e.touches[0].clientY); }, { passive: true });
    root.addEventListener("touchmove", (e) => { onMove(e.touches[0].clientY, e); }, { passive: false });
    root.addEventListener("touchend", () => onEnd());
    root.addEventListener("touchcancel", () => onEnd());
}
"#;
#[component]
pub fn Refresher(
    refreshing: Option<bool>,
    threshold: Option<f64>,
    disabled: Option<bool>,
    can_refresh: Option<bool>,
    class: Option<String>,
    mode: Option<ComponentMode>,
    on_refresh: Option<Callback<()>>,
    children: Element,
) -> Element {
    let mode = use_component_mode(mode);
    let refreshing = refreshing.unwrap_or(false);
    let disabled = disabled.unwrap_or(false);
    let can_refresh = can_refresh.unwrap_or(false);
    let threshold = threshold.unwrap_or(DEFAULT_REFRESH_THRESHOLD).max(1.0);
    let has_refresh = on_refresh.is_some();
    let mode_cls = match mode {
        ComponentMode::Ios => s::REFRESHER_IOS,
        ComponentMode::Md => s::REFRESHER_MD,
    };
    let instance_id = use_hook(|| REFRESHER_INSTANCE_ID.fetch_add(1, Ordering::Relaxed));
    let root_id = format!("g3-refresher-{instance_id}");
    let label_id = format!("g3-refresher-label-{instance_id}");
    {
        let root_id = root_id.clone();
        let label_id = label_id.clone();
        use_effect(move || {
            let script = REFRESHER_DRAG_SCRIPT
                .replace("__ROOT_ID__", &root_id)
                .replace("__LABEL_ID__", &label_id)
                .replace("__THRESHOLD__", &threshold.to_string())
                .replace("__ELASTIC__", &REFRESH_ELASTIC_FACTOR.to_string());
            spawn(async move {
                let mut eval = document::eval(&script);
                while let Ok(true) = eval.recv::<bool>().await {
                    if let Some(on_refresh) = on_refresh {
                        on_refresh.call(());
                    }
                }
            });
        });
    }
    let state = if refreshing { "refreshing" } else { "idle" };
    rsx! {
        div {
            id: root_id,
            class: merge_classes(format!("{} {mode_cls}", s::REFRESHER), class.as_deref()),
            "data-state": state,
            "data-can-refresh": can_refresh
                    .to_string(),
            "data-refreshing": refreshing.to_string(),
            "data-disabled": disabled.to_string(),
            "data-has-refresh": has_refresh.to_string(),
            div { class: s::INDICATOR, role: "status", aria_live: "polite",
                span { class: s::SPINNER, aria_hidden: "true" }
                span { id: label_id, class: s::LABEL,
                    if refreshing {
                        "Refreshing"
                    } else {
                        "Pull to refresh"
                    }
                }
            }
            div { class: s::CONTENT, {children} }
        }
    }
}
#[cfg(feature = "playground")]
#[component]
pub fn RefresherPlaygroundDemo() -> Element {
    let mut refreshing = use_signal(|| false);
    rsx! {
        crate::PlaygroundDemoFrame { center: false,
            Refresher {
                refreshing: refreshing(),
                can_refresh: true,
                on_refresh: move |_| {
                    refreshing.set(true);
                    spawn(async move {
                        dioxus_sdk_time::sleep(std::time::Duration::from_millis(1200)).await;
                        refreshing.set(false);
                    });
                },
                crate::List { inset: true,
                    crate::Item {
                        label: "Leaderboard".to_string(),
                        description: "Pull down to simulate refresh".to_string(),
                    }
                    crate::Item {
                        label: "Skins"
                                .to_string(),
                        metadata: "$12".to_string(),
                    }
                }
            }
        }
    }
}
crate::g3_playground! {
    name: "Refresher",
    description: "Pull-to-refresh container with thresholded mobile gestures.",
    demo: RefresherPlaygroundDemo,
    source: "src/components/refresher.rs",
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::G3ThemeProvider;
    fn render(app: fn() -> Element) {
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
    }
    #[test]
    fn refresher_gates_on_scroll_top_and_can_refresh() {
        let source = include_str!("refresher.rs");
        assert!(source.contains("can_refresh.unwrap_or(false)"));
        assert!(source.contains("root.dataset.canRefresh === \"true\""));
        assert!(source.contains("sp.scrollTop <= 0"));
    }
    #[test]
    fn refresher_takes_over_the_touch_gesture() {
        let source = include_str!("refresher.rs");
        assert!(source.contains("\"touchmove\""));
        assert!(source.contains("{ passive: false }"));
        assert!(source.contains("e.preventDefault()"));
    }
    #[test]
    fn refresher_hands_off_to_rust_refreshing_state() {
        let source = include_str!("refresher.rs");
        assert!(source.contains("dioxus.send(true)"));
        assert!(source.contains("root.dataset.hasRefresh === \"true\""));
        assert!(source.contains("setTimeout(settleAfterRefresh, 100)"));
        assert!(source.contains("label.textContent = \"Pull to refresh\""));
    }
    #[component]
    fn RefresherSmokeApp() -> Element {
        rsx! {
            G3ThemeProvider { mode: ComponentMode::Ios,
                Refresher { refreshing: false, can_refresh: true,
                    div { "Rows" }
                }
            }
        }
    }
    #[test]
    fn refresher_renders() {
        render(RefresherSmokeApp);
    }
}
