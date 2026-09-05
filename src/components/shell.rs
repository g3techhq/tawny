use crate::{app::Route, state::AppState};
use dioxus::prelude::*;
use dioxus_icons::lucide::{
    ChevronLeft, History, House, List as ListIcon, ListVideo, Play, Search, Settings, Users,
};
#[cfg(target_os = "android")]
use g3_native_plugins::NativePlugins;
use g3_route_transitions::{
    RouteTransitionPage, animated_go_back, animated_navigate,
    use_native_back_navigation_with_interception,
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

/// The parent route used when a deep-linked page has no history to pop.
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

fn scroll_section(route: &Route) -> &'static str {
    match route {
        Route::Playlists {} | Route::PlaylistDetail { .. } => "playlists",
        Route::Explore {} => "search",
        Route::Subscriptions {} | Route::ChannelDetail { .. } => "subscriptions",
        Route::SettingsPage {} => "settings",
        Route::QueuePage {} => "queue",
        Route::HistoryPage {} => "history",
        Route::VideoDetail { .. } => "player",
        _ => "feed",
    }
}

async fn remember_section_scroll(section: &'static str, remember_return: bool) {
    let section = serde_json::to_string(section).unwrap_or_else(|_| "\"feed\"".into());
    let remember_script = format!(
        r#"
        const scroller = document.querySelector('.g3-body-content');
        window.__tawnySectionScrollPositions ||= {{}};
        if (scroller) window.__tawnySectionScrollPositions[{section}] = scroller.scrollTop;
        if ({remember_return}) {{
            window.__tawnyAuxiliaryReturnSections ||= [];
            window.__tawnyAuxiliaryReturnSections.push({section});
        }}
        dioxus.send(true);
        "#
    );
    let mut remember = document::eval(&remember_script);
    let _ = remember.recv::<bool>().await;
}

async fn restore_section_scroll(section: &str) {
    let section = serde_json::to_string(section).unwrap_or_else(|_| "\"feed\"".into());
    let restore_script = format!(
        r#"
        const scroller = document.querySelector('.g3-body-content');
        const top = window.__tawnySectionScrollPositions?.[{section}] ?? 0;
        const restore = () => scroller?.scrollTo({{ top, left: 0, behavior: 'instant' }});
        requestAnimationFrame(() => requestAnimationFrame(restore));
        dioxus.send(true);
        "#
    );
    let mut restore = document::eval(&restore_script);
    let _ = restore.recv::<bool>().await;
}

/// Bottom tabs are independent scroll surfaces even though g3-ui deliberately
/// reuses one body scroller. Remember the surface we are leaving and restore
/// the destination after its route transition has installed the new content.
fn navigate_with_scroll(from: &'static str, route: Route, remember_return: bool) {
    let to = scroll_section(&route);
    spawn(async move {
        remember_section_scroll(from, remember_return).await;
        animated_navigate(route).await;
        restore_section_scroll(to).await;
    });
}

fn navigate_to_section(from: &'static str, route: Route) {
    navigate_with_scroll(from, route, false);
}

fn navigate_to_auxiliary(from: &'static str, route: Route) {
    navigate_with_scroll(from, route, true);
}

fn navigate_back_from_auxiliary(from: &'static str, fallback: Route) {
    let fallback_section = scroll_section(&fallback);
    spawn(async move {
        remember_section_scroll(from, false).await;
        let mut return_section = document::eval(
            r#"
            const sections = window.__tawnyAuxiliaryReturnSections || [];
            dioxus.send(sections.pop() || '');
            "#,
        );
        let return_section = return_section
            .recv::<String>()
            .await
            .ok()
            .filter(|section| !section.is_empty())
            .unwrap_or_else(|| fallback_section.to_string());
        animated_go_back(fallback).await;
        restore_section_scroll(&return_section).await;
    });
}

/// Installs the shared native-Back/router integration. Tawny keeps only its
/// app-specific overlay priority and scroll restoration on the standard DOM
/// events; the library owns the actual Dioxus pop and transition handshake.
#[component]
fn NativeBackCoordinator() -> Element {
    let app_state = use_context::<AppState>();
    let overlay_open = (app_state.playlist_picker_open)()
        || (app_state.video_actions_open)()
        || (app_state.share_open)()
        || (app_state.chapters_sheet_open)();
    use_native_back_navigation_with_interception::<Route>(overlay_open);

    #[cfg(target_os = "android")]
    let _native_back_behavior = use_hook(|| {
        document::eval(
            r#"
            const stateKey = Symbol.for('tawny.native-back-ui');
            window[stateKey]?.dispose?.();

            const onNativeBack = async (event) => {
                if (document.fullscreenElement || document.webkitFullscreenElement) {
                    event.preventDefault();
                    if (window.__tawnyFullscreenBackPending) return;
                    window.__tawnyFullscreenBackPending = true;
                    document.querySelector('[data-player-native-orientation-unlock]')?.click();
                    const exit = document.exitFullscreen || document.webkitExitFullscreen;
                    if (exit) {
                        try { await exit.call(document); } catch (_) {}
                    }
                    await new Promise((resolve) => requestAnimationFrame(resolve));
                    document.querySelector('[data-player-minimize]')?.click();
                    setTimeout(() => { window.__tawnyFullscreenBackPending = false; }, 350);
                    return;
                }

                const options = document.querySelector('[data-player-options-menu]:not([hidden])');
                if (options) {
                    event.preventDefault();
                    document.querySelector('[data-player-action="settings"]')?.click();
                    return;
                }

                const sheet = document.querySelector('.g3-sheet-backdrop-open');
                if (sheet) {
                    event.preventDefault();
                    sheet.click();
                    return;
                }

                const modal = document.querySelector('.g3-modal-overlay[data-state="open"]');
                if (modal) {
                    event.preventDefault();
                    modal.click();
                    return;
                }

                const auxiliary = document.querySelector('.tawny-shell-auxiliary');
                if (auxiliary) {
                    const title = (document.querySelector('.g3-header-title-text')?.textContent || '')
                        .trim()
                        .toLowerCase();
                    const section = ['settings', 'queue', 'history'].includes(title) ? title : 'feed';
                    const scroller = document.querySelector('.g3-body-content');
                    window.__tawnySectionScrollPositions ||= {};
                    if (scroller) window.__tawnySectionScrollPositions[section] = scroller.scrollTop;
                    const returns = window.__tawnyAuxiliaryReturnSections || [];
                    window.__tawnyPendingNativeBackScrollSection = returns.pop() || 'feed';
                }
            };

            const onTransitionEnd = () => {
                const section = window.__tawnyPendingNativeBackScrollSection;
                delete window.__tawnyPendingNativeBackScrollSection;
                if (!section) return;

                const scroller = document.querySelector('.g3-body-content');
                const top = window.__tawnySectionScrollPositions?.[section] ?? 0;
                requestAnimationFrame(() => requestAnimationFrame(() => {
                    scroller?.scrollTo({ top, left: 0, behavior: 'instant' });
                }));
            };

            window.addEventListener('g3nativeback', onNativeBack);
            window.addEventListener('g3routebacktransitionend', onTransitionEnd);
            window[stateKey] = {
                dispose() {
                    window.removeEventListener('g3nativeback', onNativeBack);
                    window.removeEventListener('g3routebacktransitionend', onTransitionEnd);
                },
            };
            "#,
        )
    });

    rsx! {}
}

#[cfg(target_os = "android")]
#[component]
fn NativeMediaCoordinator() -> Element {
    let mut plugins = use_context::<NativePlugins>();
    use_hook(move || {
        let _ = plugins.media.write().prepare();
    });
    rsx! {}
}

#[cfg(not(target_os = "android"))]
#[component]
fn NativeMediaCoordinator() -> Element {
    rsx! {}
}

#[component]
pub fn AppShell() -> Element {
    let route: Route = use_route();
    let app_state = use_context::<AppState>();

    let player_expanded = matches!(route, Route::VideoDetail { .. });
    let has_mini_player = app_state.active_video().is_some() && !player_expanded;
    let is_auxiliary = matches!(
        route,
        Route::SettingsPage {} | Route::QueuePage {} | Route::HistoryPage {}
    );
    let detail = detail_title(&route, app_state);
    let is_detail = detail.is_some();
    let title = detail.unwrap_or_default();
    // The watch page is the only sheet, so it is the only route that gives up
    // the compact bottom bar. Detail and auxiliary pages keep the shell but
    // use pushed-page motion when opened or dismissed.
    let is_cover = player_expanded;
    let back_route = route.clone();
    let current_scroll_section = scroll_section(&route);
    let mut shell_class = "tawny-shell route-transition-base".to_string();
    if is_cover {
        shell_class.push_str(" tawny-shell-cover");
    }
    if has_mini_player {
        shell_class.push_str(" has-mini-player");
    }
    if is_auxiliary {
        shell_class.push_str(" tawny-shell-auxiliary");
    }

    // Only the two browsing surfaces carry a segmented control.
    let toolbar = match route {
        Route::Feed {} => Some(rsx! {
            SegmentGroup { active: app_state.feed_filter_index,
                SegmentButton { index: 0, "All" }
                SegmentButton { index: 1, "Videos" }
                SegmentButton { index: 2, "Shorts" }
                SegmentButton { index: 3, "Live" }
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
                SegmentButton { index: 0, "All" }
                SegmentButton { index: 1, "Videos" }
                SegmentButton { index: 2, "Shorts" }
                SegmentButton { index: 3, "Live" }
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
                    if is_auxiliary {
                        navigate_back_from_auxiliary(
                            current_scroll_section,
                            back_destination(&back_route),
                        );
                        return;
                    }
                    // Popping keeps the two back affordances agreeing. Pushing
                    // the parent instead left the detail page ahead in history,
                    // so the browser's back button walked straight back into
                    // the page the user had just left. Falling back to a push
                    // covers arriving by deep link, where there is nothing to
                    // pop to.
                    let home = back_destination(&back_route);
                    spawn(async move { animated_go_back(home).await; });
                },
                ChevronLeft { size: 22 }
            }
        }
    } else {
        rsx! {
            div { class: "brand-lockup",
                span { class: "brand-mark", Play { size: 16, fill: "currentColor" } }
                span { class: "header-eyebrow", "{section_label(&route)}" }
            }
        }
    };

    rsx! {
        NativeMediaCoordinator {}
        // The shell is the base the sheet covers. The cover marker must never
        // sit on an ancestor of this: a named descendant is lifted out of its
        // ancestor's snapshot, so an outer cover would capture everything
        // except the content — an empty background sliding around.
        NativeBackCoordinator {}
        RouteTransitionPage {
            Navbar { class: shell_class,
                // No top bar on the player: minimize, back, and swipe-down all
                // leave the screen, so a bar would only steal height from the video.
                if !player_expanded {
                    Header {
                        title,
                        class: "tawny-header",
                        start_button,
                        end_button: rsx! {
                            div { class: "header-actions",
                                if !matches!(route, Route::QueuePage {}) {
                                    Button {
                                        style: ButtonStyle::Clear,
                                        aria_label: format!("Queue, {} videos", app_state.library().queue.len()),
                                        class: "icon-button",
                                        onclick: move |_| navigate_to_auxiliary(current_scroll_section, Route::QueuePage {}),
                                        ListVideo { size: 19 }
                                    }
                                }
                                if !matches!(route, Route::HistoryPage {}) {
                                    Button {
                                        style: ButtonStyle::Clear,
                                        aria_label: "History".to_string(),
                                        class: "icon-button header-history-button",
                                        onclick: move |_| navigate_to_auxiliary(current_scroll_section, Route::HistoryPage {}),
                                        History { size: 19 }
                                    }
                                }
                                if !matches!(route, Route::SettingsPage {}) {
                                    Button {
                                        style: ButtonStyle::Clear,
                                        aria_label: "Settings".to_string(),
                                        class: "icon-button",
                                        onclick: move |_| navigate_to_auxiliary(current_scroll_section, Route::SettingsPage {}),
                                        Settings { size: 20 }
                                    }
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
                        onclick: move |_| navigate_to_section(current_scroll_section, Route::Feed {}),
                    }
                    NavbarTab {
                        label: "Playlists".to_string(),
                        selected: matches!(route, Route::Playlists {} | Route::PlaylistDetail { .. }),
                        icon: rsx! { ListIcon { size: 20 } },
                        onclick: move |_| navigate_to_section(current_scroll_section, Route::Playlists {}),
                    }
                    NavbarTab {
                        label: "Search".to_string(),
                        selected: matches!(route, Route::Explore {}),
                        icon: rsx! { Search { size: 20 } },
                        onclick: move |_| navigate_to_section(current_scroll_section, Route::Explore {}),
                    }
                    NavbarTab {
                        label: "Subscriptions".to_string(),
                        selected: matches!(route, Route::Subscriptions {} | Route::ChannelDetail { .. }),
                        icon: rsx! { Users { size: 20 } },
                        onclick: move |_| navigate_to_section(current_scroll_section, Route::Subscriptions {}),
                    }
                }
            }
        }
    }
}
