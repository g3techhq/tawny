use crate::{
    api::{get_channel_details, get_channel_media_page},
    app::Route,
    models::{ChannelMediaTab, SubscriptionContent, Video},
    state::AppState,
};
use dioxus::prelude::*;
use dioxus_icons::lucide::{Check, History, ListVideo, Play, Trash2, User};
use g3_route_transitions::{ROUTE_TRANSITION_COVER_CLASS, animated_navigate};
use g3_ui::{
    Body, Button, ButtonSize, ButtonStyle, Refresher, SegmentButton, SegmentGroup, Sheet,
    StatusColor,
};

use super::{PageHeader, VideoGrid};

#[component]
pub fn QueuePage() -> Element {
    let app_state = use_context::<AppState>();
    let library = app_state.library();
    let videos = library
        .queue
        .iter()
        .filter_map(|entry| {
            library
                .videos
                .iter()
                .find(|video| &video.id == entry)
                .cloned()
        })
        .collect::<Vec<_>>();
    // The playlist being run is one entry in the queue, not a copy of its
    // contents, so it gets a row of its own rather than fifty cards. Shown above
    // the loose videos because that is where `start_playlist_run` puts it: pressing
    // play on a playlist means now, not after last week's leftovers.
    let running = app_state.playlist_run_status();
    // What pressing play actually starts, resolved exactly the way autoplay
    // resolves it: a playlist at the head of the queue plays its first shown
    // entry, not the first loose video behind it.
    let first_id = app_state.next_in_run("");
    let is_empty = videos.is_empty() && running.is_none();

    rsx! {
        // One surface, one snapshot: the header rides up with the body it
        // belongs to instead of morphing in place while the page slides
        // underneath it.
        div { class: "auxiliary-cover {ROUTE_TRANSITION_COVER_CLASS}",
            PageHeader { title: "Queue".to_string(), back_to: Route::Feed {} }
            Body { padding: false,
                main { class: "page queue-page",
                    if is_empty {
                        div { class: "empty-state",
                            div { class: "empty-icon", ListVideo { size: 25 } }
                            h3 { "Your queue is empty" }
                            p { "Use a video menu to play next or add something to the queue." }
                        }
                    } else {
                        div { class: "queue-toolbar",
                            div { class: "queue-count-label",
                                ListVideo { size: 18 }
                                strong { "{videos.len()} queued" }
                            }
                            div { class: "heading-actions",
                                Button {
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
                                    start: rsx! { Trash2 { size: 17 } },
                                    onclick: move |_| {
                                        app_state.clear_queue();
                                        app_state.show_toast("Queue cleared", StatusColor::Neutral);
                                    },
                                    "Clear"
                                }
                            }
                        }
                        if let Some(run) = running {
                            {
                                let playlist_id = run.playlist_id.clone();
                                let playlist_name = run.name.clone();
                                let total = run.total;
                                let remaining = run.remaining;
                                let order_label = run.view.sort.label(run.view.descending);
                                let filters = run.view.filter_labels();
                                let filter_text = if filters.is_empty() {
                                    "no filters".to_string()
                                } else {
                                    filters.join(", ")
                                };
                                let left_text = if remaining == 1 { "1 left".to_string() } else { format!("{remaining} left") };
                                rsx! {
                            div { class: "queue-playlist-row",
                                button {
                                    class: "queue-playlist-open",
                                    onclick: {
                                        let playlist_id = playlist_id.clone();
                                        move |_| {
                                            let id = playlist_id.clone();
                                            spawn(async move { animated_navigate(Route::PlaylistDetail { id }).await; });
                                        }
                                    },
                                    span { class: "queue-playlist-icon", ListVideo { size: 20 } }
                                    // A playlist in the queue is a rule rather
                                    // than a list: each time a video ends it
                                    // looks at the playlist as its page is
                                    // arranged *then*. So this says where the run
                                    // is and which arrangement it is following.
                                    span { class: "queue-playlist-copy",
                                        strong { "{playlist_name}" }
                                        if let (Some(position), Some(current)) = (run.position, run.current.as_ref()) {
                                            span { class: "queue-playlist-now",
                                                "Now playing {position} of {total}: "
                                                em { "{current.title}" }
                                            }
                                        } else {
                                            span { class: "queue-playlist-now",
                                                "Not started · {remaining} of {total} to play"
                                            }
                                        }
                                        // The count leads so an ellipsis on a
                                        // long title never swallows it.
                                        if let Some(next) = run.up_next.as_ref() {
                                            span { class: "queue-playlist-next",
                                                if run.position.is_some() { "{left_text} · Up next: " } else { "Starts with: " }
                                                em { "{next.title}" }
                                            }
                                        } else if run.position.is_some() {
                                            span { class: "queue-playlist-next", "Last video in this order" }
                                        }
                                        span { class: "queue-playlist-order", "Order: {order_label} · {filter_text}" }
                                    }
                                }
                                Button {
                                    style: ButtonStyle::Clear,
                                    size: ButtonSize::Sm,
                                    aria_label: "Stop playing this playlist".to_string(),
                                    onclick: move |_| {
                                        if app_state.clear_playlist_run() {
                                            app_state.show_toast(
                                                format!("Stopped playing {playlist_name}"),
                                                StatusColor::Neutral,
                                            );
                                        }
                                    },
                                    Trash2 { size: 16 }
                                }
                            }
                            p { class: "queue-playlist-note",
                                "Follows the playlist page as it is now. Change the order or filters there and the next video is picked from the new arrangement, continuing after the one playing. If the playing video no longer matches the filters, the run starts again from the top."
                            }
                                }
                            }
                        }
                        VideoGrid { videos }
                    }
                }
            }
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
        // One surface, one snapshot: the header rides up with the body it
        // belongs to instead of morphing in place while the page slides
        // underneath it.
        div { class: "auxiliary-cover {ROUTE_TRANSITION_COVER_CLASS}",
            PageHeader { title: "History".to_string(), back_to: Route::Feed {} }
            Body { padding: false,
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
    let mut channel_refreshing = use_signal(|| false);
    let mut refreshed_details = use_signal(|| None::<crate::models::ChannelDetails>);
    let mut previous_tab = use_signal(|| tab_index());
    use_effect(move || {
        let current = tab_index();
        if previous_tab() != current {
            previous_tab.set(current);
            spawn(async move {
                let mut eval = document::eval(
                    "document.querySelector('.g3-body-content')?.scrollTo({ top: 0, behavior: 'instant' }); dioxus.send(true);",
                );
                let _ = eval.recv::<bool>().await;
            });
        }
    });

    let details_resource = {
        let channel_id = id.clone();
        use_resource(move || {
            let channel_id = channel_id.clone();
            async move { get_channel_details(channel_id).await }
        })
    };
    let remote_details = refreshed_details().or_else(|| {
        details_resource
            .read()
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .cloned()
    });
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
    // `cache_channel_details` has already merged this response with richer
    // metadata discovered by the player/search surfaces. Prefer that merged
    // record so a sparse channel tab response (for example `528` instead of
    // `21.1M`) cannot flash the wrong count in the hero.
    let channel = cached_channel.or_else(|| {
        remote_details
            .as_ref()
            .map(|details| details.channel.clone())
    });

    let Some(channel) = channel else {
        let failed = details_resource.read().as_ref().is_some();
        // Still renders the bar, so a channel that fails to load keeps its back
        // affordance instead of stranding the viewer.
        return rsx! {
            PageHeader { title: String::new(), back_to: Route::Subscriptions {} }
            Body { padding: false,
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
        1 => Some(ChannelMediaTab::Videos),
        2 => Some(ChannelMediaTab::Shorts),
        3 => Some(ChannelMediaTab::Live),
        _ => None,
    };
    let mut videos = if let Some(details) = remote_details.as_ref() {
        match selected_tab {
            Some(ChannelMediaTab::Videos) => details.videos.videos.clone(),
            Some(ChannelMediaTab::Shorts) => details.shorts.videos.clone(),
            Some(ChannelMediaTab::Live) => details.live.videos.clone(),
            None => details
                .videos
                .videos
                .iter()
                .chain(&details.shorts.videos)
                .chain(&details.live.videos)
                .cloned()
                .collect(),
        }
    } else {
        local_channel_videos
            .iter()
            .filter(|video| match selected_tab {
                Some(ChannelMediaTab::Videos) => !video.is_short && !video.is_live,
                Some(ChannelMediaTab::Shorts) => video.is_short,
                Some(ChannelMediaTab::Live) => video.is_live,
                None => true,
            })
            .cloned()
            .collect::<Vec<_>>()
    };
    match selected_tab {
        Some(ChannelMediaTab::Videos) => videos.extend(extra_videos()),
        Some(ChannelMediaTab::Shorts) => videos.extend(extra_shorts()),
        Some(ChannelMediaTab::Live) => videos.extend(extra_live()),
        None => {
            videos.extend(extra_videos());
            videos.extend(extra_shorts());
            videos.extend(extra_live());
        }
    }
    videos.sort_by_cached_key(|video| std::cmp::Reverse(video.published_epoch()));
    let mut seen_ids = std::collections::HashSet::new();
    videos.retain(|video| seen_ids.insert(video.id.clone()));
    let next_page = match selected_tab {
        Some(ChannelMediaTab::Videos) => videos_next(),
        Some(ChannelMediaTab::Shorts) => shorts_next(),
        Some(ChannelMediaTab::Live) => live_next(),
        None => None,
    };
    let is_subscribed = channel.subscribed;
    // The segmented control is index-driven, so the stored preference is
    // mirrored into a signal and written back whenever the index moves.
    let content_index = use_signal(|| match channel.subscription_content {
        SubscriptionContent::All => 0usize,
        SubscriptionContent::Videos => 1,
        SubscriptionContent::Shorts => 2,
    });
    let content_channel_id = channel.id.clone();
    use_effect(move || {
        let content = match content_index() {
            1 => SubscriptionContent::Videos,
            2 => SubscriptionContent::Shorts,
            _ => SubscriptionContent::All,
        };
        if app_state.set_subscription_content(&content_channel_id, content) {
            app_state.show_toast(
                format!("Feed shows {}", content.label().to_lowercase()),
                StatusColor::Neutral,
            );
        }
    });
    let avatar_url = channel.avatar_url.clone();
    let banner_url = channel.banner_url.clone();
    let description_text = channel.description.clone();
    let load_channel_id = channel.id.clone();
    let load_token = next_page.clone();
    let refresh_channel_id = channel.id.clone();

    rsx! {
        PageHeader {
            // Deliberately untitled: the page leads with the channel name and
            // avatar, and repeating it in the bar pushed the actions off the
            // edge on a long name.
            title: String::new(),
            back_to: Route::Subscriptions {},
            toolbar: rsx! {
                SegmentGroup { active: app_state.channel_tab_index,
                    SegmentButton { index: 0, "All" }
                    SegmentButton { index: 1, "Videos" }
                    SegmentButton { index: 2, "Shorts" }
                    SegmentButton { index: 3, "Live" }
                }
            },
        }
        Body { padding: false,
            div { class: "channel-detail-page",
                    // Pull down to refresh replaces the button that used to sit in
                    // the toolbar next to the segments.
                    Refresher {
                        refreshing: channel_refreshing(),
                        can_refresh: true,
                        on_refresh: move |_| {
                            let channel_id = refresh_channel_id.clone();
                            channel_refreshing.set(true);
                            extra_videos.set(Vec::new());
                            extra_shorts.set(Vec::new());
                            extra_live.set(Vec::new());
                            spawn(async move {
                                match get_channel_details(channel_id).await {
                                    Ok(details) => {
                                        app_state.cache_channel_details(&details);
                                        videos_next.set(details.videos.next_page.clone());
                                        shorts_next.set(details.shorts.next_page.clone());
                                        live_next.set(details.live.next_page.clone());
                                        refreshed_details.set(Some(details));
                                        app_state.show_toast("Channel refreshed", StatusColor::Success);
                                    }
                                    Err(_) => app_state.show_toast(
                                        "Could not refresh this channel — showing cached videos",
                                        StatusColor::Warning,
                                    ),
                                }
                                channel_refreshing.set(false);
                            });
                        },
                        main { class: "page",
                        // Rows, not columns: see `.channel-hero` in the
                        // stylesheet for what the old grid did to a long name.
                        section { class: "channel-hero",
                            if let Some(banner_url) = banner_url {
                                div {
                                    class: "channel-hero-banner",
                                    style: "background-image: url('{banner_url}')",
                                }
                            }
                            div { class: "channel-hero-body",
                                div { class: "channel-hero-identity",
                                    if let Some(avatar_url) = avatar_url {
                                        img { class: "channel-avatar channel-avatar-hero", src: "{avatar_url}", alt: "{channel.name}" }
                                    } else {
                                        div { class: "channel-avatar channel-avatar-hero channel-avatar-fallback", User { size: 30 } }
                                    }
                                    div { class: "channel-hero-copy",
                                        h1 { "{channel.name}" }
                                        // The local cache count is an implementation
                                        // detail; it told the reader nothing about
                                        // the channel.
                                        p { class: "channel-hero-meta",
                                            if !channel.handle.trim().is_empty() {
                                                span { "{channel.handle}" }
                                            }
                                            if !channel.subscriber_count.trim().is_empty() {
                                                span { "{channel.subscriber_count} subscribers" }
                                            }
                                        }
                                    }
                                }
                                div { class: "channel-hero-actions",
                                    Button {
                                        style: if is_subscribed { ButtonStyle::Neutral } else { ButtonStyle::Solid },
                                        start: if is_subscribed { Some(rsx! { Check { size: 16 } }) } else { None },
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
                                    // Behind a button: a long channel description
                                    // pushed the videos off the first screen.
                                    if !channel.description.is_empty() {
                                        Button {
                                            style: ButtonStyle::Neutral,
                                            onclick: move |_| description_open.set(true),
                                            "About"
                                        }
                                    }
                                }
                                // Only meaningful once subscribed: this narrows what
                                // reaches the feed, it does not change the channel page.
                                if is_subscribed {
                                    div { class: "subscription-content-row",
                                        span { class: "subscription-content-label", "Feed shows" }
                                        SegmentGroup { active: content_index, class: "subscription-content-segments".to_string(),
                                            SegmentButton { index: 0, "Both" }
                                            SegmentButton { index: 1, "Videos" }
                                            SegmentButton { index: 2, "Shorts" }
                                        }
                                    }
                                }
                            }
                        }
                        Sheet { is_open: description_open, class: "channel-description-sheet",
                            p { class: "sheet-label", "About" }
                            p { class: "channel-description", "{description_text}" }
                        }
                        VideoGrid {
                            videos,
                            shorts_layout: selected_tab == Some(ChannelMediaTab::Shorts),
                            empty_message: match selected_tab {
                                Some(ChannelMediaTab::Shorts) => "No Shorts were returned for this channel.".to_string(),
                                Some(ChannelMediaTab::Live) => "No livestreams were returned for this channel.".to_string(),
                                Some(ChannelMediaTab::Videos) => "No videos were returned for this channel.".to_string(),
                                None => "No uploads were returned for this channel.".to_string(),
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
                                                    Some(ChannelMediaTab::Shorts) => "shorts".into(),
                                                    Some(ChannelMediaTab::Live) => "live".into(),
                                                    Some(ChannelMediaTab::Videos) => "videos".into(),
                                                    None => "videos".into(),
                                                },
                                                token,
                                            )
                                            .await
                                            {
                                                Ok(page) => match selected_tab {
                                                    Some(ChannelMediaTab::Videos) => {
                                                        extra_videos.write().extend(page.videos);
                                                        videos_next.set(page.next_page);
                                                    }
                                                    Some(ChannelMediaTab::Shorts) => {
                                                        extra_shorts.write().extend(page.videos);
                                                        shorts_next.set(page.next_page);
                                                    }
                                                    Some(ChannelMediaTab::Live) => {
                                                        extra_live.write().extend(page.videos);
                                                        live_next.set(page.next_page);
                                                    }
                                                    None => {}
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
    }
}
