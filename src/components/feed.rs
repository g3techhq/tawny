use crate::{
    api::{refresh_subscription_feed, sync_library},
    models::FeedFilter,
    state::AppState,
};
use dioxus::prelude::*;
use dioxus_icons::lucide::Sparkles;
use g3_ui::{Refresher, StatusColor};

use super::VideoGrid;

#[component]
pub fn Feed() -> Element {
    let mut app_state = use_context::<AppState>();
    // Owned by the header's segmented control.
    let filter_index = app_state.feed_filter_index;
    let mut selected_group = use_signal(|| "all".to_string());
    let filter = match filter_index() {
        1 => FeedFilter::Unwatched,
        2 => FeedFilter::Today,
        _ => FeedFilter::All,
    };
    let library = app_state.library();
    let groups = library.subscription_groups.clone();
    let subscribed_channel_ids = library
        .channels
        .iter()
        .filter(|channel| channel.subscribed)
        .map(|channel| channel.id.clone())
        .collect::<Vec<_>>();
    let mut videos = library.videos;
    videos.retain(|video| subscribed_channel_ids.contains(&video.channel_id));
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
    if app_state.settings().hide_watched || filter == FeedFilter::Unwatched {
        videos.retain(|video| !video.watched);
    }
    if filter == FeedFilter::Today {
        videos.retain(|video| {
            video.published_at.contains("minute")
                || video.published_at.contains("hour")
                || video.published_at == "Streaming now"
        });
    }

    rsx! {
        // Pull down to refresh, replacing the button that sat in the intro row.
        Refresher {
            refreshing: app_state.syncing(),
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
                                format!(
                                    "Feed updated; {} channel{} will retry later",
                                    refresh.failed_channels,
                                    if refresh.failed_channels == 1 { "" } else { "s" }
                                )
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
        }
        main { class: "page feed-page",
            // Count and group filters share one row rather than stacking two
            // thin bands above the grid.
            nav { class: "group-filter-row", aria_label: "Subscription groups",
                span { class: "feed-count", Sparkles { size: 14 } "{videos.len()} fresh" }
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

            VideoGrid { videos, empty_message: "Try another filter or refresh when you are back online.".to_string() }
        }
    }
}
