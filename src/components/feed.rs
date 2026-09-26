use crate::{
    api::{get_feed_page, refresh_subscription_feed},
    models::{DurationFilter, FeedFilter, FeedGroup, FeedQuery},
    state::AppState,
};
use dioxus::prelude::*;
use g3_cache::{invalidate_cached, use_cached};
use g3_ui::{
    Chip, Color, Content, Divider, DividerOrientation, InfiniteScroll, SegmentButton, SegmentGroup,
    Shelf, Space, Stack, Text, TextTone,
};

use super::{PageHeader, VideoGrid, VideoGridSkeleton, use_duration_hydration};

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
    let mut selected_group = use_signal(|| FeedGroup::All);
    let mut selected_duration = use_signal(|| None::<DurationFilter>);
    // How many pages are on screen. Each is its own cached read, so asking
    // for the next one appends it without redrawing the ones above.
    let mut pages = use_signal(|| 1usize);
    // Whether the last page on screen said there is more.
    let has_more = use_signal(|| false);
    let settings = app_state.settings();
    let groups = app_state.with_viewer(|viewer| viewer.subscription_groups.clone());
    let query = FeedQuery {
        group: selected_group(),
        kind: (app_state.feed_filter)(),
        duration: selected_duration(),
        hide_watched: settings.hide_watched,
        thresholds: (&settings).into(),
    };
    // A different question starts again at one page.
    use_effect(use_reactive!(|query| {
        let _ = &query;
        pages.set(1);
    }));

    let refresh = move |_| {
        app_state.syncing.set(true);
        spawn(async move {
            match refresh_subscription_feed().await {
                Ok(refresh) => {
                    invalidate_cached(get_feed_page);
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
        if has_more() {
            pages += 1;
        }
    };
    let page_count = pages();

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
                        selected: selected_group() == FeedGroup::All,
                        onclick: move |_| selected_group.set(FeedGroup::All),
                        "All channels"
                    }
                    for group in groups {
                        {
                            let group_id = FeedGroup::Group(group.id.clone());
                            let selected = selected_group() == group_id;
                            rsx! {
                                Chip {
                                    key: "{group.id}",
                                    selected,
                                    onclick: move |_| selected_group.set(group_id.clone()),
                                    "{group.name}"
                                }
                            }
                        }
                    }
                    Chip {
                        selected: selected_group() == FeedGroup::Ungrouped,
                        onclick: move |_| selected_group.set(FeedGroup::Ungrouped),
                        "Ungrouped"
                    }
                }

                for page in 0..page_count {
                    FeedPageView {
                        key: "{page}",
                        query: query.clone(),
                        page,
                        last: page + 1 == page_count,
                        has_more,
                    }
                }

                InfiniteScroll {
                    loading: false,
                    complete: !has_more(),
                    on_load: load_next_page,
                }
            }
        }
    }
}

/// One page of the feed: its own cached read, so the pages above it stay put
/// while it loads.
#[component]
fn FeedPageView(query: FeedQuery, page: usize, last: bool, has_more: Signal<bool>) -> Element {
    let feed = use_cached(get_feed_page, (query, page));
    let (candidates, more) = match &*feed.read() {
        Some(Ok(found)) => (found.unknown_durations.clone(), found.has_more),
        _ => (Vec::new(), false),
    };
    use_duration_hydration(candidates);
    // The last page decides whether there is another.
    use_effect(use_reactive!(|last, more| {
        if last && *has_more.peek() != more {
            has_more.set(more);
        }
    }));

    match &*feed.read() {
        None => rsx! { VideoGridSkeleton { count: if page == 0 { 8 } else { 4 } } },
        Some(Err(_)) if page == 0 => rsx! {
            VideoGrid {
                videos: Vec::new(),
                empty_message: "Could not load your feed. Pull down to try again.".to_string(),
            }
        },
        Some(Err(_)) => rsx! {},
        Some(Ok(found)) => {
            let without = found.without_duration;
            rsx! {
                if without > 0 {
                    Text { tone: TextTone::Secondary,
                        "{without} more "
                        if without == 1 { "video has" } else { "videos have" }
                        " no length yet, so a duration filter cannot speak for them. Visible cards are being filled in now."
                    }
                }
                if page == 0 || !found.videos.is_empty() {
                    VideoGrid {
                        videos: found.videos.clone(),
                        empty_message: "Try another filter, or follow some channels from Explore.".to_string(),
                    }
                }
            }
        }
    }
}
