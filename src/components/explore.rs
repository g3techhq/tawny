use crate::{
    api::{get_feed_page, search_catalog, search_catalog_page},
    app::Route,
    models::{Channel, ExploreFilter, FeedFilter, FeedGroup, FeedQuery, SearchResults, Video},
    state::AppState,
};
use dioxus::prelude::*;
use g3_cache::use_cached;
use g3_route_transitions::animated_navigate;
use g3_ui::{
    Avatar, AvatarSize, Button, ButtonFill, Chip, Color, Content, InfiniteScroll, Searchbar,
    SegmentButton, SegmentGroup, Shelf, Space, Spinner, Stack, StackAlign, Text, TextTone,
};

use super::{PageHeader, VideoGrid, VideoGridSkeleton};

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
    let explore_filter = app_state.explore_filter;
    let mut results = use_signal(|| None::<SearchResults>);
    let mut searching = use_signal(|| false);
    let mut visible_count = use_signal(|| RESULT_PAGE_SIZE);
    let needle = use_memo(move || search().trim().to_lowercase());
    // A new query or filter is a new list: start it at one page again.
    use_effect(move || {
        let _ = (needle(), explore_filter());
        visible_count.set(RESULT_PAGE_SIZE);
    });
    // With nothing typed, the page suggests the newest unwatched uploads from
    // what the viewer follows: the first page of an unwatched feed, which is
    // also what type-ahead filters before Enter asks the server.
    let thresholds = (&*app_state.settings.read()).into();
    let suggestions = use_cached(
        get_feed_page,
        (
            FeedQuery {
                group: FeedGroup::All,
                kind: FeedFilter::All,
                duration: None,
                hide_watched: true,
                thresholds,
            },
            0,
        ),
    );
    // What the page shows: the server's results for this query, or the
    // suggestions matched on the device. Worked out when one of those, the
    // filter or the page size changes, not on every render (a keystroke
    // re-renders the search box, not this).
    let shown = use_memo(move || {
        let filter = explore_filter();
        let page = visible_count();
        let results = results.read();
        if let Some(remote) = results
            .as_ref()
            .filter(|remote| remote.query.trim().eq_ignore_ascii_case(search().trim()))
        {
            return paged(
                remote.videos.iter().collect(),
                remote.channels.iter().collect(),
                filter,
                page,
            );
        }
        let suggestions = suggestions.read();
        let suggested: &[Video] = match &*suggestions {
            None => return Shown::loading(),
            Some(Ok(feed)) => &feed.videos,
            Some(Err(_)) => &[],
        };
        let needle = needle.read();
        app_state.with_viewer(|viewer| {
            let (videos, channels) = local_matches(suggested, &viewer.subscriptions, &needle);
            paged(videos, channels, filter, page)
        })
    });
    let filter = explore_filter();
    let search_value = search();
    // The next page of the server's results for what is typed, if there is one.
    let next_page = move || {
        results
            .peek()
            .as_ref()
            .filter(|remote| {
                remote
                    .query
                    .trim()
                    .eq_ignore_ascii_case(search.peek().trim())
            })
            .and_then(|remote| remote.next_page.clone())
    };
    let has_next_page = results.read().as_ref().is_some_and(|remote| {
        remote.next_page.is_some() && remote.query.trim().eq_ignore_ascii_case(search().trim())
    });
    let (loading, result_count, remaining) = {
        let shown = shown.read();
        (shown.loading, shown.result_count, shown.remaining)
    };
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
    let load_more = move |_| {
        let Some(token) = next_page() else {
            return;
        };
        let query = search().trim().to_string();
        searching.set(true);
        spawn(async move {
            match search_catalog_page(query, filter.query().to_string(), token).await {
                Ok(page) => {
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
                    if needle.read().is_empty() {
                        "The newest unwatched uploads from channels you follow"
                    } else {
                        "{result_count} results for “{search_value}”. Press Enter to search online."
                    }
                }
                if !shown.read().channels.is_empty() {
                    Shelf { title: "Channels", gap: Space::Sm,
                        for channel in shown.read().channels.iter() {
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
                    VideoGrid { videos: shown.map(|shown| &shown.videos), empty_message: "No matches yet. Check the source connection or try another search.".to_string() }
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

/// What the device has for `needle`: followed channels and suggested videos
/// that match, or with no query, the suggestions themselves. Enter asks the
/// server, which searches the whole catalog.
fn local_matches<'a>(
    suggestions: &'a [Video],
    subscriptions: &'a [Channel],
    needle: &str,
) -> (Vec<&'a Video>, Vec<&'a Channel>) {
    if needle.is_empty() {
        return (
            suggestions.iter().take(SUGGESTION_COUNT).collect(),
            Vec::new(),
        );
    }
    let videos = suggestions
        .iter()
        .filter(|video| {
            video.title.to_lowercase().contains(needle)
                || video.channel_name.to_lowercase().contains(needle)
        })
        .collect();
    let channels = subscriptions
        .iter()
        .filter(|channel| {
            channel.name.to_lowercase().contains(needle)
                || channel.handle.to_lowercase().contains(needle)
        })
        .collect();
    (videos, channels)
}

/// What the page shows, held in a memo.
#[derive(Clone, Debug, Default, PartialEq)]
struct Shown {
    /// Nothing to show yet: no server results and no suggestions.
    loading: bool,
    videos: Vec<Video>,
    channels: Vec<Channel>,
    /// What the query found, before paging.
    result_count: usize,
    /// How many of those are still off the page.
    remaining: usize,
}

impl Shown {
    fn loading() -> Self {
        Self {
            loading: true,
            ..Self::default()
        }
    }
}

/// The first `page` of each list under `filter`, copied out, with the total
/// the filter left and how many of those are still off the page.
fn paged(
    mut videos: Vec<&Video>,
    mut channels: Vec<&Channel>,
    filter: ExploreFilter,
    page: usize,
) -> Shown {
    match filter {
        ExploreFilter::Videos => channels.clear(),
        ExploreFilter::Channels => videos.clear(),
        ExploreFilter::All => {}
    }
    // Counted before paging, so the line above the results reports what the
    // query found rather than how much of it is on screen.
    let result_count = videos.len() + channels.len();
    let remaining = videos.len().saturating_sub(page) + channels.len().saturating_sub(page);
    Shown {
        loading: false,
        videos: videos.into_iter().take(page).cloned().collect(),
        channels: channels.into_iter().take(page).cloned().collect(),
        result_count,
        remaining,
    }
}
