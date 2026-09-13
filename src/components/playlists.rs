use crate::{
    app::Route,
    models::{DurationFilter, PlaylistKind, PlaylistSort, Video},
    state::AppState,
};
use dioxus::prelude::*;
use dioxus_icons::lucide::{CheckCheck, ListPlus, Play, Plus, Shuffle, Trash2};
use g3_route_transitions::animated_navigate;
use g3_ui::{
    Body, Button, ButtonSize, ButtonStyle, Card, Field, Modal, RightSlot, SegmentButton,
    SegmentGroup, StatusColor,
};

use super::{PageHeader, VideoGrid, duration_candidates, use_duration_hydration};

/// Fisher-Yates with a generator of its own.
///
/// A shuffled playlist needs no more than an order the viewer cannot predict.
/// That does not justify a randomness crate in the client bundle.
fn shuffled(mut ids: Vec<String>) -> Vec<String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    use web_time::{SystemTime, UNIX_EPOCH};

    // The clock alone is not enough. On the web `SystemTime` is `Date.now()`,
    // so two shuffles inside the same millisecond would deal the same order;
    // the counter is what separates a second tap from the first.
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
        ^ SEQUENCE.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed);
    let mut state = seed;
    let mut next = move || {
        // splitmix64.
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    for index in (1..ids.len()).rev() {
        ids.swap(index, (next() % (index as u64 + 1)) as usize);
    }
    ids
}

/// Start a run of specific videos: the first one plays, the rest wait in the
/// queue.
///
/// Used by Shuffle, whose order exists only for this sitting and so has to be
/// written down. Playing a playlist in its own order goes through
/// [`AppState::start_playlist_run`] instead, which queues the playlist itself.
///
/// The playing video is deliberately left out of the queue. It is what the
/// queue is *for* - the list of what comes next - and the player drops the
/// finished video from it as it advances anyway.
fn play_run(app_state: AppState, ids: Vec<String>, verb: &str) {
    let Some((first, rest)) = ids.split_first() else {
        return;
    };
    // A shuffled order is not the playlist's order, so any playlist run in
    // progress would fight it for what comes next.
    app_state.clear_playlist_run();
    app_state.queue_run(rest);
    let count = ids.len();
    app_state.show_toast(
        format!("{verb} {count} video{}", if count == 1 { "" } else { "s" }),
        StatusColor::Success,
    );
    let first = first.clone();
    spawn(async move {
        animated_navigate(Route::VideoDetail { id: first }).await;
    });
}

/// Hand the playlist to the queue as a run, then open `video_id`.
///
/// The whole point of the marker: what comes next is decided when the current
/// video ends, against the playlist as it is arranged then, so this does not have
/// to know or freeze the order.
fn play_from_playlist(app_state: AppState, playlist_id: &str, video_id: String, announce: bool) {
    app_state.start_playlist_run(playlist_id);
    if announce {
        // Says that the *playlist* is now what plays next, which opening one
        // video does not. Tapping a card is its own answer, so it stays quiet.
        app_state.show_toast("Playing this playlist", StatusColor::Success);
    }
    spawn(async move {
        animated_navigate(Route::VideoDetail { id: video_id }).await;
    });
}

/// One row of the index, resolved once per render.
///
/// Built from a lookup rather than a scan per playlist: finding three
/// thumbnails used to walk the whole video cache for every playlist on the
/// page, which is tens of thousands of rows each time.
struct PlaylistRow {
    id: String,
    name: String,
    count: usize,
    unwatched: usize,
    thumbnails: Vec<(String, bool)>,
}

fn chip_class(active: bool) -> &'static str {
    if active {
        "group-filter active"
    } else {
        "group-filter"
    }
}

#[component]
pub fn Playlists() -> Element {
    let app_state = use_context::<AppState>();
    let mut create_open = use_signal(|| false);
    let mut playlist_name = use_signal(String::new);
    let mut delete_target = use_signal(|| None::<(String, String)>);
    let mut delete_open = use_signal(|| false);
    // No filters here. The index is a set of covers to pick from, not a list to
    // work through - filtering it hides the playlist you came to open.
    let rows = app_state.with_library(|library| {
        let by_id = library
            .videos
            .iter()
            .map(|video| (video.id.as_str(), video))
            .collect::<std::collections::HashMap<_, _>>();
        library
            .playlists
            .iter()
            .map(|playlist| {
                let mut unwatched = 0;
                let mut thumbnails = Vec::new();
                for id in &playlist.video_ids {
                    let Some(video) = by_id.get(id.as_str()) else {
                        continue;
                    };
                    if !video.watched {
                        unwatched += 1;
                    }
                    if thumbnails.len() < 3 {
                        thumbnails.push((video.thumbnail_url.clone(), video.is_short));
                    }
                }
                PlaylistRow {
                    id: playlist.id.clone(),
                    name: playlist.name.clone(),
                    count: playlist.video_ids.len(),
                    unwatched,
                    thumbnails,
                }
            })
            .collect::<Vec<_>>()
    });
    let delete_name = delete_target()
        .map(|(_, name)| name)
        .unwrap_or_else(|| "This playlist".into());

    rsx! {
        PageHeader {}
        Body { padding: false,
            main { class: "page playlists-page",
                div { class: "page-actions-row",
                    Button {
                        start: rsx! { Plus { size: 17 } },
                        onclick: move |_| create_open.set(true),
                        "New playlist"
                    }
                }
                div { class: "playlist-grid",
                    for row in rows {
                        {
                            let playlist_id = row.id.clone();
                            let remove_id = row.id.clone();
                            let remove_name = row.name.clone();
                            let collage_class = format!("playlist-collage count-{}", row.thumbnails.len());
                            let video_count = row.count;
                            let unwatched = row.unwatched;
                            rsx! {
                                Card {
                                    key: "{row.id}",
                                    class: "playlist-card",
                                    title: row.name.clone(),
                                    right_slot: RightSlot::Element(rsx! {
                                        div { class: "playlist-card-actions",
                                            button {
                                                class: "playlist-card-action danger",
                                                aria_label: "Delete playlist".to_string(),
                                                title: "Delete playlist".to_string(),
                                                onclick: move |event: MouseEvent| {
                                                    event.stop_propagation();
                                                    delete_target.set(Some((remove_id.clone(), remove_name.clone())));
                                                    delete_open.set(true);
                                                },
                                                Trash2 { size: 15 }
                                            }
                                        }
                                    }),
                                    onclick: move |_| { { let v = playlist_id.clone(); spawn(async move { animated_navigate(Route::PlaylistDetail { id: v }).await; }); }; },
                                    div { class: collage_class,
                                        if row.thumbnails.is_empty() {
                                            div { class: "playlist-empty-art", ListPlus { size: 30 } }
                                        } else {
                                            for (thumbnail, is_short) in row.thumbnails {
                                                img {
                                                    class: if is_short { "playlist-collage-short" } else { "" },
                                                    src: "{thumbnail}",
                                                    alt: "",
                                                    loading: "lazy"
                                                }
                                            }
                                        }
                                    }
                                    div { class: "playlist-card-meta",
                                        span { class: "playlist-count",
                                            if video_count == 1 { "1 video" } else { "{video_count} videos" }
                                        }
                                        // The count that answers "is there
                                        // anything left in here", which is the
                                        // reason to open one.
                                        if unwatched > 0 && unwatched < video_count {
                                            span { class: "playlist-unwatched", "{unwatched} unwatched" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Modal {
                open: delete_open,
                title: "Delete playlist?".to_string(),
                description: rsx! { p { "{delete_name} will be removed from this device and your sync server." } },
                actions: rsx! {
                    Button { style: ButtonStyle::Clear, onclick: move |_| delete_open.set(false), "Cancel" }
                    Button {
                        style: ButtonStyle::Danger,
                        onclick: move |_| {
                            if let Some((id, _)) = delete_target()
                                && let Some(name) = app_state.delete_playlist(&id)
                            {
                                app_state.show_toast(format!("Deleted {name}"), StatusColor::Neutral);
                            }
                            delete_target.set(None);
                            delete_open.set(false);
                        },
                        "Delete"
                    }
                },
            }
            Modal {
                open: create_open,
                title: "New playlist".to_string(),
                description: rsx! { p { "It will be cached on this device immediately." } },
                actions: rsx! {
                    Button { style: ButtonStyle::Clear, onclick: move |_| create_open.set(false), "Cancel" }
                    Button {
                        disabled: playlist_name().trim().is_empty(),
                        onclick: move |_| {
                            let name = playlist_name().trim().to_string();
                            if name.is_empty() { return; }
                            app_state.create_playlist(name.clone());
                            playlist_name.set(String::new());
                            create_open.set(false);
                            app_state.show_toast(format!("Created {name}"), StatusColor::Success);
                        },
                        "Create"
                    }
                },
                Field {
                    label: "Playlist name".to_string(),
                    value: playlist_name,
                    placeholder: "Sunday watchlist".to_string(),
                    autofocus: true,
                }
            }
        }
    }
}

#[component]
pub fn PlaylistDetail(id: String) -> Element {
    let app_state = use_context::<AppState>();
    // The arrangement lives in settings rather than in local signals, because a
    // run resolved out of the queue has to be able to read it long after this
    // page is gone. See `PlaylistView`.
    let view = app_state.playlist_view(&id);
    // The segmented control owns an index, so the persisted kind is mirrored into
    // one. Settings hydrate from local storage after the first render, hence the
    // effect rather than an initial value alone.
    let mut kind_index = use_signal(|| view.kind.index());
    let persisted_kind_id = id.clone();
    use_effect(move || {
        let persisted = app_state.playlist_view(&persisted_kind_id).kind.index();
        if kind_index() != persisted {
            kind_index.set(persisted);
        }
    });

    // Resolved through a borrow, and through a lookup rather than a scan per
    // entry: a playlist of fifty videos was walking the whole cache fifty times
    // on every render.
    let (playlist, mut videos) = app_state.with_library(|library| {
        let playlist = library
            .playlists
            .iter()
            .find(|playlist| playlist.id == id)
            .cloned();
        let videos = playlist
            .as_ref()
            .map(|playlist| {
                let by_id = library
                    .videos
                    .iter()
                    .map(|video| (video.id.as_str(), video))
                    .collect::<std::collections::HashMap<_, _>>();
                playlist
                    .video_ids
                    .iter()
                    .filter_map(|id| by_id.get(id.as_str()).map(|video| (*video).clone()))
                    .collect::<Vec<Video>>()
            })
            .unwrap_or_default();
        (playlist, videos)
    });

    // Ahead of the early return below, because a hook has to run on every
    // render of this component, and ahead of the filters, because
    // `duration_candidates` has to see the rows a duration chip discards.
    //
    // A playlist is not paged, so the window is the whole list: one visit works
    // through it a batch at a time until every entry has a length. That is the
    // point - a saved video is usually saved from the feed, where RSS gave it no
    // runtime, and the refresh backfill only ever reaches followed channels.
    use_duration_hydration(duration_candidates(&videos, videos.len()));

    let Some(playlist) = playlist else {
        // Still renders the bar: reaching a stale deep link with no way back was
        // a dead end.
        return rsx! {
            PageHeader { title: "Playlist".to_string(), back_to: Route::Playlists {} }
            Body { padding: false,
                main { class: "page",
                    div { class: "empty-state", p { "This playlist is not in the local cache." } }
                }
            }
        };
    };

    let saved_count = playlist.video_ids.len();
    let watched_count = videos.iter().filter(|video| video.watched).count();
    // Same trap as the feed: the subscription RSS carries no duration, so a
    // saved row often stores 0, and a duration chip is right to exclude it but
    // cannot say so on its own. Counted before the filter runs, because after it
    // they are gone - and past the kind filter, so a Shorts view does not claim
    // to be missing the lengths of long-form uploads it was not showing anyway.
    let without_duration = if view.duration.is_some() {
        videos
            .iter()
            .filter(|video| {
                view.kind.matches(video) && !video.is_live && video.duration_seconds == 0
            })
            .count()
    } else {
        0
    };
    // Exactly what a run walks, arranged the same way, so what is on screen and
    // what plays next cannot disagree.
    view.arrange(&mut videos, &app_state.settings());
    videos.retain(|video| view.shows(video));

    let run: Vec<String> = videos.iter().map(|video| video.id.clone()).collect();
    let shuffle_run = run.clone();
    let filtered = view.is_filtered();
    let clear_id = playlist.id.clone();
    let play_all_id = playlist.id.clone();
    let play_all_first = run.first().cloned();
    let grid_playlist_id = playlist.id.clone();
    let chip_view = view.clone();
    let sort_view = view.clone();
    let unwatched_view = view.clone();
    let kind_view = view.clone();
    let kind_id = playlist.id.clone();
    let unwatched_id = playlist.id.clone();
    let chip_id = playlist.id.clone();
    let sort_id = playlist.id.clone();

    rsx! {
        PageHeader {
            title: playlist.name.clone(),
            back_to: Route::Playlists {},
            end_slot: rsx! {
                span { class: "playlist-header-count",
                    if saved_count == 1 { "1 video" } else { "{saved_count} videos" }
                }
            },
            // The same control the feed carries, in the same place, because it
            // answers the same question: which kind of upload am I looking at.
            toolbar: rsx! {
                SegmentGroup {
                    active: kind_index,
                    on_change: move |index: usize| {
                        let mut updated = kind_view.clone();
                        updated.kind = PlaylistKind::from_index(index);
                        app_state.set_playlist_view(&kind_id, updated);
                    },
                    for kind in PlaylistKind::ALL {
                        SegmentButton { index: kind.index(), "{kind.label()}" }
                    }
                }
            },
        }
        Body { padding: false,
            main { class: "page playlist-detail-page",
                // One row of actions: the two ways to start watching, and the
                // tidying that acts on the same list. Cleanup stays quiet, and on
                // the trailing edge, so it is never the thing a thumb lands on.
                header { class: "playlist-lead",
                    div { class: "playlist-lead-actions",
                        Button {
                            style: ButtonStyle::Solid,
                            class: "playlist-play-all".to_string(),
                            disabled: run.is_empty(),
                            start: rsx! { Play { size: 17, fill: "currentColor" } },
                            onclick: move |_| {
                                if let Some(first) = play_all_first.clone() {
                                    play_from_playlist(app_state, &play_all_id, first, true);
                                }
                            },
                            "Play all"
                        }
                        Button {
                            style: ButtonStyle::Neutral,
                            disabled: shuffle_run.len() < 2,
                            start: rsx! { Shuffle { size: 17 } },
                            onclick: move |_| play_run(app_state, shuffled(shuffle_run.clone()), "Shuffling"),
                            "Shuffle"
                        }
                        if watched_count > 0 {
                            Button {
                                style: ButtonStyle::Clear,
                                size: ButtonSize::Sm,
                                class: "playlist-remove-watched".to_string(),
                                // The label is hidden on narrow screens.
                                aria_label: "Remove watched".to_string(),
                                start: rsx! { CheckCheck { size: 15 } },
                                onclick: move |_| {
                                    let removed = app_state.remove_watched_from_playlist(&clear_id);
                                    if removed > 0 {
                                        app_state.show_toast(
                                            format!("Removed {removed} watched video{}", if removed == 1 { "" } else { "s" }),
                                            StatusColor::Success,
                                        );
                                    }
                                },
                                span { class: "playlist-remove-watched-label", "Remove watched" }
                            }
                        }
                    }
                }
                if saved_count > 1 {
                    nav { class: "group-filter-row", aria_label: "Playlist filters and order",
                        span { class: "group-filter-label", "Show" }
                        button {
                            class: chip_class(unwatched_view.only_unwatched),
                            aria_pressed: unwatched_view.only_unwatched.to_string(),
                            onclick: move |_| {
                                let mut updated = unwatched_view.clone();
                                updated.only_unwatched = !updated.only_unwatched;
                                app_state.set_playlist_view(&unwatched_id, updated);
                            },
                            "Unwatched"
                        }
                        for duration in DurationFilter::ALL {
                            {
                                let active = chip_view.duration == Some(duration);
                                let chip_view = chip_view.clone();
                                let chip_id = chip_id.clone();
                                rsx! {
                                    button {
                                        key: "{duration.label()}",
                                        class: if active { "group-filter duration-filter active" } else { "group-filter duration-filter" },
                                        aria_pressed: active.to_string(),
                                        onclick: move |_| {
                                            let mut updated = chip_view.clone();
                                            updated.duration = if active { None } else { Some(duration) };
                                            app_state.set_playlist_view(&chip_id, updated);
                                        },
                                        "{duration.label()}"
                                    }
                                }
                            }
                        }
                        span { class: "filter-divider", aria_hidden: "true" }
                        span { class: "group-filter-label", "Sort" }
                        for option in PlaylistSort::ALL {
                            {
                                let active = sort_view.sort == option;
                                let direction = if active { sort_view.descending } else { option.default_descending() };
                                let sort_view = sort_view.clone();
                                let sort_id = sort_id.clone();
                                rsx! {
                                    button {
                                        key: "{option:?}",
                                        class: chip_class(active),
                                        aria_pressed: active.to_string(),
                                        title: if active { "Tap again to reverse" } else { "" },
                                        onclick: move |_| {
                                            let mut updated = sort_view.clone();
                                            if active {
                                                updated.descending = !updated.descending;
                                            } else {
                                                updated.sort = option;
                                                updated.descending = option.default_descending();
                                            }
                                            app_state.set_playlist_view(&sort_id, updated);
                                        },
                                        "{option.label(direction)}"
                                    }
                                }
                            }
                        }
                    }
                }
                if without_duration > 0 {
                    p { class: "feed-filter-note",
                        "{without_duration} more "
                        if without_duration == 1 { "video has" } else { "videos have" }
                        " no length yet, so a duration filter cannot speak for them. They are being filled in now."
                    }
                }
                VideoGrid {
                    videos,
                    playlist_id: grid_playlist_id,
                    empty_message: if filtered {
                        "Nothing here matches those filters.".to_string()
                    } else {
                        "Add videos from the buttons on a card.".to_string()
                    },
                }
            }
        }
    }
}
