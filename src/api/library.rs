use super::*;

/// This account's follows, playlists, groups, queue and history.
#[get(
    "/api/v1/viewer",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn get_viewer() -> Result<Viewer> {
    Ok(state.viewer(&owner.0).await.map_err(server_error)?)
}

/// One page of the subscription feed.
#[post(
    "/api/v1/feed",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn get_feed_page(query: FeedQuery, page: usize) -> Result<FeedPage> {
    Ok(state
        .feed_page(&owner.0, &query, page)
        .await
        .map_err(server_error)?)
}

/// Videos by id, in the order asked: the queue, history.
#[post(
    "/api/v1/videos/by-id",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn get_videos(ids: Vec<String>) -> Result<Vec<Video>> {
    Ok(state
        .videos_by_id(&owner.0, &ids)
        .await
        .map_err(server_error)?)
}

/// A playlist and its videos, or `None` when this account has no such
/// playlist.
#[get(
    "/api/v1/playlists/{playlist_id}",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn get_playlist(playlist_id: String) -> Result<Option<PlaylistContents>> {
    Ok(state
        .playlist_contents(&owner.0, &playlist_id)
        .await
        .map_err(server_error)?)
}

/// Every playlist with its first few videos.
#[get(
    "/api/v1/playlists",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn get_playlist_previews() -> Result<Vec<PlaylistPreview>> {
    Ok(state
        .playlist_previews(&owner.0, 3)
        .await
        .map_err(server_error)?)
}

#[post(
    "/api/v1/subscriptions",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn set_subscriptions(changes: Vec<SubscriptionChange>) -> Result<()> {
    Ok(state
        .set_subscriptions(&owner.0, changes)
        .await
        .map_err(server_error)?)
}

/// Create a playlist, or rename one.
#[post(
    "/api/v1/playlists/save",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn save_playlist(playlist_id: String, name: String) -> Result<Playlist> {
    Ok(state
        .save_playlist(&owner.0, &playlist_id, &name)
        .await
        .map_err(server_error)?)
}

#[post(
    "/api/v1/playlists/membership",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn set_in_playlist(playlist_id: String, video_id: String, member: bool) -> Result<bool> {
    Ok(state
        .set_in_playlist(&owner.0, &playlist_id, &video_id, member)
        .await
        .map_err(server_error)?)
}

#[post(
    "/api/v1/playlists/remove-watched",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn remove_watched_from_playlist(playlist_id: String) -> Result<usize> {
    Ok(state
        .remove_watched_from_playlist(&owner.0, &playlist_id)
        .await
        .map_err(server_error)?)
}

#[post(
    "/api/v1/playlists/delete",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn delete_playlist(playlist_id: String) -> Result<()> {
    Ok(state
        .delete_playlist(&owner.0, &playlist_id)
        .await
        .map_err(server_error)?)
}

/// Watch progress and the audio-only choice, for the videos given.
#[post(
    "/api/v1/progress",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn save_progress(progress: Vec<VideoProgress>) -> Result<()> {
    Ok(state
        .save_progress(&owner.0, &progress)
        .await
        .map_err(server_error)?)
}

/// This account's saved place in one video, read fresh when it starts playing.
///
/// Lists show the position they were fetched with, which on a second device
/// can be older than where the viewer stopped on the first.
#[get(
    "/api/v1/progress/{video_id}",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn get_video_progress(video_id: String) -> Result<Option<VideoProgress>> {
    Ok(state
        .video_progress(&owner.0, &video_id)
        .await
        .map_err(server_error)?)
}

/// Create or replace subscription groups, by id.
#[post(
    "/api/v1/groups/save",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn save_groups(groups: Vec<SubscriptionGroup>) -> Result<()> {
    Ok(state
        .save_groups(&owner.0, &groups)
        .await
        .map_err(server_error)?)
}

#[post(
    "/api/v1/groups/membership",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn set_in_group(group_id: String, channel_id: String, member: bool) -> Result<bool> {
    Ok(state
        .set_in_group(&owner.0, &group_id, &channel_id, member)
        .await
        .map_err(server_error)?)
}

#[post(
    "/api/v1/groups/delete",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn delete_group(group_id: String) -> Result<()> {
    Ok(state
        .delete_group(&owner.0, &group_id)
        .await
        .map_err(server_error)?)
}

#[post(
    "/api/v1/queue",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn set_queue(queue: Vec<String>) -> Result<()> {
    Ok(state
        .set_queue(&owner.0, &queue)
        .await
        .map_err(server_error)?)
}

#[post(
    "/api/v1/history/record",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn record_history(video_id: String) -> Result<()> {
    Ok(state
        .record_history(&owner.0, &video_id)
        .await
        .map_err(server_error)?)
}

#[post(
    "/api/v1/history/clear",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn clear_history() -> Result<()> {
    Ok(state.clear_history(&owner.0).await.map_err(server_error)?)
}
