use crate::{app::Route, state::AppState};
use dioxus::prelude::*;
use dioxus_icons::lucide::{Bell, BellOff, Layers, Plus, X};
use g3_ui::{Badge, Button, ButtonSize, ButtonStyle, Field, Modal, StatusColor};

#[component]
pub fn Subscriptions() -> Element {
    let app_state = use_context::<AppState>();
    let navigator = use_navigator();
    let mut create_open = use_signal(|| false);
    let mut group_name = use_signal(String::new);
    let library = app_state.library();
    let channels = library.channels;
    let groups = library.subscription_groups;
    let subscribed_count = channels.iter().filter(|channel| channel.subscribed).count();

    rsx! {
        main { class: "page subscriptions-page",
            div { class: "section-heading",
                div {
                    span { class: "section-kicker", "{subscribed_count} CHANNELS" }
                    h2 { "Your corner of YouTube" }
                    p { "The server watches these channels for new uploads, even while Tawny is closed." }
                }
                div { class: "section-actions",
                    Badge { color: StatusColor::Success, "Polling enabled" }
                    Button {
                        size: ButtonSize::Sm,
                        start: rsx! { Plus { size: 16 } },
                        onclick: move |_| create_open.set(true),
                        "New group"
                    }
                }
            }
            section { class: "subscription-groups",
                div { class: "subsection-heading",
                    h3 { Layers { size: 18 } "Subscription groups" }
                    span { "Filter the feed without changing subscriptions." }
                }
                if groups.is_empty() {
                    p { class: "group-empty-copy", "Create a group for topics, moods, or people you watch together." }
                } else {
                    div { class: "group-summary-row",
                        for group in groups.clone() {
                            {
                                let group_id = group.id.clone();
                                let channel_count = group.channel_ids.len();
                                rsx! {
                                    div { class: "group-summary", key: "{group.id}",
                                        span { "{group.name}" }
                                        small { "{channel_count}" }
                                        button {
                                            aria_label: "Delete {group.name}",
                                            onclick: move |_| {
                                                if let Some(name) = app_state.delete_subscription_group(&group_id) {
                                                    app_state.show_toast(format!("Deleted {name}"), StatusColor::Neutral);
                                                }
                                            },
                                            X { size: 14 }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "channel-grid",
                for channel in channels {
                    {
                        let channel_id = channel.id.clone();
                        let open_channel_id = channel.id.clone();
                        let is_subscribed = channel.subscribed;
                        let initial = channel.name.chars().next().unwrap_or('T');
                        let avatar_url = channel.avatar_url.clone();
                        rsx! {
                            article {
                                class: "channel-card",
                                key: "{channel.id}",
                                role: "button",
                                tabindex: "0",
                                onclick: move |_| { navigator.push(Route::ChannelDetail { id: open_channel_id.clone() }); },
                                if let Some(avatar_url) = avatar_url {
                                    img {
                                        class: "channel-avatar channel-avatar-large",
                                        src: "{avatar_url}",
                                        alt: "{channel.name}",
                                        loading: "lazy",
                                    }
                                } else {
                                    div { class: "channel-avatar channel-avatar-large", "{initial}" }
                                }
                                div { class: "channel-card-copy",
                                    h3 { "{channel.name}" }
                                    p { "{channel.handle}" }
                                    span { "{channel.subscriber_count} subscribers" }
                                }
                                if is_subscribed && !groups.is_empty() {
                                    div { class: "channel-group-memberships", aria_label: "Groups for {channel.name}",
                                        for group in groups.clone() {
                                            {
                                                let group_id = group.id.clone();
                                                let member_channel_id = channel.id.clone();
                                                let included = group.channel_ids.contains(&channel.id);
                                                rsx! {
                                                    button {
                                                        class: if included { "membership-chip active" } else { "membership-chip" },
                                                        onclick: move |event: MouseEvent| {
                                                            event.stop_propagation();
                                                            app_state.toggle_channel_in_group(&group_id, &member_channel_id);
                                                        },
                                                        "{group.name}"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                Button {
                                    size: ButtonSize::Sm,
                                    style: if is_subscribed { ButtonStyle::Neutral } else { ButtonStyle::Solid },
                                    start: rsx! {
                                        if is_subscribed { Bell { size: 15 } } else { BellOff { size: 15 } }
                                    },
                                    onclick: move |event: MouseEvent| {
                                        event.stop_propagation();
                                        if let Some(now_subscribed) = app_state.toggle_subscription(&channel_id) {
                                            let message = if now_subscribed { "Subscription restored" } else { "Unsubscribed" };
                                            app_state.show_toast(message, StatusColor::Neutral);
                                        }
                                    },
                                    if is_subscribed { "Subscribed" } else { "Follow" }
                                }
                            }
                        }
                    }
                }
            }
        }
        Modal {
            open: create_open,
            title: "New subscription group".to_string(),
            description: rsx! { p { "Groups appear as one-tap filters above your feed." } },
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
