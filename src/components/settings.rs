use crate::{
    app::Route,
    models::{Appearance, PlatformStyle, SponsorAction, SponsorCategory, SwipeActionKind},
    state::AppState,
    subscriptions_io::{SubscriptionFormat, export_subscriptions, parse_subscription_export},
};
use dioxus::prelude::*;
use g3_route_transitions::ROUTE_TRANSITION_COVER_CLASS;

use super::{AccountSettings, BackendSettings, PageHeader};
use dioxus_icons::lucide::{
    Database, Download, Gauge, HardDrive, ListPlus, RotateCw, Server, ShieldCheck, Upload,
};
use g3_ui::{
    Badge, Body, Button, ButtonStyle, Card, Field, Item, List, RightSlot, SegmentButton,
    SegmentGroup, Select, SelectOption, StatusColor, Toggle,
};

/// Hand a generated file to the browser.
///
/// The export is built in Rust, so there is no URL to link to; a Blob and a
/// synthetic anchor click is the only way to produce a real download. The
/// object URL is revoked on the next tick, once the click has been dispatched.
fn download_text_file(file_name: &str, mime_type: &str, contents: String) {
    let file_name = serde_json::to_string(file_name).unwrap_or_else(|_| "\"export.txt\"".into());
    let mime_type = serde_json::to_string(mime_type).unwrap_or_else(|_| "\"text/plain\"".into());
    let contents = serde_json::to_string(&contents).unwrap_or_else(|_| "\"\"".into());
    spawn(async move {
        let script = format!(
            r#"
            const blob = new Blob([{contents}], {{ type: {mime_type} }});
            const url = URL.createObjectURL(blob);
            const anchor = document.createElement('a');
            anchor.href = url;
            anchor.download = {file_name};
            document.body.appendChild(anchor);
            anchor.click();
            anchor.remove();
            setTimeout(() => URL.revokeObjectURL(url), 0);
            dioxus.send(true);
            "#
        );
        let mut eval = document::eval(&script);
        let _ = eval.recv::<bool>().await;
    });
}

#[component]
pub fn SettingsPage() -> Element {
    let mut app_state = use_context::<AppState>();

    let settings = app_state.settings();
    let library = app_state.library();
    let start_name = library
        .playlists
        .iter()
        .find(|playlist| playlist.id == settings.swipe_right_playlist_id)
        .map(|playlist| playlist.name.clone())
        .unwrap_or_else(|| "Watch later".into());
    let end_name = library
        .playlists
        .iter()
        .find(|playlist| playlist.id == settings.swipe_left_playlist_id)
        .map(|playlist| playlist.name.clone())
        .unwrap_or_else(|| "Deep dives".into());
    let start_value = use_signal(|| start_name);
    let end_value = use_signal(|| end_name);
    let start_action = use_signal(|| settings.swipe_right_action.label().to_string());
    let end_action = use_signal(|| settings.swipe_left_action.label().to_string());
    let action_options = SwipeActionKind::ALL
        .iter()
        .map(|kind| SelectOption::from(kind.label().to_string()))
        .collect::<Vec<_>>();
    let action_options_for_end = action_options.clone();
    let options = library
        .playlists
        .iter()
        .map(|playlist| SelectOption::from(playlist.name.clone()))
        .collect::<Vec<_>>();
    let options_for_end = options.clone();
    let cache_size = library.videos.len() + library.channels.len() + library.playlists.len();
    let autoplay = use_signal(|| settings.autoplay);
    let shorts_autoplay = use_signal(|| settings.shorts_autoplay);
    let prefer_sabr = use_signal(|| settings.prefer_sabr);
    // Both segmented controls are index-driven, so the stored enum is mirrored
    // into a signal and written back when the index moves.
    let appearance_index = use_signal(|| match settings.appearance {
        Appearance::Dark => 0usize,
        Appearance::Light => 1,
    });
    use_effect(move || {
        let appearance = match appearance_index() {
            1 => Appearance::Light,
            _ => Appearance::Dark,
        };
        if app_state.settings.peek().appearance != appearance {
            app_state.settings.write().appearance = appearance;
        }
    });
    let platform_index = use_signal(|| match settings.platform_style {
        PlatformStyle::Auto => 0usize,
        PlatformStyle::Ios => 1,
        PlatformStyle::Material => 2,
    });
    use_effect(move || {
        let platform_style = match platform_index() {
            1 => PlatformStyle::Ios,
            2 => PlatformStyle::Material,
            _ => PlatformStyle::Auto,
        };
        if app_state.settings.peek().platform_style != platform_style {
            app_state.settings.write().platform_style = platform_style;
        }
    });
    let hide_watched = use_signal(|| settings.hide_watched);
    let auto_landscape = use_signal(|| settings.auto_landscape_fullscreen);
    let video_short_minutes = use_signal(|| (settings.video_short_max_seconds / 60).to_string());
    let video_medium_minutes = use_signal(|| (settings.video_medium_max_seconds / 60).to_string());
    let shorts_short_seconds = use_signal(|| settings.shorts_short_max_seconds.to_string());
    let shorts_medium_seconds = use_signal(|| settings.shorts_medium_max_seconds.to_string());

    rsx! {
        // One surface, one snapshot: the header rides up with the body it
        // belongs to instead of morphing in place while the page slides
        // underneath it.
        div { class: "auxiliary-cover {ROUTE_TRANSITION_COVER_CLASS}",
            PageHeader { title: "Settings".to_string(), back_to: Route::Feed {} }
            // Presented as a sheet, so the body region is the surface that rides up
            // over the dimmed shell.
            Body { padding: false,
                div { class: "page settings-page",
                        main { class: "settings-content",
                            // First, deliberately. Which server you talk to and
                            // who you are on it decide what every other setting
                            // below is even describing.
                            section { class: "settings-section",
                                span { class: "section-kicker", "ACCOUNT" }
                                AccountSettings {}
                            }
                            section { class: "settings-section",
                                span { class: "section-kicker", "SERVER" }
                                BackendSettings {}
                            }
                            section { class: "settings-section",
                                span { class: "section-kicker", "SPONSORBLOCK" }
                                SponsorBlockSettingsCard {}
                            }
                            section { class: "settings-section",
                                span { class: "section-kicker", "GESTURES" }
                                Card { title: "Swipe actions".to_string(),
                                    div { class: "setting-row",
                                        div { class: "setting-icon", ListPlus { size: 19 } }
                                        div { class: "setting-copy",
                                            strong { "Swipe right" }
                                            span { "What the gesture does" }
                                        }
                                        Select {
                                            value: start_action,
                                            options: action_options,
                                            onchange: move |label: String| {
                                                if let Some(kind) = SwipeActionKind::from_label(&label) {
                                                    app_state.settings.write().swipe_right_action = kind;
                                                }
                                            },
                                        }
                                    }
                                    // The playlist picker only matters when the action
                                    // is "Add to playlist", so it is hidden otherwise.
                                    if settings.swipe_right_action == SwipeActionKind::AddToPlaylist {
                                        div { class: "setting-row setting-row-nested",
                                            div { class: "setting-copy",
                                                span { "Destination playlist" }
                                            }
                                            Select {
                                                value: start_value,
                                                options,
                                                onchange: move |name: String| {
                                                    if let Some(playlist) = app_state.library().playlists.iter().find(|playlist| playlist.name == name) {
                                                        app_state.settings.write().swipe_right_playlist_id = playlist.id.clone();
                                                    }
                                                },
                                            }
                                        }
                                    }
                                    div { class: "setting-divider" }
                                    div { class: "setting-row",
                                        div { class: "setting-icon flip", ListPlus { size: 19 } }
                                        div { class: "setting-copy",
                                            strong { "Swipe left" }
                                            span { "What the gesture does" }
                                        }
                                        Select {
                                            value: end_action,
                                            options: action_options_for_end,
                                            onchange: move |label: String| {
                                                if let Some(kind) = SwipeActionKind::from_label(&label) {
                                                    app_state.settings.write().swipe_left_action = kind;
                                                }
                                            },
                                        }
                                    }
                                    if settings.swipe_left_action == SwipeActionKind::AddToPlaylist {
                                        div { class: "setting-row setting-row-nested",
                                            div { class: "setting-copy",
                                                span { "Destination playlist" }
                                            }
                                            Select {
                                                value: end_value,
                                                options: options_for_end,
                                                onchange: move |name: String| {
                                                    if let Some(playlist) = app_state.library().playlists.iter().find(|playlist| playlist.name == name) {
                                                        app_state.settings.write().swipe_left_playlist_id = playlist.id.clone();
                                                    }
                                                },
                                            }
                                        }
                                    }
                                }
                            }

                            section { class: "settings-section",
                                span { class: "section-kicker", "PLAYBACK" }
                                List { inset: true,
                                    Item {
                                        start: rsx! { Gauge { size: 19 } },
                                        label: "Video speed".to_string(),
                                        description: "Remembered for long-form video".to_string(),
                                        metadata: format!("{}×", settings.playback_speed),
                                    }
                                    Item {
                                        start: rsx! { Gauge { size: 19 } },
                                        label: "Shorts speed".to_string(),
                                        description: "Remembered separately from video".to_string(),
                                        metadata: format!("{}×", settings.shorts_playback_speed),
                                    }
                                    Item {
                                        label: "Autoplay videos".to_string(),
                                        description: "Continue with the next queued video".to_string(),
                                        end: rsx! {
                                            Toggle {
                                                checked: autoplay,
                                                onchange: move |enabled| app_state.settings.write().autoplay = enabled,
                                            }
                                        },
                                    }
                                    Item {
                                        label: "Autoplay Shorts".to_string(),
                                        description: "Remembered separately from video".to_string(),
                                        end: rsx! {
                                            Toggle {
                                                checked: shorts_autoplay,
                                                onchange: move |enabled| {
                                                    app_state.settings.write().shorts_autoplay = enabled
                                                },
                                            }
                                        },
                                    }
                                    Item {
                                        start: rsx! { RotateCw { size: 19 } },
                                        label: "Rotate horizontal fullscreen video".to_string(),
                                        description: "Switch phones to landscape automatically".to_string(),
                                        end: rsx! {
                                            Toggle {
                                                checked: auto_landscape,
                                                onchange: move |enabled| app_state.settings.write().auto_landscape_fullscreen = enabled,
                                            }
                                        },
                                    }
                                    Item {
                                        start: rsx! { ShieldCheck { size: 19 } },
                                        label: "Prefer SABR".to_string(),
                                        description: "Use adaptive streaming when the stream supports it".to_string(),
                                        end: rsx! {
                                            Toggle {
                                                checked: prefer_sabr,
                                                onchange: move |enabled| app_state.settings.write().prefer_sabr = enabled,
                                            }
                                        },
                                    }
                                }
                            }

                            section { class: "settings-section",
                                span { class: "section-kicker", "APPEARANCE & FEED" }
                                div { class: "setting-choice-row",
                                    div { class: "setting-choice-copy",
                                        strong { "Theme" }
                                        span { "Tawny's charcoal palette, or the light one" }
                                    }
                                    SegmentGroup { active: appearance_index, class: "setting-choice-segments".to_string(),
                                        SegmentButton { index: 0, "Dark" }
                                        SegmentButton { index: 1, "Light" }
                                    }
                                }
                                div { class: "setting-choice-row",
                                    div { class: "setting-choice-copy",
                                        strong { "Design language" }
                                        span { "Controls, sheets, and navigation motion" }
                                    }
                                    SegmentGroup { active: platform_index, class: "setting-choice-segments".to_string(),
                                        SegmentButton { index: 0, "Auto" }
                                        SegmentButton { index: 1, "iOS" }
                                        SegmentButton { index: 2, "Material" }
                                    }
                                }
                                List { inset: true,
                                    Item {
                                        label: "Hide watched videos".to_string(),
                                        description: "Keep the feed focused on what is new".to_string(),
                                        end: rsx! {
                                            Toggle {
                                                checked: hide_watched,
                                                onchange: move |hidden| app_state.settings.write().hide_watched = hidden,
                                            }
                                        },
                                    }
                                }
                                Card { class: "duration-settings-card", title: "Duration filters".to_string(),
                                    p { class: "settings-card-copy", "Set where Short ends and Medium begins for each kind of upload." }
                                    div { class: "duration-settings-grid",
                                        Field {
                                            label: "Video · Short max (minutes)".to_string(),
                                            value: video_short_minutes,
                                            r#type: "number".to_string(),
                                            min: 1,
                                            max: 240,
                                            onchange: move |event: Event<FormData>| {
                                                if let Ok(minutes) = event.value().parse::<u64>() {
                                                    let mut settings = app_state.settings.write();
                                                    settings.video_short_max_seconds = minutes * 60;
                                                    settings.video_medium_max_seconds = settings.video_medium_max_seconds.max((minutes + 1) * 60);
                                                }
                                            },
                                        }
                                        Field {
                                            label: "Video · Medium max (minutes)".to_string(),
                                            value: video_medium_minutes,
                                            r#type: "number".to_string(),
                                            min: 2,
                                            max: 600,
                                            onchange: move |event: Event<FormData>| {
                                                if let Ok(minutes) = event.value().parse::<u64>() {
                                                    let mut settings = app_state.settings.write();
                                                    settings.video_medium_max_seconds = (minutes * 60).max(settings.video_short_max_seconds + 60);
                                                }
                                            },
                                        }
                                        Field {
                                            label: "Shorts · Short max (seconds)".to_string(),
                                            value: shorts_short_seconds,
                                            r#type: "number".to_string(),
                                            min: 5,
                                            max: 170,
                                            onchange: move |event: Event<FormData>| {
                                                if let Ok(seconds) = event.value().parse::<u64>() {
                                                    let mut settings = app_state.settings.write();
                                                    settings.shorts_short_max_seconds = seconds;
                                                    settings.shorts_medium_max_seconds = settings.shorts_medium_max_seconds.max(seconds + 1);
                                                }
                                            },
                                        }
                                        Field {
                                            label: "Shorts · Medium max (seconds)".to_string(),
                                            value: shorts_medium_seconds,
                                            r#type: "number".to_string(),
                                            min: 6,
                                            max: 180,
                                            onchange: move |event: Event<FormData>| {
                                                if let Ok(seconds) = event.value().parse::<u64>() {
                                                    let mut settings = app_state.settings.write();
                                                    settings.shorts_medium_max_seconds = seconds.max(settings.shorts_short_max_seconds + 1);
                                                }
                                            },
                                        }
                                    }
                                }
                            }

                            section { class: "settings-section",
                                span { class: "section-kicker", "SUBSCRIPTIONS" }
                                p { class: "section-note",
                                    "Import a subscription list from Google Takeout, Piped, LibreTube, or any "
                                    "OPML feed reader. Tawny's own format is the only one that also carries "
                                    "your per-channel Videos/Shorts choice and your groups."
                                }
                                List { inset: true,
                                    Item {
                                        start: rsx! { Upload { size: 18 } },
                                        label: "Import subscriptions".to_string(),
                                        description: "Choose a .json, .csv, or .opml export".to_string(),
                                        onclick: move |_| {
                                            spawn(async move {
                                                let mut open = document::eval(
                                                    "document.getElementById('tawny-subscription-import')?.click(); dioxus.send(true);",
                                                );
                                                let _ = open.recv::<bool>().await;
                                            });
                                        },
                                    }
                                }
                                // Driven by the row above rather than shown directly: a
                                // bare file input cannot be styled to match the list.
                                input {
                                    id: "tawny-subscription-import",
                                    r#type: "file",
                                    accept: ".json,.csv,.opml,.xml,application/json,text/csv,text/xml",
                                    class: "visually-hidden-input",
                                    onchange: move |event: FormEvent| {
                                        async move {
                                            let Some(file) = event.files().first().cloned() else { return };
                                            let Ok(contents) = file.read_string().await else {
                                                app_state.show_toast("Could not read that file", StatusColor::Danger);
                                                return;
                                            };
                                            match parse_subscription_export(&contents) {
                                                Ok(parsed) => {
                                                    let format = parsed.format;
                                                    let summary = app_state.import_subscriptions(parsed);
                                                    let color = if summary.changed() {
                                                        StatusColor::Success
                                                    } else {
                                                        StatusColor::Neutral
                                                    };
                                                    app_state.show_toast(
                                                        format!("{} \u{2014} {}", format.label(), summary.message()),
                                                        color,
                                                    );
                                                }
                                                Err(error) => {
                                                    app_state.show_toast(format!("Import failed: {error}"), StatusColor::Danger);
                                                }
                                            }
                                        }
                                    },
                                }
                                span { class: "section-kicker section-kicker-inline", "EXPORT" }
                                div { class: "export-format-row",
                                    for format in [
                                        SubscriptionFormat::Tawny,
                                        SubscriptionFormat::Piped,
                                        SubscriptionFormat::TakeoutCsv,
                                        SubscriptionFormat::Opml,
                                    ] {
                                        Button {
                                            key: "{format.file_name()}",
                                            style: ButtonStyle::Neutral,
                                            onclick: move |_| {
                                                let library = app_state.library();
                                                let subscribed = library
                                                    .channels
                                                    .iter()
                                                    .filter(|channel| channel.subscribed)
                                                    .count();
                                                if subscribed == 0 {
                                                    app_state.show_toast("No subscriptions to export", StatusColor::Neutral);
                                                    return;
                                                }
                                                let contents = export_subscriptions(
                                                    format,
                                                    &library.channels,
                                                    &library.subscription_groups,
                                                );
                                                download_text_file(format.file_name(), format.mime_type(), contents);
                                                app_state.show_toast(
                                                    format!("Exported {subscribed} subscriptions as {}", format.label()),
                                                    StatusColor::Success,
                                                );
                                            },
                                            Download { size: 16 }
                                            "{format.label()}"
                                        }
                                    }
                                }
                            }


                            section { class: "settings-section",
                                span { class: "section-kicker", "DATA" }
                                div { class: "data-grid",
                                    Card { class: "data-card", title: "Local cache".to_string(), right_slot: RightSlot::Element(rsx! { HardDrive { size: 18 } }),
                                        strong { "{cache_size}" }
                                        span { "cached records" }
                                    }
                                    Card { class: "data-card", title: "Sync server".to_string(), right_slot: RightSlot::Element(rsx! { Server { size: 18 } }),
                                        Badge { color: StatusColor::Success, "Ready" }
                                        span { "SurrealDB-backed" }
                                    }
                                    Card { class: "data-card", title: "Playback".to_string(), right_slot: RightSlot::Element(rsx! { Database { size: 18 } }),
                                        Badge { color: StatusColor::Accent, "Built-in" }
                                        span { "PoToken per request" }
                                    }
                                }
                            }
                        }
                }
            }
        }
    }
}

/// Per-category SponsorBlock behaviour.
///
/// One row per category rather than a single on/off, because the categories are
/// not equivalent: a sponsor read is never wanted, while an intro often is the
/// video. The defaults mirror the browser extension, so someone who already uses
/// it finds Tawny behaving the way they expect.
///
/// The category rows are laid out here rather than with `Item`, which is built
/// for a one-line summary: it sets `white-space: nowrap` and clips both its
/// label and its description. That is right for "Autoplay - continue with the
/// next video" and wrong for text a viewer has to read before choosing, because
/// a truncated explanation cannot be recovered - there is nowhere to expand it.
#[component]
fn SponsorBlockSettingsCard() -> Element {
    let mut app_state = use_context::<AppState>();
    let settings = app_state.settings();
    let sponsor = settings.sponsor_block.clone();
    let enabled_value = sponsor.enabled;
    let enabled = use_signal(|| enabled_value);
    let notify = use_signal(|| sponsor.notify_on_skip);

    let action_options = SponsorAction::ALL
        .iter()
        .map(|action| SelectOption::from(action.label().to_string()))
        .collect::<Vec<_>>();

    rsx! {
        Card { title: "SponsorBlock".to_string(),
            List { inset: true,
                Item {
                    start: rsx! { ShieldCheck { size: 19 } },
                    label: "Use SponsorBlock".to_string(),
                    description: "Segments marked by the community".to_string(),
                    end: rsx! {
                        Toggle {
                            checked: enabled,
                            onchange: move |value| app_state.settings.write().sponsor_block.enabled = value,
                        }
                    },
                }
                if enabled_value {
                    Item {
                        label: "Announce skips".to_string(),
                        description: "So a jump does not look like a fault".to_string(),
                        end: rsx! {
                            Toggle {
                                checked: notify,
                                onchange: move |value| {
                                    app_state.settings.write().sponsor_block.notify_on_skip = value
                                },
                            }
                        },
                    }
                }
            }

            if enabled_value {
                div { class: "sponsor-categories",
                    for category in SponsorCategory::ALL {
                        {
                            let options = action_options.clone();
                            let value = use_signal(|| sponsor.action_for(category).label().to_string());
                            rsx! {
                                div { class: "sponsor-category", key: "{category.api_name()}",
                                    div { class: "sponsor-category-heading",
                                        // The same colour this category gets on
                                        // the timeline, so the two can be read
                                        // against each other.
                                        span {
                                            class: "sponsor-category-swatch",
                                            style: "background: {category.color()};",
                                            aria_hidden: "true",
                                        }
                                        strong { "{category.label()}" }
                                    }
                                    p { class: "sponsor-category-description", "{category.description()}" }
                                    Select {
                                        value,
                                        options,
                                        onchange: move |label: String| {
                                            let action = SponsorAction::from_label(&label);
                                            app_state
                                                .settings
                                                .write()
                                                .sponsor_block
                                                .set_action(category, action);
                                        },
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
