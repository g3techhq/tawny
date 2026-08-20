use crate::{
    models::{Appearance, SwipeActionKind},
    state::AppState,
};
use dioxus::prelude::*;
use dioxus_icons::lucide::{Database, Gauge, HardDrive, ListPlus, RotateCw, Server, ShieldCheck};
use g3_ui::{Badge, Card, Field, Item, List, RightSlot, Select, SelectOption, StatusColor, Toggle};

#[component]
pub fn SettingsPage() -> Element {
    let mut app_state = use_context::<AppState>();

    let settings = app_state.settings();
    let library = app_state.library();
    let start_name = library
        .playlists
        .iter()
        .find(|playlist| playlist.id == settings.swipe_right_playlist_id)
        .map(|playlist| playlist.name.clone())
        .unwrap_or_else(|| "Watch later".into());
    let end_name = library
        .playlists
        .iter()
        .find(|playlist| playlist.id == settings.swipe_left_playlist_id)
        .map(|playlist| playlist.name.clone())
        .unwrap_or_else(|| "Deep dives".into());
    let start_value = use_signal(|| start_name);
    let end_value = use_signal(|| end_name);
    let start_action = use_signal(|| settings.swipe_right_action.label().to_string());
    let end_action = use_signal(|| settings.swipe_left_action.label().to_string());
    let action_options = SwipeActionKind::ALL
        .iter()
        .map(|kind| SelectOption::from(kind.label().to_string()))
        .collect::<Vec<_>>();
    let action_options_for_end = action_options.clone();
    let options = library
        .playlists
        .iter()
        .map(|playlist| SelectOption::from(playlist.name.clone()))
        .collect::<Vec<_>>();
    let options_for_end = options.clone();
    let cache_size = library.videos.len() + library.channels.len() + library.playlists.len();
    let autoplay = use_signal(|| settings.autoplay);
    let prefer_sabr = use_signal(|| settings.prefer_sabr);
    let dark_appearance = use_signal(|| settings.appearance == Appearance::Dark);
    let hide_watched = use_signal(|| settings.hide_watched);
    let auto_landscape = use_signal(|| settings.auto_landscape_fullscreen);
    let video_short_minutes = use_signal(|| (settings.video_short_max_seconds / 60).to_string());
    let video_medium_minutes = use_signal(|| (settings.video_medium_max_seconds / 60).to_string());
    let shorts_short_seconds = use_signal(|| settings.shorts_short_max_seconds.to_string());
    let shorts_medium_seconds = use_signal(|| settings.shorts_medium_max_seconds.to_string());

    rsx! {
        div { class: "page settings-page",
                main { class: "settings-content",
                    section { class: "settings-section",
                        span { class: "section-kicker", "GESTURES" }
                        Card { title: "Swipe actions".to_string(),
                            div { class: "setting-row",
                                div { class: "setting-icon", ListPlus { size: 19 } }
                                div { class: "setting-copy",
                                    strong { "Swipe right" }
                                    span { "What the gesture does" }
                                }
                                Select {
                                    value: start_action,
                                    options: action_options,
                                    onchange: move |label: String| {
                                        if let Some(kind) = SwipeActionKind::from_label(&label) {
                                            app_state.settings.write().swipe_right_action = kind;
                                        }
                                    },
                                }
                            }
                            // The playlist picker only matters when the action
                            // is "Add to playlist", so it is hidden otherwise.
                            if settings.swipe_right_action == SwipeActionKind::AddToPlaylist {
                                div { class: "setting-row setting-row-nested",
                                    div { class: "setting-copy",
                                        span { "Destination playlist" }
                                    }
                                    Select {
                                        value: start_value,
                                        options,
                                        onchange: move |name: String| {
                                            if let Some(playlist) = app_state.library().playlists.iter().find(|playlist| playlist.name == name) {
                                                app_state.settings.write().swipe_right_playlist_id = playlist.id.clone();
                                            }
                                        },
                                    }
                                }
                            }
                            div { class: "setting-divider" }
                            div { class: "setting-row",
                                div { class: "setting-icon flip", ListPlus { size: 19 } }
                                div { class: "setting-copy",
                                    strong { "Swipe left" }
                                    span { "What the gesture does" }
                                }
                                Select {
                                    value: end_action,
                                    options: action_options_for_end,
                                    onchange: move |label: String| {
                                        if let Some(kind) = SwipeActionKind::from_label(&label) {
                                            app_state.settings.write().swipe_left_action = kind;
                                        }
                                    },
                                }
                            }
                            if settings.swipe_left_action == SwipeActionKind::AddToPlaylist {
                                div { class: "setting-row setting-row-nested",
                                    div { class: "setting-copy",
                                        span { "Destination playlist" }
                                    }
                                    Select {
                                        value: end_value,
                                        options: options_for_end,
                                        onchange: move |name: String| {
                                            if let Some(playlist) = app_state.library().playlists.iter().find(|playlist| playlist.name == name) {
                                                app_state.settings.write().swipe_left_playlist_id = playlist.id.clone();
                                            }
                                        },
                                    }
                                }
                            }
                        }
                    }

                    section { class: "settings-section",
                        span { class: "section-kicker", "PLAYBACK" }
                        List { inset: true,
                            Item {
                                start: rsx! { Gauge { size: 19 } },
                                label: "Video speed".to_string(),
                                description: "Remembered for long-form video".to_string(),
                                metadata: format!("{}×", settings.playback_speed),
                            }
                            Item {
                                start: rsx! { Gauge { size: 19 } },
                                label: "Shorts speed".to_string(),
                                description: "Remembered separately from video".to_string(),
                                metadata: format!("{}×", settings.shorts_playback_speed),
                            }
                            Item {
                                label: "Autoplay".to_string(),
                                description: "Continue with the next video".to_string(),
                                end: rsx! {
                                    Toggle {
                                        checked: autoplay,
                                        onchange: move |enabled| app_state.settings.write().autoplay = enabled,
                                    }
                                },
                            }
                            Item {
                                start: rsx! { RotateCw { size: 19 } },
                                label: "Rotate horizontal fullscreen video".to_string(),
                                description: "Switch phones to landscape automatically".to_string(),
                                end: rsx! {
                                    Toggle {
                                        checked: auto_landscape,
                                        onchange: move |enabled| app_state.settings.write().auto_landscape_fullscreen = enabled,
                                    }
                                },
                            }
                            Item {
                                start: rsx! { ShieldCheck { size: 19 } },
                                label: "Prefer SABR".to_string(),
                                description: "Use adaptive streaming when the stream supports it".to_string(),
                                end: rsx! {
                                    Toggle {
                                        checked: prefer_sabr,
                                        onchange: move |enabled| app_state.settings.write().prefer_sabr = enabled,
                                    }
                                },
                            }
                        }
                    }

                    section { class: "settings-section",
                        span { class: "section-kicker", "APPEARANCE & FEED" }
                        List { inset: true,
                            Item {
                                label: "Dark appearance".to_string(),
                                description: "Use Tawny's charcoal palette".to_string(),
                                end: rsx! {
                                    Toggle {
                                        checked: dark_appearance,
                                        onchange: move |dark| {
                                            app_state.settings.write().appearance = if dark { Appearance::Dark } else { Appearance::Light };
                                        },
                                    }
                                },
                            }
                            Item {
                                label: "Hide watched videos".to_string(),
                                description: "Keep the feed focused on what is new".to_string(),
                                end: rsx! {
                                    Toggle {
                                        checked: hide_watched,
                                        onchange: move |hidden| app_state.settings.write().hide_watched = hidden,
                                    }
                                },
                            }
                        }
                        Card { class: "duration-settings-card", title: "Duration filters".to_string(),
                            p { class: "settings-card-copy", "Set where Short ends and Medium begins for each kind of upload." }
                            div { class: "duration-settings-grid",
                                Field {
                                    label: "Video · Short max (minutes)".to_string(),
                                    value: video_short_minutes,
                                    r#type: "number".to_string(),
                                    min: 1,
                                    max: 240,
                                    onchange: move |event: Event<FormData>| {
                                        if let Ok(minutes) = event.value().parse::<u64>() {
                                            let mut settings = app_state.settings.write();
                                            settings.video_short_max_seconds = minutes * 60;
                                            settings.video_medium_max_seconds = settings.video_medium_max_seconds.max((minutes + 1) * 60);
                                        }
                                    },
                                }
                                Field {
                                    label: "Video · Medium max (minutes)".to_string(),
                                    value: video_medium_minutes,
                                    r#type: "number".to_string(),
                                    min: 2,
                                    max: 600,
                                    onchange: move |event: Event<FormData>| {
                                        if let Ok(minutes) = event.value().parse::<u64>() {
                                            let mut settings = app_state.settings.write();
                                            settings.video_medium_max_seconds = (minutes * 60).max(settings.video_short_max_seconds + 60);
                                        }
                                    },
                                }
                                Field {
                                    label: "Shorts · Short max (seconds)".to_string(),
                                    value: shorts_short_seconds,
                                    r#type: "number".to_string(),
                                    min: 5,
                                    max: 170,
                                    onchange: move |event: Event<FormData>| {
                                        if let Ok(seconds) = event.value().parse::<u64>() {
                                            let mut settings = app_state.settings.write();
                                            settings.shorts_short_max_seconds = seconds;
                                            settings.shorts_medium_max_seconds = settings.shorts_medium_max_seconds.max(seconds + 1);
                                        }
                                    },
                                }
                                Field {
                                    label: "Shorts · Medium max (seconds)".to_string(),
                                    value: shorts_medium_seconds,
                                    r#type: "number".to_string(),
                                    min: 6,
                                    max: 180,
                                    onchange: move |event: Event<FormData>| {
                                        if let Ok(seconds) = event.value().parse::<u64>() {
                                            let mut settings = app_state.settings.write();
                                            settings.shorts_medium_max_seconds = seconds.max(settings.shorts_short_max_seconds + 1);
                                        }
                                    },
                                }
                            }
                        }
                    }

                    section { class: "settings-section",
                        span { class: "section-kicker", "DATA" }
                        div { class: "data-grid",
                            Card { class: "data-card", title: "Local cache".to_string(), right_slot: RightSlot::Element(rsx! { HardDrive { size: 18 } }),
                                strong { "{cache_size}" }
                                span { "cached records" }
                            }
                            Card { class: "data-card", title: "Sync server".to_string(), right_slot: RightSlot::Element(rsx! { Server { size: 18 } }),
                                Badge { color: StatusColor::Success, "Ready" }
                                span { "SurrealDB-backed" }
                            }
                            Card { class: "data-card", title: "Playback".to_string(), right_slot: RightSlot::Element(rsx! { Database { size: 18 } }),
                                Badge { color: StatusColor::Accent, "Built-in" }
                                span { "PoToken per request" }
                            }
                        }
                    }
                }
        }
    }
}
