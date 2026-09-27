use super::*;

/// Where this viewer has reached in one video, sent by `save_progress`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoProgress {
    pub video_id: String,
    pub watched: bool,
    pub progress_seconds: u64,
    #[serde(default)]
    pub audio_only: bool,
}

/// One account's own state. Kilobytes: ids and names, never the videos they
/// point at.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Viewer {
    /// The channels this account follows, each with the uploads it wants.
    pub subscriptions: Vec<Channel>,
    pub playlists: Vec<Playlist>,
    pub subscription_groups: Vec<SubscriptionGroup>,
    /// Video ids, and `playlist:` markers for a queued run.
    pub queue: Vec<String>,
    /// Most recent first, at most [`HISTORY_LIMIT`].
    pub history: Vec<HistoryEntry>,
}

impl Viewer {
    pub fn follows(&self, channel_id: &str) -> Option<&Channel> {
        self.subscriptions
            .iter()
            .find(|channel| channel.id == channel_id)
    }

    pub fn playlist(&self, playlist_id: &str) -> Option<&Playlist> {
        self.playlists
            .iter()
            .find(|playlist| playlist.id == playlist_id)
    }
}

/// One subscription as a device last saw it, sent back as the answer to set.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubscriptionChange {
    pub channel: Channel,
    pub subscribed: bool,
    pub content: SubscriptionContent,
}

/// How many history entries an account keeps.
pub const HISTORY_LIMIT: usize = 250;

#[cfg_attr(not(feature = "server"), allow(dead_code))]
/// How many videos one feed page holds.
pub const FEED_PAGE_SIZE: usize = 24;

/// Which subscriptions a feed draws from.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedGroup {
    #[default]
    All,
    /// Channels in no group.
    Ungrouped,
    Group(String),
}

/// The length buckets' edges, from the viewer's settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurationThresholds {
    pub video_short_max_seconds: u64,
    pub video_medium_max_seconds: u64,
    pub shorts_short_max_seconds: u64,
    pub shorts_medium_max_seconds: u64,
}

impl From<&AppSettings> for DurationThresholds {
    fn from(settings: &AppSettings) -> Self {
        Self {
            video_short_max_seconds: settings.video_short_max_seconds,
            video_medium_max_seconds: settings.video_medium_max_seconds,
            shorts_short_max_seconds: settings.shorts_short_max_seconds,
            shorts_medium_max_seconds: settings.shorts_medium_max_seconds,
        }
    }
}

/// Everything that decides which videos the subscription feed shows. Part of
/// the cache key, so each combination is its own cached answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedQuery {
    pub group: FeedGroup,
    pub kind: FeedFilter,
    pub duration: Option<DurationFilter>,
    pub hide_watched: bool,
    pub thresholds: DurationThresholds,
}

/// One page of the subscription feed, newest first.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FeedPage {
    pub videos: Vec<Video>,
    pub has_more: bool,
    /// With a length filter on: how many videos on this page's stretch of the
    /// feed have no length yet, so the filter cannot speak for them.
    pub without_duration: usize,
    /// Those videos' ids, for the screen to ask their lengths for.
    pub unknown_durations: Vec<String>,
}

/// Anything that holds videos, so a change to one video (its progress, its
/// length) can be applied to every cached list showing it at once.
pub trait VideoList {
    fn videos_mut(&mut self) -> Box<dyn Iterator<Item = &mut Video> + '_>;

    fn update_video(&mut self, video_id: &str, mut update: impl FnMut(&mut Video)) {
        for video in self.videos_mut().filter(|video| video.id == video_id) {
            update(video);
        }
    }

    /// Take the catalog facts of `fresh` (length, live and Shorts state)
    /// without touching this viewer's progress.
    fn merge_metadata(&mut self, fresh: &[Video]) {
        for video in self.videos_mut() {
            if let Some(fresh) = fresh.iter().find(|fresh| fresh.id == video.id) {
                video.duration_seconds = fresh.duration_seconds;
                video.is_live = fresh.is_live;
                video.is_short = fresh.is_short;
            }
        }
    }
}

impl VideoList for Vec<Video> {
    fn videos_mut(&mut self) -> Box<dyn Iterator<Item = &mut Video> + '_> {
        Box::new(self.iter_mut())
    }
}

impl VideoList for FeedPage {
    fn videos_mut(&mut self) -> Box<dyn Iterator<Item = &mut Video> + '_> {
        Box::new(self.videos.iter_mut())
    }
}

/// One playlist on the playlists page: its cover and its counts, without the
/// videos themselves.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaylistPreview {
    pub id: String,
    pub name: String,
    pub count: usize,
    pub unwatched: usize,
    /// The first few videos' thumbnails, in playlist order.
    pub thumbnails: Vec<String>,
}

/// A playlist with its videos, in the playlist's own order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlaylistContents {
    pub playlist: Playlist,
    pub videos: Vec<Video>,
}

impl VideoList for PlaylistContents {
    fn videos_mut(&mut self) -> Box<dyn Iterator<Item = &mut Video> + '_> {
        Box::new(self.videos.iter_mut())
    }
}

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------
