use crate::{app::Route, state::AppState};
use dioxus::prelude::*;
use dx_route_transitions::animated_navigate;
use dioxus_icons::lucide::{
    ChevronLeft, History, House, List as ListIcon, ListVideo, Search, Settings, Users,
};
use g3_ui::{
    Body, Button, ButtonStyle, Header, Navbar, NavbarTab, NavbarTabBar, SegmentButton, SegmentGroup,
};

use super::PersistentPlayer;

/// A route that was navigated into rather than selected from the nav bar.
///
/// These used to render a second `Header` of their own inside the body, which
/// stacked two top bars. The shell now owns the single bar and supplies the
/// back button, so the pages render only their content.
fn detail_title(route: &Route, app_state: AppState) -> Option<String> {
    match route {
        // Deliberately untitled: the channel page leads with the name and
        // avatar, and repeating it in the bar pushed the header actions off
        // the edge on a long name.
        Route::ChannelDetail { .. } => Some(String::new()),
        Route::PlaylistDetail { id } => Some(
            app_state
                .library()
                .playlists
                .iter()
                .find(|playlist| &playlist.id == id)
                .map(|playlist| playlist.name.clone())
                .unwrap_or_else(|| "Playlist".into()),
        ),
        Route::SettingsPage {} => Some("Settings".into()),
        Route::QueuePage {} => Some("Queue".into()),
        Route::HistoryPage {} => Some("History".into()),
        _ => None,
    }
}

/// The nav destination a cover route sits on top of.
///
/// Going back to it by navigating — rather than popping history — is what lets
/// the sheet animate down, because the transition is chosen from the route pair
/// and a raw `go_back` gives the library nothing to compare.
fn back_destination(route: &Route) -> Route {
    match route {
        Route::PlaylistDetail { .. } => Route::Playlists {},
        Route::ChannelDetail { .. } => Route::Subscriptions {},
        _ => Route::Feed {},
    }
}

/// The gold label: the nav destination the current route belongs to.
fn section_label(route: &Route) -> &'static str {
    match route {
        Route::Subscriptions {} => "Subscriptions",
        Route::Playlists {} | Route::PlaylistDetail { .. } => "Playlists",
        Route::Explore {} => "Search",
        _ => "Feed",
    }
}

#[component]
pub fn AppShell() -> Element {
    let route: Route = use_route();
    let app_state = use_context::<AppState>();


    let player_expanded = matches!(route, Route::VideoDetail { .. });
    let detail = detail_title(&route, app_state);
    let is_detail = detail.is_some();
    let title = detail.unwrap_or_default();
    // The watch page is the only sheet, so it is the only route that gives up
    // the tab bar — and only on compact layouts, where the bar is the bottom
    // strip the sheet slides over. Playlists, channels and settings are peers
    // that keep the nav exactly where it is.
    let is_cover = player_expanded;
    let back_route = route.clone();

    // Only the two browsing surfaces carry a segmented control.
    let toolbar = match route {
        Route::Feed {} => Some(rsx! {
            SegmentGroup { active: app_state.feed_filter_index,
                SegmentButton { index: 0, "All" }
                SegmentButton { index: 1, "Unwatched" }
                SegmentButton { index: 2, "Today" }
            }
        }),
        Route::Explore {} => Some(rsx! {
            SegmentGroup { active: app_state.explore_filter_index,
                SegmentButton { index: 0, "All" }
                SegmentButton { index: 1, "Videos" }
                SegmentButton { index: 2, "Channels" }
            }
        }),
        Route::ChannelDetail { .. } => Some(rsx! {
            SegmentGroup { active: app_state.channel_tab_index,
                SegmentButton { index: 0, "Videos" }
                SegmentButton { index: 1, "Shorts" }
                SegmentButton { index: 2, "Live" }
            }
        }),
        _ => None,
    };

    let start_button = if is_detail {
        rsx! {
            Button {
                style: ButtonStyle::Clear,
                aria_label: "Back".to_string(),
                class: "icon-button",
                onclick: move |_| {
                    // Popping keeps the two back affordances agreeing. Pushing
                    // the parent instead left the detail page ahead in history,
                    // so the browser's back button walked straight back into
                    // the page the user had just left. Falling back to a push
                    // covers arriving by deep link, where there is nothing to
                    // pop to.
                    let navigator = navigator();
                    if navigator.can_go_back() {
                        navigator.go_back();
                    } else {
                        let home = back_destination(&back_route);
                        spawn(async move { animated_navigate(home).await; });
                    }
                },
                ChevronLeft { size: 22 }
            }
        }
    } else {
        rsx! {
            div { class: "brand-lockup",
                span { class: "brand-mark", "T" }
                span { class: "header-eyebrow", "{section_label(&route)}" }
            }
        }
    };

    rsx! {
        // The shell is the base the sheet covers. The cover marker must never
        // sit on an ancestor of this: a named descendant is lifted out of its
        // ancestor's snapshot, so an outer cover would capture everything
        // except the content — an empty background sliding around.
        Navbar { class: if is_cover { "tawny-shell tawny-shell-cover route-transition-base" } else { "tawny-shell route-transition-base" },
            // No top bar on the player: minimize, back, and swipe-down all
            // leave the screen, so a bar would only steal height from the video.
            if !player_expanded {
                Header {
                    title,
                    class: "tawny-header",
                    start_button,
                    end_button: rsx! {
                        div { class: "header-actions",
                            Button {
                                style: ButtonStyle::Clear,
                                aria_label: format!("Queue, {} videos", app_state.library().queue.len()),
                                class: "icon-button",
                                onclick: move |_| { spawn(async move { animated_navigate(Route::QueuePage {}).await; }); },
                                ListVideo { size: 19 }
                            }
                            Button {
                                style: ButtonStyle::Clear,
                                aria_label: "History".to_string(),
                                class: "icon-button header-history-button",
                                onclick: move |_| { spawn(async move { animated_navigate(Route::HistoryPage {}).await; }); },
                                History { size: 19 }
                            }
                            Button {
                                style: ButtonStyle::Clear,
                                aria_label: "Settings".to_string(),
                                class: "icon-button",
                                onclick: move |_| { spawn(async move { animated_navigate(Route::SettingsPage {}).await; }); },
                                Settings { size: 20 }
                            }
                        }
                    },
                    toolbar,
                }
            }
            Body { padding: false,
                PersistentPlayer { expanded: player_expanded }
                Outlet::<Route> {}
            }
            // Always rendered: the desktop rail is permanent chrome, and only
            // the compact bottom bar gets out of a sheet's way. Hiding it in
            // CSS keeps that a layout decision rather than a routing one.
            NavbarTabBar { aria_label: "Primary navigation".to_string(),
                    NavbarTab {
                        label: "Feed".to_string(),
                        selected: matches!(route, Route::Feed {}),
                        icon: rsx! { House { size: 20 } },
                        onclick: move |_| { spawn(async move { animated_navigate(Route::Feed {}).await; }); },
                    }
                    NavbarTab {
                        label: "Playlists".to_string(),
                        selected: matches!(route, Route::Playlists {} | Route::PlaylistDetail { .. }),
                        icon: rsx! { ListIcon { size: 20 } },
                        onclick: move |_| { spawn(async move { animated_navigate(Route::Playlists {}).await; }); },
                    }
                    NavbarTab {
                        label: "Search".to_string(),
                        selected: matches!(route, Route::Explore {}),
                        icon: rsx! { Search { size: 20 } },
                        onclick: move |_| { spawn(async move { animated_navigate(Route::Explore {}).await; }); },
                    }
                    NavbarTab {
                        label: "Subscriptions".to_string(),
                        selected: matches!(route, Route::Subscriptions {} | Route::ChannelDetail { .. }),
                        icon: rsx! { Users { size: 20 } },
                        onclick: move |_| { spawn(async move { animated_navigate(Route::Subscriptions {}).await; }); },
                    }
            }
        }
    }
}
