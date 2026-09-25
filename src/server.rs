use crate::models::{
    CaptionTrack, Channel, ChannelDetails, ChannelMediaPage, ChannelMediaTab, CommentsPage,
    FeedRefreshResult, HistoryEntry, LibrarySnapshot, LibraryUserState, PlaybackByteRange,
    PlaybackProtocol, PlaybackSession, PlaybackSource, PlaybackTrack, PlaybackTrackKind, Playlist,
    SearchResults, SponsorCategory, SponsorSegment, SubscriptionContent, SubscriptionGroup, Video,
    VideoChapter, VideoComment, VideoDetails, VideoPreviewFrames, VideoProgress,
};
use anyhow::{Context, Result, anyhow};
use axum::{
    Extension,
    body::{Body, Bytes},
    extract::{Path, Query},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::Response,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use dioxus::fullstack::FullstackContext;
use futures_util::FutureExt;
use hmac::{Hmac, Mac};
use quick_xml::{Reader, events::Event};
use rustypipe::{
    client::{ClientType, RustyPipe},
    model::{
        Channel as RustyChannel, ChannelItem as RustyChannelItem,
        ChannelRssVideo as RustyChannelRssVideo, Comment as RustyComment,
        VideoDetails as RustyVideoDetails, VideoItem as RustyVideoItem,
        VideoPlayer as RustyVideoPlayer, YouTubeItem, paginator::Paginator, richtext::ToPlaintext,
    },
    param::ChannelVideoTab,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use std::future::Future;
use std::{
    collections::{HashMap, HashSet, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use surrealdb::{
    Surreal,
    engine::any::{Any, connect},
    opt::auth::Root,
};
use surrealdb_types::SurrealValue;

#[derive(Clone)]
pub struct AppServerState {
    pub(crate) db: Arc<Surreal<Any>>,
    http: reqwest::Client,
    media_http: reqwest::Client,
    ytdlp_http: reqwest::Client,
    youtube: Arc<RustyPipe>,
    /// Optional containerized yt-dlp HTTP sidecar. When configured, Tawny does
    /// not discover or execute a host yt-dlp binary.
    ytdlp_service_url: Option<String>,
    /// Path to a `yt-dlp` binary used solely to obtain playback stream URLs.
    ytdlp_bin: Option<std::path::PathBuf>,
    /// Optional bgutil HTTP provider used by yt-dlp for video-bound PO tokens.
    ytdlp_po_provider_url: Option<String>,
    public_url: String,
    websub_callback_url: Option<String>,
    websub_secret: String,
    proxy_targets: Arc<tokio::sync::RwLock<HashMap<String, ProxyTarget>>>,
    proxy_counter: Arc<AtomicU64>,
    /// Resolved playback, keyed by video id. See [`AppServerState::playback_session`].
    playback_sessions: Arc<std::sync::Mutex<HashMap<String, CachedPlayback>>>,
    sync_lock: Arc<tokio::sync::Mutex<()>>,
    search_lock: Arc<tokio::sync::Mutex<()>>,
    search_cache: Arc<tokio::sync::RwLock<HashMap<(String, String), CachedSearch>>>,
    /// Keyed by video id and the category set asked for, because those are
    /// different answers for the same video.
    sponsor_cache: Arc<
        tokio::sync::RwLock<HashMap<(String, String), (std::time::Instant, Vec<SponsorSegment>)>>,
    >,
    /// A self-hosted SponsorBlock mirror, when one is configured.
    sponsor_api_url: Option<String>,
}

#[derive(Clone, Debug)]
struct ProxyTarget {
    url: String,
    request_headers: Vec<crate::models::PlaybackRequestHeader>,
    expires_at: std::time::Instant,
}

/// One video's playback resolve, shared by everyone who asks for it.
///
/// The session is held *before* it is proxied: proxy registrations are minted
/// per hand-out, so a session taken from here gets their full lifetime however
/// long it sat waiting.
#[derive(Clone)]
struct CachedPlayback {
    started_at: std::time::Instant,
    resolve: SharedPlaybackResolve,
}

type SharedPlaybackResolve = futures_util::future::Shared<
    futures_util::future::BoxFuture<'static, Result<ResolvedPlayback, String>>,
>;

#[derive(Clone)]
struct ResolvedPlayback {
    session: PlaybackSession,
    /// Whether this is worth handing out again. Only a yt-dlp source is:
    /// without one the session is either the embed or extractor URLs that stop
    /// serving part way through, and a later ask deserves a fresh attempt.
    reusable: bool,
}

/// How long a resolved session is handed out again.
///
/// Long enough that the video queued behind an ordinary upload is still warm
/// when it comes up. The bound is the googlevideo URLs, which expire after
/// roughly six hours; proxy registrations are minted per hand-out and do not
/// count against it.
const PLAYBACK_SESSION_TTL: Duration = Duration::from_secs(60 * 60);
/// Plenty for every client's current and next video.
const PLAYBACK_SESSION_LIMIT: usize = 64;

#[derive(Clone)]
struct CachedSearch {
    result: SearchResults,
    cached_at: std::time::Instant,
}

const FEED_RECONCILE_BATCH_SIZE: usize = 48;
const FEED_RSS_CONCURRENCY: usize = 32;
/// How many channels one refresh will ask for video lengths, at most.
///
/// One call covers a channel's recent uploads, so this is the number of extra
/// Innertube requests a pull-to-refresh can cost. The subscription feed has no
/// runtimes, so this covers enough of the recent backlog for duration filters
/// to work before those videos are opened.
const FEED_DURATION_BACKFILL_CHANNELS: usize = 24;
/// How many rows to look at when deciding which channels those are.
const FEED_DURATION_SCAN_ROWS: usize = 600;
/// Exact player lookups are reserved for the cards a viewer can actually see.
/// One page is 24 cards; the API cap allows a loaded second page in one call
/// without turning the endpoint into an unbounded extractor proxy.
const FEED_DURATION_VISIBLE_BATCH: usize = 24;
/// Hard cap on one on-screen hydration request, from any page. Mirrored by the
/// client's look-ahead so a duration filter can ask about a page's worth of
/// cards plus the ones that will not land in the selected bucket.
const DURATION_HYDRATION_LIMIT: usize = 48;
const WEBSUB_RENEW_CONCURRENCY: usize = 12;
/// Channel pages requested at once while backfilling avatars and subscriber
/// counts. Each is a full channel extraction, so this stays well below the
/// WebSub figure, which is a single small POST each.
const CHANNEL_METADATA_CONCURRENCY: usize = 4;
/// Channel pages one backfill pass may request.
const CHANNEL_METADATA_BATCH: usize = 100;
const SEARCH_CACHE_TTL: Duration = Duration::from_secs(5 * 60);

fn reconciliation_limit(total: usize, has_accelerated_source: bool) -> usize {
    if has_accelerated_source {
        total.min(FEED_RECONCILE_BATCH_SIZE)
    } else {
        total
    }
}

/// Innertube clients to try, in order, for player extraction.
///
/// Pinned rather than using rustypipe's `player()` default, because that
/// default is environment-dependent: it puts `Desktop` first whenever a
/// `rustypipe-botguard` binary happens to be on PATH. Every client except iOS
/// needs signature deobfuscation, which rustypipe 0.11.4 can no longer do
/// against the current player JS, so a `Desktop` attempt fails outright and
/// never falls through to the client that works. Installing an unrelated tool
/// should not be able to break playback.
const PLAYER_CLIENTS: &[ClientType] = &[ClientType::Ios, ClientType::Tv];

/// Locate a `yt-dlp` binary, honouring `TAWNY_YTDLP_BIN` first.
fn locate_ytdlp() -> Option<std::path::PathBuf> {
    if let Ok(configured) = std::env::var("TAWNY_YTDLP_BIN")
        && !configured.trim().is_empty()
    {
        let path = std::path::PathBuf::from(configured.trim());
        if path.exists() {
            return Some(path);
        }
        eprintln!("TAWNY_YTDLP_BIN does not exist: {}", path.display());
        return None;
    }
    // Fall back to PATH. `--version` is the cheapest way to confirm it runs.
    let name = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    std::process::Command::new(name)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()
        .filter(|status| status.success())
        .map(|_| std::path::PathBuf::from(name))
}

fn configured_ytdlp_po_provider_url() -> Option<String> {
    configured_http_url("TAWNY_PO_TOKEN_PROVIDER_URL")
}

fn configured_ytdlp_service_url() -> Option<String> {
    configured_http_url("TAWNY_YTDLP_SERVICE_URL")
}

fn configured_http_url(variable: &str) -> Option<String> {
    let configured = std::env::var(variable).ok()?;
    let configured = configured.trim().trim_end_matches('/');
    if configured.is_empty() {
        return None;
    }
    let valid = reqwest::Url::parse(configured)
        .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some());
    if !valid {
        eprintln!("ignoring {variable}: expected an absolute http(s) URL");
        return None;
    }
    Some(configured.to_string())
}

fn ytdlp_service_video_url(service_url: &str, video_id: &str) -> Option<reqwest::Url> {
    let mut url = reqwest::Url::parse(service_url).ok()?;
    url.path_segments_mut()
        .ok()?
        .extend(["v1", "videos", video_id]);
    Some(url)
}

fn ytdlp_service_channel_shorts_url(service_url: &str, channel_id: &str) -> Option<reqwest::Url> {
    let mut url = reqwest::Url::parse(service_url).ok()?;
    url.path_segments_mut()
        .ok()?
        .extend(["v1", "channels", channel_id, "shorts"]);
    Some(url)
}

fn ytdlp_po_provider_args(provider_url: Option<&str>) -> Vec<String> {
    let Some(provider_url) = provider_url else {
        return Vec::new();
    };
    vec![
        "--extractor-args".into(),
        format!("youtubepot-bgutilhttp:base_url={provider_url}"),
        "--extractor-args".into(),
        "youtube:player_client=mweb".into(),
    ]
}

/// A stream URL yt-dlp extracted, keyed by itag.
#[derive(Debug, Deserialize)]
struct YtdlpFormat {
    format_id: String,
    url: Option<String>,
    #[serde(default)]
    filesize: Option<u64>,
    #[serde(default)]
    filesize_approx: Option<u64>,
    #[serde(default)]
    ext: Option<String>,
    #[serde(default)]
    container: Option<String>,
    #[serde(default)]
    vcodec: Option<String>,
    #[serde(default)]
    acodec: Option<String>,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default)]
    fps: Option<f64>,
    #[serde(default)]
    tbr: Option<f64>,
    #[serde(default)]
    format_note: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    has_drm: Option<bool>,
    #[serde(default)]
    protocol: Option<String>,
    /// The master playlist an HLS format belongs to. Every m3u8 format of one
    /// extraction shares it.
    #[serde(default)]
    manifest_url: Option<String>,
    /// Filled in from the dump's top-level duration, which is where yt-dlp
    /// reports it: the per-format entries carry none, and a DASH manifest
    /// without one presents as a zero-length video.
    #[serde(skip)]
    duration_ms: Option<u64>,
}

impl YtdlpFormat {
    /// The itag, which is the format id up to any `-drc` style suffix.
    fn itag(&self) -> Option<u32> {
        self.format_id.split('-').next()?.parse().ok()
    }

    fn codec(value: &Option<String>) -> Option<&str> {
        value
            .as_deref()
            .filter(|codec| !codec.is_empty() && *codec != "none")
    }

    fn video_codec(&self) -> Option<&str> {
        Self::codec(&self.vcodec)
    }

    fn audio_codec(&self) -> Option<&str> {
        Self::codec(&self.acodec)
    }

    /// Adaptive formats carry exactly one of the two streams.
    fn adaptive_kind(&self) -> Option<PlaybackTrackKind> {
        match (self.video_codec(), self.audio_codec()) {
            (Some(_), None) => Some(PlaybackTrackKind::Video),
            (None, Some(_)) => Some(PlaybackTrackKind::Audio),
            _ => None,
        }
    }

    /// A DASH-ready container, as opposed to the muxed progressive formats and
    /// the m3u8 entries that only appear on live streams.
    fn is_dash(&self) -> bool {
        self.container
            .as_deref()
            .is_some_and(|container| container.ends_with("_dash"))
    }

    /// `video/mp4; codecs="avc1.4d400c"`, assembled the way a DASH manifest
    /// wants it. WebM and MP4 are the only containers YouTube serves adaptive.
    fn mime_type(&self, kind: PlaybackTrackKind) -> Option<String> {
        let subtype = match self.ext.as_deref() {
            Some("mp4") | Some("m4a") => "mp4",
            Some("webm") => "webm",
            _ => return None,
        };
        let top = match kind {
            PlaybackTrackKind::Video => "video",
            PlaybackTrackKind::Audio => "audio",
        };
        let codec = self.video_codec().or_else(|| self.audio_codec())?;
        Some(format!("{top}/{subtype}; codecs=\"{codec}\""))
    }

    fn content_length(&self) -> Option<u64> {
        self.filesize.or(self.filesize_approx)
    }
}

#[derive(Debug, Deserialize)]
struct YtdlpDump {
    #[serde(default)]
    formats: Vec<YtdlpFormat>,
    #[serde(default)]
    duration: Option<f64>,
}

fn formats_from_ytdlp_dump(mut dump: YtdlpDump) -> Vec<YtdlpFormat> {
    let duration_ms = dump
        .duration
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
        .map(|seconds| (seconds * 1000.0).round() as u64);
    for format in &mut dump.formats {
        format.duration_ms = duration_ms;
    }
    dump.formats
}

#[derive(Debug, Deserialize)]
struct YtdlpFlatEntry {
    #[serde(default)]
    id: String,
    title: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    view_count: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct YtdlpFlatPlaylist {
    #[serde(default)]
    entries: Vec<YtdlpFlatEntry>,
}

fn ytdlp_flat_playlist_videos(
    dump: YtdlpFlatPlaylist,
    channel_id: &str,
    channel_name: &str,
) -> Vec<Video> {
    dump.entries
        .into_iter()
        .filter(|entry| !entry.id.is_empty())
        .map(|entry| Video {
            thumbnail_url: format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", entry.id),
            id: entry.id,
            title: entry.title.unwrap_or_default(),
            channel_id: channel_id.to_string(),
            channel_name: channel_name.to_string(),
            published_at: String::new(),
            duration_seconds: entry.duration.unwrap_or(0.0) as u64,
            view_count: entry
                .view_count
                .map(|count| compact_count(count, " views"))
                .unwrap_or_default(),
            progress_seconds: 0,
            watched: false,
            is_live: false,
            is_short: true,
            audio_only: false,
        })
        .collect()
}

async fn youtube_call<T, E, F>(future: F, operation: &str) -> Result<T>
where
    E: std::fmt::Display,
    F: Future<Output = std::result::Result<T, E>>,
{
    match std::panic::AssertUnwindSafe(future).catch_unwind().await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(anyhow!("{operation}: {error}")),
        Err(_) => Err(anyhow!("{operation}: the extractor aborted its request")),
    }
}

impl dioxus::fullstack::axum_core::extract::FromRef<FullstackContext> for AppServerState {
    fn from_ref(state: &FullstackContext) -> Self {
        state
            .extension::<AppServerState>()
            .expect("Tawny server state")
    }
}

#[derive(Debug, Deserialize, SurrealValue)]
struct DbChannel {
    channel_id: String,
    name: String,
    handle: String,
    avatar_url: Option<String>,
    subscriber_count: String,
    subscribed: bool,
    /// Optional because rows written before this column existed read back as
    /// NONE, and `SurrealValue` does not honour `#[serde(default)]`. The
    /// startup backfill fills them in, but a row can still be read
    /// during the same startup that adds the column.
    subscription_content: Option<String>,
    description: String,
    banner_url: Option<String>,
}

/// One account's opinion of one channel.
#[derive(Debug, Deserialize, SurrealValue)]
struct DbUserSubscription {
    channel_id: String,
    subscribed: bool,
    /// Optional for the same reason as `DbChannel::subscription_content`.
    content: Option<String>,
}

/// One account's place in one video.
#[derive(Debug, Deserialize, SurrealValue)]
struct DbVideoProgress {
    video_id: String,
    watched: bool,
    progress_seconds: i64,
    audio_only: bool,
}

#[derive(Debug, Deserialize, SurrealValue)]
struct DbFollowerCount {
    count: i64,
}

#[derive(Debug, Deserialize, SurrealValue)]
struct DbSubscriptionFlag {
    subscribed: bool,
}

#[derive(Debug, Deserialize, SurrealValue)]
struct DbSubscriptionState {
    channel_id: String,
    subscribed: bool,
}

#[derive(Debug, Deserialize, SurrealValue)]
struct DbVideo {
    video_id: String,
    title: String,
    channel_id: String,
    channel_name: String,
    thumbnail_url: String,
    published_at: String,
    #[allow(dead_code)]
    published_sort: String,
    duration_seconds: i64,
    view_count: String,
    is_live: bool,
    is_short: bool,
    progress_seconds: i64,
    watched: bool,
    /// Optional for the same reason as `subscription_content`: rows written
    /// before the column existed read back as NONE, and `SurrealValue` does not
    /// honour `#[serde(default)]`.
    audio_only: Option<bool>,
}

#[derive(Debug, Deserialize, SurrealValue)]
struct DbPlaylist {
    playlist_id: String,
    name: String,
    video_ids: Vec<String>,
}

#[derive(Debug, Deserialize, SurrealValue)]
struct DbSubscriptionGroup {
    group_id: String,
    name: String,
    channel_ids: Vec<String>,
}

#[derive(Debug, Deserialize, SurrealValue)]
struct DbLibraryState {
    cache_revision: i64,
    queue_json: String,
    history_json: String,
}

#[derive(Debug, Deserialize, SurrealValue)]
struct DbVideoDetailCache {
    payload_json: String,
}

impl From<DbChannel> for Channel {
    fn from(value: DbChannel) -> Self {
        Self {
            id: value.channel_id,
            name: value.name,
            handle: value.handle,
            avatar_url: value.avatar_url,
            subscriber_count: value.subscriber_count,
            subscribed: value.subscribed,
            description: value.description,
            banner_url: value.banner_url,
            subscription_content: SubscriptionContent::from_storage(
                value.subscription_content.as_deref().unwrap_or_default(),
            ),
        }
    }
}

impl From<DbVideo> for Video {
    fn from(value: DbVideo) -> Self {
        Self {
            id: value.video_id,
            title: value.title,
            channel_id: value.channel_id,
            channel_name: value.channel_name,
            thumbnail_url: value.thumbnail_url,
            published_at: value.published_at,
            duration_seconds: value.duration_seconds.max(0) as u64,
            view_count: value.view_count,
            progress_seconds: value.progress_seconds.max(0) as u64,
            watched: value.watched,
            is_live: value.is_live,
            is_short: value.is_short,
            audio_only: value.audio_only.unwrap_or(false),
        }
    }
}

impl From<DbPlaylist> for Playlist {
    fn from(value: DbPlaylist) -> Self {
        Self {
            id: value.playlist_id,
            name: value.name,
            video_ids: value.video_ids,
        }
    }
}

impl From<DbSubscriptionGroup> for SubscriptionGroup {
    fn from(value: DbSubscriptionGroup) -> Self {
        Self {
            id: value.group_id,
            name: value.name,
            channel_ids: value.channel_ids,
        }
    }
}

fn default_data_dir() -> std::path::PathBuf {
    #[cfg(test)]
    return std::env::temp_dir().join(format!("tawny-test-{}", std::process::id()));

    #[cfg(not(test))]
    if let Ok(path) = std::env::var("TAWNY_DATA_DIR") {
        return path.into();
    }
    #[cfg(all(not(test), target_os = "windows"))]
    if let Ok(path) = std::env::var("LOCALAPPDATA") {
        return std::path::PathBuf::from(path).join("Tawny");
    }
    #[cfg(all(not(test), target_os = "macos"))]
    if let Ok(path) = std::env::var("HOME") {
        return std::path::PathBuf::from(path)
            .join("Library")
            .join("Application Support")
            .join("Tawny");
    }
    #[cfg(not(test))]
    if let Ok(path) = std::env::var("XDG_DATA_HOME") {
        return std::path::PathBuf::from(path).join("tawny");
    }
    #[cfg(not(test))]
    if let Ok(path) = std::env::var("HOME") {
        return std::path::PathBuf::from(path)
            .join(".local")
            .join("share")
            .join("tawny");
    }
    #[cfg(not(test))]
    std::env::temp_dir().join("tawny")
}

impl AppServerState {
    pub async fn initialize() -> Result<Self> {
        let data_dir = default_data_dir();
        std::fs::create_dir_all(&data_dir).context("create Tawny data directory")?;
        let endpoint = if cfg!(test) {
            "mem://".into()
        } else {
            std::env::var("SURREALDB_HOST").context(
                "SURREALDB_HOST is required; start the Compose dependencies or provide a SurrealDB endpoint",
            )?
        };
        let db = connect(endpoint.clone())
            .await
            .context("connect to SurrealDB")?;
        if endpoint != "mem://" {
            if let (Ok(username), Ok(password)) = (
                std::env::var("SURREALDB_USER"),
                std::env::var("SURREALDB_PASSWORD"),
            ) {
                db.signin(Root { username, password })
                    .await
                    .context("sign in to SurrealDB")?;
            }
        }
        db.use_ns(std::env::var("SURREALDB_NAMESPACE").unwrap_or_else(|_| "tawny".into()))
            .use_db(std::env::var("SURREALDB_NAME").unwrap_or_else(|_| "main".into()))
            .await
            .context("select SurrealDB namespace and database")?;
        crate::database::sync_schema(&db).await?;

        let extractor_cache = std::env::var("TAWNY_EXTRACTOR_CACHE_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| data_dir.join("extractor"));
        let mut youtube_builder = RustyPipe::builder()
            .storage_dir(extractor_cache)
            .po_token_cache();
        // Only honoured when the path actually exists. It is an optional
        // helper, so a stale or placeholder value should cost the feature, not
        // the whole server — the extractor refuses to build at all when handed
        // a binary it cannot find.
        if let Ok(botguard_bin) = std::env::var("TAWNY_BOTGUARD_BIN") {
            if std::path::Path::new(&botguard_bin).is_file() {
                youtube_builder = youtube_builder.botguard_bin(botguard_bin);
            } else {
                eprintln!("ignoring TAWNY_BOTGUARD_BIN: no binary at {botguard_bin}");
            }
        }
        let youtube = youtube_builder
            .build()
            .context("initialize direct YouTube extractor")?;
        let public_url = std::env::var("TAWNY_PUBLIC_URL")
            // The Android bundle's generated network-security policy permits
            // cleartext loopback by literal IP. `localhost` is a different
            // policy entry even though it resolves to the same interface.
            .unwrap_or_else(|_| "http://127.0.0.1:8080".into())
            .trim_end_matches('/')
            .to_string();
        let websub_callback_url = std::env::var("TAWNY_WEBSUB_CALLBACK_URL")
            .ok()
            .filter(|url| !url.trim().is_empty())
            .or_else(|| {
                (!public_url.contains("localhost") && !public_url.contains("127.0.0.1"))
                    .then(|| format!("{public_url}/api/v1/websub/youtube"))
            });
        let websub_secret_path = data_dir.join("websub-secret");
        let websub_secret = std::env::var("TAWNY_WEBSUB_SECRET")
            .ok()
            .map(|secret| secret.trim().to_string())
            .filter(|secret| !secret.is_empty())
            .unwrap_or_else(|| {
                std::fs::read_to_string(&websub_secret_path)
                    .ok()
                    .map(|secret| secret.trim().to_string())
                    .filter(|secret| !secret.is_empty())
                    .unwrap_or_else(|| {
                        let secret = hex::encode(rand::random::<[u8; 32]>());
                        let _ = std::fs::write(&websub_secret_path, &secret);
                        secret
                    })
            });

        let ytdlp_service_url = configured_ytdlp_service_url();
        let ytdlp_bin = ytdlp_service_url.is_none().then(locate_ytdlp).flatten();
        let state = Self {
            db: Arc::new(db),
            http: reqwest::Client::builder()
                .user_agent("Tawny/0.1 subscription updater")
                .timeout(Duration::from_secs(15))
                .build()?,
            media_http: reqwest::Client::builder()
                .user_agent("Tawny/0.1 media proxy")
                // A media fetch that never returns is worse than one that
                // fails: the browser request hangs open, MediaSource simply
                // starves, and playback freezes with no error for the player
                // to recover from. Bound connect and per-chunk read time
                // instead of the whole request, so long streaming bodies stay
                // legal while a wedged upstream surfaces quickly.
                .connect_timeout(Duration::from_secs(10))
                .read_timeout(Duration::from_secs(20))
                .build()?,
            ytdlp_http: reqwest::Client::builder()
                .user_agent("Tawny/0.1 yt-dlp sidecar client")
                .connect_timeout(Duration::from_secs(3))
                .timeout(Duration::from_secs(40))
                .build()?,
            youtube: Arc::new(youtube),
            ytdlp_service_url,
            ytdlp_bin,
            ytdlp_po_provider_url: configured_ytdlp_po_provider_url(),
            public_url,
            websub_callback_url,
            websub_secret,
            proxy_targets: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            proxy_counter: Arc::new(AtomicU64::new(1)),
            playback_sessions: Arc::new(std::sync::Mutex::new(HashMap::new())),
            sync_lock: Arc::new(tokio::sync::Mutex::new(())),
            search_lock: Arc::new(tokio::sync::Mutex::new(())),
            search_cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            sponsor_cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            sponsor_api_url: std::env::var("TAWNY_SPONSORBLOCK_URL")
                .ok()
                .map(|url| url.trim_end_matches('/').to_string())
                .filter(|url| !url.is_empty()),
        };
        state.seed_demo_if_empty().await?;
        Ok(state)
    }

    /// Account operations against this instance's database.
    ///
    /// Borrowed per call rather than stored: it holds nothing but the handle,
    /// and keeping the account code behind one entry point is what stops it
    /// spreading through the catalog queries below.
    pub fn accounts(&self) -> crate::auth::Accounts<'_> {
        crate::auth::Accounts::new(&self.db)
    }

    /// Per-user rows are keyed by a record id built from the owner, so two
    /// accounts can hold the same client-chosen id - every library starts with
    /// a `watch-later`, and without this the second account to save one would
    /// overwrite the first.
    fn owner_key(owner: &str, id: &str) -> String {
        // `|` cannot appear in an app_user id, so this cannot be made ambiguous
        // by a playlist name that happens to contain the separator.
        format!("{}|{}", owner.trim_start_matches("app_user:"), id)
    }

    /// What this account follows, by channel id.
    async fn user_subscriptions(
        &self,
        owner: &str,
    ) -> Result<HashMap<String, (bool, SubscriptionContent)>> {
        if owner.is_empty() {
            return Ok(HashMap::new());
        }
        let rows: Vec<DbUserSubscription> = self
            .db
            .query(
                "SELECT channel_id, subscribed, content FROM user_subscription WHERE owner = type::record($owner)",
            )
            .bind(("owner", owner.to_string()))
            .await?
            .check()?
            .take(0)?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let content =
                    SubscriptionContent::from_storage(row.content.as_deref().unwrap_or("all"));
                (row.channel_id, (row.subscribed, content))
            })
            .collect())
    }

    /// Whether this account follows one channel, and which uploads it wants.
    ///
    /// Anything shown as a Subscribe button asks this, not `channel.subscribed`:
    /// that column means someone on the instance follows the channel - see
    /// `refresh_channel_interest` - so it reads as subscribed to an account
    /// that is not, and the other way round for details cached by another one.
    async fn user_follows(
        &self,
        owner: &str,
        channel_id: &str,
    ) -> Result<Option<(bool, SubscriptionContent)>> {
        if owner.is_empty() {
            return Ok(None);
        }
        let rows: Vec<DbUserSubscription> = self
            .db
            .query(
                "SELECT channel_id, subscribed, content FROM user_subscription WHERE owner = type::record($owner) AND channel_id = $channel_id LIMIT 1",
            )
            .bind(("owner", owner.to_string()))
            .bind(("channel_id", channel_id.to_string()))
            .await?
            .check()?
            .take(0)?;
        Ok(rows.into_iter().next().map(|row| {
            let content =
                SubscriptionContent::from_storage(row.content.as_deref().unwrap_or("all"));
            (row.subscribed, content)
        }))
    }

    /// Details with the channel's follow state set for `owner`.
    ///
    /// The details cache is shared by every account and keyed by video alone,
    /// so whatever follow state it holds belongs to whoever fetched it first -
    /// and until now it was served as-is for two hours, and indefinitely as the
    /// stale fallback. Applied to every copy on the way out, cached or fresh.
    async fn with_owner_follow_state(
        &self,
        owner: &str,
        mut details: VideoDetails,
    ) -> Result<VideoDetails> {
        if let Some(channel) = details.channel.as_mut() {
            let (subscribed, content) = self
                .user_follows(owner, &channel.id)
                .await?
                .unwrap_or((false, channel.subscription_content));
            channel.subscribed = subscribed;
            channel.subscription_content = content;
        }
        Ok(details)
    }

    /// Search results with every channel's follow state set for `owner`.
    ///
    /// Search is cached by query alone, and the channels in it carried the
    /// instance-wide flag, which is not anyone's own. That mattered beyond the
    /// button: the client adds a channel it has not seen to the library as it
    /// arrives, flag included, so a wrong "subscribed" here became a follow.
    async fn with_owner_follows_in_search(
        &self,
        owner: &str,
        mut results: SearchResults,
    ) -> Result<SearchResults> {
        if results.channels.is_empty() {
            return Ok(results);
        }
        let follows = self.user_subscriptions(owner).await?;
        for channel in &mut results.channels {
            match follows.get(&channel.id) {
                Some((subscribed, content)) => {
                    channel.subscribed = *subscribed;
                    channel.subscription_content = *content;
                }
                None => channel.subscribed = false,
            }
        }
        Ok(results)
    }

    /// Where this account has reached in each video it has touched.
    async fn user_progress(&self, owner: &str) -> Result<HashMap<String, DbVideoProgress>> {
        if owner.is_empty() {
            return Ok(HashMap::new());
        }
        let rows: Vec<DbVideoProgress> = self
            .db
            .query(
                "SELECT video_id, watched, progress_seconds, audio_only FROM video_progress WHERE owner = type::record($owner)",
            )
            .bind(("owner", owner.to_string()))
            .await?
            .check()?
            .take(0)?;
        Ok(rows
            .into_iter()
            .map(|row| (row.video_id.clone(), row))
            .collect())
    }

    /// Recompute `channel.subscribed` from the per-user table.
    ///
    /// That column is no longer anyone's opinion - it means "someone on this
    /// instance follows this channel", which is the question the poller and the
    /// WebSub renewals ask. It must never be written directly: one account
    /// unsubscribing would otherwise stop the feed for everyone else who still
    /// follows it.
    async fn refresh_channel_interest(&self, channel_ids: &[String]) -> Result<()> {
        for channel_id in channel_ids {
            let counts: Vec<DbFollowerCount> = self
                .db
                .query(
                    "SELECT count() FROM user_subscription WHERE channel_id = $channel_id AND subscribed = true GROUP ALL",
                )
                .bind(("channel_id", channel_id.clone()))
                .await?
                .check()?
                .take(0)?;
            let followed = counts.first().map(|row| row.count > 0).unwrap_or(false);
            self.db
                .query("UPDATE channel SET subscribed = $followed WHERE channel_id = $channel_id")
                .bind(("channel_id", channel_id.clone()))
                .bind(("followed", followed))
                .await?
                .check()?;
        }
        Ok(())
    }

    pub async fn library_snapshot(&self, owner: &str) -> Result<LibrarySnapshot> {
        let channels: Vec<DbChannel> = self
            .db
            .query(
                "SELECT channel_id, name, handle, avatar_url, subscriber_count, subscribed, subscription_content, description, banner_url FROM channel ORDER BY name",
            )
            .await?
            .take(0)?;
        let videos: Vec<DbVideo> = self
            .db
            .query(
                "SELECT video_id, title, channel_id, channel_name, thumbnail_url, published_at, published_sort, duration_seconds, view_count, is_live, is_short, progress_seconds, watched, audio_only FROM video ORDER BY published_sort DESC",
            )
            .await?
            .take(0)?;
        // Playlists, groups and the queue/history blob belong to one account.
        // The channel and video rows above do not: they are YouTube's facts,
        // and two accounts on one server should not fetch and store the same
        // upload twice.
        // An empty owner is the shared catalog on its own, for the two paths
        // with no viewer to speak for: the subscription poller and WebSub
        // ingest. They want channel and video rows and nothing else.
        //
        // The guard is load-bearing rather than tidy: `type::record("")` is a
        // runtime error ("Found  for the Record ID but this is not a valid
        // table name"), so without it every poll fails.
        let (playlists, subscription_groups, states) = if owner.is_empty() {
            (Vec::new(), Vec::new(), Vec::new())
        } else {
            let playlists: Vec<DbPlaylist> = self
                .db
                .query("SELECT playlist_id, name, video_ids FROM playlist WHERE owner = type::record($owner) ORDER BY name")
                .bind(("owner", owner.to_string()))
                .await?
                .check()?
                .take(0)?;
            let subscription_groups: Vec<DbSubscriptionGroup> = self
                .db
                .query("SELECT group_id, name, channel_ids FROM subscription_group WHERE owner = type::record($owner) ORDER BY name")
                .bind(("owner", owner.to_string()))
                .await?
                .check()?
                .take(0)?;
            let states: Vec<DbLibraryState> = self
                .db
                .query("SELECT cache_revision, queue_json, history_json FROM library_state WHERE owner = type::record($owner) LIMIT 1")
                .bind(("owner", owner.to_string()))
                .await?
                .check()?
                .take(0)?;
            (playlists, subscription_groups, states)
        };
        let state = states.into_iter().next();
        let queue = state
            .as_ref()
            .and_then(|state| serde_json::from_str(&state.queue_json).ok())
            .unwrap_or_default();
        let history: Vec<HistoryEntry> = state
            .as_ref()
            .and_then(|state| serde_json::from_str(&state.history_json).ok())
            .unwrap_or_default();

        // The shared rows carry whatever the last writer left in their
        // per-user columns, which for a second account is somebody else's
        // history. Overlay this account's own rows over them, and default the
        // rest to untouched rather than inheriting.
        let subscriptions = self.user_subscriptions(owner).await?;
        let progress = self.user_progress(owner).await?;

        let mut videos = videos.into_iter().map(Into::into).collect::<Vec<Video>>();
        for video in &mut videos {
            match progress.get(&video.id) {
                Some(row) => {
                    video.watched = row.watched;
                    video.progress_seconds = row.progress_seconds.max(0) as u64;
                    video.audio_only = row.audio_only;
                }
                None => {
                    video.watched = false;
                    video.progress_seconds = 0;
                    video.audio_only = false;
                }
            }
        }
        videos.sort_by_key(|video| std::cmp::Reverse(video_published_epoch(&video.published_at)));

        let mut channels = channels.into_iter().map(Channel::from).collect::<Vec<_>>();
        for channel in &mut channels {
            let (subscribed, content) = subscriptions
                .get(&channel.id)
                .copied()
                .unwrap_or((false, SubscriptionContent::default()));
            channel.subscribed = subscribed;
            channel.subscription_content = content;
        }

        Ok(LibrarySnapshot {
            channels,
            videos,
            playlists: playlists.into_iter().map(Into::into).collect(),
            subscription_groups: subscription_groups.into_iter().map(Into::into).collect(),
            queue,
            history,
            last_synced_at: Some("Just now".into()),
            cache_revision: state
                .map(|state| state.cache_revision.max(0) as u64)
                .unwrap_or_default(),
        })
    }

    async fn reachable_ytdlp_po_provider_url(&self) -> Option<&str> {
        let provider_url = self.ytdlp_po_provider_url.as_deref()?;
        let ping_url = format!("{provider_url}/ping");
        match tokio::time::timeout(Duration::from_secs(2), self.http.get(&ping_url).send()).await {
            Ok(Ok(response)) if response.status().is_success() => Some(provider_url),
            Ok(Ok(response)) => {
                eprintln!(
                    "PO-token provider at {provider_url} returned {} from /ping; using yt-dlp's default client",
                    response.status()
                );
                None
            }
            Ok(Err(error)) => {
                eprintln!(
                    "PO-token provider at {provider_url} is unavailable ({error}); using yt-dlp's default client"
                );
                None
            }
            Err(_) => {
                eprintln!(
                    "PO-token provider at {provider_url} did not answer /ping; using yt-dlp's default client"
                );
                None
            }
        }
    }

    /// Replace this account's subscription rows, and report which channels
    /// changed hands.
    ///
    /// One delete plus one bulk insert rather than a statement per channel: a
    /// full sync carries every subscription a viewer has, which is hundreds of
    /// rows, and a query each would be hundreds of round trips.
    async fn write_user_subscriptions(
        &self,
        owner: &str,
        incoming: &[(String, bool, SubscriptionContent)],
    ) -> Result<Vec<String>> {
        let prior = self.user_subscriptions(owner).await?;
        // A channel counts as changed when this account's answer moved. An
        // unknown channel only counts when the answer is yes, so a first sync
        // does not report every channel the viewer has ever declined.
        let changed = incoming
            .iter()
            .filter(|(channel_id, subscribed, _)| {
                prior
                    .get(channel_id)
                    .map(|(was, _)| was != subscribed)
                    .unwrap_or(*subscribed)
            })
            .map(|(channel_id, _, _)| channel_id.clone())
            .collect::<Vec<_>>();

        self.db
            .query("DELETE user_subscription WHERE owner = type::record($owner)")
            .bind(("owner", owner.to_string()))
            .await?
            .check()?;
        if !incoming.is_empty() {
            let rows = incoming
                .iter()
                .map(|(channel_id, subscribed, content)| {
                    serde_json::json!({
                        "owner": owner,
                        "channel_id": channel_id,
                        "subscribed": subscribed,
                        "content": content.as_storage(),
                    })
                })
                .collect::<Vec<_>>();
            self.db
                .query(
                    "INSERT INTO user_subscription (SELECT channel_id, subscribed, content, type::record(owner) AS owner, time::now() AS updated_at FROM $rows)",
                )
                .bind(("rows", rows))
                .await?
                .check()?;
        }
        Ok(changed)
    }

    /// Merge this account's watch progress for the videos it sent.
    ///
    /// A merge, not a replacement: the client sends only videos it has touched,
    /// so rows it did not mention are still this account's history and have to
    /// survive.
    async fn write_user_progress(&self, owner: &str, progress: &[VideoProgress]) -> Result<()> {
        for entry in progress {
            self.db
                .query(
                    r#"UPSERT type::record('video_progress', $record_key) SET
                        owner = type::record($owner),
                        video_id = $video_id,
                        watched = $watched,
                        progress_seconds = $progress_seconds,
                        audio_only = $audio_only,
                        updated_at = time::now()"#,
                )
                .bind(("record_key", Self::owner_key(owner, &entry.video_id)))
                .bind(("owner", owner.to_string()))
                .bind(("video_id", entry.video_id.clone()))
                .bind(("watched", entry.watched))
                .bind(("progress_seconds", entry.progress_seconds as i64))
                .bind(("audio_only", entry.audio_only))
                .await?
                .check()?;
        }
        Ok(())
    }

    /// Replace this account's queue, history and revision marker.
    async fn write_library_state(
        &self,
        owner: &str,
        cache_revision: u64,
        queue: &[String],
        history: &[HistoryEntry],
    ) -> Result<()> {
        // Delete then create, under the caller's sync lock. The unique index on
        // the owner makes a duplicate impossible rather than merely unlikely.
        self.db
            .query("DELETE library_state WHERE owner = type::record($owner)")
            .bind(("owner", owner.to_string()))
            .await?
            .check()?;
        self.db
            .query(
                r#"CREATE library_state SET
                    owner = type::record($owner),
                    cache_revision = $cache_revision,
                    queue_json = $queue_json,
                    history_json = $history_json,
                    updated_at = time::now()"#,
            )
            .bind(("owner", owner.to_string()))
            .bind(("cache_revision", cache_revision as i64))
            .bind(("queue_json", serde_json::to_string(queue)?))
            .bind(("history_json", serde_json::to_string(history)?))
            .await?
            .check()?;
        Ok(())
    }

    pub async fn sync_library(
        &self,
        owner: &str,
        snapshot: LibrarySnapshot,
    ) -> Result<LibrarySnapshot> {
        let guard = self.sync_lock.lock().await;
        let server_revision = self.library_revision(owner).await?;
        if snapshot.cache_revision <= server_revision {
            return self.library_snapshot(owner).await;
        }

        // Merged, not overwritten. The client's copy of a channel it imported is
        // still the stub from the export file, and writing it over the row would
        // blank the avatar and subscriber count the server has since fetched.
        for channel in &snapshot.channels {
            self.upsert_discovered_channel(channel).await?;
        }
        for video in &snapshot.videos {
            self.upsert_video(video, video_sort_key(video)).await?;
        }
        let subscriptions = snapshot
            .channels
            .iter()
            .map(|channel| {
                (
                    channel.id.clone(),
                    channel.subscribed,
                    channel.subscription_content,
                )
            })
            .collect::<Vec<_>>();
        let changed_ids = self.write_user_subscriptions(owner, &subscriptions).await?;
        self.write_user_progress(owner, &LibraryUserState::from(&snapshot).progress)
            .await?;

        for playlist in &snapshot.playlists {
            self.upsert_playlist(owner, playlist).await?;
        }
        let playlist_ids = snapshot
            .playlists
            .iter()
            .map(|playlist| playlist.id.clone())
            .collect::<Vec<_>>();
        // Scoped to the owner: without it, one account saving a playlist would
        // delete every other account's.
        self.db
            .query("DELETE playlist WHERE owner = type::record($owner) AND playlist_id NOT IN $playlist_ids")
            .bind(("owner", owner.to_string()))
            .bind(("playlist_ids", playlist_ids))
            .await?
            .check()?;
        for group in &snapshot.subscription_groups {
            self.upsert_subscription_group(owner, group).await?;
        }
        let group_ids = snapshot
            .subscription_groups
            .iter()
            .map(|group| group.id.clone())
            .collect::<Vec<_>>();
        self.db
            .query("DELETE subscription_group WHERE owner = type::record($owner) AND group_id NOT IN $group_ids")
            .bind(("owner", owner.to_string()))
            .bind(("group_ids", group_ids))
            .await?
            .check()?;

        self.write_library_state(
            owner,
            snapshot.cache_revision,
            &snapshot.queue,
            &snapshot.history,
        )
        .await?;

        self.refresh_channel_interest(&changed_ids).await?;

        drop(guard);

        let changed_channels = snapshot
            .channels
            .iter()
            .filter(|channel| changed_ids.contains(&channel.id))
            .filter(|channel| channel.id.starts_with("UC") && !channel.id.starts_with("UC-tawny"))
            .cloned()
            .collect::<Vec<_>>();
        self.spawn_subscription_changes(changed_channels);

        self.library_snapshot(owner).await
    }

    /// Hand newly changed channels to the WebSub and feed machinery.
    ///
    /// The channels carry *this* account's subscribed flag, but the work they
    /// trigger is instance-wide, so the decision has to be re-read from the
    /// derived column rather than taken from the caller: one viewer
    /// unsubscribing must not cancel a WebSub lease that other viewers still
    /// depend on.
    fn spawn_subscription_changes(&self, changed: Vec<Channel>) {
        if changed.is_empty() {
            return;
        }
        let state = self.clone();
        tokio::spawn(async move {
            let ids = changed
                .iter()
                .map(|channel| channel.id.clone())
                .collect::<Vec<_>>();
            let interest: Vec<DbChannel> = match state
                .db
                .query("SELECT channel_id, name, handle, avatar_url, subscriber_count, subscribed, subscription_content, description, banner_url FROM channel WHERE channel_id IN $ids")
                .bind(("ids", ids))
                .await
                .and_then(|mut response| response.take(0))
            {
                Ok(rows) => rows,
                Err(error) => {
                    eprintln!("could not re-read channel interest: {error}");
                    return;
                }
            };
            let channels = interest.into_iter().map(Channel::from).collect::<Vec<_>>();
            if !channels.is_empty() {
                state.process_subscription_changes(channels).await;
            }
        });
    }

    /// Apply the client-owned half of the library.
    ///
    /// The counterpart to [`Self::sync_library`] for the common case: a single
    /// mutation such as marking a video watched. It touches only rows the client
    /// can actually change, where the full snapshot path rewrote every cached
    /// channel and video on every keystroke-sized edit.
    pub async fn apply_user_state(&self, owner: &str, user: LibraryUserState) -> Result<u64> {
        let guard = self.sync_lock.lock().await;
        let server_revision = self.library_revision(owner).await?;
        if user.cache_revision <= server_revision {
            return Ok(server_revision);
        }

        // Nothing here touches the shared catalog any more. Subscriptions and
        // watch progress used to be written straight onto the `channel` and
        // `video` rows, which meant every account on an instance shared one
        // set of subscriptions and one viewing history; both now live in
        // tables keyed by owner.
        let subscriptions = user
            .subscriptions
            .iter()
            .map(|subscription| {
                (
                    subscription.channel_id.clone(),
                    subscription.subscribed,
                    subscription.content,
                )
            })
            .collect::<Vec<_>>();
        let changed_ids = self.write_user_subscriptions(owner, &subscriptions).await?;
        self.write_user_progress(owner, &user.progress).await?;

        for playlist in &user.playlists {
            self.upsert_playlist(owner, playlist).await?;
        }
        let playlist_ids = user
            .playlists
            .iter()
            .map(|playlist| playlist.id.clone())
            .collect::<Vec<_>>();
        // Scoped to the owner. Without it, one account saving a playlist would
        // delete every other account's.
        self.db
            .query("DELETE playlist WHERE owner = type::record($owner) AND playlist_id NOT IN $playlist_ids")
            .bind(("owner", owner.to_string()))
            .bind(("playlist_ids", playlist_ids))
            .await?
            .check()?;

        for group in &user.subscription_groups {
            self.upsert_subscription_group(owner, group).await?;
        }
        let group_ids = user
            .subscription_groups
            .iter()
            .map(|group| group.id.clone())
            .collect::<Vec<_>>();
        self.db
            .query("DELETE subscription_group WHERE owner = type::record($owner) AND group_id NOT IN $group_ids")
            .bind(("owner", owner.to_string()))
            .bind(("group_ids", group_ids))
            .await?
            .check()?;

        self.write_library_state(owner, user.cache_revision, &user.queue, &user.history)
            .await?;

        // The derived flag on `channel` has to catch up before anything reads
        // it to decide what to poll.
        self.refresh_channel_interest(&changed_ids).await?;

        drop(guard);

        // Subscribing still has to kick off WebSub and a first feed fetch, and
        // that side of it needs the full channel records - re-read, so they
        // carry the instance-wide flag rather than this account's answer.
        if !changed_ids.is_empty() {
            let changed_channels: Vec<DbChannel> = self
                .db
                .query("SELECT channel_id, name, handle, avatar_url, subscriber_count, subscribed, subscription_content, description, banner_url FROM channel WHERE channel_id IN $ids")
                .bind(("ids", changed_ids))
                .await?
                .take(0)?;
            let changed_channels = changed_channels
                .into_iter()
                .map(Channel::from)
                .filter(|channel| {
                    channel.id.starts_with("UC") && !channel.id.starts_with("UC-tawny")
                })
                .collect::<Vec<_>>();
            self.spawn_subscription_changes(changed_channels);
        }

        Ok(user.cache_revision)
    }

    async fn process_subscription_changes(&self, changed_channels: Vec<Channel>) {
        let newly_subscribed = changed_channels
            .iter()
            .filter(|channel| channel.subscribed)
            .cloned()
            .collect::<Vec<_>>();

        use futures_util::{StreamExt, stream};
        let websub_job = async {
            stream::iter(changed_channels.iter().cloned().map(|channel| async move {
                self.request_websub_subscription(&channel.id, channel.subscribed)
                    .await
            }))
            .buffer_unordered(WEBSUB_RENEW_CONCURRENCY)
            .collect::<Vec<_>>()
            .await
        };
        let feed_job =
            async {
                if newly_subscribed.len() <= 3 {
                    let _ =
                        stream::iter(newly_subscribed.iter().cloned().map(|channel| async move {
                            self.refresh_channel_local(channel).await
                        }))
                        .buffer_unordered(3)
                        .collect::<Vec<_>>()
                        .await;
                } else {
                    let ids = newly_subscribed
                        .iter()
                        .map(|channel| channel.id.clone())
                        .collect::<Vec<_>>();
                    let limit = reconciliation_limit(
                        newly_subscribed.len(),
                        self.websub_callback_url.is_some(),
                    );
                    let _ = self
                        .reconcile_channels_rss(newly_subscribed.into_iter().take(limit).collect())
                        .await;
                    // RSS carries no avatar or subscriber count, and a bulk
                    // import is the one path that never reads a channel page.
                    self.backfill_channel_metadata(Some(&ids), CHANNEL_METADATA_BATCH)
                        .await;
                }
            };
        let _ = tokio::join!(websub_job, feed_job);
    }

    async fn library_revision(&self, owner: &str) -> Result<u64> {
        if owner.is_empty() {
            return Ok(0);
        }
        let states: Vec<DbLibraryState> = self
            .db
            .query("SELECT cache_revision, queue_json, history_json FROM library_state WHERE owner = type::record($owner) LIMIT 1")
            .bind(("owner", owner.to_string()))
            .await?
            .check()?
            .take(0)?;
        Ok(states
            .first()
            .map(|state| state.cache_revision.max(0) as u64)
            .unwrap_or_default())
    }

    async fn register_proxy_target(
        &self,
        url: String,
        request_headers: Vec<crate::models::PlaybackRequestHeader>,
        ttl: Duration,
    ) -> String {
        let counter = self.proxy_counter.fetch_add(1, Ordering::Relaxed);
        let mut hasher = DefaultHasher::new();
        self.websub_secret.hash(&mut hasher);
        url.hash(&mut hasher);
        counter.hash(&mut hasher);
        let token = format!("{:x}{counter:x}", hasher.finish());
        let now = std::time::Instant::now();
        let mut targets = self.proxy_targets.write().await;
        targets.retain(|_, target| target.expires_at > now);
        targets.insert(
            token.clone(),
            ProxyTarget {
                url,
                request_headers,
                expires_at: now + ttl,
            },
        );
        format!("{}/api/v1/playback/proxy/{token}", self.public_url)
    }

    async fn proxy_source(&self, mut source: PlaybackSource) -> PlaybackSource {
        if matches!(source.protocol, PlaybackProtocol::EmbedFallback) {
            return source;
        }
        let ttl = source
            .expires_at
            .as_ref()
            .map(|_| Duration::from_secs(90 * 60))
            .unwrap_or_else(|| Duration::from_secs(2 * 60 * 60));
        if source.tracks.is_empty() {
            source.url = self
                .register_proxy_target(source.url, source.request_headers.clone(), ttl)
                .await;
        } else {
            for track in &mut source.tracks {
                let headers = if track.request_headers.is_empty() {
                    source.request_headers.clone()
                } else {
                    track.request_headers.clone()
                };
                track.url = self
                    .register_proxy_target(track.url.clone(), headers, ttl)
                    .await;
                track.request_headers.clear();
            }
            if let Some(video) = source
                .tracks
                .iter()
                .find(|track| track.kind == PlaybackTrackKind::Video)
            {
                source.url = video.url.clone();
            }
        }
        source.request_headers.clear();
        source
    }

    async fn proxy_session(&self, mut session: PlaybackSession) -> PlaybackSession {
        session.primary = self.proxy_source(session.primary).await;
        let mut proxied = Vec::with_capacity(session.alternatives.len());
        for source in session.alternatives {
            proxied.push(self.proxy_source(source).await);
        }
        session.alternatives = proxied;
        session
    }

    async fn proxy_captions(&self, mut details: VideoDetails) -> VideoDetails {
        for caption in &mut details.captions {
            if caption.url.starts_with(&self.public_url) {
                continue;
            }
            caption.url = self
                .register_proxy_target(
                    caption.url.clone(),
                    Vec::new(),
                    Duration::from_secs(2 * 60 * 60),
                )
                .await;
        }
        details
    }

    async fn adaptive_tail_is_available(&self, source: &PlaybackSource) -> bool {
        if source.tracks.is_empty()
            || !matches!(
                source.protocol,
                PlaybackProtocol::Dash | PlaybackProtocol::Sabr
            )
        {
            return true;
        }
        let video = source
            .tracks
            .iter()
            .filter(|track| track.kind == PlaybackTrackKind::Video)
            .max_by_key(|track| {
                (
                    track.height.unwrap_or_default(),
                    track.bitrate.unwrap_or_default(),
                )
            });
        let audio = source
            .tracks
            .iter()
            .filter(|track| track.kind == PlaybackTrackKind::Audio)
            .max_by_key(|track| track.bitrate.unwrap_or_default());

        // Both ends at once: this sits in front of the first frame.
        let probes = [video, audio]
            .into_iter()
            .flatten()
            .map(|track| async move {
                let Some(length) = track.content_length.filter(|length| *length > 0) else {
                    return true;
                };
                let start = length.saturating_sub(2_048);
                let mut request = self
                    .media_http
                    .get(&track.url)
                    .header(
                        reqwest::header::RANGE,
                        format!("bytes={start}-{}", length - 1),
                    )
                    .header(reqwest::header::ACCEPT_ENCODING, "identity");
                for configured in source
                    .request_headers
                    .iter()
                    .chain(track.request_headers.iter())
                {
                    if let (Ok(name), Ok(value)) = (
                        reqwest::header::HeaderName::from_bytes(configured.name.as_bytes()),
                        reqwest::header::HeaderValue::from_str(&configured.value),
                    ) {
                        request = request.header(name, value);
                    }
                }
                let response = tokio::time::timeout(Duration::from_secs(4), request.send()).await;
                matches!(response, Ok(Ok(response)) if response.status().is_success())
            });
        futures_util::future::join_all(probes)
            .await
            .into_iter()
            .all(|available| available)
    }

    /// A channel's Shorts, listed by yt-dlp.
    ///
    /// rustypipe 0.11.4 cannot read the Shorts tab at all: verified 2026-08-13
    /// against two channels that publish Shorts, both returned zero items on
    /// the first page *and* zero after following the continuation token. yt-dlp
    /// parses the same tab correctly, and it is already a dependency for
    /// playback URLs, so it fills the gap rather than leaving the tab empty.
    async fn ytdlp_channel_shorts(&self, channel_id: &str, channel_name: &str) -> Vec<Video> {
        if let Some(service_url) = self.ytdlp_service_url.as_deref() {
            let Some(url) = ytdlp_service_channel_shorts_url(service_url, channel_id) else {
                return Vec::new();
            };
            let dump = match self.ytdlp_http.get(url).send().await {
                Ok(response) if response.status().is_success() => response
                    .json::<YtdlpFlatPlaylist>()
                    .await
                    .map_err(anyhow::Error::from),
                Ok(response) => Err(anyhow!(
                    "yt-dlp service returned {} for channel Shorts",
                    response.status()
                )),
                Err(error) => Err(error.into()),
            };
            return match dump {
                Ok(dump) => ytdlp_flat_playlist_videos(dump, channel_id, channel_name),
                Err(error) => {
                    eprintln!("yt-dlp service could not list Shorts for {channel_id}: {error}");
                    Vec::new()
                }
            };
        }
        let Some(binary) = self.ytdlp_bin.as_ref() else {
            return Vec::new();
        };
        let output = tokio::process::Command::new(binary)
            .args([
                "--flat-playlist",
                "-J",
                "--playlist-end",
                "30",
                "--no-warnings",
                "--socket-timeout",
                "15",
                &format!("https://www.youtube.com/channel/{channel_id}/shorts"),
            ])
            .stdin(std::process::Stdio::null())
            .output();
        let output = match tokio::time::timeout(Duration::from_secs(40), output).await {
            Ok(Ok(output)) if output.status.success() => output,
            _ => {
                eprintln!("yt-dlp could not list Shorts for {channel_id}");
                return Vec::new();
            }
        };
        let Ok(dump) = serde_json::from_slice::<YtdlpFlatPlaylist>(&output.stdout) else {
            return Vec::new();
        };
        ytdlp_flat_playlist_videos(dump, channel_id, channel_name)
    }

    /// Stream URLs from yt-dlp, keyed by itag.
    ///
    /// YouTube increasingly gates every client behind a video-bound GVS PO
    /// token. When `TAWNY_PO_TOKEN_PROVIDER_URL` is configured, yt-dlp uses its
    /// bgutil HTTP provider with the mweb client so those URLs remain valid for
    /// the full media object. Without a provider we keep yt-dlp's own default
    /// client selection as a best-effort fallback.
    ///
    /// Only the URL is taken. The byte ranges, codecs, and sizes still come
    /// from rustypipe, which is sound because an itag identifies one specific
    /// transcode: the two clients hand out different URLs for the same file.
    /// `filesize` is compared per itag so a mismatch is skipped rather than
    /// producing a manifest whose ranges point into the wrong bytes.
    /// Every format yt-dlp can see, which is the whole basis of playback now.
    async fn ytdlp_service_formats(
        &self,
        service_url: &str,
        video_id: &str,
    ) -> std::result::Result<Vec<YtdlpFormat>, String> {
        let url = ytdlp_service_video_url(service_url, video_id)
            .ok_or_else(|| format!("the yt-dlp service URL {service_url} is invalid"))?;
        let response = self.ytdlp_http.get(url).send().await.map_err(|error| {
            format!("the yt-dlp service at {service_url} is unreachable ({error})")
        })?;
        if !response.status().is_success() {
            return Err(format!(
                "the yt-dlp service answered {} for this video",
                response.status()
            ));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|error| format!("the yt-dlp service's answer could not be read ({error})"))?;
        serde_json::from_slice::<YtdlpDump>(&bytes)
            .map(formats_from_ytdlp_dump)
            .map_err(|error| format!("the yt-dlp service's answer could not be parsed ({error})"))
    }

    /// Every format yt-dlp found, or why there are none. The reason is shown
    /// to the viewer, so it names the cause rather than the symptom.
    async fn ytdlp_formats(&self, video_id: &str) -> std::result::Result<Vec<YtdlpFormat>, String> {
        let formats = self.run_ytdlp(video_id).await;
        if let Err(reason) = &formats {
            eprintln!("yt-dlp gave no formats for {video_id}: {reason}");
        }
        formats
    }

    async fn run_ytdlp(&self, video_id: &str) -> std::result::Result<Vec<YtdlpFormat>, String> {
        if let Some(service_url) = self.ytdlp_service_url.as_deref() {
            return self.ytdlp_service_formats(service_url, video_id).await;
        }
        let Some(binary) = self.ytdlp_bin.as_ref() else {
            return Err(
                "no yt-dlp is configured (set TAWNY_YTDLP_SERVICE_URL or TAWNY_YTDLP_BIN)".into(),
            );
        };
        let mut command = tokio::process::Command::new(binary);
        command.args([
            "-J",
            "--no-warnings",
            "--no-playlist",
            "--socket-timeout",
            "15",
        ]);
        let po_provider_url = self.reachable_ytdlp_po_provider_url().await;
        command.args(ytdlp_po_provider_args(po_provider_url));
        command.arg(format!("https://www.youtube.com/watch?v={video_id}"));
        let output = command.stdin(std::process::Stdio::null()).output();
        let output = match tokio::time::timeout(Duration::from_secs(30), output).await {
            Ok(Ok(output)) if output.status.success() => output,
            Ok(Ok(output)) => {
                return Err(format!(
                    "yt-dlp failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
            Ok(Err(error)) => return Err(format!("yt-dlp could not be run ({error})")),
            Err(_) => return Err("yt-dlp timed out".into()),
        };
        serde_json::from_slice::<YtdlpDump>(&output.stdout)
            .map(formats_from_ytdlp_dump)
            .map_err(|error| format!("yt-dlp's output could not be parsed ({error})"))
    }

    /// The itag-to-URL view the extractor-layout path still needs.
    fn ytdlp_url_map(formats: &[YtdlpFormat]) -> HashMap<u32, (String, Option<u64>)> {
        formats
            .iter()
            .filter_map(|format| {
                let url = format.url.clone().filter(|url| !url.is_empty())?;
                Some((format.itag()?, (url, format.filesize)))
            })
            .collect()
    }

    /// A playback session for `video_id`, resolved once and handed out again.
    ///
    /// Resolving costs several seconds, almost all of it yt-dlp, and it is paid
    /// before the first frame. Clients ask ahead - for the video queued next,
    /// and on opening a watch page before its details arrive - so by the time
    /// the player asks, the answer is usually here or already on its way. Asks
    /// for a video whose resolve is still running wait on that one rather than
    /// starting another, which also keeps the sidecar's two extraction slots
    /// free. The resolve runs as its own task, so an ask that gives up (a
    /// client moving on) does not abandon it for the next one.
    ///
    /// `fresh` is for a player whose session stopped working: it discards what
    /// is held and resolves again.
    pub async fn playback_session(
        &self,
        video_id: &str,
        _prefer_sabr: bool,
        fresh: bool,
    ) -> Result<PlaybackSession> {
        let resolve = self.shared_playback_resolve(video_id, fresh);
        let resolved = resolve.clone().await;
        if !resolved.as_ref().is_ok_and(|resolved| resolved.reusable) {
            self.forget_playback_resolve(video_id, &resolve);
        }
        let resolved = resolved.map_err(|error| anyhow!(error))?;
        Ok(self.proxy_session(resolved.session).await)
    }

    fn shared_playback_resolve(&self, video_id: &str, fresh: bool) -> SharedPlaybackResolve {
        let mut sessions = self
            .playback_sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let now = std::time::Instant::now();
        sessions.retain(|_, cached| now.duration_since(cached.started_at) < PLAYBACK_SESSION_TTL);
        if fresh {
            sessions.remove(video_id);
        }
        if let Some(cached) = sessions.get(video_id) {
            return cached.resolve.clone();
        }
        if sessions.len() >= PLAYBACK_SESSION_LIMIT
            && let Some(oldest) = sessions
                .iter()
                .min_by_key(|(_, cached)| cached.started_at)
                .map(|(id, _)| id.clone())
        {
            sessions.remove(&oldest);
        }
        let state = self.clone();
        let id = video_id.to_string();
        let task = tokio::spawn(async move {
            state
                .resolve_playback(&id)
                .await
                .map_err(|error| format!("{error:#}"))
        });
        let resolve = async move {
            task.await
                .unwrap_or_else(|error| Err(format!("playback resolve aborted: {error}")))
        }
        .boxed()
        .shared();
        sessions.insert(
            video_id.to_string(),
            CachedPlayback {
                started_at: now,
                resolve: resolve.clone(),
            },
        );
        resolve
    }

    /// Drop a resolve that is not worth handing out again - unless it has
    /// already been replaced, in which case the replacement stays.
    fn forget_playback_resolve(&self, video_id: &str, resolve: &SharedPlaybackResolve) {
        let mut sessions = self
            .playback_sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if sessions
            .get(video_id)
            .is_some_and(|cached| cached.resolve.ptr_eq(resolve))
        {
            sessions.remove(video_id);
        }
    }

    /// Resolve playback from scratch. The session is not yet proxied.
    async fn resolve_playback(&self, video_id: &str) -> Result<ResolvedPlayback> {
        let fallback_url = embed_url(video_id);
        let mut provider_sources: Vec<PlaybackSource> = Vec::new();
        // Run both extractors together: rustypipe supplies the stream layout
        // (byte ranges, codecs, languages) and yt-dlp supplies URLs that are
        // not subject to the iOS client's 403 gate.
        let started = std::time::Instant::now();
        let query = self.youtube.query();
        let (player, ytdlp_result) = tokio::join!(
            youtube_call(
                query.player_from_clients(video_id, PLAYER_CLIENTS),
                "extract YouTube player"
            ),
            self.ytdlp_formats(video_id),
        );
        let extracted_at = started.elapsed();
        // Why yt-dlp gave nothing, kept for the error the viewer sees if no
        // other ungated source turns up either.
        let ytdlp_failure = ytdlp_result.as_ref().err().cloned();
        let mut ytdlp_formats = ytdlp_result.unwrap_or_default();

        // yt-dlp owns the adaptive source. The extractor is consulted only for
        // what yt-dlp does not produce: the HLS manifest a live stream needs.
        // Its own stream URLs are gated, so they are never played.
        let mut ytdlp_source = ytdlp_playback_source(&self.http, video_id, &ytdlp_formats).await;
        let ranged_at = started.elapsed();
        // Now and then yt-dlp hands out URLs that serve the first minute or so
        // and then answer 403 to every later range: the video starts, plays its
        // buffered head, and dies. Which extraction gets gated is luck - on
        // 2026-09-24 the same long video came back gated 3 times in 18 tries,
        // across both provider versions - so one more extraction usually cures
        // it, and costs nothing when the first one was sound.
        let mut ytdlp_verified = None;
        if let Some(source) = ytdlp_source.as_ref() {
            let sound = self.adaptive_tail_is_available(source).await;
            ytdlp_verified = Some(sound);
            if !sound {
                eprintln!(
                    "playback for {video_id}: yt-dlp URLs are gated past the head, extracting again"
                );
                let retried_formats = self.ytdlp_formats(video_id).await.unwrap_or_default();
                if let Some(retried) =
                    ytdlp_playback_source(&self.http, video_id, &retried_formats).await
                    && self.adaptive_tail_is_available(&retried).await
                {
                    ytdlp_source = Some(retried);
                    ytdlp_formats = retried_formats;
                    ytdlp_verified = Some(true);
                }
            }
        }
        let verified_at = started.elapsed();
        // A session known to be gated is still handed to this player - its head
        // plays, and the transport refreshes when it fails - but it is not kept
        // for the next ask, which would otherwise inherit the same dead URLs for
        // the whole session TTL.
        let reusable = ytdlp_source.is_some() && ytdlp_verified != Some(false);
        // Identifies the yt-dlp source after sorting, so its tail is not
        // probed a second time below.
        let verified_track_url = ytdlp_source
            .as_ref()
            .filter(|_| ytdlp_verified == Some(true))
            .and_then(|source| source.tracks.first())
            .map(|track| track.url.clone());
        if let Some(source) = ytdlp_source {
            eprintln!(
                "playback for {video_id}: {} yt-dlp tracks, ranges derived locally",
                source.tracks.len()
            );
            provider_sources.push(source);
        } else if let Some(source) = ytdlp_hls_source(&ytdlp_formats) {
            eprintln!("playback for {video_id}: yt-dlp produced no DASH ladder, using its HLS");
            provider_sources.push(source);
        }

        let ytdlp_urls = Self::ytdlp_url_map(&ytdlp_formats);
        let youtube_sources = match player {
            // Only tracks yt-dlp covers, plus the extractor's manifests.
            Ok(player) => rusty_playback_sources(&player, &ytdlp_urls),
            Err(error) => {
                eprintln!("playback extraction failed for {video_id}: {error:#}");
                Vec::new()
            }
        };
        for source in youtube_sources {
            if !provider_sources
                .iter()
                .any(|existing| existing.url == source.url)
            {
                provider_sources.push(source);
            }
        }
        // Last resort when the default client order yielded nothing at all.
        //
        // Measured 2026-08-12 against rustypipe 0.11.4: both of these clients
        // fail signature deobfuscation ("could not extract sig fn name"), so
        // this loop is currently inert and only costs a request when nothing
        // else was found. It is kept because it starts working again whenever
        // upstream refreshes the deobfuscator.
        if provider_sources.is_empty() {
            for client in [ClientType::Android, ClientType::Tv] {
                if let Ok(player) = youtube_call(
                    self.youtube.query().player_from_client(video_id, client),
                    "extract alternate YouTube player",
                )
                .await
                {
                    for source in rusty_playback_sources(&player, &ytdlp_urls) {
                        if !provider_sources
                            .iter()
                            .any(|existing| existing.url == source.url)
                        {
                            provider_sources.push(source);
                        }
                    }
                }
            }
        }

        provider_sources.sort_by_key(playback_source_priority);
        let primary_verified = verified_track_url.is_some()
            && provider_sources
                .first()
                .and_then(|source| source.tracks.first())
                .map(|track| &track.url)
                == verified_track_url.as_ref();

        use futures_util::StreamExt;

        // The tail probe is a reachability hint with a short timeout, not proof,
        // so it may only reorder — never discard. An earlier version treated it
        // as a filter, which could empty the list and strand playback on the
        // embed. Probe in priority order, stop at the first source that answers,
        // and cap the whole phase: this runs before the user sees any video, so
        // it must never become the reason playback feels slow to start. If the
        // budget expires the priority order simply stands.
        //
        // Every source is probed at once, and the answer is in as soon as the
        // earliest source that answered has nothing still pending ahead of it:
        // the usual case - the first source confirming - costs one round trip,
        // and none at all when that source is the yt-dlp one verified above.
        let promoted = if primary_verified {
            Some(0)
        } else {
            tokio::time::timeout(Duration::from_secs(6), async {
                let mut probes = provider_sources
                    .iter()
                    .enumerate()
                    .map(|(index, source)| async move {
                        (index, self.adaptive_tail_is_available(source).await)
                    })
                    .collect::<futures_util::stream::FuturesUnordered<_>>();
                let mut answers = vec![None; provider_sources.len()];
                while let Some((index, available)) = probes.next().await {
                    answers[index] = Some(available);
                    for (index, answer) in answers.iter().enumerate() {
                        match answer {
                            Some(true) => return Some(index),
                            Some(false) => continue,
                            None => break,
                        }
                    }
                }
                None
            })
            .await
            .unwrap_or(None)
        };
        if let Some(index) = promoted.filter(|index| *index > 0) {
            let verified = provider_sources.remove(index);
            provider_sources.insert(0, verified);
        }

        if !provider_sources.is_empty() {
            // Cumulative, so the slow phase is the one with the big step. This
            // all runs before the first frame, and a slow start is otherwise
            // impossible to attribute from the log.
            eprintln!(
                "playback for {video_id}: {} sources, primary {:?}, tail probe {} \
                 (extract {}ms, ranges {}ms, verify {}ms, total {}ms)",
                provider_sources.len(),
                provider_sources[0].protocol,
                match promoted {
                    Some(_) => "confirmed a source",
                    None => "confirmed nothing (using priority order)",
                },
                extracted_at.as_millis(),
                ranged_at.as_millis(),
                verified_at.as_millis(),
                started.elapsed().as_millis(),
            );
            let primary = provider_sources.remove(0);
            return Ok(ResolvedPlayback {
                session: PlaybackSession {
                    primary,
                    alternatives: provider_sources,
                    fallback_url,
                },
                reusable,
            });
        }

        // An error, not a session: the player shows this message, and the
        // viewer learns the cause (a stopped sidecar, say) instead of meeting
        // gated URLs that freeze a minute in.
        let reason = no_stream_reason(ytdlp_failure.as_deref(), ytdlp_formats.len());
        eprintln!("playback for {video_id}: {reason}");
        Err(anyhow!(reason))
    }

    async fn seed_demo_if_empty(&self) -> Result<()> {
        let existing: Vec<DbChannel> = self
            .db
            .query(
                "SELECT channel_id, name, handle, avatar_url, subscriber_count, subscribed, subscription_content, description, banner_url FROM channel LIMIT 1",
            )
            .await?
            .take(0)?;
        if !existing.is_empty() {
            return Ok(());
        }
        let demo = LibrarySnapshot::demo();
        for channel in &demo.channels {
            self.upsert_channel(channel).await?;
        }
        for (index, video) in demo.videos.iter().enumerate() {
            self.upsert_video(video, format!("demo-{index:04}")).await?;
        }
        Ok(())
    }

    async fn upsert_channel(&self, channel: &Channel) -> Result<()> {
        self.db
            .query(
                r#"UPSERT type::record('channel', $record_key) SET
                    channel_id = $channel_id,
                    name = $name,
                    handle = $handle,
                    avatar_url = $avatar_url,
                    subscriber_count = $subscriber_count,
                    subscribed = $subscribed,
                    subscription_content = $subscription_content,
                    description = $description,
                    banner_url = $banner_url"#,
            )
            .bind(("record_key", channel.id.clone()))
            .bind(("channel_id", channel.id.clone()))
            .bind(("name", channel.name.clone()))
            .bind(("handle", channel.handle.clone()))
            .bind(("avatar_url", channel.avatar_url.clone()))
            .bind(("subscriber_count", channel.subscriber_count.clone()))
            .bind(("subscribed", channel.subscribed))
            .bind((
                "subscription_content",
                channel.subscription_content.as_storage(),
            ))
            .bind(("description", channel.description.clone()))
            .bind(("banner_url", channel.banner_url.clone()))
            .await?
            .check()?;
        Ok(())
    }

    async fn upsert_video(&self, video: &Video, published_sort: String) -> Result<()> {
        let published_sort = canonical_sort_key(&published_sort);
        self.db
            .query(
                r#"UPSERT type::record('video', $record_key) SET
                    video_id = $video_id,
                    title = $title,
                    channel_id = $channel_id,
                    channel_name = $channel_name,
                    thumbnail_url = $thumbnail_url,
                    published_at = $published_at,
                    published_sort = $published_sort,
                    duration_seconds = $duration_seconds,
                    view_count = $view_count,
                    is_live = $is_live,
                    is_short = $is_short,
                    progress_seconds = $progress_seconds,
                    watched = $watched,
                    audio_only = $audio_only"#,
            )
            .bind(("record_key", video.id.clone()))
            .bind(("video_id", video.id.clone()))
            .bind(("title", video.title.clone()))
            .bind(("channel_id", video.channel_id.clone()))
            .bind(("channel_name", video.channel_name.clone()))
            .bind(("thumbnail_url", video.thumbnail_url.clone()))
            .bind(("published_at", video.published_at.clone()))
            .bind(("published_sort", published_sort))
            .bind(("duration_seconds", video.duration_seconds as i64))
            .bind(("view_count", video.view_count.clone()))
            .bind(("is_live", video.is_live))
            .bind(("is_short", video.is_short))
            .bind(("progress_seconds", video.progress_seconds as i64))
            .bind(("watched", video.watched))
            .bind(("audio_only", video.audio_only))
            .await?
            .check()?;
        Ok(())
    }

    async fn upsert_feed_video(&self, video: &Video, published_sort: String) -> Result<()> {
        let published_sort = canonical_sort_key(&published_sort);
        self.db
            .query(
                r#"UPSERT type::record('video', $record_key) SET
                    video_id = $video_id,
                    title = $title,
                    channel_id = $channel_id,
                    channel_name = $channel_name,
                    thumbnail_url = $thumbnail_url,
                    published_at = $published_at,
                    published_sort = $published_sort,
                    duration_seconds = $duration_seconds,
                    view_count = $view_count,
                    is_live = $is_live,
                    is_short = $is_short"#,
            )
            .bind(("record_key", video.id.clone()))
            .bind(("video_id", video.id.clone()))
            .bind(("title", video.title.clone()))
            .bind(("channel_id", video.channel_id.clone()))
            .bind(("channel_name", video.channel_name.clone()))
            .bind(("thumbnail_url", video.thumbnail_url.clone()))
            .bind(("published_at", video.published_at.clone()))
            .bind(("published_sort", published_sort))
            .bind(("duration_seconds", video.duration_seconds as i64))
            .bind(("view_count", video.view_count.clone()))
            .bind(("is_live", video.is_live))
            .bind(("is_short", video.is_short))
            .await?
            .check()?;
        Ok(())
    }

    async fn upsert_feed_hint(&self, video: &Video, published_sort: String) -> Result<()> {
        let published_sort = canonical_sort_key(&published_sort);
        self.db
            .query(
                r#"UPSERT type::record('video', $record_key) SET
                    video_id = $video_id,
                    title = $title,
                    channel_id = $channel_id,
                    channel_name = $channel_name,
                    thumbnail_url = $thumbnail_url,
                    published_at = $published_at,
                    published_sort = $published_sort"#,
            )
            .bind(("record_key", video.id.clone()))
            .bind(("video_id", video.id.clone()))
            .bind(("title", video.title.clone()))
            .bind(("channel_id", video.channel_id.clone()))
            .bind(("channel_name", video.channel_name.clone()))
            .bind(("thumbnail_url", video.thumbnail_url.clone()))
            .bind(("published_at", video.published_at.clone()))
            .bind(("published_sort", published_sort))
            .await?
            .check()?;
        Ok(())
    }

    async fn upsert_discovered_channel(&self, channel: &Channel) -> Result<()> {
        let existing: Vec<DbChannel> = self
            .db
            .query("SELECT * FROM channel WHERE channel_id = $channel_id LIMIT 1")
            .bind(("channel_id", channel.id.clone()))
            .await?
            .take(0)?;
        let mut merged = existing
            .into_iter()
            .next()
            .map(Channel::from)
            .unwrap_or_else(|| channel.clone());
        if merged.id == channel.id {
            merged.merge_metadata_from(channel);
        }
        self.db
            .query(
                r#"UPSERT type::record('channel', $record_key) SET
                    channel_id = $channel_id,
                    name = $name,
                    handle = $handle,
                    avatar_url = $avatar_url,
                    subscriber_count = $subscriber_count,
                    description = $description,
                    banner_url = $banner_url,
                    subscribed = $subscribed,
                    subscription_content = $subscription_content"#,
            )
            .bind(("record_key", channel.id.clone()))
            .bind(("channel_id", channel.id.clone()))
            .bind(("name", merged.name))
            .bind(("handle", merged.handle))
            .bind(("avatar_url", merged.avatar_url))
            .bind(("subscriber_count", merged.subscriber_count))
            .bind(("description", merged.description))
            .bind(("banner_url", merged.banner_url))
            .bind(("subscribed", merged.subscribed))
            .bind((
                "subscription_content",
                merged.subscription_content.as_storage(),
            ))
            .await?
            .check()?;
        Ok(())
    }

    /// Fill in the avatar and subscriber count of followed channels missing
    /// either, and report how many rows gained something.
    ///
    /// An imported subscription arrives as a stub holding only an id and a name.
    /// Following a few channels reads each channel page, which is where those
    /// facts come from, but a bulk import is reconciled over RSS - which has
    /// neither - so imported channels stayed faceless for good. `only` narrows a
    /// pass to channels that just changed; `limit` caps the channel pages one
    /// pass requests.
    async fn backfill_channel_metadata(&self, only: Option<&[String]>, limit: usize) -> usize {
        let rows: Vec<DbChannel> = match self
            .db
            .query("SELECT channel_id, name, handle, avatar_url, subscriber_count, subscribed, subscription_content, description, banner_url FROM channel WHERE subscribed = true")
            .await
            .and_then(|mut response| response.take(0))
        {
            Ok(rows) => rows,
            Err(error) => {
                eprintln!("could not list channels for a metadata backfill: {error}");
                return 0;
            }
        };
        let candidates = rows
            .into_iter()
            .map(Channel::from)
            .filter(|channel| channel.id.starts_with("UC") && !channel.id.starts_with("UC-tawny"))
            .filter(|channel| only.is_none_or(|ids| ids.contains(&channel.id)))
            .filter(|channel| {
                channel.avatar_url.is_none() || channel.subscriber_count.trim().is_empty()
            })
            .take(limit)
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return 0;
        }

        use futures_util::{StreamExt, stream};
        stream::iter(candidates.into_iter().map(|channel| async move {
            let query = self.youtube.query();
            let page = match youtube_call(
                query.channel_videos(&channel.id),
                "extract channel metadata",
            )
            .await
            {
                Ok(page) => page,
                Err(error) => {
                    eprintln!("metadata backfill for {} failed: {error:#}", channel.id);
                    return false;
                }
            };
            let discovered = rusty_channel_to_channel(&page, channel.subscribed);
            match self.store_channel_metadata(&channel, &discovered).await {
                Ok(changed) => changed,
                Err(error) => {
                    eprintln!("could not store metadata for {}: {error:#}", channel.id);
                    false
                }
            }
        }))
        .buffer_unordered(CHANNEL_METADATA_CONCURRENCY)
        .filter(|changed| std::future::ready(*changed))
        .count()
        .await
    }

    /// Write fetched channel facts onto an existing row without touching who
    /// follows it.
    ///
    /// `channel.subscribed` is derived from every account's own rows, so a
    /// metadata write must never carry it. Merged rather than replaced, so a
    /// sparse response cannot blank what a richer one stored earlier.
    async fn store_channel_metadata(
        &self,
        existing: &Channel,
        discovered: &Channel,
    ) -> Result<bool> {
        let mut merged = existing.clone();
        if !merged.merge_metadata_from(discovered) {
            return Ok(false);
        }
        self.db
            .query(
                r#"UPDATE channel SET
                    name = $name,
                    handle = $handle,
                    avatar_url = $avatar_url,
                    subscriber_count = $subscriber_count,
                    description = $description,
                    banner_url = $banner_url
                WHERE channel_id = $channel_id"#,
            )
            .bind(("channel_id", merged.id))
            .bind(("name", merged.name))
            .bind(("handle", merged.handle))
            .bind(("avatar_url", merged.avatar_url))
            .bind(("subscriber_count", merged.subscriber_count))
            .bind(("description", merged.description))
            .bind(("banner_url", merged.banner_url))
            .await?
            .check()?;
        Ok(true)
    }

    async fn upsert_playlist(&self, owner: &str, playlist: &Playlist) -> Result<()> {
        self.db
            .query(
                r#"UPSERT type::record('playlist', $record_key) SET
                    owner = type::record($owner),
                    playlist_id = $playlist_id,
                    name = $name,
                    video_ids = $video_ids,
                    updated_at = time::now()"#,
            )
            .bind(("owner", owner.to_string()))
            .bind(("record_key", Self::owner_key(owner, &playlist.id)))
            .bind(("playlist_id", playlist.id.clone()))
            .bind(("name", playlist.name.clone()))
            .bind(("video_ids", playlist.video_ids.clone()))
            .await?
            .check()?;
        Ok(())
    }

    async fn upsert_subscription_group(
        &self,
        owner: &str,
        group: &SubscriptionGroup,
    ) -> Result<()> {
        self.db
            .query(
                r#"UPSERT type::record('subscription_group', $record_key) SET
                    owner = type::record($owner),
                    group_id = $group_id,
                    name = $name,
                    channel_ids = $channel_ids,
                    updated_at = time::now()"#,
            )
            .bind(("owner", owner.to_string()))
            .bind(("record_key", Self::owner_key(owner, &group.id)))
            .bind(("group_id", group.id.clone()))
            .bind(("name", group.name.clone()))
            .bind(("channel_ids", group.channel_ids.clone()))
            .await?
            .check()?;
        Ok(())
    }

    pub async fn comments_page(&self, _video_id: &str, next_page: &str) -> Result<CommentsPage> {
        if next_page.starts_with("local-comments:") {
            let paginator = decode_page::<RustyComment>(next_page, "local-comments")?;
            let page = youtube_call(
                paginator.next(self.youtube.query()),
                "fetch next direct comments page",
            )
            .await?
            .ok_or_else(|| anyhow!("comments are exhausted"))?;
            return Ok(rusty_comments_page(&page));
        }
        Ok(CommentsPage::default())
    }

    async fn fetch_direct_video_details(
        &self,
        video_id: &str,
        fallback_video: Option<Video>,
        fallback_channel: Option<Channel>,
    ) -> Result<VideoDetails> {
        let query = self.youtube.query();
        let (details, player) = tokio::join!(
            youtube_call(
                query.video_details(video_id),
                "extract direct YouTube video details"
            ),
            youtube_call(
                query.player_from_clients(video_id, PLAYER_CLIENTS),
                "extract direct YouTube player"
            ),
        );
        let details = details?;
        let comments = match youtube_call(
            details.top_comments.clone().next(query),
            "extract direct YouTube comments",
        )
        .await
        {
            Ok(Some(page)) => rusty_comments_page(&page),
            _ => CommentsPage {
                disabled: details.top_comments.ctoken.is_none(),
                remote_available: true,
                ..CommentsPage::default()
            },
        };
        Ok(normalize_rusty_video_details(
            video_id,
            details,
            player.ok(),
            comments,
            fallback_video,
            fallback_channel,
        ))
    }

    pub async fn video_details(&self, owner: &str, video_id: &str) -> Result<VideoDetails> {
        let details = self.video_details_for_anyone(owner, video_id).await?;
        self.with_owner_follow_state(owner, details).await
    }

    /// The details any account would see. The follow state in them is not
    /// this account's - [`Self::video_details`] sets that.
    async fn video_details_for_anyone(&self, owner: &str, video_id: &str) -> Result<VideoDetails> {
        if let Some(cached) = self.read_video_details_cache(video_id, true).await? {
            return Ok(self.proxy_captions(cached).await);
        }
        let stale = self.read_video_details_cache(video_id, false).await?;
        let library = self.library_snapshot(owner).await?;
        let local_video = library
            .videos
            .iter()
            .find(|video| video.id == video_id)
            .cloned();
        let local_channel = local_video.as_ref().and_then(|video| {
            library
                .channels
                .iter()
                .find(|channel| channel.id == video.channel_id)
                .cloned()
        });
        let local_related = library
            .videos
            .iter()
            .filter(|video| video.id != video_id)
            .take(8)
            .cloned()
            .collect::<Vec<_>>();

        if let Ok(details) = self
            .fetch_direct_video_details(video_id, local_video.clone(), local_channel.clone())
            .await
        {
            if let Some(channel) = &details.channel {
                self.upsert_discovered_channel(channel).await?;
            }
            self.upsert_feed_video(&details.video, video_sort_key(&details.video))
                .await?;
            for related in &details.related_videos {
                self.upsert_feed_video(related, video_sort_key(related))
                    .await?;
            }
            self.write_video_details_cache(&details).await?;
            return Ok(self.proxy_captions(details).await);
        }

        // Direct extraction is the only remote source. When it fails, serve the
        // most recent cached payload, then fall back to whatever the library
        // already knows about the video.
        if let Some(cached) = stale {
            return Ok(self.proxy_captions(cached).await);
        }
        let video = local_video.ok_or_else(|| anyhow!("video is not in the local cache"))?;
        Ok(VideoDetails {
            video,
            channel: local_channel,
            description: String::new(),
            like_count: 0,
            dislike_count: 0,
            captions: Vec::new(),
            chapters: Vec::new(),
            preview_frames: None,
            related_videos: local_related,
            comments: CommentsPage::default(),
            remote_available: false,
        })
    }

    async fn read_video_details_cache(
        &self,
        video_id: &str,
        fresh_only: bool,
    ) -> Result<Option<VideoDetails>> {
        let query = if fresh_only {
            "SELECT payload_json FROM video_detail_cache WHERE video_id = $video_id AND fetched_at > time::now() - 2h LIMIT 1"
        } else {
            "SELECT payload_json FROM video_detail_cache WHERE video_id = $video_id LIMIT 1"
        };
        let cached: Vec<DbVideoDetailCache> = self
            .db
            .query(query)
            .bind(("video_id", video_id.to_string()))
            .await?
            .take(0)?;
        Ok(cached
            .into_iter()
            .next()
            .and_then(|cached| serde_json::from_str(&cached.payload_json).ok()))
    }

    async fn write_video_details_cache(&self, details: &VideoDetails) -> Result<()> {
        self.db
            .query(
                r#"UPSERT type::record('video_detail_cache', $record_key) SET
                    video_id = $video_id,
                    payload_json = $payload_json,
                    fetched_at = time::now()"#,
            )
            .bind(("record_key", details.video.id.clone()))
            .bind(("video_id", details.video.id.clone()))
            .bind(("payload_json", serde_json::to_string(details)?))
            .await?
            .check()?;
        Ok(())
    }

    async fn channel_is_subscribed(&self, channel_id: &str) -> Result<bool> {
        let flags: Vec<DbSubscriptionFlag> = self
            .db
            .query("SELECT subscribed FROM channel WHERE channel_id = $channel_id LIMIT 1")
            .bind(("channel_id", channel_id.to_string()))
            .await?
            .take(0)?;
        Ok(flags
            .first()
            .map(|channel| channel.subscribed)
            .unwrap_or(false))
    }

    async fn search_direct(&self, query: &str, filter: &str) -> Result<SearchResults> {
        let result = youtube_call(
            self.youtube.query().search::<YouTubeItem, _>(query),
            "search YouTube directly",
        )
        .await?;
        let mut videos = Vec::new();
        let mut channels = Vec::new();
        for item in &result.items.items {
            match item {
                YouTubeItem::Video(item) if filter != "channels" => {
                    let video = rusty_video_item_to_video(item, None, item.is_short);
                    self.upsert_feed_video(&video, video_sort_key(&video))
                        .await?;
                    push_unique_video(&mut videos, video);
                }
                YouTubeItem::Channel(item) if filter != "videos" => {
                    let mut channel = rusty_channel_item_to_channel(item);
                    channel.subscribed = self.channel_is_subscribed(&channel.id).await?;
                    self.upsert_discovered_channel(&channel).await?;
                    push_unique_channel(&mut channels, channel);
                }
                _ => {}
            }
        }
        Ok(SearchResults {
            query: query.to_string(),
            videos,
            channels,
            suggestion: result.corrected_query,
            remote_available: true,
            next_page: encode_page("local-search", &result.items),
        })
    }

    async fn classify_rss_videos(
        &self,
        channel: &Channel,
        items: Vec<RustyChannelRssVideo>,
    ) -> Vec<(Video, String)> {
        use futures_util::{StreamExt, stream};

        stream::iter(items.into_iter().map(|item| async move {
            let video = Video {
                id: item.id,
                title: item.name,
                channel_id: channel.id.clone(),
                channel_name: channel.name.clone(),
                thumbnail_url: item.thumbnail.url,
                published_at: item.publish_date.to_string(),
                duration_seconds: 0,
                view_count: compact_count(item.view_count as i64, " views"),
                progress_seconds: 0,
                watched: false,
                is_live: false,
                is_short: false,
                audio_only: false,
            };
            let video = self.enrich_video_player(video).await;
            let published = video.published_at.clone();
            (video, published)
        }))
        .buffer_unordered(4)
        .collect()
        .await
    }

    async fn enrich_video_player(&self, mut video: Video) -> Video {
        let Ok(player) = youtube_call(
            self.youtube
                .query()
                .player_from_clients(&video.id, PLAYER_CLIENTS),
            "enrich YouTube upload",
        )
        .await
        else {
            return video;
        };
        let vertical = player
            .video_streams
            .iter()
            .chain(player.video_only_streams.iter())
            .any(|stream| stream.height > stream.width);
        video.duration_seconds = player.details.duration as u64;
        video.is_live = player.details.is_live;
        video.is_short = !video.is_live
            && video.duration_seconds > 0
            && video.duration_seconds <= 180
            && vertical;
        if let Some(title) = player.details.name.filter(|title| !title.trim().is_empty()) {
            video.title = title;
        }
        if let Some(thumbnail) = best_thumbnail(&player.details.thumbnail) {
            video.thumbnail_url = thumbnail;
        }
        if let Some(views) = player.details.view_count {
            video.view_count = compact_count(views as i64, " views");
        }
        video
    }

    pub async fn channel_details(&self, owner: &str, channel_id: &str) -> Result<ChannelDetails> {
        let query = self.youtube.query();
        let (rss_result, videos_result, shorts_result, live_result) = tokio::join!(
            youtube_call(query.channel_rss(channel_id), "extract channel RSS"),
            youtube_call(query.channel_videos(channel_id), "extract channel videos"),
            youtube_call(
                query.channel_videos_tab(channel_id, ChannelVideoTab::Shorts),
                "extract channel Shorts",
            ),
            youtube_call(
                query.channel_videos_tab(channel_id, ChannelVideoTab::Live),
                "extract channel livestreams",
            ),
        );
        if let Ok(videos_channel) = videos_result {
            // This account's follow, not the instance's - see `user_follows`.
            let follow = self.user_follows(owner, channel_id).await?;
            let mut channel = rusty_channel_to_channel(
                &videos_channel,
                follow.is_some_and(|(subscribed, _)| subscribed),
            );
            if let Some((_, content)) = follow {
                channel.subscription_content = content;
            }
            let mut videos = rusty_page(
                channel_id,
                ChannelMediaTab::Videos,
                &videos_channel.content,
                &channel.name,
            );
            let mut shorts = shorts_result
                .ok()
                .map(|result| {
                    rusty_page(
                        channel_id,
                        ChannelMediaTab::Shorts,
                        &result.content,
                        &channel.name,
                    )
                })
                .unwrap_or_else(|| {
                    empty_channel_page(channel_id, ChannelMediaTab::Shorts, "Direct YouTube")
                });
            let mixed_shorts = videos
                .videos
                .iter()
                .filter(|video| video.is_short)
                .cloned()
                .collect::<Vec<_>>();
            videos.videos.retain(|video| !video.is_short);
            for video in mixed_shorts {
                push_unique_video(&mut shorts.videos, video);
            }
            // The extractor cannot read the Shorts tab, so ask yt-dlp before
            // falling back to guessing from the RSS feed.
            if shorts.videos.is_empty() {
                for video in self.ytdlp_channel_shorts(channel_id, &channel.name).await {
                    push_unique_video(&mut shorts.videos, video);
                }
            }
            if shorts.videos.is_empty()
                && let Ok(rss) = rss_result
            {
                for (video, _) in self.classify_rss_videos(&channel, rss.videos).await {
                    if video.is_short {
                        push_unique_video(&mut shorts.videos, video);
                    } else if video.is_live {
                        // Added below once the live page exists.
                    }
                }
            }
            let live = live_result
                .ok()
                .map(|result| {
                    rusty_page(
                        channel_id,
                        ChannelMediaTab::Live,
                        &result.content,
                        &channel.name,
                    )
                })
                .unwrap_or_else(|| {
                    empty_channel_page(channel_id, ChannelMediaTab::Live, "Direct YouTube")
                });
            self.upsert_discovered_channel(&channel).await?;
            for video in videos
                .videos
                .iter()
                .chain(shorts.videos.iter())
                .chain(live.videos.iter())
            {
                let sort = video_sort_key(video);
                self.upsert_feed_video(video, sort).await?;
            }
            return Ok(ChannelDetails {
                channel,
                videos,
                shorts,
                live,
                remote_available: true,
            });
        }

        let library = self.library_snapshot(owner).await?;
        let channel = library
            .channels
            .into_iter()
            .find(|channel| channel.id == channel_id)
            .ok_or_else(|| anyhow!("channel is unavailable"))?;
        let mut videos = Vec::new();
        let mut shorts = Vec::new();
        let mut live = Vec::new();
        for video in library
            .videos
            .into_iter()
            .filter(|video| video.channel_id == channel_id)
        {
            if video.is_short {
                shorts.push(video);
            } else if video.is_live {
                live.push(video);
            } else {
                videos.push(video);
            }
        }
        Ok(ChannelDetails {
            channel,
            videos: ChannelMediaPage {
                channel_id: channel_id.into(),
                tab: ChannelMediaTab::Videos,
                videos,
                next_page: None,
                source: "Cache".into(),
            },
            shorts: ChannelMediaPage {
                channel_id: channel_id.into(),
                tab: ChannelMediaTab::Shorts,
                videos: shorts,
                next_page: None,
                source: "Cache".into(),
            },
            live: ChannelMediaPage {
                channel_id: channel_id.into(),
                tab: ChannelMediaTab::Live,
                videos: live,
                next_page: None,
                source: "Cache".into(),
            },
            remote_available: false,
        })
    }

    /// A page of a channel's uploads.
    ///
    /// Pure catalog data - the rows here are the same for everyone. The owner
    /// is taken so the endpoint still refuses an unauthenticated caller, and is
    /// otherwise unused: watched state reaches these cards from the client's
    /// own library, which is already scoped to the account.
    pub async fn channel_media_page(
        &self,
        _owner: &str,
        channel_id: &str,
        tab: ChannelMediaTab,
        next_page: &str,
    ) -> Result<ChannelMediaPage> {
        if next_page.starts_with("local-channel:") {
            let paginator = decode_page::<RustyVideoItem>(next_page, "local-channel")?;
            let page = youtube_call(
                paginator.next(self.youtube.query()),
                "fetch next direct channel page",
            )
            .await?
            .ok_or_else(|| anyhow!("channel page is exhausted"))?;
            let channel_name = self
                // Just the cached name; nothing here is the viewer's.
                .library_snapshot("")
                .await?
                .channels
                .into_iter()
                .find(|channel| channel.id == channel_id)
                .map(|channel| channel.name)
                .unwrap_or_else(|| "YouTube channel".into());
            let result = rusty_page(channel_id, tab, &page, &channel_name);
            for video in &result.videos {
                self.upsert_feed_video(video, video_sort_key(video)).await?;
            }
            return Ok(result);
        }
        Err(anyhow!("invalid channel continuation"))
    }

    pub async fn search_catalog(
        &self,
        owner: &str,
        query: &str,
        filter: &str,
    ) -> Result<SearchResults> {
        let results = self.search_catalog_for_anyone(owner, query, filter).await?;
        self.with_owner_follows_in_search(owner, results).await
    }

    /// Search as any account would see it; the follow state in it is set by
    /// [`Self::search_catalog`].
    async fn search_catalog_for_anyone(
        &self,
        owner: &str,
        query: &str,
        filter: &str,
    ) -> Result<SearchResults> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(SearchResults {
                query: String::new(),
                videos: Vec::new(),
                channels: Vec::new(),
                suggestion: None,
                remote_available: true,
                next_page: None,
            });
        }
        let filter = match filter {
            "videos" | "channels" | "playlists" => filter,
            _ => "all",
        };
        let cache_key = (query.to_lowercase(), filter.to_string());
        if let Some(cached) = self.search_cache.read().await.get(&cache_key)
            && cached.cached_at.elapsed() < SEARCH_CACHE_TTL
        {
            return Ok(cached.result.clone());
        }

        // Coalesce bursts from repeated submits or several clients. The cache is checked
        // again after taking the lock so identical searches produce one upstream request.
        let _guard = self.search_lock.lock().await;
        if let Some(cached) = self.search_cache.read().await.get(&cache_key)
            && cached.cached_at.elapsed() < SEARCH_CACHE_TTL
        {
            return Ok(cached.result.clone());
        }

        let result = match self.search_direct(query, filter).await {
            Ok(result) => result,
            // Never into the shared cache: this searches the asking account's
            // own library, and cached under the query alone it was served to
            // every other account that searched the same words.
            Err(_) => return self.search_cached(owner, query, filter).await,
        };

        self.search_cache.write().await.insert(
            cache_key,
            CachedSearch {
                result: result.clone(),
                cached_at: std::time::Instant::now(),
            },
        );
        Ok(result)
    }

    pub async fn search_page(
        &self,
        owner: &str,
        query_text: &str,
        filter: &str,
        next_page: &str,
    ) -> Result<SearchResults> {
        let results = self
            .search_page_for_anyone(query_text, filter, next_page)
            .await?;
        self.with_owner_follows_in_search(owner, results).await
    }

    async fn search_page_for_anyone(
        &self,
        query_text: &str,
        filter: &str,
        next_page: &str,
    ) -> Result<SearchResults> {
        let paginator = decode_page::<YouTubeItem>(next_page, "local-search")?;
        let page = youtube_call(
            paginator.next(self.youtube.query()),
            "fetch next direct YouTube search page",
        )
        .await?
        .ok_or_else(|| anyhow!("search results are exhausted"))?;
        let mut videos = Vec::new();
        let mut channels = Vec::new();
        for item in &page.items {
            match item {
                YouTubeItem::Video(item) if filter != "channels" => {
                    let video = rusty_video_item_to_video(item, None, item.is_short);
                    self.upsert_feed_video(&video, video_sort_key(&video))
                        .await?;
                    push_unique_video(&mut videos, video);
                }
                YouTubeItem::Channel(item) if filter != "videos" => {
                    let mut channel = rusty_channel_item_to_channel(item);
                    channel.subscribed = self.channel_is_subscribed(&channel.id).await?;
                    self.upsert_discovered_channel(&channel).await?;
                    push_unique_channel(&mut channels, channel);
                }
                _ => {}
            }
        }
        Ok(SearchResults {
            query: query_text.to_string(),
            videos,
            channels,
            suggestion: None,
            remote_available: true,
            next_page: encode_page("local-search", &page),
        })
    }

    async fn search_cached(&self, owner: &str, query: &str, filter: &str) -> Result<SearchResults> {
        let snapshot = self.library_snapshot(owner).await?;
        let needle = query.to_lowercase();
        let videos = if filter == "channels" {
            Vec::new()
        } else {
            snapshot
                .videos
                .into_iter()
                .filter(|video| {
                    video.title.to_lowercase().contains(&needle)
                        || video.channel_name.to_lowercase().contains(&needle)
                })
                .collect()
        };
        let channels = if filter == "videos" {
            Vec::new()
        } else {
            snapshot
                .channels
                .into_iter()
                .filter(|channel| {
                    channel.name.to_lowercase().contains(&needle)
                        || channel.handle.to_lowercase().contains(&needle)
                })
                .collect()
        };
        Ok(SearchResults {
            query: query.to_string(),
            videos,
            channels,
            suggestion: None,
            remote_available: false,
            next_page: None,
        })
    }

    async fn refresh_channel_rss_fast(&self, channel: Channel) -> Result<usize> {
        let xml = self
            .http
            .get("https://www.youtube.com/feeds/videos.xml")
            .query(&[("channel_id", channel.id.as_str())])
            .send()
            .await
            .context("request YouTube channel RSS")?
            .error_for_status()
            .context("YouTube rejected channel RSS request")?
            .text()
            .await
            .context("read YouTube channel RSS")?;
        let entries = parse_youtube_feed(&xml)?;
        if entries.is_empty() {
            return Err(anyhow!("channel RSS is empty for {}", channel.id));
        }
        for entry in &entries {
            let video = feed_entry_video(entry, &channel);
            self.upsert_feed_hint(&video, entry.published.clone())
                .await?;
        }
        self.db
            .query("UPDATE channel SET last_polled_at = time::now() WHERE channel_id = $channel_id")
            .bind(("channel_id", channel.id))
            .await?
            .check()?;
        Ok(entries.len())
    }

    /// Channels with at least one video of unknown length, most recent first.
    ///
    /// Reads rows rather than grouping: the newest videos are the ones a viewer
    /// is about to filter or sort, and a scan of a few hundred is cheaper than
    /// an aggregate over the whole catalog.
    async fn channels_missing_durations(&self, limit: usize) -> Result<Vec<String>> {
        // `published_sort` is selected because SurrealDB 3.x refuses to order by
        // an idiom the projection does not carry.
        #[derive(Debug, Deserialize, SurrealValue)]
        struct Row {
            channel_id: String,
            #[allow(dead_code)]
            published_sort: String,
        }
        let rows: Vec<Row> = self
            .db
            .query(
                "SELECT channel_id, published_sort FROM video WHERE duration_seconds = 0 AND is_live = false ORDER BY published_sort DESC LIMIT $scan",
            )
            .bind(("scan", FEED_DURATION_SCAN_ROWS as i64))
            .await?
            .take(0)?;
        let mut seen = HashSet::new();
        let mut channels = Vec::new();
        for row in rows {
            if !row.channel_id.starts_with("UC") || row.channel_id.starts_with("UC-tawny") {
                continue;
            }
            if seen.insert(row.channel_id.clone()) {
                channels.push(row.channel_id);
            }
            if channels.len() >= limit {
                break;
            }
        }
        Ok(channels)
    }

    /// Channel ids whose uploads are part of this viewer's feed. The poller has
    /// no viewer, so its empty owner means every channel followed by anyone on
    /// the instance.
    async fn feed_channel_ids(&self, owner: &str) -> Result<Vec<String>> {
        if !owner.is_empty() {
            return Ok(self
                .user_subscriptions(owner)
                .await?
                .into_iter()
                .filter_map(|(channel_id, (subscribed, _))| subscribed.then_some(channel_id))
                .collect());
        }

        #[derive(Debug, Deserialize, SurrealValue)]
        struct Row {
            channel_id: String,
        }
        let rows: Vec<Row> = self
            .db
            .query("SELECT channel_id FROM channel WHERE subscribed = true")
            .await?
            .take(0)?;
        Ok(rows.into_iter().map(|row| row.channel_id).collect())
    }

    /// Every video this account has saved to a playlist.
    ///
    /// A saved video is this viewer's own reference to it, whatever channel it
    /// came from, so it earns the same hydration a subscribed upload gets. A
    /// playlist is the one place a reader assembles videos from channels they
    /// do not follow, which is exactly where a channel-only allowlist left the
    /// duration filters with nothing to filter.
    async fn user_playlist_video_ids(&self, owner: &str) -> Result<HashSet<String>> {
        if owner.is_empty() {
            return Ok(HashSet::new());
        }
        #[derive(Debug, Deserialize, SurrealValue)]
        struct Row {
            video_ids: Vec<String>,
        }
        let rows: Vec<Row> = self
            .db
            .query("SELECT video_ids FROM playlist WHERE owner = type::record($owner)")
            .bind(("owner", owner.to_string()))
            .await?
            .check()?
            .take(0)?;
        Ok(rows
            .into_iter()
            .flat_map(|row| row.video_ids.into_iter())
            .collect())
    }

    /// Newest exact rows that still need a runtime, scoped to the calling
    /// viewer rather than the server's entire shared catalog.
    async fn feed_duration_candidates(&self, owner: &str, limit: usize) -> Result<Vec<Video>> {
        let channel_ids = self.feed_channel_ids(owner).await?;
        if channel_ids.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let rows: Vec<DbVideo> = self
            .db
            .query(
                "SELECT video_id, title, channel_id, channel_name, thumbnail_url, published_at, published_sort, duration_seconds, view_count, is_live, is_short, progress_seconds, watched, audio_only FROM video WHERE duration_seconds = 0 AND is_live = false AND channel_id IN $channel_ids ORDER BY published_sort DESC LIMIT $limit",
            )
            .bind(("channel_ids", channel_ids))
            .bind(("limit", limit as i64))
            .await?
            .take(0)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Resolve each requested upload itself. Channel listings are efficient,
    /// but they can omit Shorts and older rows; this exact path is what makes
    /// progress deterministic for visible cards.
    async fn resolve_video_durations(&self, videos: Vec<Video>) -> Result<Vec<Video>> {
        use futures_util::{StreamExt, stream};

        let enriched = stream::iter(
            videos
                .into_iter()
                .map(|video| async move { self.enrich_video_player(video).await }),
        )
        .buffer_unordered(4)
        .collect::<Vec<_>>()
        .await;
        let mut resolved = Vec::new();
        for video in enriched {
            if video.duration_seconds == 0 && !video.is_live {
                continue;
            }
            self.upsert_feed_video(&video, video_sort_key(&video))
                .await?;
            resolved.push(video);
        }
        Ok(resolved)
    }

    /// Metadata hydration for the cards a viewer is looking at, wherever they
    /// are looking at them.
    ///
    /// Accepts only videos already in the catalog that this account has a claim
    /// on - an upload from a channel it follows, or a video it saved to a
    /// playlist - and caps the request before any extractor work begins, so the
    /// endpoint cannot be driven as a general-purpose extractor proxy.
    pub async fn hydrate_video_durations(
        &self,
        owner: &str,
        video_ids: Vec<String>,
    ) -> Result<Vec<Video>> {
        let (allowed_channels, allowed_videos) = tokio::try_join!(
            async {
                Ok::<_, anyhow::Error>(
                    self.feed_channel_ids(owner)
                        .await?
                        .into_iter()
                        .collect::<HashSet<_>>(),
                )
            },
            self.user_playlist_video_ids(owner),
        )?;
        if allowed_channels.is_empty() && allowed_videos.is_empty() {
            return Ok(Vec::new());
        }
        let mut seen = HashSet::new();
        let video_ids = video_ids
            .into_iter()
            .filter(|video_id| seen.insert(video_id.clone()))
            .take(DURATION_HYDRATION_LIMIT)
            .collect::<Vec<_>>();
        if video_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows: Vec<DbVideo> = self
            .db
            .query(
                "SELECT video_id, title, channel_id, channel_name, thumbnail_url, published_at, published_sort, duration_seconds, view_count, is_live, is_short, progress_seconds, watched, audio_only FROM video WHERE video_id IN $video_ids",
            )
            .bind(("video_ids", video_ids))
            .await?
            .take(0)?;
        let mut resolved = Vec::new();
        let mut missing = Vec::new();
        for video in rows.into_iter().map(Video::from) {
            if !allowed_channels.contains(&video.channel_id) && !allowed_videos.contains(&video.id)
            {
                continue;
            }
            if video.duration_seconds > 0 || video.is_live {
                resolved.push(video);
            } else {
                missing.push(video);
            }
        }
        resolved.extend(self.resolve_video_durations(missing).await?);
        Ok(resolved)
    }

    /// Write lengths a listing reported onto rows that still have none.
    ///
    /// Guarded on `duration_seconds = 0` in the statement rather than in Rust:
    /// the player writes an exact length when a video is opened, and a listing's
    /// rounded seconds must never overwrite it.
    async fn apply_video_durations(&self, rows: &[(String, u64, String)]) -> Result<usize> {
        let mut filled = 0;
        for (video_id, duration_seconds, view_count) in rows {
            let updated: Vec<serde_json::Value> = self
                .db
                .query(
                    r#"UPDATE video SET
                        duration_seconds = $duration_seconds,
                        view_count = IF $view_count = "" THEN view_count ELSE $view_count END
                    WHERE video_id = $video_id AND duration_seconds = 0 RETURN video_id"#,
                )
                .bind(("video_id", video_id.clone()))
                .bind(("duration_seconds", *duration_seconds as i64))
                .bind(("view_count", view_count.clone()))
                .await?
                .take(0)?;
            filled += updated.len();
        }
        Ok(filled)
    }

    /// Fill in lengths for videos that arrived over RSS.
    ///
    /// YouTube's subscription feed is an Atom document, and the format carries
    /// no duration at all - which is why a video imported from it has none until
    /// something opens it, and why a duration filter used to discard almost the
    /// whole library. A channel's own uploads tab does report lengths, and one
    /// call covers its recent uploads, so this costs one request per channel
    /// with a gap rather than one per video.
    ///
    /// Best effort throughout. A refresh that cannot reach the extractor still
    /// returns the feed it did reconcile; the lengths are simply filled in on a
    /// later refresh.
    async fn backfill_durations(&self, owner: &str) -> usize {
        use futures_util::{StreamExt, stream};

        let Ok(channels) = self
            .channels_missing_durations(FEED_DURATION_BACKFILL_CHANNELS)
            .await
        else {
            return 0;
        };
        let listings = stream::iter(channels.into_iter().map(|channel_id| async move {
            youtube_call(
                self.youtube.query().channel_videos(&channel_id),
                "extract channel videos for lengths",
            )
            .await
        }))
        .buffer_unordered(8)
        .collect::<Vec<_>>()
        .await;

        let mut filled = 0;
        for listing in listings.into_iter().flatten() {
            let rows = listing
                .content
                .items
                .iter()
                .filter(|item| !item.is_live)
                .filter_map(|item| {
                    let duration = u64::from(item.duration?);
                    if duration == 0 {
                        return None;
                    }
                    let views = item
                        .view_count
                        .map(|count| compact_count(count as i64, " views"))
                        .unwrap_or_default();
                    Some((item.id.clone(), duration, views))
                })
                .collect::<Vec<_>>();
            filled += self.apply_video_durations(&rows).await.unwrap_or(0);
        }
        // A listing can leave exactly the same unknown Shorts/older uploads at
        // the top forever. Resolve the newest visible page directly so every
        // successful refresh advances the feed even when that happens.
        if let Ok(candidates) = self
            .feed_duration_candidates(owner, FEED_DURATION_VISIBLE_BATCH)
            .await
            && let Ok(resolved) = self.resolve_video_durations(candidates).await
        {
            filled += resolved.len();
        }
        filled
    }

    async fn reconcile_channels_rss(
        &self,
        channels: Vec<Channel>,
    ) -> (usize, Vec<String>, Vec<String>) {
        use futures_util::{StreamExt, stream};

        let results = stream::iter(channels.into_iter().map(|channel| async move {
            let channel_id = channel.id.clone();
            (channel_id, self.refresh_channel_rss_fast(channel).await)
        }))
        .buffer_unordered(FEED_RSS_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;
        let mut imported = 0;
        let mut refreshed = Vec::new();
        let mut failed = Vec::new();
        for (channel_id, result) in results {
            match result {
                Ok(count) => {
                    imported += count;
                    refreshed.push(channel_id);
                }
                Err(_) => failed.push(channel_id),
            }
        }
        (imported, refreshed, failed)
    }

    async fn refresh_channel_local(&self, channel: Channel) -> Result<usize> {
        let query = self.youtube.query();
        let (rss_result, videos_result, shorts_result, live_result) = tokio::join!(
            youtube_call(query.channel_rss(&channel.id), "extract channel RSS"),
            youtube_call(query.channel_videos(&channel.id), "extract channel videos"),
            youtube_call(
                query.channel_videos_tab(&channel.id, ChannelVideoTab::Shorts),
                "extract channel Shorts",
            ),
            youtube_call(
                query.channel_videos_tab(&channel.id, ChannelVideoTab::Live),
                "extract channel livestreams",
            ),
        );
        let mut normalized = HashMap::<String, (Video, String)>::new();

        if let Ok(shorts) = &shorts_result {
            for item in &shorts.content.items {
                let video =
                    rusty_video_item_to_video(item, Some((&channel.id, &channel.name)), true);
                let sort = item
                    .publish_date
                    .map(|date| date.to_string())
                    .unwrap_or_else(search_sort_key);
                normalized.insert(video.id.clone(), (video, sort));
            }
        }
        if let Ok(videos) = &videos_result {
            let mut enriched_channel = rusty_channel_to_channel(videos, channel.subscribed);
            enriched_channel.subscribed = true;
            enriched_channel.subscription_content = channel.subscription_content;
            self.upsert_channel(&enriched_channel).await?;
            for item in &videos.content.items {
                let video = rusty_video_item_to_video(
                    item,
                    Some((&channel.id, &channel.name)),
                    item.is_short,
                );
                let sort = item
                    .publish_date
                    .map(|date| date.to_string())
                    .unwrap_or_else(search_sort_key);
                normalized.insert(video.id.clone(), (video, sort));
            }
        }
        if let Ok(live) = &live_result {
            for item in &live.content.items {
                let video =
                    rusty_video_item_to_video(item, Some((&channel.id, &channel.name)), false);
                let sort = item
                    .publish_date
                    .map(|date| date.to_string())
                    .unwrap_or_else(search_sort_key);
                normalized.insert(video.id.clone(), (video, sort));
            }
        }
        if let Ok(rss) = rss_result {
            let uncategorized = rss
                .videos
                .into_iter()
                .filter(|item| !normalized.contains_key(&item.id))
                .collect::<Vec<_>>();
            for (video, published) in self.classify_rss_videos(&channel, uncategorized).await {
                normalized
                    .entry(video.id.clone())
                    .or_insert((video, published));
            }
        }

        if normalized.is_empty() {
            return Err(anyhow!("no feed items extracted for {}", channel.id));
        }
        let count = normalized.len();
        for (_, (video, published_sort)) in normalized {
            self.upsert_feed_video(&video, published_sort).await?;
        }
        self.db
            .query(
                "UPDATE channel SET last_polled_at = time::now(), last_full_refresh_at = time::now() WHERE channel_id = $channel_id",
            )
            .bind(("channel_id", channel.id))
            .await?
            .check()?;
        Ok(count)
    }

    pub async fn refresh_feed(&self, owner: &str) -> Result<FeedRefreshResult> {
        let channels: Vec<DbChannel> = self
            .db
            .query(
                "SELECT channel_id, name, handle, avatar_url, subscriber_count, subscribed, subscription_content, description, banner_url, last_polled_at FROM channel WHERE subscribed = true ORDER BY last_polled_at ASC",
            )
            .await?
            .take(0)?;
        let real_channels = channels
            .into_iter()
            .map(Channel::from)
            .filter(|channel| channel.id.starts_with("UC") && !channel.id.starts_with("UC-tawny"))
            .collect::<Vec<_>>();
        let channel_ids = real_channels
            .iter()
            .map(|channel| channel.id.clone())
            .collect::<Vec<_>>();
        let total_channels = real_channels.len();
        let mut imported = 0;
        let mut sources = Vec::new();
        let mut covered_channels = HashSet::<String>::new();

        if self.websub_callback_url.is_some() {
            sources.push("YouTube WebSub".into());
            covered_channels.extend(channel_ids.iter().cloned());
        }

        // Without a push subscription there is no accelerated source, so every
        // subscribed channel is reconciled over RSS rather than a capped slice.
        let accelerated = self.websub_callback_url.is_some();
        let reconciliation_count = reconciliation_limit(total_channels, accelerated);
        let rss_ids = real_channels
            .iter()
            .take(reconciliation_count)
            .map(|channel| channel.id.clone())
            .collect::<HashSet<_>>();
        let rss_channels = real_channels
            .into_iter()
            .filter(|channel| rss_ids.contains(&channel.id))
            .collect::<Vec<_>>();
        if !rss_channels.is_empty() {
            let (rss_imported, rss_refreshed, _) = self.reconcile_channels_rss(rss_channels).await;
            imported += rss_imported;
            covered_channels.extend(rss_refreshed);
            sources.push("YouTube RSS reconciliation".into());
        }
        if sources.is_empty() {
            sources.push("Local cache".into());
        }
        // After reconciliation, so it sees the videos this refresh just
        // imported, and before the snapshot, so their lengths travel with them.
        self.backfill_durations(owner).await;
        let refreshed_channels = covered_channels.len();
        let failed_channels = total_channels.saturating_sub(refreshed_channels);
        Ok(FeedRefreshResult {
            library: self.library_snapshot(owner).await?,
            imported,
            refreshed_channels,
            failed_channels,
            sources,
        })
    }

    async fn request_websub_subscription(&self, channel_id: &str, subscribe: bool) -> Result<bool> {
        let Some(callback) = self.websub_callback_url.as_deref() else {
            return Ok(false);
        };
        if !channel_id.starts_with("UC") || channel_id.starts_with("UC-tawny") {
            return Ok(false);
        }
        let topic = format!("https://www.youtube.com/xml/feeds/videos.xml?channel_id={channel_id}");
        let mode = if subscribe {
            "subscribe"
        } else {
            "unsubscribe"
        };
        let form = [
            ("hub.callback", callback.to_string()),
            ("hub.topic", topic),
            ("hub.verify", "async".to_string()),
            ("hub.mode", mode.to_string()),
            ("hub.verify_token", self.websub_secret.clone()),
            ("hub.secret", self.websub_secret.clone()),
            ("hub.lease_seconds", (10 * 24 * 60 * 60).to_string()),
        ];
        self.http
            .post("https://pubsubhubbub.appspot.com/subscribe")
            .form(&form)
            .send()
            .await
            .context("request YouTube WebSub subscription")?
            .error_for_status()
            .context("YouTube WebSub hub rejected subscription")?;
        self.db
            .query(
                "UPDATE channel SET websub_status = $status, websub_requested_at = time::now() WHERE channel_id = $channel_id",
            )
            .bind(("channel_id", channel_id.to_string()))
            .bind(("status", format!("{mode}-requested")))
            .await?
            .check()?;
        Ok(true)
    }

    async fn renew_websub_subscriptions(&self) -> Result<usize> {
        use futures_util::{StreamExt, stream};

        let channels: Vec<DbSubscriptionState> = self
            .db
            .query(
                "SELECT channel_id, subscribed FROM channel WHERE subscribed = true AND (websub_requested_at = NONE OR websub_requested_at < time::now() - 3d)",
            )
            .await?
            .take(0)?;
        let results = stream::iter(channels.into_iter().map(|channel| async move {
            self.request_websub_subscription(&channel.channel_id, true)
                .await
        }))
        .buffer_unordered(WEBSUB_RENEW_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;
        Ok(results
            .into_iter()
            .filter(|result| result.as_ref().is_ok_and(|requested| *requested))
            .count())
    }

    async fn ingest_websub_notification(&self, xml: &str) -> Result<usize> {
        let entries = parse_youtube_feed(xml)?;
        let Some(channel_id) = entries
            .iter()
            .find_map(|entry| (!entry.channel_id.is_empty()).then(|| entry.channel_id.clone()))
        else {
            return Ok(0);
        };
        if !self.channel_is_subscribed(&channel_id).await? {
            return Ok(0);
        }
        // Catalog only: a WebSub push belongs to the instance, not to a viewer,
        // and all it needs from here is the channel's cached metadata.
        let library = self.library_snapshot("").await?;
        let channel = library
            .channels
            .into_iter()
            .find(|channel| channel.id == channel_id)
            .ok_or_else(|| anyhow!("WebSub channel is not cached"))?;

        let mut hints = Vec::with_capacity(entries.len());
        for entry in &entries {
            let video = feed_entry_video(entry, &channel);
            self.upsert_feed_hint(&video, entry.published.clone())
                .await?;
            hints.push(video);
        }
        self.db
            .query(
                "UPDATE channel SET last_websub_at = time::now(), last_polled_at = time::now() WHERE channel_id = $channel_id",
            )
            .bind(("channel_id", channel.id))
            .await?
            .check()?;

        use futures_util::{StreamExt, stream};
        let _ = stream::iter(hints.into_iter().map(|video| async move {
            let enriched = self.enrich_video_player(video).await;
            self.upsert_feed_video(&enriched, video_sort_key(&enriched))
                .await
        }))
        .buffer_unordered(4)
        .collect::<Vec<_>>()
        .await;
        Ok(entries.len())
    }

    pub async fn poll_subscriptions_once(&self) -> Result<usize> {
        // The poller has no viewer; it only wants the count.
        Ok(self.refresh_feed("").await?.imported)
    }
}

fn proxy_error(status: StatusCode, message: &str) -> Response {
    let mut response = Response::new(Body::from(message.to_string()));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
}

/// Identify a media container from its leading bytes.
///
/// YouTube serves every HLS segment as `application/octet-stream`, which leaves
/// the player unable to tell packed AAC from MPEG-TS. Shaka then assumes the
/// audio rendition is fMP4, builds an `audio/mp4` SourceBuffer, and the ADTS
/// bytes it appends decode to nothing: the video buffer fills, the audio buffer
/// stays empty, and the element sits at readyState 0 forever. Reporting the
/// real type is what lets the audio track play at all.
fn sniff_media_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"ID3") {
        return Some("audio/aac");
    }
    // ADTS sync word: 12 set bits, then layer/protection bits.
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] & 0xF6 == 0xF0 {
        return Some("audio/aac");
    }
    if bytes.first() == Some(&0x47) {
        return Some("video/mp2t");
    }
    if bytes.len() >= 8 && &bytes[4..8] == b"ftyp" {
        return Some("video/mp4");
    }
    if bytes.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return Some("video/webm");
    }
    if bytes.starts_with(b"OggS") {
        return Some("audio/ogg");
    }
    None
}

/// Whether the URL's path — ignoring query and fragment — ends with `suffix`.
///
/// Substring matching is wrong here. YouTube derives HLS segment URLs from the
/// playlist path, so a segment looks like `.../file/index.m3u8/sq/3/goap/...`
/// and *contains* `.m3u8` while being MPEG-TS media. Treating those as
/// playlists rewrites the media into a list of proxy URLs, and playback stalls
/// at readyState 0 with the manifest parsed but nothing ever buffered.
fn url_path_ends_with(url: &str, suffix: &str) -> bool {
    url.split(['?', '#'])
        .next()
        .unwrap_or(url)
        .ends_with(suffix)
}

fn resolved_media_url(base: &str, value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.starts_with("data:") || value.starts_with("blob:") {
        return None;
    }
    reqwest::Url::parse(value)
        .or_else(|_| reqwest::Url::parse(base)?.join(value))
        .ok()
        .map(Into::into)
}

async fn proxy_manifest_url(
    state: &AppServerState,
    base: &str,
    value: &str,
    request_headers: &[crate::models::PlaybackRequestHeader],
) -> Option<String> {
    let url = resolved_media_url(base, value)?;
    Some(
        state
            .register_proxy_target(url, request_headers.to_vec(), Duration::from_secs(90 * 60))
            .await,
    )
}

async fn rewrite_hls_manifest(
    state: &AppServerState,
    base: &str,
    manifest: &str,
    request_headers: &[crate::models::PlaybackRequestHeader],
) -> String {
    let mut output = String::with_capacity(manifest.len() + 256);
    for line in manifest.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('#') && !trimmed.is_empty() {
            if let Some(url) = proxy_manifest_url(state, base, trimmed, request_headers).await {
                output.push_str(&url);
            } else {
                output.push_str(line);
            }
        } else {
            let mut rewritten = line.to_string();
            let mut search_from = 0;
            while let Some(offset) = rewritten[search_from..].find("URI=\"") {
                let value_start = search_from + offset + 5;
                let Some(value_end_offset) = rewritten[value_start..].find('"') else {
                    break;
                };
                let value_end = value_start + value_end_offset;
                let value = rewritten[value_start..value_end].to_string();
                let Some(url) = proxy_manifest_url(state, base, &value, request_headers).await
                else {
                    break;
                };
                rewritten.replace_range(value_start..value_end, &url);
                search_from = value_start + url.len();
            }
            output.push_str(&rewritten);
        }
        output.push('\n');
    }
    output
}

async fn rewrite_dash_manifest(
    state: &AppServerState,
    base: &str,
    manifest: &str,
    request_headers: &[crate::models::PlaybackRequestHeader],
) -> String {
    let mut output = manifest.to_string();
    let mut search_from = 0;
    while let Some(start_offset) = output[search_from..].find("<BaseURL>") {
        let value_start = search_from + start_offset + "<BaseURL>".len();
        let Some(end_offset) = output[value_start..].find("</BaseURL>") else {
            break;
        };
        let value_end = value_start + end_offset;
        let value = output[value_start..value_end].to_string();
        let Some(url) = proxy_manifest_url(state, base, &value, request_headers).await else {
            search_from = value_end + "</BaseURL>".len();
            continue;
        };
        output.replace_range(value_start..value_end, &url);
        search_from = value_start + url.len() + "</BaseURL>".len();
    }
    output
}

async fn request_playback_upstream(
    state: &AppServerState,
    target: &ProxyTarget,
    request_headers: &HeaderMap,
) -> std::result::Result<reqwest::Response, reqwest::Error> {
    let mut request = state
        .media_http
        .get(&target.url)
        .header(reqwest::header::ACCEPT_ENCODING, "identity");
    for configured in &target.request_headers {
        if let (Ok(name), Ok(value)) = (
            reqwest::header::HeaderName::from_bytes(configured.name.as_bytes()),
            reqwest::header::HeaderValue::from_str(&configured.value),
        ) {
            request = request.header(name, value);
        }
    }
    for name in [header::RANGE, header::IF_RANGE, header::IF_NONE_MATCH] {
        if let Some(value) = request_headers.get(&name) {
            request = request.header(name, value.clone());
        }
    }
    request.send().await
}

/// The first byte, and the last if it is given, of a `Range: bytes=a-b` header.
fn requested_byte_range(headers: &HeaderMap) -> Option<(u64, Option<u64>)> {
    let value = headers.get(header::RANGE)?.to_str().ok()?.trim();
    let (start, end) = value.strip_prefix("bytes=")?.split_once('-')?;
    let start = start.trim().parse().ok()?;
    let end = match end.trim() {
        "" => None,
        end => Some(end.parse().ok()?),
    };
    Some((start, end))
}

const RANGE_RESUME_ATTEMPTS: u32 = 3;

/// A ranged upstream body that picks up where it stopped if the CDN drops it.
///
/// Each resume asks for only the bytes that are still missing, so the browser
/// sees one continuous body. If the resumes run out, the stream ends with an
/// error. Content-Length is set, so the browser treats the short body as a
/// failed request and does not append a partial segment.
fn resumable_range_stream(
    state: AppServerState,
    target: ProxyTarget,
    request_headers: HeaderMap,
    upstream: reqwest::Response,
) -> futures_util::stream::BoxStream<'static, std::result::Result<Bytes, std::io::Error>> {
    use futures_util::StreamExt;
    type UpstreamBody =
        futures_util::stream::BoxStream<'static, std::result::Result<Bytes, String>>;
    fn upstream_body(response: reqwest::Response) -> UpstreamBody {
        response
            .bytes_stream()
            .map(|chunk| chunk.map_err(|error| error.to_string()))
            .boxed()
    }
    fn failed_body(reason: String) -> UpstreamBody {
        futures_util::stream::once(async move { Err(reason) }).boxed()
    }
    struct Resume {
        state: AppServerState,
        target: ProxyTarget,
        request_headers: HeaderMap,
        range: Option<(u64, Option<u64>)>,
        sent: u64,
        attempts_left: u32,
        body: Option<UpstreamBody>,
    }
    let range = requested_byte_range(&request_headers);
    let resume = Resume {
        state,
        target,
        request_headers,
        range,
        sent: 0,
        attempts_left: RANGE_RESUME_ATTEMPTS,
        body: Some(upstream_body(upstream)),
    };
    futures_util::stream::unfold(resume, |mut resume| async move {
        loop {
            let body = resume.body.as_mut()?;
            let failure = match body.next().await {
                Some(Ok(chunk)) => {
                    resume.sent += chunk.len() as u64;
                    return Some((Ok(chunk), resume));
                }
                None => return None,
                Some(Err(error)) => error,
            };
            let Some((start, end)) = resume.range.filter(|_| resume.attempts_left > 0) else {
                resume.body = None;
                return Some((Err(std::io::Error::other(failure)), resume));
            };
            let attempt = RANGE_RESUME_ATTEMPTS - resume.attempts_left;
            resume.attempts_left -= 1;
            tokio::time::sleep(Duration::from_millis(150 * u64::from(attempt + 1))).await;
            let next = start + resume.sent;
            let range = match end {
                Some(end) => format!("bytes={next}-{end}"),
                None => format!("bytes={next}-"),
            };
            let mut headers = resume.request_headers.clone();
            let Ok(value) = HeaderValue::from_str(&range) else {
                resume.body = None;
                return Some((Err(std::io::Error::other(failure)), resume));
            };
            headers.insert(header::RANGE, value);
            // A resume that is not a matching 206 would splice the wrong bytes
            // into the segment, so it counts as another failure instead.
            resume.body = Some(
                match request_playback_upstream(&resume.state, &resume.target, &headers).await {
                    Ok(response) if response.status() == StatusCode::PARTIAL_CONTENT => {
                        upstream_body(response)
                    }
                    Ok(response) => failed_body(format!("resume answered {}", response.status())),
                    Err(error) => failed_body(error.to_string()),
                },
            );
        }
    })
    .boxed()
}

pub async fn playback_proxy(
    Extension(state): Extension<AppServerState>,
    Path(token): Path<String>,
    request_headers: HeaderMap,
) -> Response {
    let target = state.proxy_targets.read().await.get(&token).cloned();
    let Some(target) = target else {
        return media_proxy_error(StatusCode::NOT_FOUND, "Playback session was not found");
    };
    if target.expires_at <= std::time::Instant::now() {
        state.proxy_targets.write().await.remove(&token);
        return media_proxy_error(
            StatusCode::GONE,
            "Playback session expired; resolve it again",
        );
    }

    let range_requested = request_headers.contains_key(header::RANGE);
    // A refusal here is usually transient: the CDN gates a burst of ranged
    // requests and clears within a second or so. Passing the 403 straight
    // through instead makes it worse, because the player answers with its own
    // four attempts per segment across every segment in flight, and that
    // burst is what keeps the gate shut. Absorbing it here turns a retry
    // storm into a few paced requests.
    let mut upstream = match request_playback_upstream(&state, &target, &request_headers).await {
        Ok(response) => response,
        Err(_) => {
            return media_proxy_error(StatusCode::BAD_GATEWAY, "Media source is unavailable");
        }
    };
    for attempt in 0..3u32 {
        if !matches!(
            upstream.status(),
            StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS
        ) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(200 * u64::from(attempt + 1) * 2)).await;
        match request_playback_upstream(&state, &target, &request_headers).await {
            Ok(response) => upstream = response,
            Err(_) => break,
        }
    }
    let status = upstream.status();
    let upstream_headers = upstream.headers().clone();
    let content_type = upstream_headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    // Content type is the trustworthy signal; the path suffix is only a
    // fallback for upstreams that serve a playlist as a generic byte stream.
    let generic_type = content_type.is_empty() || content_type.contains("octet-stream");
    let is_hls = content_type.contains("mpegurl")
        || (generic_type && url_path_ends_with(&target.url, ".m3u8"));
    let is_dash = content_type.contains("dash+xml")
        || (generic_type && url_path_ends_with(&target.url, ".mpd"));
    let mut sniffed_type: Option<&'static str> = None;
    let is_vtt = content_type.contains("text/vtt")
        || target.url.contains("fmt=vtt")
        || target.url.contains("format=vtt");

    let body = if is_hls || is_dash {
        let bytes = match upstream.bytes().await {
            Ok(bytes) => bytes,
            Err(_) => {
                return media_proxy_error(StatusCode::BAD_GATEWAY, "Media manifest is unavailable");
            }
        };
        let text = String::from_utf8_lossy(&bytes);
        let rewritten = if is_hls {
            rewrite_hls_manifest(&state, &target.url, &text, &target.request_headers).await
        } else {
            rewrite_dash_manifest(&state, &target.url, &text, &target.request_headers).await
        };
        Body::from(rewritten)
    } else if is_vtt {
        let bytes = match upstream.bytes().await {
            Ok(bytes) => bytes,
            Err(_) => {
                return media_proxy_error(StatusCode::BAD_GATEWAY, "Caption track is unavailable");
            }
        };
        Body::from(center_vtt_cues(&String::from_utf8_lossy(&bytes)))
    } else if range_requested && status == StatusCode::PARTIAL_CONTENT {
        // Stream the range; never hold the headers until all of it has
        // arrived. Buffering a 4K segment (~8 MB) first delayed the headers
        // past the player's 8 s connection timeout whenever the server's link
        // was busy. The player then aborted with zero bytes, so ABR never got
        // a throughput sample and kept asking for the same 4K segment. Every
        // abandoned request also kept downloading on the server, which slowed
        // the next one. A body that fails midway is resumed from the byte it
        // reached, so MediaSource still gets one whole segment.
        use futures_util::StreamExt;
        let mut stream = resumable_range_stream(
            state.clone(),
            target.clone(),
            request_headers.clone(),
            upstream,
        );
        match stream.next().await {
            Some(Ok(head)) => {
                if generic_type {
                    sniffed_type = sniff_media_type(&head);
                }
                let head = futures_util::stream::once(async move { Ok::<_, std::io::Error>(head) });
                Body::from_stream(head.chain(stream))
            }
            Some(Err(_)) => {
                return media_proxy_error(StatusCode::BAD_GATEWAY, "Media range is unavailable");
            }
            None => Body::empty(),
        }
    } else {
        // Peek the first chunk so the container can be identified, then put it
        // back at the front of the stream. Buffering the whole body instead
        // would stall progressive playback of a multi-hundred-megabyte file.
        use futures_util::StreamExt;
        let mut stream = upstream.bytes_stream();
        match stream.next().await {
            Some(Ok(head)) => {
                if generic_type {
                    sniffed_type = sniff_media_type(&head);
                }
                let head = futures_util::stream::once(async move { Ok::<_, reqwest::Error>(head) });
                Body::from_stream(head.chain(stream))
            }
            Some(Err(_)) => {
                return media_proxy_error(StatusCode::BAD_GATEWAY, "Media stream is unavailable");
            }
            None => Body::empty(),
        }
    };
    let mut response = Response::new(body);
    *response.status_mut() = status;
    for name in [
        header::CONTENT_TYPE,
        header::CONTENT_RANGE,
        header::ACCEPT_RANGES,
        header::CACHE_CONTROL,
        header::ETAG,
        header::LAST_MODIFIED,
        header::CONTENT_DISPOSITION,
    ] {
        if let Some(value) = upstream_headers.get(&name) {
            response.headers_mut().insert(name, value.clone());
        }
    }
    if let Some(kind) = sniffed_type
        && let Ok(value) = HeaderValue::from_str(kind)
    {
        response.headers_mut().insert(header::CONTENT_TYPE, value);
    }
    if !is_hls
        && !is_dash
        && !is_vtt
        && let Some(value) = upstream_headers.get(header::CONTENT_LENGTH)
    {
        response
            .headers_mut()
            .insert(header::CONTENT_LENGTH, value.clone());
    }
    add_media_cors_headers(&mut response);
    response
}

fn add_media_cors_headers(response: &mut Response) {
    let headers = response.headers_mut();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    headers.insert(
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static(
            "Accept-Ranges, Content-Length, Content-Range, ETag, Last-Modified",
        ),
    );
    headers.insert(
        header::HeaderName::from_static("cross-origin-resource-policy"),
        HeaderValue::from_static("cross-origin"),
    );
    // Chromium's Private Network Access preflight uses this when a secure
    // WebView asset origin reads the loopback development server.
    headers.insert(
        header::HeaderName::from_static("access-control-allow-private-network"),
        HeaderValue::from_static("true"),
    );
}

fn media_proxy_error(status: StatusCode, message: &str) -> Response {
    let mut response = proxy_error(status, message);
    add_media_cors_headers(&mut response);
    response
}

pub async fn playback_proxy_options() -> Response {
    let mut response = Response::new(Body::empty());
    *response.status_mut() = StatusCode::NO_CONTENT;
    add_media_cors_headers(&mut response);
    response.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, OPTIONS"),
    );
    response.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("Range, Content-Type"),
    );
    response
}

pub async fn health(Extension(state): Extension<AppServerState>) -> StatusCode {
    match state.db.health().await {
        Ok(()) => StatusCode::OK,
        Err(error) => {
            eprintln!("SurrealDB health check failed: {error}");
            StatusCode::SERVICE_UNAVAILABLE
        }
    }
}

fn center_vtt_cues(input: &str) -> String {
    input
        .lines()
        .map(|line| {
            if !line.contains("-->") {
                return line.to_string();
            }
            let mut settings = line
                .split_whitespace()
                .filter(|part| !part.starts_with("align:") && !part.starts_with("position:"))
                .collect::<Vec<_>>()
                .join(" ");
            settings.push_str(" align:center position:50%");
            settings
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn best_thumbnail(thumbnails: &[rustypipe::model::Thumbnail]) -> Option<String> {
    thumbnails
        .iter()
        .max_by_key(|thumbnail| u64::from(thumbnail.width) * u64::from(thumbnail.height))
        .map(|thumbnail| thumbnail.url.clone())
}

fn rusty_channel_to_channel<T>(channel: &RustyChannel<T>, subscribed: bool) -> Channel {
    Channel {
        id: channel.id.clone(),
        name: channel.name.clone(),
        handle: channel.handle.clone().unwrap_or_else(|| {
            handle_for(
                &channel.name,
                &format!("https://youtube.com/channel/{}", channel.id),
            )
        }),
        avatar_url: best_thumbnail(&channel.avatar),
        subscriber_count: channel
            .subscriber_count
            .map(|count| compact_count(count as i64, ""))
            .unwrap_or_default(),
        subscribed,
        description: channel.description.clone(),
        banner_url: best_thumbnail(&channel.banner),
        subscription_content: SubscriptionContent::default(),
    }
}

fn rusty_channel_item_to_channel(item: &RustyChannelItem) -> Channel {
    Channel {
        id: item.id.clone(),
        name: item.name.clone(),
        handle: item.handle.clone().unwrap_or_else(|| {
            handle_for(
                &item.name,
                &format!("https://youtube.com/channel/{}", item.id),
            )
        }),
        avatar_url: best_thumbnail(&item.avatar),
        subscriber_count: item
            .subscriber_count
            .map(|count| compact_count(count as i64, ""))
            .unwrap_or_default(),
        subscribed: false,
        description: item.short_description.clone(),
        banner_url: None,
        subscription_content: SubscriptionContent::default(),
    }
}

fn rusty_video_item_to_video(
    item: &RustyVideoItem,
    fallback_channel: Option<(&str, &str)>,
    force_short: bool,
) -> Video {
    let channel_id = item
        .channel
        .as_ref()
        .map(|channel| channel.id.clone())
        .or_else(|| fallback_channel.map(|(id, _)| id.to_string()))
        .unwrap_or_else(|| fallback_channel_id("Unknown channel"));
    // Recommendation entries sometimes omit the uploader. An empty name reads
    // as "no channel to show" and the card hides the line, which is better than
    // captioning every such video "Unknown channel".
    let channel_name = item
        .channel
        .as_ref()
        .map(|channel| channel.name.clone())
        .or_else(|| fallback_channel.map(|(_, name)| name.to_string()))
        .unwrap_or_default();
    Video {
        id: item.id.clone(),
        title: item.name.clone(),
        channel_id,
        channel_name,
        thumbnail_url: best_thumbnail(&item.thumbnail)
            .unwrap_or_else(|| format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", item.id)),
        published_at: item
            .publish_date_txt
            .clone()
            .or_else(|| item.publish_date.map(|date| date.to_string()))
            .unwrap_or_else(|| {
                if item.is_live {
                    "Streaming now".into()
                } else {
                    "From YouTube".into()
                }
            }),
        duration_seconds: item.duration.unwrap_or_default() as u64,
        view_count: item
            .view_count
            .map(|count| {
                compact_count(
                    count as i64,
                    if item.is_live { " watching" } else { " views" },
                )
            })
            .unwrap_or_default(),
        progress_seconds: 0,
        watched: false,
        is_live: item.is_live,
        is_short: force_short || item.is_short,
        audio_only: false,
    }
}

fn encode_page<T: Serialize>(prefix: &str, paginator: &Paginator<T>) -> Option<String> {
    paginator.ctoken.as_ref()?;
    serde_json::to_vec(paginator)
        .ok()
        .map(|json| format!("{prefix}:{}", URL_SAFE_NO_PAD.encode(json)))
}

fn decode_page<T: DeserializeOwned>(token: &str, prefix: &str) -> Result<Paginator<T>> {
    let encoded = token
        .strip_prefix(&format!("{prefix}:"))
        .ok_or_else(|| anyhow!("unsupported continuation token"))?;
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .context("decode continuation token")?;
    serde_json::from_slice(&bytes).context("deserialize continuation token")
}

fn rusty_page(
    channel_id: &str,
    tab: ChannelMediaTab,
    paginator: &Paginator<RustyVideoItem>,
    fallback_name: &str,
) -> ChannelMediaPage {
    ChannelMediaPage {
        channel_id: channel_id.to_string(),
        tab,
        videos: paginator
            .items
            .iter()
            .map(|item| {
                rusty_video_item_to_video(
                    item,
                    Some((channel_id, fallback_name)),
                    tab == ChannelMediaTab::Shorts,
                )
            })
            .collect(),
        next_page: encode_page("local-channel", paginator),
        source: "Direct YouTube".into(),
    }
}

fn empty_channel_page(channel_id: &str, tab: ChannelMediaTab, source: &str) -> ChannelMediaPage {
    ChannelMediaPage {
        channel_id: channel_id.to_string(),
        tab,
        videos: Vec::new(),
        next_page: None,
        source: source.into(),
    }
}

fn embed_url(video_id: &str) -> String {
    format!(
        "https://www.youtube-nocookie.com/embed/{video_id}?enablejsapi=1&autoplay=1&playsinline=1"
    )
}

/// What the viewer is told when no ungated stream could be found.
fn no_stream_reason(ytdlp_failure: Option<&str>, ytdlp_format_count: usize) -> String {
    match ytdlp_failure {
        Some(failure) => format!("No playable stream: {failure}."),
        None if ytdlp_format_count == 0 => {
            "No playable stream: yt-dlp found no formats for this video.".into()
        }
        None => format!(
            "No playable stream: none of yt-dlp's {ytdlp_format_count} formats could be played."
        ),
    }
}

/// Ordering for playback sources, lowest first.
///
/// HLS leads deliberately. YouTube gates raw googlevideo (GVS) range requests
/// behind a PO token: the CDN serves roughly the first minute of content and
/// then answers 403 for further ranges. That budget is measured in bytes, so a
/// high-bitrate ladder burns through it in well under a minute, which surfaces
/// as quality collapsing and then playback stopping around 1:05-1:10. The
/// segments behind the HLS manifest are not gated the same way — a full
/// 19 Mbps variant was drained end to end (68 MiB) with no 403 on 2026-08-12,
/// while the adaptive ladder died mid-playback in the browser.
///
/// The adaptive track set stays as the next choice: it gives per-language audio
/// and finer quality control, and it plays fine for content whose bitrate never
/// exhausts the budget.
fn playback_source_priority(source: &PlaybackSource) -> u8 {
    match source.protocol {
        PlaybackProtocol::Hls => 0,
        PlaybackProtocol::Sabr if !source.tracks.is_empty() => 1,
        PlaybackProtocol::Dash if !source.tracks.is_empty() => 1,
        PlaybackProtocol::Progressive => 2,
        PlaybackProtocol::Dash => 3,
        PlaybackProtocol::Sabr => 4,
        PlaybackProtocol::EmbedFallback => 4,
    }
}

/// The ungated URL for this itag, or `None` when the track should be dropped.
///
/// The size check guards the byte ranges: they come from rustypipe and are only
/// valid against the exact same transcode, so a length mismatch means the two
/// clients are not describing the same file and the ranges would address the
/// wrong bytes.
///
/// The byte ranges a DASH `SegmentBase` needs.
///
/// yt-dlp reports everything about a format except these, so they are derived
/// from the container itself rather than taken from a second extractor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SegmentRanges {
    init_end: u64,
    index_start: u64,
    index_end: u64,
}

/// Walk ISO-BMFF boxes looking for `sidx`.
///
/// YouTube's DASH-ready MP4 is laid out `ftyp`, `moov`, `sidx`, then fragments,
/// so the initialisation segment is everything before `sidx` and the index is
/// the `sidx` box itself. Verified against the extractor's own numbers.
fn mp4_segment_ranges(head: &[u8]) -> Option<SegmentRanges> {
    let mut offset = 0usize;
    while offset + 8 <= head.len() {
        let declared = u32::from_be_bytes(head[offset..offset + 4].try_into().ok()?);
        let kind = &head[offset + 4..offset + 8];
        // A declared size of 1 means the real size is a 64-bit value following
        // the header. 0 means "to end of file", which cannot precede an index.
        let (size, header) = if declared == 1 {
            if offset + 16 > head.len() {
                return None;
            }
            (
                u64::from_be_bytes(head[offset + 8..offset + 16].try_into().ok()?),
                16u64,
            )
        } else {
            (u64::from(declared), 8u64)
        };
        if kind == b"sidx" {
            let start = u64::try_from(offset).ok()?;
            return Some(SegmentRanges {
                init_end: start.checked_sub(1)?,
                index_start: start,
                index_end: start.checked_add(size)?.checked_sub(1)?,
            });
        }
        if size < header {
            return None;
        }
        offset = offset.checked_add(usize::try_from(size).ok()?)?;
    }
    None
}

/// Read an EBML variable-length integer, returning its value and width.
///
/// The leading zero count of the first byte gives the width. The marker bit is
/// part of an element ID but not of a size, hence `keep_marker`.
fn ebml_vint(bytes: &[u8], offset: usize, keep_marker: bool) -> Option<(u64, usize)> {
    let first = *bytes.get(offset)?;
    if first == 0 {
        return None;
    }
    let width = first.leading_zeros() as usize + 1;
    if width > 8 || offset + width > bytes.len() {
        return None;
    }
    let mut value = if keep_marker {
        u64::from(first)
    } else if width == 8 {
        // Every value bit lives in the following bytes; the first is only the
        // marker, and shifting a u8 by its full width is an overflow.
        0
    } else {
        u64::from(first & (0xFFu8 >> width))
    };
    for index in 1..width {
        value = (value << 8) | u64::from(bytes[offset + index]);
    }
    Some((value, width))
}

const EBML_SEGMENT_ID: u64 = 0x1853_8067;
const EBML_CUES_ID: u64 = 0x1C53_BB6B;

/// Find the `Cues` element that indexes a WebM stream.
///
/// The initialisation segment is everything before `Cues` and the index is the
/// element itself, header included — the same shape as MP4's `sidx`, which is
/// why both share one return type.
fn webm_segment_ranges(head: &[u8]) -> Option<SegmentRanges> {
    fn scan(head: &[u8], start: usize, end: usize, descended: bool) -> Option<SegmentRanges> {
        let mut offset = start;
        while offset < end {
            let (id, id_width) = ebml_vint(head, offset, true)?;
            let (size, size_width) = ebml_vint(head, offset + id_width, false)?;
            let data = offset.checked_add(id_width)?.checked_add(size_width)?;
            if id == EBML_CUES_ID {
                let start = u64::try_from(offset).ok()?;
                let total = u64::try_from(id_width + size_width)
                    .ok()?
                    .checked_add(size)?;
                return Some(SegmentRanges {
                    init_end: start.checked_sub(1)?,
                    index_start: start,
                    index_end: start.checked_add(total)?.checked_sub(1)?,
                });
            }
            // Cues live inside the Segment master element, so that one is
            // stepped into rather than over.
            if id == EBML_SEGMENT_ID && !descended {
                let limit = usize::try_from(size)
                    .ok()
                    .and_then(|size| data.checked_add(size))
                    .unwrap_or(end)
                    .min(end);
                return scan(head, data, limit, true);
            }
            offset = data.checked_add(usize::try_from(size).ok()?)?;
        }
        None
    }
    scan(head, 0, head.len(), false)
}

/// Build the adaptive source entirely from yt-dlp.
///
/// This is the split: yt-dlp owns extraction, because its URLs are not subject
/// to the gate that kills the other client's after a few MiB, and the byte
/// ranges it does not report are derived from the containers directly. The
/// other extractor keeps metadata, search, channels and comments, where it is
/// faster and needs no subprocess.
async fn ytdlp_playback_source(
    client: &reqwest::Client,
    video_id: &str,
    formats: &[YtdlpFormat],
) -> Option<PlaybackSource> {
    let candidates = formats
        .iter()
        .filter(|format| !format.has_drm.unwrap_or(false))
        .filter(|format| format.is_dash())
        .filter_map(|format| {
            let url = format.url.as_deref().filter(|url| !url.is_empty())?;
            let kind = format.adaptive_kind()?;
            Some((format, format.itag()?, kind, url))
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return None;
    }

    // A few at a time, not all at once. Every format lives on the same
    // googlevideo host, which speaks HTTP/1.1 only, so each probe in flight is
    // its own connection - and opening two dozen at once gets connection
    // attempts dropped, each one waiting out SYN retries (1s, 3s, 7s) before
    // it gets through or times out. Measured 2026-09-24: all 25 at once took
    // 10s every time; four at a time over reused connections took 0.1-0.5s.
    //
    // Boxed so the stream's type does not carry the closure: unboxed, it is
    // not general enough to cross the `tokio::spawn` the resolve runs in.
    use futures_util::StreamExt;
    let probes = candidates
        .iter()
        .map(|(format, itag, _, url)| {
            // Without an exact size the file cannot be told apart from its
            // siblings, so it is probed every time rather than cached.
            let key = format.filesize.map(|size| SegmentRangeKey {
                video_id: video_id.to_string(),
                itag: *itag,
                language: format.language.clone(),
                size,
            });
            probe_segment_ranges(client, key, url).boxed()
        })
        .collect::<Vec<_>>();
    let ranges = futures_util::stream::iter(probes)
        .buffered(SEGMENT_PROBE_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;

    let mut tracks = candidates
        .into_iter()
        .zip(ranges)
        .filter_map(|((format, _, kind, url), ranges)| {
            let ranges = ranges?;
            let mime_type = format.mime_type(kind.clone())?;
            Some(PlaybackTrack {
                kind,
                url: url.to_string(),
                mime_type,
                bitrate: format.tbr.map(|rate| (rate * 1000.0).round() as u64),
                content_length: format.content_length(),
                duration_ms: format.duration_ms,
                width: format.width,
                height: format.height,
                fps: format.fps.map(|fps| fps.round() as u32),
                quality_label: format.format_note.clone(),
                language: format.language.clone(),
                label: None,
                is_default: false,
                init_range: Some(PlaybackByteRange {
                    start: 0,
                    end: ranges.init_end,
                }),
                index_range: Some(PlaybackByteRange {
                    start: ranges.index_start,
                    end: ranges.index_end,
                }),
                request_headers: Vec::new(),
            })
        })
        .collect::<Vec<_>>();
    if tracks.is_empty() {
        return None;
    }
    // Highest quality first, so the manifest lists them the way the ABR manager
    // expects to read them.
    tracks.sort_by_key(|track| {
        std::cmp::Reverse((
            track.height.unwrap_or_default(),
            track.bitrate.unwrap_or_default(),
        ))
    });
    if let Some(first) = tracks.first_mut() {
        first.is_default = true;
    }

    Some(PlaybackSource {
        protocol: PlaybackProtocol::Dash,
        url: String::new(),
        mime_type: None,
        tracks,
        expires_at: None,
        po_token: None,
        quality_label: None,
        request_headers: Vec::new(),
    })
}

/// The HLS master playlist yt-dlp found, as a source of its own.
///
/// Some extractions come back with no DASH formats at all, only the m3u8
/// ladder (the Safari web client's). Its itags (229-234, 269-270, 602-625)
/// match nothing the other extractor lists, so without this the resolve fell
/// through to that extractor's gated URLs: playback started, then stalled on a
/// 403 at about 1:09. The HLS segments are not gated - a 1080p variant of a
/// 40-minute video was fetched at its middle and end with no 403 on
/// 2026-09-24 - so this is the one to play when the DASH ladder is missing.
fn ytdlp_hls_source(formats: &[YtdlpFormat]) -> Option<PlaybackSource> {
    let manifest = formats
        .iter()
        .filter(|format| {
            format
                .protocol
                .as_deref()
                .is_some_and(|protocol| protocol.starts_with("m3u8"))
        })
        .find_map(|format| format.manifest_url.as_deref().filter(|url| !url.is_empty()))?;
    Some(PlaybackSource {
        protocol: PlaybackProtocol::Hls,
        url: manifest.to_string(),
        mime_type: Some("application/vnd.apple.mpegurl".into()),
        po_token: None,
        expires_at: None,
        quality_label: Some("Adaptive HLS".into()),
        request_headers: Vec::new(),
        tracks: Vec::new(),
    })
}

/// How many segment-range probes may be in flight at once. See
/// `ytdlp_playback_source`: past about four, connection attempts start being
/// dropped and the resolve waits whole seconds on SYN retries.
const SEGMENT_PROBE_CONCURRENCY: usize = 4;

/// The exact file whose container layout was probed.
///
/// The URLs expire but the container layout does not, so a probe is paid for
/// once per file rather than once per playback. The itag alone does not name
/// a file. Dubbed uploads ship one itag 251 (and one 140, 249, 250...) per
/// audio language, and the original's header is a different size from the
/// dubs'. Keyed by itag, every language got the ranges of whichever one was
/// probed first. Seen 2026-09-25 (XKSjCOKDtpk): English Opus's index range
/// pointed at cluster bytes, and Shaka failed the whole source with 3007
/// (WEBM_CUES_ELEMENT_MISSING). The transport then fell through to the gated
/// extractor source, which froze at about 1:01. Size alone is not enough
/// either: that video's Japanese and Turkish itag 140 are the same length.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct SegmentRangeKey {
    video_id: String,
    itag: u32,
    language: Option<String>,
    /// yt-dlp's exact `filesize`, never the estimate.
    size: u64,
}

static SEGMENT_RANGE_CACHE: std::sync::OnceLock<
    std::sync::Mutex<HashMap<SegmentRangeKey, SegmentRanges>>,
> = std::sync::OnceLock::new();

fn cached_segment_ranges(key: &SegmentRangeKey) -> Option<SegmentRanges> {
    SEGMENT_RANGE_CACHE
        .get_or_init(Default::default)
        .lock()
        .ok()?
        .get(key)
        .copied()
}

fn store_segment_ranges(key: SegmentRangeKey, ranges: SegmentRanges) {
    if let Ok(mut cache) = SEGMENT_RANGE_CACHE.get_or_init(Default::default).lock() {
        cache.insert(key, ranges);
    }
}

/// Read enough of a stream to find its index.
///
/// 32 KiB covers `ftyp` + `moov` + `sidx` with room to spare on every format
/// measured; a container whose index sits beyond that is skipped rather than
/// chased, since a second round trip per track would cost more than the format
/// is worth.
async fn probe_segment_ranges(
    client: &reqwest::Client,
    key: Option<SegmentRangeKey>,
    url: &str,
) -> Option<SegmentRanges> {
    if let Some(cached) = key.as_ref().and_then(cached_segment_ranges) {
        return Some(cached);
    }
    // One format that stalls is dropped from the manifest rather than holding
    // up the first frame for the client's whole timeout.
    let head = tokio::time::timeout(Duration::from_secs(4), async {
        let response = client
            .get(url)
            .header(reqwest::header::RANGE, "bytes=0-32767")
            .send()
            .await
            .ok()?;
        if !response.status().is_success() {
            return None;
        }
        response.bytes().await.ok()
    })
    .await
    .ok()??;
    let ranges = segment_ranges(&head)?;
    if let Some(key) = key {
        store_segment_ranges(key, ranges);
    }
    Some(ranges)
}

/// Pick a parser from the container's magic rather than its declared extension.
fn segment_ranges(head: &[u8]) -> Option<SegmentRanges> {
    if head.len() >= 8 && &head[4..8] == b"ftyp" {
        mp4_segment_ranges(head)
    } else if head.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        webm_segment_ranges(head)
    } else {
        None
    }
}

/// A track yt-dlp does not cover is dropped, never played from the
/// extractor's own URL. Those URLs are gated: they serve about a minute and
/// then answer 403. Playing them when yt-dlp was down made a stopped sidecar
/// look like a mysterious freeze at ~1:01 instead of an error that names the
/// cause (2026-09-25).
fn resolved_stream_url(
    ytdlp_urls: &HashMap<u32, (String, Option<u64>)>,
    itag: u32,
    size: Option<u64>,
) -> Option<String> {
    match ytdlp_urls.get(&itag) {
        Some((url, reported)) if reported.is_none() || size.is_none() || *reported == size => {
            Some(url.clone())
        }
        _ => None,
    }
}

fn rusty_playback_sources(
    player: &rustypipe::model::VideoPlayer,
    ytdlp_urls: &HashMap<u32, (String, Option<u64>)>,
) -> Vec<PlaybackSource> {
    let expires_at = Some(player.valid_until.to_string());
    let mut streams = player.video_streams.clone();
    streams.sort_by_key(|stream| {
        let is_mp4 = stream.mime.contains("mp4");
        (is_mp4, stream.height.min(1080), stream.bitrate)
    });
    streams.reverse();
    let mut sources = streams
        .into_iter()
        .filter(|stream| !stream.url.is_empty())
        .filter_map(|stream| {
            Some(PlaybackSource {
                protocol: PlaybackProtocol::Progressive,
                url: resolved_stream_url(ytdlp_urls, stream.itag, stream.size)?,
                mime_type: Some(stream.mime),
                po_token: None,
                expires_at: expires_at.clone(),
                quality_label: Some(stream.quality),
                request_headers: Vec::new(),
                tracks: Vec::new(),
            })
        })
        .collect::<Vec<_>>();

    let mut adaptive_tracks = player
        .video_only_streams
        .iter()
        .filter(|stream| !stream.url.is_empty() && stream.drm_track_type.is_none())
        .filter_map(|stream| {
            Some(PlaybackTrack {
                kind: PlaybackTrackKind::Video,
                url: resolved_stream_url(ytdlp_urls, stream.itag, stream.size)?,
                mime_type: stream.mime.clone(),
                bitrate: Some(u64::from(stream.bitrate)),
                content_length: stream.size,
                duration_ms: stream.duration_ms.map(u64::from),
                width: Some(stream.width),
                height: Some(stream.height),
                fps: Some(u32::from(stream.fps)),
                quality_label: Some(stream.quality.clone()),
                language: None,
                label: None,
                is_default: false,
                init_range: stream.init_range.as_ref().map(|range| PlaybackByteRange {
                    start: u64::from(range.start),
                    end: u64::from(range.end),
                }),
                index_range: stream.index_range.as_ref().map(|range| PlaybackByteRange {
                    start: u64::from(range.start),
                    end: u64::from(range.end),
                }),
                request_headers: Vec::new(),
            })
        })
        .filter(|track| track.init_range.is_some() && track.index_range.is_some())
        .collect::<Vec<_>>();
    adaptive_tracks.extend(
        player
            .audio_streams
            .iter()
            .filter(|stream| !stream.url.is_empty() && stream.drm_track_type.is_none())
            .filter_map(|stream| {
                let track_info = stream.track.as_ref();
                Some(PlaybackTrack {
                    kind: PlaybackTrackKind::Audio,
                    url: resolved_stream_url(ytdlp_urls, stream.itag, Some(stream.size))?,
                    mime_type: stream.mime.clone(),
                    bitrate: Some(u64::from(stream.bitrate)),
                    content_length: Some(stream.size),
                    duration_ms: stream.duration_ms.map(u64::from),
                    width: None,
                    height: None,
                    fps: None,
                    quality_label: Some(format!("{} kbps", stream.bitrate / 1_000)),
                    language: track_info.and_then(|track| track.lang.clone()),
                    label: track_info.map(|track| track.lang_name.clone()),
                    is_default: track_info.is_none_or(|track| track.is_default),
                    init_range: stream.init_range.as_ref().map(|range| PlaybackByteRange {
                        start: u64::from(range.start),
                        end: u64::from(range.end),
                    }),
                    index_range: stream.index_range.as_ref().map(|range| PlaybackByteRange {
                        start: u64::from(range.start),
                        end: u64::from(range.end),
                    }),
                    request_headers: Vec::new(),
                })
            })
            .filter(|track| track.init_range.is_some() && track.index_range.is_some()),
    );
    if adaptive_tracks
        .iter()
        .any(|track| track.kind == PlaybackTrackKind::Video)
        && adaptive_tracks
            .iter()
            .any(|track| track.kind == PlaybackTrackKind::Audio)
    {
        adaptive_tracks.sort_by_key(|track| {
            (
                track.kind == PlaybackTrackKind::Audio,
                track.height.unwrap_or_default(),
                track.bitrate.unwrap_or_default(),
            )
        });
        let quality_label = adaptive_tracks
            .iter()
            .filter(|track| track.kind == PlaybackTrackKind::Video)
            .filter_map(|track| track.height)
            .max()
            .map(|height| format!("Adaptive up to {height}p"));
        let url = adaptive_tracks
            .iter()
            .find(|track| track.kind == PlaybackTrackKind::Video)
            .map(|track| track.url.clone())
            .unwrap_or_default();
        sources.push(PlaybackSource {
            protocol: PlaybackProtocol::Dash,
            url,
            mime_type: Some("application/dash+xml".into()),
            po_token: None,
            expires_at: expires_at.clone(),
            quality_label,
            request_headers: Vec::new(),
            tracks: adaptive_tracks,
        });
    }
    if let Some(hls) = &player.hls_manifest_url {
        sources.push(PlaybackSource {
            protocol: PlaybackProtocol::Hls,
            url: hls.clone(),
            mime_type: Some("application/vnd.apple.mpegurl".into()),
            po_token: None,
            expires_at: expires_at.clone(),
            quality_label: Some("Adaptive HLS".into()),
            request_headers: Vec::new(),
            tracks: Vec::new(),
        });
    }
    if let Some(dash) = &player.dash_manifest_url {
        sources.push(PlaybackSource {
            protocol: PlaybackProtocol::Dash,
            url: dash.clone(),
            mime_type: Some("application/dash+xml".into()),
            po_token: None,
            expires_at,
            quality_label: Some("Adaptive DASH".into()),
            request_headers: Vec::new(),
            tracks: Vec::new(),
        });
    }
    sources.dedup_by(|left, right| left.url == right.url);
    sources
}

fn rusty_comment(comment: &RustyComment) -> VideoComment {
    VideoComment {
        id: comment.id.clone(),
        author: comment
            .author
            .as_ref()
            .map(|author| author.name.clone())
            .unwrap_or_else(|| "Deleted user".into()),
        author_avatar_url: comment
            .author
            .as_ref()
            .and_then(|author| best_thumbnail(&author.avatar)),
        author_channel_id: comment.author.as_ref().map(|author| author.id.clone()),
        text: comment.text.to_plaintext(),
        published_at: comment.publish_date_txt.clone(),
        like_count: comment.like_count.unwrap_or_default() as u64,
        reply_count: comment.reply_count as u64,
        pinned: comment.pinned,
        hearted: comment.hearted,
        creator: comment.by_owner,
    }
}

fn rusty_comments_page(paginator: &Paginator<RustyComment>) -> CommentsPage {
    CommentsPage {
        comments: paginator.items.iter().map(rusty_comment).collect(),
        next_page: encode_page("local-comments", paginator),
        disabled: paginator.items.is_empty() && paginator.ctoken.is_none(),
        remote_available: true,
    }
}

fn normalize_rusty_video_details(
    video_id: &str,
    details: RustyVideoDetails,
    player: Option<RustyVideoPlayer>,
    comments: CommentsPage,
    fallback_video: Option<Video>,
    fallback_channel: Option<Channel>,
) -> VideoDetails {
    let duration = player
        .as_ref()
        .map(|player| player.details.duration as u64)
        .or_else(|| fallback_video.as_ref().map(|video| video.duration_seconds))
        .unwrap_or_default();
    let thumbnail_url = player
        .as_ref()
        .and_then(|player| best_thumbnail(&player.details.thumbnail))
        .or_else(|| {
            fallback_video
                .as_ref()
                .map(|video| video.thumbnail_url.clone())
        })
        .unwrap_or_else(|| format!("https://i.ytimg.com/vi/{video_id}/hqdefault.jpg"));
    let channel = Channel {
        id: details.channel.id.clone(),
        name: details.channel.name.clone(),
        handle: fallback_channel
            .as_ref()
            .map(|channel| channel.handle.clone())
            .unwrap_or_else(|| {
                handle_for(
                    &details.channel.name,
                    &format!("https://youtube.com/channel/{}", details.channel.id),
                )
            }),
        avatar_url: best_thumbnail(&details.channel.avatar).or_else(|| {
            fallback_channel
                .as_ref()
                .and_then(|channel| channel.avatar_url.clone())
        }),
        subscriber_count: details
            .channel
            .subscriber_count
            .map(|count| compact_count(count as i64, ""))
            .or_else(|| {
                fallback_channel
                    .as_ref()
                    .map(|channel| channel.subscriber_count.clone())
            })
            .unwrap_or_default(),
        subscribed: fallback_channel
            .as_ref()
            .map(|channel| channel.subscribed)
            .unwrap_or(false),
        description: fallback_channel
            .as_ref()
            .map(|channel| channel.description.clone())
            .unwrap_or_default(),
        subscription_content: fallback_channel
            .as_ref()
            .map(|channel| channel.subscription_content)
            .unwrap_or_default(),
        banner_url: fallback_channel.and_then(|channel| channel.banner_url),
    };
    let video = Video {
        id: video_id.into(),
        title: details.name.clone(),
        channel_id: channel.id.clone(),
        channel_name: channel.name.clone(),
        thumbnail_url,
        published_at: details
            .publish_date_txt
            .clone()
            .or_else(|| details.publish_date.map(|date| date.to_string()))
            .unwrap_or_else(|| "From YouTube".into()),
        duration_seconds: duration,
        view_count: compact_count(details.view_count as i64, " views"),
        progress_seconds: fallback_video
            .as_ref()
            .map(|video| video.progress_seconds)
            .unwrap_or_default(),
        watched: fallback_video
            .as_ref()
            .map(|video| video.watched)
            .unwrap_or(false),
        is_live: details.is_live,
        is_short: fallback_video
            .as_ref()
            .map(|video| video.is_short)
            .unwrap_or_else(|| {
                player.as_ref().is_some_and(|player| {
                    player.details.duration <= 180
                        && player
                            .video_streams
                            .iter()
                            .any(|stream| stream.height > stream.width)
                })
            }),
        audio_only: false,
    };
    let captions = player
        .as_ref()
        .map(|player| {
            player
                .subtitles
                .iter()
                .map(|subtitle| CaptionTrack {
                    label: subtitle.lang_name.clone(),
                    language_code: subtitle.lang.clone(),
                    mime_type: "text/vtt".into(),
                    url: if subtitle.url.contains("fmt=") {
                        subtitle.url.clone()
                    } else if subtitle.url.contains('?') {
                        format!("{}&fmt=vtt", subtitle.url)
                    } else {
                        format!("{}?fmt=vtt", subtitle.url)
                    },
                    auto_generated: subtitle.auto_generated,
                })
                .collect()
        })
        .unwrap_or_default();
    let preview_frames = player.as_ref().and_then(|player| {
        player
            .preview_frames
            .iter()
            .filter(|frames| frames.page_count > 0 && frames.total_count > 0)
            .max_by_key(|frames| frames.frame_width)
            .map(|frames| VideoPreviewFrames {
                page_urls: frames.urls().collect(),
                frame_width: frames.frame_width,
                frame_height: frames.frame_height,
                total_count: frames.total_count,
                duration_per_frame_ms: frames.duration_per_frame,
                frames_per_page_x: frames.frames_per_page_x,
                frames_per_page_y: frames.frames_per_page_y,
            })
    });
    let description = details.description.to_plaintext();
    // YouTube only reports structured chapters when the uploader's markers were
    // recognised. Plenty of videos just list timestamps in the description, so
    // fall back to parsing those rather than showing no chapters at all.
    let mut chapters = details
        .chapters
        .iter()
        .map(|chapter| VideoChapter {
            title: chapter.name.clone(),
            start_seconds: chapter.position as u64,
        })
        .collect::<Vec<_>>();
    if chapters.is_empty() {
        chapters = extract_chapters(&description);
    }
    VideoDetails {
        video,
        channel: Some(channel),
        description,
        like_count: details.like_count.unwrap_or_default() as u64,
        dislike_count: 0,
        captions,
        chapters,
        preview_frames,
        related_videos: details
            .recommended
            .items
            .iter()
            .map(|item| rusty_video_item_to_video(item, None, item.is_short))
            .collect(),
        comments,
        remote_available: true,
    }
}

fn parse_timestamp(value: &str) -> Option<u64> {
    let parts = value
        .trim_matches(|character: char| !character.is_ascii_digit() && character != ':')
        .split(':')
        .map(str::parse::<u64>)
        .collect::<std::result::Result<Vec<_>, _>>()
        .ok()?;
    match parts.as_slice() {
        [minutes, seconds] if *seconds < 60 => Some(minutes * 60 + seconds),
        [hours, minutes, seconds] if *minutes < 60 && *seconds < 60 => {
            Some(hours * 3600 + minutes * 60 + seconds)
        }
        _ => None,
    }
}

fn extract_chapters(description: &str) -> Vec<VideoChapter> {
    let mut chapters = description
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            // Chapter lists are often bulleted or numbered, so the timestamp
            // is not guaranteed to be the very first token.
            let (timestamp, start_seconds) = line
                .split_whitespace()
                .take(3)
                .find_map(|token| parse_timestamp(token).map(|seconds| (token, seconds)))?;
            let timestamp_end = line.find(timestamp)? + timestamp.len();
            let title = line[timestamp_end..]
                .trim()
                .trim_start_matches(['-', '–', '—', ':', ')', ']'])
                .trim();
            (!title.is_empty()).then(|| VideoChapter {
                title: title.to_string(),
                start_seconds,
            })
        })
        .collect::<Vec<_>>();
    chapters.sort_by_key(|chapter| chapter.start_seconds);
    chapters.dedup_by_key(|chapter| chapter.start_seconds);
    if chapters.len() >= 2
        && chapters
            .first()
            .is_some_and(|chapter| chapter.start_seconds <= 1)
    {
        chapters
    } else {
        Vec::new()
    }
}

fn fallback_channel_id(name: &str) -> String {
    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    format!("discovered-{:x}", hasher.finish())
}

fn compact_count(value: i64, suffix: &str) -> String {
    let value = value.max(0) as f64;
    let count = if value >= 1_000_000_000.0 {
        format!("{:.1}B", value / 1_000_000_000.0)
    } else if value >= 1_000_000.0 {
        format!("{:.1}M", value / 1_000_000.0)
    } else if value >= 1_000.0 {
        format!("{:.1}K", value / 1_000.0)
    } else {
        format!("{}", value as i64)
    };
    format!("{}{}", count.trim_end_matches(".0"), suffix)
}

fn handle_for(name: &str, url: &str) -> String {
    if let Some(handle) = url
        .split('/')
        .find(|segment| segment.starts_with('@') && segment.len() > 1)
    {
        return handle.to_string();
    }
    format!(
        "@{}",
        name.to_lowercase()
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .collect::<String>()
    )
}

fn push_unique_video(videos: &mut Vec<Video>, video: Video) {
    if !videos.iter().any(|existing| existing.id == video.id) {
        videos.push(video);
    }
}

fn push_unique_channel(channels: &mut Vec<Channel>, channel: Channel) {
    if !channels.iter().any(|existing| existing.id == channel.id) {
        channels.push(channel);
    }
}

fn search_sort_key() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{millis:020}")
}

fn video_sort_key(video: &Video) -> String {
    canonical_sort_key(&video.published_at)
}

fn canonical_sort_key(value: &str) -> String {
    if value.len() == 20 && value.bytes().all(|byte| byte.is_ascii_digit()) {
        return value.to_string();
    }
    format!("{:020}", video_published_epoch(value).max(0))
}

fn video_published_epoch(value: &str) -> i64 {
    use time::{Date, OffsetDateTime, format_description::well_known::Rfc3339};

    let value = value.trim();
    if let Ok(timestamp) = OffsetDateTime::parse(value, &Rfc3339) {
        return timestamp.unix_timestamp();
    }
    if value.len() >= 10
        && let Ok(format) = time::format_description::parse_borrowed::<2>("[year]-[month]-[day]")
        && let Ok(date) = Date::parse(&value[..10], &format)
    {
        return date.midnight().assume_utc().unix_timestamp();
    }

    let lower = value.to_ascii_lowercase();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    if lower.contains("streaming now") || lower == "live" || lower.contains("just now") {
        return now;
    }
    if lower.contains("yesterday") {
        return now - 86_400;
    }

    let words = lower.split_whitespace().collect::<Vec<_>>();
    for (index, word) in words.iter().enumerate() {
        let count = word
            .trim_matches(|character: char| !character.is_ascii_digit())
            .parse::<i64>()
            .ok()
            .or_else(|| matches!(*word, "a" | "an").then_some(1));
        let Some(count) = count else { continue };
        let Some(unit) = words.get(index + 1) else {
            continue;
        };
        let seconds = if unit.starts_with("second") {
            1
        } else if unit.starts_with("minute") {
            60
        } else if unit.starts_with("hour") {
            3_600
        } else if unit.starts_with("day") {
            86_400
        } else if unit.starts_with("week") {
            7 * 86_400
        } else if unit.starts_with("month") {
            30 * 86_400
        } else if unit.starts_with("year") {
            365 * 86_400
        } else {
            continue;
        };
        return now.saturating_sub(count.saturating_mul(seconds));
    }
    0
}

#[derive(Default)]
struct FeedEntry {
    video_id: String,
    channel_id: String,
    title: String,
    published: String,
}

fn feed_entry_video(entry: &FeedEntry, channel: &Channel) -> Video {
    Video {
        id: entry.video_id.clone(),
        title: entry.title.clone(),
        channel_id: channel.id.clone(),
        channel_name: channel.name.clone(),
        thumbnail_url: format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", entry.video_id),
        published_at: entry.published.clone(),
        duration_seconds: 0,
        view_count: String::new(),
        progress_seconds: 0,
        watched: false,
        is_live: false,
        is_short: false,
        audio_only: false,
    }
}

fn parse_youtube_feed(xml: &str) -> Result<Vec<FeedEntry>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut entries = Vec::new();
    let mut entry: Option<FeedEntry> = None;
    let mut active_tag = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(tag)) => {
                let name = String::from_utf8_lossy(tag.local_name().as_ref()).into_owned();
                if name == "entry" {
                    entry = Some(FeedEntry::default());
                }
                active_tag = name;
            }
            Ok(Event::Text(text)) => {
                if let Some(entry) = entry.as_mut() {
                    let value = text.unescape()?.into_owned();
                    match active_tag.as_str() {
                        "videoId" => entry.video_id = value,
                        "channelId" => entry.channel_id = value,
                        "title" => entry.title = value,
                        "published" => entry.published = value,
                        _ => {}
                    }
                }
            }
            Ok(Event::End(tag)) => {
                let name = String::from_utf8_lossy(tag.local_name().as_ref()).into_owned();
                if name == "entry" {
                    if let Some(entry) = entry.take().filter(|entry| !entry.video_id.is_empty()) {
                        entries.push(entry);
                    }
                }
                active_tag.clear();
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(error.into()),
            _ => {}
        }
    }
    Ok(entries)
}

#[derive(Debug, Deserialize)]
pub struct WebSubVerification {
    #[serde(rename = "hub.mode")]
    mode: String,
    #[serde(rename = "hub.topic")]
    topic: String,
    #[serde(rename = "hub.challenge")]
    challenge: String,
    #[serde(rename = "hub.verify_token")]
    verify_token: Option<String>,
    #[serde(rename = "hub.lease_seconds")]
    lease_seconds: Option<u64>,
}

fn websub_channel_from_topic(topic: &str) -> Option<String> {
    let url = reqwest::Url::parse(topic).ok()?;
    if url.host_str()? != "www.youtube.com" || url.path() != "/xml/feeds/videos.xml" {
        return None;
    }
    url.query_pairs()
        .find_map(|(key, value)| (key == "channel_id").then(|| value.into_owned()))
        .filter(|id| id.starts_with("UC"))
}

pub async fn youtube_websub_verify(
    Extension(state): Extension<AppServerState>,
    Query(verification): Query<WebSubVerification>,
) -> Response {
    if verification.mode != "subscribe" && verification.mode != "unsubscribe" {
        return proxy_error(StatusCode::BAD_REQUEST, "Unsupported WebSub mode");
    }
    if verification
        .verify_token
        .as_deref()
        .is_some_and(|token| token != state.websub_secret)
    {
        return proxy_error(StatusCode::FORBIDDEN, "Invalid WebSub verification token");
    }
    let Some(channel_id) = websub_channel_from_topic(&verification.topic) else {
        return proxy_error(StatusCode::BAD_REQUEST, "Invalid YouTube WebSub topic");
    };
    let subscribed = state
        .channel_is_subscribed(&channel_id)
        .await
        .unwrap_or(false);
    if verification.mode == "subscribe" && !subscribed {
        return proxy_error(StatusCode::NOT_FOUND, "Channel is not subscribed");
    }
    let status = if verification.mode == "subscribe" {
        format!("active:{}s", verification.lease_seconds.unwrap_or_default())
    } else {
        "inactive".into()
    };
    let _ = state
        .db
        .query("UPDATE channel SET websub_status = $status WHERE channel_id = $channel_id")
        .bind(("channel_id", channel_id))
        .bind(("status", status))
        .await;
    let mut response = Response::new(Body::from(verification.challenge));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
}

pub async fn youtube_websub_notification(
    Extension(state): Extension<AppServerState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let signature = headers
        .get("x-hub-signature")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("sha1="))
        .and_then(|value| hex::decode(value).ok());
    let Some(signature) = signature else {
        return proxy_error(StatusCode::UNAUTHORIZED, "Missing WebSub signature");
    };
    let Ok(mut mac) = Hmac::<Sha1>::new_from_slice(state.websub_secret.as_bytes()) else {
        return proxy_error(StatusCode::INTERNAL_SERVER_ERROR, "Invalid WebSub secret");
    };
    mac.update(&body);
    if mac.verify_slice(&signature).is_err() {
        return proxy_error(StatusCode::UNAUTHORIZED, "Invalid WebSub signature");
    }
    let Ok(xml) = String::from_utf8(body.to_vec()) else {
        return proxy_error(StatusCode::BAD_REQUEST, "WebSub payload is not UTF-8");
    };
    tokio::spawn(async move {
        if let Err(error) = state.ingest_websub_notification(&xml).await {
            eprintln!("WebSub ingestion failed: {error:#}");
        }
    });
    let mut response = Response::new(Body::empty());
    *response.status_mut() = StatusCode::NO_CONTENT;
    response
}

pub fn spawn_subscription_poller(state: AppServerState) {
    tokio::spawn(async move {
        let renewal_state = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(3 * 24 * 60 * 60));
            loop {
                interval.tick().await;
                if let Err(error) = renewal_state.renew_websub_subscriptions().await {
                    eprintln!("WebSub renewal failed: {error:#}");
                }
            }
        });

        let mut interval = tokio::time::interval(Duration::from_secs(15 * 60));
        loop {
            interval.tick().await;
            // Also catches channels imported before the import path learned to
            // do this, a batch per tick so a large library does not arrive at
            // YouTube as one burst.
            state
                .backfill_channel_metadata(None, CHANNEL_METADATA_BATCH)
                .await;
            if let Err(error) = state.poll_subscriptions_once().await {
                eprintln!("subscription poll failed: {error:#}");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use crate::models::{Playlist, SubscriptionGroup, playlist_queue_entry, queued_playlist_id};

    use super::DbVideo;
    use super::{
        AppServerState, SegmentRangeKey, SegmentRanges, cached_segment_ranges, canonical_sort_key,
        center_vtt_cues, ebml_vint, extract_chapters, health, mp4_segment_ranges, no_stream_reason,
        parse_youtube_feed, playback_proxy, playback_proxy_options, reconciliation_limit,
        requested_byte_range, segment_ranges, sniff_media_type, store_segment_ranges,
        url_path_ends_with, video_published_epoch, webm_segment_ranges, websub_channel_from_topic,
        ytdlp_hls_source, ytdlp_po_provider_args, ytdlp_service_channel_shorts_url,
        ytdlp_service_video_url,
    };
    use crate::models::PlaybackProtocol;
    use crate::models::Video;
    use axum::extract::Path;
    use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
    use std::time::Duration;

    #[test]
    fn parses_youtube_atom_entries() {
        let xml = r#"<feed xmlns:yt="http://www.youtube.com/xml/schemas/2015"><entry><yt:videoId>abc123</yt:videoId><yt:channelId>UC-real-channel</yt:channelId><title>Fresh upload</title><published>2026-08-10T12:00:00Z</published></entry></feed>"#;
        let entries = parse_youtube_feed(xml).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].video_id, "abc123");
        assert_eq!(entries[0].channel_id, "UC-real-channel");
        assert_eq!(entries[0].title, "Fresh upload");
    }

    #[test]
    fn validates_youtube_websub_topics() {
        let valid = "https://www.youtube.com/xml/feeds/videos.xml?channel_id=UC-real-channel";
        assert_eq!(
            websub_channel_from_topic(valid).as_deref(),
            Some("UC-real-channel")
        );
        assert!(websub_channel_from_topic("https://example.com/?channel_id=UC-bad").is_none());
    }

    /// A fresh account to own whatever the test writes.
    ///
    /// Library calls now refuse an unauthenticated caller outright, so every
    /// test needs one - and a test getting its own means one test's
    /// subscriptions can no longer surface in another's snapshot.
    async fn test_owner(state: &AppServerState) -> String {
        state
            .accounts()
            .create_guest()
            .await
            .expect("mint a guest for the test")
            .id
    }

    #[tokio::test]
    async fn permits_android_webview_media_requests() {
        let response = playback_proxy_options().await;
        assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);
        assert_eq!(response.headers()["access-control-allow-origin"], "*");
        assert_eq!(
            response.headers()["access-control-allow-private-network"],
            "true"
        );
        assert!(
            response.headers()["access-control-allow-headers"]
                .to_str()
                .unwrap()
                .contains("Range")
        );
    }

    #[test]
    fn dubbed_audio_tracks_do_not_share_segment_ranges() {
        // One itag, two languages, two files with different headers.
        let original = SegmentRanges {
            init_end: 258,
            index_start: 259,
            index_end: 4037,
        };
        let dub = SegmentRanges {
            init_end: 300,
            index_start: 301,
            index_end: 4100,
        };
        let key = |language: &str, size: u64| SegmentRangeKey {
            video_id: "dubbed-cache-test".into(),
            itag: 140,
            language: Some(language.into()),
            size,
        };
        // XKSjCOKDtpk's Japanese and Turkish itag 140 are the same length.
        store_segment_ranges(key("ja", 34_831_565), original);
        store_segment_ranges(key("tr", 34_831_565), dub);
        assert_eq!(
            cached_segment_ranges(&key("ja", 34_831_565)),
            Some(original)
        );
        assert_eq!(cached_segment_ranges(&key("tr", 34_831_565)), Some(dub));
        assert_eq!(cached_segment_ranges(&key("en", 34_830_834)), None);
    }

    #[test]
    fn reads_the_requested_byte_range() {
        let mut headers = axum::http::HeaderMap::new();
        assert_eq!(requested_byte_range(&headers), None);
        headers.insert(header::RANGE, HeaderValue::from_static("bytes=100-199"));
        assert_eq!(requested_byte_range(&headers), Some((100, Some(199))));
        headers.insert(header::RANGE, HeaderValue::from_static("bytes=100-"));
        assert_eq!(requested_byte_range(&headers), Some((100, None)));
        headers.insert(header::RANGE, HeaderValue::from_static("bytes=0-1,5-9"));
        assert_eq!(requested_byte_range(&headers), None);
    }

    /// An upstream that drops the first ranged response halfway through, then
    /// serves whatever range it is asked for next. Returns the ranges it saw.
    async fn flaky_range_upstream(
        media: Vec<u8>,
    ) -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = seen.clone();
        tokio::spawn(async move {
            let mut served = 0;
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let mut request = Vec::new();
                let mut buffer = [0u8; 1024];
                while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                    let read = socket.read(&mut buffer).await.unwrap();
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..read]);
                }
                let request = String::from_utf8_lossy(&request).to_ascii_lowercase();
                let range = request
                    .lines()
                    .find_map(|line| line.strip_prefix("range: bytes="))
                    .unwrap()
                    .trim()
                    .to_string();
                log.lock().unwrap().push(range.clone());
                let (start, end) = range.split_once('-').unwrap();
                let (start, end): (usize, usize) = (start.parse().unwrap(), end.parse().unwrap());
                let body = &media[start..=end];
                let head = format!(
                    "HTTP/1.1 206 Partial Content\r\ncontent-type: application/octet-stream\r\ncontent-length: {}\r\ncontent-range: bytes {start}-{end}/{}\r\nconnection: close\r\n\r\n",
                    body.len(),
                    media.len()
                );
                socket.write_all(head.as_bytes()).await.unwrap();
                let cut = if served == 0 {
                    body.len() / 2
                } else {
                    body.len()
                };
                socket.write_all(&body[..cut]).await.unwrap();
                socket.flush().await.unwrap();
                served += 1;
            }
        });
        (format!("http://{address}/media"), seen)
    }

    #[tokio::test]
    async fn resumes_a_range_the_upstream_drops_midway() {
        let state = AppServerState::initialize().await.unwrap();
        let media = (0..200_000u32)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        let (upstream, seen) = flaky_range_upstream(media.clone()).await;
        let proxied = state
            .register_proxy_target(upstream, Vec::new(), Duration::from_secs(60))
            .await;
        let token = proxied.rsplit('/').next().unwrap().to_string();
        let mut headers = HeaderMap::new();
        headers.insert(header::RANGE, HeaderValue::from_static("bytes=1000-150999"));

        let response = playback_proxy(axum::Extension(state), Path(token), headers).await;
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(response.headers()[header::CONTENT_LENGTH], "150000");
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("the resumed body completes");
        assert_eq!(&body[..], &media[1000..=150_999]);
        let seen = seen.lock().unwrap().clone();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0], "1000-150999");
        let resumed_from: usize = seen[1].split_once('-').unwrap().0.parse().unwrap();
        assert!(resumed_from > 1000 && resumed_from <= 76_000, "{seen:?}");
    }

    #[tokio::test]
    async fn reports_container_health() {
        let state = AppServerState::initialize().await.unwrap();
        assert_eq!(
            health(axum::Extension(state)).await,
            axum::http::StatusCode::OK
        );
    }

    #[test]
    fn bounds_local_reconciliation_when_push_is_active() {
        assert_eq!(reconciliation_limit(700, true), 48);
        assert_eq!(reconciliation_limit(700, false), 700);
        assert_eq!(reconciliation_limit(12, true), 12);
    }

    #[test]
    fn configures_ytdlp_for_video_bound_po_tokens() {
        assert!(ytdlp_po_provider_args(None).is_empty());
        assert_eq!(
            ytdlp_po_provider_args(Some("http://127.0.0.1:4416")),
            vec![
                "--extractor-args",
                "youtubepot-bgutilhttp:base_url=http://127.0.0.1:4416",
                "--extractor-args",
                "youtube:player_client=mweb",
            ]
        );
    }

    #[test]
    fn builds_encoded_ytdlp_sidecar_video_urls() {
        let url = ytdlp_service_video_url("http://yt-dlp:8080/base", "aNXB-8Aqt88").unwrap();
        assert_eq!(
            url.as_str(),
            "http://yt-dlp:8080/base/v1/videos/aNXB-8Aqt88"
        );
        assert!(ytdlp_service_video_url("not a URL", "aNXB-8Aqt88").is_none());
    }

    #[test]
    fn falls_back_to_ytdlp_hls_master_playlist() {
        let dump = serde_json::json!({
            "duration": 60.0,
            "formats": [
                {"format_id": "sb0", "protocol": "mhtml", "url": "https://i.ytimg.com/sb"},
                {
                    "format_id": "232",
                    "protocol": "m3u8_native",
                    "url": "https://manifest.googlevideo.com/api/manifest/hls_playlist/itag/232/index.m3u8",
                    "manifest_url": "https://manifest.googlevideo.com/api/manifest/hls_variant/file/index.m3u8"
                }
            ]
        });
        let formats = super::formats_from_ytdlp_dump(serde_json::from_value(dump).unwrap());
        let source = ytdlp_hls_source(&formats).expect("an HLS source");
        assert_eq!(source.protocol, PlaybackProtocol::Hls);
        assert_eq!(
            source.url,
            "https://manifest.googlevideo.com/api/manifest/hls_variant/file/index.m3u8"
        );

        let dash_only = serde_json::json!({
            "formats": [{"format_id": "140", "protocol": "https", "url": "https://rr1.googlevideo.com/videoplayback"}]
        });
        let formats = super::formats_from_ytdlp_dump(serde_json::from_value(dash_only).unwrap());
        assert!(ytdlp_hls_source(&formats).is_none());
    }

    #[test]
    fn builds_ytdlp_sidecar_channel_shorts_urls() {
        let url =
            ytdlp_service_channel_shorts_url("http://yt-dlp:8080/base", "UCsXVk37bltHxD1rDPwtNM8Q")
                .unwrap();
        assert_eq!(
            url.as_str(),
            "http://yt-dlp:8080/base/v1/channels/UCsXVk37bltHxD1rDPwtNM8Q/shorts"
        );
    }

    #[test]
    fn orders_human_and_atom_dates_by_actual_recency() {
        assert!(video_published_epoch("2 hours ago") > video_published_epoch("1 year ago"));
        assert!(
            canonical_sort_key("2026-08-10T12:00:00Z") > canonical_sort_key("2025-08-10T12:00:00Z")
        );
        assert_eq!(canonical_sort_key("unknown"), "00000000000000000000");
    }

    #[test]
    fn extracts_ordered_description_chapters() {
        let chapters =
            extract_chapters("0:00 Opening\n1:05 First idea\n12:34 Second idea\n1:02:03 Closing");
        assert_eq!(chapters.len(), 4);
        assert_eq!(chapters[1].title, "First idea");
        assert_eq!(chapters[1].start_seconds, 65);
        assert_eq!(chapters[3].start_seconds, 3723);
        let bulleted = extract_chapters("• 0:00 Opening\n2. (1:05) First idea\n- 2:10 Closing");
        assert_eq!(bulleted.len(), 3);
        assert_eq!(bulleted[1].title, "First idea");
        assert!(extract_chapters("1:22 One stray timestamp").is_empty());
    }

    #[test]
    fn treats_only_real_playlists_as_playlists() {
        // A YouTube HLS segment carries the playlist path plus /sq/N/, so it
        // contains ".m3u8" without being one. Rewriting it as a playlist
        // replaces the media with proxy URLs and playback never buffers.
        assert!(url_path_ends_with(
            "https://manifest.googlevideo.com/api/manifest/hls_playlist/file/index.m3u8",
            ".m3u8"
        ));
        assert!(!url_path_ends_with(
            "https://rr3---sn-x.googlevideo.com/videoplayback/file/index.m3u8/sq/3/goap/clen/123",
            ".m3u8"
        ));
        assert!(url_path_ends_with(
            "https://example.test/stream/index.m3u8?itag=0",
            ".m3u8"
        ));
        assert!(!url_path_ends_with(
            "https://example.test/seg/index.m3u8/sq/0?itag=0",
            ".m3u8"
        ));
    }

    #[test]
    fn identifies_hls_segment_containers_youtube_serves_as_octet_stream() {
        // Packed AAC, which Shaka otherwise mistakes for fMP4 and never decodes.
        assert_eq!(sniff_media_type(b"ID3\x03\x00\x00"), Some("audio/aac"));
        assert_eq!(
            sniff_media_type(&[0xFF, 0xF1, 0x50, 0x80]),
            Some("audio/aac")
        );
        // MPEG-TS sync byte.
        assert_eq!(
            sniff_media_type(&[0x47, 0x40, 0x00, 0x30]),
            Some("video/mp2t")
        );
        assert_eq!(
            sniff_media_type(b"\x00\x00\x00\x18ftypmp42"),
            Some("video/mp4")
        );
        assert_eq!(
            sniff_media_type(&[0x1A, 0x45, 0xDF, 0xA3]),
            Some("video/webm")
        );
        assert_eq!(sniff_media_type(b"not media at all"), None);
        assert_eq!(sniff_media_type(&[]), None);
    }

    #[test]
    fn centers_caption_cues_without_discarding_other_settings() {
        let vtt = "WEBVTT\n\n00:00:01.000 --> 00:00:03.000 align:start position:0% line:90%\nHello";
        let centered = center_vtt_cues(vtt);
        assert!(
            centered.contains("00:00:01.000 --> 00:00:03.000 line:90% align:center position:50%")
        );
        assert!(!centered.contains("align:start"));
    }

    #[tokio::test]
    async fn serves_video_details_for_a_seeded_video() {
        let state = AppServerState::initialize().await.unwrap();
        let owner = test_owner(&state).await;
        let snapshot = state.library_snapshot(&owner).await.unwrap();
        let video_id = snapshot.videos[0].id.clone();
        let details = state.video_details(&owner, &video_id).await.unwrap();
        assert_eq!(details.video.id, video_id);
        // Seeded videos use real YouTube ids, so direct extraction may or may
        // not reach YouTube from the machine running the tests. Either way the
        // call has to resolve to that video with something to show next, which
        // is what the offline library fallback guarantees.
        assert!(!details.related_videos.is_empty());
    }

    /// The guard that keeps a listing from overwriting a known length lives in
    /// the statement, so the statement is what the test exercises. A rounded
    /// duration from a channel page replacing the exact one the player recorded
    /// would be a silent regression in every timeline and filter.
    #[tokio::test]
    async fn a_listing_fills_an_unknown_length_but_never_replaces_a_known_one() {
        let state = AppServerState::initialize().await.unwrap();
        let _owner = test_owner(&state).await;
        let video = Video {
            id: "duration-test".into(),
            title: "From the subscription feed".into(),
            channel_id: "UC-real-channel".into(),
            channel_name: "Channel".into(),
            thumbnail_url: String::new(),
            published_at: "2026-08-10T12:00:00Z".into(),
            duration_seconds: 0,
            view_count: String::new(),
            progress_seconds: 0,
            watched: false,
            is_live: false,
            is_short: false,
            audio_only: false,
        };
        state
            .upsert_feed_hint(&video, video.published_at.clone())
            .await
            .unwrap();

        let filled = state
            .apply_video_durations(&[("duration-test".to_string(), 754, "12K views".to_string())])
            .await
            .unwrap();
        assert_eq!(filled, 1, "an RSS row starts at zero and takes the length");

        let again = state
            .apply_video_durations(&[("duration-test".to_string(), 12, String::new())])
            .await
            .unwrap();
        assert_eq!(again, 0, "a known length is never overwritten");

        let stored: Vec<DbVideo> = state
            .db
            .query("SELECT * FROM video WHERE video_id = $video_id")
            .bind(("video_id", "duration-test"))
            .await
            .unwrap()
            .take(0)
            .unwrap();
        assert_eq!(stored[0].duration_seconds, 754);
        assert_eq!(stored[0].view_count, "12K views");
    }

    /// Channels are chosen by where the gaps actually are. Picking them any
    /// other way spends the refresh budget on channels that are already filled.
    #[tokio::test]
    async fn only_channels_with_unknown_lengths_are_asked_for_them() {
        let state = AppServerState::initialize().await.unwrap();
        let _owner = test_owner(&state).await;
        let known = Video {
            id: "known-length".into(),
            title: "Already measured".into(),
            channel_id: "UC-measured-channel".into(),
            channel_name: "Measured".into(),
            thumbnail_url: String::new(),
            published_at: "2026-08-11T12:00:00Z".into(),
            duration_seconds: 600,
            view_count: String::new(),
            progress_seconds: 0,
            watched: false,
            is_live: false,
            is_short: false,
            audio_only: false,
        };
        let unknown = Video {
            id: "unknown-length".into(),
            channel_id: "UC-unmeasured-channel".into(),
            channel_name: "Unmeasured".into(),
            ..known.clone()
        };
        state
            .upsert_feed_video(&known, known.published_at.clone())
            .await
            .unwrap();
        state
            .upsert_feed_hint(&unknown, unknown.published_at.clone())
            .await
            .unwrap();

        let channels = state.channels_missing_durations(6).await.unwrap();
        assert!(channels.contains(&"UC-unmeasured-channel".to_string()));
        assert!(!channels.contains(&"UC-measured-channel".to_string()));
    }

    #[tokio::test]
    async fn visible_duration_hydration_is_scoped_to_the_viewers_feed() {
        let state = AppServerState::initialize().await.unwrap();
        let owner = test_owner(&state).await;
        let mut snapshot = state.library_snapshot(&owner).await.unwrap();
        let video = snapshot
            .videos
            .iter()
            .find(|video| video.duration_seconds > 0)
            .cloned()
            .expect("seeded catalog has a measured video");
        snapshot
            .channels
            .iter_mut()
            .find(|channel| channel.id == video.channel_id)
            .expect("video channel is cached")
            .subscribed = true;
        snapshot.cache_revision += 1;
        state.sync_library(&owner, snapshot).await.unwrap();

        let resolved = state
            .hydrate_video_durations(
                &owner,
                vec![video.id.clone(), video.id.clone(), "not-cached".into()],
            )
            .await
            .unwrap();
        assert_eq!(resolved.len(), 1, "duplicates and unknown ids are ignored");
        assert_eq!(resolved[0].id, video.id);
        assert!(resolved[0].duration_seconds > 0);

        let other_owner = test_owner(&state).await;
        assert!(
            state
                .hydrate_video_durations(&other_owner, vec![video.id])
                .await
                .unwrap()
                .is_empty(),
            "a video outside the viewer's subscriptions is not hydrated"
        );
    }

    /// A playlist is where a reader collects videos from channels they do not
    /// follow, so a subscription-only allowlist left exactly those rows without
    /// a length - and a duration filter silently drops an unknown length.
    #[tokio::test]
    async fn a_saved_video_is_hydrated_without_following_its_channel() {
        let state = AppServerState::initialize().await.unwrap();
        let owner = test_owner(&state).await;
        let mut snapshot = state.library_snapshot(&owner).await.unwrap();
        let video = snapshot
            .videos
            .iter()
            .find(|video| video.duration_seconds > 0)
            .cloned()
            .expect("seeded catalog has a measured video");
        for channel in &mut snapshot.channels {
            channel.subscribed = false;
        }
        snapshot.playlists.push(Playlist {
            id: "saved-from-elsewhere".into(),
            name: "Saved".into(),
            video_ids: vec![video.id.clone()],
        });
        snapshot.cache_revision += 1;
        state.sync_library(&owner, snapshot).await.unwrap();

        let resolved = state
            .hydrate_video_durations(&owner, vec![video.id.clone()])
            .await
            .unwrap();
        assert_eq!(resolved.len(), 1, "a saved video is hydrated on its own");
        assert_eq!(resolved[0].id, video.id);

        assert!(
            state
                .hydrate_video_durations(&test_owner(&state).await, vec![video.id])
                .await
                .unwrap()
                .is_empty(),
            "another account's playlist grants nothing"
        );
    }

    /// The queue carries one marker entry standing in for a playlist run, so the
    /// server has to store an entry that is not a video id. Nothing here resolves
    /// queue entries against the catalog, and this is what keeps it that way.
    #[tokio::test]
    async fn a_queued_playlist_run_survives_a_round_trip() {
        let state = AppServerState::initialize().await.unwrap();
        let owner = test_owner(&state).await;
        let mut snapshot = state.library_snapshot(&owner).await.unwrap();
        let video_id = snapshot.videos[0].id.clone();
        snapshot.playlists.push(Playlist {
            id: "watch-later".into(),
            name: "Watch later".into(),
            video_ids: vec![video_id.clone()],
        });
        snapshot.queue = vec![playlist_queue_entry("watch-later"), video_id.clone()];
        snapshot.cache_revision += 1;

        let synced = state.sync_library(&owner, snapshot).await.unwrap();
        assert_eq!(
            synced.queue,
            vec![playlist_queue_entry("watch-later"), video_id],
            "the run marker keeps its place among the queued videos"
        );
        let reread = state.library_snapshot(&owner).await.unwrap();
        assert_eq!(
            queued_playlist_id(&reread.queue[0]),
            Some("watch-later"),
            "and reads back as the run it was"
        );
    }

    #[tokio::test]
    async fn websub_renewal_is_a_noop_without_a_public_callback() {
        let state = AppServerState::initialize().await.unwrap();
        let _owner = test_owner(&state).await;
        assert_eq!(state.renew_websub_subscriptions().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn playback_session_is_ungated_or_says_why_not() {
        let state = AppServerState::initialize().await.unwrap();
        let _owner = test_owner(&state).await;
        match state.playback_session("aqz-KE-bpKQ", true, true).await {
            Ok(session) => {
                assert!(session.fallback_url.contains("youtube-nocookie.com"));
                assert!(session.primary.url.contains("/api/v1/playback/proxy/"));
            }
            Err(error) => assert!(
                error.to_string().starts_with("No playable stream"),
                "{error:#}"
            ),
        }
    }

    #[test]
    fn names_why_no_stream_could_be_played() {
        assert_eq!(
            no_stream_reason(
                Some("the yt-dlp service at http://127.0.0.1:8090 is unreachable (refused)"),
                0
            ),
            "No playable stream: the yt-dlp service at http://127.0.0.1:8090 is unreachable (refused)."
        );
        assert!(no_stream_reason(None, 0).contains("found no formats"));
        assert!(no_stream_reason(None, 12).contains("none of yt-dlp's 12 formats"));
    }

    #[tokio::test]
    async fn syncs_revisioned_library_state() {
        let state = AppServerState::initialize().await.unwrap();
        let owner = test_owner(&state).await;
        let mut snapshot = state.library_snapshot(&owner).await.unwrap();
        // A new account owns no playlists or groups. The seeded catalog is
        // shared, but everything personal starts empty and is created by the
        // client on first run - which is the whole point of the scoping.
        assert!(snapshot.playlists.is_empty());
        assert!(snapshot.subscription_groups.is_empty());
        assert!(!snapshot.videos.is_empty(), "the catalog is still shared");

        let video_id = snapshot.videos[0].id.clone();
        snapshot.subscription_groups.push(SubscriptionGroup {
            id: "favorites".into(),
            name: "Favorites".into(),
            channel_ids: vec![snapshot.channels[0].id.clone()],
        });
        snapshot.playlists.push(Playlist {
            id: "watch-later".into(),
            name: "Watch later".into(),
            video_ids: vec![video_id.clone()],
        });
        snapshot.queue.insert(0, video_id.clone());
        snapshot.videos[0].watched = true;
        snapshot.videos[0].progress_seconds = snapshot.videos[0].duration_seconds;
        snapshot.cache_revision += 1;

        let synced = state.sync_library(&owner, snapshot).await.unwrap();
        assert_eq!(synced.queue.first(), Some(&video_id));
        assert!(synced.videos.iter().any(|video| video.watched));
        assert!(
            synced
                .subscription_groups
                .iter()
                .any(|group| group.name == "Favorites")
        );

        let mut stale = synced.clone();
        stale.cache_revision -= 1;
        stale.queue.clear();
        let server_wins = state.sync_library(&owner, stale).await.unwrap();
        assert_eq!(server_wins.queue.first(), Some(&video_id));
    }

    /// The property the whole per-user rewrite exists for.
    ///
    /// Two accounts on one instance share the catalog and nothing else. Before
    /// this, subscriptions and watch progress were columns on the shared
    /// `channel` and `video` rows, so a second account inherited the first
    /// account's entire library the moment it connected.
    #[tokio::test]
    async fn one_account_cannot_see_another_account_library() {
        let state = AppServerState::initialize().await.unwrap();
        let first = test_owner(&state).await;
        let second = test_owner(&state).await;

        let mut mine = state.library_snapshot(&first).await.unwrap();
        let video_id = mine.videos[0].id.clone();
        let channel_id = mine.channels[0].id.clone();
        mine.videos[0].watched = true;
        mine.videos[0].progress_seconds = 42;
        mine.channels[0].subscribed = true;
        mine.playlists.push(Playlist {
            id: "watch-later".into(),
            name: "Mine".into(),
            video_ids: vec![video_id.clone()],
        });
        mine.cache_revision += 1;
        let mine = state.sync_library(&first, mine).await.unwrap();

        assert!(mine.videos.iter().any(|video| video.watched));
        assert!(mine.channels.iter().any(|channel| channel.subscribed));
        assert_eq!(mine.playlists.len(), 1);

        let theirs = state.library_snapshot(&second).await.unwrap();
        assert!(
            theirs.videos.iter().all(|video| !video.watched),
            "watch progress leaked between accounts"
        );
        assert!(
            theirs
                .videos
                .iter()
                .all(|video| video.progress_seconds == 0),
            "playback position leaked between accounts"
        );
        assert!(
            theirs.channels.iter().all(|channel| !channel.subscribed),
            "subscriptions leaked between accounts"
        );
        assert!(
            theirs.playlists.is_empty(),
            "playlists leaked between accounts"
        );
        assert!(
            theirs.queue.is_empty() && theirs.history.is_empty(),
            "queue or history leaked between accounts"
        );
        // The catalog is deliberately shared: the same upload must not be
        // fetched and stored twice because two people follow the channel.
        assert!(theirs.videos.iter().any(|video| video.id == video_id));
        assert!(
            theirs
                .channels
                .iter()
                .any(|channel| channel.id == channel_id)
        );

        // The second account can hold the same client-chosen playlist id.
        let mut ours = theirs;
        ours.playlists.push(Playlist {
            id: "watch-later".into(),
            name: "Theirs".into(),
            video_ids: Vec::new(),
        });
        ours.cache_revision += 1;
        let ours = state.sync_library(&second, ours).await.unwrap();
        assert_eq!(ours.playlists.len(), 1);
        assert_eq!(ours.playlists[0].name, "Theirs");

        // ...without disturbing the first account's playlist of the same id.
        let mine_again = state.library_snapshot(&first).await.unwrap();
        assert_eq!(mine_again.playlists.len(), 1);
        assert_eq!(mine_again.playlists[0].name, "Mine");
    }

    /// The viewerless path the poller and WebSub ingest take.
    ///
    /// Regression test: these call `library_snapshot("")`, and the per-account
    /// queries inside it bind `type::record($owner)`, which is a *runtime*
    /// error on an empty string rather than a compile error. Every other test
    /// passes a real account, so nothing here was exercised until a live poll
    /// failed with "Found  for the Record ID but this is not a valid table
    /// name".
    #[tokio::test]
    async fn the_catalog_only_snapshot_needs_no_account() {
        let state = AppServerState::initialize().await.unwrap();

        let catalog = state.library_snapshot("").await.unwrap();
        assert!(
            !catalog.videos.is_empty(),
            "the shared catalog is still read"
        );
        assert!(!catalog.channels.is_empty());
        // Nothing personal belongs to nobody.
        assert!(catalog.playlists.is_empty());
        assert!(catalog.subscription_groups.is_empty());
        assert!(catalog.queue.is_empty());
        assert!(catalog.history.is_empty());
        assert_eq!(catalog.cache_revision, 0);
        assert!(catalog.videos.iter().all(|video| !video.watched));
        assert!(catalog.channels.iter().all(|channel| !channel.subscribed));

        // And the whole poll, which is what actually broke.
        state.poll_subscriptions_once().await.unwrap();
    }

    /// Unsubscribing must not silence a channel other accounts still follow.
    #[tokio::test]
    async fn a_channel_stays_polled_while_anyone_still_follows_it() {
        let state = AppServerState::initialize().await.unwrap();
        let first = test_owner(&state).await;
        let second = test_owner(&state).await;

        let subscribe = |owner: String, subscribed: bool| {
            let state = state.clone();
            async move {
                let mut snapshot = state.library_snapshot(&owner).await.unwrap();
                snapshot.channels[0].subscribed = subscribed;
                snapshot.cache_revision += 1;
                state.sync_library(&owner, snapshot).await.unwrap()
            }
        };

        let first_view = subscribe(first.clone(), true).await;
        let channel_id = first_view.channels[0].id.clone();
        subscribe(second.clone(), true).await;
        assert!(state.channel_is_subscribed(&channel_id).await.unwrap());

        // One leaves; the instance still has a reason to poll.
        subscribe(first.clone(), false).await;
        assert!(
            state.channel_is_subscribed(&channel_id).await.unwrap(),
            "the last remaining follower lost their feed"
        );

        // Both gone, and only then does the instance stop caring.
        subscribe(second.clone(), false).await;
        assert!(!state.channel_is_subscribed(&channel_id).await.unwrap());
    }

    /// Video details are cached for every account at once, so the follow
    /// state inside them is whoever fetched them first. Each account has to
    /// see its own.
    #[tokio::test]
    async fn video_details_carry_each_accounts_own_follow_state() {
        let state = AppServerState::initialize().await.unwrap();
        let follower = test_owner(&state).await;
        let other = test_owner(&state).await;

        let mut snapshot = state.library_snapshot(&follower).await.unwrap();
        snapshot.channels[0].subscribed = true;
        snapshot.cache_revision += 1;
        let synced = state.sync_library(&follower, snapshot).await.unwrap();
        let channel = synced.channels[0].clone();
        let video = synced
            .videos
            .iter()
            .find(|video| video.channel_id == channel.id)
            .cloned()
            .unwrap_or_else(|| synced.videos[0].clone());
        use crate::models::{Channel, CommentsPage, VideoDetails};
        let cached_with = |subscribed: bool| VideoDetails {
            video: video.clone(),
            channel: Some(Channel {
                subscribed,
                ..channel.clone()
            }),
            description: String::new(),
            like_count: 0,
            dislike_count: 0,
            captions: Vec::new(),
            chapters: Vec::new(),
            preview_frames: None,
            related_videos: Vec::new(),
            comments: CommentsPage::default(),
            remote_available: true,
        };
        let followed = |details: VideoDetails| details.channel.unwrap().subscribed;

        // Cached by the follower, read by someone who does not follow.
        let seen = state
            .with_owner_follow_state(&other, cached_with(true))
            .await
            .unwrap();
        assert!(!followed(seen), "another account's follow leaked through");

        // Cached before following - or by the other account - read by the
        // follower: the reported bug.
        let seen = state
            .with_owner_follow_state(&follower, cached_with(false))
            .await
            .unwrap();
        assert!(followed(seen), "a follower was shown Subscribe");
    }

    /// Search is cached by query for every account, and a channel in it that
    /// the client has not seen is added to the library flag and all - so a
    /// wrong "subscribed" here is a follow nobody asked for.
    #[tokio::test]
    async fn search_results_carry_each_accounts_own_follow_state() {
        use crate::models::{Channel, SearchResults};
        let state = AppServerState::initialize().await.unwrap();
        let follower = test_owner(&state).await;
        let other = test_owner(&state).await;

        let mut snapshot = state.library_snapshot(&follower).await.unwrap();
        snapshot.channels[0].subscribed = true;
        snapshot.cache_revision += 1;
        let synced = state.sync_library(&follower, snapshot).await.unwrap();
        let followed_channel = synced.channels[0].clone();
        let unfollowed_channel = synced.channels[1].clone();
        // As the shared cache might hold them: flags from the instance, or
        // from whichever account searched first.
        let cached = SearchResults {
            query: "anything".into(),
            videos: Vec::new(),
            channels: vec![
                Channel {
                    subscribed: false,
                    ..followed_channel.clone()
                },
                Channel {
                    subscribed: true,
                    ..unfollowed_channel.clone()
                },
            ],
            suggestion: None,
            remote_available: true,
            next_page: None,
        };
        let flags = |results: SearchResults| {
            results
                .channels
                .into_iter()
                .map(|channel| channel.subscribed)
                .collect::<Vec<_>>()
        };

        let mine = state
            .with_owner_follows_in_search(&follower, cached.clone())
            .await
            .unwrap();
        assert_eq!(flags(mine), vec![true, false]);

        let theirs = state
            .with_owner_follows_in_search(&other, cached)
            .await
            .unwrap();
        assert_eq!(flags(theirs), vec![false, false]);
    }

    #[tokio::test]
    #[ignore = "live YouTube connectivity check"]
    async fn live_youtube_search_subscribe_channel_and_feed_round_trip() {
        let state = AppServerState::initialize().await.unwrap();
        let owner = test_owner(&state).await;
        let results = state
            .search_catalog(&owner, "Alan Chikin Chow", "all")
            .await
            .unwrap();
        assert!(results.remote_available);
        assert!(!results.videos.is_empty());
        let channel = results
            .channels
            .iter()
            .find(|channel| {
                channel.id.starts_with("UC")
                    && channel
                        .name
                        .to_ascii_lowercase()
                        .contains("alan chikin chow")
            })
            .or_else(|| {
                results
                    .channels
                    .iter()
                    .find(|channel| channel.id.starts_with("UC"))
            })
            .cloned()
            .expect("real YouTube channel search result");

        let details = state.channel_details(&owner, &channel.id).await.unwrap();
        eprintln!(
            "live channel: {} ({}) videos={} shorts={} live={}",
            channel.name,
            channel.id,
            details.videos.videos.len(),
            details.shorts.videos.len(),
            details.live.videos.len()
        );
        assert!(details.remote_available);
        assert!(
            !details.videos.videos.is_empty()
                || !details.shorts.videos.is_empty()
                || !details.live.videos.is_empty()
        );
        assert!(!details.shorts.videos.is_empty(), "Shorts tab is populated");
        let playable_video = details
            .videos
            .videos
            .first()
            .or_else(|| details.shorts.videos.first())
            .expect("channel has a playable upload");
        let video_details = state
            .video_details(&owner, &playable_video.id)
            .await
            .unwrap();
        assert!(video_details.remote_available);
        let playback = state
            .playback_session(&playable_video.id, true, true)
            .await
            .unwrap();
        eprintln!("live playback protocol: {:?}", playback.primary.protocol);
        assert!(playback.primary.url.contains("/api/v1/playback/proxy/"));

        let mut snapshot = state.library_snapshot(&owner).await.unwrap();
        snapshot
            .channels
            .iter_mut()
            .find(|cached| cached.id == channel.id)
            .expect("searched channel is cached")
            .subscribed = true;
        snapshot.cache_revision += 1;
        let synced = state.sync_library(&owner, snapshot).await.unwrap();
        assert!(
            synced
                .channels
                .iter()
                .any(|cached| cached.id == channel.id && cached.subscribed)
        );

        let refresh = state.refresh_feed(&owner).await.unwrap();
        assert_eq!(refresh.failed_channels, 0);
        assert!(
            refresh
                .library
                .videos
                .iter()
                .any(|video| video.channel_id == channel.id)
        );
        let channel_feed = refresh
            .library
            .videos
            .iter()
            .filter(|video| video.channel_id == channel.id)
            .collect::<Vec<_>>();
        assert!(
            channel_feed
                .windows(2)
                .all(|pair| { pair[0].published_epoch() >= pair[1].published_epoch() })
        );
    }
    /// Header layout measured from a real YouTube itag 160 stream:
    /// ftyp@0+28, moov@28+710, sidx@738+3812.
    fn mp4_head(boxes: &[(&[u8; 4], u32)]) -> Vec<u8> {
        let mut out = Vec::new();
        for (kind, size) in boxes {
            out.extend_from_slice(&size.to_be_bytes());
            out.extend_from_slice(*kind);
            out.resize(out.len() + (*size as usize - 8), 0);
        }
        out
    }

    #[test]
    fn mp4_index_is_the_sidx_box_and_init_is_everything_before_it() {
        let head = mp4_head(&[(b"ftyp", 28), (b"moov", 710), (b"sidx", 3812)]);
        let ranges = mp4_segment_ranges(&head).expect("sidx found");
        assert_eq!(ranges.init_end, 737);
        assert_eq!(ranges.index_start, 738);
        assert_eq!(ranges.index_end, 4549);
    }

    #[test]
    fn mp4_walk_stops_rather_than_looping_on_a_zero_sized_box() {
        let mut head = Vec::new();
        head.extend_from_slice(&0u32.to_be_bytes());
        head.extend_from_slice(b"free");
        head.resize(64, 0);
        assert_eq!(mp4_segment_ranges(&head), None);
    }

    #[test]
    fn ebml_vint_widths_and_marker_handling() {
        // 0x81 is a one-byte value of 1 once the marker is stripped.
        assert_eq!(ebml_vint(&[0x81], 0, false), Some((1, 1)));
        assert_eq!(ebml_vint(&[0x81], 0, true), Some((0x81, 1)));
        // 0x1A45DFA3 is the four-byte EBML header id, kept whole.
        assert_eq!(
            ebml_vint(&[0x1A, 0x45, 0xDF, 0xA3], 0, true),
            Some((0x1A45_DFA3, 4))
        );
        assert_eq!(ebml_vint(&[0x00], 0, false), None);
    }

    /// Emit one EBML element: id bytes, then a size vint of the given width,
    /// then a zeroed payload.
    fn ebml_element(out: &mut Vec<u8>, id: &[u8], payload: usize, size_width: usize) {
        out.extend_from_slice(id);
        let marker = 1u64 << (7 * size_width);
        let size = marker | payload as u64;
        let bytes = size.to_be_bytes();
        out.extend_from_slice(&bytes[8 - size_width..]);
        out.resize(out.len() + payload, 0);
    }

    /// Layout measured from a real YouTube itag 278 stream: an EBML header of
    /// 36 bytes, a Segment whose children run 44..219, then Cues at 219
    /// spanning 5345 bytes.
    #[test]
    fn webm_index_is_the_cues_element_inside_the_segment() {
        let mut head = Vec::new();
        ebml_element(&mut head, &[0x1A, 0x45, 0xDF, 0xA3], 31, 1);
        assert_eq!(head.len(), 36);
        // Segment is a master element: header only, with a four-byte declared
        // size covering the children that follow rather than a payload of its
        // own.
        head.extend_from_slice(&[0x18, 0x53, 0x80, 0x67]);
        let segment_payload = 5564u64 - 44;
        head.extend_from_slice(&((1u64 << 28) | segment_payload).to_be_bytes()[4..]);
        assert_eq!(head.len(), 44);
        for payload in [42usize, 54, 64] {
            ebml_element(&mut head, &[0x11, 0x4D, 0x9B, 0x74], payload, 1);
        }
        assert_eq!(head.len(), 219);
        ebml_element(&mut head, &[0x1C, 0x53, 0xBB, 0x6B], 5339, 2);
        assert_eq!(head.len(), 5564);

        let ranges = webm_segment_ranges(&head).expect("cues found");
        assert_eq!(ranges.init_end, 218);
        assert_eq!(ranges.index_start, 219);
        assert_eq!(ranges.index_end, 5563);
    }

    #[test]
    fn ebml_vint_of_full_width_does_not_overflow_its_mask() {
        // An eight-byte size carries no value bits in its first byte.
        assert_eq!(
            ebml_vint(&[0x01, 0, 0, 0, 0, 0, 0, 7], 0, false),
            Some((7, 8))
        );
    }

    #[test]
    fn container_is_chosen_by_magic_not_by_extension() {
        let mp4 = mp4_head(&[(b"ftyp", 28), (b"sidx", 100)]);
        assert!(segment_ranges(&mp4).is_some());
        assert_eq!(segment_ranges(b"not a media container at all"), None);
    }
}

// ---------------------------------------------------------------------------
// SponsorBlock
// ---------------------------------------------------------------------------

/// How long a fetched segment list is trusted.
///
/// Segments change when someone submits or votes, which is slow enough that an
/// hour is generous and short enough that a correction lands the same evening.
const SPONSOR_CACHE_TTL: Duration = Duration::from_secs(60 * 60);

/// The public instance. Self-hosters who run their own can point at it.
const SPONSOR_API_DEFAULT: &str = "https://sponsor.ajay.app";

#[derive(Debug, Deserialize)]
struct SponsorApiVideo {
    #[serde(rename = "videoID")]
    video_id: String,
    segments: Vec<SponsorApiSegment>,
}

#[derive(Debug, Deserialize)]
struct SponsorApiSegment {
    #[serde(rename = "UUID")]
    uuid: String,
    category: String,
    /// `[start, end]` in seconds. A point segment has both the same.
    segment: Vec<f64>,
    #[serde(rename = "actionType", default)]
    action_type: String,
    #[serde(default)]
    locked: i64,
    #[serde(default)]
    votes: i64,
}

impl AppServerState {
    /// Segments for one video, from SponsorBlock.
    ///
    /// Fetched by the server rather than the client on purpose. The client would
    /// otherwise talk to sponsor.ajay.app directly from every viewer's address,
    /// which is exactly the correlation a self-hosted instance exists to avoid -
    /// and it lets one fetch serve every account here.
    ///
    /// The lookup is by hash prefix, which is SponsorBlock's own privacy
    /// mechanism: the first four hex characters of the video id's SHA-256 go in
    /// the path, the API answers with every video sharing that prefix, and the
    /// caller picks its own out. The server therefore never tells SponsorBlock
    /// which video was actually watched.
    pub async fn sponsor_segments(
        &self,
        video_id: &str,
        categories: &[SponsorCategory],
    ) -> Result<Vec<SponsorSegment>> {
        if categories.is_empty() {
            return Ok(Vec::new());
        }
        let cache_key = (
            video_id.to_string(),
            sponsor_cache_discriminator(categories),
        );
        if let Some(cached) = self.sponsor_cache.read().await.get(&cache_key)
            && cached.0.elapsed() < SPONSOR_CACHE_TTL
        {
            return Ok(cached.1.clone());
        }

        let digest = hex::encode(Sha256::digest(video_id.as_bytes()));
        let prefix = &digest[..4];
        let wanted = categories
            .iter()
            .map(|category| format!("\"{}\"", category.api_name()))
            .collect::<Vec<_>>()
            .join(",");
        let url = format!(
            "{}/api/skipSegments/{prefix}?categories=[{wanted}]&actionTypes=[\"skip\",\"poi\"]",
            self.sponsor_api_url()
        );

        let response = self.http.get(&url).send().await?;
        // 404 is the ordinary answer for "nobody has submitted anything with
        // this prefix", not a failure worth surfacing.
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            self.remember_sponsor_segments(cache_key, Vec::new()).await;
            return Ok(Vec::new());
        }
        let response = response.error_for_status()?;
        let videos: Vec<SponsorApiVideo> = response.json().await?;

        let segments = videos
            .into_iter()
            .find(|video| video.video_id == video_id)
            .map(|video| convert_sponsor_segments(video.segments))
            .unwrap_or_default();
        self.remember_sponsor_segments(cache_key, segments.clone())
            .await;
        Ok(segments)
    }

    fn sponsor_api_url(&self) -> &str {
        self.sponsor_api_url
            .as_deref()
            .unwrap_or(SPONSOR_API_DEFAULT)
    }

    async fn remember_sponsor_segments(
        &self,
        key: (String, String),
        segments: Vec<SponsorSegment>,
    ) {
        self.sponsor_cache
            .write()
            .await
            .insert(key, (std::time::Instant::now(), segments));
    }
}

/// Two requests for the same video with different categories are different
/// answers, so the cache has to tell them apart.
fn sponsor_cache_discriminator(categories: &[SponsorCategory]) -> String {
    let mut names = categories
        .iter()
        .map(|category| category.api_name())
        .collect::<Vec<_>>();
    names.sort_unstable();
    names.join(",")
}

/// Turn the API's shape into Tawny's, dropping what cannot be used.
///
/// Overlaps are real: several people submit the same sponsor with slightly
/// different bounds. Keeping all of them would draw a muddle on the timeline
/// and skip the same break twice, so within a category the best-supported
/// submission wins any overlap - locked first, then votes, which is the order
/// SponsorBlock itself trusts them in.
fn convert_sponsor_segments(raw: Vec<SponsorApiSegment>) -> Vec<SponsorSegment> {
    let mut segments = raw
        .into_iter()
        .filter_map(|item| {
            let category = SponsorCategory::from_api_name(&item.category)?;
            let start = *item.segment.first()?;
            let end = *item.segment.get(1)?;
            if !start.is_finite() || !end.is_finite() || end < start {
                return None;
            }
            // A poi is a point; anything else with no length would be an
            // invisible segment that still triggers a skip.
            let is_point = item.action_type == "poi" || category.is_point();
            if !is_point && end <= start {
                return None;
            }
            Some(SponsorSegment {
                uuid: item.uuid,
                category,
                start_seconds: start.max(0.0),
                end_seconds: end.max(0.0),
                locked: item.locked > 0,
                votes: item.votes,
            })
        })
        .collect::<Vec<_>>();

    // Best first, so the retain below keeps the winner of each overlap.
    segments.sort_by(|a, b| {
        b.locked
            .cmp(&a.locked)
            .then(b.votes.cmp(&a.votes))
            .then(a.start_seconds.total_cmp(&b.start_seconds))
    });

    let mut kept: Vec<SponsorSegment> = Vec::new();
    for segment in segments {
        let overlaps = kept.iter().any(|existing| {
            existing.category == segment.category
                && segment.start_seconds < existing.end_seconds
                && existing.start_seconds < segment.end_seconds
        });
        if !overlaps {
            kept.push(segment);
        }
    }

    kept.sort_by(|a, b| a.start_seconds.total_cmp(&b.start_seconds));
    kept
}

#[cfg(test)]
mod sponsor_server_tests {
    use super::{SponsorApiSegment, convert_sponsor_segments, sponsor_cache_discriminator};
    use crate::models::SponsorCategory;

    fn raw(category: &str, start: f64, end: f64, locked: i64, votes: i64) -> SponsorApiSegment {
        SponsorApiSegment {
            uuid: format!("{category}-{start}-{end}"),
            category: category.into(),
            segment: vec![start, end],
            action_type: "skip".into(),
            locked,
            votes,
        }
    }

    /// The exact shape sponsor.ajay.app returns, checked against a live
    /// response: `locked` and `votes` are integers, not booleans.
    #[test]
    fn the_api_payload_deserializes() {
        let payload = r#"{
            "category": "intro",
            "actionType": "skip",
            "segment": [0, 14.827],
            "UUID": "5d683910",
            "videoDuration": 3238.835,
            "locked": 0,
            "votes": 3,
            "description": ""
        }"#;
        let parsed: SponsorApiSegment = serde_json::from_str(payload).expect("parse");
        assert_eq!(parsed.uuid, "5d683910");
        assert_eq!(parsed.segment, vec![0.0, 14.827]);
        assert_eq!(parsed.votes, 3);
    }

    #[test]
    fn segments_come_back_in_playback_order() {
        let converted = convert_sponsor_segments(vec![
            raw("outro", 500.0, 540.0, 0, 0),
            raw("sponsor", 30.0, 60.0, 0, 0),
            raw("intro", 0.0, 10.0, 0, 0),
        ]);
        let starts = converted
            .iter()
            .map(|segment| segment.start_seconds)
            .collect::<Vec<_>>();
        assert_eq!(starts, vec![0.0, 30.0, 500.0]);
    }

    #[test]
    fn a_locked_submission_wins_an_overlap() {
        // Two people submitted the same sponsor break. Drawing both would
        // muddle the timeline and skip it twice.
        let converted = convert_sponsor_segments(vec![
            raw("sponsor", 30.0, 60.0, 0, 40),
            raw("sponsor", 28.0, 62.0, 1, 2),
        ]);
        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0].start_seconds, 28.0);
        assert!(converted[0].locked);
    }

    #[test]
    fn votes_break_a_tie_when_neither_is_locked() {
        let converted = convert_sponsor_segments(vec![
            raw("sponsor", 30.0, 60.0, 0, 2),
            raw("sponsor", 31.0, 59.0, 0, 99),
        ]);
        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0].start_seconds, 31.0);
    }

    #[test]
    fn overlapping_segments_of_different_categories_both_survive() {
        // An intro that overlaps a sponsor is two separate things to show.
        let converted = convert_sponsor_segments(vec![
            raw("sponsor", 0.0, 30.0, 0, 0),
            raw("intro", 0.0, 10.0, 0, 0),
        ]);
        assert_eq!(converted.len(), 2);
    }

    #[test]
    fn unusable_segments_are_dropped_rather_than_skipped_over() {
        let converted = convert_sponsor_segments(vec![
            // Unknown category from a newer API.
            raw("chapter", 0.0, 10.0, 0, 0),
            // Backwards.
            raw("sponsor", 60.0, 30.0, 0, 0),
            // Zero length, which would be an invisible segment that still fires.
            raw("outro", 45.0, 45.0, 0, 0),
        ]);
        assert!(converted.is_empty());
    }

    #[test]
    fn a_point_segment_survives_having_no_length() {
        let mut point = raw("poi_highlight", 90.0, 90.0, 0, 0);
        point.action_type = "poi".into();
        let converted = convert_sponsor_segments(vec![point]);
        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0].category, SponsorCategory::Highlight);
    }

    #[test]
    fn the_cache_key_ignores_the_order_categories_were_asked_in() {
        let one = sponsor_cache_discriminator(&[SponsorCategory::Sponsor, SponsorCategory::Intro]);
        let other =
            sponsor_cache_discriminator(&[SponsorCategory::Intro, SponsorCategory::Sponsor]);
        assert_eq!(one, other);
        // ...but not which categories they were.
        assert_ne!(
            one,
            sponsor_cache_discriminator(&[SponsorCategory::Sponsor])
        );
    }
}
