use crate::{app::Route, models::Channel, state::AppState};
use dioxus::prelude::*;
use dioxus_icons::lucide::{Check, Plus, Trash2, Users};
use g3_route_transitions::animated_navigate;
use g3_ui::{
    Avatar, AvatarSize, Button, ButtonFill, ButtonSize, Card, Color, Content, EmptyState, Grid,
    GridColumns, InfiniteScroll, Input, Item, List, ListLines, ListVariant, Modal, Searchbar,
    Skeleton, SkeletonShape, Space, Stack, Text, TextTone,
};

use super::{PageHeader, use_after_first_paint};

/// How many subscribed channels the grid lays out before asking for more.
/// Each is a card with an avatar image, and a long subscription list built
/// them all in one render.
const CHANNEL_PAGE_SIZE: usize = 48;

/// One channel tile, used by both the subscribed grid and the suggestions.
#[component]
pub fn ChannelCard(channel: Channel) -> Element {
    let app_state = use_context::<AppState>();
    let toggle_channel = channel.clone();
    let open_channel_id = channel.id.clone();
    let is_subscribed = app_state.follows(&channel.id);
    let subscribers = channel.subscriber_count.trim().to_string();

    rsx! {
        Card {
            title: channel.name.clone(),
            subtitle: (!subscribers.is_empty()).then(|| format!("{subscribers} subscribers")),
            class: "h-full [&_.g3-card-title]:line-clamp-1",
            start: rsx! {
                Avatar { name: channel.name.clone(), src: channel.avatar_url.clone() }
            },
            onclick: move |_| { spawn(animated_navigate(Route::ChannelDetail { id: open_channel_id.clone() })); },
            end: rsx! {
                Button {
                    size: ButtonSize::Sm,
                    fill: if is_subscribed { ButtonFill::Outline } else { ButtonFill::Solid },
                    color: if is_subscribed { Color::Neutral } else { Color::Accent },
                    "aria-pressed": if is_subscribed { "true" } else { "false" },
                    start: is_subscribed.then(|| rsx! { Check { size: 15 } }),
                    onclick: move |_| {
                        let now_subscribed = app_state.toggle_subscription_for(&toggle_channel);
                        {
                            let message = if now_subscribed { "Subscribed" } else { "Unsubscribed" };
                            app_state.show_toast(message, Color::Neutral);
                        }
                    },
                    if is_subscribed { "Subscribed" } else { "Subscribe" }
                }
            },
        }
    }
}

/// "1 channel", "3 channels".
fn channel_count(count: usize) -> String {
    if count == 1 {
        "1 channel".to_string()
    } else {
        format!("{count} channels")
    }
}

#[component]
pub fn Subscriptions() -> Element {
    let app_state = use_context::<AppState>();
    let mut create_open = use_signal(|| false);
    let mut group_name = use_signal(String::new);
    // Which group's membership editor is open.
    let mut editing_group = use_signal(|| None::<String>);
    let mut editor_open = use_signal(|| false);
    let mut group_search = use_signal(String::new);

    let mut visible_count = use_signal(|| CHANNEL_PAGE_SIZE);
    // Sorting out the channel catalogue is the slow part of this page, so the
    // header and placeholders go up first.
    let painted = use_after_first_paint();

    let groups = app_state.with_viewer(|viewer| viewer.subscription_groups.clone());
    let subscribed = if painted() {
        app_state.with_viewer(|viewer| viewer.subscriptions.clone())
    } else {
        Vec::new()
    };
    let remaining = subscribed.len().saturating_sub(visible_count());

    let editing = editing_group();
    let editing_group_name = editing
        .as_ref()
        .and_then(|id| groups.iter().find(|group| &group.id == id))
        .map(|group| group.name.clone())
        .unwrap_or_default();
    let editing_members = editing
        .as_ref()
        .and_then(|id| groups.iter().find(|group| &group.id == id))
        .map(|group| group.channel_ids.clone())
        .unwrap_or_default();
    let group_query = group_search().trim().to_lowercase();
    // Only while the editor is up: it lists every subscription at once.
    let subscribed_for_editor = if editor_open() {
        subscribed
            .iter()
            .filter(|channel| {
                group_query.is_empty()
                    || channel.name.to_lowercase().contains(&group_query)
                    || channel.handle.to_lowercase().contains(&group_query)
            })
            .cloned()
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    rsx! {
        PageHeader {}
        Content {
            Stack { gap: Space::Lg,
                Card {
                    title: "Groups",
                    subtitle: "Group channels to filter your feed with one tap.",
                    end: rsx! {
                        Button {
                            size: ButtonSize::Sm,
                            fill: ButtonFill::Clear,
                            start: rsx! { Plus { size: 16 } },
                            onclick: move |_| create_open.set(true),
                            "New group"
                        }
                    },
                    if !groups.is_empty() {
                        List { variant: ListVariant::Filled, lines: ListLines::Inset,
                            for group in groups.clone() {
                                {
                                    let group_id = group.id.clone();
                                    let delete_id = group.id.clone();
                                    let members = group
                                        .channel_ids
                                        .iter()
                                        .filter_map(|id| subscribed.iter().find(|channel| &channel.id == id))
                                        .take(5)
                                        .cloned()
                                        .collect::<Vec<_>>();
                                    rsx! {
                                        Item {
                                            key: "{group.id}",
                                            label: group.name.clone(),
                                            description: channel_count(group.channel_ids.len()),
                                            onclick: move |_| {
                                                editing_group.set(Some(group_id.clone()));
                                                group_search.set(String::new());
                                                editor_open.set(true);
                                            },
                                            end: rsx! {
                                                // Faces make a group's contents readable at a glance.
                                                div { class: "flex -space-x-2", aria_hidden: "true",
                                                    for member in members.iter().take(5) {
                                                        Avatar {
                                                            key: "{member.id}",
                                                            name: member.name.clone(),
                                                            src: member.avatar_url.clone(),
                                                            size: AvatarSize::Sm,
                                                        }
                                                    }
                                                }
                                                Button {
                                                    fill: ButtonFill::Clear,
                                                    color: Color::Danger,
                                                    size: ButtonSize::Sm,
                                                    aria_label: "Delete {group.name}",
                                                    onclick: move |_| {
                                                        if let Some(name) = app_state.delete_subscription_group(&delete_id) {
                                                            app_state.show_toast(format!("Deleted {name}"), Color::Neutral);
                                                        }
                                                    },
                                                    Trash2 { size: 16 }
                                                }
                                            },
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                Stack { gap: Space::Sm,
                    // Styled as the Suggested shelf title below, so the two
                    // sections read as peers.
                    if !painted() {
                        Text { variant: g3_ui::TextVariant::Overline, "Subscriptions" }
                        Grid { columns: GridColumns::Fit(20.0),
                            for index in 0..6 {
                                Skeleton { key: "{index}", shape: SkeletonShape::Block, class: "h-16 rounded-lg" }
                            }
                        }
                    } else if subscribed.is_empty() {
                        Text { variant: g3_ui::TextVariant::Overline, "Subscriptions · 0" }
                        EmptyState {
                            title: "No subscriptions yet",
                            icon: rsx! { Users { size: 40 } },
                            "Subscribe to a channel from search or a video to see it here."
                        }
                    } else {
                        Text { variant: g3_ui::TextVariant::Overline, "Subscriptions · {subscribed.len()}" }
                        Grid { columns: GridColumns::Fit(20.0),
                            for channel in subscribed.iter().take(visible_count()).cloned() {
                                ChannelCard { key: "{channel.id}", channel }
                            }
                        }
                        InfiniteScroll {
                            loading: false,
                            complete: remaining == 0,
                            on_load: move |_| {
                                if remaining > 0 {
                                    visible_count += CHANNEL_PAGE_SIZE;
                                }
                            },
                        }
                    }
                }

            }

            // Membership editing lives in one place, so adding a channel to a
            // specific group does not mean hunting for that channel's tile. A
            // modal rather than a sheet: this is a focused editing task with a
            // search field and a long list.
            Modal {
                open: editor_open,
                title: editing_group_name.clone(),
                actions: rsx! {
                    Button { onclick: move |_| editor_open.set(false), "Done" }
                },
                Stack {
                    Text { tone: TextTone::Secondary, "Choose which channels belong to this group." }
                    Searchbar { value: group_search, placeholder: "Search subscribed channels", debounce_ms: 0 }
                    if subscribed.is_empty() {
                        Text { tone: TextTone::Secondary, "Subscribe to a channel first." }
                    } else if subscribed_for_editor.is_empty() {
                        Text { tone: TextTone::Secondary, "No subscribed channels match your search." }
                    } else {
                        List { lines: ListLines::Inset,
                            for channel in subscribed_for_editor {
                                {
                                    let channel_id = channel.id.clone();
                                    let group_id = editing.clone().unwrap_or_default();
                                    rsx! {
                                        Item {
                                            key: "{channel.id}",
                                            label: channel.name.clone(),
                                            checked: editing_members.contains(&channel.id),
                                            start: rsx! {
                                                Avatar { name: channel.name.clone(), src: channel.avatar_url.clone(), size: AvatarSize::Sm }
                                            },
                                            onclick: move |_| {
                                                app_state.toggle_channel_in_group(&group_id, &channel_id);
                                            },
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Modal {
                open: create_open,
                title: "New group",
                actions: rsx! {
                    Button { fill: ButtonFill::Clear, onclick: move |_| create_open.set(false), "Cancel" }
                    Button {
                        disabled: group_name().trim().is_empty(),
                        onclick: move |_| {
                            let name = group_name().trim().to_string();
                            if name.is_empty() { return; }
                            app_state.create_subscription_group(name.clone());
                            group_name.set(String::new());
                            create_open.set(false);
                            app_state.show_toast(format!("Created {name}"), Color::Success);
                        },
                        "Create"
                    }
                },
                Input {
                    label: "Group name",
                    value: group_name,
                    placeholder: "Documentaries",
                    autofocus: true,
                }
            }
        }
    }
}
