use crate::{
    components::{
        AppOverlays, AppShell, ChannelDetail, Explore, Feed, HistoryPage, PlaylistDetail,
        Playlists, QueuePage, SettingsPage, Subscriptions, VideoDetail,
    },
    models::Appearance,
    state::{AppState, AppStateProvider},
};
use dioxus::prelude::*;
use g3_ui::{AppWrapper, Theme};

const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");
const SHAKA_PLAYER_JS: Asset = asset!("/node_modules/shaka-player/dist/shaka-player.compiled.js");
const TAWNY_TRANSPORT_JS: Asset = asset!("/assets/tawny_transport.js");
const TAWNY_PLAYER_CONTROLS_JS: Asset = asset!("/assets/tawny_player_controls.js");

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(AppShell)]
        #[route("/")]
        Feed {},
        #[route("/subscriptions")]
        Subscriptions {},
        #[route("/playlists")]
        Playlists {},
        #[route("/explore")]
        Explore {},
        #[route("/queue")]
        QueuePage {},
        #[route("/history")]
        HistoryPage {},
        #[route("/watch/:id")]
        VideoDetail { id: String },
        #[route("/playlists/:id")]
        PlaylistDetail { id: String },
        #[route("/settings")]
        SettingsPage {},
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
        document::Script { src: SHAKA_PLAYER_JS }
        document::Script { src: TAWNY_TRANSPORT_JS }
        document::Script { src: TAWNY_PLAYER_CONTROLS_JS }
        AppStateProvider {
            ThemedApp {}
        }
    }
}

#[component]
fn ThemedApp() -> Element {
    let app_state = use_context::<AppState>();
    let appearance = app_state.settings().appearance;
    rsx! {
        AppWrapper {
            key: "{appearance:?}",
            theme: tawny_theme(appearance),
            disable_text_selection: true,
            Router::<Route> {}
            AppOverlays {}
        }
    }
}
