use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use surrealdb_types::SurrealValue;

/// Which of a channel's uploads a subscription actually wants in the feed.
///
/// A channel's Shorts and its long-form uploads are often two different shows,
/// and subscribing is currently all-or-nothing. This narrows a subscription
/// without unsubscribing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, SurrealValue)]
#[serde(rename_all = "kebab-case")]
pub enum SubscriptionContent {
    /// Everything the channel publishes, including livestreams.
    #[default]
    All,
    /// Long-form uploads only; Shorts are dropped from the feed.
    Videos,
    /// Shorts only.
    Shorts,
}

impl SubscriptionContent {
    /// SurrealDB stores this as a plain string so the column stays readable and
    /// an unknown value degrades to "everything" instead of failing the row.
    /// Only the server reads and writes it.
    #[cfg_attr(not(feature = "server"), allow(dead_code))]
    pub fn from_storage(value: &str) -> Self {
        match value {
            "videos" => Self::Videos,
            "shorts" => Self::Shorts,
            _ => Self::All,
        }
    }

    #[cfg_attr(not(feature = "server"), allow(dead_code))]
    pub fn as_storage(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Videos => "videos",
            Self::Shorts => "shorts",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "Videos & Shorts",
            Self::Videos => "Videos only",
            Self::Shorts => "Shorts only",
        }
    }
}

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
    #[serde(default)]
    pub subscription_content: SubscriptionContent,
}

impl Channel {
    #[cfg_attr(not(feature = "server"), allow(dead_code))]
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

#[cfg_attr(not(feature = "server"), allow(dead_code))]
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
            subscription_content: super::SubscriptionContent::All,
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
    /// Play this one without its video stream. Remembered per video, because it
    /// is a property of the thing being watched - a podcast stays audio, a music
    /// video does not - rather than a global mode.
    #[serde(default)]
    pub audio_only: bool,
    /// The channel's avatar, filled in by the server on every video it hands
    /// out so a card need not look the channel up.
    #[serde(default)]
    pub channel_avatar_url: Option<String>,
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
        if let Some(date) = text_date(value) {
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

    #[cfg_attr(not(feature = "server"), allow(dead_code))]
    /// How exactly `published_at` pins the upload down: 3 for a timestamp, 2 for
    /// a calendar date, 1 for relative text ("3 days ago"), 0 for nothing usable.
    fn publish_precision(&self) -> u8 {
        use time::{OffsetDateTime, format_description::well_known::Rfc3339};

        let value = self.published_at.trim();
        if OffsetDateTime::parse(value, &Rfc3339).is_ok() {
            return 3;
        }
        let iso_date = value.len() >= 10
            && time::format_description::parse_borrowed::<2>("[year]-[month]-[day]")
                .is_ok_and(|format| time::Date::parse(&value[..10], &format).is_ok());
        if iso_date || text_date(value).is_some() {
            return 2;
        }
        if self.published_epoch() != 0 { 1 } else { 0 }
    }

    #[cfg_attr(not(feature = "server"), allow(dead_code))]
    /// Keep `previous`'s publish date when it is more exact than this one.
    ///
    /// A video's date does not change, but the sources describing it do: the
    /// feed gives a timestamp, the watch page a day, a related-videos shelf "11
    /// months ago". Taking whichever arrived last moved a video within a
    /// newest-first playlist just for having been opened.
    pub fn keep_finer_publish_date(&mut self, previous: &Video) {
        if previous.publish_precision() > self.publish_precision() {
            self.published_at = previous.published_at.clone();
        }
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

/// A written-out English date such as "Aug 2, 2013", "Premiered Aug 2, 2013"
/// or "Streamed live on August 2, 2013".
///
/// Video details used to be stored with YouTube's display text, and a date the
/// sorts cannot read counts as 1970: the video opened last sank to the bottom
/// of a newest-first playlist, stranding a run on it. Rows cached that way are
/// still around, so they are read rather than refetched.
pub fn text_date(value: &str) -> Option<time::Date> {
    let words = value
        .split(|character: char| character.is_whitespace() || character == ',')
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    words.windows(3).find_map(|window| {
        let month = month_from_name(window[0])?;
        let day = window[1].parse::<u8>().ok()?;
        let year = window[2].parse::<i32>().ok().filter(|year| *year >= 1000)?;
        time::Date::from_calendar_date(year, month, day).ok()
    })
}

fn month_from_name(word: &str) -> Option<time::Month> {
    use time::Month::*;
    let prefix = word.get(..3)?.to_ascii_lowercase();
    Some(match prefix.as_str() {
        "jan" => January,
        "feb" => February,
        "mar" => March,
        "apr" => April,
        "may" => May,
        "jun" => June,
        "jul" => July,
        "aug" => August,
        "sep" => September,
        "oct" => October,
        "nov" => November,
        "dec" => December,
        _ => return None,
    })
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

/// One selectable audio language for the video being watched.
///
/// Reported by the transport rather than the extractor: Shaka is the authority
/// on which of the manifest's audio adaptations it can actually decode and
/// switch between, and it carries the display names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioTrackOption {
    pub language: String,
    pub label: String,
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

impl FeedFilter {
    pub const ALL: [Self; 4] = [Self::All, Self::Videos, Self::Shorts, Self::Live];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Videos => "Videos",
            Self::Shorts => "Shorts",
            Self::Live => "Live",
        }
    }
}

/// What a search looks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExploreFilter {
    All,
    Videos,
    Channels,
}

impl ExploreFilter {
    pub const ALL: [Self; 3] = [Self::All, Self::Videos, Self::Channels];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Videos => "Videos",
            Self::Channels => "Channels",
        }
    }

    /// The server's name for the filter.
    pub fn query(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Videos => "videos",
            Self::Channels => "channels",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

/// How a playlist's videos are ordered on its page.
///
/// `Added` is the playlist's own order and is the default, because a hand-built
/// watchlist already carries the order its owner chose. Anything else would make
/// the control destructive by default rather than helpful.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaylistSort {
    #[default]
    Added,
    Published,
    Duration,
    Title,
}

impl PlaylistSort {
    pub const ALL: [Self; 4] = [Self::Added, Self::Published, Self::Duration, Self::Title];

    /// The label names the resulting order rather than the field it sorts on,
    /// because the chip is the only place the direction is visible: tapping an
    /// active chip flips it, and the text has to be what says so.
    pub fn label(self, descending: bool) -> &'static str {
        match (self, descending) {
            (Self::Added, false) => "First added",
            (Self::Added, true) => "Last added",
            (Self::Published, true) => "Newest",
            (Self::Published, false) => "Oldest",
            (Self::Duration, true) => "Longest",
            (Self::Duration, false) => "Shortest",
            (Self::Title, false) => "A-Z",
            (Self::Title, true) => "Z-A",
        }
    }

    /// Which direction a sort lands on when it is first picked. Newest and
    /// longest are what someone reaches for; oldest and shortest are the second
    /// tap.
    pub fn default_descending(self) -> bool {
        matches!(self, Self::Published | Self::Duration)
    }

    /// Order `videos` in place. They arrive in the playlist's own order, which
    /// is why `Added` sorts on nothing.
    ///
    /// Reversing after a stable sort rather than comparing backwards also
    /// inverts ties. For `Added` that is the whole point, and elsewhere a tie
    /// means equal keys, so the two orders are equally correct.
    pub fn apply(self, videos: &mut [Video], descending: bool) {
        match self {
            Self::Added => {}
            // Cached, because parsing a published date is not free and a
            // playlist can hold hundreds of them.
            Self::Published => videos.sort_by_cached_key(|video| video.published_epoch()),
            Self::Duration => videos.sort_by_key(|video| video.duration_seconds),
            Self::Title => videos.sort_by_cached_key(|video| video.title.to_lowercase()),
        }
        if descending {
            videos.reverse();
        }
    }
}

/// Which kinds of upload a playlist page is showing.
///
/// Deliberately not [`FeedFilter`], which also carries `Live`. A playlist holds
/// whatever was saved to it, and a persisted filter the segmented control cannot
/// display is a filter nobody can turn off.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaylistKind {
    #[default]
    All,
    Videos,
    Shorts,
}

impl PlaylistKind {
    pub const ALL: [Self; 3] = [Self::All, Self::Videos, Self::Shorts];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Videos => "Videos",
            Self::Shorts => "Shorts",
        }
    }

    /// A live upload counts as a video here. The feed splits Live out because a
    /// stream is a different thing to sit down to; a playlist entry is something
    /// its owner filed by hand, and hiding it under a chip it never named would
    /// just lose it.
    pub fn matches(self, video: &Video) -> bool {
        match self {
            Self::All => true,
            Self::Videos => !video.is_short,
            Self::Shorts => video.is_short,
        }
    }
}

/// How one playlist is arranged: what it shows and in what order.
///
/// Persisted per playlist, because a run is resolved lazily out of the queue -
/// see [`PLAYLIST_QUEUE_PREFIX`] - so the arrangement has to outlive the page
/// that set it. Without that, autoplay would walk a different playlist than the
/// one the viewer was looking at when they pressed play.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlaylistView {
    #[serde(default)]
    pub kind: PlaylistKind,
    #[serde(default)]
    pub only_unwatched: bool,
    #[serde(default)]
    pub duration: Option<DurationFilter>,
    #[serde(default)]
    pub sort: PlaylistSort,
    #[serde(default)]
    pub descending: bool,
}

impl PlaylistView {
    /// Kind and duration filters plus the sort - everything except the watched
    /// filter.
    ///
    /// The watched filter is left out on purpose. A run finds its place by
    /// locating the video that just finished, and finishing a video is exactly
    /// what marks it watched: dropping it here would lose the position and send
    /// the run back to the top of the playlist. Apply [`Self::shows`] on top of
    /// this order instead.
    pub fn arrange(&self, videos: &mut Vec<Video>, settings: &AppSettings) {
        videos.retain(|video| self.kind.matches(video));
        if let Some(duration) = self.duration {
            videos.retain(|video| duration.matches(video, settings));
        }
        self.sort.apply(videos, self.descending);
    }

    /// Whether the watched filter keeps this video on screen.
    pub fn shows(&self, video: &Video) -> bool {
        !self.only_unwatched || !video.watched
    }

    /// What a run plays after `current_video_id`, given an [`Self::arrange`]d
    /// order.
    ///
    /// A current video that is not in `order` means the run has not started yet -
    /// it is sitting behind something else in the queue - so it begins at the
    /// top.
    pub fn next_after(&self, order: &[Video], current_video_id: &str) -> Option<String> {
        let start = order
            .iter()
            .position(|video| video.id == current_video_id)
            .map_or(0, |index| index + 1);
        order
            .get(start..)?
            .iter()
            .find(|video| self.shows(video))
            .map(|video| video.id.clone())
    }

    /// What a run plays before `current_video_id`. `None` once there is nothing
    /// shown ahead of it, and for a video the playlist does not hold: going back
    /// into a run you were never in has no meaning.
    pub fn previous_before(&self, order: &[Video], current_video_id: &str) -> Option<String> {
        let index = order
            .iter()
            .position(|video| video.id == current_video_id)?;
        order
            .get(..index)?
            .iter()
            .rev()
            .find(|video| self.shows(video))
            .map(|video| video.id.clone())
    }

    /// Whether anything is being hidden, which is what an empty page has to
    /// explain: "nothing matches" and "nothing saved" are different problems.
    pub fn is_filtered(&self) -> bool {
        self.only_unwatched || self.duration.is_some() || self.kind != PlaylistKind::All
    }

    /// The filters in force, in words, for somewhere other than the playlist's
    /// own page to say what a run is walking. Empty when nothing is hidden.
    pub fn filter_labels(&self) -> Vec<String> {
        let mut labels = Vec::new();
        match self.kind {
            PlaylistKind::All => {}
            PlaylistKind::Videos => labels.push("Videos only".to_string()),
            PlaylistKind::Shorts => labels.push("Shorts only".to_string()),
        }
        if let Some(duration) = self.duration {
            labels.push(format!("{} length", duration.label()));
        }
        if self.only_unwatched {
            labels.push("Unwatched only".to_string());
        }
        labels
    }
}

/// Marks the queue entry that stands in for "the rest of a playlist".
///
/// A run used to be spilled into the queue as a list of ids. That froze the
/// order and the filters at the moment Play was pressed, could not be told apart
/// from videos queued by hand, and buried a fifty-video playlist in the one
/// place meant for what comes next. A marker is resolved against the playlist's
/// current [`PlaylistView`] each time the run advances, so the queue stays one
/// flat list and a playlist occupies one entry in it.
///
/// A YouTube video id is eleven characters of `[A-Za-z0-9_-]`, so the `:` here
/// can never collide with one.
pub const PLAYLIST_QUEUE_PREFIX: &str = "playlist:";

pub fn playlist_queue_entry(playlist_id: &str) -> String {
    format!("{PLAYLIST_QUEUE_PREFIX}{playlist_id}")
}

/// The playlist a queue entry stands for, or `None` when it is a plain video id.
pub fn queued_playlist_id(entry: &str) -> Option<&str> {
    entry.strip_prefix(PLAYLIST_QUEUE_PREFIX)
}

/// Which native design language the component library renders with.
///
/// g3-ui detects this from the platform at startup, which is right for a
/// packaged build but arbitrary on the web, where the same browser serves both
/// looks. Storing an explicit override lets the choice be made once and kept.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlatformStyle {
    /// Follow the running platform, which is what an installed build wants.
    #[default]
    Auto,
    Ios,
    Material,
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
    #[serde(default)]
    pub platform_style: PlatformStyle,
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
    /// Long-form autoplay. Shorts get their own below, for the same reason
    /// speed does: a feed of ninety-second clips and a forty-minute video are
    /// not the same decision.
    pub autoplay: bool,
    #[serde(default = "default_true")]
    pub shorts_autoplay: bool,
    #[serde(default = "default_true")]
    pub auto_landscape_fullscreen: bool,
    /// Whether a desktop player opens in theater mode. A viewer who chose the
    /// wider stage once meant it for watching, not for this one video.
    #[serde(default)]
    pub theater_mode: bool,
    #[serde(default = "default_video_short_max_seconds")]
    pub video_short_max_seconds: u64,
    #[serde(default = "default_video_medium_max_seconds")]
    pub video_medium_max_seconds: u64,
    #[serde(default = "default_shorts_short_max_seconds")]
    pub shorts_short_max_seconds: u64,
    #[serde(default = "default_shorts_medium_max_seconds")]
    pub shorts_medium_max_seconds: u64,
    #[serde(default)]
    pub sponsor_block: SponsorBlockSettings,
    /// BCP-47 language tag the playback transport should prefer for dubbed
    /// uploads. Stored separately from the browser locale so a device's UI
    /// language cannot unexpectedly change what a viewer hears.
    #[serde(default = "default_audio_language")]
    pub preferred_audio_language: String,
    pub prefer_sabr: bool,
    pub po_token_provider_url: Option<String>,
    /// How each playlist is arranged, keyed by playlist id.
    ///
    /// Kept here rather than on [`Playlist`] because it is a per-device view of
    /// shared data - a phone and a desktop can reasonably sit on different
    /// filters - and because it has to be readable while resolving a queued run,
    /// which happens nowhere near the playlist page. Entries that hold nothing
    /// but defaults are dropped, so this stays empty until a filter is used.
    #[serde(default)]
    pub playlist_views: BTreeMap<String, PlaylistView>,
}

fn default_speed() -> f64 {
    1.0
}

fn default_true() -> bool {
    true
}

fn default_audio_language() -> String {
    "en".to_string()
}

fn default_video_short_max_seconds() -> u64 {
    10 * 60
}

fn default_video_medium_max_seconds() -> u64 {
    35 * 60
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

    /// Compact text for the closed selector; the full label remains in the
    /// menu where it has room to explain the action.
    pub fn trigger_label(self) -> &'static str {
        match self {
            Self::AddToPlaylist => "Playlist",
            Self::AddToQueue => "Queue",
            Self::PlayNext => "Play next",
            Self::Share => "Share",
            Self::MarkWatched => "Watched",
        }
    }
}

impl AppSettings {
    /// The remembered speed for this kind of video.
    pub fn autoplay_for(&self, is_short: bool) -> bool {
        if is_short {
            self.shorts_autoplay
        } else {
            self.autoplay
        }
    }

    pub fn set_autoplay_for(&mut self, is_short: bool, autoplay: bool) {
        if is_short {
            self.shorts_autoplay = autoplay;
        } else {
            self.autoplay = autoplay;
        }
    }

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
            platform_style: PlatformStyle::Auto,
            playback_speed: 1.0,
            shorts_playback_speed: 1.0,
            swipe_right_action: SwipeActionKind::AddToPlaylist,
            swipe_left_action: SwipeActionKind::AddToPlaylist,
            swipe_right_playlist_id: "watch-later".to_string(),
            swipe_left_playlist_id: "deep-dives".to_string(),
            hide_watched: false,
            autoplay: true,
            shorts_autoplay: true,
            auto_landscape_fullscreen: true,
            theater_mode: false,
            video_short_max_seconds: default_video_short_max_seconds(),
            video_medium_max_seconds: default_video_medium_max_seconds(),
            shorts_short_max_seconds: default_shorts_short_max_seconds(),
            shorts_medium_max_seconds: default_shorts_medium_max_seconds(),
            sponsor_block: SponsorBlockSettings::default(),
            preferred_audio_language: default_audio_language(),
            prefer_sabr: true,
            po_token_provider_url: None,
            playlist_views: BTreeMap::new(),
        }
    }
}

#[cfg(test)]
mod settings_tests {
    use super::AppSettings;

    #[test]
    fn audio_preference_starts_with_english() {
        assert_eq!(AppSettings::default().preferred_audio_language, "en");
    }
}

/// Where this viewer has reached in one video, sent by `save_progress`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoProgress {
    pub video_id: String,
    pub watched: bool,
    pub progress_seconds: u64,
    #[serde(default)]
    pub audio_only: bool,
}

#[cfg_attr(not(feature = "server"), allow(dead_code))]
/// What a new account starts with: a few real channels and videos, so the
/// feed is not empty on first launch, and the playlists the default swipes
/// save to. The server seeds the catalog with it on first start.
#[derive(Clone, Debug, PartialEq)]
pub struct DemoLibrary {
    pub channels: Vec<Channel>,
    pub videos: Vec<Video>,
    pub playlists: Vec<Playlist>,
    pub subscription_groups: Vec<SubscriptionGroup>,
    pub queue: Vec<String>,
}

#[cfg_attr(not(feature = "server"), allow(dead_code))]
/// The demo channels, playlists and groups a new account starts with.
pub fn demo_viewer() -> Viewer {
    let demo = DemoLibrary::demo();
    Viewer {
        subscriptions: demo.channels,
        playlists: demo.playlists,
        subscription_groups: demo.subscription_groups,
        queue: demo.queue,
        history: Vec::new(),
    }
}

#[cfg_attr(not(feature = "server"), allow(dead_code))]
impl DemoLibrary {
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
                subscription_content: SubscriptionContent::All,
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
                subscription_content: SubscriptionContent::All,
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
                subscription_content: SubscriptionContent::All,
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
                subscription_content: SubscriptionContent::All,
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
                audio_only: false,
                channel_avatar_url: None,
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
                audio_only: false,
                channel_avatar_url: None,
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
                audio_only: false,
                channel_avatar_url: None,
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
                audio_only: false,
                channel_avatar_url: None,
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
                audio_only: false,
                channel_avatar_url: None,
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
                audio_only: false,
                channel_avatar_url: None,
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

/// Whose side a playback failure is on, which decides what can be done about
/// it: nothing (YouTube's own rules), a fix to the deployment, or a fix to
/// Tawny's code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackFailureOrigin {
    /// YouTube refused the video: region, privacy, age, removal, or YouTube
    /// blocking the server itself. Tawny is working as intended.
    Youtube,
    /// Something Tawny runs on is down, misconfigured or out of date - a
    /// sidecar, an environment variable, the yt-dlp pin. Whoever runs the
    /// server can fix it.
    Setup,
    /// Tawny's own code got it wrong.
    Bug,
    /// An error Tawny does not recognise, so it cannot say whose it is.
    Unknown,
}

impl PlaybackFailureOrigin {
    pub fn label(self) -> &'static str {
        match self {
            Self::Youtube => "Blocked by YouTube",
            Self::Setup => "Server setup problem",
            Self::Bug => "Tawny bug",
            Self::Unknown => "Unrecognised error",
        }
    }
}

/// Why a video cannot be played, sent to the player instead of a bare string
/// so it can say whose problem it is and what, if anything, fixes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackFailure {
    pub origin: PlaybackFailureOrigin,
    /// What went wrong, as one sentence.
    pub summary: String,
    /// What the viewer or whoever runs the server can do, when anything helps.
    #[serde(default)]
    pub remedy: Option<String>,
    /// The underlying error verbatim, for bug reports and the server log.
    #[serde(default)]
    pub detail: Option<String>,
}

impl PlaybackFailure {
    pub fn new(origin: PlaybackFailureOrigin, summary: impl Into<String>) -> Self {
        Self {
            origin,
            summary: summary.into(),
            remedy: None,
            detail: None,
        }
    }

    pub fn remedy(mut self, remedy: impl Into<String>) -> Self {
        self.remedy = Some(remedy.into());
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        let detail = detail.into();
        self.detail = (!detail.trim().is_empty()).then_some(detail);
        self
    }
}

impl std::fmt::Display for PlaybackFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.origin.label(), self.summary)?;
        if let Some(detail) = &self.detail {
            write!(f, " ({detail})")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// The viewer
//
// What one account keeps, read by nearly every screen. The catalog (every
// channel and video the instance has met) stays on the server; each screen
// asks for the videos it shows. See docs/architecture.md.
// ---------------------------------------------------------------------------

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

/// Who the client is currently acting as.
///
/// There is always someone: the first launch mints a guest rather than showing
/// a sign-in wall, so every code path downstream can assume an owner instead of
/// carrying an `Option` for the un-authenticated case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub display_name: String,
    /// `None` for a guest. Its presence is what "has a real account" means.
    #[serde(default)]
    pub email: Option<String>,
    pub is_guest: bool,
}

impl Account {
    /// A guest has nothing to sign back in with, so clearing its cookie loses
    /// the library. That is worth saying out loud in the UI, and this is the
    /// test the UI asks.
    pub fn is_recoverable(&self) -> bool {
        !self.is_guest && self.email.is_some()
    }

    pub fn label(&self) -> &str {
        if let Some(email) = self.email.as_deref() {
            email
        } else {
            &self.display_name
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credentials {
    pub email: String,
    pub password: String,
}

/// Why a credential was refused, in the terms the form needs to render.
///
/// A bare string would do for display, but the form also has to decide *which
/// field* to mark, and parsing that back out of prose is how those two drift
/// apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CredentialProblem {
    EmailEmpty,
    EmailMalformed,
    EmailTaken,
    PasswordTooShort,
    PasswordTooLong,
}

impl CredentialProblem {
    pub fn message(self) -> &'static str {
        match self {
            Self::EmailEmpty => "Enter an email address.",
            Self::EmailMalformed => "That does not look like an email address.",
            Self::EmailTaken => "An account already uses that email.",
            Self::PasswordTooShort => "Use at least 8 characters.",
            Self::PasswordTooLong => "Use at most 512 characters.",
        }
    }
}

/// Argon2 is deliberately slow, so a password long enough to be a denial of
/// service is rejected before it reaches the hasher.
pub const PASSWORD_MIN_LENGTH: usize = 8;
pub const PASSWORD_MAX_LENGTH: usize = 512;

/// Shared by both sides so a client-side form and the server agree on what is
/// acceptable without the rules being written twice.
///
/// Deliberately not an RFC 5322 parser. The only thing this can usefully
/// establish is that the viewer typed something address-shaped; whether it
/// receives mail is not knowable here, and Tawny never sends any.
pub fn validate_email(email: &str) -> Result<String, CredentialProblem> {
    let normalized = email.trim().to_lowercase();
    if normalized.is_empty() {
        return Err(CredentialProblem::EmailEmpty);
    }
    let mut parts = normalized.split('@');
    let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
        return Err(CredentialProblem::EmailMalformed);
    };
    let domain_is_dotted = domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains("..");
    if local.is_empty() || domain.is_empty() || !domain_is_dotted {
        return Err(CredentialProblem::EmailMalformed);
    }
    if normalized.chars().any(char::is_whitespace) {
        return Err(CredentialProblem::EmailMalformed);
    }
    Ok(normalized)
}

pub fn validate_password(password: &str) -> Result<(), CredentialProblem> {
    // Not trimmed: leading and trailing spaces are legitimate password
    // characters, and silently removing them would lock out anyone who used
    // one deliberately.
    if password.chars().count() < PASSWORD_MIN_LENGTH {
        return Err(CredentialProblem::PasswordTooShort);
    }
    if password.chars().count() > PASSWORD_MAX_LENGTH {
        return Err(CredentialProblem::PasswordTooLong);
    }
    Ok(())
}

#[cfg(test)]
mod account_tests {
    use super::*;

    #[test]
    fn email_validation_normalizes_case_and_surrounding_space() {
        assert_eq!(
            validate_email("  Viewer@Example.COM "),
            Ok("viewer@example.com".into())
        );
    }

    #[test]
    fn email_validation_rejects_addresses_that_are_not_address_shaped() {
        for candidate in [
            "",
            "   ",
            "viewer",
            "viewer@",
            "@example.com",
            "viewer@example",
            "viewer@.com",
            "viewer@example.",
            "viewer@exa..mple.com",
            "one@two@example.com",
            "view er@example.com",
        ] {
            assert!(
                validate_email(candidate).is_err(),
                "expected {candidate:?} to be rejected"
            );
        }
    }

    #[test]
    fn password_length_is_bounded_at_both_ends() {
        assert_eq!(
            validate_password("short"),
            Err(CredentialProblem::PasswordTooShort)
        );
        assert!(validate_password("longenough").is_ok());
        assert_eq!(
            validate_password(&"x".repeat(PASSWORD_MAX_LENGTH + 1)),
            Err(CredentialProblem::PasswordTooLong)
        );
    }

    #[test]
    fn password_whitespace_is_significant() {
        // Eight characters only if the spaces count, which is the point.
        assert!(validate_password(" pass a ").is_ok());
    }

    #[test]
    fn only_a_real_account_is_recoverable() {
        let guest = Account {
            id: "app_user:abc".into(),
            display_name: "Guest".into(),
            email: None,
            is_guest: true,
        };
        assert!(!guest.is_recoverable());
        assert_eq!(guest.label(), "Guest");

        let member = Account {
            id: "app_user:abc".into(),
            display_name: "Guest".into(),
            email: Some("viewer@example.com".into()),
            is_guest: false,
        };
        assert!(member.is_recoverable());
        assert_eq!(member.label(), "viewer@example.com");
    }
}

// ---------------------------------------------------------------------------
// SponsorBlock
// ---------------------------------------------------------------------------

/// A community-submitted category of segment.
///
/// The names on the wire are SponsorBlock's own, and the colours are the ones
/// its extension uses - a viewer who already knows what a green bar means
/// should not have to relearn it here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SponsorCategory {
    Sponsor,
    #[serde(rename = "selfpromo")]
    SelfPromo,
    Interaction,
    Intro,
    Outro,
    Preview,
    #[serde(rename = "music_offtopic")]
    MusicOfftopic,
    Filler,
    #[serde(rename = "poi_highlight")]
    Highlight,
}

#[cfg_attr(not(feature = "server"), allow(dead_code))]
impl SponsorCategory {
    /// Every category Tawny asks the API for, in the order the settings list
    /// them.
    pub const ALL: [Self; 9] = [
        Self::Sponsor,
        Self::SelfPromo,
        Self::Interaction,
        Self::Intro,
        Self::Outro,
        Self::Preview,
        Self::MusicOfftopic,
        Self::Filler,
        Self::Highlight,
    ];

    pub fn api_name(self) -> &'static str {
        match self {
            Self::Sponsor => "sponsor",
            Self::SelfPromo => "selfpromo",
            Self::Interaction => "interaction",
            Self::Intro => "intro",
            Self::Outro => "outro",
            Self::Preview => "preview",
            Self::MusicOfftopic => "music_offtopic",
            Self::Filler => "filler",
            Self::Highlight => "poi_highlight",
        }
    }

    pub fn from_api_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|category| category.api_name() == name)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Sponsor => "Sponsor",
            Self::SelfPromo => "Unpaid self-promotion",
            Self::Interaction => "Interaction reminder",
            Self::Intro => "Intermission / intro",
            Self::Outro => "Endcards / credits",
            Self::Preview => "Preview / recap",
            Self::MusicOfftopic => "Non-music section",
            Self::Filler => "Filler tangent",
            Self::Highlight => "Highlight",
        }
    }

    /// What the category actually covers.
    ///
    /// The names alone do not settle it - "Preview" and "Filler" in particular
    /// are guessable in several directions, and choosing an action for one you
    /// have misread is how a viewer ends up skipping the video itself.
    pub fn description(self) -> &'static str {
        match self {
            Self::Sponsor => "Paid promotion, referral codes, and direct advertising.",
            Self::SelfPromo => {
                "Unpaid plugs for the creator's own merch, Patreon, or other channels."
            }
            Self::Interaction => "Brief reminders to like, subscribe, or comment.",
            Self::Intro => "Title cards and animated openers with no content in them.",
            Self::Outro => "Endcards and credits, where the video is effectively over.",
            Self::Preview => {
                "A recap of this video, or a run-through of what is coming later in it."
            }
            Self::MusicOfftopic => "The parts of a music video that are not the music.",
            Self::Filler => "Tangents and jokes the creator added that are not the subject.",
            Self::Highlight => "The moment the video is actually about. Jumped to, never over.",
        }
    }

    /// What a skip toast says once it has happened.
    pub fn skip_message(self) -> &'static str {
        match self {
            Self::Sponsor => "Skipped sponsor",
            Self::SelfPromo => "Skipped self-promotion",
            Self::Interaction => "Skipped interaction reminder",
            Self::Intro => "Skipped intro",
            Self::Outro => "Skipped endcards",
            Self::Preview => "Skipped recap",
            Self::MusicOfftopic => "Skipped non-music section",
            Self::Filler => "Skipped filler",
            Self::Highlight => "Jumped to the highlight",
        }
    }

    /// SponsorBlock's own palette, so the timeline reads the same as the
    /// extension's does.
    pub fn color(self) -> &'static str {
        match self {
            Self::Sponsor => "#00d400",
            Self::SelfPromo => "#ffff00",
            Self::Interaction => "#cc00ff",
            Self::Intro => "#00ffff",
            Self::Outro => "#0202ed",
            Self::Preview => "#008fd6",
            Self::MusicOfftopic => "#ff9900",
            Self::Filler => "#7300ff",
            Self::Highlight => "#ff1684",
        }
    }

    /// A highlight is a point, not a stretch: skipping *to* it is the whole
    /// action, so it is never skipped over and never auto-applied.
    pub fn is_point(self) -> bool {
        matches!(self, Self::Highlight)
    }

    /// What a fresh install does with each category.
    ///
    /// Matches the extension's defaults: the categories nobody wants are
    /// skipped, the ones that are sometimes content are offered, and the rest
    /// stay off until asked for.
    pub fn default_action(self) -> SponsorAction {
        match self {
            Self::Sponsor | Self::SelfPromo | Self::Interaction => SponsorAction::Skip,
            Self::Intro | Self::Outro | Self::Preview | Self::MusicOfftopic => SponsorAction::Show,
            Self::Filler | Self::Highlight => SponsorAction::Off,
        }
    }
}

/// What to do when playback reaches a segment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SponsorAction {
    /// Jump past it, and say so.
    Skip,
    /// Mark it on the timeline and offer a button, but do not move the
    /// playhead. The right default for anything that is sometimes the video.
    #[default]
    Show,
    /// Not fetched, not drawn, not skipped.
    Off,
}

impl SponsorAction {
    pub const ALL: [Self; 3] = [Self::Skip, Self::Show, Self::Off];

    pub fn label(self) -> &'static str {
        match self {
            Self::Skip => "Skip automatically",
            Self::Show => "Show on the timeline",
            Self::Off => "Ignore",
        }
    }

    pub fn trigger_label(self) -> &'static str {
        match self {
            Self::Skip => "Skip",
            Self::Show => "Show",
            Self::Off => "Off",
        }
    }
}

/// One segment of one video.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SponsorSegment {
    pub uuid: String,
    pub category: SponsorCategory,
    pub start_seconds: f64,
    pub end_seconds: f64,
    /// SponsorBlock's `locked` flag: a segment vetted by a moderator. Kept
    /// because it is the tie-breaker when two submissions overlap.
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub votes: i64,
}

/// Per-category behaviour, stored as a list rather than a map so it serialises
/// stably and an unknown category from a future build round-trips untouched.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SponsorCategorySetting {
    pub category: SponsorCategory,
    pub action: SponsorAction,
}

/// Everything the viewer controls about SponsorBlock.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SponsorBlockSettings {
    pub enabled: bool,
    /// Whether a skip announces itself. On by default: a video that silently
    /// jumps looks broken until you know why.
    pub notify_on_skip: bool,
    pub categories: Vec<SponsorCategorySetting>,
}

impl Default for SponsorBlockSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            notify_on_skip: true,
            categories: SponsorCategory::ALL
                .into_iter()
                .map(|category| SponsorCategorySetting {
                    category,
                    action: category.default_action(),
                })
                .collect(),
        }
    }
}

impl SponsorBlockSettings {
    pub fn action_for(&self, category: SponsorCategory) -> SponsorAction {
        if !self.enabled {
            return SponsorAction::Off;
        }
        self.categories
            .iter()
            .find(|setting| setting.category == category)
            .map(|setting| setting.action)
            // A category this build knows but the stored settings predate.
            .unwrap_or_else(|| category.default_action())
    }

    pub fn set_action(&mut self, category: SponsorCategory, action: SponsorAction) {
        match self
            .categories
            .iter_mut()
            .find(|setting| setting.category == category)
        {
            Some(setting) => setting.action = action,
            None => self
                .categories
                .push(SponsorCategorySetting { category, action }),
        }
    }

    /// The categories worth asking the API for. Requesting the ignored ones
    /// would mean fetching data only to discard it.
    pub fn requested_categories(&self) -> Vec<SponsorCategory> {
        SponsorCategory::ALL
            .into_iter()
            .filter(|category| self.action_for(*category) != SponsorAction::Off)
            .collect()
    }
}

#[cfg(test)]
mod sponsor_tests {
    use super::*;

    #[test]
    fn api_names_round_trip() {
        for category in SponsorCategory::ALL {
            assert_eq!(
                SponsorCategory::from_api_name(category.api_name()),
                Some(category),
                "{category:?}"
            );
        }
        assert_eq!(SponsorCategory::from_api_name("not_a_category"), None);
    }

    #[test]
    fn the_wire_format_matches_sponsorblocks_own_names() {
        // These strings go to sponsor.ajay.app and come back from it, so they
        // are a contract with someone else's API rather than an internal name.
        let encoded = serde_json::to_string(&SponsorCategory::MusicOfftopic).unwrap();
        assert_eq!(encoded, "\"music_offtopic\"");
        assert_eq!(
            serde_json::from_str::<SponsorCategory>("\"poi_highlight\"").unwrap(),
            SponsorCategory::Highlight
        );
        assert_eq!(
            serde_json::from_str::<SponsorCategory>("\"selfpromo\"").unwrap(),
            SponsorCategory::SelfPromo
        );
    }

    #[test]
    fn disabling_sponsorblock_turns_every_category_off() {
        let mut settings = SponsorBlockSettings::default();
        assert_eq!(
            settings.action_for(SponsorCategory::Sponsor),
            SponsorAction::Skip
        );
        settings.enabled = false;
        for category in SponsorCategory::ALL {
            assert_eq!(settings.action_for(category), SponsorAction::Off);
        }
        assert!(settings.requested_categories().is_empty());
    }

    #[test]
    fn only_the_categories_in_use_are_requested() {
        let mut settings = SponsorBlockSettings::default();
        settings.set_action(SponsorCategory::Sponsor, SponsorAction::Off);
        let requested = settings.requested_categories();
        assert!(!requested.contains(&SponsorCategory::Sponsor));
        assert!(requested.contains(&SponsorCategory::Intro));
    }

    #[test]
    fn a_category_missing_from_stored_settings_falls_back_to_its_default() {
        // Settings written by a build that predates a category must not make it
        // silently Off - that would look like SponsorBlock ignoring it.
        let settings = SponsorBlockSettings {
            enabled: true,
            notify_on_skip: true,
            categories: Vec::new(),
        };
        assert_eq!(
            settings.action_for(SponsorCategory::Sponsor),
            SponsorAction::Skip
        );
        assert_eq!(
            settings.action_for(SponsorCategory::Filler),
            SponsorAction::Off
        );
    }

    #[test]
    fn a_highlight_is_a_point_and_nothing_else_is() {
        for category in SponsorCategory::ALL {
            assert_eq!(
                category.is_point(),
                category == SponsorCategory::Highlight,
                "{category:?}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video(id: &str, title: &str, published_at: &str, duration_seconds: u64) -> Video {
        Video {
            id: id.into(),
            title: title.into(),
            channel_id: "channel".into(),
            channel_name: "Channel".into(),
            thumbnail_url: String::new(),
            published_at: published_at.into(),
            duration_seconds,
            view_count: String::new(),
            progress_seconds: 0,
            watched: false,
            is_live: false,
            is_short: false,
            audio_only: false,
            channel_avatar_url: None,
        }
    }

    fn ids(videos: &[Video]) -> Vec<&str> {
        videos.iter().map(|video| video.id.as_str()).collect()
    }

    fn sample() -> Vec<Video> {
        vec![
            video("b", "Beta", "2024-03-01T00:00:00Z", 600),
            video("a", "alpha", "2024-01-01T00:00:00Z", 90),
            video("c", "Gamma", "2024-02-01T00:00:00Z", 3600),
        ]
    }

    #[test]
    fn added_keeps_the_playlists_own_order() {
        let mut videos = sample();
        PlaylistSort::Added.apply(&mut videos, false);
        assert_eq!(ids(&videos), ["b", "a", "c"]);
    }

    #[test]
    fn added_reversed_walks_the_playlist_backwards() {
        let mut videos = sample();
        PlaylistSort::Added.apply(&mut videos, true);
        assert_eq!(ids(&videos), ["c", "a", "b"]);
    }

    #[test]
    fn published_sorts_newest_first_when_descending() {
        let mut videos = sample();
        PlaylistSort::Published.apply(&mut videos, true);
        assert_eq!(ids(&videos), ["b", "c", "a"]);
        let mut videos = sample();
        PlaylistSort::Published.apply(&mut videos, false);
        assert_eq!(ids(&videos), ["a", "c", "b"]);
    }

    /// Opening a video stored the watch page's "Feb 1, 2024", which used to
    /// read as 1970 and sink it to the end of Newest - leaving a run started on
    /// it with nothing after it and everything before it.
    #[test]
    fn a_written_out_date_keeps_its_place_in_newest() {
        let mut videos = sample();
        videos[2].published_at = "Feb 1, 2024".into();
        PlaylistSort::Published.apply(&mut videos, true);
        assert_eq!(ids(&videos), ["b", "c", "a"]);
        let view = PlaylistView {
            sort: PlaylistSort::Published,
            descending: true,
            ..PlaylistView::default()
        };
        assert_eq!(view.next_after(&videos, "c").as_deref(), Some("a"));
    }

    #[test]
    fn text_dates_read_through_youtube_prefixes() {
        let expected = time::Date::from_calendar_date(2013, time::Month::August, 2).ok();
        assert_eq!(text_date("Aug 2, 2013"), expected);
        assert_eq!(text_date("Premiered Aug 2, 2013"), expected);
        assert_eq!(text_date("Streamed live on August 2, 2013"), expected);
        assert_eq!(text_date("2 months ago"), None);
        assert_eq!(text_date("From YouTube"), None);
    }

    /// The feed's timestamp outranks the watch page's day, which outranks a
    /// related shelf's "3 years ago"; a finer date is never replaced by a coarser one.
    #[test]
    fn merging_details_keeps_the_finer_publish_date() {
        let feed = video("a", "A", "2024-02-01T15:30:00Z", 0);
        let mut detail = video("a", "A", "2024-02-01 0:00:00.0 +00:00:00", 0);
        detail.keep_finer_publish_date(&feed);
        assert_eq!(detail.published_at, "2024-02-01T15:30:00Z");

        let mut related = video("a", "A", "3 years ago", 0);
        related.keep_finer_publish_date(&video("a", "A", "Feb 1, 2024", 0));
        assert_eq!(related.published_at, "Feb 1, 2024");

        let mut fresh = video("a", "A", "2024-02-01T15:30:00Z", 0);
        fresh.keep_finer_publish_date(&video("a", "A", "3 years ago", 0));
        assert_eq!(fresh.published_at, "2024-02-01T15:30:00Z");
    }

    #[test]
    fn duration_sorts_longest_first_when_descending() {
        let mut videos = sample();
        PlaylistSort::Duration.apply(&mut videos, true);
        assert_eq!(ids(&videos), ["c", "b", "a"]);
    }

    /// Case is not order: "alpha" belongs before "Beta", not after "Gamma".
    #[test]
    fn title_ignores_case() {
        let mut videos = sample();
        PlaylistSort::Title.apply(&mut videos, false);
        assert_eq!(ids(&videos), ["a", "b", "c"]);
    }

    /// Every chip is reachable in both directions, and the two directions never
    /// read the same - the label is the only place the direction is shown.
    #[test]
    fn every_sort_labels_both_directions_distinctly() {
        for sort in PlaylistSort::ALL {
            assert_ne!(sort.label(true), sort.label(false), "{sort:?}");
        }
    }

    fn run_order(view: &PlaylistView, videos: Vec<Video>) -> Vec<Video> {
        let mut videos = videos;
        view.arrange(&mut videos, &AppSettings::default());
        videos
    }

    #[test]
    fn a_run_walks_the_arrangement_on_screen() {
        let view = PlaylistView {
            sort: PlaylistSort::Title,
            ..PlaylistView::default()
        };
        let order = run_order(&view, sample());
        assert_eq!(ids(&order), ["a", "b", "c"]);
        assert_eq!(view.next_after(&order, "a").as_deref(), Some("b"));
        assert_eq!(view.previous_before(&order, "b").as_deref(), Some("a"));
        assert_eq!(view.next_after(&order, "c"), None, "the last entry ends it");
        assert_eq!(
            view.previous_before(&order, "a"),
            None,
            "nothing before the first"
        );
    }

    /// The regression the split between `arrange` and `shows` exists to prevent.
    /// Finishing a video is what marks it watched, so an order that had already
    /// dropped watched videos could not say where the run was - and every advance
    /// would hand back the top of the playlist forever.
    #[test]
    fn finishing_a_video_does_not_send_an_unwatched_run_back_to_the_top() {
        let view = PlaylistView {
            only_unwatched: true,
            ..PlaylistView::default()
        };
        let mut videos = sample();
        videos[0].watched = true;
        let order = run_order(&view, videos);
        assert_eq!(ids(&order), ["b", "a", "c"], "playlist order, watched kept");
        assert_eq!(
            view.next_after(&order, "b").as_deref(),
            Some("a"),
            "advances past the video that just finished"
        );
    }

    #[test]
    fn a_run_skips_over_what_the_filters_hide() {
        let view = PlaylistView {
            only_unwatched: true,
            ..PlaylistView::default()
        };
        let mut videos = sample();
        videos[1].watched = true;
        let order = run_order(&view, videos);
        assert_eq!(view.next_after(&order, "b").as_deref(), Some("c"));
        assert_eq!(view.previous_before(&order, "c").as_deref(), Some("b"));
    }

    #[test]
    fn a_run_starts_at_the_top_for_a_video_it_does_not_hold() {
        let view = PlaylistView::default();
        let order = run_order(&view, sample());
        assert_eq!(view.next_after(&order, "unrelated").as_deref(), Some("b"));
        assert_eq!(view.previous_before(&order, "unrelated"), None);
    }

    #[test]
    fn a_kind_filter_narrows_what_the_run_can_reach() {
        let mut videos = sample();
        videos[1].is_short = true;
        let shorts = PlaylistView {
            kind: PlaylistKind::Shorts,
            ..PlaylistView::default()
        };
        assert_eq!(ids(&run_order(&shorts, videos.clone())), ["a"]);
        let long_form = PlaylistView {
            kind: PlaylistKind::Videos,
            ..PlaylistView::default()
        };
        assert_eq!(ids(&run_order(&long_form, videos)), ["b", "c"]);
    }

    #[test]
    fn a_default_view_hides_nothing() {
        assert!(!PlaylistView::default().is_filtered());
    }

    #[test]
    fn a_queue_marker_is_told_apart_from_a_video_id() {
        let entry = playlist_queue_entry("watch-later");
        assert_eq!(queued_playlist_id(&entry), Some("watch-later"));
        assert_eq!(
            queued_playlist_id("dQw4w9WgXcQ"),
            None,
            "a video id is never mistaken for a run"
        );
    }
}
