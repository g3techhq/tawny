use crate::{
    api::{get_library, sync_library},
    cache::use_persistent_signal,
    models::{
        AppSettings, CaptionTrack, Channel, ChannelDetails, HistoryEntry, LibrarySnapshot,
        Playlist, SearchResults, SubscriptionGroup, Video, VideoChapter, VideoDetails,
        VideoPreviewFrames,
    },
};
use dioxus::prelude::*;
use g3_ui::StatusColor;

#[derive(Clone, Copy)]
pub struct AppState {
    pub library: Signal<LibrarySnapshot>,
    pub settings: Signal<AppSettings>,
    pub toast_open: Signal<bool>,
    pub toast: Signal<(String, StatusColor)>,
    pub playlist_picker_video: Signal<Option<Video>>,
    pub playlist_picker_open: Signal<bool>,
    pub video_actions_video: Signal<Option<Video>>,
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
    pub active_preview_frames: Signal<Option<VideoPreviewFrames>>,
    pub syncing: Signal<bool>,
    /// Segment selections live here because the header owns the segmented
    /// control while the page owns the list it filters.
    pub chapters_sheet_open: Signal<bool>,
    pub feed_filter_index: Signal<usize>,
    pub explore_filter_index: Signal<usize>,
    pub channel_tab_index: Signal<usize>,
}

impl AppState {
    pub fn library(self) -> LibrarySnapshot {
        (self.library)()
    }

    pub fn settings(self) -> AppSettings {
        (self.settings)()
    }

    pub fn toast(self) -> (String, StatusColor) {
        (self.toast)()
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
            self.active_captions.set(Vec::new());
            self.selected_caption.set(None);
            self.captions_enabled.set(false);
            self.active_chapters.set(Vec::new());
            self.active_preview_frames.set(None);
        }
        self.active_video.set(Some(video));
    }

    pub fn set_player_metadata(
        mut self,
        captions: Vec<CaptionTrack>,
        chapters: Vec<VideoChapter>,
        preview_frames: Option<VideoPreviewFrames>,
    ) {
        if (self.selected_caption)().is_none() && !captions.is_empty() {
            self.selected_caption.set(Some(0));
        }
        self.active_captions.set(captions);
        self.active_chapters.set(chapters);
        self.active_preview_frames.set(preview_frames);
    }

    pub fn stop_playback(mut self) {
        self.active_video.set(None);
        self.active_captions.set(Vec::new());
        self.selected_caption.set(None);
        self.captions_enabled.set(false);
        self.active_chapters.set(Vec::new());
        self.active_preview_frames.set(None);
    }

    fn sync_in_background(self) {
        let snapshot = self.library();
        spawn(async move {
            let _ = sync_library(snapshot).await;
        });
    }

    pub fn show_toast(mut self, message: impl Into<String>, color: StatusColor) {
        self.toast.set((message.into(), color));
        self.toast_open.set(true);
    }

    pub fn add_to_playlist(mut self, video_id: &str, playlist_id: &str) -> Option<String> {
        let mut library = self.library.write();
        let playlist_name = {
            let playlist = library
                .playlists
                .iter_mut()
                .find(|playlist| playlist.id == playlist_id)?;
            if playlist.video_ids.iter().any(|id| id == video_id) {
                return Some(format!("Already in {}", playlist.name));
            }
            playlist.video_ids.insert(0, video_id.to_string());
            playlist.name.clone()
        };
        library.cache_revision += 1;
        drop(library);
        self.sync_in_background();
        Some(format!("Added to {playlist_name}"))
    }

    pub fn create_playlist(mut self, name: String) -> String {
        let id = format!("local-{}", self.library().cache_revision + 1);
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
        library.cache_revision += 1;
        drop(library);
        let mut settings = self.settings.write();
        if settings.swipe_right_playlist_id == playlist_id {
            settings.swipe_right_playlist_id = "watch-later".into();
        }
        if settings.swipe_left_playlist_id == playlist_id {
            settings.swipe_left_playlist_id = "deep-dives".into();
        }
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
        let revision = self.library().cache_revision + 1;
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
                let progress_seconds = video.progress_seconds;
                let watched = video.watched;
                *video = discovered;
                video.progress_seconds = progress_seconds;
                video.watched = watched;
            } else {
                library.videos.push(discovered);
            }
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
        let merged = keep_referenced_videos(&self.library(), remote);
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
            SwipeActionKind::AddToPlaylist => {
                if let Some(message) = self.add_to_playlist(video_id, &playlist_id) {
                    self.show_toast(message, StatusColor::Success);
                }
            }
            SwipeActionKind::AddToQueue => {
                let message = self.add_to_queue(video_id, false);
                self.show_toast(message, StatusColor::Success);
            }
            SwipeActionKind::PlayNext => {
                let message = self.add_to_queue(video_id, true);
                self.show_toast(message, StatusColor::Success);
            }
            SwipeActionKind::MarkWatched => {
                self.mark_watched(video_id, true);
                self.show_toast("Marked watched", StatusColor::Neutral);
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
            SwipeActionKind::AddToPlaylist => self
                .library()
                .playlists
                .iter()
                .find(|playlist| playlist.id == playlist_id)
                .map(|playlist| playlist.name.clone())
                .unwrap_or_else(|| "Playlist".into()),
            other => other.label().to_string(),
        }
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
        let mut library = self.library.write();
        if library
            .history
            .first()
            .is_some_and(|entry| entry.video_id == video_id && entry.played_at == "Just now")
        {
            return;
        }
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
    let mut library = use_persistent_signal("tawny-library-v1", LibrarySnapshot::demo);
    let settings = use_persistent_signal("tawny-settings-v1", AppSettings::default);
    let mut initial_sync_started = use_signal(|| false);
    let mut initial_syncing = use_signal(|| false);

    use_effect(move || {
        if initial_sync_started() {
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
        toast_open: Signal::new(false),
        toast: Signal::new((String::new(), StatusColor::Neutral)),
        playlist_picker_video: Signal::new(None),
        playlist_picker_open: Signal::new(false),
        video_actions_video: Signal::new(None),
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
        active_preview_frames: Signal::new(None),
        syncing: initial_syncing,
        chapters_sheet_open: Signal::new(false),
        feed_filter_index: Signal::new(0),
        explore_filter_index: Signal::new(0),
        channel_tab_index: Signal::new(0),
    });

    rsx! { {children} }
}
