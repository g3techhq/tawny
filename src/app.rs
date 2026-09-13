use crate::{
    components::{
        AccountGate, AppOverlays, AppShell, ChannelDetail, Explore, Feed, HistoryPage,
        PlaylistDetail, Playlists, QueuePage, SettingsPage, Subscriptions, VideoDetail,
    },
    models::{Appearance, PlatformStyle},
    state::{AppState, AppStateProvider},
};
use dioxus::prelude::*;
use g3_native_plugins::NativePluginsProvider;
use g3_route_transitions::{
    Platform as TransitionPlatform, ROUTE_TRANSITIONS_CSS, RouteTransitionProvider,
    init_auto_platform, route_transitions, set_platform,
};
use g3_ui::{AppWrapper, ComponentMode, Theme};

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
const TAWNY_HORIZONTAL_SCROLL_JS: Asset = asset!("/assets/tawny_horizontal_scroll.js");
const FAVICON_SVG: Asset = asset!("/assets/favicon.svg");

/// Primary destinations are stable roots. Pages reached from a card push above
/// them; the watch page and the three header destinations present as sheets.
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
        // Queue, History, and Settings are peers: each one's header offers the
        // other two, so a lateral move between them slides in the order the header
        // lists them instead of taking the unrelated-peer fade.
        #[transition(cover, forward = (HistoryPage, SettingsPage))]
        #[route("/queue")]
        QueuePage {},
        #[transition(cover, forward = SettingsPage)]
        #[route("/history")]
        HistoryPage {},
        // A new item in an active run replaces the current watch entry.  Back
        // then dismisses the player to the page beneath it instead of walking
        // back through every video autoplay (or the queue controls) visited.
        // Opening a video from Queue or History hands that sheet off to this
        // one: it rises like any video and takes the header sheet's place in
        // history, so minimizing lands on the page beneath both instead of
        // reopening the list the video was picked from.
        #[transition(cover, replace, replaces = (QueuePage, HistoryPage))]
        #[route("/watch/:id")]
        VideoDetail { id: String },
        #[transition(pushed)]
        #[route("/playlists/:id")]
        PlaylistDetail { id: String },
        #[transition(cover)]
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
        Appearance::Light => Theme {
            // Tawny in daylight: warm parchment and weathered-sage layers,
            // not a white system theme with an orange accent. Keep every
            // elevation visibly distinct, as the g3 playground themes do.
            focused: "#a9530b".into(),
            label_primary: "#342a21".into(),
            label_secondary: "#725f4d".into(),
            card_border: "#bea27f".into(),
            bg: "#d8c5a8".into(),
            bg_secondary: "#cbb595".into(),
            card: "#eadcc5".into(),
            card_inset: "#ddc9aa".into(),
            surface: "#e3d2b8".into(),
            control: "#d4bea0".into(),
            text: "#342a21".into(),
            text_secondary: "#725f4d".into(),
            shadow: "rgba(70, 45, 25, 0.24)".into(),
            success: "#178451".into(),
            warning: "#a9530b".into(),
            danger: "#c93b45".into(),
            color_scheme: "light".into(),
        },
    }
}

#[component]
pub fn App() -> Element {
    rsx! {
        // Declared rather than left to the browser, which otherwise probes
        // /favicon.ico on every load and takes a 404 for it.
        document::Link { rel: "icon", r#type: "image/svg+xml", href: FAVICON_SVG }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        // Without this the browser falls back to its own root cross-fade: no
        // cover keyframes, no base snapshot, and a transparent page behind it.
        document::Link { rel: "stylesheet", href: ROUTE_TRANSITIONS_CSS }
        document::Script { src: SHAKA_PLAYER_JS }
        document::Script { src: TAWNY_TRANSPORT_JS }
        document::Script { src: TAWNY_PLAYER_CONTROLS_JS }
        // Must load before the router so its popstate listener registers first.
        document::Script { src: TAWNY_ROUTE_HISTORY_JS }
        document::Script { src: TAWNY_HORIZONTAL_SCROLL_JS }
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
    let settings = app_state.settings();
    let appearance = settings.appearance;

    // g3-ui and the transition library each keep their own notion of platform,
    // and both have to agree or the components render one language while the
    // navigation animates in the other. `Auto` leaves each library on its own
    // detection, which is the right answer for a packaged build.
    let component_mode = match settings.platform_style {
        PlatformStyle::Auto => None,
        PlatformStyle::Ios => Some(ComponentMode::Ios),
        PlatformStyle::Material => Some(ComponentMode::Md),
    };
    // Read from the signal, not the `settings` copy above. Settings hydrate from
    // local storage after the first render, and an effect that reads no signal
    // runs exactly once: it applied the default and never saw the saved style,
    // so the components switched to iOS while every transition stayed Material.
    use_effect(move || match app_state.settings.read().platform_style {
        PlatformStyle::Auto => init_auto_platform(),
        PlatformStyle::Ios => set_platform(TransitionPlatform::Ios),
        PlatformStyle::Material => set_platform(TransitionPlatform::Md),
    });

    // View-transition snapshots are painted on the document element, outside
    // the wrapper that carries the theme variables, so the stylesheet's
    // `--color-bg` lookup missed and fell back to near-white. Mirroring the
    // real background onto the root keeps transitions on the theme's colour.
    use_effect(move || {
        // From the signal for the same reason as the platform above: a copy
        // taken at render time pinned the root to the default dark background,
        // so light mode kept a dark page wherever the root showed through.
        let background = tawny_theme(app_state.settings.read().appearance).bg.clone();
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
            mode: component_mode,
            disable_text_selection: true,
            // The sheet being presented owns the cover marker - the watch page,
            // or the body region on the three header destinations. This wrapper
            // must not claim it: it is an ancestor of the navbar that carries
            // the base marker, and a named descendant is lifted out of its
            // ancestor, so the cover would capture everything except the app
            // and slide an empty background over the page.
            route_transition_root: false,
            // The provider supplies the shared stylesheet; the platform above
            // decides whether it renders the iOS or the Material motion.
            RouteTransitionProvider {
                // Inside the wrapper so the setup screen is themed, and inside
                // the transition provider so it does not sit between the
                // navbar's base marker and the sheet that covers it. A
                // component adds no DOM node of its own, so passing children
                // through leaves the snapshot structure untouched.
                AccountGate {
                    Router::<Route> {}
                }
            }
            AppOverlays {}
        }
    }
}

#[cfg(test)]
mod transition_tests {
    use super::*;
    use g3_route_transitions::{NavigationAnimation, RouteTransitions};

    /// Queue, History, and Settings all reach each other from the header, so a
    /// lateral move is a push in header order rather than the fallback fade.
    #[test]
    fn auxiliary_peers_slide_in_header_order() {
        let queue = Route::QueuePage {};
        let history = Route::HistoryPage {};
        let settings = Route::SettingsPage {};

        assert_eq!(queue.transition_to(&history), NavigationAnimation::PushLeft);
        assert_eq!(
            history.transition_to(&settings),
            NavigationAnimation::PushLeft
        );
        assert_eq!(
            queue.transition_to(&settings),
            NavigationAnimation::PushLeft
        );

        assert_eq!(
            history.transition_to(&queue),
            NavigationAnimation::PushRight
        );
        assert_eq!(
            settings.transition_to(&history),
            NavigationAnimation::PushRight
        );
        assert_eq!(
            settings.transition_to(&queue),
            NavigationAnimation::PushRight
        );
    }

    /// Reached from a header action rather than a tab, the three present as
    /// sheets over whichever destination launched them. The forward edges that
    /// order them against each other must not steal that.
    #[test]
    fn auxiliary_pages_present_as_sheets_over_a_root() {
        let feed = Route::Feed {};
        let queue = Route::QueuePage {};

        assert_eq!(feed.transition_to(&queue), NavigationAnimation::CoverUp);
        assert_eq!(queue.transition_to(&feed), NavigationAnimation::UncoverDown);
        assert_eq!(queue.transition_back(), NavigationAnimation::UncoverDown);
    }

    /// Opening a channel from the watch sheet dismisses the sheet. A `forward`
    /// edge would read better going in, but it is bidirectional: it would also
    /// turn channel-to-watch into a push, and opening a video has to cover.
    #[test]
    fn opening_a_video_from_a_channel_still_covers() {
        let channel = Route::ChannelDetail {
            id: "channel".into(),
        };
        let watch = Route::VideoDetail { id: "video".into() };

        assert_eq!(channel.transition_to(&watch), NavigationAnimation::CoverUp);
        assert_eq!(
            watch.transition_to(&channel),
            NavigationAnimation::UncoverDown
        );
    }

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

    #[test]
    fn opening_a_video_from_a_header_sheet_replaces_that_sheet() {
        let watch = Route::VideoDetail { id: "video".into() };
        for sheet in [Route::QueuePage {}, Route::HistoryPage {}] {
            assert_eq!(sheet.transition_to(&watch), NavigationAnimation::CoverUp);
            assert!(sheet.replaces_history(&watch));
        }
        assert!(!Route::Feed {}.replaces_history(&watch));
        assert_eq!(watch.transition_back(), NavigationAnimation::UncoverDown);
    }

    #[test]
    fn advancing_between_videos_reuses_the_watch_history_entry() {
        let first = Route::VideoDetail { id: "first".into() };
        let next = Route::VideoDetail { id: "next".into() };

        assert!(first.replaces_history(&next));
        assert!(!Route::Feed {}.replaces_history(&next));
    }
}
