use crate::{app::Route, models::Video, state::AppState};
use dioxus::prelude::*;
use dioxus_icons::lucide::{Check, EllipsisVertical, ListPlus, Trash2, Video as VideoIcon};
use g3_route_transitions::animated_navigate;
use g3_ui::{
    Avatar, AvatarSize, Badge, Button, ButtonFill, ButtonSize, Card, CardVariant, Color,
    EmptyState, Grid, GridColumns, Img, Progress, Space, SwipeAction, SwipeBehavior, SwipeItem,
    SwipeSide, SwipeState,
};

#[component]
pub fn VideoGrid(
    videos: Vec<Video>,
    empty_message: Option<String>,
    playlist_id: Option<String>,
    shorts_layout: Option<bool>,
) -> Element {
    let shorts = shorts_layout.unwrap_or(false);
    let empty_copy =
        empty_message.unwrap_or_else(|| "Your cached library will appear here.".to_string());
    rsx! {
        if videos.is_empty() {
            EmptyState {
                title: "Nothing here yet",
                icon: rsx! { VideoIcon { size: 40 } },
                "{empty_copy}"
            }
        } else {
            // Shorts are portrait, so more of them fit across.
            Grid {
                columns: GridColumns::Count(if shorts { 2 } else { 1 }),
                wide_columns: GridColumns::Count(if shorts { 6 } else { 4 }),
                gap: Space::Lg,
                for video in videos {
                    VideoCard { key: "{video.id}", video, playlist_id: playlist_id.clone(), short: shorts }
                }
            }
        }
    }
}

#[component]
pub fn VideoCard(
    video: Video,
    /// Set when this card is one entry of a playlist. Opening it then hands the
    /// playlist to the queue as a run, so autoplay and the player's arrows walk
    /// the playlist rather than stopping at this one video. The card also
    /// offers to remove the video from it.
    playlist_id: Option<String>,
    /// Draw the thumbnail portrait, for Shorts.
    short: Option<bool>,
) -> Element {
    let mut app_state = use_context::<AppState>();
    let start_playlist_name = app_state.swipe_action_label(true);
    let end_playlist_name = app_state.swipe_action_label(false);
    let channel_avatar_url = app_state.with_library(|library| {
        library
            .channels
            .iter()
            .find(|channel| channel.id == video.channel_id)
            .and_then(|channel| channel.avatar_url.clone())
    });
    let progress = video.progress_percent();
    let stats = video.stats_label();

    let open = {
        let video = video.clone();
        let playlist_id = playlist_id.clone();
        move |_| {
            // History is recorded by the watch page, not here. On the History
            // page, recording moves this card to the top, which rebuilds it and
            // cancels a navigation spawned from its scope.
            app_state.play(video.clone());
            if let Some(playlist_id) = &playlist_id {
                app_state.start_playlist_run(playlist_id);
            }
            spawn(animated_navigate(Route::VideoDetail {
                id: video.id.clone(),
            }));
        }
    };
    let open_channel = {
        let id = video.channel_id.clone();
        move |_| {
            spawn(animated_navigate(Route::ChannelDetail { id: id.clone() }));
        }
    };
    let open_menu = {
        let video = video.clone();
        move |_| {
            app_state.video_actions_video.set(Some(video.clone()));
            app_state.video_actions_open.set(true);
        }
    };
    let remove = playlist_id.clone().map(|playlist_id| {
        let video_id = video.id.clone();
        move |_| {
            if app_state.remove_from_playlist(&video_id, &playlist_id) {
                app_state.show_toast("Removed from playlist", Color::Neutral);
            }
        }
    });
    let start_id = video.id.clone();
    let end_id = video.id.clone();
    let desktop_start_id = video.id.clone();
    let desktop_end_id = video.id.clone();
    let swipe_id = video.id.clone();
    let short = short.unwrap_or(false);

    rsx! {
        // A swipe files the video into one of two playlists. On a mouse the
        // gesture gives way to full-height thumbnail edge actions. Those
        // desktop controls are separate from the swipe layer so they remain
        // real, clickable buttons while the touch actions stay hidden.
        div {
            class: if short {
                "video-card-frame video-card-short h-full rounded-[inherit]"
            } else {
                "video-card-frame h-full rounded-[inherit]"
            },
            SwipeItem {
                class: "video-card-swipe h-full rounded-[inherit]",
                start_behavior: SwipeBehavior::Activate,
                end_behavior: SwipeBehavior::Activate,
                mouse_swipe: false,
                start_actions: rsx! {
                    SwipeAction {
                        color: Color::Accent,
                        aria_label: "Add to {start_playlist_name}",
                        onclick: move |_| app_state.run_swipe_action(&start_id, true),
                        ListPlus { size: 22 }
                        span { "{start_playlist_name}" }
                    }
                },
                end_actions: rsx! {
                    SwipeAction {
                        aria_label: "Add to {end_playlist_name}",
                        onclick: move |_| app_state.run_swipe_action(&end_id, false),
                        ListPlus { size: 22 }
                        span { "{end_playlist_name}" }
                    }
                },
                on_activate: move |swipe: SwipeState| {
                    app_state.run_swipe_action(&swipe_id, swipe.side == SwipeSide::Start);
                },
                Card {
                    variant: CardVariant::Flat,
                    class: "h-full [&_.g3-card-title]:text-[0.95rem]",
                    title: video.title.clone(),
                    onclick: open,
                    media: rsx! {
                        div { class: "relative",
                            Img {
                                src: video.thumbnail_url.clone(),
                                alt: "",
                                aspect_ratio: if short { "9 / 16" } else { "16 / 9" },
                            }
                        // Listings that come from a flat playlist carry no
                        // runtime. No badge is honest; "0:00" is not.
                        div { class: "pointer-events-none absolute right-2 bottom-2 flex gap-1",
                            if video.watched {
                                Badge { Check { size: 12 } "Watched" }
                            }
                            if video.is_live {
                                Badge { color: Color::Danger, "LIVE" }
                            } else if video.duration_seconds > 0 {
                                Badge { "{video.duration_label()}" }
                            }
                        }
                        if progress > 0.0 {
                            Progress {
                                class: "absolute inset-x-0 bottom-0",
                                value: progress,
                                max: 100.0,
                                label: "Watched {progress:.0}%",
                            }
                        }
                            if let Some(remove) = remove {
                                div { class: "video-card-remove absolute top-2 right-2",
                                    Button {
                                        size: ButtonSize::Sm,
                                        color: Color::Neutral,
                                        aria_label: "Remove from playlist",
                                        onclick: remove,
                                        Trash2 { size: 15 }
                                    }
                                }
                            }
                        }
                    },
                    start: rsx! {
                        button {
                            r#type: "button",
                            class: "rounded-full",
                            aria_label: "Open {video.channel_name}",
                            onclick: open_channel,
                            Avatar {
                                name: video.channel_name.clone(),
                                src: channel_avatar_url,
                                size: AvatarSize::Sm,
                            }
                        }
                    },
                    end: rsx! {
                        Button {
                            fill: ButtonFill::Clear,
                            size: ButtonSize::Sm,
                            aria_label: "Video actions",
                            onclick: open_menu,
                            EllipsisVertical { size: 20 }
                        }
                    },
                    div { class: "video-card-metadata",
                        span { class: "video-card-metadata-line",
                            if video.channel_name.is_empty() { "\u{00a0}" } else { "{video.channel_name}" }
                        }
                        span { class: "video-card-metadata-line", "{stats}" }
                    }
                }
            }
            button {
                r#type: "button",
                class: "video-card-edge-action video-card-edge-action-start",
                aria_label: "Add to {start_playlist_name}",
                onclick: move |_| app_state.run_swipe_action(&desktop_start_id, true),
                ListPlus { size: 24 }
            }
            button {
                r#type: "button",
                class: "video-card-edge-action video-card-edge-action-end",
                aria_label: "Add to {end_playlist_name}",
                onclick: move |_| app_state.run_swipe_action(&desktop_end_id, false),
                ListPlus { size: 24 }
            }
        }
    }
}
