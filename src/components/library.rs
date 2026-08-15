use crate::{
    api::{get_channel_details, get_channel_media_page},
    app::Route,
    models::{ChannelMediaTab, Video},
    state::AppState,
};
use dioxus::prelude::*;
use dx_route_transitions::animated_navigate;
use dioxus_icons::lucide::{History, ListVideo, Play, Trash2};
use g3_ui::{Button, ButtonSize, ButtonStyle, Refresher, Sheet, StatusColor};

use super::VideoGrid;

#[component]
pub fn QueuePage() -> Element {
    let app_state = use_context::<AppState>();
    let library = app_state.library();
    let videos = library
        .queue
        .iter()
        .filter_map(|id| library.videos.iter().find(|video| &video.id == id).cloned())
        .collect::<Vec<_>>();
    let first_id = videos.first().map(|video| video.id.clone());

    rsx! {
        main { class: "page queue-page",
            div { class: "section-heading library-heading",
                div {
                    span { class: "section-kicker", "PLAYBACK QUEUE" }
                    h2 { "Ready when you are." }
                    p { "Play next puts a video at the front; add to queue keeps your current order." }
                }
                div { class: "heading-actions",
                    Button {
                        disabled: first_id.is_none(),
                        start: rsx! { Play { size: 17 } },
                        onclick: move |_| {
                            if let Some(id) = &first_id {
                                app_state.record_history(id);
                                { let v = id.clone(); spawn(async move { animated_navigate(Route::VideoDetail { id: v }).await; }); };
                            }
                        },
                        "Play all"
                    }
                    Button {
                        style: ButtonStyle::Clear,
                        disabled: videos.is_empty(),
                        start: rsx! { Trash2 { size: 17 } },
                        onclick: move |_| {
                            app_state.clear_queue();
                            app_state.show_toast("Queue cleared", StatusColor::Neutral);
                        },
                        "Clear"
                    }
                }
            }
            div { class: "library-summary",
                ListVideo { size: 18 }
                strong { "{videos.len()} queued" }
                span { "Synced with your Tawny library" }
            }
            VideoGrid { videos, empty_message: "Use a video menu to add something to the queue.".to_string() }
        }
    }
}

#[component]
pub fn HistoryPage() -> Element {
    let app_state = use_context::<AppState>();
    let library = app_state.library();
    let videos = library
        .history
        .iter()
        .filter_map(|entry| {
            library
                .videos
                .iter()
                .find(|video| video.id == entry.video_id)
                .cloned()
        })
        .collect::<Vec<_>>();

    rsx! {
        main { class: "page history-page",
            div { class: "section-heading library-heading",
                div {
                    span { class: "section-kicker", "WATCH HISTORY" }
                    h2 { "Pick up where you left off." }
                    p { "Recently opened videos stay available from the local cache and sync to your server." }
                }
                Button {
                    style: ButtonStyle::Clear,
                    disabled: videos.is_empty(),
                    start: rsx! { Trash2 { size: 17 } },
                    onclick: move |_| {
                        app_state.clear_history();
                        app_state.show_toast("History cleared", StatusColor::Neutral);
                    },
                    "Clear"
                }
            }
            div { class: "library-summary",
                History { size: 18 }
                strong { "{videos.len()} recent" }
                span { "Most recent first" }
            }
            VideoGrid { videos, empty_message: "Videos you open will appear here.".to_string() }
        }
    }
}

#[component]
pub fn ChannelDetail(id: String) -> Element {
    let app_state = use_context::<AppState>();

    // Owned by the header segmented control.
    let tab_index = app_state.channel_tab_index;
    let mut extra_videos = use_signal(Vec::<Video>::new);
    let mut extra_shorts = use_signal(Vec::<Video>::new);
    let mut extra_live = use_signal(Vec::<Video>::new);
    let mut videos_next = use_signal(|| None::<String>);
    let mut shorts_next = use_signal(|| None::<String>);
    let mut live_next = use_signal(|| None::<String>);
    let mut initialized = use_signal(|| false);
    let mut page_loading = use_signal(|| false);

    let mut details_resource = {
        let channel_id = id.clone();
        use_resource(move || {
            let channel_id = channel_id.clone();
            async move { get_channel_details(channel_id).await }
        })
    };
    let remote_details = details_resource
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .cloned();
    let details_to_cache = remote_details.clone();
    use_effect(move || {
        if initialized() {
            return;
        }
        if let Some(details) = details_to_cache.as_ref() {
            app_state.cache_channel_details(details);
            videos_next.set(details.videos.next_page.clone());
            shorts_next.set(details.shorts.next_page.clone());
            live_next.set(details.live.next_page.clone());
            initialized.set(true);
        }
    });

    let library = app_state.library();
    let cached_channel = library
        .channels
        .iter()
        .find(|channel| channel.id == id)
        .cloned();
    let channel = remote_details
        .as_ref()
        .map(|details| details.channel.clone())
        .or(cached_channel);

    let Some(channel) = channel else {
        let failed = details_resource.read().as_ref().is_some();
        return rsx! {
            main { class: "page",
                div { class: "empty-state",
                    if failed {
                        p { "This channel could not be loaded from YouTube or the local cache." }
                    } else {
                        div { class: "loading-orbit" }
                        p { "Loading channel videos and Shorts…" }
                    }
                }
            }
        };
    };

    let channel_id = channel.id.clone();
    let local_channel_videos = library
        .videos
        .iter()
        .filter(|video| video.channel_id == channel.id)
        .cloned()
        .collect::<Vec<_>>();
    let mut description_open = use_signal(|| false);
    let selected_tab = match tab_index() {
        1 => ChannelMediaTab::Shorts,
        2 => ChannelMediaTab::Live,
        _ => ChannelMediaTab::Videos,
    };
    let mut videos = if let Some(details) = remote_details.as_ref() {
        match selected_tab {
            ChannelMediaTab::Videos => details.videos.videos.clone(),
            ChannelMediaTab::Shorts => details.shorts.videos.clone(),
            ChannelMediaTab::Live => details.live.videos.clone(),
        }
    } else {
        local_channel_videos
            .iter()
            .filter(|video| match selected_tab {
                ChannelMediaTab::Videos => !video.is_short && !video.is_live,
                ChannelMediaTab::Shorts => video.is_short,
                ChannelMediaTab::Live => video.is_live,
            })
            .cloned()
            .collect::<Vec<_>>()
    };
    match selected_tab {
        ChannelMediaTab::Videos => videos.extend(extra_videos()),
        ChannelMediaTab::Shorts => videos.extend(extra_shorts()),
        ChannelMediaTab::Live => videos.extend(extra_live()),
    }
    videos.dedup_by(|left, right| left.id == right.id);
    let next_page = match selected_tab {
        ChannelMediaTab::Videos => videos_next(),
        ChannelMediaTab::Shorts => shorts_next(),
        ChannelMediaTab::Live => live_next(),
    };
    let is_subscribed = channel.subscribed;
    let initial = channel.name.chars().next().unwrap_or('T');
    let avatar_url = channel.avatar_url.clone();
    let banner_url = channel.banner_url.clone();
    let description_text = channel.description.clone();
    let load_channel_id = channel.id.clone();
    let load_token = next_page.clone();

    rsx! {
        div { class: "channel-detail-page",
                // Pull down to refresh replaces the button that used to sit in
                // the toolbar next to the segments.
                Refresher {
                    refreshing: page_loading(),
                    on_refresh: move |_| {
                        initialized.set(false);
                        extra_videos.set(Vec::new());
                        extra_shorts.set(Vec::new());
                        extra_live.set(Vec::new());
                        details_resource.restart();
                    },
                }
                main { class: "page",
                    if let Some(banner_url) = banner_url {
                        div {
                            class: "channel-banner",
                            style: "background-image: linear-gradient(180deg, transparent, var(--color-bg)), url('{banner_url}')",
                        }
                    }
                    section { class: "channel-hero",
                        if let Some(avatar_url) = avatar_url {
                            img { class: "channel-avatar channel-avatar-hero", src: "{avatar_url}", alt: "{channel.name}" }
                        } else {
                            div { class: "channel-avatar channel-avatar-hero", "{initial}" }
                        }
                        div { class: "channel-hero-copy",
                            span { class: "section-kicker", "{channel.handle}" }
                            h1 { "{channel.name}" }
                            // The local cache count is an implementation
                            // detail; it told the reader nothing about the
                            // channel.
                            p { "{channel.subscriber_count} subscribers" }
                            if !channel.description.is_empty() {
                                // Behind a button: a long channel description
                                // pushed the videos off the first screen.
                                Button {
                                    style: ButtonStyle::Clear,
                                    size: ButtonSize::Sm,
                                    class: "channel-description-button",
                                    onclick: move |_| description_open.set(true),
                                    "Description"
                                }
                            }
                        }
                        Button {
                            style: if is_subscribed { ButtonStyle::Neutral } else { ButtonStyle::Solid },
                            onclick: move |_| {
                                if let Some(now_subscribed) = app_state.toggle_subscription(&channel_id) {
                                    app_state.show_toast(
                                        if now_subscribed { "Subscribed" } else { "Unsubscribed" },
                                        StatusColor::Neutral,
                                    );
                                }
                            },
                            if is_subscribed { "Subscribed" } else { "Subscribe" }
                        }
                    }
                    Sheet { is_open: description_open, class: "channel-description-sheet",
                        p { class: "sheet-label", "About" }
                        p { class: "channel-description", "{description_text}" }
                    }
                    VideoGrid {
                        videos,
                        empty_message: match selected_tab {
                            ChannelMediaTab::Shorts => "No Shorts were returned for this channel.".to_string(),
                            ChannelMediaTab::Live => "No livestreams were returned for this channel.".to_string(),
                            ChannelMediaTab::Videos => "No videos were returned for this channel.".to_string(),
                        }
                    }
                    if let Some(load_token) = load_token {
                        div { class: "load-more-row",
                            Button {
                                style: ButtonStyle::Neutral,
                                disabled: page_loading(),
                                onclick: move |_| {
                                    let channel_id = load_channel_id.clone();
                                    let token = load_token.clone();
                                    page_loading.set(true);
                                    spawn(async move {
                                        match get_channel_media_page(
                                            channel_id,
                                            match selected_tab {
                                                ChannelMediaTab::Shorts => "shorts".into(),
                                                ChannelMediaTab::Live => "live".into(),
                                                ChannelMediaTab::Videos => "videos".into(),
                                            },
                                            token,
                                        )
                                        .await
                                        {
                                            Ok(page) => match selected_tab {
                                                ChannelMediaTab::Videos => {
                                                    extra_videos.write().extend(page.videos);
                                                    videos_next.set(page.next_page);
                                                }
                                                ChannelMediaTab::Shorts => {
                                                    extra_shorts.write().extend(page.videos);
                                                    shorts_next.set(page.next_page);
                                                }
                                                ChannelMediaTab::Live => {
                                                    extra_live.write().extend(page.videos);
                                                    live_next.set(page.next_page);
                                                }
                                            },
                                            Err(_) => app_state.show_toast(
                                                "Could not load the next page",
                                                StatusColor::Warning,
                                            ),
                                        }
                                        page_loading.set(false);
                                    });
                                },
                                if page_loading() { "Loading…" } else { "Load more" }
                            }
                        }
                    }
                }
        }
    }
}
