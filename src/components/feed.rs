use crate::{
    api::{refresh_subscription_feed, sync_library},
    models::{DurationFilter, FeedFilter},
    state::AppState,
};
use dioxus::prelude::*;
use g3_ui::{Body, Button, ButtonStyle, Refresher, SegmentButton, SegmentGroup, StatusColor};

use super::{PageHeader, VideoGrid};

/// How many videos the feed renders before asking for more.
///
/// The whole subscription history was being laid out on every visit, which is
/// what made opening the feed feel slow: the work grows with the cache, not
/// with what the reader can see.
const FEED_PAGE_SIZE: usize = 24;

#[component]
pub fn Feed() -> Element {
    let mut app_state = use_context::<AppState>();
    // Owned by the header's segmented control.
    let filter_index = app_state.feed_filter_index;
    let mut selected_group = use_signal(|| "all".to_string());
    let mut selected_duration = use_signal(|| None::<DurationFilter>);
    let mut visible_count = use_signal(|| FEED_PAGE_SIZE);
    let filter = match filter_index() {
        1 => FeedFilter::Videos,
        2 => FeedFilter::Shorts,
        3 => FeedFilter::Live,
        _ => FeedFilter::All,
    };
    // Read through a borrow and copy out only the videos that survive the
    // subscription filter. Cloning the whole snapshot first meant every render
    // copied the entire cache - tens of thousands of videos - to show a page of
    // them, which is what stalled the swipe release animation.
    let (groups, mut videos) = app_state.with_library(|library| {
        // Each subscription carries its own Videos/Shorts/both preference, so
        // the feed keeps a video only when its channel is subscribed *and* that
        // channel still wants that kind of upload.
        let subscribed_channels = library
            .channels
            .iter()
            .filter(|channel| channel.subscribed)
            .map(|channel| (channel.id.as_str(), channel.subscription_content))
            .collect::<std::collections::HashMap<_, _>>();
        let videos = library
            .videos
            .iter()
            .filter(|video| {
                subscribed_channels
                    .get(video.channel_id.as_str())
                    .is_some_and(|content| content.accepts(video.is_short))
            })
            .cloned()
            .collect::<Vec<_>>();
        (library.subscription_groups.clone(), videos)
    });
    videos.sort_by_cached_key(|video| std::cmp::Reverse(video.published_epoch()));
    let active_group = selected_group();
    if active_group == "ungrouped" {
        let grouped_ids = groups
            .iter()
            .flat_map(|group| group.channel_ids.iter())
            .collect::<Vec<_>>();
        videos.retain(|video| !grouped_ids.contains(&&video.channel_id));
    } else if active_group != "all"
        && let Some(group) = groups.iter().find(|group| group.id == active_group)
    {
        videos.retain(|video| group.channel_ids.contains(&video.channel_id));
    }
    match filter {
        FeedFilter::All => {}
        FeedFilter::Videos => videos.retain(|video| !video.is_live && !video.is_short),
        FeedFilter::Shorts => videos.retain(|video| video.is_short && !video.is_live),
        FeedFilter::Live => videos.retain(|video| video.is_live),
    }
    // A duration chip can only speak for videos whose length is known, and the
    // subscription feed is built from YouTube's RSS, which does not carry one -
    // `feed_entry_video` stores 0 because there is nothing to store. Measured
    // here: 57 of 11,762 rows had a duration, so a chip silently discarded 99.5%
    // of the library and left too little behind to page, which is why the Load
    // more button looked broken rather than the filter.
    //
    // The filter itself is right to exclude them - an unknown length is not
    // long. What was missing is saying so.
    let mut without_duration = 0usize;
    if let Some(duration) = selected_duration() {
        let settings = app_state.settings();
        without_duration = videos
            .iter()
            .filter(|video| !video.is_live && video.duration_seconds == 0)
            .count();
        videos.retain(|video| duration.matches(video, &settings));
    }
    if app_state.settings().hide_watched {
        videos.retain(|video| !video.watched);
    }
    // Paged last, so the count reflects what the filters actually left.
    let remaining = videos.len().saturating_sub(visible_count());
    videos.truncate(visible_count());

    rsx! {
        PageHeader {
            toolbar: rsx! {
                SegmentGroup { active: app_state.feed_filter_index,
                    SegmentButton { index: 0, "All" }
                    SegmentButton { index: 1, "Videos" }
                    SegmentButton { index: 2, "Shorts" }
                    SegmentButton { index: 3, "Live" }
                }
            },
        }
        Body { padding: false,
        // Pull down to refresh, replacing the button that sat in the intro row.
        Refresher {
            refreshing: app_state.syncing(),
            can_refresh: true,
            on_refresh: move |_| {
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
                            app_state.adopt_library(refresh.library);
                            let message = if refresh.imported == 0 {
                                "No new videos".to_string()
                            } else if refresh.failed_channels == 0 {
                                format!("Feed refreshed from {}", refresh.sources.join(" + "))
                            } else {
                                "Feed refreshed; cached results filled the few sources that were unavailable"
                                    .to_string()
                            };
                            app_state.show_toast(message, StatusColor::Success);
                        }
                        Err(_) => app_state.show_toast(
                            "Offline — showing your cached feed",
                            StatusColor::Warning,
                        ),
                    }
                    app_state.syncing.set(false);
                });
            },
            main { class: "page feed-page",
            // Count and group filters share one row rather than stacking two
            // thin bands above the grid.
            nav { class: "group-filter-row", aria_label: "Subscription groups",
                for duration in DurationFilter::ALL {
                    button {
                        key: "{duration.label()}",
                        class: if selected_duration() == Some(duration) { "group-filter duration-filter active" } else { "group-filter duration-filter" },
                        aria_pressed: (selected_duration() == Some(duration)).to_string(),
                        onclick: move |_| {
                            selected_duration.set(if selected_duration() == Some(duration) {
                                None
                            } else {
                                Some(duration)
                            });
                        },
                        "{duration.label()}"
                    }
                }
                span { class: "filter-divider", aria_hidden: "true" }
                button {
                    class: if selected_group() == "all" { "group-filter active" } else { "group-filter" },
                    onclick: move |_| selected_group.set("all".into()),
                    "All"
                }
                for group in groups {
                    {
                        let group_id = group.id.clone();
                        let active = selected_group() == group.id;
                        rsx! {
                            button {
                                class: if active { "group-filter active" } else { "group-filter" },
                                key: "{group.id}",
                                onclick: move |_| selected_group.set(group_id.clone()),
                                "{group.name}"
                            }
                        }
                    }
                }
                button {
                    class: if selected_group() == "ungrouped" { "group-filter active" } else { "group-filter" },
                    onclick: move |_| selected_group.set("ungrouped".into()),
                    "Ungrouped"
                }
            }

                if without_duration > 0 {
                    p { class: "feed-filter-note",
                        "{without_duration} more "
                        if without_duration == 1 { "video has" } else { "videos have" }
                        " no length yet, so they cannot be sorted by duration. Opening one records it."
                    }
                }

                VideoGrid {
                    videos,
                    empty_message: "Try another filter or refresh when you are back online.".to_string(),
                }

            if remaining > 0 {
                div { class: "load-more-row",
                    Button {
                        style: ButtonStyle::Neutral,
                        onclick: move |_| visible_count += FEED_PAGE_SIZE,
                        "Load {remaining.min(FEED_PAGE_SIZE)} more"
                    }
                }
            }
        }
        }
        }
    }
}
