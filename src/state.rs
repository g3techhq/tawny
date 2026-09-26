use crate::subscriptions_io::{ImportSummary, ParsedImport};
use crate::{
    api::{get_library, push_library_state, sync_library},
    cache::use_persistent_signal,
    models::{
        AppSettings, AudioTrackOption, CaptionTrack, Channel, ChannelDetails, FeedFilter,
        HistoryEntry, LibrarySnapshot, LibraryUserState, Playlist, PlaylistView, SearchResults,
        SponsorSegment, SubscriptionContent, SubscriptionGroup, Video, VideoChapter, VideoDetails,
        VideoPreviewFrames, playlist_queue_entry, queued_playlist_id,
    },
    session::use_session_provider,
};
use dioxus::prelude::*;
use g3_ui::{Color, ToastOptions, Toaster};

/// How close to the end counts as finished. YouTube stops short of the exact
/// duration often enough that an exact match would rarely fire.
const PROGRESS_WATCHED_TAIL_SECONDS: u64 = 15;

/// What saving a video to a playlist did. A swipe that lands on a video the
/// playlist already holds has changed nothing, and saying so in the same
/// green as a save reads as though it saved again.
pub enum PlaylistSave {
    Saved(String),
    AlreadyThere(String),
}

/// A snapshot of the playlist run, resolved the same way autoplay resolves it.
pub struct PlaylistRunStatus {
    pub playlist_id: String,
    pub name: String,
    /// The arrangement the run is walking right now. A run reads the playlist's
    /// live view each time it advances, so this is also what it will use next.
    pub view: PlaylistView,
    /// Entries in the arranged order, watched filter aside.
    pub total: usize,
    /// 1-based position of the playing video, when the run contains it.
    pub position: Option<usize>,
    pub current: Option<Video>,
    /// Entries still to play after the current one.
    pub remaining: usize,
    pub up_next: Option<Video>,
}

#[derive(Clone, Copy)]
pub struct AppState {
    pub library: Signal<LibrarySnapshot>,
    pub settings: Signal<AppSettings>,
    /// g3-ui's toast queue. It belongs to the `AppWrapper`, which sits below
    /// this state, so a component inside the wrapper hands it over once it
    /// mounts. Until then a notice has nowhere to show and is dropped.
    pub toaster: Signal<Option<Toaster>>,
    pub playlist_picker_video: Signal<Option<Video>>,
    pub playlist_picker_open: Signal<bool>,
    pub video_actions_video: Signal<Option<Video>>,
    /// The playlist the actions sheet was opened from, so it can offer to
    /// take the video out of it.
    pub video_actions_playlist: Signal<Option<String>>,
    pub video_actions_open: Signal<bool>,
    pub share_video: Signal<Option<Video>>,
    pub share_open: Signal<bool>,
    pub share_with_timestamp: Signal<bool>,
    pub share_timestamp_seconds: Signal<u64>,
    pub active_video: Signal<Option<Video>>,
    pub active_captions: Signal<Vec<CaptionTrack>>,
    pub selected_caption: Signal<Option<usize>>,
    pub captions_enabled: Signal<bool>,
    pub active_chapters: Signal<Vec<VideoChapter>>,
    /// Fetched separately from the rest of the metadata: which categories to ask
    /// for depends on viewer settings, so it cannot ride along with the details.
    pub active_sponsor_segments: Signal<Vec<SponsorSegment>>,
    /// Audio languages the transport can switch between, and which one is live.
    /// Empty for the ordinary single-language upload, which is why the chip that
    /// reads these only appears when there is a choice to make.
    pub active_audio_tracks: Signal<Vec<AudioTrackOption>>,
    pub selected_audio_track: Signal<Option<String>>,
    pub active_preview_frames: Signal<Option<VideoPreviewFrames>>,
    pub syncing: Signal<bool>,
    /// Segment selections live here because the header owns the segmented
    /// control while the page owns the list it filters.
    pub chapters_sheet_open: Signal<bool>,
    /// Videos this sitting has advanced away from, most recent last.
    ///
    /// Deliberately not in the library: it is what "previous" means during one
    /// run of the queue, not something worth carrying to another device or
    /// remembering tomorrow. History cannot answer it - playing a video moves it
    /// to the front of history, so stepping back through history immediately
    /// starts bouncing between two videos.
    pub run_back_stack: Signal<Vec<String>>,
    pub feed_filter: Signal<FeedFilter>,
    pub explore_filter: Signal<crate::models::ExploreFilter>,
    pub channel_tab: Signal<FeedFilter>,
}

impl AppState {
    pub fn library(self) -> LibrarySnapshot {
        (self.library)()
    }

    /// Read the library without copying it.
    ///
    /// [`Self::library`] clones the whole snapshot, which is tens of thousands
    /// of videos and channels once a cache has filled up. That is fine for a
    /// caller that needs to own the data, but ruinous on a render path: every
    /// video card was cloning the entire library three times just to look up a
    /// playlist name and an avatar, which stalled the swipe release animation.
    /// Subscribes to the signal, so it stays reactive.
    pub fn with_library<T>(self, read: impl FnOnce(&LibrarySnapshot) -> T) -> T {
        read(&(self.library).read())
    }

    pub fn settings(self) -> AppSettings {
        (self.settings)()
    }

    pub fn playlist_picker_video(self) -> Option<Video> {
        (self.playlist_picker_video)()
    }

    pub fn syncing(self) -> bool {
        (self.syncing)()
    }

    pub fn active_video(self) -> Option<Video> {
        (self.active_video)()
    }

    pub fn play(mut self, video: Video) {
        if self
            .active_video()
            .as_ref()
            .is_none_or(|active| active.id != video.id)
        {
            // The outgoing video keeps whatever position its last tick recorded.
            // Locally that is already correct; this is what stops the server
            // from being up to half a minute behind when a video is swapped.
            self.flush_progress();
            self.active_captions.set(Vec::new());
            self.selected_caption.set(None);
            self.captions_enabled.set(false);
            self.active_chapters.set(Vec::new());
            self.active_sponsor_segments.set(Vec::new());
            self.active_preview_frames.set(None);
            self.active_audio_tracks.set(Vec::new());
            self.selected_audio_track.set(None);
        }
        self.active_video.set(Some(video));
    }

    /// Fill in what is known about the video that is playing.
    ///
    /// Only ever adds. The same player is handed two descriptions of a video:
    /// the copy cached in the library, which carries no chapters, captions or
    /// preview frames because the library does not store them, and the one the
    /// details request returns, which carries all three. They arrive in either
    /// order and more than once, so applying an empty field on top of a full
    /// one would strip the timeline of its divisions and its segment colours
    /// part way through a video. Changing video is what clears this - see
    /// [`Self::play`] and [`Self::stop_playback`].
    pub fn set_player_metadata(
        mut self,
        captions: Vec<CaptionTrack>,
        chapters: Vec<VideoChapter>,
        preview_frames: Option<VideoPreviewFrames>,
    ) {
        if !captions.is_empty() {
            if (self.selected_caption)().is_none() {
                self.selected_caption.set(Some(0));
            }
            self.active_captions.set(captions);
        }
        if !chapters.is_empty() {
            self.active_chapters.set(chapters);
        }
        if preview_frames.is_some() {
            self.active_preview_frames.set(preview_frames);
        }
    }

    pub fn stop_playback(mut self) {
        self.flush_progress();
        self.active_video.set(None);
        self.active_captions.set(Vec::new());
        self.selected_caption.set(None);
        self.captions_enabled.set(false);
        self.active_chapters.set(Vec::new());
        self.active_sponsor_segments.set(Vec::new());
        self.active_preview_frames.set(None);
        self.active_audio_tracks.set(Vec::new());
        self.selected_audio_track.set(None);
    }

    fn sync_in_background(self) {
        // Only the client-owned half. The reply is just the accepted revision,
        // so this costs kilobytes rather than the whole cache in both
        // directions - see `LibraryUserState`. Built from a borrow: cloning the
        // snapshot first made every edit copy the entire cache before sending a
        // few kilobytes of it.
        let user_state = LibraryUserState::from(&*self.library.peek());
        spawn(async move {
            let _ = push_library_state(user_state).await;
        });
    }

    /// Remember where playback has reached.
    ///
    /// Called on a timer while a video plays, so it deliberately does not push
    /// to the server on every tick - `flush_progress` does that at the points
    /// where losing the position would actually matter. A whole second of
    /// change is the smallest step worth a re-render.
    pub fn record_progress(mut self, video_id: &str, seconds: u64) -> bool {
        // Checked through a peek first. Taking the write guard notifies every
        // subscriber when it drops, changed or not - and the library's
        // subscribers include the effect that persists the whole cache.
        let unchanged = self
            .library
            .peek()
            .videos
            .iter()
            .find(|video| video.id == video_id)
            .is_none_or(|video| video.progress_seconds == seconds);
        if unchanged {
            return false;
        }
        {
            let mut library = self.library.write();
            let Some(video) = library.videos.iter_mut().find(|video| video.id == video_id) else {
                return false;
            };
            if video.progress_seconds == seconds {
                return false;
            }
            video.progress_seconds = seconds;
            // Treated as watched once it is effectively over, which is what the
            // card's progress bar and the "hide watched" filter both key on.
            if video.duration_seconds > 0
                && seconds + PROGRESS_WATCHED_TAIL_SECONDS >= video.duration_seconds
            {
                video.watched = true;
            }
            library.cache_revision += 1;
        }
        true
    }

    /// Push the remembered positions to the server.
    ///
    /// Separate from [`Self::record_progress`] so the timer can run often while
    /// the network call stays rare: on pause, on leaving the video, and on a
    /// slower interval than the tick itself.
    pub fn flush_progress(self) {
        self.sync_in_background();
    }

    /// Play this video without its video stream, and remember that choice for
    /// this video specifically.
    pub fn set_audio_only(mut self, video_id: &str, audio_only: bool) -> bool {
        {
            let mut library = self.library.write();
            let Some(video) = library.videos.iter_mut().find(|video| video.id == video_id) else {
                return false;
            };
            if video.audio_only == audio_only {
                return false;
            }
            video.audio_only = audio_only;
            library.cache_revision += 1;
        }
        // Also mirror onto the playing copy so the player re-reads it without
        // waiting for a library round trip.
        if let Some(active) = (self.active_video)()
            && active.id == video_id
        {
            let mut updated = active;
            updated.audio_only = audio_only;
            self.active_video.set(Some(updated));
        }
        self.sync_in_background();
        true
    }

    pub fn show_toast(self, message: impl Into<String>, color: Color) {
        if let Some(toaster) = *self.toaster.peek() {
            // Most notices confirm an action that can be repeated at once -
            // a swipe, a toggle - so the latest replaces rather than queues.
            toaster.show(ToastOptions::new(message).color(color).replace());
        }
    }

    pub fn add_to_playlist(mut self, video_id: &str, playlist_id: &str) -> Option<PlaylistSave> {
        let mut library = self.library.write();
        let playlist_name = {
            let playlist = library
                .playlists
                .iter_mut()
                .find(|playlist| playlist.id == playlist_id)?;
            if playlist.video_ids.iter().any(|id| id == video_id) {
                return Some(PlaylistSave::AlreadyThere(playlist.name.clone()));
            }
            playlist.video_ids.insert(0, video_id.to_string());
            playlist.name.clone()
        };
        library.cache_revision += 1;
        drop(library);
        self.sync_in_background();
        Some(PlaylistSave::Saved(playlist_name))
    }

    pub fn create_playlist(mut self, name: String) -> String {
        let id = format!("local-{}", self.library.peek().cache_revision + 1);
        {
            let mut library = self.library.write();
            library.playlists.push(Playlist {
                id: id.clone(),
                name,
                video_ids: Vec::new(),
            });
            library.cache_revision += 1;
        }
        self.sync_in_background();
        id
    }

    pub fn delete_playlist(mut self, playlist_id: &str) -> Option<String> {
        let mut library = self.library.write();
        let index = library
            .playlists
            .iter()
            .position(|playlist| playlist.id == playlist_id)?;
        let removed = library.playlists.remove(index);
        // A marker for a deleted playlist would resolve to nothing and be swept
        // up on the next advance anyway, but leaving it there means the queue
        // page lists a playlist that no longer exists.
        library
            .queue
            .retain(|entry| queued_playlist_id(entry) != Some(playlist_id));
        library.cache_revision += 1;
        drop(library);
        let mut settings = self.settings.write();
        if settings.swipe_right_playlist_id == playlist_id {
            settings.swipe_right_playlist_id = "watch-later".into();
        }
        if settings.swipe_left_playlist_id == playlist_id {
            settings.swipe_left_playlist_id = "deep-dives".into();
        }
        settings.playlist_views.remove(playlist_id);
        drop(settings);
        self.sync_in_background();
        Some(removed.name)
    }

    pub fn remove_from_playlist(mut self, video_id: &str, playlist_id: &str) -> bool {
        let mut library = self.library.write();
        let Some(playlist) = library
            .playlists
            .iter_mut()
            .find(|playlist| playlist.id == playlist_id)
        else {
            return false;
        };
        let before = playlist.video_ids.len();
        playlist.video_ids.retain(|id| id != video_id);
        if playlist.video_ids.len() == before {
            return false;
        }
        library.cache_revision += 1;
        drop(library);
        self.sync_in_background();
        true
    }

    /// Drop every already-watched video from a playlist, returning how many
    /// were removed.
    pub fn remove_watched_from_playlist(mut self, playlist_id: &str) -> usize {
        let mut library = self.library.write();
        let watched_ids = library
            .videos
            .iter()
            .filter(|video| video.watched)
            .map(|video| video.id.clone())
            .collect::<Vec<_>>();
        let Some(playlist) = library
            .playlists
            .iter_mut()
            .find(|playlist| playlist.id == playlist_id)
        else {
            return 0;
        };
        let before = playlist.video_ids.len();
        playlist.video_ids.retain(|id| !watched_ids.contains(id));
        let removed = before - playlist.video_ids.len();
        if removed == 0 {
            return 0;
        }
        library.cache_revision += 1;
        drop(library);
        self.sync_in_background();
        removed
    }

    pub fn mark_watched(mut self, video_id: &str, watched: bool) {
        let mut library = self.library.write();
        let changed =
            if let Some(video) = library.videos.iter_mut().find(|video| video.id == video_id) {
                video.watched = watched;
                video.progress_seconds = if watched { video.duration_seconds } else { 0 };
                true
            } else {
                false
            };
        if changed {
            library.cache_revision += 1;
            drop(library);
            self.sync_in_background();
        }
    }

    pub fn toggle_subscription(mut self, channel_id: &str) -> Option<bool> {
        let mut library = self.library.write();
        let channel = library
            .channels
            .iter_mut()
            .find(|channel| channel.id == channel_id)?;
        channel.subscribed = !channel.subscribed;
        let subscribed = channel.subscribed;
        library.cache_revision += 1;
        drop(library);
        self.sync_in_background();
        Some(subscribed)
    }

    /// [`Self::toggle_subscription`] for a channel the library may not hold
    /// yet - one met through a video opened from search or a related list.
    /// It is added unfollowed first, so the toggle always has something to
    /// flip rather than the button silently doing nothing.
    pub fn toggle_subscription_for(mut self, channel: &Channel) -> bool {
        let known = self
            .library
            .peek()
            .channels
            .iter()
            .any(|known| known.id == channel.id);
        if !known {
            self.library.write().channels.push(Channel {
                subscribed: false,
                ..channel.clone()
            });
        }
        self.toggle_subscription(&channel.id).unwrap_or(false)
    }

    /// Narrow (or widen) which of a channel's uploads reach the feed. Leaves
    /// the subscription itself alone: this is a filter, not an unsubscribe.
    pub fn set_subscription_content(
        mut self,
        channel_id: &str,
        content: SubscriptionContent,
    ) -> bool {
        let mut library = self.library.write();
        let Some(channel) = library
            .channels
            .iter_mut()
            .find(|channel| channel.id == channel_id)
        else {
            return false;
        };
        if channel.subscription_content == content {
            return false;
        }
        channel.subscription_content = content;
        library.cache_revision += 1;
        drop(library);
        self.sync_in_background();
        true
    }

    /// Apply a parsed subscription export to the library.
    ///
    /// Channels already known are subscribed in place so their cached metadata
    /// and videos survive. Unknown channels are written as stubs carrying only
    /// the id and name from the export: the next sync fills in the avatar,
    /// handle, and uploads, which is the same path a freshly discovered channel
    /// takes.
    pub fn import_subscriptions(mut self, parsed: ParsedImport) -> ImportSummary {
        let mut summary = ImportSummary::default();
        let mut library = self.library.write();

        for entry in parsed.subscriptions {
            // Handle-only records cannot be matched or fetched offline. They
            // are counted rather than silently dropped so the toast can say so.
            if entry.channel_id.is_empty() {
                summary.unresolved += 1;
                continue;
            }
            match library
                .channels
                .iter_mut()
                .find(|channel| channel.id == entry.channel_id)
            {
                Some(channel) => {
                    let was_subscribed = channel.subscribed;
                    channel.subscribed = true;
                    channel.subscription_content = entry.content;
                    if channel.name.trim().is_empty() && !entry.name.is_empty() {
                        channel.name = entry.name;
                    }
                    if was_subscribed {
                        summary.already_subscribed += 1;
                    } else {
                        summary.subscribed += 1;
                    }
                }
                None => {
                    library.channels.push(Channel {
                        id: entry.channel_id,
                        name: entry.name,
                        handle: entry.handle,
                        avatar_url: None,
                        subscriber_count: String::new(),
                        subscribed: true,
                        description: String::new(),
                        banner_url: None,
                        subscription_content: entry.content,
                    });
                    summary.subscribed += 1;
                }
            }
        }

        // Groups only ever arrive from Tawny's own export. Matching on name
        // keeps a re-import from stacking duplicates of the same group.
        for group in parsed.groups {
            match library
                .subscription_groups
                .iter_mut()
                .find(|existing| existing.name.eq_ignore_ascii_case(&group.name))
            {
                Some(existing) => {
                    for channel_id in group.channel_ids {
                        if !existing.channel_ids.contains(&channel_id) {
                            existing.channel_ids.push(channel_id);
                        }
                    }
                }
                None => {
                    summary.groups += 1;
                    library.subscription_groups.push(group);
                }
            }
        }

        if summary.changed() {
            library.cache_revision += 1;
            drop(library);
            self.sync_in_background();
        }
        summary
    }

    pub fn create_subscription_group(mut self, name: String) -> String {
        let slug = name
            .to_lowercase()
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() {
                    character
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .trim_matches('-')
            .to_string();
        let revision = self.library.peek().cache_revision + 1;
        let id = if slug.is_empty() {
            format!("group-{revision}")
        } else {
            format!("{slug}-{revision}")
        };
        {
            let mut library = self.library.write();
            library.subscription_groups.push(SubscriptionGroup {
                id: id.clone(),
                name,
                channel_ids: Vec::new(),
            });
            library.cache_revision += 1;
        }
        self.sync_in_background();
        id
    }

    pub fn delete_subscription_group(mut self, group_id: &str) -> Option<String> {
        let mut library = self.library.write();
        let index = library
            .subscription_groups
            .iter()
            .position(|group| group.id == group_id)?;
        let group = library.subscription_groups.remove(index);
        library.cache_revision += 1;
        drop(library);
        self.sync_in_background();
        Some(group.name)
    }

    pub fn toggle_channel_in_group(mut self, group_id: &str, channel_id: &str) -> Option<bool> {
        let mut library = self.library.write();
        let group = library
            .subscription_groups
            .iter_mut()
            .find(|group| group.id == group_id)?;
        let included = if group.channel_ids.iter().any(|id| id == channel_id) {
            group.channel_ids.retain(|id| id != channel_id);
            false
        } else {
            group.channel_ids.push(channel_id.to_string());
            true
        };
        library.cache_revision += 1;
        drop(library);
        self.sync_in_background();
        Some(included)
    }

    pub fn ingest_search_results(mut self, results: &SearchResults) {
        let mut library = self.library.write();
        let mut changed = false;

        for discovered in &results.channels {
            if let Some(channel) = library
                .channels
                .iter_mut()
                .find(|channel| channel.id == discovered.id)
            {
                changed |= channel.merge_metadata_from(discovered);
            } else {
                library.channels.push(discovered.clone());
                changed = true;
            }
        }

        for discovered in &results.videos {
            if !library
                .channels
                .iter()
                .any(|channel| channel.id == discovered.channel_id)
            {
                let handle = format!(
                    "@{}",
                    discovered
                        .channel_name
                        .to_lowercase()
                        .chars()
                        .filter(|character| character.is_ascii_alphanumeric())
                        .collect::<String>()
                );
                library.channels.push(Channel {
                    id: discovered.channel_id.clone(),
                    name: discovered.channel_name.clone(),
                    handle,
                    avatar_url: None,
                    subscriber_count: String::new(),
                    subscribed: false,
                    description: String::new(),
                    banner_url: None,
                    subscription_content: SubscriptionContent::All,
                });
                changed = true;
            }
            if !library.videos.iter().any(|video| video.id == discovered.id) {
                library.videos.push(discovered.clone());
                changed = true;
            }
        }

        if changed {
            library.cache_revision += 1;
            drop(library);
            self.sync_in_background();
        }
    }

    pub fn cache_video_details(mut self, details: &VideoDetails) {
        let mut library = self.library.write();
        if let Some(discovered) = &details.channel {
            if let Some(channel) = library
                .channels
                .iter_mut()
                .find(|channel| channel.id == discovered.id)
            {
                channel.merge_metadata_from(discovered);
            } else {
                library.channels.push(discovered.clone());
            }
        }

        let mut discovered_videos = vec![details.video.clone()];
        discovered_videos.extend(details.related_videos.clone());
        for discovered in discovered_videos {
            if let Some(video) = library
                .videos
                .iter_mut()
                .find(|video| video.id == discovered.id)
            {
                let previous = std::mem::replace(video, discovered);
                video.progress_seconds = previous.progress_seconds;
                video.watched = previous.watched;
                video.keep_finer_publish_date(&previous);
            } else {
                library.videos.push(discovered);
            }
        }
    }

    /// Merge server-resolved feed metadata without disturbing local playback
    /// progress. Duration, live state, and Shorts classification are catalog
    /// facts; watched/audio state belongs to this viewer.
    pub fn cache_feed_metadata(mut self, discovered_videos: Vec<Video>) {
        let mut library = self.library.write();
        for discovered in discovered_videos {
            let Some(video) = library
                .videos
                .iter_mut()
                .find(|video| video.id == discovered.id)
            else {
                library.videos.push(discovered);
                continue;
            };
            let previous = std::mem::replace(video, discovered);
            video.progress_seconds = previous.progress_seconds;
            video.watched = previous.watched;
            video.audio_only = previous.audio_only;
            video.keep_finer_publish_date(&previous);
        }
    }

    pub fn cache_channel_details(mut self, details: &ChannelDetails) {
        let mut library = self.library.write();
        if let Some(channel) = library
            .channels
            .iter_mut()
            .find(|channel| channel.id == details.channel.id)
        {
            channel.merge_metadata_from(&details.channel);
        } else {
            library.channels.push(details.channel.clone());
        }

        for discovered in details
            .videos
            .videos
            .iter()
            .chain(details.shorts.videos.iter())
            .chain(details.live.videos.iter())
        {
            if let Some(video) = library
                .videos
                .iter_mut()
                .find(|video| video.id == discovered.id)
            {
                let progress_seconds = video.progress_seconds;
                let watched = video.watched;
                *video = discovered.clone();
                video.progress_seconds = progress_seconds;
                video.watched = watched;
            } else {
                library.videos.push(discovered.clone());
            }
        }
    }

    /// Replace the library with a server snapshot without losing videos that
    /// something still points at.
    ///
    /// The queue, playlists, and history store ids and resolve them against
    /// `videos`. A refreshed snapshot only carries the current feed, so a video
    /// that scrolled out of it would vanish from the queue even though its id is
    /// still queued. Carrying those records over keeps the queue honest.
    pub fn adopt_library(mut self, remote: LibrarySnapshot) {
        let merged = keep_referenced_videos(&self.library.peek(), remote);
        self.library.set(merged);
    }

    /// Run whichever action a swipe direction is configured for.
    ///
    /// Centralised so the gesture, the desktop buttons, and the settings page
    /// all agree on what a direction means.
    pub fn run_swipe_action(self, video_id: &str, start_side: bool) {
        use crate::models::SwipeActionKind;

        let settings = self.settings();
        let (kind, playlist_id) = if start_side {
            (
                settings.swipe_right_action,
                settings.swipe_right_playlist_id,
            )
        } else {
            (settings.swipe_left_action, settings.swipe_left_playlist_id)
        };
        match kind {
            SwipeActionKind::AddToPlaylist => match self.add_to_playlist(video_id, &playlist_id) {
                Some(PlaylistSave::Saved(name)) => {
                    self.show_toast(format!("Added to {name}"), Color::Success)
                }
                Some(PlaylistSave::AlreadyThere(name)) => {
                    self.show_toast(format!("Already in {name}"), Color::Warning)
                }
                None => {}
            },
            SwipeActionKind::AddToQueue => {
                let message = self.add_to_queue(video_id, false);
                self.show_toast(message, Color::Success);
            }
            SwipeActionKind::PlayNext => {
                let message = self.add_to_queue(video_id, true);
                self.show_toast(message, Color::Success);
            }
            SwipeActionKind::MarkWatched => {
                self.mark_watched(video_id, true);
                self.show_toast("Marked watched", Color::Neutral);
            }
            SwipeActionKind::Share => {
                if let Some(video) = self
                    .library()
                    .videos
                    .into_iter()
                    .find(|video| video.id == video_id)
                {
                    self.open_share(video);
                }
            }
        }
    }

    pub fn open_share(mut self, video: Video) {
        self.share_timestamp_seconds.set(0);
        self.share_with_timestamp.set(false);
        self.share_video.set(Some(video.clone()));
        self.share_open.set(true);

        if self
            .active_video()
            .as_ref()
            .is_some_and(|active| active.id == video.id)
        {
            spawn(async move {
                let mut eval = document::eval(
                    "dioxus.send(Math.max(0, Math.floor(document.querySelector('#tawny-player video')?.currentTime || 0)));",
                );
                if let Ok(seconds) = eval.recv::<u64>().await {
                    self.share_timestamp_seconds.set(seconds);
                }
            });
        }
    }

    /// The label a swipe direction should show on its action panel.
    /// Which action a swipe on this side is set to, so a card can show the
    /// icon for what it will actually do.
    pub fn swipe_action_kind(self, start_side: bool) -> crate::models::SwipeActionKind {
        let settings = self.settings();
        if start_side {
            settings.swipe_right_action
        } else {
            settings.swipe_left_action
        }
    }

    /// The few words a swipe action wears under its icon.
    ///
    /// A swipe reveals about a thumb's travel, which is no room for a
    /// sentence. A playlist action carries the playlist's own name, since
    /// that is the part that differs between the two directions; the rest
    /// name themselves in a word. The full phrase stays in
    /// [`Self::swipe_action_label`], which is what a screen reader hears.
    pub fn swipe_action_caption(self, start_side: bool) -> String {
        use crate::models::SwipeActionKind;

        let settings = self.settings();
        let (kind, playlist_id) = if start_side {
            (
                settings.swipe_right_action,
                settings.swipe_right_playlist_id,
            )
        } else {
            (settings.swipe_left_action, settings.swipe_left_playlist_id)
        };
        match kind {
            SwipeActionKind::AddToPlaylist => self.with_library(|library| {
                library
                    .playlists
                    .iter()
                    .find(|playlist| playlist.id == playlist_id)
                    .map(|playlist| playlist.name.clone())
                    .unwrap_or_else(|| SwipeActionKind::AddToPlaylist.trigger_label().to_string())
            }),
            other => other.trigger_label().to_string(),
        }
    }

    /// What a swipe on this side says it will do, named in full: a playlist
    /// action carries the playlist's own name, and the rest speak for
    /// themselves. This is the accessible name; the card shows the shorter
    /// [`Self::swipe_action_caption`].
    pub fn swipe_action_label(self, start_side: bool) -> String {
        use crate::models::SwipeActionKind;

        let settings = self.settings();
        let (kind, playlist_id) = if start_side {
            (
                settings.swipe_right_action,
                settings.swipe_right_playlist_id,
            )
        } else {
            (settings.swipe_left_action, settings.swipe_left_playlist_id)
        };
        match kind {
            SwipeActionKind::AddToPlaylist => self.with_library(|library| {
                let name = library
                    .playlists
                    .iter()
                    .find(|playlist| playlist.id == playlist_id)
                    .map(|playlist| playlist.name.clone());
                match name {
                    Some(name) => format!("Add to {name}"),
                    None => SwipeActionKind::AddToPlaylist.label().to_string(),
                }
            }),
            other => other.label().to_string(),
        }
    }

    /// How this playlist is currently arranged, defaults included.
    pub fn playlist_view(self, playlist_id: &str) -> PlaylistView {
        self.settings
            .read()
            .playlist_views
            .get(playlist_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Remember how a playlist is arranged, so a run resolved later walks the
    /// list the viewer was actually looking at.
    ///
    /// A view holding nothing but defaults is removed rather than stored: the map
    /// is keyed by playlist id and would otherwise accumulate an entry for every
    /// playlist ever opened.
    pub fn set_playlist_view(mut self, playlist_id: &str, view: PlaylistView) {
        let mut settings = self.settings.write();
        if view == PlaylistView::default() {
            settings.playlist_views.remove(playlist_id);
        } else {
            settings
                .playlist_views
                .insert(playlist_id.to_string(), view);
        }
    }

    /// A playlist's videos in the order its page is showing them, with the
    /// watched filter *not* applied - see [`PlaylistView::arrange`].
    pub fn playlist_run_order(self, playlist_id: &str) -> Vec<Video> {
        let settings = self.settings();
        let view = self.playlist_view(playlist_id);
        let mut videos = self.with_library(|library| {
            let Some(playlist) = library
                .playlists
                .iter()
                .find(|playlist| playlist.id == playlist_id)
            else {
                return Vec::new();
            };
            let by_id = library
                .videos
                .iter()
                .map(|video| (video.id.as_str(), video))
                .collect::<std::collections::HashMap<_, _>>();
            playlist
                .video_ids
                .iter()
                .filter_map(|id| by_id.get(id.as_str()).map(|video| (*video).clone()))
                .collect::<Vec<Video>>()
        });
        view.arrange(&mut videos, &settings);
        videos
    }

    /// The entry a playlist run plays after `current_video_id`.
    ///
    /// Position is found in the arranged order, which still contains the current
    /// video even once it counts as watched; the watched filter is applied only to
    /// the candidates ahead of it. See [`PlaylistView::next_after`].
    fn next_in_playlist_run(self, playlist_id: &str, current_video_id: &str) -> Option<String> {
        self.playlist_view(playlist_id)
            .next_after(&self.playlist_run_order(playlist_id), current_video_id)
    }

    /// The entry a playlist run plays before `current_video_id`.
    fn previous_in_playlist_run(self, playlist_id: &str, current_video_id: &str) -> Option<String> {
        self.playlist_view(playlist_id)
            .previous_before(&self.playlist_run_order(playlist_id), current_video_id)
    }

    /// What the run would hand over next, plus the playlist markers it walked
    /// past on the way.
    ///
    /// Resolved in queue order, so a video queued by hand still plays before a
    /// playlist sitting behind it. A marker the run has reached the end of - or
    /// one whose playlist has been deleted - resolves to nothing and is reported
    /// as spent so the caller can drop it.
    fn resolve_next_in_run(self, finished_video_id: &str) -> (Option<String>, Vec<String>) {
        let queue = self.with_library(|library| library.queue.clone());
        let mut spent_markers = Vec::new();
        for entry in queue {
            match queued_playlist_id(&entry) {
                Some(playlist_id) => {
                    match self.next_in_playlist_run(playlist_id, finished_video_id) {
                        Some(video_id) => return (Some(video_id), spent_markers),
                        None => spent_markers.push(entry),
                    }
                }
                None if entry != finished_video_id => return (Some(entry), spent_markers),
                None => {}
            }
        }
        (None, spent_markers)
    }

    /// What the run would play after `current_video_id`, without consuming it.
    ///
    /// Pass an empty id to ask what the run starts with.
    pub fn next_in_run(self, current_video_id: &str) -> Option<String> {
        self.resolve_next_in_run(current_video_id).0
    }

    /// Whether there is anywhere forward to go from this video.
    pub fn has_next_in_run(self, current_video_id: &str) -> bool {
        self.next_in_run(current_video_id).is_some()
    }

    /// Take the next video in the run, skipping the one that just finished, and
    /// remember the one being left so [`Self::step_back_in_run`] can return to it.
    ///
    /// Removing the video as it is handed over is what stops autoplay looping: an
    /// entry that stays queued would be chosen again the moment it ends. A
    /// playlist marker is the exception - it is the run, not a place in it, so it
    /// stays until the playlist is spent.
    pub fn take_next_queued(mut self, finished_video_id: &str) -> Option<String> {
        let (next, spent_markers) = self.resolve_next_in_run(finished_video_id);
        let video_id = next?;
        let mut library = self.library.write();
        // The finished video is done either way, whether or not it was queued,
        // and the one taken never stays behind as a duplicate further down. A
        // playlist marker is neither, so a run keeps its place in the queue.
        library.queue.retain(|entry| {
            entry != finished_video_id && entry != &video_id && !spent_markers.contains(entry)
        });
        library.cache_revision += 1;
        drop(library);
        self.push_back_stack(finished_video_id);
        self.sync_in_background();
        Some(video_id)
    }

    /// Whether there is a video to go back to without leaving the run.
    ///
    /// Either somewhere this run has already been, or - on the first video of a
    /// playlist run, where the stack is still empty - the entry before it in the
    /// playlist's own order.
    pub fn can_step_back_in_run(self, current_video_id: &str) -> bool {
        if !(self.run_back_stack)().is_empty() {
            return true;
        }
        self.queued_playlist_run()
            .and_then(|playlist_id| self.previous_in_playlist_run(&playlist_id, current_video_id))
            .is_some()
    }

    /// The playlist the queue is running, as (id, name), when it still exists.
    pub fn running_playlist(self) -> Option<(String, String)> {
        let playlist_id = self.queued_playlist_run()?;
        self.with_library(|library| {
            library
                .playlists
                .iter()
                .find(|playlist| playlist.id == playlist_id)
                .map(|playlist| (playlist.id.clone(), playlist.name.clone()))
        })
    }

    /// Where the playlist run stands, for the queue page to explain it.
    ///
    /// Positions count the arranged order *without* the watched filter, so the
    /// number does not jump back to 1 every time finishing a video hides it.
    /// `remaining` is what the run will actually still play.
    pub fn playlist_run_status(self) -> Option<PlaylistRunStatus> {
        let (playlist_id, name) = self.running_playlist()?;
        let view = self.playlist_view(&playlist_id);
        let order = self.playlist_run_order(&playlist_id);
        let current_id = self
            .active_video()
            .map(|video| video.id)
            .unwrap_or_default();
        let index = order.iter().position(|video| video.id == current_id);
        let ahead = index.map_or(&order[..], |index| &order[index + 1..]);
        let remaining = ahead.iter().filter(|video| view.shows(video)).count();
        let up_next = view
            .next_after(&order, &current_id)
            .and_then(|id| order.iter().find(|video| video.id == id).cloned());
        Some(PlaylistRunStatus {
            playlist_id,
            name,
            total: order.len(),
            position: index.map(|index| index + 1),
            current: index.map(|index| order[index].clone()),
            remaining,
            up_next,
            view,
        })
    }

    /// The name of the playlist the queue is running, for controls that would
    /// otherwise only be able to say "queue".
    pub fn running_playlist_name(self) -> Option<String> {
        self.running_playlist().map(|(_, name)| name)
    }

    /// The playlist the queue is currently running, if any.
    fn queued_playlist_run(self) -> Option<String> {
        self.with_library(|library| {
            library
                .queue
                .iter()
                .find_map(|entry| queued_playlist_id(entry).map(str::to_string))
        })
    }

    /// Return to the video this run last advanced away from, or to the previous
    /// entry of the playlist being run when there is no history yet.
    ///
    /// The video being left goes back to the front of the queue, so the run is
    /// exactly where it was: pressing next again plays it, rather than skipping
    /// whatever the viewer stepped back past. A playlist run needs no such
    /// bookmark - it finds its place from whatever is playing - so stepping back
    /// into one leaves the queue alone.
    pub fn step_back_in_run(mut self, current_video_id: &str) -> Option<String> {
        let Some(previous) = self.run_back_stack.write().pop() else {
            let playlist_id = self.queued_playlist_run()?;
            return self.previous_in_playlist_run(&playlist_id, current_video_id);
        };
        if previous == current_video_id {
            return None;
        }
        let mut library = self.library.write();
        library.queue.retain(|id| id != current_video_id);
        library.queue.insert(0, current_video_id.to_string());
        library.cache_revision += 1;
        drop(library);
        self.sync_in_background();
        Some(previous)
    }

    /// Hand the queue a playlist to run, replacing any playlist already running.
    ///
    /// Goes to the front: pressing play on a playlist is a request to watch it
    /// now, not after whatever was queued by hand last week. Only one playlist
    /// runs at a time, because two markers would interleave in an order neither
    /// playlist's page could show.
    pub fn start_playlist_run(mut self, playlist_id: &str) {
        let marker = playlist_queue_entry(playlist_id);
        let mut library = self.library.write();
        library
            .queue
            .retain(|entry| queued_playlist_id(entry).is_none());
        library.queue.insert(0, marker);
        library.cache_revision += 1;
        drop(library);
        self.sync_in_background();
    }

    /// Stop running a playlist, leaving anything queued by hand in place.
    pub fn clear_playlist_run(mut self) -> bool {
        let mut library = self.library.write();
        let before = library.queue.len();
        library
            .queue
            .retain(|entry| queued_playlist_id(entry).is_none());
        if library.queue.len() == before {
            return false;
        }
        library.cache_revision += 1;
        drop(library);
        self.sync_in_background();
        true
    }

    fn push_back_stack(mut self, video_id: &str) {
        let mut stack = self.run_back_stack.write();
        if stack.last().is_some_and(|last| last == video_id) {
            return;
        }
        stack.push(video_id.to_string());
        // One sitting's worth. The stack is only ever walked from the end.
        if stack.len() > 50 {
            stack.remove(0);
        }
    }

    /// Append a whole run of videos to the queue, in the order given.
    ///
    /// One write and one sync rather than one of each per video: queueing a
    /// playlist through `add_to_queue` would hit the server once per entry.
    ///
    /// Ids already in the queue are moved rather than duplicated, because the
    /// run is what the viewer just chose to watch, in the order they chose it;
    /// a stale copy earlier in the queue would play them out of that order.
    pub fn queue_run(mut self, video_ids: &[String]) -> usize {
        if video_ids.is_empty() {
            return 0;
        }
        let mut library = self.library.write();
        library.queue.retain(|id| !video_ids.contains(id));
        library.queue.extend(video_ids.iter().cloned());
        library.cache_revision += 1;
        drop(library);
        self.sync_in_background();
        video_ids.len()
    }

    pub fn add_to_queue(mut self, video_id: &str, play_next: bool) -> String {
        let mut library = self.library.write();
        library.queue.retain(|id| id != video_id);
        if play_next {
            library.queue.insert(0, video_id.to_string());
        } else {
            library.queue.push(video_id.to_string());
        }
        library.cache_revision += 1;
        drop(library);
        self.sync_in_background();
        if play_next {
            "Playing next".into()
        } else {
            "Added to queue".into()
        }
    }

    pub fn remove_from_queue(mut self, video_id: &str) -> bool {
        let mut library = self.library.write();
        let before = library.queue.len();
        library.queue.retain(|id| id != video_id);
        if library.queue.len() == before {
            return false;
        }
        library.cache_revision += 1;
        drop(library);
        self.sync_in_background();
        true
    }

    pub fn clear_queue(mut self) {
        let mut library = self.library.write();
        library.queue.clear();
        library.cache_revision += 1;
        drop(library);
        self.sync_in_background();
    }

    pub fn record_history(mut self, video_id: &str) {
        // A peek, not the write guard: see `record_progress`. Reopening the
        // video already at the top is the usual case - expanding the mini
        // player - and it landed a full re-render and cache write mid-morph.
        if self
            .library
            .peek()
            .history
            .first()
            .is_some_and(|entry| entry.video_id == video_id && entry.played_at == "Just now")
        {
            return;
        }
        let mut library = self.library.write();
        library.history.retain(|entry| entry.video_id != video_id);
        library.history.insert(
            0,
            HistoryEntry {
                video_id: video_id.to_string(),
                played_at: "Just now".into(),
            },
        );
        library.history.truncate(250);
        library.cache_revision += 1;
        drop(library);
        self.sync_in_background();
    }

    pub fn clear_history(mut self) {
        let mut library = self.library.write();
        library.history.clear();
        library.cache_revision += 1;
        drop(library);
        self.sync_in_background();
    }
}

/// Carry over any video the incoming snapshot dropped but something still
/// references, so ids in the queue, playlists, or history always resolve.
fn keep_referenced_videos(
    current: &LibrarySnapshot,
    mut remote: LibrarySnapshot,
) -> LibrarySnapshot {
    let mut referenced = current.queue.clone();
    referenced.extend(current.history.iter().map(|entry| entry.video_id.clone()));
    referenced.extend(
        current
            .playlists
            .iter()
            .flat_map(|playlist| playlist.video_ids.iter().cloned()),
    );
    for id in referenced {
        if remote.videos.iter().any(|video| video.id == id) {
            continue;
        }
        if let Some(kept) = current.videos.iter().find(|video| video.id == id) {
            remote.videos.push(kept.clone());
        }
    }
    remote
}

#[component]
pub fn AppStateProvider(children: Element) -> Element {
    // Before the library, deliberately. Every library endpoint is scoped to an
    // account now and refuses an unauthenticated caller, so a sync that starts
    // first gets a 500 and the app silently keeps its cached copy.
    let session = use_session_provider();
    // The library is megabytes and changes every few seconds while a video
    // plays, so its writes are spaced out; the server holds anything that
    // matters in between. Settings are small and changed by hand.
    let mut library = use_persistent_signal("tawny-library-v1", 10_000, LibrarySnapshot::demo);
    let settings = use_persistent_signal("tawny-settings-v1", 250, AppSettings::default);
    let mut initial_sync_started = use_signal(|| false);
    let mut initial_syncing = use_signal(|| false);

    use_effect(move || {
        // Re-runs when the bootstrap lands, which is what actually starts this.
        if !session.is_ready() || initial_sync_started() {
            return;
        }
        initial_sync_started.set(true);
        initial_syncing.set(true);
        spawn(async move {
            if let Ok(remote) = get_library().await {
                let local = library();
                if remote.cache_revision > local.cache_revision {
                    library.set(keep_referenced_videos(&local, remote));
                } else if local.cache_revision > remote.cache_revision
                    && let Ok(merged) = sync_library(local.clone()).await
                {
                    library.set(keep_referenced_videos(&local, merged));
                }
            }
            initial_syncing.set(false);
        });
    });

    use_context_provider(|| AppState {
        library,
        settings,
        toaster: Signal::new(None),
        playlist_picker_video: Signal::new(None),
        playlist_picker_open: Signal::new(false),
        video_actions_video: Signal::new(None),
        video_actions_playlist: Signal::new(None),
        video_actions_open: Signal::new(false),
        share_video: Signal::new(None),
        share_open: Signal::new(false),
        share_with_timestamp: Signal::new(false),
        share_timestamp_seconds: Signal::new(0),
        active_video: Signal::new(None),
        active_captions: Signal::new(Vec::new()),
        selected_caption: Signal::new(None),
        captions_enabled: Signal::new(false),
        active_chapters: Signal::new(Vec::new()),
        active_sponsor_segments: Signal::new(Vec::new()),
        active_audio_tracks: Signal::new(Vec::new()),
        selected_audio_track: Signal::new(None),
        active_preview_frames: Signal::new(None),
        syncing: initial_syncing,
        chapters_sheet_open: Signal::new(false),
        run_back_stack: Signal::new(Vec::new()),
        feed_filter: Signal::new(FeedFilter::All),
        explore_filter: Signal::new(crate::models::ExploreFilter::All),
        channel_tab: Signal::new(FeedFilter::All),
    });

    rsx! { {children} }
}
