use crate::models::{
    Account, ChannelDetails, ChannelMediaPage, CommentsPage, Credentials, FeedPage, FeedQuery,
    FeedRefreshResult, PlaybackFailure, PlaybackFailureOrigin, PlaybackSession, Playlist,
    PlaylistContents, PlaylistPreview, SearchResults, SponsorSegment, SubscriptionChange,
    SubscriptionGroup, Video, VideoDetails, VideoProgress, Viewer,
};
use dioxus::prelude::*;

#[get(
    "/api/v1/search?query&filter",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn search_catalog(query: String, filter: String) -> Result<SearchResults> {
    let mut answer = state
        .search_catalog(&owner.0, &query, &filter)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?;
    state
        .overlay_viewer(&owner.0, &mut answer.videos)
        .await
        .map_err(server_error)?;
    Ok(answer)
}

#[get(
    "/api/v1/search/page?query&filter&next_page",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn search_catalog_page(
    query: String,
    filter: String,
    next_page: String,
) -> Result<SearchResults> {
    let mut answer = state
        .search_page(&owner.0, &query, &filter, &next_page)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?;
    state
        .overlay_viewer(&owner.0, &mut answer.videos)
        .await
        .map_err(server_error)?;
    Ok(answer)
}

#[get(
    "/api/v1/playback/{video_id}?prefer_sabr&fresh",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>
)]
pub async fn resolve_playback(
    video_id: String,
    prefer_sabr: bool,
    fresh: bool,
) -> Result<PlaybackSession, PlaybackFailure> {
    // A typed error, so the player learns whose problem it is and what fixes
    // it rather than a flattened message.
    state.playback_session(&video_id, prefer_sabr, fresh).await
}

/// What the typed server function error needs. Failures the resolver never
/// saw - the request not arriving, or not decoding - are classified here.
impl From<ServerFnError> for PlaybackFailure {
    fn from(error: ServerFnError) -> Self {
        match &error {
            ServerFnError::Request(_) | ServerFnError::StreamError(_) => PlaybackFailure::new(
                PlaybackFailureOrigin::Setup,
                "The Tawny server could not be reached.",
            )
            .remedy("Check your connection and that the server is running, then try again."),
            ServerFnError::ServerError { .. } => PlaybackFailure::new(
                PlaybackFailureOrigin::Unknown,
                "The server could not start playback.",
            )
            .remedy("Check the server log for the cause."),
            _ => PlaybackFailure::new(
                PlaybackFailureOrigin::Bug,
                "The app and the server disagreed about the playback request.",
            )
            .remedy(
                "Reload the app, since it may be older than the server. If it persists, report \
                 it with the detail below.",
            ),
        }
        .detail(error.to_string())
    }
}

impl dioxus::fullstack::AsStatusCode for PlaybackFailure {
    fn as_status_code(&self) -> dioxus::fullstack::StatusCode {
        use dioxus::fullstack::StatusCode;
        match self.origin {
            // The upstream refused or a dependency is down: a gateway failure.
            PlaybackFailureOrigin::Youtube | PlaybackFailureOrigin::Setup => {
                StatusCode::BAD_GATEWAY
            }
            PlaybackFailureOrigin::Bug | PlaybackFailureOrigin::Unknown => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}

#[get(
    "/api/v1/videos/{video_id}",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn get_video_details(video_id: String) -> Result<VideoDetails> {
    Ok(state
        .video_details(&owner.0, &video_id)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

#[get(
    "/api/v1/videos/{video_id}/comments?next_page",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>
)]
pub async fn get_comments_page(video_id: String, next_page: String) -> Result<CommentsPage> {
    Ok(state
        .comments_page(&video_id, &next_page)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

#[get(
    "/api/v1/channels/{channel_id}",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn get_channel_details(channel_id: String) -> Result<ChannelDetails> {
    Ok(state
        .channel_details(&owner.0, &channel_id)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

#[get(
    "/api/v1/channels/{channel_id}/media?tab&next_page",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn get_channel_media_page(
    channel_id: String,
    tab: String,
    next_page: String,
) -> Result<ChannelMediaPage> {
    let tab = match tab.as_str() {
        "shorts" => crate::models::ChannelMediaTab::Shorts,
        "live" => crate::models::ChannelMediaTab::Live,
        _ => crate::models::ChannelMediaTab::Videos,
    };
    let mut answer = state
        .channel_media_page(&owner.0, &channel_id, tab, &next_page)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?;
    state
        .overlay_viewer(&owner.0, &mut answer.videos)
        .await
        .map_err(server_error)?;
    Ok(answer)
}

#[post(
    "/api/v1/feed/refresh",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn refresh_subscription_feed() -> Result<FeedRefreshResult> {
    Ok(state
        .refresh_feed(&owner.0)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

/// Resolve exact metadata for the cards currently on screen.
///
/// RSS has no runtime, so the feed, a playlist, and anything else built from
/// cached rows all show videos whose length is unknown - and a duration filter
/// cannot speak for those. Keeping this separate from the full feed refresh
/// lets a page fill in what it is showing without reconciling any channels.
#[post(
    "/api/v1/videos/durations",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn hydrate_video_durations(video_ids: Vec<String>) -> Result<Vec<Video>> {
    Ok(state
        .hydrate_video_durations(&owner.0, video_ids)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

// ---------------------------------------------------------------------------
// Accounts
//
// Axum owns the opaque cookie and restores the account before each handler.
// The client never receives a session identifier to persist or replay.
//
// Errors come back as their rendered message. Field-level marking is done on
// the client with the shared validators in `models`, before the request is
// made; by the time the server refuses something, the only useful thing left to
// say is why.
// ---------------------------------------------------------------------------

/// Mint an anonymous account. Called once, on a first launch.
///
/// Deliberately unauthenticated - this is where a viewer gets their first
/// credential, so requiring one would be circular.
#[g3_auth::public]
#[post(
    "/api/v1/auth/guest",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    crate::auth::SessionContext { auth_session, .. }: crate::auth::SessionContext
)]
pub async fn create_guest_account() -> Result<Account> {
    let account = state
        .accounts()
        .create_guest()
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?;
    state
        .seed_new_account(&account.id)
        .await
        .map_err(server_error)?;
    crate::auth::sign_in_session(&auth_session, &account);
    Ok(account)
}

/// Promote the calling guest into a real account, keeping its library.
#[post(
    "/api/v1/auth/register",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn register_account(credentials: Credentials) -> Result<Account> {
    Ok(state
        .accounts()
        .register(&owner.0, &credentials.email, &credentials.password)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

/// Sign in to an existing account, abandoning whatever session was held.
///
/// Public: a device whose session expired signs back in from here.
#[g3_auth::public]
#[post(
    "/api/v1/auth/sign-in",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    crate::auth::SessionContext { auth_session, .. }: crate::auth::SessionContext
)]
pub async fn sign_in_to_account(credentials: Credentials) -> Result<Account> {
    let account = state
        .accounts()
        .sign_in(&credentials.email, &credentials.password)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?;
    auth_session.logout_user();
    crate::auth::sign_in_session(&auth_session, &account);
    Ok(account)
}

/// Drop this device's session. Other devices keep theirs.
///
/// Public: signing out of a session that already expired has to succeed, not
/// strand the device on an error.
#[g3_auth::public]
#[post(
    "/api/v1/auth/sign-out",
    crate::auth::SessionContext { auth_session, .. }: crate::auth::SessionContext
)]
pub async fn sign_out_of_account() -> Result<()> {
    auth_session.logout_user();
    Ok(())
}

/// Who the caller is, used on launch to restore the cookie-backed session.
#[get(
    "/api/v1/auth/account",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn current_account() -> Result<Account> {
    Ok(state
        .accounts()
        .find(&owner.0)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

/// SponsorBlock segments for one video.
///
/// The categories come from the client because they are the viewer's settings,
/// and asking for the ones they ignore would fetch data only to discard it.
/// Authenticated like the rest of the library surface: this instance is not an
/// open SponsorBlock proxy.
#[get(
    "/api/v1/videos/{video_id}/sponsor?categories",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    _owner: crate::auth::Owner
)]
pub async fn get_sponsor_segments(
    video_id: String,
    categories: String,
) -> Result<Vec<SponsorSegment>> {
    let categories = categories
        .split(',')
        .filter(|name| !name.is_empty())
        .filter_map(crate::models::SponsorCategory::from_api_name)
        .collect::<Vec<_>>();
    Ok(state
        .sponsor_segments(&video_id, &categories)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

// ---------------------------------------------------------------------------
// The viewer
//
// One account's own state, and the videos each screen shows. Reads are
// screen-sized; nothing here returns the catalog. Writes are "set to": a
// device showing stale state sends what it saw, and a toggle would undo
// another device's change.
//
// Complex arguments go over POST, since a GET query string cannot carry a
// struct; these reads are per-viewer and never cached anywhere but the
// device, so nothing is lost.
// ---------------------------------------------------------------------------

#[cfg(feature = "server")]
fn server_error(error: anyhow::Error) -> ServerFnError {
    ServerFnError::new(error.to_string())
}

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
