use crate::{
    components::{
        AppOverlays, AppShell, ChannelDetail, Explore, Feed, HistoryPage, PlaylistDetail,
        Playlists, QueuePage, SettingsPage, Subscriptions, VideoDetail,
    },
    models::Appearance,
    state::{AppState, AppStateProvider},
};
use dioxus::prelude::*;
use dx_native_plugins::NativePluginsProvider;
use dx_route_transitions::{
    Platform, ROUTE_TRANSITIONS_CSS, RouteTransitionProvider, route_transitions, set_platform,
};
use g3_ui::{AppWrapper, Theme};

// Statically headed so web builds block first paint on it, the same as
// g3_ui.css - linking it only at runtime left the app rendering unstyled while
// the WASM booted. Desktop and mobile bundles do not collect statically-headed
// assets, so the runtime link below is what reaches those builds.
const TAILWIND_CSS: Asset = asset!(
    "/assets/tailwind.css",
    AssetOptions::css().with_static_head(true)
);
const SHAKA_PLAYER_JS: Asset = asset!("/node_modules/shaka-player/dist/shaka-player.compiled.js");
const TAWNY_TRANSPORT_JS: Asset = asset!("/assets/tawny_transport.js");
const TAWNY_PLAYER_CONTROLS_JS: Asset = asset!("/assets/tawny_player_controls.js");
const TAWNY_ROUTE_HISTORY_JS: Asset = asset!("/assets/tawny_route_history.js");

/// Every route is a `base` peer that cross-fades, except the watch page: it is
/// the one true sheet, covering the shell on the way in and uncovering it on
/// the way out. Playlists, channels and settings keep the nav in place, so
/// sliding a sheet over it would only misrepresent where the user is.
#[route_transitions]
#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(AppShell)]
        #[transition(base)]
        #[route("/")]
        Feed {},
        #[transition(base)]
        #[route("/subscriptions")]
        Subscriptions {},
        #[transition(base)]
        #[route("/playlists")]
        Playlists {},
        #[transition(base)]
        #[route("/explore")]
        Explore {},
        #[transition(base)]
        #[route("/queue")]
        QueuePage {},
        #[transition(base)]
        #[route("/history")]
        HistoryPage {},
        #[transition(cover)]
        #[route("/watch/:id")]
        VideoDetail { id: String },
        #[transition(base)]
        #[route("/playlists/:id")]
        PlaylistDetail { id: String },
        #[transition(base)]
        #[route("/settings")]
        SettingsPage {},
        #[transition(base)]
        #[route("/channel/:id")]
        ChannelDetail { id: String },
}

fn tawny_theme(appearance: Appearance) -> Theme {
    match appearance {
        Appearance::Dark => Theme {
            focused: "#f5a524".into(),
            label_primary: "#f8fafc".into(),
            label_secondary: "#9aa6b2".into(),
            card_border: "rgba(255, 255, 255, 0.09)".into(),
            bg: "#0b0f14".into(),
            bg_secondary: "#10161e".into(),
            card: "#141b24".into(),
            card_inset: "#10161e".into(),
            surface: "#18212c".into(),
            control: "rgba(255, 255, 255, 0.09)".into(),
            text: "#f8fafc".into(),
            text_secondary: "#a8b3bf".into(),
            shadow: "rgba(0, 0, 0, 0.42)".into(),
            success: "#5bc987".into(),
            warning: "#f5a524".into(),
            danger: "#ff6b6b".into(),
            color_scheme: "dark".into(),
        },
        Appearance::Light => Theme::default_light().with_focused("#c96e12"),
    }
}

#[component]
pub fn App() -> Element {
    rsx! {
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        // Without this the browser falls back to its own root cross-fade: no
        // cover keyframes, no base snapshot, and a transparent page behind it.
        document::Link { rel: "stylesheet", href: ROUTE_TRANSITIONS_CSS }
        document::Script { src: SHAKA_PLAYER_JS }
        document::Script { src: TAWNY_TRANSPORT_JS }
        document::Script { src: TAWNY_PLAYER_CONTROLS_JS }
        // Must load before the router so its popstate listener registers first.
        document::Script { src: TAWNY_ROUTE_HISTORY_JS }
        NativePluginsProvider {
            AppStateProvider {
                ThemedApp {}
            }
        }
    }
}

#[component]
fn ThemedApp() -> Element {
    let app_state = use_context::<AppState>();
    let appearance = app_state.settings().appearance;

    // Material's Fade is a "fade through": the old page shrinks away, then the
    // new one grows in — the zoom-out/zoom-in that made tab switches feel
    // wrong. iOS Fade is a plain cross-dissolve, which is what a tab change
    // should look like.
    use_hook(|| set_platform(Platform::Ios));

    // View-transition snapshots are painted on the document element, outside
    // the wrapper that carries the theme variables, so the stylesheet's
    // `--color-bg` lookup missed and fell back to near-white. Mirroring the
    // real background onto the root keeps transitions dark.
    let background = tawny_theme(appearance).bg.clone();
    use_effect(move || {
        let background = background.clone();
        spawn(async move {
            let script = format!(
                "document.documentElement.style.setProperty('--route-transition-bg', {background:?});\
                 document.documentElement.style.setProperty('--color-bg', {background:?});\
                 dioxus.send(true);"
            );
            let mut eval = document::eval(&script);
            let _ = eval.recv::<bool>().await;
        });
    });

    rsx! {
        AppWrapper {
            theme: tawny_theme(appearance),
            disable_text_selection: true,
            // Provider only, never RouteTransitionRoot: AppWrapper already
            // carries the cover marker, and two elements claiming
            // `view-transition-name: cover` make the browser skip the
            // transition outright — it ran for one frame, then snapped.
            RouteTransitionProvider {
                Router::<Route> {}
            }
            AppOverlays {}
        }
    }
}
