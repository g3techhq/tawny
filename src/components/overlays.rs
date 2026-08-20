use crate::state::AppState;
use dioxus::prelude::*;
use dioxus_icons::lucide::{Check, ListPlus, Play, Plus, Rows3, Share2};
use g3_ui::{Button, ButtonStyle, Item, List, ListLines, Sheet, StatusColor, Toast};

#[component]
pub fn AppOverlays() -> Element {
    let mut app_state = use_context::<AppState>();
    let (message, color) = app_state.toast();
    let target = app_state.playlist_picker_video();
    let action_target = (app_state.video_actions_video)();

    let share_video = move |video_id: String, title: String| {
        spawn(async move {
            let url = format!("https://www.youtube.com/watch?v={video_id}");
            let payload = serde_json::to_string(&(title, url)).unwrap_or_default();
            let script = format!(
                r#"
                const [title, url] = {payload};
                if (navigator.share) await navigator.share({{ title, url }});
                else await navigator.clipboard.writeText(url);
                dioxus.send(true);
                "#
            );
            let mut eval = document::eval(&script);
            let _ = eval.recv::<bool>().await;
        });
    };

    rsx! {
        Sheet { is_open: app_state.video_actions_open, class: "video-actions-sheet",
            if let Some(video) = &action_target {
                {
                    let play_next_id = video.id.clone();
                    let queue_id = video.id.clone();
                    let save_video = video.clone();
                    let watched_id = video.id.clone();
                    let share_id = video.id.clone();
                    let share_title = video.title.clone();
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
                                    share_video(share_id.clone(), share_title.clone());
                                    app_state.video_actions_open.set(false);
                                    app_state.show_toast("Share sheet opened", StatusColor::Neutral);
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
        Toast {
            open: app_state.toast_open,
            message,
            color,
            duration_ms: 2800,
        }
    }
}
