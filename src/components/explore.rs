use crate::{
    api::{search_catalog, search_catalog_page},
    app::Route,
    models::{ExploreFilter, SearchResults},
    state::AppState,
};
use dioxus::prelude::*;
use g3_route_transitions::animated_navigate;
use g3_ui::{
    Avatar, AvatarSize, Button, ButtonFill, Chip, Color, Content, Searchbar, SegmentButton,
    SegmentGroup, Shelf, Space, Spinner, Stack, StackAlign, Text, TextTone,
};
use std::collections::{HashMap, HashSet};

use super::{PageHeader, VideoGrid};

/// How many recent videos stand in for a query on an empty search page.
const SUGGESTION_COUNT: usize = 24;

#[component]
pub fn Explore() -> Element {
    let app_state = use_context::<AppState>();
    let search = use_signal(String::new);
    let filter = (app_state.explore_filter)();
    let mut results = use_signal(|| None::<SearchResults>);
    let mut searching = use_signal(|| false);
    let needle = search().trim().to_lowercase();
    let library = app_state.library();
    let mut videos = library.videos.clone();
    let mut channels = library.channels.clone();
    if !needle.is_empty() {
        videos.retain(|video| {
            video.title.to_lowercase().contains(&needle)
                || video.channel_name.to_lowercase().contains(&needle)
        });
        channels.retain(|channel| {
            channel.name.to_lowercase().contains(&needle)
                || channel.handle.to_lowercase().contains(&needle)
        });
    } else {
        channels.clear();
        let subscribed_ids = library
            .channels
            .iter()
            .filter(|channel| channel.subscribed)
            .map(|channel| channel.id.as_str())
            .collect::<HashSet<_>>();
        videos.retain(|video| subscribed_ids.contains(video.channel_id.as_str()) && !video.watched);
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
            .cloned()
            .collect::<Vec<_>>();
        let mut familiar = videos.clone();
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
                && seen.insert(video.id.clone())
            {
                ranked.push(video.clone());
            }
        }
        for video in recent.into_iter().chain(familiar) {
            if ranked.len() >= SUGGESTION_COUNT {
                break;
            }
            if seen.insert(video.id.clone()) {
                ranked.push(video);
            }
        }
        videos = ranked;
    }

    if let Some(remote) = results()
        && remote.query.trim().eq_ignore_ascii_case(search().trim())
    {
        videos = remote.videos;
        channels = remote.channels;
    }
    match filter {
        ExploreFilter::Videos => channels.clear(),
        ExploreFilter::Channels => videos.clear(),
        ExploreFilter::All => {}
    }
    let result_count = videos.len() + channels.len();
    let search_value = search();
    let next_page = results()
        .filter(|results| results.query.trim().eq_ignore_ascii_case(search().trim()))
        .and_then(|results| results.next_page);
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
                VideoGrid { videos, empty_message: "No matches yet. Check the source connection or try another search.".to_string() }
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
