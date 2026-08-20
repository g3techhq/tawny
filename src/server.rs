use crate::models::{
    CaptionTrack, Channel, ChannelDetails, ChannelMediaPage, ChannelMediaTab, CommentsPage,
    FeedRefreshResult, HistoryEntry, LibrarySnapshot, PlaybackByteRange, PlaybackProtocol,
    PlaybackSession, PlaybackSource, PlaybackTrack, PlaybackTrackKind, Playlist, SearchResults,
    SubscriptionGroup, Video, VideoChapter, VideoComment, VideoDetails, VideoPreviewFrames,
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
    db: Arc<Surreal<Any>>,
    http: reqwest::Client,
    media_http: reqwest::Client,
    youtube: Arc<RustyPipe>,
    /// Path to a `yt-dlp` binary used solely to obtain ungated stream URLs.
    ytdlp_bin: Option<std::path::PathBuf>,
    public_url: String,
    websub_callback_url: Option<String>,
    websub_secret: String,
    proxy_targets: Arc<tokio::sync::RwLock<HashMap<String, ProxyTarget>>>,
    proxy_counter: Arc<AtomicU64>,
    sync_lock: Arc<tokio::sync::Mutex<()>>,
    search_lock: Arc<tokio::sync::Mutex<()>>,
    search_cache: Arc<tokio::sync::RwLock<HashMap<(String, String), CachedSearch>>>,
}

#[derive(Clone, Debug)]
struct ProxyTarget {
    url: String,
    request_headers: Vec<crate::models::PlaybackRequestHeader>,
    expires_at: std::time::Instant,
}

#[derive(Clone)]
struct CachedSearch {
    result: SearchResults,
    cached_at: std::time::Instant,
}

const FEED_RECONCILE_BATCH_SIZE: usize = 48;
const FEED_RSS_CONCURRENCY: usize = 32;
const WEBSUB_RENEW_CONCURRENCY: usize = 12;
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
    description: String,
    banner_url: Option<String>,
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
}

#[derive(Debug, Deserialize, SurrealValue)]
struct DbPlaylist {
    playlist_id: String,
    name: String,
    video_ids: Vec<String>,
    pinned: bool,
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
        }
    }
}

impl From<DbPlaylist> for Playlist {
    fn from(value: DbPlaylist) -> Self {
        Self {
            id: value.playlist_id,
            name: value.name,
            video_ids: value.video_ids,
            pinned: value.pinned,
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
        let endpoint = std::env::var("SURREALDB_HOST").unwrap_or_else(|_| {
            if cfg!(test) {
                "mem://".into()
            } else {
                format!(
                    "rocksdb://{}",
                    data_dir
                        .join("surrealdb")
                        .to_string_lossy()
                        .replace('\\', "/")
                )
            }
        });
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
        db.query(include_str!("../database/schema.surql"))
            .await
            .context("apply Tawny schema")?
            .check()
            .context("validate Tawny schema")?;

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
        let websub_secret = std::env::var("TAWNY_WEBSUB_SECRET").unwrap_or_else(|_| {
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
            youtube: Arc::new(youtube),
            ytdlp_bin: locate_ytdlp(),
            public_url,
            websub_callback_url,
            websub_secret,
            proxy_targets: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            proxy_counter: Arc::new(AtomicU64::new(1)),
            sync_lock: Arc::new(tokio::sync::Mutex::new(())),
            search_lock: Arc::new(tokio::sync::Mutex::new(())),
            search_cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        };
        state.seed_demo_if_empty().await?;
        Ok(state)
    }

    pub async fn library_snapshot(&self) -> Result<LibrarySnapshot> {
        let channels: Vec<DbChannel> = self
            .db
            .query(
                "SELECT channel_id, name, handle, avatar_url, subscriber_count, subscribed, description, banner_url FROM channel ORDER BY name",
            )
            .await?
            .take(0)?;
        let videos: Vec<DbVideo> = self
            .db
            .query(
                "SELECT video_id, title, channel_id, channel_name, thumbnail_url, published_at, published_sort, duration_seconds, view_count, is_live, is_short, progress_seconds, watched FROM video ORDER BY published_sort DESC",
            )
            .await?
            .take(0)?;
        let playlists: Vec<DbPlaylist> = self
            .db
            .query(
                "SELECT playlist_id, name, video_ids, pinned FROM playlist ORDER BY pinned DESC, name",
            )
            .await?
            .take(0)?;
        let subscription_groups: Vec<DbSubscriptionGroup> = self
            .db
            .query("SELECT group_id, name, channel_ids FROM subscription_group ORDER BY name")
            .await?
            .take(0)?;
        let states: Vec<DbLibraryState> = self
            .db
            .query("SELECT cache_revision, queue_json, history_json FROM library_state:primary")
            .await?
            .take(0)?;
        let state = states.into_iter().next();
        let queue = state
            .as_ref()
            .and_then(|state| serde_json::from_str(&state.queue_json).ok())
            .unwrap_or_default();
        let history: Vec<HistoryEntry> = state
            .as_ref()
            .and_then(|state| serde_json::from_str(&state.history_json).ok())
            .unwrap_or_default();

        let mut videos = videos.into_iter().map(Into::into).collect::<Vec<Video>>();
        videos.sort_by_key(|video| std::cmp::Reverse(video_published_epoch(&video.published_at)));

        Ok(LibrarySnapshot {
            channels: channels.into_iter().map(Into::into).collect(),
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

    pub async fn sync_library(&self, snapshot: LibrarySnapshot) -> Result<LibrarySnapshot> {
        let guard = self.sync_lock.lock().await;
        let server_revision = self.library_revision().await?;
        if snapshot.cache_revision <= server_revision {
            return self.library_snapshot().await;
        }

        let prior_subscriptions: Vec<DbSubscriptionState> = self
            .db
            .query("SELECT channel_id, subscribed FROM channel")
            .await?
            .take(0)?;
        let prior_subscriptions = prior_subscriptions
            .into_iter()
            .map(|channel| (channel.channel_id, channel.subscribed))
            .collect::<HashMap<_, _>>();

        for channel in &snapshot.channels {
            self.upsert_channel(channel).await?;
        }
        for video in &snapshot.videos {
            self.upsert_video(video, video_sort_key(video)).await?;
        }
        for playlist in &snapshot.playlists {
            self.upsert_playlist(playlist).await?;
        }
        let playlist_ids = snapshot
            .playlists
            .iter()
            .map(|playlist| playlist.id.clone())
            .collect::<Vec<_>>();
        self.db
            .query("DELETE playlist WHERE playlist_id NOT IN $playlist_ids")
            .bind(("playlist_ids", playlist_ids))
            .await?
            .check()?;
        for group in &snapshot.subscription_groups {
            self.upsert_subscription_group(group).await?;
        }
        let group_ids = snapshot
            .subscription_groups
            .iter()
            .map(|group| group.id.clone())
            .collect::<Vec<_>>();
        self.db
            .query("DELETE subscription_group WHERE group_id NOT IN $group_ids")
            .bind(("group_ids", group_ids))
            .await?
            .check()?;

        self.db
            .query(
                r#"UPSERT library_state:primary SET
                    cache_revision = $cache_revision,
                    queue_json = $queue_json,
                    history_json = $history_json,
                    updated_at = time::now()"#,
            )
            .bind(("cache_revision", snapshot.cache_revision as i64))
            .bind(("queue_json", serde_json::to_string(&snapshot.queue)?))
            .bind(("history_json", serde_json::to_string(&snapshot.history)?))
            .await?
            .check()?;

        drop(guard);

        let changed_channels = snapshot
            .channels
            .iter()
            .filter(|channel| {
                prior_subscriptions
                    .get(&channel.id)
                    .copied()
                    .unwrap_or(false)
                    != channel.subscribed
            })
            .filter(|channel| channel.id.starts_with("UC") && !channel.id.starts_with("UC-tawny"))
            .cloned()
            .collect::<Vec<_>>();
        if !changed_channels.is_empty() {
            let state = self.clone();
            tokio::spawn(async move {
                state.process_subscription_changes(changed_channels).await;
            });
        }

        self.library_snapshot().await
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
                    let limit = reconciliation_limit(
                        newly_subscribed.len(),
                        self.websub_callback_url.is_some(),
                    );
                    let _ = self
                        .reconcile_channels_rss(newly_subscribed.into_iter().take(limit).collect())
                        .await;
                }
            };
        let _ = tokio::join!(websub_job, feed_job);
    }

    async fn library_revision(&self) -> Result<u64> {
        let states: Vec<DbLibraryState> = self
            .db
            .query("SELECT cache_revision, queue_json, history_json FROM library_state:primary")
            .await?
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

        for track in [video, audio].into_iter().flatten() {
            let Some(length) = track.content_length.filter(|length| *length > 0) else {
                continue;
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
            if !matches!(response, Ok(Ok(response)) if response.status().is_success()) {
                return false;
            }
        }
        true
    }

    /// A channel's Shorts, listed by yt-dlp.
    ///
    /// rustypipe 0.11.4 cannot read the Shorts tab at all: verified 2026-08-13
    /// against two channels that publish Shorts, both returned zero items on
    /// the first page *and* zero after following the continuation token. yt-dlp
    /// parses the same tab correctly, and it is already a dependency for
    /// playback URLs, so it fills the gap rather than leaving the tab empty.
    async fn ytdlp_channel_shorts(&self, channel_id: &str, channel_name: &str) -> Vec<Video> {
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
            })
            .collect()
    }

    /// Ungated stream URLs from yt-dlp, keyed by itag.
    ///
    /// rustypipe can only reach the iOS client, whose googlevideo URLs YouTube
    /// gates: they serve roughly 6 MiB and then answer 403, which reaches the
    /// viewer as playback dying about a minute in. yt-dlp uses the `android_vr`
    /// client, whose URLs carry no such gate — verified on 2026-08-13 by
    /// draining the identical itag (same `filesize`) to completion through
    /// yt-dlp while the iOS URL 403'd at 6 MiB on the same unthrottled
    /// connection.
    ///
    /// Only the URL is taken. The byte ranges, codecs, and sizes still come
    /// from rustypipe, which is sound because an itag identifies one specific
    /// transcode: the two clients hand out different URLs for the same file.
    /// `filesize` is compared per itag so a mismatch is skipped rather than
    /// producing a manifest whose ranges point into the wrong bytes.
    /// Every format yt-dlp can see, which is the whole basis of playback now.
    async fn ytdlp_formats(&self, video_id: &str) -> Vec<YtdlpFormat> {
        let Some(binary) = self.ytdlp_bin.as_ref() else {
            return Vec::new();
        };
        let output = tokio::process::Command::new(binary)
            .args([
                "-J",
                "--no-warnings",
                "--no-playlist",
                "--socket-timeout",
                "15",
                &format!("https://www.youtube.com/watch?v={video_id}"),
            ])
            .stdin(std::process::Stdio::null())
            .output();
        let output = match tokio::time::timeout(Duration::from_secs(30), output).await {
            Ok(Ok(output)) if output.status.success() => output,
            Ok(Ok(output)) => {
                eprintln!(
                    "yt-dlp failed for {video_id}: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                );
                return Vec::new();
            }
            Ok(Err(error)) => {
                eprintln!("could not run yt-dlp: {error}");
                return Vec::new();
            }
            Err(_) => {
                eprintln!("yt-dlp timed out for {video_id}");
                return Vec::new();
            }
        };
        let dump = match serde_json::from_slice::<YtdlpDump>(&output.stdout) {
            Ok(dump) => dump,
            Err(error) => {
                eprintln!("could not parse yt-dlp output for {video_id}: {error}");
                return Vec::new();
            }
        };
        let duration_ms = dump
            .duration
            .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
            .map(|seconds| (seconds * 1000.0).round() as u64);
        let mut formats = dump.formats;
        for format in &mut formats {
            format.duration_ms = duration_ms;
        }
        formats
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

    pub async fn playback_session(
        &self,
        video_id: &str,
        _prefer_sabr: bool,
    ) -> Result<PlaybackSession> {
        let fallback_url = embed_url(video_id);
        let mut provider_sources: Vec<PlaybackSource> = Vec::new();
        // Run both extractors together: rustypipe supplies the stream layout
        // (byte ranges, codecs, languages) and yt-dlp supplies URLs that are
        // not subject to the iOS client's 403 gate.
        let query = self.youtube.query();
        let (player, ytdlp_formats) = tokio::join!(
            youtube_call(
                query.player_from_clients(video_id, PLAYER_CLIENTS),
                "extract YouTube player"
            ),
            self.ytdlp_formats(video_id),
        );

        // yt-dlp owns the adaptive source. The extractor is consulted only for
        // what yt-dlp does not produce — the HLS manifest a live stream needs —
        // and as a fallback when yt-dlp itself came back empty.
        if let Some(source) = ytdlp_playback_source(&self.http, video_id, &ytdlp_formats).await {
            eprintln!(
                "playback for {video_id}: {} yt-dlp tracks, ranges derived locally",
                source.tracks.len()
            );
            provider_sources.push(source);
        }

        let ytdlp_urls = Self::ytdlp_url_map(&ytdlp_formats);
        let youtube_sources = match player {
            Ok(player) => {
                let sources = rusty_playback_sources(&player, &ytdlp_urls);
                // Tracks yt-dlp does not cover are dropped, which is right when
                // it covers most of them and wrong when it covers none: a
                // yt-dlp hiccup would otherwise empty the list and report "no
                // playable stream" for a video that is perfectly playable.
                if sources.is_empty() && !ytdlp_urls.is_empty() {
                    eprintln!(
                        "playback for {video_id}: yt-dlp matched no itag, using extractor URLs"
                    );
                    rusty_playback_sources(&player, &HashMap::new())
                } else {
                    sources
                }
            }
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

        // The tail probe is a reachability hint with a short timeout, not proof,
        // so it may only reorder — never discard. An earlier version treated it
        // as a filter, which could empty the list and strand playback on the
        // embed. Probe in priority order, stop at the first source that answers,
        // and cap the whole phase: this runs before the user sees any video, so
        // it must never become the reason playback feels slow to start. If the
        // budget expires the priority order simply stands.
        let promoted = tokio::time::timeout(Duration::from_secs(6), async {
            for index in 0..provider_sources.len() {
                if self
                    .adaptive_tail_is_available(&provider_sources[index])
                    .await
                {
                    return Some(index);
                }
            }
            None
        })
        .await
        .unwrap_or(None);
        if let Some(index) = promoted.filter(|index| *index > 0) {
            let verified = provider_sources.remove(index);
            provider_sources.insert(0, verified);
        }

        if !provider_sources.is_empty() {
            eprintln!(
                "playback for {video_id}: {} sources, primary {:?}, tail probe {}",
                provider_sources.len(),
                provider_sources[0].protocol,
                match promoted {
                    Some(_) => "confirmed a source",
                    None => "confirmed nothing (using priority order)",
                },
            );
            let primary = provider_sources.remove(0);
            return Ok(self
                .proxy_session(PlaybackSession {
                    primary,
                    alternatives: provider_sources,
                    fallback_url,
                })
                .await);
        }

        eprintln!(
            "playback for {video_id}: extraction produced no usable source; \
             the client will report a direct-stream failure"
        );
        Ok(PlaybackSession {
            primary: PlaybackSource {
                protocol: PlaybackProtocol::EmbedFallback,
                url: fallback_url.clone(),
                mime_type: Some("text/html".into()),
                po_token: None,
                expires_at: None,
                quality_label: None,
                request_headers: Vec::new(),
                tracks: Vec::new(),
            },
            alternatives: Vec::new(),
            fallback_url,
        })
    }

    async fn seed_demo_if_empty(&self) -> Result<()> {
        let existing: Vec<DbChannel> = self
            .db
            .query(
                "SELECT channel_id, name, handle, avatar_url, subscriber_count, subscribed, description, banner_url FROM channel LIMIT 1",
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
        for playlist in &demo.playlists {
            self.upsert_playlist(playlist).await?;
        }
        for group in &demo.subscription_groups {
            self.upsert_subscription_group(group).await?;
        }
        self.db
            .query(
                r#"UPSERT library_state:primary SET
                    cache_revision = $cache_revision,
                    queue_json = $queue_json,
                    history_json = $history_json,
                    updated_at = time::now()"#,
            )
            .bind(("cache_revision", demo.cache_revision as i64))
            .bind(("queue_json", serde_json::to_string(&demo.queue)?))
            .bind(("history_json", serde_json::to_string(&demo.history)?))
            .await?
            .check()?;
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
                    watched = $watched"#,
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
                    subscribed = $subscribed"#,
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
            .await?
            .check()?;
        Ok(())
    }

    async fn upsert_playlist(&self, playlist: &Playlist) -> Result<()> {
        self.db
            .query(
                r#"UPSERT type::record('playlist', $record_key) SET
                    playlist_id = $playlist_id,
                    name = $name,
                    video_ids = $video_ids,
                    pinned = $pinned,
                    updated_at = time::now()"#,
            )
            .bind(("record_key", playlist.id.clone()))
            .bind(("playlist_id", playlist.id.clone()))
            .bind(("name", playlist.name.clone()))
            .bind(("video_ids", playlist.video_ids.clone()))
            .bind(("pinned", playlist.pinned))
            .await?
            .check()?;
        Ok(())
    }

    async fn upsert_subscription_group(&self, group: &SubscriptionGroup) -> Result<()> {
        self.db
            .query(
                r#"UPSERT type::record('subscription_group', $record_key) SET
                    group_id = $group_id,
                    name = $name,
                    channel_ids = $channel_ids,
                    updated_at = time::now()"#,
            )
            .bind(("record_key", group.id.clone()))
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

    pub async fn video_details(&self, video_id: &str) -> Result<VideoDetails> {
        if let Some(cached) = self.read_video_details_cache(video_id, true).await? {
            return Ok(self.proxy_captions(cached).await);
        }
        let stale = self.read_video_details_cache(video_id, false).await?;
        let library = self.library_snapshot().await?;
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

    pub async fn channel_details(&self, channel_id: &str) -> Result<ChannelDetails> {
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
            let subscribed = self.channel_is_subscribed(channel_id).await?;
            let channel = rusty_channel_to_channel(&videos_channel, subscribed);
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

        let library = self.library_snapshot().await?;
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

    pub async fn channel_media_page(
        &self,
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
                .library_snapshot()
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

    pub async fn search_catalog(&self, query: &str, filter: &str) -> Result<SearchResults> {
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
            Err(_) => self.search_cached(query, filter).await?,
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

    async fn search_cached(&self, query: &str, filter: &str) -> Result<SearchResults> {
        let snapshot = self.library_snapshot().await?;
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

    pub async fn refresh_feed(&self) -> Result<FeedRefreshResult> {
        let channels: Vec<DbChannel> = self
            .db
            .query(
                "SELECT channel_id, name, handle, avatar_url, subscriber_count, subscribed, description, banner_url, last_polled_at FROM channel WHERE subscribed = true ORDER BY last_polled_at ASC",
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
        let refreshed_channels = covered_channels.len();
        let failed_channels = total_channels.saturating_sub(refreshed_channels);
        Ok(FeedRefreshResult {
            library: self.library_snapshot().await?,
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
        let library = self.library_snapshot().await?;
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
        Ok(self.refresh_feed().await?.imported)
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
    let mut status = upstream.status();
    let mut upstream_headers = upstream.headers().clone();
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
    } else if range_requested {
        // Do not commit response headers to the browser until an entire DASH
        // byte range has arrived. If the upstream CDN closes mid-segment,
        // retry here so MediaSource sees one complete segment instead of a
        // truncated response followed by a fatal playback error.
        let mut bytes = upstream.bytes().await.ok();
        for attempt in 0..3 {
            if bytes.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(150 * (attempt + 1))).await;
            let retry = match request_playback_upstream(&state, &target, &request_headers).await {
                Ok(response) => response,
                Err(_) => continue,
            };
            status = retry.status();
            upstream_headers = retry.headers().clone();
            bytes = retry.bytes().await.ok();
        }
        let Some(bytes) = bytes else {
            return media_proxy_error(StatusCode::BAD_GATEWAY, "Media range is unavailable");
        };
        if generic_type {
            sniffed_type = sniff_media_type(&bytes);
        }
        Body::from(bytes)
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

    // Probed together rather than in sequence: one round trip per format is
    // tolerable in parallel and ruinous serially.
    let probes = candidates
        .iter()
        .map(|(_, itag, _, url)| probe_segment_ranges(client, video_id, *itag, url));
    let ranges = futures_util::future::join_all(probes).await;

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

/// Ranges keyed by video and itag.
///
/// The URLs expire but the container layout does not, so a probe is paid for
/// once per format rather than once per playback.
static SEGMENT_RANGE_CACHE: std::sync::OnceLock<
    std::sync::Mutex<HashMap<(String, u32), SegmentRanges>>,
> = std::sync::OnceLock::new();

fn cached_segment_ranges(video_id: &str, itag: u32) -> Option<SegmentRanges> {
    SEGMENT_RANGE_CACHE
        .get_or_init(Default::default)
        .lock()
        .ok()?
        .get(&(video_id.to_string(), itag))
        .copied()
}

fn store_segment_ranges(video_id: &str, itag: u32, ranges: SegmentRanges) {
    if let Some(cache) = SEGMENT_RANGE_CACHE
        .get_or_init(Default::default)
        .lock()
        .ok()
    {
        let mut cache = cache;
        cache.insert((video_id.to_string(), itag), ranges);
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
    video_id: &str,
    itag: u32,
    url: &str,
) -> Option<SegmentRanges> {
    if let Some(cached) = cached_segment_ranges(video_id, itag) {
        return Some(cached);
    }
    let response = client
        .get(url)
        .header(reqwest::header::RANGE, "bytes=0-32767")
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let head = response.bytes().await.ok()?;
    let ranges = segment_ranges(&head)?;
    store_segment_ranges(video_id, itag, ranges);
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

/// When yt-dlp produced any URLs at all, a track it does not cover is dropped
/// rather than kept: its extractor URL is known to be gated, so offering it
/// only invites the ABR manager to select it, take a 403, and recover. When
/// yt-dlp is unavailable entirely the extractor URL is used as-is, which still
/// plays for about a minute — degraded, but better than refusing to play.
fn resolved_stream_url(
    ytdlp_urls: &HashMap<u32, (String, Option<u64>)>,
    itag: u32,
    size: Option<u64>,
    fallback: &str,
) -> Option<String> {
    if ytdlp_urls.is_empty() {
        return Some(fallback.to_string());
    }
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
                url: resolved_stream_url(ytdlp_urls, stream.itag, stream.size, &stream.url)?,
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
                url: resolved_stream_url(ytdlp_urls, stream.itag, stream.size, &stream.url)?,
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
                    url: resolved_stream_url(
                        ytdlp_urls,
                        stream.itag,
                        Some(stream.size),
                        &stream.url,
                    )?,
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
            let (timestamp, title) = line.split_once(char::is_whitespace)?;
            let start_seconds = parse_timestamp(timestamp)?;
            let title = title.trim().trim_start_matches(['-', '–', '—', ':']).trim();
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
            if let Err(error) = state.poll_subscriptions_once().await {
                eprintln!("subscription poll failed: {error:#}");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{
        AppServerState, canonical_sort_key, center_vtt_cues, ebml_vint, extract_chapters,
        mp4_segment_ranges, parse_youtube_feed, playback_proxy_options, reconciliation_limit,
        segment_ranges, sniff_media_type, url_path_ends_with, video_published_epoch,
        webm_segment_ranges, websub_channel_from_topic,
    };
    use crate::models::PlaybackProtocol;

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
    fn bounds_local_reconciliation_when_push_is_active() {
        assert_eq!(reconciliation_limit(700, true), 48);
        assert_eq!(reconciliation_limit(700, false), 700);
        assert_eq!(reconciliation_limit(12, true), 12);
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
        let snapshot = state.library_snapshot().await.unwrap();
        let video_id = snapshot.videos[0].id.clone();
        let details = state.video_details(&video_id).await.unwrap();
        assert_eq!(details.video.id, video_id);
        // Seeded videos use real YouTube ids, so direct extraction may or may
        // not reach YouTube from the machine running the tests. Either way the
        // call has to resolve to that video with something to show next, which
        // is what the offline library fallback guarantees.
        assert!(!details.related_videos.is_empty());
    }

    #[tokio::test]
    async fn websub_renewal_is_a_noop_without_a_public_callback() {
        let state = AppServerState::initialize().await.unwrap();
        assert_eq!(state.renew_websub_subscriptions().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn playback_session_always_has_a_safe_fallback() {
        let state = AppServerState::initialize().await.unwrap();
        let session = state.playback_session("aqz-KE-bpKQ", true).await.unwrap();
        assert!(session.fallback_url.contains("youtube-nocookie.com"));
        assert!(!session.primary.url.is_empty());
    }

    #[tokio::test]
    async fn syncs_revisioned_library_state() {
        let state = AppServerState::initialize().await.unwrap();
        let mut snapshot = state.library_snapshot().await.unwrap();
        assert!(!snapshot.playlists.is_empty());
        assert!(!snapshot.subscription_groups.is_empty());

        let video_id = snapshot.videos[0].id.clone();
        snapshot.subscription_groups[0].name = "Favorites".into();
        snapshot.queue.insert(0, video_id.clone());
        snapshot.videos[0].watched = true;
        snapshot.videos[0].progress_seconds = snapshot.videos[0].duration_seconds;
        snapshot.cache_revision += 1;

        let synced = state.sync_library(snapshot).await.unwrap();
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
        let server_wins = state.sync_library(stale).await.unwrap();
        assert_eq!(server_wins.queue.first(), Some(&video_id));
    }

    #[tokio::test]
    #[ignore = "live YouTube connectivity check"]
    async fn live_youtube_search_subscribe_channel_and_feed_round_trip() {
        let state = AppServerState::initialize().await.unwrap();
        let results = state
            .search_catalog("Alan Chikin Chow", "all")
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

        let details = state.channel_details(&channel.id).await.unwrap();
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
        let video_details = state.video_details(&playable_video.id).await.unwrap();
        assert!(video_details.remote_available);
        let playback = state
            .playback_session(&playable_video.id, true)
            .await
            .unwrap();
        eprintln!("live playback protocol: {:?}", playback.primary.protocol);
        match playback.primary.protocol {
            PlaybackProtocol::EmbedFallback => {
                assert!(playback.primary.url.contains(&playable_video.id));
                assert!(playback.primary.url.contains("youtube-nocookie.com"));
            }
            _ => assert!(playback.primary.url.contains("/api/v1/playback/proxy/")),
        }

        let mut snapshot = state.library_snapshot().await.unwrap();
        snapshot
            .channels
            .iter_mut()
            .find(|cached| cached.id == channel.id)
            .expect("searched channel is cached")
            .subscribed = true;
        snapshot.cache_revision += 1;
        let synced = state.sync_library(snapshot).await.unwrap();
        assert!(
            synced
                .channels
                .iter()
                .any(|cached| cached.id == channel.id && cached.subscribed)
        );

        let refresh = state.refresh_feed().await.unwrap();
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
