use crate::{
    components::{
        AppOverlays, AppShell, ChannelDetail, Explore, Feed, HistoryPage, PlaylistDetail,
        Playlists, QueuePage, SettingsPage, Subscriptions, VideoDetail,
    },
    models::Appearance,
    state::{AppState, AppStateProvider},
};
use dioxus::prelude::*;
use g3_native_plugins::NativePluginsProvider;
use g3_route_transitions::{ROUTE_TRANSITIONS_CSS, RouteTransitionProvider, route_transitions};
use g3_ui::{AppWrapper, Theme};

// Statically headed so web builds block first paint on it, the same as
// g3-ui.css - linking it only at runtime left the app rendering unstyled while
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

/// Primary destinations are stable roots, pages with a back affordance are
/// pushed above them, and the watch page remains the one true sheet.
#[route_transitions]
#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(AppShell)]
        #[transition(root)]
        #[route("/")]
        Feed {},
        #[transition(root)]
        #[route("/subscriptions")]
        Subscriptions {},
        #[transition(root)]
        #[route("/playlists")]
        Playlists {},
        #[transition(root)]
        #[route("/explore")]
        Explore {},
        #[transition(pushed)]
        #[route("/queue")]
        QueuePage {},
        #[transition(pushed)]
        #[route("/history")]
        HistoryPage {},
        #[transition(cover)]
        #[route("/watch/:id")]
        VideoDetail { id: String },
        #[transition(pushed)]
        #[route("/playlists/:id")]
        PlaylistDetail { id: String },
        #[transition(pushed)]
        #[route("/settings")]
        SettingsPage {},
        #[transition(pushed)]
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
            // The player owns the cover marker; the provider supplies the
            // shared stylesheet while the library auto-detects iOS vs MD.
            RouteTransitionProvider {
                Router::<Route> {}
            }
            AppOverlays {}
        }
    }
}

#[cfg(test)]
mod transition_tests {
    use super::*;
    use g3_route_transitions::{NavigationAnimation, RouteTransitions};

    #[test]
    fn primary_destinations_cross_fade() {
        assert_eq!(
            Route::Feed {}.transition_to(&Route::Subscriptions {}),
            NavigationAnimation::Fade
        );
    }

    #[test]
    fn detail_pages_push_and_pop() {
        let root = Route::Playlists {};
        let detail = Route::PlaylistDetail {
            id: "playlist:test".into(),
        };

        assert_eq!(root.transition_to(&detail), NavigationAnimation::PushLeft);
        assert_eq!(detail.transition_back(), NavigationAnimation::PushRight);
    }

    #[test]
    fn player_covers_and_uncovers_the_current_page() {
        let root = Route::Feed {};
        let player = Route::VideoDetail { id: "video".into() };

        assert_eq!(root.transition_to(&player), NavigationAnimation::CoverUp);
        assert_eq!(player.transition_back(), NavigationAnimation::UncoverDown);
    }
}
