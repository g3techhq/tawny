use crate::{
    api::{search_catalog, search_catalog_page},
    app::Route,
    models::{Channel, ExploreFilter, LibrarySnapshot, SearchResults, Video},
    state::AppState,
};
use dioxus::prelude::*;
use g3_route_transitions::animated_navigate;
use g3_ui::{
    Avatar, AvatarSize, Button, ButtonFill, Chip, Color, Content, InfiniteScroll, Searchbar,
    SegmentButton, SegmentGroup, Shelf, Space, Spinner, Stack, StackAlign, Text, TextTone,
};
use std::collections::{HashMap, HashSet};

use super::{PageHeader, VideoGrid, VideoGridSkeleton, use_after_first_paint};

/// How many recent videos stand in for a query on an empty search page.
const SUGGESTION_COUNT: usize = 24;

/// How many matches a query lays out before the reader asks for more.
///
/// A one-letter query matches most of a real library. Laying all of it out
/// builds tens of thousands of cards in a single render, which takes the tab
/// down with it - the page has to grow with what the reader can see, not with
/// the size of the cache.
const RESULT_PAGE_SIZE: usize = 24;

#[component]
pub fn Explore() -> Element {
    let app_state = use_context::<AppState>();
    let search = use_signal(String::new);
    let filter = (app_state.explore_filter)();
    let mut results = use_signal(|| None::<SearchResults>);
    let mut searching = use_signal(|| false);
    let mut visible_count = use_signal(|| RESULT_PAGE_SIZE);
    let needle = search().trim().to_lowercase();
    // A new query is a new list: start it at one page again.
    {
        let query = needle.clone();
        use_effect(use_reactive!(|(query, filter)| {
            let _ = (&query, filter);
            visible_count.set(RESULT_PAGE_SIZE);
        }));
    }
    // Ranking the library is the slow part of this page, so the search bar
    // and placeholders go up first.
    let painted = use_after_first_paint();
    let page = visible_count();
    let remote =
        results().filter(|remote| remote.query.trim().eq_ignore_ascii_case(search().trim()));
    let loading = remote.is_none() && !painted();
    // Read through a borrow, rank references, and copy out only the page on
    // screen. A real library holds tens of thousands of videos, and cloning
    // every match on the way to a page of them ran that copy on every
    // keystroke.
    let (videos, channels, result_count, remaining) = if let Some(remote) = &remote {
        paged(
            remote.videos.iter().collect(),
            remote.channels.iter().collect(),
            filter,
            page,
        )
    } else if loading {
        (Vec::new(), Vec::new(), 0, 0)
    } else {
        app_state.with_library(|library| {
            let (videos, channels) = local_matches(library, &needle);
            paged(videos, channels, filter, page)
        })
    };
    let search_value = search();
    let next_page = remote.and_then(|results| results.next_page);
    // Enter runs the remote search; typing filters the cache as it goes.
    let run_search = move |query: String| {
        let query = query.trim().to_string();
        if query.chars().count() < 2 || searching() {
            return;
        }
        searching.set(true);
        spawn(async move {
            match search_catalog(query, filter.query().to_string()).await {
                Ok(found) => {
                    app_state.ingest_search_results(&found);
                    if !found.remote_available {
                        app_state.show_toast(
                            "Showing cached matches — remote source is unavailable",
                            Color::Warning,
                        );
                    }
                    results.set(Some(found));
                }
                Err(_) => app_state
                    .show_toast("Server is offline — showing cached matches", Color::Warning),
            }
            searching.set(false);
        });
    };
    let has_next_page = next_page.is_some();
    let load_more = move |_| {
        let Some(token) = next_page.clone() else {
            return;
        };
        let query = search().trim().to_string();
        searching.set(true);
        spawn(async move {
            match search_catalog_page(query, filter.query().to_string(), token).await {
                Ok(page) => {
                    app_state.ingest_search_results(&page);
                    results.with_mut(|current| {
                        if let Some(current) = current.as_mut() {
                            for video in page.videos {
                                if !current.videos.iter().any(|item| item.id == video.id) {
                                    current.videos.push(video);
                                }
                            }
                            for channel in page.channels {
                                if !current.channels.iter().any(|item| item.id == channel.id) {
                                    current.channels.push(channel);
                                }
                            }
                            current.next_page = page.next_page;
                        }
                    });
                }
                Err(_) => {
                    app_state.show_toast("Could not load more search results", Color::Warning)
                }
            }
            searching.set(false);
        });
    };

    rsx! {
        PageHeader {
            toolbar: rsx! {
                SegmentGroup { value: app_state.explore_filter, aria_label: "Search for",
                    for option in ExploreFilter::ALL {
                        SegmentButton { key: "{option.label()}", value: option, "{option.label()}" }
                    }
                }
            },
        }
        Content {
            Stack { gap: Space::Md,
                Stack { horizontal: true, gap: Space::Sm, align: StackAlign::Center,
                    Searchbar {
                        class: "min-w-0 flex-1",
                        value: search,
                        placeholder: "Search videos and channels",
                        debounce_ms: 0,
                        on_submit: run_search,
                    }
                    if searching() {
                        Spinner {}
                    }
                }
                Text { tone: TextTone::Secondary,
                    if needle.is_empty() {
                        "Unwatched recents, mixed with channels you watch often"
                    } else {
                        "{result_count} results for “{search_value}”. Press Enter to search online."
                    }
                }
                if !channels.is_empty() {
                    Shelf { title: "Channels", gap: Space::Sm,
                        for channel in channels {
                            {
                                let id = channel.id.clone();
                                rsx! {
                                    Chip {
                                        key: "{channel.id}",
                                        start: rsx! {
                                            Avatar { name: channel.name.clone(), src: channel.avatar_url.clone(), size: AvatarSize::Sm }
                                        },
                                        onclick: move |_| { spawn(animated_navigate(Route::ChannelDetail { id: id.clone() })); },
                                        "{channel.name}"
                                    }
                                }
                            }
                        }
                    }
                }
                if loading {
                    VideoGridSkeleton { count: 8 }
                } else {
                    VideoGrid { videos, empty_message: "No matches yet. Check the source connection or try another search.".to_string() }
                }
                InfiniteScroll {
                    loading: false,
                    complete: remaining == 0,
                    on_load: move |_| {
                        if remaining > 0 {
                            visible_count += RESULT_PAGE_SIZE;
                        }
                    },
                }
                if has_next_page {
                    Stack { align: StackAlign::Center,
                        Button {
                            fill: ButtonFill::Outline,
                            color: Color::Neutral,
                            loading: searching(),
                            onclick: load_more,
                            "Load more results"
                        }
                    }
                }
            }
        }
    }
}

/// What the cache has for `needle`: matching videos and channels, or with no
/// query, suggestions to stand in for one.
fn local_matches<'a>(
    library: &'a LibrarySnapshot,
    needle: &str,
) -> (Vec<&'a Video>, Vec<&'a Channel>) {
    if !needle.is_empty() {
        let videos = library
            .videos
            .iter()
            .filter(|video| {
                video.title.to_lowercase().contains(needle)
                    || video.channel_name.to_lowercase().contains(needle)
            })
            .collect::<Vec<_>>();
        let channels = library
            .channels
            .iter()
            .filter(|channel| {
                channel.name.to_lowercase().contains(needle)
                    || channel.handle.to_lowercase().contains(needle)
            })
            .collect::<Vec<_>>();
        return (videos, channels);
    }

    let subscribed_ids = library
        .channels
        .iter()
        .filter(|channel| channel.subscribed)
        .map(|channel| channel.id.as_str())
        .collect::<HashSet<_>>();
    let mut videos = library
        .videos
        .iter()
        .filter(|video| subscribed_ids.contains(video.channel_id.as_str()) && !video.watched)
        .collect::<Vec<_>>();
    videos.sort_by_cached_key(|video| std::cmp::Reverse(video.published_epoch()));

    // Keep the page fresh, then mix in missed uploads from channels the
    // viewer actually returns to. History only records distinct videos,
    // so this is affinity rather than a replay-count feedback loop.
    let video_channels = library
        .videos
        .iter()
        .map(|video| (video.id.as_str(), video.channel_id.as_str()))
        .collect::<HashMap<_, _>>();
    let mut affinity = HashMap::<&str, usize>::new();
    for entry in &library.history {
        if let Some(channel_id) = video_channels.get(entry.video_id.as_str()) {
            *affinity.entry(channel_id).or_default() += 1;
        }
    }
    let recent = videos
        .iter()
        .take(SUGGESTION_COUNT)
        .copied()
        .collect::<Vec<_>>();
    let mut familiar = videos;
    familiar.sort_by_cached_key(|video| {
        (
            std::cmp::Reverse(*affinity.get(video.channel_id.as_str()).unwrap_or(&0)),
            std::cmp::Reverse(video.published_epoch()),
        )
    });
    let mut ranked = Vec::with_capacity(SUGGESTION_COUNT);
    let mut seen = HashSet::new();
    for index in 0..SUGGESTION_COUNT {
        let candidate = if index % 3 == 2 {
            familiar.get(index / 3)
        } else {
            recent.get(index - index / 3)
        };
        if let Some(video) = candidate
            && seen.insert(video.id.as_str())
        {
            ranked.push(*video);
        }
    }
    for video in recent.into_iter().chain(familiar) {
        if ranked.len() >= SUGGESTION_COUNT {
            break;
        }
        if seen.insert(video.id.as_str()) {
            ranked.push(video);
        }
    }
    (ranked, Vec::new())
}

/// The first `page` of each list under `filter`, copied out, with the total
/// the filter left and how many of those are still off the page.
fn paged(
    mut videos: Vec<&Video>,
    mut channels: Vec<&Channel>,
    filter: ExploreFilter,
    page: usize,
) -> (Vec<Video>, Vec<Channel>, usize, usize) {
    match filter {
        ExploreFilter::Videos => channels.clear(),
        ExploreFilter::Channels => videos.clear(),
        ExploreFilter::All => {}
    }
    // Counted before paging, so the line above the results reports what the
    // query found rather than how much of it is on screen.
    let result_count = videos.len() + channels.len();
    let remaining = videos.len().saturating_sub(page) + channels.len().saturating_sub(page);
    (
        videos.into_iter().take(page).cloned().collect(),
        channels.into_iter().take(page).cloned().collect(),
        result_count,
        remaining,
    )
}
