use crate::{
    api::{get_channel_details, get_channel_media_page},
    app::Route,
    models::{Channel, ChannelMediaTab, FeedFilter, SubscriptionContent, Video},
    state::AppState,
};
use dioxus::prelude::*;
use dioxus_icons::lucide::{Check, History, ListVideo, Play, Trash2, Tv};
use g3_route_transitions::{ROUTE_TRANSITION_OVERLAY_REGION_CLASS, animated_navigate};
use g3_ui::{
    Avatar, AvatarSize, BottomSheet, Button, ButtonFill, ButtonSize, Card, Color, Content,
    EmptyState, InfiniteScroll, SegmentButton, SegmentGroup, Space, Spinner, Stack, StackAlign,
    Text, TextTone, TextVariant,
};

use super::{FeedFilterSegments, PageHeader, VideoGrid};
use std::collections::HashMap;

/// How many history entries the page lays out before asking for more.
const HISTORY_PAGE_SIZE: usize = 24;

/// The library's videos by id. Queue and History each resolve a list of ids,
/// and searching the whole library once per id grew with both.
fn videos_by_id(videos: &[Video]) -> HashMap<&str, &Video> {
    videos
        .iter()
        .map(|video| (video.id.as_str(), video))
        .collect()
}

/// Queue and History are sheets, and the sheet is the whole page - its own
/// header included - so the header rides up with the body it belongs to.
#[component]
fn AuxiliarySheet(title: String, children: Element) -> Element {
    rsx! {
        div { class: "auxiliary-cover {ROUTE_TRANSITION_OVERLAY_REGION_CLASS}",
            PageHeader { title, back_to: Route::Feed {} }
            Content { {children} }
        }
    }
}

#[component]
pub fn QueuePage() -> Element {
    let app_state = use_context::<AppState>();
    let videos = app_state.with_library(|library| {
        let by_id = videos_by_id(&library.videos);
        library
            .queue
            .iter()
            .filter_map(|entry| by_id.get(entry.as_str()).map(|video| (*video).clone()))
            .collect::<Vec<_>>()
    });
    // The playlist being run is one entry in the queue, not a copy of its
    // contents, so it gets a card of its own rather than fifty. Shown above
    // the loose videos because that is where `start_playlist_run` puts it:
    // pressing play on a playlist means now, not after last week's leftovers.
    let running = app_state.playlist_run_status();
    // What pressing play actually starts, resolved exactly the way autoplay
    // resolves it: a playlist at the head of the queue plays its first shown
    // entry, not the first loose video behind it.
    let first_id = app_state.next_in_run("");
    let is_empty = videos.is_empty() && running.is_none();

    rsx! {
        AuxiliarySheet { title: "Queue",
            if is_empty {
                EmptyState {
                    title: "Your queue is empty",
                    icon: rsx! { ListVideo { size: 40 } },
                    "Use a video menu to play next or add something to the queue."
                }
            } else {
                Stack { gap: Space::Md,
                    Stack { horizontal: true, gap: Space::Sm, align: StackAlign::Center,
                        Text { variant: TextVariant::Label, class: "mr-auto", "{videos.len()} queued" }
                        Button {
                            start: rsx! { Play { size: 17 } },
                            onclick: move |_| {
                                if let Some(id) = &first_id {
                                    app_state.record_history(id);
                                    spawn(animated_navigate(Route::VideoDetail { id: id.clone() }));
                                }
                            },
                            "Play all"
                        }
                        Button {
                            fill: ButtonFill::Clear,
                            color: Color::Neutral,
                            start: rsx! { Trash2 { size: 17 } },
                            onclick: move |_| {
                                app_state.clear_queue();
                                app_state.show_toast("Queue cleared", Color::Neutral);
                            },
                            "Clear"
                        }
                    }
                    if let Some(run) = running {
                        {
                            let playlist_id = run.playlist_id.clone();
                            let playlist_name = run.name.clone();
                            let stop_name = run.name.clone();
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
                            // A playlist in the queue is a rule rather than a
                            // list: each time a video ends it looks at the
                            // playlist as its page is arranged *then*. So this
                            // says where the run is and which arrangement it
                            // is following.
                            let now = match (run.position, run.current.as_ref()) {
                                (Some(position), Some(current)) => {
                                    format!("Now playing {position} of {total}: {}", current.title)
                                }
                                _ => format!("Not started · {remaining} of {total} to play"),
                            };
                            // The count leads so an ellipsis on a long title
                            // never swallows it.
                            let next = match (run.up_next.as_ref(), run.position.is_some()) {
                                (Some(next), true) => Some(format!("{left_text} · Up next: {}", next.title)),
                                (Some(next), false) => Some(format!("Starts with: {}", next.title)),
                                (None, true) => Some("Last video in this order".to_string()),
                                (None, false) => None,
                            };
                            rsx! {
                                Card {
                                    title: playlist_name,
                                    subtitle: "Order: {order_label} · {filter_text}",
                                    start: rsx! { ListVideo { size: 22 } },
                                    onclick: move |_| {
                                        spawn(animated_navigate(Route::PlaylistDetail { id: playlist_id.clone() }));
                                    },
                                    end: rsx! {
                                        Button {
                                            fill: ButtonFill::Clear,
                                            size: ButtonSize::Sm,
                                            aria_label: "Stop playing this playlist",
                                            onclick: move |_| {
                                                if app_state.clear_playlist_run() {
                                                    app_state.show_toast(format!("Stopped playing {stop_name}"), Color::Neutral);
                                                }
                                            },
                                            Trash2 { size: 16 }
                                        }
                                    },
                                    Stack { gap: Space::Xs,
                                        Text { class: "line-clamp-1", "{now}" }
                                        if let Some(next) = next {
                                            Text { tone: TextTone::Secondary, class: "line-clamp-1", "{next}" }
                                        }
                                        Text { variant: TextVariant::Caption,
                                            "Follows the playlist page as it is now. Change the order or filters there and the next video is picked from the new arrangement, continuing after the one playing. If the playing video no longer matches the filters, the run starts again from the top."
                                        }
                                    }
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

#[component]
pub fn HistoryPage() -> Element {
    let app_state = use_context::<AppState>();
    let mut visible_count = use_signal(|| HISTORY_PAGE_SIZE);
    // Every video ever opened, so it is laid out a page at a time, and only
    // that page is copied out of the library.
    let (videos, total) = app_state.with_library(|library| {
        let by_id = videos_by_id(&library.videos);
        let found = library
            .history
            .iter()
            .filter_map(|entry| by_id.get(entry.video_id.as_str()).copied())
            .collect::<Vec<_>>();
        let total = found.len();
        let page = found
            .into_iter()
            .take(visible_count())
            .cloned()
            .collect::<Vec<_>>();
        (page, total)
    });
    let remaining = total.saturating_sub(videos.len());

    rsx! {
        AuxiliarySheet { title: "History",
            Stack { gap: Space::Md,
                Stack { horizontal: true, gap: Space::Sm, align: StackAlign::Center,
                    History { size: 18 }
                    Text { variant: TextVariant::Label, "{total} recent" }
                    Text { tone: TextTone::Secondary, class: "mr-auto", "Most recent first" }
                    Button {
                        fill: ButtonFill::Clear,
                        color: Color::Neutral,
                        disabled: videos.is_empty(),
                        start: rsx! { Trash2 { size: 17 } },
                        onclick: move |_| {
                            app_state.clear_history();
                            app_state.show_toast("History cleared", Color::Neutral);
                        },
                        "Clear"
                    }
                }
                VideoGrid { videos, empty_message: "Videos you open will appear here.".to_string() }
                InfiniteScroll {
                    loading: false,
                    complete: remaining == 0,
                    on_load: move |_| {
                        if remaining > 0 {
                            visible_count += HISTORY_PAGE_SIZE;
                        }
                    },
                }
            }
        }
    }
}

/// Who the channel is, and the viewer's relationship to it. Its own component
/// so its hooks run on every render, not only once the channel has loaded.
#[component]
fn ChannelHero(channel: Channel) -> Element {
    let app_state = use_context::<AppState>();
    let mut description_open = use_signal(|| false);
    // Mirrors the stored preference, written back when the viewer picks.
    let mut content = use_signal(|| channel.subscription_content);
    let stored = channel.subscription_content;
    use_effect(use_reactive!(|stored| content.set(stored)));
    let channel_id = channel.id.clone();
    let content_channel_id = channel.id.clone();
    let is_subscribed = channel.subscribed;
    let handle = channel.handle.trim();
    let subscribers = channel.subscriber_count.trim();
    let meta = match (handle.is_empty(), subscribers.is_empty()) {
        (false, false) => format!("{handle} · {subscribers} subscribers"),
        (false, true) => handle.to_string(),
        (true, false) => format!("{subscribers} subscribers"),
        (true, true) => String::new(),
    };

    rsx! {
        Card {
            media: channel.banner_url.clone().map(|banner_url| rsx! {
                div {
                    class: "aspect-[6/1] bg-cover bg-center",
                    style: "background-image: url('{banner_url}')",
                }
            }),
            Stack { gap: Space::Md,
                Stack { horizontal: true, gap: Space::Md, align: StackAlign::Center,
                    Avatar { name: channel.name.clone(), src: channel.avatar_url.clone(), size: AvatarSize::Lg }
                    Stack { gap: Space::Xs, class: "min-w-0",
                        Text { variant: TextVariant::Title, class: "line-clamp-2", "{channel.name}" }
                        if !meta.is_empty() {
                            Text { tone: TextTone::Secondary, "{meta}" }
                        }
                    }
                }
                Stack { horizontal: true, gap: Space::Sm, wrap: true,
                    Button {
                        fill: if is_subscribed { ButtonFill::Outline } else { ButtonFill::Solid },
                        color: if is_subscribed { Color::Neutral } else { Color::Accent },
                        "aria-pressed": if is_subscribed { "true" } else { "false" },
                        start: is_subscribed.then(|| rsx! { Check { size: 16 } }),
                        onclick: move |_| {
                            if let Some(now_subscribed) = app_state.toggle_subscription(&channel_id) {
                                app_state.show_toast(
                                    if now_subscribed { "Subscribed" } else { "Unsubscribed" },
                                    Color::Neutral,
                                );
                            }
                        },
                        if is_subscribed { "Subscribed" } else { "Subscribe" }
                    }
                    // Behind a button: a long channel description pushed the
                    // videos off the first screen.
                    if !channel.description.is_empty() {
                        Button {
                            fill: ButtonFill::Outline,
                            color: Color::Neutral,
                            onclick: move |_| description_open.set(true),
                            "About"
                        }
                    }
                }
                // Only meaningful once subscribed: this narrows what reaches
                // the feed, it does not change the channel page.
                if is_subscribed {
                    SegmentGroup {
                        value: content,
                        label: "Feed shows",
                        onchange: move |picked: SubscriptionContent| {
                            if app_state.set_subscription_content(&content_channel_id, picked) {
                                app_state.show_toast(
                                    format!("Feed shows {}", picked.label().to_lowercase()),
                                    Color::Neutral,
                                );
                            }
                        },
                        SegmentButton { value: SubscriptionContent::All, "Both" }
                        SegmentButton { value: SubscriptionContent::Videos, "Videos" }
                        SegmentButton { value: SubscriptionContent::Shorts, "Shorts" }
                    }
                }
            }
        }
        BottomSheet { open: description_open, title: "About",
            Text { class: "whitespace-pre-line", "{channel.description}" }
        }
    }
}

/// The channel tab a feed filter picks; `None` is every tab together.
fn media_tab(filter: FeedFilter) -> Option<ChannelMediaTab> {
    match filter {
        FeedFilter::All => None,
        FeedFilter::Videos => Some(ChannelMediaTab::Videos),
        FeedFilter::Shorts => Some(ChannelMediaTab::Shorts),
        FeedFilter::Live => Some(ChannelMediaTab::Live),
    }
}

#[component]
pub fn ChannelDetail(id: String) -> Element {
    let app_state = use_context::<AppState>();

    // Owned by the header segmented control.
    let tab = app_state.channel_tab;
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
    let mut previous_tab = use_signal(|| *tab.peek());
    use_effect(move || {
        let current = tab();
        if previous_tab() != current {
            previous_tab.set(current);
            spawn(async move {
                let mut eval = document::eval(
                    "document.querySelector('.g3-content-scroll')?.scrollTo({ top: 0, behavior: 'instant' }); dioxus.send(true);",
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

    // Read through a borrow: `AppState::library` clones the whole snapshot,
    // every cached video included, to find one channel.
    let cached_channel = app_state.with_library(|library| {
        library
            .channels
            .iter()
            .find(|channel| channel.id == id)
            .cloned()
    });
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
            Content {
                if failed {
                    EmptyState {
                        title: "Channel unavailable",
                        color: Color::Danger,
                        icon: rsx! { Tv { size: 40 } },
                        "This channel could not be loaded from YouTube or the local cache."
                    }
                } else {
                    Spinner { center: true }
                }
            }
        };
    };

    let selected_tab = media_tab(tab());
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
        let filter = tab();
        app_state.with_library(|library| {
            library
                .videos
                .iter()
                .filter(|video| video.channel_id == channel.id && filter.accepts(video))
                .cloned()
                .collect::<Vec<_>>()
        })
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
    let load_channel_id = channel.id.clone();
    let refresh_channel_id = channel.id.clone();

    let refresh = move |_| {
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
                    app_state.show_toast("Channel refreshed", Color::Success);
                }
                Err(_) => app_state.show_toast(
                    "Could not refresh this channel — showing cached videos",
                    Color::Warning,
                ),
            }
            channel_refreshing.set(false);
        });
    };
    let has_next_page = next_page.is_some();
    let load_more = move |_| {
        let Some(token) = next_page.clone() else {
            return;
        };
        let channel_id = load_channel_id.clone();
        page_loading.set(true);
        spawn(async move {
            let kind = match selected_tab {
                Some(ChannelMediaTab::Shorts) => "shorts",
                Some(ChannelMediaTab::Live) => "live",
                _ => "videos",
            };
            match get_channel_media_page(channel_id, kind.into(), token).await {
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
                Err(_) => app_state.show_toast("Could not load the next page", Color::Warning),
            }
            page_loading.set(false);
        });
    };

    rsx! {
        PageHeader {
            // Deliberately untitled: the page leads with the channel name and
            // avatar, and repeating it in the bar pushed the actions off the
            // edge on a long name.
            title: String::new(),
            back_to: Route::Subscriptions {},
            toolbar: rsx! { FeedFilterSegments { value: tab } },
        }
        // Pull down to refresh replaces the button that used to sit in the
        // toolbar next to the segments.
        Content { on_refresh: refresh, refreshing: channel_refreshing(),
            Stack { gap: Space::Lg,
                ChannelHero { channel }
                VideoGrid {
                    videos,
                    empty_message: match selected_tab {
                        Some(ChannelMediaTab::Shorts) => "No Shorts were returned for this channel.".to_string(),
                        Some(ChannelMediaTab::Live) => "No livestreams were returned for this channel.".to_string(),
                        Some(ChannelMediaTab::Videos) => "No videos were returned for this channel.".to_string(),
                        None => "No uploads were returned for this channel.".to_string(),
                    }
                }
                InfiniteScroll {
                    loading: page_loading(),
                    complete: !has_next_page,
                    on_load: load_more,
                }
            }
        }
    }
}
