use crate::{
    api::{search_catalog, search_catalog_page},
    app::Route,
    models::SearchResults,
    state::AppState,
};
use dioxus::prelude::*;
use dx_route_transitions::animated_navigate;
use dioxus_icons::lucide::Search;
use g3_ui::Field;
use g3_ui::{Button, StatusColor};

use super::VideoGrid;

/// How many recent videos stand in for a query on an empty search page.
const SUGGESTION_COUNT: usize = 24;

#[component]
pub fn Explore() -> Element {
    let app_state = use_context::<AppState>();
    let search = use_signal(String::new);
    // Owned by the header's segmented control.
    let filter_index = app_state.explore_filter_index;
    let mut results = use_signal(|| None::<SearchResults>);
    let mut searching = use_signal(|| false);
    let needle = search().trim().to_lowercase();
    let can_search = search().trim().chars().count() >= 2;
    let library = app_state.library();
    let mut videos = library.videos;
    let mut channels = library.channels;
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
        // With no query this page was rendering the entire cached library as
        // one grid, which is what made opening it feel like it was fetching
        // something. It is a starting point, not a catalogue, so it is capped
        // at the most recent handful.
        videos.sort_by_cached_key(|video| std::cmp::Reverse(video.published_epoch()));
        videos.truncate(SUGGESTION_COUNT);
    }

    if let Some(remote) = results()
        && remote.query.trim().eq_ignore_ascii_case(search().trim())
    {
        videos = remote.videos;
        channels = remote.channels;
    }
    match filter_index() {
        1 => channels.clear(),
        2 => videos.clear(),
        _ => {}
    }
    let result_count = videos.len() + channels.len();
    let search_value = search();
    let next_page = results()
        .filter(|results| results.query.trim().eq_ignore_ascii_case(search().trim()))
        .and_then(|results| results.next_page);
    // The magnifier inside the field is the search control, so there is no
    // separate submit button to keep in sync.
    let run_search = move |_| {
        let query = search().trim().to_string();
        if query.chars().count() < 2 || searching() {
            return;
        }
        let filter = match filter_index() {
            1 => "videos",
            2 => "channels",
            _ => "all",
        }
        .to_string();
        searching.set(true);
        spawn(async move {
            match search_catalog(query, filter).await {
                Ok(found) => {
                    app_state.ingest_search_results(&found);
                    if !found.remote_available {
                        app_state.show_toast(
                            "Showing cached matches — remote source is unavailable",
                            StatusColor::Warning,
                        );
                    }
                    results.set(Some(found));
                }
                Err(_) => app_state.show_toast(
                    "Server is offline — showing cached matches",
                    StatusColor::Warning,
                ),
            }
            searching.set(false);
        });
    };

    rsx! {
        main { class: "page explore-page",
            div { class: "explore-search-stack",
                div { class: "explore-search",
                    Field {
                        label: "".to_string(),
                        value: search,
                        placeholder: "Search videos and channels".to_string(),
                        end: rsx! {
                            button {
                                class: "explore-search-submit",
                                r#type: "button",
                                aria_label: "Search".to_string(),
                                disabled: searching() || !can_search,
                                onclick: run_search,
                                if searching() {
                                    span { class: "explore-search-spinner" }
                                } else {
                                    Search { size: 19 }
                                }
                            }
                        },
                    }
                }
            }
            if !needle.is_empty() {
                p { class: "search-summary", "{result_count} results for “{search_value}”" }
            }
            if !channels.is_empty() {
                section { class: "explore-channels",
                    div { class: "subsection-heading",
                        h3 { "Channels" }
                        span { "{channels.len()} found" }
                    }
                    div { class: "channel-result-row",
                        for channel in channels {
                            {
                                let channel_id = channel.id.clone();
                                let initial = channel.name.chars().next().unwrap_or('T');
                                rsx! {
                                    button {
                                        class: "channel-result",
                                        key: "{channel.id}",
                                        onclick: move |_| { { let v = channel_id.clone(); spawn(async move { animated_navigate(Route::ChannelDetail { id: v }).await; }); }; },
                                        if let Some(avatar_url) = channel.avatar_url {
                                            img { src: "{avatar_url}", alt: "", loading: "lazy" }
                                        } else {
                                            span { class: "channel-avatar", "{initial}" }
                                        }
                                        span {
                                            strong { "{channel.name}" }
                                            small { "{channel.handle}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            VideoGrid { videos, empty_message: "No matches yet. Check the source connection or try another search.".to_string() }
            if let Some(next_page) = next_page {
                div { class: "load-more-row",
                    Button {
                        style: g3_ui::ButtonStyle::Neutral,
                        disabled: searching(),
                        onclick: move |_| {
                            let query = search().trim().to_string();
                            let filter = match filter_index() {
                                1 => "videos",
                                2 => "channels",
                                _ => "all",
                            }
                            .to_string();
                            let token = next_page.clone();
                            searching.set(true);
                            spawn(async move {
                                match search_catalog_page(query, filter, token).await {
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
                                    Err(_) => app_state.show_toast(
                                        "Could not load more search results",
                                        StatusColor::Warning,
                                    ),
                                }
                                searching.set(false);
                            });
                        },
                        if searching() { "Loading…" } else { "Load more results" }
                    }
                }
            }
        }
    }
}
