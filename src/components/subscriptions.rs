use crate::{app::Route, models::Channel, state::AppState};
use dioxus::prelude::*;
use dioxus_icons::lucide::{Check, Plus, Trash2, Users};
use g3_route_transitions::animated_navigate;
use g3_ui::{
    Avatar, AvatarSize, Button, ButtonFill, ButtonSize, Card, Color, Content, EmptyState, Grid,
    GridColumns, Input, Item, List, ListLines, ListVariant, Modal, Searchbar, Shelf, Space, Stack,
    Text, TextTone,
};

use super::PageHeader;

/// One channel tile, used by both the subscribed grid and the suggestions.
#[component]
pub fn ChannelCard(channel: Channel) -> Element {
    let app_state = use_context::<AppState>();
    let channel_id = channel.id.clone();
    let open_channel_id = channel.id.clone();
    let is_subscribed = channel.subscribed;
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
                        if let Some(now_subscribed) = app_state.toggle_subscription(&channel_id) {
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

    let library = app_state.library();
    let groups = library.subscription_groups.clone();
    // The page is about channels you follow. Everything else is a suggestion
    // and belongs below them, not mixed in.
    let (subscribed, suggested): (Vec<Channel>, Vec<Channel>) = library
        .channels
        .into_iter()
        .partition(|channel| channel.subscribed);

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
    let subscribed_for_editor = subscribed
        .iter()
        .filter(|channel| {
            group_query.is_empty()
                || channel.name.to_lowercase().contains(&group_query)
                || channel.handle.to_lowercase().contains(&group_query)
        })
        .cloned()
        .collect::<Vec<_>>();

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
                    Text { variant: g3_ui::TextVariant::Overline, "Subscriptions · {subscribed.len()}" }
                    if subscribed.is_empty() {
                        EmptyState {
                            title: "No subscriptions yet",
                            icon: rsx! { Users { size: 40 } },
                            "Subscribe to a channel from search or a video to see it here."
                        }
                    } else {
                        Grid { columns: GridColumns::Fit(20.0),
                            for channel in subscribed.clone() {
                                ChannelCard { key: "{channel.id}", channel }
                            }
                        }
                    }
                }

                if !suggested.is_empty() {
                    Shelf { title: "Suggested", gap: Space::Md,
                        end: rsx! { Text { tone: TextTone::Secondary, "From your searches and watch history" } },
                        // A shelf is a preview, not a second copy of the full
                        // channel catalogue. Keeping it bounded also avoids
                        // mounting hundreds of image cards in one scroll row.
                        for channel in suggested.into_iter().take(12) {
                            div { key: "{channel.id}", class: "w-72",
                                ChannelCard { channel }
                            }
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
