use crate::{app::Route, state::AppState};
use dioxus::prelude::*;
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
        Route::ChannelDetail { id } => Some(
            app_state
                .library()
                .channels
                .iter()
                .find(|channel| &channel.id == id)
                .map(|channel| channel.name.clone())
                .unwrap_or_else(|| "Channel".into()),
        ),
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
    let navigator = use_navigator();

    let player_expanded = matches!(route, Route::VideoDetail { .. });
    let detail = detail_title(&route, app_state);
    let is_detail = detail.is_some();
    let title = detail.unwrap_or_default();

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
                onclick: move |_| { navigator.go_back(); },
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
        Navbar { class: if player_expanded { "tawny-shell tawny-shell-immersive" } else { "tawny-shell" },
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
                                onclick: move |_| { navigator.push(Route::QueuePage {}); },
                                ListVideo { size: 19 }
                            }
                            Button {
                                style: ButtonStyle::Clear,
                                aria_label: "History".to_string(),
                                class: "icon-button header-history-button",
                                onclick: move |_| { navigator.push(Route::HistoryPage {}); },
                                History { size: 19 }
                            }
                            Button {
                                style: ButtonStyle::Clear,
                                aria_label: "Settings".to_string(),
                                class: "icon-button",
                                onclick: move |_| { navigator.push(Route::SettingsPage {}); },
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
            // The player owns the whole screen; its own controls and gestures
            // are the way out, so the tab bar steps aside there.
            if !player_expanded {
                NavbarTabBar { aria_label: "Primary navigation".to_string(),
                    NavbarTab {
                        label: "Feed".to_string(),
                        selected: matches!(route, Route::Feed {}),
                        icon: rsx! { House { size: 20 } },
                        onclick: move |_| { navigator.push(Route::Feed {}); },
                    }
                    NavbarTab {
                        label: "Playlists".to_string(),
                        selected: matches!(route, Route::Playlists {}),
                        icon: rsx! { ListIcon { size: 20 } },
                        onclick: move |_| { navigator.push(Route::Playlists {}); },
                    }
                    NavbarTab {
                        label: "Search".to_string(),
                        selected: matches!(route, Route::Explore {}),
                        icon: rsx! { Search { size: 20 } },
                        onclick: move |_| { navigator.push(Route::Explore {}); },
                    }
                    NavbarTab {
                        label: "Subscriptions".to_string(),
                        selected: matches!(route, Route::Subscriptions {}),
                        icon: rsx! { Users { size: 20 } },
                        onclick: move |_| { navigator.push(Route::Subscriptions {}); },
                    }
                }
            }
        }
    }
}
