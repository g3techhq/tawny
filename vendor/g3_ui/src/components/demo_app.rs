//! A small end-to-end demo that composes the full g3_ui component set into one app.
//! Kitchen-sink reference app for the playground gallery.
//!
//! This is a component-owned playground demo (like every other component), but instead
//! of exercising a single component it wires the whole library together into a responsive
//! app shell so the gallery has a "kitchen sink" reference to open first.

#[cfg(feature = "playground")]
use dioxus::prelude::*;
#[cfg(feature = "playground")]
use dioxus_icons::lucide::{
    Activity, Bell, CalendarDays, ChevronRight, CircleUserRound, Heart, House, LogOut, MapPin,
    Menu, Search, Settings, Star, Trophy, Zap,
};

#[cfg(feature = "playground")]
#[component]
pub fn DemoAppPlaygroundDemo() -> Element {
    let mode = crate::use_component_mode(None);

    // Which bottom-tab screen is showing.
    let mut tab = use_signal(|| 0_usize);
    // Header toolbar segment (Live / Upcoming) on the Rounds screen.
    let toolbar_segment = use_signal(|| 0_usize);

    // Overlay + feedback state. Every overlay is rendered once at the app root
    // (below) so its fixed/absolute backdrop resolves against the app frame and
    // covers the header, instead of being clipped inside the scrolling body.
    let mut toast_open = use_signal(|| false);
    let mut menu_open = use_signal(|| false);
    let mut filter_sheet_open = use_signal(|| false);
    let mut leave_modal_open = use_signal(|| false);
    let mut confirm_open = use_signal(|| false);
    let info_open = use_signal(|| false);
    let mut fab_open = use_signal(|| false);

    // Refresher state on the Rounds screen.
    let refreshing = use_signal(|| false);

    // Discover "More filters" bottom-sheet state — real toggles so the sheet
    // shows what is selected and "Apply" has something to commit.
    let filter_nearby = use_signal(|| true);
    let filter_friends = use_signal(|| false);
    let filter_weekend = use_signal(|| false);

    // Discover-screen form state.
    let accordion = use_signal(|| vec!["round".to_string()]);
    let course = use_signal(|| "Pebble Creek".to_string());
    let handicap = use_signal(|| "12".to_string());
    let tee = use_signal(|| "White".to_string());
    let format_segment = use_signal(|| 0_usize);
    let walking = use_signal(|| true);
    let scoring = use_signal(|| "skins".to_string());

    let screen = match tab() {
        1 => rsx! {
            DiscoverScreen {
                accordion,
                course,
                handicap,
                tee,
                format_segment,
                walking,
                scoring,
                filter_sheet_open,
            }
        },
        2 => rsx! {
            ProfileScreen { leave_modal_open, confirm_open }
        },
        3 => rsx! {
            ActivityScreen {}
        },
        _ => rsx! {
            RoundsScreen {
                toolbar_segment,
                refreshing,
                toast_open,
                info_open,
            }
        },
    };

    let title = match tab() {
        1 => "Discover",
        2 => "Profile",
        3 => "Activity",
        _ => "Fairway",
    };

    rsx! {
        crate::PlaygroundDemoFrame { app: false,
            crate::AppWrapper { mode, class: "g3-playground-device-app",
                crate::Navbar {
                    crate::Header {
                        title: title.to_string(),
                        start_button: rsx! {
                            crate::Button {
                                style: crate::ButtonStyle::Clear,
                                size: crate::ButtonSize::Sm,
                                aria_label: "Open menu".to_string(),
                                onclick: move |_| menu_open.set(true),
                                Menu { size: 22, class: "fill-none" }
                            }
                        },
                        end_button: rsx! {
                            crate::Button {
                                style: crate::ButtonStyle::Clear,
                                size: crate::ButtonSize::Sm,
                                aria_label: "Notifications".to_string(),
                                onclick: move |_| toast_open.set(true),
                                Bell { size: 20, class: "fill-none" }
                            }
                        },
                        toolbar: (tab() == 0).then(|| rsx! {
                            crate::SegmentGroup { active: toolbar_segment,
                                crate::SegmentButton { index: 0, "Live" }
                                crate::SegmentButton { index: 1, "Upcoming" }
                            }
                        }),
                    }

                    crate::Body {
                        has_footer_space: false,
                        fab: (tab() == 0).then_some(rsx! {
                            crate::Fab {
                                vertical: crate::FabVertical::Bottom,
                                horizontal: crate::FabHorizontal::End,
                                class: "g3-demo-fab",
                                crate::FabButton { onclick: move |_| fab_open.toggle(),
                                    Zap { size: 22, class: "fill-none" }
                                }
                                crate::FabList { activated: fab_open(),
                                    crate::FabButton { onclick: move |_| {}, size: crate::FabSize::Small,
                                        Star { size: 18, class: "fill-none" }
                                    }
                                    crate::FabButton { onclick: move |_| {}, size: crate::FabSize::Small,
                                        MapPin { size: 18, class: "fill-none" }
                                    }
                                }
                            }
                        }),
                        {screen}
                    }

                    crate::NavbarTabBar {
                        crate::NavbarTab {
                            label: "Rounds".to_string(),
                            selected: tab() == 0,
                            icon: rsx! {
                                House { size: 20, class: "fill-none" }
                            },
                            onclick: move |_| tab.set(0),
                        }
                        crate::NavbarTab {
                            label: "Discover".to_string(),
                            selected: tab() == 1,
                            icon: rsx! {
                                Search { size: 20, class: "fill-none" }
                            },
                            onclick: move |_| tab.set(1),
                        }
                        crate::NavbarTab {
                            label: "Profile".to_string(),
                            selected: tab() == 2,
                            icon: rsx! {
                                CircleUserRound { size: 20, class: "fill-none" }
                            },
                            onclick: move |_| tab.set(2),
                        }
                        crate::NavbarTab {
                            label: "Activity".to_string(),
                            selected: tab() == 3,
                            icon: rsx! {
                                Activity { size: 20, class: "fill-none" }
                            },
                            onclick: move |_| tab.set(3),
                        }
                    }
                }

                // Overlays live at the app root so they float above every screen.
                crate::Toast {
                    open: toast_open,
                    message: "You have 3 new invites".to_string(),
                    color: crate::StatusColor::Accent,
                    position: crate::ToastPosition::Bottom,
                    duration_ms: 2500,
                }
                crate::Modal {
                    open: leave_modal_open,
                    title: "Round details".to_string(),
                    description: rsx! { "Started 42 minutes ago at Pebble Creek." },
                    actions: rsx! {
                        crate::Button {
                            style: crate::ButtonStyle::Clear,
                            onclick: move |_| leave_modal_open.set(false),
                            "Close"
                        }
                    },
                    crate::List { lines: crate::ListLines::Full,
                        crate::Item {
                            label: "Front nine".to_string(),
                            metadata: "+2".to_string(),
                        }
                        crate::Item {
                            label: "Back nine".to_string(),
                            metadata: "E".to_string(),
                        }
                    }
                }
                crate::ConfirmModal {
                    open: confirm_open,
                    title: "Leave this round?".to_string(),
                    description: rsx! { "Your scores are saved. You can rejoin any time." },
                    confirm_text: "Leave".to_string(),
                    on_confirm: move |_| confirm_open.set(false),
                }

                // Left side-menu opened from the header hamburger button.
                crate::Sheet {
                    is_open: menu_open,
                    placement: crate::SheetPlacement::Left,
                    div { class: "g3-demo-menu",
                        div { class: "g3-demo-menu-head",
                            crate::Avatar { fallback: "MW" }
                            span { class: "g3-demo-menu-name", "Matthew W." }
                        }
                        crate::List { lines: crate::ListLines::Full,
                            crate::Item {
                                kind: crate::ItemKind::Button,
                                label: "Rounds".to_string(),
                                start: rsx! {
                                    House { size: 20, class: "fill-none" }
                                },
                                onclick: move |_| {
                                    tab.set(0);
                                    menu_open.set(false);
                                },
                            }
                            crate::Item {
                                kind: crate::ItemKind::Button,
                                label: "Discover".to_string(),
                                start: rsx! {
                                    Search { size: 20, class: "fill-none" }
                                },
                                onclick: move |_| {
                                    tab.set(1);
                                    menu_open.set(false);
                                },
                            }
                            crate::Item {
                                kind: crate::ItemKind::Button,
                                label: "Profile".to_string(),
                                start: rsx! {
                                    CircleUserRound { size: 20, class: "fill-none" }
                                },
                                onclick: move |_| {
                                    tab.set(2);
                                    menu_open.set(false);
                                },
                            }
                            crate::Item {
                                kind: crate::ItemKind::Button,
                                label: "Sign out".to_string(),
                                start: rsx! {
                                    LogOut { size: 20, class: "fill-none" }
                                },
                                onclick: move |_| menu_open.set(false),
                            }
                        }
                    }
                }

                // Advanced filters bottom sheet (opened from the Discover screen).
                crate::Sheet {
                    is_open: filter_sheet_open,
                    placement: crate::SheetPlacement::Bottom,
                    div { class: "g3-demo-sheet-body",
                        h3 { "Filters" }
                        crate::List { inset: true, lines: crate::ListLines::Inset,
                            crate::Item {
                                start: rsx! {
                                    MapPin { size: 20, class: "fill-none" }
                                },
                                label: "Nearby courses".to_string(),
                                description: "Within 25 miles".to_string(),
                                end: rsx! {
                                    crate::Toggle { checked: filter_nearby }
                                },
                            }
                            crate::Item {
                                start: rsx! {
                                    CircleUserRound { size: 20, class: "fill-none" }
                                },
                                label: "Friends only".to_string(),
                                description: "Rounds with people you follow".to_string(),
                                end: rsx! {
                                    crate::Toggle { checked: filter_friends }
                                },
                            }
                            crate::Item {
                                start: rsx! {
                                    CalendarDays { size: 20, class: "fill-none" }
                                },
                                label: "This weekend".to_string(),
                                description: "Sat and Sun tee times".to_string(),
                                end: rsx! {
                                    crate::Toggle { checked: filter_weekend }
                                },
                            }
                        }
                        crate::Button {
                            expand: true,
                            onclick: move |_| filter_sheet_open.set(false),
                            {
                                let count = filter_nearby() as u8 + filter_friends() as u8
                                    + filter_weekend() as u8;
                                format!("Apply ({count})")
                            }
                        }
                    }
                }

                // Scoring explainer opened from the Live players info button.
                crate::Sheet {
                    is_open: info_open,
                    placement: crate::SheetPlacement::Bottom,
                    div { class: "g3-demo-info",
                        h3 { class: "g3-demo-info-title", "How scoring works" }
                        p { class: "g3-demo-info-lead",
                            "Standings update after every hole, relative to par."
                        }
                        div { class: "g3-demo-info-row",
                            crate::Badge { color: crate::StatusColor::Success, "-3" }
                            div { class: "g3-demo-info-text",
                                span { class: "g3-demo-info-term", "Under par" }
                                span { class: "g3-demo-info-desc", "Fewer strokes than the course par." }
                            }
                        }
                        div { class: "g3-demo-info-row",
                            crate::Badge { "E" }
                            div { class: "g3-demo-info-text",
                                span { class: "g3-demo-info-term", "Even" }
                                span { class: "g3-demo-info-desc", "Exactly on par for holes played." }
                            }
                        }
                        div { class: "g3-demo-info-row",
                            crate::Badge { color: crate::StatusColor::Warning, "+1" }
                            div { class: "g3-demo-info-text",
                                span { class: "g3-demo-info-term", "Over par" }
                                span { class: "g3-demo-info-desc", "More strokes than the course par." }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Tab 1 — a scrollable feed showcasing cards, lists, primitives, feedback + refresh.
#[cfg(feature = "playground")]
#[component]
fn RoundsScreen(
    toolbar_segment: Signal<usize>,
    refreshing: Signal<bool>,
    toast_open: Signal<bool>,
    info_open: Signal<bool>,
) -> Element {
    let mut refreshing = refreshing;
    let mut info_open = info_open;
    let live = toolbar_segment() == 0;

    rsx! {
        crate::Refresher {
            refreshing: refreshing(),
            can_refresh: true,
            on_refresh: move |_| {
                refreshing.set(true);
                spawn(async move {
                    dioxus_sdk_time::sleep(std::time::Duration::from_millis(700)).await;
                    refreshing.set(false);
                });
            },

            crate::Card {
                title: "Today's round",
                right_slot: crate::RightSlot::Text("Par 72".to_string()),
                div { class: "g3-demo-progress-row",
                    span { "Thru 12 holes" }
                    crate::Progress { value: 66.0 }
                }
                div { class: "g3-demo-chip-row",
                    crate::Chip {
                        selected: true,
                        onclick: move |_| {},
                        start: rsx! {
                            Trophy { size: 14, class: "fill-none" }
                        },
                        "Match play"
                    }
                    crate::Chip { onclick: move |_| {}, "Skins" }
                    crate::Chip { onclick: move |_| {}, "Walking" }
                }
            }

            div { class: "g3-demo-section-head",
                span { class: "g3-demo-section-title",
                    if live {
                        "Live players"
                    } else {
                        "Upcoming tee times"
                    }
                }
                crate::InfoButton {
                    onclick: move |_| info_open.set(true),
                    aria_label: "About scoring".to_string(),
                }
            }

            crate::List { inset: true, lines: crate::ListLines::Inset,
                crate::Item {
                    start: rsx! {
                        crate::Avatar { fallback: "JD" }
                    },
                    label: "Jordan Diaz".to_string(),
                    description: "3 under · leader".to_string(),
                    end: rsx! {
                        crate::Badge { color: crate::StatusColor::Success, "-3" }
                    },
                    detail: crate::ItemDetail::Show,
                }
                crate::Item {
                    start: rsx! {
                        crate::Avatar { fallback: "SM" }
                    },
                    label: "Sam Meyer".to_string(),
                    description: "1 over".to_string(),
                    end: rsx! {
                        crate::Badge { color: crate::StatusColor::Warning, "+1" }
                    },
                    detail: crate::ItemDetail::Show,
                }
                crate::Item {
                    start: rsx! {
                        crate::Avatar { fallback: "AL" }
                    },
                    label: "Alex Lin".to_string(),
                    description: "even".to_string(),
                    end: rsx! {
                        crate::Badge { "E" }
                    },
                    detail: crate::ItemDetail::Show,
                }
            }

            crate::Line { class: "g3-demo-separator" }

            div { class: "g3-demo-button-row",
                crate::Button { onclick: move |_| toast_open.set(true), "Invite" }
                crate::Button { style: crate::ButtonStyle::Outline, onclick: move |_| {}, "Share" }
            }
        }
    }
}

/// Tab 2 — a settings-style form built from disclosures, inputs and choice controls.
#[cfg(feature = "playground")]
#[component]
fn DiscoverScreen(
    accordion: Signal<Vec<String>>,
    course: Signal<String>,
    handicap: Signal<String>,
    tee: Signal<String>,
    format_segment: Signal<usize>,
    walking: Signal<bool>,
    scoring: Signal<String>,
    filter_sheet_open: Signal<bool>,
) -> Element {
    let mut filter_sheet_open = filter_sheet_open;
    let notify_scores = use_signal(|| true);
    let notify_cheers = use_signal(|| false);

    rsx! {
        crate::AccordionGroup { value: accordion,
            crate::AccordionItem {
                value: "round".to_string(),
                label: "Round setup".to_string(),
                description: "Course, handicap and tees".to_string(),
                crate::Field { label: "Course".to_string(), value: course }
                crate::Field {
                    label: "Handicap".to_string(),
                    value: handicap,
                    r#type: "number".to_string(),
                }
                div { class: "g3-demo-field-label",
                    span { "Tees" }
                }
                crate::Select {
                    value: tee,
                    options: vec![
                        crate::SelectOption::from("White"),
                        crate::SelectOption::from("Blue"),
                        crate::SelectOption::from("Gold"),
                    ],
                }
            }

            crate::AccordionItem {
                value: "format".to_string(),
                label: "Format".to_string(),
                description: "How the group scores".to_string(),
                crate::SegmentGroup { active: format_segment,
                    crate::SegmentButton { index: 0, "Stroke" }
                    crate::SegmentButton { index: 1, "Match" }
                    crate::SegmentButton { index: 2, "Stable" }
                }
                div { class: "g3-demo-radio-group",
                    crate::RadioGroup { value: scoring,
                        crate::Radio {
                            value: "skins".to_string(),
                            label: "Skins".to_string(),
                        }
                        crate::Radio {
                            value: "nassau".to_string(),
                            label: "Nassau".to_string(),
                        }
                        crate::Radio {
                            value: "none".to_string(),
                            label: "No side bets".to_string(),
                        }
                    }
                }
                crate::Checkbox { checked: walking, label: "Walking round".to_string() }
            }

            crate::AccordionItem {
                value: "notify".to_string(),
                label: "Notifications".to_string(),
                description: "Live scoring alerts".to_string(),
                crate::List { lines: crate::ListLines::Full,
                    crate::Item {
                        start: rsx! {
                            Bell { size: 20, class: "fill-none" }
                        },
                        label: "Score updates".to_string(),
                        end: rsx! {
                            crate::Toggle { checked: notify_scores }
                        },
                    }
                    crate::Item {
                        start: rsx! {
                            Heart { size: 20, class: "fill-none" }
                        },
                        label: "Cheers".to_string(),
                        end: rsx! {
                            crate::Toggle { checked: notify_cheers }
                        },
                    }
                }
            }
        }

        div { class: "g3-demo-button-row",
            crate::Button {
                style: crate::ButtonStyle::Neutral,
                expand: true,
                onclick: move |_| filter_sheet_open.set(true),
                start: rsx! {
                    Settings { size: 18, class: "fill-none" }
                },
                "More filters"
            }
        }
    }
}

/// Tab 3 — an identity/summary screen with avatar, stats, links and a loading state.
#[cfg(feature = "playground")]
#[component]
fn ProfileScreen(leave_modal_open: Signal<bool>, confirm_open: Signal<bool>) -> Element {
    let mut leave_modal_open = leave_modal_open;
    let mut confirm_open = confirm_open;

    rsx! {
        crate::Card {
            div { class: "g3-demo-profile-head",
                crate::Avatar { fallback: "MW", size: crate::AvatarSize::Lg }
                div { class: "g3-demo-profile-meta",
                    div { class: "g3-demo-profile-name",
                        span { "Matthew W." }
                        crate::Badge { color: crate::StatusColor::Accent, "Pro" }
                    }
                    div { class: "g3-demo-chip-row",
                        crate::Chip {
                            onclick: move |_| {},
                            start: rsx! {
                                Star { size: 14, class: "fill-none" }
                            },
                            "8.4 hcp"
                        }
                        crate::Chip { onclick: move |_| {}, "42 rounds" }
                    }
                }
            }
        }

        crate::List { inset: true,
            crate::Item {
                start: rsx! {
                    Trophy { size: 20, class: "fill-none" }
                },
                label: "Achievements".to_string(),
                detail: crate::ItemDetail::Show,
                kind: crate::ItemKind::Button,
                onclick: move |_| leave_modal_open.set(true),
            }
            crate::Item {
                start: rsx! {
                    CalendarDays { size: 20, class: "fill-none" }
                },
                label: "Round history".to_string(),
                metadata: "42".to_string(),
                detail: crate::ItemDetail::Show,
                kind: crate::ItemKind::Button,
                onclick: move |_| leave_modal_open.set(true),
            }
            crate::Item {
                start: rsx! {
                    Settings { size: 20, class: "fill-none" }
                },
                label: "Settings".to_string(),
                end: rsx! {
                    ChevronRight { size: 18, class: "fill-none" }
                },
                kind: crate::ItemKind::Button,
                onclick: move |_| {},
            }
        }

        div { class: "g3-demo-section-head",
            span { class: "g3-demo-section-title", "Syncing latest scores" }
        }

        crate::List { inset: true, lines: crate::ListLines::Inset,
            crate::Item {
                start: rsx! {
                    crate::Skeleton { shape: crate::SkeletonShape::Avatar }
                },
                children: rsx! {
                    crate::Skeleton { shape: crate::SkeletonShape::Row }
                },
            }
            crate::Item {
                start: rsx! {
                    crate::Skeleton { shape: crate::SkeletonShape::Avatar }
                },
                children: rsx! {
                    crate::Skeleton { shape: crate::SkeletonShape::Row }
                },
            }
            crate::Item {
                start: rsx! {
                    crate::Skeleton { shape: crate::SkeletonShape::Avatar }
                },
                children: rsx! {
                    crate::Skeleton { shape: crate::SkeletonShape::Row }
                },
            }
        }

        div { class: "g3-demo-button-row",
            crate::Button {
                style: crate::ButtonStyle::Outline,
                expand: true,
                onclick: move |_| confirm_open.set(true),
                "Leave round"
            }
        }
    }
}

/// Tab 4 — a dedicated loading screen showcasing spinner + skeleton placeholders.
#[cfg(feature = "playground")]
#[component]
fn ActivityScreen() -> Element {
    rsx! {
        crate::Spinner { center: true }
    }
}

crate::g3_playground! {
    name: "Demo App",
    description: "A complete responsive app shell that composes every g3_ui component together.",
    demo: DemoAppPlaygroundDemo,
    source: "src/components/demo_app.rs",
}
