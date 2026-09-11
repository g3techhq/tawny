use crate::{
    api::{get_comments_page, get_sponsor_segments, get_video_details, resolve_playback},
    app::Route,
    models::{
        AudioTrackOption, CaptionTrack, PlaybackProtocol, PlaybackSession, PlaybackSource,
        SponsorAction, SponsorBlockSettings, SponsorSegment, Video, VideoChapter, VideoComment,
        VideoDetails, VideoPreviewFrames,
    },
    state::AppState,
};
use dioxus::prelude::*;
use dioxus_icons::lucide::{
    Captions, Check, ChevronLeft, ChevronRight, Clock, Heart, Languages, ListVideo, Maximize2,
    MessageSquare, Minimize2, Pause, PictureInPicture, Play, RotateCcw, RotateCw, Settings2,
    Share2, ThumbsDown, ThumbsUp, ToggleLeft, ToggleRight, User, Volume2, VolumeX, X,
};
use g3_route_transitions::{animated_go_back, animated_navigate};
use g3_ui::{Badge, Body, Button, ButtonSize, ButtonStyle, Sheet, StatusColor};

use super::VideoGrid;

const CONFIGURED_SERVER_URL: Option<&str> = option_env!("SERVER_URL");

fn playback_server_url() -> &'static str {
    if let Some(url) = CONFIGURED_SERVER_URL {
        return url;
    }
    if cfg!(target_os = "android") {
        "http://127.0.0.1:8080"
    } else {
        "http://localhost:8080"
    }
}

fn sync_player_metadata(
    tracks: Vec<CaptionTrack>,
    selected_caption: Option<usize>,
    captions_enabled: bool,
    chapters: Vec<VideoChapter>,
    preview_frames: Option<VideoPreviewFrames>,
    sponsor_segments: Vec<SponsorTimelineSegment>,
    sponsor_notify: bool,
    autoplay: bool,
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
    let Ok(sponsor_segments) = serde_json::to_string(&sponsor_segments) else {
        return;
    };
    let Ok(server_url) = serde_json::to_string(playback_server_url()) else {
        return;
    };
    let selected_caption = selected_caption
        .map(|index| index.to_string())
        .unwrap_or_else(|| "null".to_string());
    spawn(async move {
        let script = format!(
            r#"
            const rawTracks = {tracks};
            const chapters = {chapters};
            const previewFrames = {preview_frames};
            const sponsorSegments = {sponsor_segments};
            const sponsorNotify = {sponsor_notify};
            const autoplay = {autoplay};
            const serverUrl = {server_url};
            let media = document.getElementById('tawny-player-media');
            for (let attempt = 0; attempt < 800 && (!media || !window.TawnyPlayerControls || !window.TawnyTransport); attempt++) {{
                await new Promise((resolve) => setTimeout(resolve, 25));
                media = document.getElementById('tawny-player-media');
            }}
            if (media && window.TawnyPlayerControls && window.TawnyTransport) {{
                const options = {{ serverUrl }};
                const tracks = rawTracks.map((track) => ({{
                    ...track,
                    url: window.TawnyTransport.normalizePlaybackUrl(track.url, options),
                }}));
                const elements = media.querySelectorAll('track[data-tawny-caption]');
                tracks.forEach((track, index) => {{
                    const element = elements[index];
                    if (element && element.getAttribute('src') !== track.url) {{
                        element.setAttribute('src', track.url);
                    }}
                }});
                window.TawnyPlayerControls.setMetadata(media, {{
                    captions: tracks,
                    selectedCaption: {selected_caption},
                    captionsEnabled: {captions_enabled},
                    chapters,
                    previewFrames,
                    sponsorSegments,
                    sponsorNotify,
                    autoplay,
                }});
            }}
            dioxus.send(true);
            "#
        );
        let mut eval = document::eval(&script);
        let _ = eval.recv::<bool>().await;
    });
}

#[cfg(target_os = "android")]
#[component]
fn NativePlayerBridges(title: String) -> Element {
    let mut plugins = use_context::<g3_native_plugins::NativePlugins>();
    rsx! {
        button {
            r#type: "button", class: "player-caption-state-bridge", tabindex: "-1", aria_hidden: "true",
            "data-player-native-pip-landscape": "",
            onclick: move |_| { let _ = plugins.media.write().enter_picture_in_picture(16, 9); },
        }
        button {
            r#type: "button", class: "player-caption-state-bridge", tabindex: "-1", aria_hidden: "true",
            "data-player-native-pip-portrait": "",
            onclick: move |_| { let _ = plugins.media.write().enter_picture_in_picture(9, 16); },
        }
        button {
            r#type: "button", class: "player-caption-state-bridge", tabindex: "-1", aria_hidden: "true",
            "data-player-native-landscape": "",
            onclick: move |_| { let _ = plugins.media.write().set_orientation("landscape"); },
        }
        button {
            r#type: "button", class: "player-caption-state-bridge", tabindex: "-1", aria_hidden: "true",
            "data-player-native-orientation-unlock": "",
            onclick: move |_| { let _ = plugins.media.write().set_orientation("unspecified"); },
        }
        button {
            r#type: "button", class: "player-caption-state-bridge", tabindex: "-1", aria_hidden: "true",
            "data-player-native-playback-start": "",
            onclick: move |_| { let _ = plugins.media.write().set_playback_active(true, title.clone()); },
        }
        button {
            r#type: "button", class: "player-caption-state-bridge", tabindex: "-1", aria_hidden: "true",
            "data-player-native-playback-stop": "",
            onclick: move |_| { let _ = plugins.media.write().set_playback_active(false, String::new()); },
        }
    }
}

#[cfg(not(target_os = "android"))]
#[component]
fn NativePlayerBridges(title: String) -> Element {
    let _ = title;
    rsx! {}
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
                event.phase === 'shaka-error'
                || event.phase === 'source-failed'
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
                // A bare Shaka code says almost nothing. The status and the URI
                // it failed on are what separate a gated stream from a host the
                // device cannot reach, and there is no console on a handset.
                const code = last.code ? ` · code ${last.code}` : '';
                const status = last.httpStatus ? ` · HTTP ${last.httpStatus}` : '';
                const uri = last.uri ? ` · ${last.uri}` : '';
                parts.push(`${last.phase}${source}${code}${status}${at}${cause}${uri}`);
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

/// Switch the live audio language.
///
/// The transport owns the decision - Shaka rebuilds its variant list around the
/// chosen language - so this is a one-shot command rather than state Rust holds
/// and replays. The reporter below sends the result back.
fn set_player_audio_track(language: String) {
    spawn(async move {
        let script = format!(
            r#"
            const media = document.getElementById('tawny-player-media');
            if (media && window.TawnyTransport?.setAudioTrack) {{
                window.TawnyTransport.setAudioTrack(media, {language:?});
            }}
            dioxus.send(true);
            "#
        );
        let mut eval = document::eval(&script);
        let _ = eval.recv::<bool>().await;
    });
}

/// Resolves once the route transition has finished and the page is idle.
///
/// Anything the watch route renders costs real main-thread time, and spending
/// it while a transition is running is what starves that transition: the page
/// slide runs on the compositor and keeps its frames, while the player's morph
/// - which animates width and height, and so cannot - drops most of its own and
/// arrives late, catching up in one jump. Chromium schedules this differently
/// between builds, which is why the same page can look fine in one browser and
/// snap in another. Rendering a beat later costs a late-arriving row of
/// thumbnails and buys a transition that is smooth everywhere.
///
/// The related grid was the first thing held back here, but it was never the
/// only one: the two closed sheets build their whole lists regardless, and the
/// remote details land in the middle of the animation and re-render the body
/// under it. See `VideoDetailInner` for what each of them waits on.
const AFTER_ROUTE_TRANSITION_JS: &str = r#"
const frame = () => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
// Frames do not tick inside a view transition's update callback, but they do
// once its animations are running, so this settles exactly when they end.
let guard = 0;
while (document.documentElement.dataset.routeTransition && guard < 120) {
    guard += 1;
    await frame();
}
await frame();
dioxus.send(true);
"#;

/// Reports the transport's audio languages back to Rust.
///
/// Dubbed uploads carry a dozen or more, and which ones survive codec filtering
/// is a decision only the transport has made by this point - so the list is read
/// from it rather than from the extractor's track layout.
const AUDIO_TRACK_REPORTER_JS: &str = r#"
const key = Symbol.for('tawny.audio-track-reporter');
window[key]?.dispose?.();

const media = () => document.getElementById('tawny-player-media');
let last = '';
const send = () => {
    const element = media();
    if (!element || !window.TawnyTransport?.audioTrackState) return;
    const state = window.TawnyTransport.audioTrackState(element);
    // A single language is not a choice, so it is reported as none at all and
    // the chip stays away.
    const tracks = (state?.tracks || []).length > 1 ? state.tracks : [];
    const payload = [
        tracks.map((track) => [track.language, track.label]),
        tracks.length ? (state.active || '') : '',
    ];
    const encoded = JSON.stringify(payload);
    if (encoded === last) return;
    last = encoded;
    dioxus.send(payload);
};

// The list only exists once the manifest is parsed, and it changes again on a
// quality or language switch.
const timer = setInterval(send, 1000);
const onChange = () => send();
const element = media();
element?.addEventListener('tawnyaudiotrackchange', onChange);
element?.addEventListener('tawnytransportchange', onChange);
element?.addEventListener('loadedmetadata', onChange);
window[key] = {
    dispose() {
        clearInterval(timer);
        element?.removeEventListener('tawnyaudiotrackchange', onChange);
        element?.removeEventListener('tawnytransportchange', onChange);
        element?.removeEventListener('loadedmetadata', onChange);
    },
};
"#;

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
    audio_only: bool,
    resume_seconds: u64,
) {
    let Ok(session) = serde_json::to_string(&session) else {
        return;
    };
    let Ok(server_url) = serde_json::to_string(playback_server_url()) else {
        return;
    };
    let video_id = video_id.to_string();
    spawn(async move {
        let script = format!(
            r#"
            const session = {session};
            const videoId = {video_id:?};
            const serverUrl = {server_url};
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
                const serverBase = window.TawnyTransport.playbackServerBase({{ serverUrl }});
                const refreshPath = `/api/v1/playback/${{encodeURIComponent(videoId)}}?prefer_sabr={prefer_sabr}`;
                await window.TawnyTransport.attach(media, session, {{
                    playbackRate: {playback_rate},
                    videoId,
                    serverUrl,
                    audioOnly: {audio_only},
                    startTime: {resume_seconds},
                    refreshUrl: new URL(refreshPath, `${{serverBase}}/`).href,
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

/// Reports the media element's position back to Rust on a timer.
///
/// Each run replaces the previous registration outright: leaving the old timer
/// and listeners in place leaked one set per video change, and a stale one would
/// keep writing its own position over the video that replaced it.
const PROGRESS_REPORTER_JS: &str = r#"
const key = Symbol.for('tawny.progress-reporter');
window[key]?.dispose?.();

const media = () => document.getElementById('tawny-player-media');
const send = (flush) => {
    const element = media();
    if (element && Number.isFinite(element.currentTime)) {
        dioxus.send([Math.max(0, Math.floor(element.currentTime)), Boolean(flush)]);
    }
};

const timer = setInterval(() => send(false), 5000);
// The moments where losing the position would actually be felt.
const onPause = () => send(true);
const onHide = () => send(true);
// pagehide is not reliable when a mobile browser is merely backgrounded;
// visibility is, and it is the edge where a position is most often lost.
const onVisibility = () => { if (document.visibilityState !== 'visible') send(true); };
document.addEventListener('pause', onPause, true);
document.addEventListener('visibilitychange', onVisibility);
window.addEventListener('pagehide', onHide);
window.addEventListener('beforeunload', onHide);
window[key] = {
    dispose() {
        clearInterval(timer);
        document.removeEventListener('pause', onPause, true);
        document.removeEventListener('visibilitychange', onVisibility);
        window.removeEventListener('pagehide', onHide);
        window.removeEventListener('beforeunload', onHide);
    },
};
"#;

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
    // Persist where playback has reached. Polled rather than driven by
    // `timeupdate`, which fires several times a second and would drag a
    // re-render along with it.
    //
    // Declared before the early return below: a hook that only runs on renders
    // where a video happens to be active is a conditional hook, and it never
    // registers. Tracking the signal here also means the effect re-runs when the
    // video changes, which is what replaces the previous reporter.
    let progress_state = app_state;
    use_effect(move || {
        let Some(playing_id) = (progress_state.active_video)().map(|video| video.id) else {
            return;
        };
        spawn(async move {
            let mut eval = document::eval(PROGRESS_REPORTER_JS);
            let mut ticks: u32 = 0;
            while let Ok((seconds, flush)) = eval.recv::<(u64, bool)>().await {
                if !progress_state.record_progress(&playing_id, seconds) {
                    continue;
                }
                ticks += 1;
                // Local on every tick so the card updates as you watch; the
                // server every third - about fifteen seconds - plus whenever
                // the page says it is going away, the video changes, or
                // playback stops.
                if flush || ticks.is_multiple_of(3) {
                    progress_state.flush_progress();
                }
            }
        });
    });

    // Same shape as the progress reporter above, and declared before the same
    // early return for the same reason: a hook that only runs when a video
    // happens to be active never registers at all.
    let mut audio_state = app_state;
    use_effect(move || {
        if (audio_state.active_video)().is_none() {
            return;
        }
        spawn(async move {
            let mut eval = document::eval(AUDIO_TRACK_REPORTER_JS);
            while let Ok((tracks, active)) = eval.recv::<(Vec<(String, String)>, String)>().await {
                audio_state.active_audio_tracks.set(
                    tracks
                        .into_iter()
                        .map(|(language, label)| AudioTrackOption { language, label })
                        .collect(),
                );
                audio_state
                    .selected_audio_track
                    .set((!active.is_empty()).then_some(active));
            }
        });
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
    // Per-video preference, read here so a change re-attaches the transport with
    // (or without) the video stream rather than only hiding the picture.
    let attach_audio_only = video.audio_only;
    // Resume where this video was left, unless it already ran to the end - then
    // starting over is what replaying it means.
    let attach_resume_seconds = if video.watched {
        0
    } else {
        video.progress_seconds
    };
    let audio_only_active = video.audio_only;
    let audio_only_video_id = video.id.clone();
    let captions_to_sync = app_state.active_captions;
    let selected_caption_to_sync = app_state.selected_caption;
    let captions_enabled_to_sync = app_state.captions_enabled;
    let chapters_to_sync = app_state.active_chapters;
    let preview_frames_to_sync = app_state.active_preview_frames;
    let playback_attempt_to_sync = playback_attempt;
    let sponsor_segments_to_sync = app_state.active_sponsor_segments;
    let sponsor_settings_state = app_state;
    let autoplay_to_sync = app_state.settings().autoplay_for(is_short);
    use_effect(move || {
        // Re-send metadata when a failed stream creates a replacement video
        // element. The chapter/controller state is attached per element.
        let _ = playback_attempt_to_sync();
        let sponsor_settings = sponsor_settings_state.settings().sponsor_block;
        sync_player_metadata(
            captions_to_sync(),
            selected_caption_to_sync(),
            captions_enabled_to_sync(),
            chapters_to_sync(),
            preview_frames_to_sync(),
            timeline_segments(&sponsor_segments_to_sync(), &sponsor_settings),
            sponsor_settings.notify_on_skip,
            autoplay_to_sync,
        )
    });
    let mut captions_enabled = app_state.captions_enabled;
    let captions_for_toggle = app_state.active_captions;
    let caption_tracks_to_render = (app_state.active_captions)();
    let mut chapters_sheet = app_state.chapters_sheet_open;
    let open_video_id = video.id.clone();
    // The audio-only choice is baked into the manifest the transport builds, so
    // it only takes effect on attach. Keying the element on it forces a remount
    // and re-attach; without it the toggle changed state while the already-
    // attached video stream kept playing.
    let player_key = format!(
        "{}-{}-{}",
        video.id,
        playback_attempt(),
        if video.audio_only { "audio" } else { "av" }
    );
    let auto_landscape = app_state.settings().auto_landscape_fullscreen;
    let autoplay_enabled = app_state.settings().autoplay_for(is_short);
    let autoplay_video_id = video.id.clone();
    let mut autoplay_settings = app_state.settings;
    // A run is anything with somewhere to go: a queue ahead, a video stepped
    // back from, or both. Outside one there is nothing for the pair to do, and
    // a permanently dead control either side of play is worse than no control.
    let has_next_queued =
        app_state.with_library(|library| library.queue.iter().any(|id| id != &video.id));
    let can_step_back = app_state.can_step_back_in_run();
    let in_a_run = has_next_queued || can_step_back;
    let step_forward_id = video.id.clone();
    let step_back_id = video.id.clone();
    let playback_title = video.title.clone();

    rsx! {
        div { class: if expanded { "persistent-player expanded" } else { "persistent-player mini" },
            section {
                id: "tawny-player",
                // Dropping the video AdaptationSets covers the adaptive path, but a
                // progressive or HLS source has no separable video track to leave
                // out. Covering the surface with the artwork makes "audio only"
                // mean the same thing whichever transport ends up being used.
                class: if video.audio_only { "persistent-player-stage audio-only" } else { "persistent-player-stage" },
                "data-video-id": "{video.id}",
                "data-thumbnail": "{video.thumbnail_url}",
                // Feeds the audio-only artwork above; a CSS rule cannot reach the
                // thumbnail on its own.
                style: "--player-artwork: url('{video.thumbnail_url}');",
                "data-auto-landscape": auto_landscape.to_string(),
                if direct_stream {
                    video {
                        id: "tawny-player-media",
                        key: "{player_key}",
                        autoplay: true,
                        playsinline: true,
                        crossorigin: "anonymous",
                        poster: "{video.thumbnail_url}",
                        onmounted: move |_| {
                            if let Some(session) = session_to_attach.clone() {
                                attach_player_session(
                                    session,
                                    current_speed,
                                    &attach_video_id,
                                    prefer_sabr,
                                    attach_audio_only,
                                    attach_resume_seconds,
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
                                // Deliberately the same call the swipe-down
                                // makes: popping history instead picks its
                                // animation from whatever came before, which
                                // is not always the page under the sheet.
                                "data-player-action": "back",
                                ChevronLeft { size: 26 }
                            }
                            strong { class: "player-overlay-title", "{video.title}" }
                            div { class: "player-controls-top-actions",
                                // Autoplay is a per-sitting decision as often
                                // as a preference, so it is reachable without
                                // leaving the video. It writes the same setting
                                // the settings page does, split by kind: a run
                                // of Shorts and a long video are not the same
                                // choice.
                                button {
                                    r#type: "button",
                                    class: if autoplay_enabled {
                                        "player-control-button player-autoplay-button is-on"
                                    } else {
                                        "player-control-button player-autoplay-button"
                                    },
                                    aria_label: if is_short { "Autoplay Shorts" } else { "Autoplay videos" },
                                    title: if autoplay_enabled { "Autoplay is on" } else { "Autoplay is off" },
                                    aria_pressed: autoplay_enabled.to_string(),
                                    onclick: move |_| {
                                        autoplay_settings
                                            .write()
                                            .set_autoplay_for(is_short, !autoplay_enabled);
                                    },
                                    // A switch rather than a glyph: this button
                                    // reports a state as much as it invites a
                                    // tap, and the list icon it used to carry
                                    // said "queue" - which is the thing beside
                                    // it in the header.
                                    if autoplay_enabled {
                                        ToggleRight { size: 23 }
                                    } else {
                                        ToggleLeft { size: 23 }
                                    }
                                }
                                // Clicked by the player JS when a video ends.
                                // Advancing is decided here because the queue
                                // and the setting both live in Rust.
                                button {
                                    r#type: "button",
                                    class: "player-caption-state-bridge",
                                    tabindex: "-1",
                                    aria_hidden: "true",
                                    "data-player-autoplay-next": "",
                                    onclick: move |_| {
                                        let finished = autoplay_video_id.clone();
                                        spawn(async move {
                                            let Some(next) = app_state.take_next_queued(&finished) else {
                                                return;
                                            };
                                            animated_navigate(Route::VideoDetail { id: next }).await;
                                        });
                                    },
                                }
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
                        // Seeking has no buttons here on purpose - it is a
                        // double-tap on either side of the video, and the arrow
                        // keys on a keyboard - but moving through a queue is not
                        // seeking, and there is no gesture for it. The pair only
                        // appears when there is a run to move through.
                        div { class: "player-controls-center",
                            if in_a_run {
                                button {
                                    r#type: "button",
                                    class: "player-control-button player-step-button",
                                    aria_label: "Previous in queue",
                                    title: "Previous in queue",
                                    disabled: !can_step_back,
                                    onclick: move |_| {
                                        let current = step_back_id.clone();
                                        spawn(async move {
                                            let Some(previous) = app_state.step_back_in_run(&current) else {
                                                return;
                                            };
                                            animated_navigate(Route::VideoDetail { id: previous }).await;
                                        });
                                    },
                                    ChevronLeft { size: 27 }
                                }
                            }
                            button {
                                r#type: "button",
                                class: "player-control-button player-play-button",
                                aria_label: "Play",
                                title: "Play or pause",
                                "data-player-action": "toggle",
                                span { class: "player-play-icon", Play { size: 38, fill: "currentColor" } }
                                span { class: "player-pause-icon", Pause { size: 38, fill: "currentColor" } }
                            }
                            if in_a_run {
                                button {
                                    r#type: "button",
                                    class: "player-control-button player-step-button",
                                    aria_label: "Next in queue",
                                    title: "Next in queue",
                                    disabled: !has_next_queued,
                                    onclick: move |_| {
                                        let current = step_forward_id.clone();
                                        spawn(async move {
                                            let Some(next) = app_state.take_next_queued(&current) else {
                                                return;
                                            };
                                            animated_navigate(Route::VideoDetail { id: next }).await;
                                        });
                                    },
                                    ChevronRight { size: 27 }
                                }
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
                                strong { "Audio only" }
                                div { class: "player-option-grid",
                                    button {
                                        r#type: "button",
                                        class: if audio_only_active { "player-option-button selected" } else { "player-option-button" },
                                        onclick: move |_| {
                                            let id = audio_only_video_id.clone();
                                            app_state.set_audio_only(&id, !audio_only_active);
                                        },
                                        if audio_only_active { "On" } else { "Off" }
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
                        // Announces an automatic skip. Without it a video that
                        // jumps forward on its own reads as a fault.
                        div {
                            class: "player-sponsor-notice",
                            "data-player-sponsor-notice": "",
                            role: "status",
                            aria_live: "polite",
                        }
                        div { class: "player-controls-bottom",
                            div { class: "player-control-row",
                                span { class: "player-time",
                                    span { "data-player-current-time": "", "0:00" }
                                    span { class: "player-time-separator", " / " }
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
                                NativePlayerBridges { title: playback_title.clone() }
                                button {
                                    r#type: "button",
                                    class: "player-caption-state-bridge",
                                    tabindex: "-1",
                                    aria_hidden: "true",
                                    "data-player-minimize": "",
                                    // Minimizing dismisses the watch sheet, so it has to pop
                                    // history rather than push Feed: pushing sent anyone who
                                    // opened the video from Subscriptions or a playlist to the
                                    // wrong page, and left the watch route ahead in history.
                                    // Feed is only the deep-link fallback.
                                    onclick: move |_| { spawn(async move { animated_go_back(Route::Feed {}).await; }); },
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
                                size: ButtonSize::Sm,
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
            if (!changed) {
                if (media) media.__tawnyPlaybackIntent = !media.paused && !media.ended;
                window.dispatchEvent(new Event('tawnynativepiprequest'));
                const portrait = media && media.videoHeight > media.videoWidth;
                const selector = portrait
                    ? '#tawny-player [data-player-native-pip-portrait]'
                    : '#tawny-player [data-player-native-pip-landscape]';
                document.querySelector(selector)?.click();
                changed = Boolean(document.querySelector(selector));
            }
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

/// Splits the route param off from the page so that changing it builds a new
/// page rather than re-rendering the old one.
///
/// The key has to hang off a *list* to do anything. Dioxus only compares keys in
/// `diff_keyed_children`, which runs for siblings; a single keyed child goes down
/// the positional path where the key is never read, so the inner page kept its
/// scope across a watch-to-watch navigation and every `use_resource` and
/// `use_effect` in it went on serving the video it had captured at mount. The
/// address bar changed and nothing else did - by autoplay, by a related video,
/// by anything that moved between two watch pages.
///
/// A one-element loop is a keyed sibling list, so the key is compared, and a new
/// id unmounts the old page and mounts a fresh one.
#[component]
pub fn VideoDetail(id: String) -> Element {
    rsx! {
        for video_id in [id.clone()] {
            VideoDetailInner { key: "{video_id}", id: video_id.clone() }
        }
    }
}

#[component]
fn VideoDetailInner(id: String) -> Element {
    let mut app_state = use_context::<AppState>();

    let mut description_expanded = use_signal(|| false);
    let mut chapters_open = app_state.chapters_sheet_open;
    let mut captions_open = use_signal(|| false);
    let mut audio_open = use_signal(|| false);
    let audio_tracks = app_state.active_audio_tracks;
    let selected_audio = app_state.selected_audio_track;
    let mut comments_open = use_signal(|| false);
    // Everything this route can put off until the sheet has landed - see
    // AFTER_ROUTE_TRANSITION_JS.
    //
    // The player is the only thing in a cover transition whose two snapshots
    // differ in size, so its `::view-transition-group` animates width and
    // height while every other group is a pure translate. Width and height are
    // main-thread work: the sheet glides on the compositor whatever happens,
    // and the player alone starves on any long task this route runs while the
    // animation is in flight - which is why opening stutters and minimizing,
    // landing on a page that is already built, never does.
    //
    // So nothing renders here that the first 0.6s does not need.
    let mut transition_settled = use_signal(|| false);
    use_effect(move || {
        spawn(async move {
            let mut eval = document::eval(AFTER_ROUTE_TRANSITION_JS);
            if eval.recv::<bool>().await.is_ok() {
                transition_settled.set(true);
            }
        });
    });
    // SponsorBlock, fetched per video and per category set. Held behind the
    // same gate as everything else here: segments are not needed in the first
    // frame, and the request landing mid-morph is what starves it.
    let mut sponsor_segments = app_state.active_sponsor_segments;
    let sponsor_video_id = id.clone();
    use_effect(move || {
        if !transition_settled() {
            return;
        }
        let video_id = sponsor_video_id.clone();
        let categories = app_state
            .settings()
            .sponsor_block
            .requested_categories()
            .iter()
            .map(|category| category.api_name())
            .collect::<Vec<_>>()
            .join(",");
        spawn(async move {
            if categories.is_empty() {
                sponsor_segments.set(Vec::new());
                return;
            }
            // A failure here is not worth a toast: SponsorBlock being
            // unreachable means the video plays exactly as it would have.
            let fetched = get_sponsor_segments(video_id, categories)
                .await
                .unwrap_or_default();
            sponsor_segments.set(fetched);
        });
    });
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
    let library = app_state.library();
    let cached_video = library.videos.iter().find(|video| video.id == id).cloned();
    // Swapping the cached copy for the remote one re-renders the whole details
    // body, and opening from the mini player is exactly the case where the
    // cached copy is already correct. Waiting costs nothing visible: the page
    // is showing the right video throughout, and 0.6s later it quietly gets the
    // remote refinements. With nothing cached there is a loading state on
    // screen instead, and holding *that* back would be worse than the stutter.
    let hold_remote_details = cached_video.is_some() && !transition_settled();
    let remote_details = if hold_remote_details {
        None
    } else {
        details_resource
            .read()
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .cloned()
    };
    let details_to_cache = details_resource.clone();
    use_effect(move || {
        // This one always waits. It writes the library, which every subscriber
        // on this page reads, so landing it mid-morph re-renders the details
        // body even when `hold_remote_details` kept the swap itself back.
        if !transition_settled() || details_cached() {
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
        let local = {
            let library = player_library();
            library
                .videos
                .iter()
                .find(|video| video.id == player_video_id)
                .cloned()
                .map(|video| {
                    let related = library
                        .videos
                        .iter()
                        .filter(|candidate| candidate.id != player_video_id)
                        .take(8)
                        .cloned()
                        .collect();
                    local_details(video, related)
                })
        };
        // `set_player_metadata` re-renders the player - its caption `<track>`
        // children come from here - and during a cover transition that player
        // is the element the morph is painting live. So the remote copy waits
        // out the animation whenever the library already answered, on the same
        // terms as `hold_remote_details`. Nothing cached means nothing is
        // playing yet, and starting playback is worth more than a smooth 0.6s.
        let details = if local.is_some() && !transition_settled() {
            local
        } else {
            player_details_resource
                .read()
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .cloned()
                .or(local)
        };
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
            // Carries the cover marker too: the sheet has to exist in the
            // new DOM when the snapshot is taken, and details arrive later than
            // that. Without it the route committed first and the animation then
            // played over the page it had already swapped to.
            Body { padding: false,
            main { class: "page player-loading route-transition-cover",
                    div { class: "empty-state",
                        if failed {
                            p { "This video is unavailable from both the local cache and configured source." }
                        } else {
                            div { class: "loading-orbit" }
                        }
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
    let share_video = video.clone();
    let channel_id = video.channel_id.clone();
    let open_channel_id = video.channel_id.clone();
    let is_subscribed = channel
        .as_ref()
        .map(|channel| channel.subscribed)
        .unwrap_or(false);
    let caption_tracks = details.captions.clone();
    let chapters = details.chapters.clone();
    let comments_disabled = details.comments.disabled;
    // The seeding effect waits out the transition, so until it has run the
    // count comes from the details themselves rather than reading 0.
    let comments_count = if comments_initialized() {
        comments().len()
    } else {
        details.comments.comments.len()
    };
    let details_remote_available = details.remote_available;
    let description_class = if description_expanded() {
        "video-description expanded"
    } else {
        "video-description"
    };

    rsx! {
        // The sheet itself: this is what slides up over the shell and back
        // down off it, so it is what carries the cover snapshot.
        Body { padding: false,
        main { class: "player-content player-detail-content route-transition-cover",
                    section { class: "player-details page",
                        h1 { "{video.title}" }
                        p { class: "video-stats", "{video.stats_label()}" }

                        // Captions and chapters are chips that open their own
                        // sheet, so the page is not padded out with two panels
                        // that are mostly idle.
                        if details.like_count > 0 || details.dislike_count > 0 || !caption_tracks.is_empty() || !chapters.is_empty() || audio_tracks().len() > 1 {
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
                                // Only a dubbed upload offers a choice here, and only
                                // then is the chip worth a slot in this row.
                                if audio_tracks().len() > 1 {
                                    button {
                                        class: "engagement-chip",
                                        onclick: move |_| audio_open.set(true),
                                        Languages { size: 15 }
                                        "{audio_track_label(&audio_tracks(), &selected_audio())}"
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
                                size: ButtonSize::Sm,
                                start: rsx! { Check { size: 17 } },
                                onclick: move |_| {
                                    app_state.mark_watched(&watched_id, true);
                                    app_state.show_toast("Marked as watched", StatusColor::Success);
                                },
                                "Watched"
                            }
                            Button {
                                style: ButtonStyle::Neutral,
                                size: ButtonSize::Sm,
                                start: rsx! { Clock { size: 17 } },
                                onclick: move |_| {
                                    app_state.playlist_picker_video.set(Some(playlist_video.clone()));
                                    app_state.playlist_picker_open.set(true);
                                },
                                "Save"
                            }
                            Button {
                                style: ButtonStyle::Neutral,
                                size: ButtonSize::Sm,
                                start: rsx! { Share2 { size: 17 } },
                                onclick: move |_| app_state.open_share(share_video.clone()),
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
                                    div { class: "channel-avatar channel-avatar-medium channel-avatar-fallback", User { size: 20 } }
                                }
                                div {
                                    h3 { "{channel.name}" }
                                    if !channel.subscriber_count.trim().is_empty() {
                                        p { "{channel.subscriber_count} subscribers" }
                                    }
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
                                "Comments · {comments_count}"
                            }
                        }

                        div { class: "section-heading compact",
                            div {
                                span { class: "section-kicker", "UP NEXT" }
                                h2 { if details.remote_available { "Related videos" } else { "From your library" } }
                            }
                        }
                        if transition_settled() {
                            VideoGrid { videos: related }
                        }
                    }
        }

        // A small label, not a masthead: the list is the content, and a full
        // heading block was taking half the sheet before a chapter was visible.
        Sheet { is_open: chapters_open, class: "player-sheet",
            p { class: "sheet-label", "Chapters" }
            div { class: "chapter-sheet-list",
                // A closed sheet still renders its list. This one cannot be
                // open while the route that owns it is animating in, so it is
                // pure cost there - and a long upload has a hundred rows.
                for chapter in if transition_settled() { chapters.clone() } else { Vec::new() } {
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

        Sheet { is_open: audio_open, class: "player-sheet",
            p { class: "sheet-label", "Audio track" }
            AudioTrackPicker {
                tracks: audio_tracks(),
                selected: selected_audio(),
            }
        }

        // No heading and no running count: the button that opened this already
        // said "Comments", so the sheet is just the list. Load-more lives at the
        // end of the scrolled list rather than pinned above it.
        Sheet { is_open: comments_open, class: "player-sheet comments-sheet",
            // Same as the chapter list, and the heaviest of the two: twenty
            // cards, each with an avatar, built while the player is trying to
            // grow.
            if !transition_settled() {
                p { class: "detail-muted", "Loading comments…" }
            } else if comments().is_empty() {
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
}

/// What the chip says: the language you are hearing, not a count.
///
/// A count would be the wrong fact here - the question a dubbed video raises is
/// "which language is this?", and the answer is the thing to show.
fn audio_track_label(tracks: &[AudioTrackOption], selected: &Option<String>) -> String {
    selected
        .as_ref()
        .and_then(|language| tracks.iter().find(|track| &track.language == language))
        .map(|track| track.label.clone())
        .unwrap_or_else(|| "Audio".to_string())
}

#[component]
fn AudioTrackPicker(tracks: Vec<AudioTrackOption>, selected: Option<String>) -> Element {
    rsx! {
        div { class: "caption-strip",
            for track in tracks {
                button {
                    key: "{track.language}",
                    class: if selected.as_deref() == Some(track.language.as_str()) { "caption-chip active" } else { "caption-chip" },
                    onclick: {
                        let language = track.language.clone();
                        move |_| set_player_audio_track(language.clone())
                    },
                    "{track.label}"
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

/// A segment in the shape the player JS wants.
///
/// Deliberately not `SponsorSegment` itself: the JS needs the resolved colour,
/// the resolved action and the message to show, none of which are properties of
/// the segment - they come from the viewer's settings. Resolving them here
/// keeps that decision in Rust, where the settings live, instead of shipping
/// the settings to JS and duplicating the logic.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
struct SponsorTimelineSegment {
    uuid: String,
    start_seconds: f64,
    end_seconds: f64,
    color: &'static str,
    /// `"skip"` or `"show"`. Ignored categories never reach here.
    action: &'static str,
    message: &'static str,
    label: &'static str,
}

/// Resolve segments against the viewer's settings, dropping the ignored ones.
fn timeline_segments(
    segments: &[SponsorSegment],
    settings: &SponsorBlockSettings,
) -> Vec<SponsorTimelineSegment> {
    segments
        .iter()
        .filter_map(|segment| {
            let action = match settings.action_for(segment.category) {
                SponsorAction::Off => return None,
                SponsorAction::Skip => "skip",
                SponsorAction::Show => "show",
            };
            Some(SponsorTimelineSegment {
                uuid: segment.uuid.clone(),
                start_seconds: segment.start_seconds,
                end_seconds: segment.end_seconds,
                color: segment.category.color(),
                action,
                message: segment.category.skip_message(),
                label: segment.category.label(),
            })
        })
        .collect()
}

#[cfg(test)]
mod timeline_tests {
    use super::*;
    // Imported here rather than at file scope: the non-test build resolves
    // categories on the server side only, so a top-level import is dead there.
    use crate::models::SponsorCategory;

    fn segment(category: SponsorCategory, start: f64, end: f64) -> SponsorSegment {
        SponsorSegment {
            uuid: format!("{category:?}"),
            category,
            start_seconds: start,
            end_seconds: end,
            locked: false,
            votes: 0,
        }
    }

    #[test]
    fn an_ignored_category_never_reaches_the_player() {
        let mut settings = SponsorBlockSettings::default();
        settings.set_action(SponsorCategory::Sponsor, SponsorAction::Off);
        let resolved = timeline_segments(
            &[
                segment(SponsorCategory::Sponsor, 0.0, 10.0),
                segment(SponsorCategory::Intro, 10.0, 20.0),
            ],
            &settings,
        );
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].action, "show");
    }

    #[test]
    fn the_action_the_player_gets_is_the_viewers_choice() {
        let mut settings = SponsorBlockSettings::default();
        settings.set_action(SponsorCategory::Intro, SponsorAction::Skip);
        let resolved = timeline_segments(&[segment(SponsorCategory::Intro, 0.0, 5.0)], &settings);
        assert_eq!(resolved[0].action, "skip");
        assert_eq!(resolved[0].color, SponsorCategory::Intro.color());
    }

    #[test]
    fn turning_sponsorblock_off_empties_the_timeline() {
        let settings = SponsorBlockSettings {
            enabled: false,
            ..SponsorBlockSettings::default()
        };
        assert!(
            timeline_segments(&[segment(SponsorCategory::Sponsor, 0.0, 10.0)], &settings)
                .is_empty()
        );
    }
}

#[cfg(test)]
mod route_tests {
    /// A string check, because the thing being guarded is a diffing rule rather
    /// than anything this crate can call: Dioxus compares keys only in
    /// `diff_keyed_children`, which runs for a list of siblings. Written as a
    /// single keyed child the key is never read, the page keeps its scope across
    /// a watch-to-watch navigation, and every hook in it goes on serving the
    /// video it captured at mount. Nothing errors - the address bar changes and
    /// the page does not - so only the shape of the call can defend it.
    #[test]
    fn the_watch_page_is_keyed_through_a_sibling_list() {
        let source = include_str!("player.rs");
        let body = source
            .split("pub fn VideoDetail(id: String) -> Element {")
            .nth(1)
            .expect("the route component");
        let body = body.split("#[component]").next().expect("its body");
        assert!(
            body.contains("for video_id in [id.clone()]"),
            "VideoDetailInner must be keyed inside a list or it never remounts: {body}"
        );
    }
}
