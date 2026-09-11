use crate::{app::Route, state::AppState};
use dioxus::prelude::*;
use dioxus_icons::lucide::{
    ChevronLeft, History, House, List as ListIcon, ListVideo, Play, Search, Settings, Users,
};
#[cfg(target_os = "android")]
use g3_native_plugins::NativePlugins;
use g3_route_transitions::{
    animated_go_back, animated_navigate, use_native_back_navigation_with_interception,
};
use g3_ui::{Button, ButtonStyle, Header, Navbar, NavbarTab, NavbarTabBar};

use super::PersistentPlayer;

/// The gold label: the nav destination the current route belongs to.
fn section_label(route: &Route) -> &'static str {
    match route {
        Route::Subscriptions {} => "Subscriptions",
        Route::Playlists {} | Route::PlaylistDetail { .. } => "Playlists",
        Route::Explore {} => "Search",
        _ => "Feed",
    }
}

/// Installs the shared native-Back/router integration. Tawny keeps only its
/// app-specific overlay priority on the standard DOM events; the library owns
/// the actual Dioxus pop and transition handshake.
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

            };

            window.addEventListener('g3nativeback', onNativeBack);
            window[stateKey] = {
                dispose() {
                    window.removeEventListener('g3nativeback', onNativeBack);
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
/// The bar every page renders for itself.
///
/// The shell used to own a single header and swap its title per route. That bar
/// belonged to the shell, so it stayed put while the page slid out from under
/// it, which read as the content sliding behind fixed furniture. Giving each
/// page its own header lets the bar travel with the page it describes.
#[component]
pub fn PageHeader(
    /// Blank for pages that lead with their own title block, such as a channel.
    title: Option<String>,
    /// Supplied by pages that were navigated into: renders a back affordance in
    /// place of the brand lockup, and is the destination used when the page was
    /// deep-linked and has no history to pop.
    back_to: Option<Route>,
    /// Optional segmented control rendered under the bar.
    toolbar: Option<Element>,
    /// Set false on a page that is somewhere the viewer went on purpose and is
    /// working inside. Queue, History and Settings are app-wide errands, and
    /// offering them from the top of a playlist puts three ways to leave next
    /// to the name of the thing that was opened.
    global_actions: Option<bool>,
) -> Element {
    let route: Route = use_route();
    let app_state = use_context::<AppState>();
    // All three are one group of peers presented over the page that launched
    // them. Once any of them is up, the group has done its job - offering it
    // again from inside itself is just chrome the sheet has to carry.
    let show_global_actions = global_actions.unwrap_or(true) && !is_auxiliary_route(&route);

    let start_button = match back_to {
        Some(home) => rsx! {
            Button {
                style: ButtonStyle::Clear,
                aria_label: "Back".to_string(),
                class: "icon-button",
                onclick: move |_| {
                    // Popping keeps the two back affordances agreeing. Pushing
                    // the parent instead left the detail page ahead in history,
                    // so the browser's back button walked straight back into
                    // the page the user had just left. The route is only a
                    // fallback, for arriving by deep link with nothing to pop.
                    let home = home.clone();
                    spawn(async move { animated_go_back(home).await; });
                },
                ChevronLeft { size: 22 }
            }
        },
        None => rsx! {
            div { class: "brand-lockup",
                span { class: "brand-mark", Play { size: 16, fill: "currentColor" } }
                span { class: "header-eyebrow", "{section_label(&route)}" }
            }
        },
    };

    rsx! {
        Header {
            title: title.unwrap_or_default(),
            class: "tawny-header",
            start_button,
            end_button: rsx! {
                div { class: "header-actions",
                    if show_global_actions {
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
                }
            },
            toolbar,
        }
    }
}

#[component]
pub fn AppShell() -> Element {
    let route: Route = use_route();
    let app_state = use_context::<AppState>();

    let player_expanded = matches!(route, Route::VideoDetail { .. });
    let is_auxiliary = is_auxiliary_route(&route);
    // Queue, History and Settings are sheets presented over whatever launched
    // them, and the mini bar belongs to the page underneath. Leaving it on top
    // covers the bottom of a sheet that is already scrolling under it.
    let has_mini_player = app_state.active_video().is_some() && !player_expanded && !is_auxiliary;
    let mut shell_class = "tawny-shell".to_string();
    if player_expanded {
        shell_class.push_str(" tawny-shell-cover");
    }
    if has_mini_player {
        shell_class.push_str(" has-mini-player");
    }
    if is_auxiliary {
        shell_class.push_str(" tawny-shell-auxiliary");
    }

    rsx! {
        NativeMediaCoordinator {}
        // The navbar is the base a sheet covers, and g3-ui marks it as such.
        // Nothing may wrap it in another snapshot marker: a named descendant is
        // lifted out of its ancestor, so an outer full-page marker ends up
        // naming a region that contains only the lifted base, and paints as a
        // bare background behind the sheet.
        NativeBackCoordinator {}
        Navbar { class: shell_class,
            // Mounted here rather than inside a page so playback survives
            // navigation. Minimized it is fixed and out of flow; expanded it is
            // in flow above the watch page's own body.
            PersistentPlayer { expanded: player_expanded }
            Outlet::<Route> {}
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

/// Queue, History, and Settings: reached from the header rather than a tab, and
/// presented as a sheet over whatever launched them.
fn is_auxiliary_route(route: &Route) -> bool {
    matches!(
        route,
        Route::SettingsPage {} | Route::QueuePage {} | Route::HistoryPage {}
    )
}
