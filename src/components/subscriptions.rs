use crate::{app::Route, models::Channel, state::AppState};
use dioxus::prelude::*;
use dioxus_icons::lucide::{Check, Layers, Plus, Search, Trash2, User};
use g3_route_transitions::animated_navigate;
use g3_ui::{Button, ButtonSize, ButtonStyle, Card, Field, Modal, Sheet, StatusColor};

/// One channel tile, used by both the subscribed grid and the suggestions row.
#[component]
fn ChannelCard(channel: Channel) -> Element {
    let app_state = use_context::<AppState>();
    let channel_id = channel.id.clone();
    let open_channel_id = channel.id.clone();
    let is_subscribed = channel.subscribed;
    let avatar_url = channel.avatar_url.clone();

    rsx! {
        article {
            class: "channel-card",
            role: "button",
            tabindex: "0",
            onclick: move |_| { { let v = open_channel_id.clone(); spawn(async move { animated_navigate(Route::ChannelDetail { id: v }).await; }); }; },
            if let Some(avatar_url) = avatar_url {
                img {
                    class: "channel-avatar channel-avatar-large",
                    src: "{avatar_url}",
                    alt: "{channel.name}",
                    loading: "lazy",
                }
            } else {
                div { class: "channel-avatar channel-avatar-large channel-avatar-fallback", User { size: 24 } }
            }
            div { class: "channel-card-copy",
                h3 { "{channel.name}" }
                if !channel.subscriber_count.trim().is_empty() {
                    span { "{channel.subscriber_count} subscribers" }
                }
            }
            Button {
                size: ButtonSize::Sm,
                class: "channel-subscribe-button",
                start: if is_subscribed { Some(rsx! { Check { size: 15 } }) } else { None },
                style: if is_subscribed { ButtonStyle::Neutral } else { ButtonStyle::Solid },
                onclick: move |event: MouseEvent| {
                    event.stop_propagation();
                    if let Some(now_subscribed) = app_state.toggle_subscription(&channel_id) {
                        let message = if now_subscribed { "Subscribed" } else { "Unsubscribed" };
                        app_state.show_toast(message, StatusColor::Neutral);
                    }
                },
                if is_subscribed { "Subscribed" } else { "Subscribe" }
            }
        }
    }
}

#[component]
pub fn Subscriptions() -> Element {
    let app_state = use_context::<AppState>();
    let mut create_open = use_signal(|| false);
    let mut group_name = use_signal(String::new);
    // Which group's membership sheet is open.
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
        main { class: "page subscriptions-page",
            section { class: "subscription-groups",
                Card { title: "Groups".to_string(),
                    if groups.is_empty() {
                        p { class: "group-empty-copy", "Group channels to filter your feed with one tap." }
                    } else {
                        div { class: "group-card-list",
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
                                    let member_count = group.channel_ids.len();
                                    rsx! {
                                        div { class: "group-row", key: "{group.id}",
                                            div { class: "group-row-copy",
                                                strong { "{group.name}" }
                                                span {
                                                    if member_count == 1 {
                                                        "1 channel"
                                                    } else {
                                                        "{member_count} channels"
                                                    }
                                                }
                                            }
                                            // Faces make a group's contents readable at a glance.
                                            div { class: "group-row-faces",
                                                for member in members.iter().take(6) {
                                                    if let Some(avatar_url) = member.avatar_url.clone() {
                                                        img {
                                                            class: "channel-avatar channel-avatar-fallback",
                                                            key: "{member.id}",
                                                            src: "{avatar_url}",
                                                            alt: "{member.name}",
                                                            title: "{member.name}",
                                                            loading: "lazy",
                                                        }
                                                    } else {
                                                        div {
                                                            class: "channel-avatar",
                                                            key: "{member.id}",
                                                            title: "{member.name}",
                                                            User { size: 16 }
                                                        }
                                                    }
                                                }
                                            }
                                            Button {
                                                size: ButtonSize::Sm,
                                                style: ButtonStyle::Neutral,
                                                onclick: move |_| {
                                                    editing_group.set(Some(group_id.clone()));
                                                    group_search.set(String::new());
                                                    editor_open.set(true);
                                                },
                                                "Edit"
                                            }
                                            button {
                                                class: "group-row-delete",
                                                aria_label: "Delete {group.name}",
                                                onclick: move |event: MouseEvent| {
                                                    event.stop_propagation();
                                                    if let Some(name) = app_state.delete_subscription_group(&delete_id) {
                                                        app_state.show_toast(format!("Deleted {name}"), StatusColor::Neutral);
                                                    }
                                                },
                                                Trash2 { size: 15 }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // Creating a group belongs with the groups, not in a page header.
                    Button {
                        expand: true,
                        style: ButtonStyle::Neutral,
                        start: rsx! { Plus { size: 16 } },
                        onclick: move |_| create_open.set(true),
                        "New group"
                    }
                }
            }

            div { class: "subsection-heading subscription-heading",
                h3 { Layers { size: 18 } "Subscriptions" }
                span { class: "subscription-count", "{subscribed.len()}" }
            }
            if subscribed.is_empty() {
                div { class: "empty-state",
                    div { class: "empty-icon", User { size: 25 } }
                    h3 { "No subscriptions yet" }
                    p { "You have not subscribed to any channels yet." }
                }
            } else {
                div { class: "channel-grid",
                    for channel in subscribed.clone() {
                        ChannelCard { key: "{channel.id}", channel }
                    }
                }
            }

            if !suggested.is_empty() {
                div { class: "subsection-heading",
                    h3 { "Suggested" }
                    span { "From your searches and watch history" }
                }
                div { class: "channel-grid",
                    for channel in suggested {
                        ChannelCard { key: "{channel.id}", channel }
                    }
                }
            }
        }

        // Membership editing lives in one place, so adding a channel to a
        // specific group does not mean hunting for that channel's tile.
        Sheet { is_open: editor_open, class: "group-editor-sheet",
            div { class: "sheet-heading",
                div { class: "sheet-icon", Layers { size: 22 } }
                div {
                    h2 { "{editing_group_name}" }
                    p { "Choose which channels belong to this group" }
                }
            }
            Field {
                label: "Search channels".to_string(),
                value: group_search,
                r#type: "search".to_string(),
                placeholder: "Search subscribed channels".to_string(),
                class: "group-editor-search",
                end: rsx! { Search { size: 18 } },
            }
            if subscribed.is_empty() {
                p { class: "detail-muted", "Subscribe to a channel first." }
            } else if subscribed_for_editor.is_empty() {
                p { class: "group-editor-empty", "No subscribed channels match your search." }
            } else {
                div { class: "group-editor-list",
                    for channel in subscribed_for_editor {
                        {
                            let channel_id = channel.id.clone();
                            let included = editing_members.contains(&channel.id);
                            let group_id = editing.clone().unwrap_or_default();
                            rsx! {
                                button {
                                    class: if included { "group-editor-row included" } else { "group-editor-row" },
                                    key: "{channel.id}",
                                    onclick: move |_| {
                                        app_state.toggle_channel_in_group(&group_id, &channel_id);
                                    },
                                    if let Some(avatar_url) = channel.avatar_url.clone() {
                                        img { class: "channel-avatar", src: "{avatar_url}", alt: "", loading: "lazy" }
                                    } else {
                                        div { class: "channel-avatar channel-avatar-fallback", User { size: 16 } }
                                    }
                                    span { "{channel.name}" }
                                    if included {
                                        Check { size: 17 }
                                    } else {
                                        Plus { size: 17 }
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
            title: "New group".to_string(),
            actions: rsx! {
                Button { style: ButtonStyle::Clear, onclick: move |_| create_open.set(false), "Cancel" }
                Button {
                    disabled: group_name().trim().is_empty(),
                    onclick: move |_| {
                        let name = group_name().trim().to_string();
                        if name.is_empty() { return; }
                        app_state.create_subscription_group(name.clone());
                        group_name.set(String::new());
                        create_open.set(false);
                        app_state.show_toast(format!("Created {name}"), StatusColor::Success);
                    },
                    "Create"
                }
            },
            Field {
                label: "Group name".to_string(),
                value: group_name,
                placeholder: "Documentaries".to_string(),
                autofocus: true,
            }
        }
    }
}
