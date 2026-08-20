use serde::{Deserialize, Serialize};
use surrealdb_types::SurrealValue;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SurrealValue)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub handle: String,
    pub avatar_url: Option<String>,
    pub subscriber_count: String,
    pub subscribed: bool,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub banner_url: Option<String>,
}

impl Channel {
    /// Merge metadata discovered by another YouTube surface without letting a
    /// sparse search result erase the richer channel record already cached.
    /// Subscription state is local state and is therefore never replaced.
    pub fn merge_metadata_from(&mut self, discovered: &Channel) -> bool {
        let before = self.clone();
        if !discovered.name.trim().is_empty() {
            self.name = discovered.name.clone();
        }
        if !discovered.handle.trim().is_empty() {
            self.handle = discovered.handle.clone();
        }
        if discovered.avatar_url.is_some() {
            self.avatar_url = discovered.avatar_url.clone();
        }
        if discovered.banner_url.is_some() {
            self.banner_url = discovered.banner_url.clone();
        }
        if !discovered.description.trim().is_empty() {
            self.description = discovered.description.clone();
        }

        let existing_count = compact_count_value(&self.subscriber_count);
        let discovered_count = compact_count_value(&discovered.subscriber_count);
        if !discovered.subscriber_count.trim().is_empty()
            && (self.subscriber_count.trim().is_empty()
                || discovered_count
                    .zip(existing_count)
                    .is_some_and(|(new, old)| new > old))
        {
            self.subscriber_count = discovered.subscriber_count.clone();
        }
        *self != before
    }
}

fn compact_count_value(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    let numeric = trimmed
        .chars()
        .take_while(|character| character.is_ascii_digit() || matches!(character, '.' | ','))
        .collect::<String>()
        .replace(',', "");
    let suffix = trimmed
        .chars()
        .skip_while(|character| character.is_ascii_digit() || matches!(character, '.' | ','))
        .find(|character| character.is_ascii_alphabetic());
    let multiplier = match suffix.map(|character| character.to_ascii_uppercase()) {
        Some('K') => 1_000.0,
        Some('M') => 1_000_000.0,
        Some('B') => 1_000_000_000.0,
        _ => 1.0,
    };
    numeric.parse::<f64>().ok().map(|count| count * multiplier)
}

#[cfg(test)]
mod channel_tests {
    use super::Channel;

    fn channel(count: &str) -> Channel {
        Channel {
            id: "UC-test".into(),
            name: "Veritasium".into(),
            handle: "@veritasium".into(),
            avatar_url: Some("rich-avatar".into()),
            subscriber_count: count.into(),
            subscribed: true,
            description: "Rich description".into(),
            banner_url: Some("rich-banner".into()),
        }
    }

    #[test]
    fn sparse_channel_metadata_does_not_erase_richer_cache() {
        let mut cached = channel("21.1M");
        let mut sparse = channel("528");
        sparse.avatar_url = None;
        sparse.banner_url = None;
        sparse.description.clear();
        sparse.subscribed = false;

        cached.merge_metadata_from(&sparse);

        assert_eq!(cached.subscriber_count, "21.1M");
        assert_eq!(cached.avatar_url.as_deref(), Some("rich-avatar"));
        assert_eq!(cached.banner_url.as_deref(), Some("rich-banner"));
        assert_eq!(cached.description, "Rich description");
        assert!(cached.subscribed);
    }

    #[test]
    fn richer_channel_count_wins_even_with_a_label() {
        let mut cached = channel("528 subscribers");
        let discovered = channel("21.1M subscribers");
        cached.merge_metadata_from(&discovered);
        assert_eq!(cached.subscriber_count, "21.1M subscribers");
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SurrealValue)]
pub struct Video {
    pub id: String,
    pub title: String,
    pub channel_id: String,
    pub channel_name: String,
    pub thumbnail_url: String,
    pub published_at: String,
    pub duration_seconds: u64,
    pub view_count: String,
    pub progress_seconds: u64,
    pub watched: bool,
    pub is_live: bool,
    #[serde(default)]
    pub is_short: bool,
}

impl Video {
    pub fn duration_label(&self) -> String {
        if self.is_live {
            return "LIVE".to_string();
        }
        let hours = self.duration_seconds / 3600;
        let minutes = (self.duration_seconds % 3600) / 60;
        let seconds = self.duration_seconds % 60;
        if hours > 0 {
            format!("{hours}:{minutes:02}:{seconds:02}")
        } else {
            format!("{minutes}:{seconds:02}")
        }
    }

    pub fn progress_percent(&self) -> f64 {
        if self.duration_seconds == 0 {
            0.0
        } else {
            (self.progress_seconds as f64 / self.duration_seconds as f64 * 100.0).min(100.0)
        }
    }

    pub fn published_epoch(&self) -> i64 {
        use time::{Date, OffsetDateTime, format_description::well_known::Rfc3339};
        use web_time::{SystemTime, UNIX_EPOCH};

        let value = self.published_at.trim();
        if let Ok(timestamp) = OffsetDateTime::parse(value, &Rfc3339) {
            return timestamp.unix_timestamp();
        }
        if value.len() >= 10
            && let Ok(format) =
                time::format_description::parse_borrowed::<2>("[year]-[month]-[day]")
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

    /// The publish date as it should be shown.
    ///
    /// Most sources give human text ("2 hours ago", "Jul 7, 2026") which is
    /// passed through untouched. RSS entries and extractor records that carry
    /// no display text fall back to a machine timestamp such as
    /// `2026-07-07 0:00:00.0 +00:00:00`, which must not reach the UI. Those are
    /// reformatted here rather than at ingest so rows already cached with a raw
    /// timestamp render correctly too.
    pub fn published_label(&self) -> String {
        use time::{Date, OffsetDateTime, format_description::well_known::Rfc3339};

        let value = self.published_at.trim();
        // "From YouTube" is the ingest placeholder for "no date was reported".
        // It is not a date, so it should not be rendered where one belongs.
        if value.is_empty() || value == "From YouTube" {
            return String::new();
        }
        let date = OffsetDateTime::parse(value, &Rfc3339)
            .ok()
            .map(|timestamp| timestamp.date())
            .or_else(|| {
                let format =
                    time::format_description::parse_borrowed::<2>("[year]-[month]-[day]").ok()?;
                Date::parse(value.get(..10)?, &format).ok()
            });
        match date {
            Some(date) => format!(
                "{} {}, {}",
                short_month(date.month()),
                date.day(),
                date.year()
            ),
            None => value.to_string(),
        }
    }
}

impl Video {
    /// The "views · date" line, dropping either half when it is unknown so the
    /// separator never dangles.
    pub fn stats_label(&self) -> String {
        let date = self.published_label();
        match (self.view_count.trim().is_empty(), date.is_empty()) {
            (true, true) => String::new(),
            (false, true) => self.view_count.trim().to_string(),
            (true, false) => date,
            (false, false) => format!("{} · {date}", self.view_count.trim()),
        }
    }
}

fn short_month(month: time::Month) -> &'static str {
    use time::Month::*;
    match month {
        January => "Jan",
        February => "Feb",
        March => "Mar",
        April => "Apr",
        May => "May",
        June => "Jun",
        July => "Jul",
        August => "Aug",
        September => "Sep",
        October => "Oct",
        November => "Nov",
        December => "Dec",
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SurrealValue)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub video_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SurrealValue)]
pub struct SubscriptionGroup {
    pub id: String,
    pub name: String,
    pub channel_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResults {
    pub query: String,
    pub videos: Vec<Video>,
    pub channels: Vec<Channel>,
    pub suggestion: Option<String>,
    pub remote_available: bool,
    #[serde(default)]
    pub next_page: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelMediaTab {
    #[default]
    Videos,
    Shorts,
    Live,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelMediaPage {
    pub channel_id: String,
    pub tab: ChannelMediaTab,
    pub videos: Vec<Video>,
    pub next_page: Option<String>,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelDetails {
    pub channel: Channel,
    pub videos: ChannelMediaPage,
    pub shorts: ChannelMediaPage,
    pub live: ChannelMediaPage,
    pub remote_available: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FeedRefreshResult {
    pub library: LibrarySnapshot,
    pub imported: usize,
    pub refreshed_channels: usize,
    pub failed_channels: usize,
    pub sources: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoChapter {
    pub title: String,
    pub start_seconds: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoPreviewFrames {
    pub page_urls: Vec<String>,
    pub frame_width: u32,
    pub frame_height: u32,
    pub total_count: u32,
    pub duration_per_frame_ms: u32,
    pub frames_per_page_x: u32,
    pub frames_per_page_y: u32,
}

impl VideoChapter {
    pub fn timestamp_label(&self) -> String {
        let hours = self.start_seconds / 3600;
        let minutes = (self.start_seconds % 3600) / 60;
        let seconds = self.start_seconds % 60;
        if hours > 0 {
            format!("{hours}:{minutes:02}:{seconds:02}")
        } else {
            format!("{minutes}:{seconds:02}")
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptionTrack {
    pub label: String,
    pub language_code: String,
    pub mime_type: String,
    pub url: String,
    pub auto_generated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoComment {
    pub id: String,
    pub author: String,
    pub author_avatar_url: Option<String>,
    pub author_channel_id: Option<String>,
    pub text: String,
    pub published_at: String,
    pub like_count: u64,
    pub reply_count: u64,
    pub pinned: bool,
    pub hearted: bool,
    pub creator: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentsPage {
    pub comments: Vec<VideoComment>,
    pub next_page: Option<String>,
    pub disabled: bool,
    pub remote_available: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoDetails {
    pub video: Video,
    pub channel: Option<Channel>,
    pub description: String,
    pub like_count: u64,
    pub dislike_count: u64,
    pub captions: Vec<CaptionTrack>,
    pub chapters: Vec<VideoChapter>,
    #[serde(default)]
    pub preview_frames: Option<VideoPreviewFrames>,
    pub related_videos: Vec<Video>,
    pub comments: CommentsPage,
    pub remote_available: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub video_id: String,
    pub played_at: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedFilter {
    All,
    Videos,
    Shorts,
    Live,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DurationFilter {
    Short,
    Medium,
    Long,
}

impl DurationFilter {
    pub const ALL: [Self; 3] = [Self::Short, Self::Medium, Self::Long];

    pub fn label(self) -> &'static str {
        match self {
            Self::Short => "Short",
            Self::Medium => "Medium",
            Self::Long => "Long",
        }
    }

    pub fn matches(self, video: &Video, settings: &AppSettings) -> bool {
        if video.is_live || video.duration_seconds == 0 {
            return false;
        }
        let (short_max, medium_max) = if video.is_short {
            (
                settings.shorts_short_max_seconds,
                settings.shorts_medium_max_seconds,
            )
        } else {
            (
                settings.video_short_max_seconds,
                settings.video_medium_max_seconds,
            )
        };
        match self {
            Self::Short => video.duration_seconds <= short_max,
            Self::Medium => {
                video.duration_seconds > short_max && video.duration_seconds <= medium_max
            }
            Self::Long => video.duration_seconds > medium_max,
        }
    }
}

impl Default for FeedFilter {
    fn default() -> Self {
        Self::All
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Appearance {
    Dark,
    Light,
}

impl Default for Appearance {
    fn default() -> Self {
        Self::Dark
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    pub appearance: Appearance,
    pub playback_speed: f64,
    /// Shorts are watched at a different pace than long-form video, so the two
    /// speeds are remembered independently rather than sharing one setting.
    #[serde(default = "default_speed")]
    pub shorts_playback_speed: f64,
    pub swipe_right_playlist_id: String,
    pub swipe_left_playlist_id: String,
    /// What each swipe direction actually does. The playlist ids above are only
    /// consulted when the action is `AddToPlaylist`.
    #[serde(default)]
    pub swipe_right_action: SwipeActionKind,
    #[serde(default)]
    pub swipe_left_action: SwipeActionKind,
    pub hide_watched: bool,
    pub autoplay: bool,
    #[serde(default = "default_true")]
    pub auto_landscape_fullscreen: bool,
    #[serde(default = "default_video_short_max_seconds")]
    pub video_short_max_seconds: u64,
    #[serde(default = "default_video_medium_max_seconds")]
    pub video_medium_max_seconds: u64,
    #[serde(default = "default_shorts_short_max_seconds")]
    pub shorts_short_max_seconds: u64,
    #[serde(default = "default_shorts_medium_max_seconds")]
    pub shorts_medium_max_seconds: u64,
    pub prefer_sabr: bool,
    pub po_token_provider_url: Option<String>,
}

fn default_speed() -> f64 {
    1.0
}

fn default_true() -> bool {
    true
}

fn default_video_short_max_seconds() -> u64 {
    5 * 60
}

fn default_video_medium_max_seconds() -> u64 {
    20 * 60
}

fn default_shorts_short_max_seconds() -> u64 {
    30
}

fn default_shorts_medium_max_seconds() -> u64 {
    60
}

/// What a swipe on a video card does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwipeActionKind {
    #[default]
    AddToPlaylist,
    AddToQueue,
    PlayNext,
    Share,
    MarkWatched,
}

impl SwipeActionKind {
    pub const ALL: [Self; 5] = [
        Self::AddToPlaylist,
        Self::AddToQueue,
        Self::PlayNext,
        Self::Share,
        Self::MarkWatched,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::AddToPlaylist => "Add to playlist",
            Self::AddToQueue => "Add to queue",
            Self::PlayNext => "Play next",
            Self::Share => "Share",
            Self::MarkWatched => "Mark watched",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.label() == label)
    }
}

impl AppSettings {
    /// The remembered speed for this kind of video.
    pub fn speed_for(&self, is_short: bool) -> f64 {
        if is_short {
            self.shorts_playback_speed
        } else {
            self.playback_speed
        }
    }

    pub fn set_speed_for(&mut self, is_short: bool, speed: f64) {
        if is_short {
            self.shorts_playback_speed = speed;
        } else {
            self.playback_speed = speed;
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            appearance: Appearance::Dark,
            playback_speed: 1.0,
            shorts_playback_speed: 1.0,
            swipe_right_action: SwipeActionKind::AddToPlaylist,
            swipe_left_action: SwipeActionKind::AddToPlaylist,
            swipe_right_playlist_id: "watch-later".to_string(),
            swipe_left_playlist_id: "deep-dives".to_string(),
            hide_watched: false,
            autoplay: true,
            auto_landscape_fullscreen: true,
            video_short_max_seconds: default_video_short_max_seconds(),
            video_medium_max_seconds: default_video_medium_max_seconds(),
            shorts_short_max_seconds: default_shorts_short_max_seconds(),
            shorts_medium_max_seconds: default_shorts_medium_max_seconds(),
            prefer_sabr: true,
            po_token_provider_url: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LibrarySnapshot {
    pub channels: Vec<Channel>,
    pub videos: Vec<Video>,
    pub playlists: Vec<Playlist>,
    #[serde(default)]
    pub subscription_groups: Vec<SubscriptionGroup>,
    #[serde(default)]
    pub queue: Vec<String>,
    #[serde(default)]
    pub history: Vec<HistoryEntry>,
    pub last_synced_at: Option<String>,
    pub cache_revision: u64,
}

impl LibrarySnapshot {
    pub fn demo() -> Self {
        let channels = vec![
            Channel {
                id: "UC-tawny-byte".into(),
                name: "Byte Sized".into(),
                handle: "@bytesized".into(),
                avatar_url: None,
                subscriber_count: "482K".into(),
                subscribed: true,
                description: "Practical Rust and software architecture.".into(),
                banner_url: None,
            },
            Channel {
                id: "UC-tawny-signal".into(),
                name: "Signal Path".into(),
                handle: "@signalpath".into(),
                avatar_url: None,
                subscriber_count: "1.2M".into(),
                subscribed: true,
                description: "Signals, space, and ambitious experiments.".into(),
                banner_url: None,
            },
            Channel {
                id: "UC-tawny-field".into(),
                name: "Field Notes".into(),
                handle: "@fieldnotes".into(),
                avatar_url: None,
                subscriber_count: "206K".into(),
                subscribed: true,
                description: "Thoughtful stories from outdoors.".into(),
                banner_url: None,
            },
            Channel {
                id: "UC-tawny-slow".into(),
                name: "Slow Living Lab".into(),
                handle: "@slowlivinglab".into(),
                avatar_url: None,
                subscriber_count: "734K".into(),
                subscribed: true,
                description: "Make technology feel quieter.".into(),
                banner_url: None,
            },
        ];

        let videos = vec![
            Video {
                id: "aqz-KE-bpKQ".into(),
                title: "I rebuilt my tiny studio around one quiet idea".into(),
                channel_id: channels[3].id.clone(),
                channel_name: channels[3].name.clone(),
                thumbnail_url: "https://picsum.photos/seed/tawny-studio/1280/720".into(),
                published_at: "18 minutes ago".into(),
                duration_seconds: 1128,
                view_count: "42K views".into(),
                progress_seconds: 0,
                watched: false,
                is_live: false,
                is_short: false,
            },
            Video {
                id: "M7lc1UVf-VE".into(),
                title: "Rust UI architecture that still feels simple".into(),
                channel_id: channels[0].id.clone(),
                channel_name: channels[0].name.clone(),
                thumbnail_url: "https://picsum.photos/seed/tawny-rust/1280/720".into(),
                published_at: "2 hours ago".into(),
                duration_seconds: 1642,
                view_count: "91K views".into(),
                progress_seconds: 420,
                watched: false,
                is_live: false,
                is_short: false,
            },
            Video {
                id: "ysz5S6PUM-U".into(),
                title: "The surprising physics of a perfect trail".into(),
                channel_id: channels[2].id.clone(),
                channel_name: channels[2].name.clone(),
                thumbnail_url: "https://picsum.photos/seed/tawny-trail/1280/720".into(),
                published_at: "Yesterday".into(),
                duration_seconds: 812,
                view_count: "318K views".into(),
                progress_seconds: 0,
                watched: false,
                is_live: false,
                is_short: false,
            },
            Video {
                id: "jNQXAC9IVRw".into(),
                title: "Live: decoding the signal from deep space".into(),
                channel_id: channels[1].id.clone(),
                channel_name: channels[1].name.clone(),
                thumbnail_url: "https://picsum.photos/seed/tawny-space/1280/720".into(),
                published_at: "Streaming now".into(),
                duration_seconds: 0,
                view_count: "8.4K watching".into(),
                progress_seconds: 0,
                watched: false,
                is_live: true,
                is_short: false,
            },
            Video {
                id: "dQw4w9WgXcQ".into(),
                title: "Five shortcuts I wish I knew before switching editors".into(),
                channel_id: channels[0].id.clone(),
                channel_name: channels[0].name.clone(),
                thumbnail_url: "https://picsum.photos/seed/tawny-editor/1280/720".into(),
                published_at: "3 days ago".into(),
                duration_seconds: 625,
                view_count: "126K views".into(),
                progress_seconds: 625,
                watched: true,
                is_live: false,
                is_short: false,
            },
            Video {
                id: "ScMzIvxBSi4".into(),
                title: "A week in the high desert, without notifications".into(),
                channel_id: channels[2].id.clone(),
                channel_name: channels[2].name.clone(),
                thumbnail_url: "https://picsum.photos/seed/tawny-desert/1280/720".into(),
                published_at: "5 days ago".into(),
                duration_seconds: 2210,
                view_count: "604K views".into(),
                progress_seconds: 0,
                watched: false,
                is_live: false,
                is_short: false,
            },
        ];

        let playlists = vec![
            Playlist {
                id: "watch-later".into(),
                name: "Watch later".into(),
                video_ids: vec![videos[1].id.clone(), videos[2].id.clone()],
            },
            Playlist {
                id: "deep-dives".into(),
                name: "Deep dives".into(),
                video_ids: vec![videos[3].id.clone(), videos[5].id.clone()],
            },
            Playlist {
                id: "weekend-queue".into(),
                name: "Weekend queue".into(),
                video_ids: vec![videos[0].id.clone()],
            },
        ];

        let subscription_groups = vec![
            SubscriptionGroup {
                id: "makers".into(),
                name: "Makers".into(),
                channel_ids: vec![channels[0].id.clone(), channels[1].id.clone()],
            },
            SubscriptionGroup {
                id: "outside".into(),
                name: "Outside".into(),
                channel_ids: vec![channels[2].id.clone(), channels[3].id.clone()],
            },
        ];

        Self {
            channels,
            videos,
            playlists,
            subscription_groups,
            queue: vec!["M7lc1UVf-VE".into(), "ysz5S6PUM-U".into()],
            history: vec![HistoryEntry {
                video_id: "dQw4w9WgXcQ".into(),
                played_at: "3 days ago".into(),
            }],
            last_synced_at: Some("Just now".into()),
            cache_revision: 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackProtocol {
    Sabr,
    Hls,
    Dash,
    Progressive,
    EmbedFallback,
}

/// An inclusive byte range within an adaptive media resource.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackByteRange {
    pub start: u64,
    pub end: u64,
}

/// A single audio or video representation used by an adaptive transport.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackTrack {
    pub kind: PlaybackTrackKind,
    pub url: String,
    pub mime_type: String,
    #[serde(default)]
    pub bitrate: Option<u64>,
    #[serde(default)]
    pub content_length: Option<u64>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub fps: Option<u32>,
    #[serde(default)]
    pub quality_label: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default)]
    pub init_range: Option<PlaybackByteRange>,
    #[serde(default)]
    pub index_range: Option<PlaybackByteRange>,
    #[serde(default)]
    pub request_headers: Vec<PlaybackRequestHeader>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackTrackKind {
    Video,
    Audio,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackSource {
    pub protocol: PlaybackProtocol,
    pub url: String,
    pub mime_type: Option<String>,
    pub po_token: Option<String>,
    pub expires_at: Option<String>,
    #[serde(default)]
    pub quality_label: Option<String>,
    #[serde(default)]
    pub request_headers: Vec<PlaybackRequestHeader>,
    /// Adaptive representations. An empty list keeps the legacy single-URL
    /// progressive/manifest resolver contract backwards compatible.
    #[serde(default)]
    pub tracks: Vec<PlaybackTrack>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackRequestHeader {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackSession {
    pub primary: PlaybackSource,
    #[serde(default)]
    pub alternatives: Vec<PlaybackSource>,
    pub fallback_url: String,
}
