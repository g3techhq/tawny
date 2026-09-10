use crate::{app::Route, models::Video, state::AppState};
use dioxus::prelude::*;
use dioxus_icons::lucide::{Check, EllipsisVertical, ListPlus, Trash2, User};
use g3_route_transitions::animated_navigate;
use g3_ui::{
    Badge, Button, ButtonStyle, StatusColor, SwipeAction, SwipeBehavior, SwipeItem, SwipeSide,
    SwipeState,
};

#[component]
pub fn VideoGrid(
    videos: Vec<Video>,
    empty_message: Option<String>,
    playlist_id: Option<String>,
    shorts_layout: Option<bool>,
) -> Element {
    let app_state = use_context::<AppState>();
    let empty_copy =
        empty_message.unwrap_or_else(|| "Your cached library will appear here.".to_string());
    rsx! {
        if videos.is_empty() {
            div { class: "empty-state",
                div { class: "empty-icon", User { size: 25 } }
                h3 { "Nothing here yet" }
                p { "{empty_copy}" }
            }
        } else {
            div { class: if shorts_layout.unwrap_or(false) { "video-grid video-grid-shorts" } else { "video-grid" },
                for video in videos {
                    {
                        let video_id = video.id.clone();
                        let playlist_id = playlist_id.clone();
                        rsx! {
                            div { class: "video-grid-cell", key: "{video.id}",
                                VideoCard { video }
                                // Sits on the thumbnail rather than below the
                                // card so it does not add a row of chrome to
                                // every tile in a playlist.
                                if let Some(playlist_id) = playlist_id {
                                    button {
                                        class: "playlist-remove-video",
                                        aria_label: "Remove from playlist",
                                        title: "Remove from playlist",
                                        onclick: move |event: MouseEvent| {
                                            event.stop_propagation();
                                            if app_state.remove_from_playlist(&video_id, &playlist_id) {
                                                app_state.show_toast("Removed from playlist", StatusColor::Neutral);
                                            }
                                        },
                                        Trash2 { size: 15 }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn VideoCard(video: Video) -> Element {
    let mut app_state = use_context::<AppState>();
    let start_playlist_name = app_state.swipe_action_label(true);
    let end_playlist_name = app_state.swipe_action_label(false);
    let button_start_name = start_playlist_name.clone();
    let button_end_name = end_playlist_name.clone();
    let button_start_video = video.id.clone();
    let button_end_video = video.id.clone();
    let open_id = video.id.clone();
    let meta_open_id = video.id.clone();
    let keyboard_open_id = video.id.clone();
    let start_video_id = video.id.clone();
    let end_video_id = video.id.clone();
    let full_video_id = video.id.clone();
    let avatar_channel_id = video.channel_id.clone();
    let watched_video_id = video.id.clone();
    let menu_video = video.clone();
    let channel_avatar_url = app_state.with_library(|library| {
        library
            .channels
            .iter()
            .find(|channel| channel.id == video.channel_id)
            .and_then(|channel| channel.avatar_url.clone())
    });
    let keyboard_video = video.clone();
    let open_video = video.clone();
    let meta_open_video = video.clone();
    let progress = video.progress_percent();

    rsx! {
        SwipeItem {
            class: "video-swipe-row",
            behavior: SwipeBehavior::Activate,
            start_actions: rsx! {
                SwipeAction {
                    side: SwipeSide::Start,
                    accent: true,
                    onclick: move |_| app_state.run_swipe_action(&start_video_id, true),
                    div { class: "swipe-action-content",
                        ListPlus { size: 22 }
                        span { "{start_playlist_name}" }
                    }
                }
            },
            end_actions: rsx! {
                SwipeAction {
                    side: SwipeSide::End,
                    onclick: move |_| app_state.run_swipe_action(&end_video_id, false),
                    div { class: "swipe-action-content",
                        ListPlus { size: 22 }
                        span { "{end_playlist_name}" }
                    }
                }
            },
            on_swipe_action: move |swipe: SwipeState| {
                app_state.run_swipe_action(&full_video_id, swipe.side == SwipeSide::Start);
            },
            article {
                class: "video-card",
                tabindex: "0",
                role: "button",
                onkeydown: move |event| {
                    if event.key() == Key::Enter {
                        app_state.play(keyboard_video.clone());
                        app_state.record_history(&keyboard_open_id);
                        { let v = keyboard_open_id.clone(); spawn(async move { animated_navigate(Route::VideoDetail { id: v }).await; }); };
                    }
                },
                div {
                    class: "thumbnail-shell",
                    onclick: move |_| {
                        app_state.play(open_video.clone());
                        app_state.record_history(&open_id);
                        { let v = open_id.clone(); spawn(async move { animated_navigate(Route::VideoDetail { id: v }).await; }); };
                    },
                    img {
                        class: "video-thumbnail",
                        src: "{video.thumbnail_url}",
                        alt: "Thumbnail for {video.title}",
                        loading: "lazy",
                    }
                    div { class: "thumbnail-vignette" }
                    // Pointer equivalents of the swipe gestures. A swipe is
                    // awkward with a mouse, so desktop gets explicit controls
                    // on each side of the thumbnail; CSS hides them on touch
                    // layouts where the gesture is the better affordance.
                    button {
                        class: "card-quick-action card-quick-action-start",
                        aria_label: "Add to {button_start_name}",
                        title: "Add to {button_start_name}",
                        onclick: move |event: MouseEvent| {
                            event.stop_propagation();
                            app_state.run_swipe_action(&button_start_video, true);
                        },
                        ListPlus { size: 18 }
                    }
                    button {
                        class: "card-quick-action card-quick-action-end",
                        aria_label: "Add to {button_end_name}",
                        title: "Add to {button_end_name}",
                        onclick: move |event: MouseEvent| {
                            event.stop_propagation();
                            app_state.run_swipe_action(&button_end_video, false);
                        },
                        ListPlus { size: 18 }
                    }
                    // Listings that come from a flat playlist carry no runtime.
                    // No badge is honest; "0:00" is not.
                    if video.is_live {
                        Badge { color: StatusColor::Danger, class: "duration-badge", "LIVE" }
                    } else if video.duration_seconds > 0 {
                        span { class: "duration-badge", "{video.duration_label()}" }
                    }
                    if progress > 0.0 {
                        div { class: "watch-progress", style: "--progress: {progress}%;" }
                    }
                    if video.watched {
                        span { class: "watched-badge", Check { size: 13 } "Watched" }
                    }
                }
                div { class: "video-meta-row",
                    button {
                        class: "channel-avatar-link",
                        aria_label: "Open {video.channel_name}",
                        onclick: move |event: MouseEvent| {
                            event.stop_propagation();
                            let id = avatar_channel_id.clone();
                            spawn(async move { animated_navigate(Route::ChannelDetail { id }).await; });
                        },
                        if let Some(avatar_url) = channel_avatar_url {
                            img {
                                class: "channel-avatar",
                                src: "{avatar_url}",
                                alt: "",
                                loading: "lazy",
                            }
                        } else {
                            span { class: "channel-avatar channel-avatar-fallback", User { size: 18 } }
                        }
                    }
                    div {
                        class: "video-copy",
                        onclick: move |_| {
                            app_state.play(meta_open_video.clone());
                            app_state.record_history(&meta_open_id);
                            { let v = meta_open_id.clone(); spawn(async move { animated_navigate(Route::VideoDetail { id: v }).await; }); };
                        },
                        h2 { "{video.title}" }
                        if !video.channel_name.is_empty() {
                            p { "{video.channel_name}" }
                        }
                        p { class: "video-stats", "{video.stats_label()}" }
                    }
                    Button {
                        style: ButtonStyle::Clear,
                        aria_label: "Video actions".to_string(),
                        class: "card-menu-button",
                        onclick: move |event: MouseEvent| {
                            event.stop_propagation();
                            app_state.video_actions_video.set(Some(menu_video.clone()));
                            app_state.video_actions_open.set(true);
                        },
                        EllipsisVertical { size: 21 }
                    }
                }
                button {
                    class: "sr-action",
                    onclick: move |event| {
                        event.stop_propagation();
                        app_state.mark_watched(&watched_video_id, true);
                    },
                    "Mark watched"
                }
            }
        }
    }
}
