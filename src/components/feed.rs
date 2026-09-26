use crate::{
    api::{refresh_subscription_feed, sync_library},
    models::{DurationFilter, FeedFilter},
    state::AppState,
};
use dioxus::prelude::*;
use g3_ui::{
    Chip, Color, Content, Divider, DividerOrientation, InfiniteScroll, SegmentButton, SegmentGroup,
    Shelf, Space, Stack, Text, TextTone,
};
use std::collections::{HashMap, HashSet};

use super::{
    DURATION_LOOKAHEAD, PageHeader, VideoGrid, VideoGridSkeleton, duration_candidates,
    use_after_first_paint, use_duration_hydration,
};

/// How many videos the feed renders before asking for more.
///
/// The whole subscription history was being laid out on every visit, which is
/// what made opening the feed feel slow: the work grows with the cache, not
/// with what the reader can see.
const FEED_PAGE_SIZE: usize = 24;

/// The segmented control that picks what kind of upload a list shows. Shared
/// by the feed and a channel page.
#[component]
pub fn FeedFilterSegments(value: Signal<FeedFilter>) -> Element {
    rsx! {
        SegmentGroup { value, aria_label: "Show",
            for filter in FeedFilter::ALL {
                SegmentButton { key: "{filter.label()}", value: filter, "{filter.label()}" }
            }
        }
    }
}

#[component]
pub fn Feed() -> Element {
    let mut app_state = use_context::<AppState>();
    let mut selected_group = use_signal(|| "all".to_string());
    let mut selected_duration = use_signal(|| None::<DurationFilter>);
    let mut visible_count = use_signal(|| FEED_PAGE_SIZE);
    let filter = (app_state.feed_filter)();
    // Sorting and filtering the whole subscription history is the slow part
    // of this page, so the header and placeholders go up first.
    let painted = use_after_first_paint();
    let groups = app_state.with_library(|library| library.subscription_groups.clone());
    let settings = app_state.settings();
    let active_group = selected_group();
    let duration_filter = selected_duration();
    let page = visible_count();
    // Asked for before the duration filter runs - see `duration_candidates`.
    let duration_window = if duration_filter.is_some() {
        page.max(DURATION_LOOKAHEAD)
    } else {
        page
    };
    // Read through a borrow, filter and sort references, and copy out only the
    // page on screen. Cloning every match first meant each render copied tens
    // of thousands of videos to show two dozen of them.
    let (videos, remaining, without_duration, candidates) = if painted() {
        app_state.with_library(|library| {
            // Each subscription carries its own Videos/Shorts/both preference,
            // so the feed keeps a video only when its channel is subscribed
            // *and* that channel still wants that kind of upload.
            let subscribed_channels = library
                .channels
                .iter()
                .filter(|channel| channel.subscribed)
                .map(|channel| (channel.id.as_str(), channel.subscription_content))
                .collect::<HashMap<_, _>>();
            let grouped_ids = library
                .subscription_groups
                .iter()
                .flat_map(|group| group.channel_ids.iter().map(String::as_str))
                .collect::<HashSet<_>>();
            let group = library
                .subscription_groups
                .iter()
                .find(|group| group.id == active_group);
            let mut videos = library
                .videos
                .iter()
                .filter(|video| {
                    subscribed_channels
                        .get(video.channel_id.as_str())
                        .is_some_and(|content| content.accepts(video.is_short))
                })
                .filter(|video| match active_group.as_str() {
                    "all" => true,
                    "ungrouped" => !grouped_ids.contains(video.channel_id.as_str()),
                    _ => group.is_none_or(|group| group.channel_ids.contains(&video.channel_id)),
                })
                .filter(|video| filter.accepts(video))
                .filter(|video| !(settings.hide_watched && video.watched))
                .collect::<Vec<_>>();
            videos.sort_by_cached_key(|video| std::cmp::Reverse(video.published_epoch()));
            let candidates = duration_candidates(videos.iter().copied(), duration_window);
            let mut without_duration = 0usize;
            if let Some(duration) = duration_filter {
                without_duration = videos
                    .iter()
                    .filter(|video| !video.is_live && video.duration_seconds == 0)
                    .count();
                videos.retain(|video| duration.matches(video, &settings));
            }
            // Paged last, so the count reflects what the filters actually left.
            let remaining = videos.len().saturating_sub(page);
            let videos = videos.into_iter().take(page).cloned().collect::<Vec<_>>();
            (videos, remaining, without_duration, candidates)
        })
    } else {
        (Vec::new(), 0, 0, Vec::new())
    };
    use_duration_hydration(candidates);

    let refresh = move |_| {
        app_state.syncing.set(true);
        spawn(async move {
            let local = app_state.library();
            let result = match sync_library(local).await {
                Ok(remote) => {
                    app_state.adopt_library(remote);
                    refresh_subscription_feed().await
                }
                Err(error) => Err(error),
            };
            match result {
                Ok(refresh) => {
                    let message = if refresh.imported == 0 {
                        "No new videos".to_string()
                    } else if refresh.failed_channels == 0 {
                        format!("Feed refreshed from {}", refresh.sources.join(" + "))
                    } else {
                        "Feed refreshed".to_string()
                    };
                    app_state.show_toast(message, Color::Success);
                }
                Err(_) => {
                    app_state.show_toast("Offline — showing your cached feed", Color::Warning)
                }
            }
            app_state.syncing.set(false);
        });
    };
    let load_next_page = move |_| {
        if remaining == 0 {
            return;
        }
        visible_count += FEED_PAGE_SIZE;
    };

    rsx! {
        PageHeader {
            toolbar: rsx! { FeedFilterSegments { value: app_state.feed_filter } },
        }
        Content {
            // Pull down to refresh, replacing the button that sat in the intro row.
            on_refresh: refresh,
            refreshing: app_state.syncing(),
            Stack { gap: Space::Md,
                // Length and group filters share one strip rather than stacking
                // two thin bands above the grid.
                Shelf { aria_label: "Filters", gap: Space::Sm,
                    for duration in DurationFilter::ALL {
                        Chip {
                            key: "{duration.label()}",
                            selected: selected_duration() == Some(duration),
                            onclick: move |_| {
                                selected_duration.set(
                                    if selected_duration() == Some(duration) { None } else { Some(duration) },
                                );
                            },
                            "{duration.label()}"
                        }
                    }
                    Divider { orientation: DividerOrientation::Vertical }
                    Chip {
                        selected: selected_group() == "all",
                        onclick: move |_| selected_group.set("all".into()),
                        "All channels"
                    }
                    for group in groups {
                        {
                            let group_id = group.id.clone();
                            rsx! {
                                Chip {
                                    key: "{group.id}",
                                    selected: selected_group() == group.id,
                                    onclick: move |_| selected_group.set(group_id.clone()),
                                    "{group.name}"
                                }
                            }
                        }
                    }
                    Chip {
                        selected: selected_group() == "ungrouped",
                        onclick: move |_| selected_group.set("ungrouped".into()),
                        "Ungrouped"
                    }
                }

                if without_duration > 0 {
                    Text { tone: TextTone::Secondary,
                        "{without_duration} more "
                        if without_duration == 1 { "video has" } else { "videos have" }
                        " no length yet, so a duration filter cannot speak for them. Visible cards are being filled in now."
                    }
                }

                if !painted() {
                    VideoGridSkeleton { count: 8 }
                } else {
                    VideoGrid {
                        videos,
                        empty_message: "Try another filter or refresh when you are back online.".to_string(),
                    }

                    InfiniteScroll {
                        loading: false,
                        complete: remaining == 0,
                        on_load: load_next_page,
                    }
                }
            }
        }
    }
}
