use crate::{models::Appearance, state::AppState};
use dioxus::prelude::*;
use dioxus_icons::lucide::{
    Database, Gauge, HardDrive, ListPlus, Server, ShieldCheck,
};
use g3_ui::{
    Badge, Card, Item, List, RightSlot, Select, SelectOption,
    StatusColor, Toggle,
};

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

    rsx! {
        div { class: "settings-page",
                main { class: "settings-content",
                    div { class: "settings-hero",
                        span { class: "brand-mark brand-mark-large", "T" }
                        div {
                            span { class: "section-kicker", "TAWNY 0.1" }
                            h2 { "Built to feel quiet." }
                            p { "Your feed and library stay useful even when the network does not." }
                        }
                    }

                    section { class: "settings-section",
                        span { class: "section-kicker", "GESTURES" }
                        Card { title: "Swipe destinations".to_string(),
                            div { class: "setting-row",
                                div { class: "setting-icon", ListPlus { size: 19 } }
                                div { class: "setting-copy",
                                    strong { "Swipe right" }
                                    span { "Quick-save destination" }
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
                            div { class: "setting-divider" }
                            div { class: "setting-row",
                                div { class: "setting-icon flip", ListPlus { size: 19 } }
                                div { class: "setting-copy",
                                    strong { "Swipe left" }
                                    span { "Quick-save destination" }
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
