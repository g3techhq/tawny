use crate::{app::Route, models::Playlist, state::AppState};
use dioxus::prelude::*;
use dioxus_icons::lucide::{CheckCheck, ListPlus, Pin, Plus, Trash2};
use g3_ui::{Button, ButtonStyle, Card, Field, Modal, RightSlot, StatusColor};

use super::VideoGrid;

fn playlist_thumbnails(state: AppState, playlist: &Playlist) -> Vec<String> {
    playlist
        .video_ids
        .iter()
        .filter_map(|id| {
            state
                .library()
                .videos
                .iter()
                .find(|video| &video.id == id)
                .map(|video| video.thumbnail_url.clone())
        })
        .take(3)
        .collect()
}

#[component]
pub fn Playlists() -> Element {
    let app_state = use_context::<AppState>();
    let navigator = use_navigator();
    let mut create_open = use_signal(|| false);
    let mut playlist_name = use_signal(String::new);
    let mut delete_target = use_signal(|| None::<(String, String)>);
    let mut delete_open = use_signal(|| false);
    let playlists = app_state.library().playlists;
    let delete_name = delete_target()
        .map(|(_, name)| name)
        .unwrap_or_else(|| "This playlist".into());

    rsx! {
        main { class: "page playlists-page",
            div { class: "page-actions-row",
                Button {
                    start: rsx! { Plus { size: 17 } },
                    onclick: move |_| create_open.set(true),
                    "New playlist"
                }
            }
            div { class: "playlist-grid",
                for playlist in playlists {
                    {
                        let playlist_id = playlist.id.clone();
                        let pin_id = playlist.id.clone();
                        let remove_id = playlist.id.clone();
                        let remove_name = playlist.name.clone();
                        let thumbs = playlist_thumbnails(app_state, &playlist);
                        let video_count = playlist.video_ids.len();
                        let is_pinned = playlist.pinned;
                        rsx! {
                            Card {
                                key: "{playlist.id}",
                                class: "playlist-card",
                                title: playlist.name.clone(),
                                // Pin and delete live with the playlist itself
                                // rather than in a second top bar on its page.
                                right_slot: RightSlot::Element(rsx! {
                                    div { class: "playlist-card-actions",
                                        button {
                                            class: if is_pinned { "playlist-card-action pinned" } else { "playlist-card-action" },
                                            aria_label: if is_pinned { "Unpin playlist" } else { "Pin playlist" },
                                            title: if is_pinned { "Unpin playlist" } else { "Pin playlist" },
                                            onclick: move |event: MouseEvent| {
                                                event.stop_propagation();
                                                if let Some(pinned) = app_state.toggle_playlist_pin(&pin_id) {
                                                    app_state.show_toast(
                                                        if pinned { "Playlist pinned" } else { "Playlist unpinned" },
                                                        StatusColor::Neutral,
                                                    );
                                                }
                                            },
                                            Pin { size: 15 }
                                        }
                                        button {
                                            class: "playlist-card-action danger",
                                            aria_label: "Delete playlist".to_string(),
                                            title: "Delete playlist".to_string(),
                                            onclick: move |event: MouseEvent| {
                                                event.stop_propagation();
                                                delete_target.set(Some((remove_id.clone(), remove_name.clone())));
                                                delete_open.set(true);
                                            },
                                            Trash2 { size: 15 }
                                        }
                                    }
                                }),
                                onclick: move |_| { navigator.push(Route::PlaylistDetail { id: playlist_id.clone() }); },
                                div { class: "playlist-collage",
                                    if thumbs.is_empty() {
                                        div { class: "playlist-empty-art", ListPlus { size: 30 } }
                                    } else {
                                        for thumbnail in thumbs {
                                            img { src: "{thumbnail}", alt: "", loading: "lazy" }
                                        }
                                    }
                                }
                                p { "{playlist.description}" }
                                span { class: "playlist-count", "{video_count} videos" }
                            }
                        }
                    }
                }
            }
        }
        Modal {
            open: delete_open,
            title: "Delete playlist?".to_string(),
            description: rsx! { p { "{delete_name} will be removed from this device and your sync server." } },
            actions: rsx! {
                Button { style: ButtonStyle::Clear, onclick: move |_| delete_open.set(false), "Cancel" }
                Button {
                    style: ButtonStyle::Danger,
                    onclick: move |_| {
                        if let Some((id, _)) = delete_target()
                            && let Some(name) = app_state.delete_playlist(&id)
                        {
                            app_state.show_toast(format!("Deleted {name}"), StatusColor::Neutral);
                        }
                        delete_target.set(None);
                        delete_open.set(false);
                    },
                    "Delete"
                }
            },
        }
        Modal {
            open: create_open,
            title: "New playlist".to_string(),
            description: rsx! { p { "It will be cached on this device immediately." } },
            actions: rsx! {
                Button { style: ButtonStyle::Clear, onclick: move |_| create_open.set(false), "Cancel" }
                Button {
                    disabled: playlist_name().trim().is_empty(),
                    onclick: move |_| {
                        let name = playlist_name().trim().to_string();
                        if name.is_empty() { return; }
                        app_state.create_playlist(name.clone());
                        playlist_name.set(String::new());
                        create_open.set(false);
                        app_state.show_toast(format!("Created {name}"), StatusColor::Success);
                    },
                    "Create"
                }
            },
            Field {
                label: "Playlist name".to_string(),
                value: playlist_name,
                placeholder: "Sunday watchlist".to_string(),
                autofocus: true,
            }
        }
    }
}

#[component]
pub fn PlaylistDetail(id: String) -> Element {
    let app_state = use_context::<AppState>();
    let library = app_state.library();
    let playlist = library
        .playlists
        .iter()
        .find(|playlist| playlist.id == id)
        .cloned();

    let Some(playlist) = playlist else {
        return rsx! {
            main { class: "page",
                div { class: "empty-state", p { "This playlist is not in the local cache." } }
            }
        };
    };

    let videos = playlist
        .video_ids
        .iter()
        .filter_map(|id| library.videos.iter().find(|video| &video.id == id).cloned())
        .collect::<Vec<_>>();
    let video_count = videos.len();
    let watched_count = videos.iter().filter(|video| video.watched).count();
    let clear_id = playlist.id.clone();

    rsx! {
        main { class: "page playlist-detail-page",
            div { class: "page-actions-row",
                span { class: "playlist-detail-count", "{video_count} saved" }
                Button {
                    style: ButtonStyle::Neutral,
                    disabled: watched_count == 0,
                    start: rsx! { CheckCheck { size: 16 } },
                    onclick: move |_| {
                        let removed = app_state.remove_watched_from_playlist(&clear_id);
                        if removed > 0 {
                            app_state.show_toast(
                                format!("Removed {removed} watched video{}", if removed == 1 { "" } else { "s" }),
                                StatusColor::Success,
                            );
                        }
                    },
                    "Remove watched"
                }
            }
            VideoGrid {
                videos,
                playlist_id: playlist.id.clone(),
                empty_message: "Add videos from the buttons on a card.".to_string(),
            }
        }
    }
}
