use crate::state::AppState;
use dioxus::prelude::*;
use dioxus_icons::lucide::{Check, Clock3, Copy, ListPlus, Play, Plus, Rows3, Share2};
use g3_ui::{Button, ButtonStyle, Item, List, ListLines, Modal, Sheet, StatusColor, Toast, Toggle};

fn youtube_share_url(video_id: &str, with_timestamp: bool, timestamp_seconds: u64) -> String {
    let mut url = format!("https://www.youtube.com/watch?v={video_id}");
    if with_timestamp && timestamp_seconds > 0 {
        url.push_str(&format!("&t={timestamp_seconds}s"));
    }
    url
}

fn timestamp_label(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let seconds = seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[component]
pub fn AppOverlays() -> Element {
    let mut app_state = use_context::<AppState>();
    #[cfg(any(
        target_arch = "wasm32",
        target_os = "android",
        target_os = "ios",
        target_os = "macos"
    ))]
    let mut plugins = use_context::<dx_native_plugins::NativePlugins>();
    let (message, color) = app_state.toast();
    let target = app_state.playlist_picker_video();
    let action_target = (app_state.video_actions_video)();
    let share_target = (app_state.share_video)();
    let share_url = share_target
        .as_ref()
        .map(|video| {
            youtube_share_url(
                &video.id,
                (app_state.share_with_timestamp)(),
                (app_state.share_timestamp_seconds)(),
            )
        })
        .unwrap_or_default();
    let share_action_url = share_url.clone();
    let copy_action_url = share_url.clone();

    rsx! {
        Sheet { is_open: app_state.video_actions_open, class: "video-actions-sheet",
            if let Some(video) = &action_target {
                {
                    let play_next_id = video.id.clone();
                    let queue_id = video.id.clone();
                    let save_video = video.clone();
                    let watched_id = video.id.clone();
                    let share_video = video.clone();
                    let is_queued = app_state.library().queue.iter().any(|id| id == &video.id);
                    let is_watched = video.watched;
                    rsx! {
                        div { class: "sheet-heading video-action-heading",
                            img { class: "action-sheet-thumbnail", src: "{video.thumbnail_url}", alt: "" }
                            div {
                                span { class: "section-kicker", "VIDEO ACTIONS" }
                                h2 { "{video.title}" }
                                p { "{video.channel_name}" }
                            }
                        }
                        List { inset: true, class: "video-action-list",
                            Item {
                                start: rsx! { Play { size: 18 } },
                                label: "Play next".to_string(),
                                description: "Move to the front of the queue".to_string(),
                                onclick: move |_| {
                                    let message = app_state.add_to_queue(&play_next_id, true);
                                    app_state.video_actions_open.set(false);
                                    app_state.show_toast(message, StatusColor::Success);
                                },
                            }
                            Item {
                                start: rsx! { Rows3 { size: 18 } },
                                label: if is_queued { "Remove from queue".to_string() } else { "Add to queue".to_string() },
                                description: "Keep your playback order offline".to_string(),
                                onclick: move |_| {
                                    let message = if is_queued {
                                        app_state.remove_from_queue(&queue_id);
                                        "Removed from queue".to_string()
                                    } else {
                                        app_state.add_to_queue(&queue_id, false)
                                    };
                                    app_state.video_actions_open.set(false);
                                    app_state.show_toast(message, StatusColor::Neutral);
                                },
                            }
                            Item {
                                start: rsx! { ListPlus { size: 18 } },
                                label: "Save to playlist".to_string(),
                                description: "Choose a local or synced list".to_string(),
                                onclick: move |_| {
                                    app_state.video_actions_open.set(false);
                                    app_state.playlist_picker_video.set(Some(save_video.clone()));
                                    app_state.playlist_picker_open.set(true);
                                },
                            }
                            Item {
                                start: rsx! { Check { size: 18 } },
                                label: if is_watched { "Mark unwatched".to_string() } else { "Mark watched".to_string() },
                                onclick: move |_| {
                                    app_state.mark_watched(&watched_id, !is_watched);
                                    app_state.video_actions_open.set(false);
                                    app_state.show_toast(
                                        if is_watched { "Marked unwatched" } else { "Marked watched" },
                                        StatusColor::Neutral,
                                    );
                                },
                            }
                            Item {
                                start: rsx! { Share2 { size: 18 } },
                                label: "Share".to_string(),
                                // Keep the final action divider-free even when
                                // a sheet appends animation/focus nodes after it.
                                lines: ListLines::None,
                                onclick: move |_| {
                                    app_state.video_actions_open.set(false);
                                    app_state.open_share(share_video.clone());
                                },
                            }
                        }
                    }
                }
            }
        }
        Sheet { is_open: app_state.playlist_picker_open, class: "playlist-picker-sheet",
            div { class: "sheet-heading",
                div { class: "sheet-icon", ListPlus { size: 22 } }
                div {
                    span { class: "section-kicker", "SAVE VIDEO" }
                    h2 { "Choose a playlist" }
                    if let Some(video) = &target {
                        p { "{video.title}" }
                    }
                }
            }
            List { inset: true,
                for playlist in app_state.library().playlists {
                    {
                        let video_id = target.as_ref().map(|video| video.id.clone()).unwrap_or_default();
                        let playlist_id = playlist.id.clone();
                        let playlist_name = playlist.name.clone();
                        let already_saved = playlist.video_ids.iter().any(|id| id == &video_id);
                        rsx! {
                            Item {
                                key: "{playlist.id}",
                                start: if already_saved { Some(rsx! { Check { size: 18 } }) } else { None },
                                label: playlist.name,
                                description: if already_saved {
                                    format!("Saved · {} videos", playlist.video_ids.len())
                                } else {
                                    format!("{} videos", playlist.video_ids.len())
                                },
                                onclick: move |_| {
                                    if let Some(message) = app_state.add_to_playlist(&video_id, &playlist_id) {
                                        app_state.show_toast(message, StatusColor::Success);
                                    } else {
                                        app_state.show_toast(format!("Could not open {playlist_name}"), StatusColor::Danger);
                                    }
                                    app_state.playlist_picker_open.set(false);
                                },
                            }
                        }
                    }
                }
            }
            Button {
                expand: true,
                style: ButtonStyle::Neutral,
                start: rsx! { Plus { size: 18 } },
                onclick: move |_| {
                    app_state.playlist_picker_open.set(false);
                    app_state.show_toast("Create playlists from the Playlists tab", StatusColor::Neutral);
                },
                "New playlist"
            }
        }
        Modal {
            open: app_state.share_open,
            title: "Share video".to_string(),
            class: "share-video-modal",
            description: if let Some(video) = &share_target {
                Some(rsx! {
                    div { class: "share-video-summary",
                        strong { "{video.title}" }
                        code { "{share_url}" }
                    }
                })
            } else {
                None
            },
            actions: rsx! {
                Button {
                    style: ButtonStyle::Clear,
                    onclick: move |_| app_state.share_open.set(false),
                    "Cancel"
                }
                Button {
                    start: rsx! { Share2 { size: 17 } },
                    onclick: move |_| {
                        let link = share_action_url.clone();
                        let text = share_target
                            .as_ref()
                            .map(|video| format!("{}\n{}", video.title, link))
                            .unwrap_or(link);
                        cfg_if::cfg_if! {
                            if #[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios", target_os = "macos"))] {
                                if let Err(error) = plugins.clipboard.write().share(text) {
                                    app_state.show_toast(error, StatusColor::Warning);
                                }
                            } else {
                                let _ = text;
                                app_state.show_toast("Sharing is not available on this platform", StatusColor::Warning);
                            }
                        }
                        app_state.share_open.set(false);
                    },
                    "Share"
                }
            },
            div { class: "share-video-options",
                div { class: "share-time-row",
                    Clock3 { size: 19 }
                    div {
                        strong { "Start at {timestamp_label((app_state.share_timestamp_seconds)())}" }
                        span { "Add the current playback position to the link" }
                    }
                    Toggle {
                        checked: app_state.share_with_timestamp,
                    }
                }
                Button {
                    style: ButtonStyle::Neutral,
                    expand: true,
                    start: rsx! { Copy { size: 17 } },
                    onclick: move |_| {
                        let link = copy_action_url.clone();
                        cfg_if::cfg_if! {
                            if #[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios", target_os = "macos"))] {
                                match plugins.clipboard.write().copy_to_clipboard(link) {
                                    Ok(()) => app_state.show_toast("Link copied", StatusColor::Success),
                                    Err(error) => app_state.show_toast(error, StatusColor::Warning),
                                }
                            } else {
                                let _ = link;
                                app_state.show_toast("Copy is not available on this platform", StatusColor::Warning);
                            }
                        }
                    },
                    "Copy link"
                }
            }
        }
        Toast {
            open: app_state.toast_open,
            message,
            color,
            duration_ms: 2800,
        }
    }
}
