use crate::{
    api::{get_comments_page, get_video_details, resolve_playback},
    app::Route,
    models::{
        CaptionTrack, PlaybackProtocol, PlaybackSession, PlaybackSource, Video, VideoChapter,
        VideoComment, VideoDetails, VideoPreviewFrames,
    },
    state::AppState,
};
use dioxus::prelude::*;
use dx_route_transitions::animated_navigate;
use dioxus_icons::lucide::{
    Captions, Check, ChevronLeft, Clock, Heart, ListVideo, Maximize2, MessageSquare, Minimize2,
    Pause,
    PictureInPicture, Play, RotateCcw, RotateCw, Settings2, Share2, ThumbsDown, ThumbsUp, Volume2,
    VolumeX, X,
};
use g3_ui::{Badge, Button, ButtonSize, ButtonStyle, Sheet, StatusColor};

use super::VideoGrid;

fn sync_player_metadata(
    tracks: Vec<CaptionTrack>,
    selected_caption: Option<usize>,
    captions_enabled: bool,
    chapters: Vec<VideoChapter>,
    preview_frames: Option<VideoPreviewFrames>,
) {
    let Ok(tracks) = serde_json::to_string(&tracks) else {
        return;
    };
    let Ok(chapters) = serde_json::to_string(&chapters) else {
        return;
    };
    let Ok(preview_frames) = serde_json::to_string(&preview_frames) else {
        return;
    };
    let selected_caption = selected_caption
        .map(|index| index.to_string())
        .unwrap_or_else(|| "null".to_string());
    spawn(async move {
        let script = format!(
            r#"
            const tracks = {tracks};
            const chapters = {chapters};
            const previewFrames = {preview_frames};
            let media = document.getElementById('tawny-player-media');
            for (let attempt = 0; attempt < 800 && (!media || !window.TawnyPlayerControls); attempt++) {{
                await new Promise((resolve) => setTimeout(resolve, 25));
                media = document.getElementById('tawny-player-media');
            }}
            if (media && window.TawnyPlayerControls) {{
                window.TawnyPlayerControls.setMetadata(media, {{
                    captions: tracks,
                    selectedCaption: {selected_caption},
                    captionsEnabled: {captions_enabled},
                    chapters,
                    previewFrames,
                }});
            }}
            dioxus.send(true);
            "#
        );
        let mut eval = document::eval(&script);
        let _ = eval.recv::<bool>().await;
    });
}

/// Pull the transport's own account of why playback stopped.
///
/// `TawnyTransport.events` outlives the media element, so this stays readable
/// after the failed player has been torn down.
fn read_transport_failure(mut detail: Signal<String>) {
    spawn(async move {
        let mut eval = document::eval(
            r#"
            const events = (window.TawnyTransport && window.TawnyTransport.events) || [];
            const notable = events.filter((event) =>
                event.phase === 'source-failed'
                || event.phase === 'recovering'
                || event.phase === 'retrying-stream');
            const last = notable[notable.length - 1] || window.__tawnyTransportDebug || null;
            const describe = (value) =>
                typeof value === 'string' ? value : JSON.stringify(value);
            const parts = [];
            if (last) {
                const source = last.protocol ? ` · ${last.protocol}` : '';
                const cause = last.error ? ` · ${describe(last.error)}` : '';
                const at = Number.isFinite(last.position)
                    ? ` · at ${Math.floor(last.position)}s`
                    : '';
                parts.push(`${last.phase}${source}${at}${cause}`);
            }
            const media = document.getElementById('tawny-player-media');
            if (media && media.error) {
                parts.push(`media error ${media.error.code}: ${media.error.message || 'no detail'}`);
            }
            dioxus.send(parts.join(' — '));
            "#,
        );
        if let Ok(text) = eval.recv::<String>().await
            && !text.is_empty()
        {
            detail.set(text);
        }
    });
}

fn set_player_speed(speed: f64) {
    spawn(async move {
        let script = format!(
            r#"
            const media = document.querySelector('#tawny-player video');
            if (media) media.playbackRate = {speed};
            const frame = document.getElementById('tawny-player-frame');
            if (frame && frame.contentWindow) {{
                frame.contentWindow.postMessage(JSON.stringify({{
                    event: 'command', func: 'setPlaybackRate', args: [{speed}]
                }}), '*');
            }}
            dioxus.send(true);
            "#
        );
        let mut eval = document::eval(&script);
        let _ = eval.recv::<bool>().await;
    });
}

fn persist_player_speed(mut app_state: AppState, is_short: bool) {
    spawn(async move {
        let mut eval = document::eval(
            r#"
            const media = document.getElementById('tawny-player-media');
            dioxus.send(media ? media.playbackRate : 1);
            "#,
        );
        if let Ok(speed) = eval.recv::<f64>().await
            && speed.is_finite()
            && speed > 0.0
        {
            app_state.settings.write().set_speed_for(is_short, speed);
        }
    });
}

fn attach_player_session(
    session: PlaybackSession,
    playback_rate: f64,
    video_id: &str,
    prefer_sabr: bool,
) {
    let Ok(session) = serde_json::to_string(&session) else {
        return;
    };
    let video_id = video_id.to_string();
    spawn(async move {
        let script = format!(
            r#"
            const session = {session};
            const videoId = {video_id:?};
            const media = document.getElementById('tawny-player-media');
            window.__tawnyAttachEval = {{ phase: 'starting', hasMedia: Boolean(media) }};
            if (!media) {{ dioxus.send(false); return; }}
            for (let attempt = 0; attempt < 100 && !window.TawnyTransport; attempt++) {{
                await new Promise((resolve) => setTimeout(resolve, 25));
            }}
            if (!window.TawnyTransport) {{
                window.__tawnyAttachEval = {{ phase: 'transport-missing', hasMedia: true }};
                media.dispatchEvent(new Event('error'));
                dioxus.send(false);
                return;
            }}
            try {{
                if (window.TawnyPlayerControls) {{
                    window.TawnyPlayerControls.attach(media);
                }}
                await window.TawnyTransport.attach(media, session, {{
                    playbackRate: {playback_rate},
                    videoId,
                    refreshUrl: `/api/v1/playback/${{encodeURIComponent(videoId)}}?prefer_sabr={prefer_sabr}`,
                }});
                window.__tawnyAttachEval = {{ phase: 'attached', hasMedia: true }};
                dioxus.send(true);
            }} catch (error) {{
                window.__tawnyAttachEval = {{ phase: 'failed', hasMedia: true, error: String(error) }};
                console.warn('Tawny transport exhausted its sources', error);
                media.dispatchEvent(new Event('error'));
                dioxus.send(false);
            }}
            "#
        );
        let mut eval = document::eval(&script);
        let _ = eval.recv::<bool>().await;
    });
}

#[component]
pub fn PersistentPlayer(expanded: bool) -> Element {
    let app_state = use_context::<AppState>();
    let mut playback_attempt = use_signal(|| 0_u8);
    // The privacy-enhanced iframe is a manual escape hatch, never an automatic
    // one. Silently swapping to it hides extraction regressions behind a player
    // that ignores the app's controls, captions, and chapters.
    let mut use_embed = use_signal(|| false);
    let mut playback_failed = use_signal(|| false);
    let mut failure_detail = use_signal(String::new);
    let mut observed_video_id = use_signal(String::new);
    let prefer_sabr = app_state.settings().prefer_sabr;

    // The resolved session is tagged with the video it belongs to. `use_resource`
    // keeps serving its previous value while it re-runs, so without the tag a
    // freshly opened video reads the *previous* video's session and renders a
    // conclusion — including a failure — that was never about this video.
    let playback_resource = use_resource(move || {
        let active_video = app_state.active_video();
        let attempt = playback_attempt();
        async move {
            let _attempt = attempt;
            match active_video {
                Some(video) => {
                    let id = video.id.clone();
                    Some((id, resolve_playback(video.id, prefer_sabr).await))
                }
                None => None,
            }
        }
    });

    let active_video = app_state.active_video();
    let video_id = active_video
        .as_ref()
        .map(|video| video.id.clone())
        .unwrap_or_default();
    use_effect(move || {
        if observed_video_id() != video_id {
            observed_video_id.set(video_id.clone());
            playback_attempt.set(0);
            use_embed.set(false);
            playback_failed.set(false);
            failure_detail.set(String::new());
        }
    });
    let Some(video) = active_video else {
        return rsx! {};
    };

    let resolved_for_this_video = playback_resource
        .read()
        .as_ref()
        .and_then(|value| value.as_ref())
        .is_some_and(|(id, _)| *id == video.id);
    let playback_session = playback_resource
        .read()
        .as_ref()
        .and_then(|value| value.as_ref())
        .filter(|(id, _)| *id == video.id)
        .and_then(|(_, result)| result.as_ref().ok())
        .cloned();
    let fallback_url = playback_session
        .as_ref()
        .map(|session| session.fallback_url.clone())
        .unwrap_or_else(|| {
            format!(
                "https://www.youtube-nocookie.com/embed/{}?enablejsapi=1&autoplay=1&playsinline=1",
                video.id
            )
        });
    let playback_source = playback_session.as_ref().map(|session| {
        if source_is_transportable(&session.primary) {
            session.primary.clone()
        } else {
            session
                .alternatives
                .iter()
                .find(|source| source_is_transportable(source))
                .cloned()
                .unwrap_or_else(|| session.primary.clone())
        }
    });
    // Only a session actually resolved for *this* video can justify the failure
    // panel; anything else means the resolve is still in flight.
    let resolving = !resolved_for_this_video;
    let direct_stream = !use_embed()
        && !playback_failed()
        && playback_source
            .as_ref()
            .is_some_and(source_is_transportable);
    // Two very different failures land on the same panel, and they need
    // different fixes: the server extracting nothing at all, versus the server
    // handing over streams that the browser then failed to play.
    let nothing_extracted = resolved_for_this_video
        && !playback_source
            .as_ref()
            .is_some_and(source_is_transportable);
    let is_short = video.is_short;
    let current_speed = app_state.settings().speed_for(is_short);
    let session_to_attach = playback_session.clone();
    let attach_video_id = video.id.clone();
    let captions_to_sync = app_state.active_captions;
    let selected_caption_to_sync = app_state.selected_caption;
    let captions_enabled_to_sync = app_state.captions_enabled;
    let chapters_to_sync = app_state.active_chapters;
    let preview_frames_to_sync = app_state.active_preview_frames;
    use_effect(move || {
        sync_player_metadata(
            captions_to_sync(),
            selected_caption_to_sync(),
            captions_enabled_to_sync(),
            chapters_to_sync(),
            preview_frames_to_sync(),
        )
    });
    let mut captions_enabled = app_state.captions_enabled;
    let captions_for_toggle = app_state.active_captions;
    let caption_tracks_to_render = (app_state.active_captions)();
    let mut chapters_sheet = app_state.chapters_sheet_open;
    let open_video_id = video.id.clone();
    let player_key = format!("{}-{}", video.id, playback_attempt());

    rsx! {
        div { class: if expanded { "persistent-player expanded" } else { "persistent-player mini" },
            section {
                id: "tawny-player",
                class: "persistent-player-stage",
                "data-video-id": "{video.id}",
                "data-thumbnail": "{video.thumbnail_url}",
                if direct_stream {
                    video {
                        id: "tawny-player-media",
                        key: "{player_key}",
                        autoplay: true,
                        playsinline: true,
                        poster: "{video.thumbnail_url}",
                        onmounted: move |_| {
                            if let Some(session) = session_to_attach.clone() {
                                attach_player_session(
                                    session,
                                    current_speed,
                                    &attach_video_id,
                                    prefer_sabr,
                                );
                            }
                        },
                        onloadedmetadata: move |_| set_player_speed(current_speed),
                        onratechange: move |_| persist_player_speed(app_state, is_short),
                        onerror: move |_| {
                            // Re-resolve the session a couple of times: playback
                            // URLs expire, and a fresh resolve is the fix. When
                            // that stops helping, say so instead of quietly
                            // handing the video to YouTube's own player.
                            if playback_attempt() < 2 {
                                playback_attempt += 1;
                            } else {
                                read_transport_failure(failure_detail);
                                playback_failed.set(true);
                            }
                        },
                        for caption in caption_tracks_to_render {
                            track {
                                kind: "subtitles",
                                src: "{caption.url}",
                                srclang: "{caption.language_code}",
                                label: "{caption.label}",
                                "data-tawny-caption": "",
                            }
                        }
                    }
                    div { class: "player-buffering-indicator", aria_hidden: "true",
                        div { class: "player-buffering-ring" }
                    }
                    div { class: "player-seek-feedback player-seek-feedback-left", aria_hidden: "true",
                        RotateCcw { size: 30 }
                        span { "10 seconds" }
                    }
                    div { class: "player-seek-feedback player-seek-feedback-right", aria_hidden: "true",
                        RotateCw { size: 30 }
                        span { "10 seconds" }
                    }
                    div {
                        class: "tawny-player-controls controls-visible",
                        role: "group",
                        aria_label: "Video controls",
                        div { class: "player-controls-scrim" }
                        div { class: "player-controls-top",
                            // Same affordance as every other page: this leaves
                            // the video, it does not shrink it in place.
                            button {
                                r#type: "button",
                                class: "player-control-button player-minimize-button",
                                aria_label: "Back",
                                title: "Back",
                                onclick: move |_| {
                                    let navigator = navigator();
                                    if navigator.can_go_back() {
                                        navigator.go_back();
                                    } else {
                                        spawn(async move { animated_navigate(Route::Feed {}).await; });
                                    }
                                },
                                ChevronLeft { size: 26 }
                            }
                            strong { class: "player-overlay-title", "{video.title}" }
                            div { class: "player-controls-top-actions",
                                button {
                                    r#type: "button",
                                    class: "player-control-button",
                                    aria_label: "Picture in picture",
                                    title: "Picture in picture",
                                    "data-player-action": "pip",
                                    PictureInPicture { size: 21 }
                                }
                                button {
                                    r#type: "button",
                                    class: "player-control-button",
                                    aria_label: "Player settings",
                                    title: "Player settings",
                                    "data-player-action": "settings",
                                    Settings2 { size: 22 }
                                }
                            }
                        }
                        // Play/pause alone. Skipping is a double-tap on either
                        // side of the video on any platform, and the arrow keys
                        // on a keyboard, so dedicated buttons only crowded the
                        // video they sit on.
                        div { class: "player-controls-center",
                            button {
                                r#type: "button",
                                class: "player-control-button player-play-button",
                                aria_label: "Play",
                                title: "Play or pause",
                                "data-player-action": "toggle",
                                span { class: "player-play-icon", Play { size: 38, fill: "currentColor" } }
                                span { class: "player-pause-icon", Pause { size: 38, fill: "currentColor" } }
                            }
                        }
                        div { class: "player-options-menu", "data-player-options-menu": "", hidden: true,
                            div { class: "player-options-section",
                                strong { "Playback speed" }
                                div { class: "player-option-grid",
                                    for speed in [0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0] {
                                        button {
                                            r#type: "button",
                                            class: "player-option-button",
                                            "data-player-speed": "{speed}",
                                            "{speed}×"
                                        }
                                    }
                                }
                            }
                            div { class: "player-options-section",
                                strong { "Quality" }
                                div { class: "player-option-grid", "data-player-quality-options": "",
                                    button { r#type: "button", class: "player-option-button selected", "Auto" }
                                }
                            }
                        }
                        div { class: "player-controls-bottom",
                            div { class: "player-control-row",
                                span { class: "player-time",
                                    span { "data-player-current-time": "", "0:00" }
                                    span { class: "player-time-separator", " • " }
                                    span { "data-player-duration": "", "0:00" }
                                    // Current chapter, to the right of the clock; tapping it
                                    // opens the chapter list rather than making the user scrub.
                                    button {
                                        r#type: "button",
                                        class: "player-chapter-label",
                                        "data-player-chapter-label": "",
                                        "data-player-action": "chapters",
                                        hidden: true,
                                    }
                                }
                                span { class: "player-control-spacer" }
                                button {
                                    r#type: "button",
                                    class: "player-control-button player-mute-button",
                                    aria_label: "Mute",
                                    title: "Mute",
                                    "data-player-action": "mute",
                                    span { class: "player-volume-icon", Volume2 { size: 20 } }
                                    span { class: "player-muted-icon", VolumeX { size: 20 } }
                                }
                                button {
                                    r#type: "button",
                                    class: "player-control-button player-captions-button",
                                    aria_label: "Captions",
                                    title: "Captions",
                                    "data-player-action": "captions",
                                    Captions { size: 21 }
                                }
                                button {
                                    r#type: "button",
                                    class: "player-caption-state-bridge",
                                    tabindex: "-1",
                                    aria_hidden: "true",
                                    "data-player-chapters-open": "",
                                    onclick: move |_| chapters_sheet.set(true),
                                }
                                button {
                                    r#type: "button",
                                    class: "player-caption-state-bridge",
                                    tabindex: "-1",
                                    aria_hidden: "true",
                                    "data-player-minimize": "",
                                    onclick: move |_| { spawn(async move { animated_navigate(Route::Feed {}).await; }); },
                                }
                                button {
                                    r#type: "button",
                                    class: "player-caption-state-bridge",
                                    tabindex: "-1",
                                    aria_hidden: "true",
                                    "data-player-caption-toggle": "",
                                    onclick: move |_| {
                                        if !captions_for_toggle().is_empty() {
                                            captions_enabled.toggle();
                                        }
                                    },
                                }
                                button {
                                    r#type: "button",
                                    class: "player-control-button player-speed-control",
                                    aria_label: "Playback speed",
                                    title: "Change playback speed",
                                    "data-player-action": "speed",
                                    span { "data-player-speed-label": "", "{current_speed}×" }
                                }
                                // Both icons ship; CSS shows whichever matches
                                // the current state, the same way play/pause does.
                                button {
                                    r#type: "button",
                                    class: "player-control-button player-fullscreen-button",
                                    aria_label: "Enter fullscreen",
                                    title: "Fullscreen",
                                    "data-player-action": "fullscreen",
                                    span { class: "player-fullscreen-enter-icon", Maximize2 { size: 21 } }
                                    span { class: "player-fullscreen-exit-icon", Minimize2 { size: 21 } }
                                }
                            }
                            div { class: "player-timeline",
                                div {
                                    class: "player-seek-preview",
                                    "data-player-seek-preview": "",
                                    hidden: true,
                                    div { class: "player-seek-preview-image", "data-player-preview-image": "" }
                                    div { class: "player-seek-preview-copy",
                                        strong { "data-player-preview-chapter": "" }
                                        span { "data-player-preview-time": "", "0:00" }
                                    }
                                }
                                div {
                                    class: "player-chapter-markers",
                                    "data-player-chapter-markers": "",
                                    aria_hidden: "true",
                                }
                                input {
                                    r#type: "range",
                                    class: "player-progress",
                                    min: "0",
                                    max: "10000",
                                    step: "1",
                                    value: "0",
                                    aria_label: "Video progress",
                                    "data-player-progress": "",
                                }
                            }
                        }
                    }
                } else if use_embed() {
                    iframe {
                        id: "tawny-player-frame",
                        src: "{fallback_url}",
                        title: "{video.title}",
                        allow: "accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture",
                        allowfullscreen: true,
                    }
                } else if resolving {
                    // The spinner already says "loading"; naming the mechanism
                    // only reads as something going wrong.
                    div { class: "player-stage-status",
                        div { class: "loading-orbit" }
                    }
                } else {
                    div { class: "player-stage-status player-stage-error",
                        if nothing_extracted {
                            p { "No playable stream could be extracted for this video. Check the server log for the extraction error." }
                        } else {
                            p { "Streams were found but playback failed." }
                        }
                        if !failure_detail().is_empty() {
                            code { class: "player-stage-detail", "{failure_detail()}" }
                        }
                        div { class: "player-stage-actions",
                            Button {
                                style: ButtonStyle::Solid,
                                onclick: move |_| {
                                    playback_failed.set(false);
                                    // Rewinding to zero both re-runs the
                                    // resolver and restores the retry budget.
                                    playback_attempt.set(0);
                                },
                                "Try again"
                            }
                            Button {
                                style: ButtonStyle::Neutral,
                                onclick: move |_| use_embed.set(true),
                                "Use YouTube embed"
                            }
                        }
                    }
                }
            }
            if !expanded {
                button {
                    class: "mini-player-copy",
                    aria_label: "Open {video.title}",
                    onclick: move |_| { { let v = open_video_id.clone(); spawn(async move { animated_navigate(Route::VideoDetail { id: v }).await; }); }; },
                    strong { "{video.title}" }
                    span { "{video.channel_name}" }
                }
                button {
                    class: "mini-player-close",
                    aria_label: "Close player",
                    onclick: move |_| app_state.stop_playback(),
                    X { size: 20 }
                }
            }
        }
    }
}

fn seek_player(seconds: u64) {
    spawn(async move {
        let script = format!(
            r#"
            const media = document.querySelector('#tawny-player video');
            if (media) {{ media.currentTime = {seconds}; media.play(); }}
            const frame = document.getElementById('tawny-player-frame');
            if (frame && frame.contentWindow) {{
                frame.contentWindow.postMessage(JSON.stringify({{
                    event: 'command', func: 'seekTo', args: [{seconds}, true]
                }}), '*');
            }}
            dioxus.send(true);
            "#
        );
        let mut eval = document::eval(&script);
        let _ = eval.recv::<bool>().await;
    });
}

fn toggle_picture_in_picture() {
    spawn(async move {
        let script = r#"
            const media = document.getElementById('tawny-player-media');
            let changed = false;
            try {
                if (document.pictureInPictureElement && document.exitPictureInPicture) {
                    await document.exitPictureInPicture();
                    changed = true;
                } else if (media && media.requestPictureInPicture) {
                    await media.requestPictureInPicture();
                    changed = true;
                } else if (media && media.webkitSupportsPresentationMode) {
                    const mode = media.webkitPresentationMode === 'picture-in-picture'
                        ? 'inline'
                        : 'picture-in-picture';
                    media.webkitSetPresentationMode(mode);
                    changed = true;
                }
            } catch (_) {}
            dioxus.send(changed);
        "#;
        let mut eval = document::eval(script);
        let _ = eval.recv::<bool>().await;
    });
}

fn scroll_video_page_to_top() {
    spawn(async move {
        // The shell's scroll container is `.g3-body-content`; its `.g3-body`
        // parent is `overflow: hidden`, so resetting the parent (or the window)
        // does nothing. That container outlives route changes, so it arrives on
        // the video page still holding the feed's offset.
        //
        // Content lands in stages — placeholder, cached details, then the
        // remote payload — and each stage can restore scroll height that the
        // browser had clamped away, so reassert the reset across a few frames
        // instead of once.
        let mut eval = document::eval(
            r#"
            const reset = () => {
                const containers = document.querySelectorAll(
                    '.g3-body-content, .g3-body, [data-g3-body], .tawny-shell'
                );
                for (const element of containers) {
                    if (element.scrollTop !== 0) element.scrollTop = 0;
                }
                if (document.scrollingElement) document.scrollingElement.scrollTop = 0;
                if (window.scrollY !== 0) window.scrollTo({ top: 0, left: 0, behavior: 'auto' });
            };
            reset();
            for (let frame = 0; frame < 4; frame++) {
                await new Promise((resolve) => requestAnimationFrame(resolve));
                reset();
            }
            for (const delay of [60, 180]) {
                await new Promise((resolve) => setTimeout(resolve, delay));
                reset();
            }
            dioxus.send(true);
            "#,
        );
        let _ = eval.recv::<bool>().await;
    });
}

fn compact_number(value: u64) -> String {
    let value = value as f64;
    if value >= 1_000_000_000.0 {
        format!("{:.1}B", value / 1_000_000_000.0)
    } else if value >= 1_000_000.0 {
        format!("{:.1}M", value / 1_000_000.0)
    } else if value >= 1_000.0 {
        format!("{:.1}K", value / 1_000.0)
    } else {
        format!("{}", value as u64)
    }
    .replace(".0", "")
}

fn local_details(video: Video, related_videos: Vec<Video>) -> VideoDetails {
    VideoDetails {
        video,
        channel: None,
        description: String::new(),
        like_count: 0,
        dislike_count: 0,
        captions: Vec::new(),
        chapters: Vec::new(),
        preview_frames: None,
        related_videos,
        comments: Default::default(),
        remote_available: false,
    }
}

fn source_is_transportable(source: &PlaybackSource) -> bool {
    if !source.request_headers.is_empty() {
        return false;
    }
    match source.protocol {
        PlaybackProtocol::Hls | PlaybackProtocol::Dash | PlaybackProtocol::Progressive => true,
        PlaybackProtocol::Sabr => {
            !source.tracks.is_empty()
                || source
                    .mime_type
                    .as_deref()
                    .is_some_and(|mime| mime.contains("dash+xml") || mime.contains("mpegurl"))
        }
        PlaybackProtocol::EmbedFallback => false,
    }
}


#[component]
pub fn VideoDetail(id: String) -> Element {
    rsx! { VideoDetailInner { key: "{id}", id } }
}

#[component]
fn VideoDetailInner(id: String) -> Element {
    let mut app_state = use_context::<AppState>();

    let mut description_expanded = use_signal(|| false);
    let mut chapters_open = app_state.chapters_sheet_open;
    let mut captions_open = use_signal(|| false);
    let mut comments_open = use_signal(|| false);
    let mut comments = use_signal(Vec::<VideoComment>::new);
    let mut comments_next_page = use_signal(|| None::<String>);
    let mut comments_initialized = use_signal(|| false);
    let mut comments_loading = use_signal(|| false);
    let mut details_cached = use_signal(|| false);
    let mut history_recorded = use_signal(|| false);
    let route_video_id = id.clone();
    use_effect(move || {
        let _ = &route_video_id;
        scroll_video_page_to_top();
    });

    let details_resource = {
        let video_id = id.clone();
        use_resource(move || {
            let video_id = video_id.clone();
            async move { get_video_details(video_id).await }
        })
    };
    let remote_details = details_resource
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .cloned();
    let details_to_cache = details_resource.clone();
    use_effect(move || {
        if details_cached() {
            return;
        }
        let details = details_to_cache
            .read()
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .cloned();
        if let Some(details) = details.as_ref() {
            app_state.cache_video_details(details);
            comments.set(details.comments.comments.clone());
            comments_next_page.set(details.comments.next_page.clone());
            comments_initialized.set(true);
            details_cached.set(true);
        }
    });

    let library = app_state.library();
    let cached_video = library.videos.iter().find(|video| video.id == id).cloned();
    let cached_related = library
        .videos
        .iter()
        .filter(|video| video.id != id)
        .take(8)
        .cloned()
        .collect::<Vec<_>>();
    let details = remote_details.clone().or_else(|| {
        cached_video
            .clone()
            .map(|video| local_details(video, cached_related))
    });
    let player_details_resource = details_resource.clone();
    let player_library = app_state.library;
    let player_video_id = id.clone();
    use_effect(move || {
        let remote = player_details_resource
            .read()
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .cloned();
        let details = remote.or_else(|| {
            let library = player_library();
            let video = library
                .videos
                .iter()
                .find(|video| video.id == player_video_id)
                .cloned()?;
            let related = library
                .videos
                .iter()
                .filter(|candidate| candidate.id != player_video_id)
                .take(8)
                .cloned()
                .collect();
            Some(local_details(video, related))
        });
        if let Some(details) = details {
            let video = &details.video;
            if app_state
                .active_video()
                .as_ref()
                .is_none_or(|active| active.id != video.id)
            {
                app_state.play(video.clone());
            }
            app_state.set_player_metadata(
                details.captions,
                details.chapters,
                details.preview_frames,
            );
            if !history_recorded() {
                app_state.record_history(&video.id);
                history_recorded.set(true);
            }
        }
    });

    let Some(details) = details else {
        let failed = details_resource.read().as_ref().is_some();
        return rsx! {
            main { class: "page player-loading",
                    div { class: "empty-state",
                        if failed {
                            p { "This video is unavailable from both the local cache and configured source." }
                        } else {
                            div { class: "loading-orbit" }
                        }
                    }
            }
        };
    };

    let video = details.video.clone();
    let channel = details.channel.clone().or_else(|| {
        library
            .channels
            .iter()
            .find(|channel| channel.id == video.channel_id)
            .cloned()
    });
    let related = if details.related_videos.is_empty() {
        library
            .videos
            .iter()
            .filter(|candidate| candidate.id != video.id)
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
    } else {
        details.related_videos.clone()
    };
    let watched_id = video.id.clone();
    let playlist_video = video.clone();
    let channel_id = video.channel_id.clone();
    let open_channel_id = video.channel_id.clone();
    let is_subscribed = channel
        .as_ref()
        .map(|channel| channel.subscribed)
        .unwrap_or(false);
    let caption_tracks = details.captions.clone();
    let chapters = details.chapters.clone();
    let comments_disabled = details.comments.disabled;
    let details_remote_available = details.remote_available;
    let description_class = if description_expanded() {
        "video-description expanded"
    } else {
        "video-description"
    };

    rsx! {
        // The sheet itself: this is what slides up over the shell and back
        // down off it, so it is what carries the cover snapshot.
        main { class: "player-content player-detail-content route-transition-cover",
                    section { class: "player-details page",
                        h1 { "{video.title}" }
                        p { class: "video-stats", "{video.stats_label()}" }

                        // Captions and chapters are chips that open their own
                        // sheet, so the page is not padded out with two panels
                        // that are mostly idle.
                        if details.like_count > 0 || details.dislike_count > 0 || !caption_tracks.is_empty() || !chapters.is_empty() {
                            div { class: "video-engagement",
                                if details.like_count > 0 {
                                    span { ThumbsUp { size: 15 } "{compact_number(details.like_count)}" }
                                }
                                if details.dislike_count > 0 {
                                    span { ThumbsDown { size: 15 } "{compact_number(details.dislike_count)}" }
                                }
                                if !caption_tracks.is_empty() {
                                    button {
                                        class: "engagement-chip",
                                        onclick: move |_| captions_open.set(true),
                                        Captions { size: 15 }
                                        "{caption_tracks.len()} captions"
                                    }
                                }
                                if !chapters.is_empty() {
                                    button {
                                        class: "engagement-chip",
                                        onclick: move |_| chapters_open.set(true),
                                        ListVideo { size: 15 }
                                        "{chapters.len()} chapters"
                                    }
                                }
                            }
                        }

                        div { class: "player-actions",
                            Button {
                                style: ButtonStyle::Neutral,
                                start: rsx! { Check { size: 17 } },
                                onclick: move |_| {
                                    app_state.mark_watched(&watched_id, true);
                                    app_state.show_toast("Marked as watched", StatusColor::Success);
                                },
                                "Watched"
                            }
                            Button {
                                style: ButtonStyle::Neutral,
                                start: rsx! { Clock { size: 17 } },
                                onclick: move |_| {
                                    app_state.playlist_picker_video.set(Some(playlist_video.clone()));
                                    app_state.playlist_picker_open.set(true);
                                },
                                "Save"
                            }
                            Button {
                                style: ButtonStyle::Neutral,
                                start: rsx! { Share2 { size: 17 } },
                                onclick: move |_| app_state.show_toast("Link ready to share", StatusColor::Neutral),
                                "Share"
                            }
                            Button {
                                style: ButtonStyle::Neutral,
                                start: rsx! { PictureInPicture { size: 17 } },
                                onclick: move |_| toggle_picture_in_picture(),
                                "PiP"
                            }
                        }

                        if let Some(channel) = channel {
                            article {
                                class: "player-channel-row",
                                role: "button",
                                tabindex: "0",
                                onclick: move |_| { { let v = open_channel_id.clone(); spawn(async move { animated_navigate(Route::ChannelDetail { id: v }).await; }); }; },
                                if let Some(avatar_url) = channel.avatar_url {
                                    img { class: "channel-avatar channel-avatar-medium", src: "{avatar_url}", alt: "" }
                                } else {
                                    div { class: "channel-avatar channel-avatar-medium", "{channel.name.chars().next().unwrap_or('T')}" }
                                }
                                div {
                                    h3 { "{channel.name}" }
                                    p { "{channel.subscriber_count} subscribers" }
                                }
                                Button {
                                    size: ButtonSize::Sm,
                                    style: if is_subscribed { ButtonStyle::Neutral } else { ButtonStyle::Solid },
                                    onclick: move |event: MouseEvent| {
                                        event.stop_propagation();
                                        app_state.toggle_subscription(&channel_id);
                                    },
                                    if is_subscribed { "Subscribed" } else { "Subscribe" }
                                }
                            }
                        }

                        if !details.description.is_empty() {
                            section { class: "detail-section description-panel",
                                div { class: "subsection-heading",
                                    h3 { "Description" }
                                    if !details_remote_available { span { "Cached" } }
                                }
                                p { class: "{description_class}", "{details.description}" }
                                button {
                                    class: "text-action",
                                    onclick: move |_| description_expanded.toggle(),
                                    if description_expanded() { "Show less" } else { "Show more" }
                                }
                            }
                        }

                        Button {
                            class: "comments-open-button",
                            style: ButtonStyle::Neutral,
                            expand: true,
                            disabled: comments_disabled,
                            start: rsx! { MessageSquare { size: 17 } },
                            onclick: move |_| comments_open.set(true),
                            if comments_disabled {
                                "Comments are disabled"
                            } else {
                                "Comments · {comments().len()}"
                            }
                        }

                        div { class: "section-heading compact",
                            div {
                                span { class: "section-kicker", "UP NEXT" }
                                h2 { if details.remote_available { "Related videos" } else { "From your library" } }
                            }
                        }
                        VideoGrid { videos: related }
                    }
        }

        // A small label, not a masthead: the list is the content, and a full
        // heading block was taking half the sheet before a chapter was visible.
        Sheet { is_open: chapters_open, class: "player-sheet",
            p { class: "sheet-label", "Chapters" }
            div { class: "chapter-sheet-list",
                for chapter in chapters.clone() {
                    {
                        let start_seconds = chapter.start_seconds;
                        rsx! {
                            button {
                                class: "chapter-sheet-row",
                                key: "{chapter.start_seconds}",
                                onclick: move |_| {
                                    seek_player(start_seconds);
                                    chapters_open.set(false);
                                },
                                strong { "{chapter.timestamp_label()}" }
                                span { "{chapter.title}" }
                            }
                        }
                    }
                }
            }
        }

        Sheet { is_open: captions_open, class: "player-sheet",
            p { class: "sheet-label", "Captions" }
            CaptionPicker {
                tracks: caption_tracks.clone(),
                selected: app_state.selected_caption,
            }
        }

        // No heading and no running count: the button that opened this already
        // said "Comments", so the sheet is just the list. Load-more lives at the
        // end of the scrolled list rather than pinned above it.
        Sheet { is_open: comments_open, class: "player-sheet comments-sheet",
            if comments().is_empty() {
                p { class: "detail-muted",
                    if comments_initialized() && !details.comments.remote_available {
                        "Comments need a connected video source."
                    } else {
                        "No comments are available."
                    }
                }
            } else {
                div { class: "comment-list",
                    for comment in comments() {
                        CommentCard { comment }
                    }
                    if let Some(next_page) = comments_next_page() {
                        Button {
                            class: "load-comments-button",
                            style: ButtonStyle::Neutral,
                            expand: true,
                            disabled: comments_loading(),
                            onclick: move |_| {
                                let video_id = video.id.clone();
                                let token = next_page.clone();
                                comments_loading.set(true);
                                spawn(async move {
                                    match get_comments_page(video_id, token).await {
                                        Ok(page) => {
                                            comments.write().extend(page.comments);
                                            comments_next_page.set(page.next_page);
                                        }
                                        Err(_) => app_state.show_toast(
                                            "Could not load more comments",
                                            StatusColor::Warning,
                                        ),
                                    }
                                    comments_loading.set(false);
                                });
                            },
                            if comments_loading() { "Loading…" } else { "Load more comments" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn CaptionPicker(tracks: Vec<CaptionTrack>, mut selected: Signal<Option<usize>>) -> Element {
    // Chips only. The sheet's own label already names this, and the section
    // heading it used to carry repeated that under a second title.
    rsx! {
        div { class: "caption-strip",
            for (index, track) in tracks.into_iter().enumerate() {
                button {
                    class: if selected() == Some(index) { "caption-chip active" } else { "caption-chip" },
                    onclick: move |_| {
                        selected.set(Some(index));
                    },
                    "{track.label}"
                    if track.auto_generated { small { "Auto" } }
                }
            }
        }
    }
}

#[component]
fn CommentCard(comment: VideoComment) -> Element {
    let initial = comment.author.chars().next().unwrap_or('T');
    rsx! {
        article { class: if comment.pinned { "comment-card pinned" } else { "comment-card" },
            if let Some(avatar_url) = comment.author_avatar_url {
                img { class: "comment-avatar", src: "{avatar_url}", alt: "", loading: "lazy" }
            } else {
                div { class: "comment-avatar comment-avatar-fallback", "{initial}" }
            }
            div { class: "comment-content",
                div { class: "comment-heading",
                    strong { "{comment.author}" }
                    if comment.creator { Badge { color: StatusColor::Accent, "Creator" } }
                    if comment.pinned { Badge { color: StatusColor::Neutral, "Pinned" } }
                    span { "{comment.published_at}" }
                }
                p { "{comment.text}" }
                div { class: "comment-stats",
                    span { ThumbsUp { size: 13 } "{compact_number(comment.like_count)}" }
                    if comment.reply_count > 0 { span { "{comment.reply_count} replies" } }
                    if comment.hearted { span { Heart { size: 13 } "Hearted" } }
                }
            }
        }
    }
}
