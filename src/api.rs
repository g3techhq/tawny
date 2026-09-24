use crate::models::{
    Account, ChannelDetails, ChannelMediaPage, CommentsPage, Credentials, FeedRefreshResult,
    LibrarySnapshot, LibraryUserState, PlaybackSession, SearchResults, SponsorSegment, Video,
    VideoDetails,
};
use dioxus::prelude::*;

#[get(
    "/api/v1/library",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn get_library() -> Result<LibrarySnapshot> {
    Ok(state
        .library_snapshot(&owner.0)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

#[post(
    "/api/v1/library/sync",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn sync_library(snapshot: LibrarySnapshot) -> Result<LibrarySnapshot> {
    Ok(state
        .sync_library(&owner.0, snapshot)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

/// The hot path: one mutation, only the rows the client owns.
///
/// `sync_library` above round-trips the entire cache and is reserved for the
/// two places that genuinely reconcile - first load and pull-to-refresh. Using
/// it for every edit meant a 4.3 MB upload, a discarded 4.3 MB reply, and
/// ~11,900 redundant upserts each time a video was marked watched.
#[post(
    "/api/v1/library/state",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn push_library_state(user_state: LibraryUserState) -> Result<u64> {
    Ok(state
        .apply_user_state(&owner.0, user_state)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

#[get(
    "/api/v1/search?query&filter",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    owner: crate::auth::Owner
)]
pub async fn search_catalog(query: String, filter: String) -> Result<SearchResults> {
    Ok(state
        .search_catalog(&owner.0, &query, &filter)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

#[get(
    "/api/v1/search/page?query&filter&next_page",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>
)]
pub async fn search_catalog_page(
    query: String,
    filter: String,
    next_page: String,
) -> Result<SearchResults> {
    Ok(state
        .search_page(&query, &filter, &next_page)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

#[get(
    "/api/v1/playback/{video_id}?prefer_sabr&fresh",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>
)]
pub async fn resolve_playback(
    video_id: String,
    prefer_sabr: bool,
    fresh: bool,
) -> Result<PlaybackSession> {
    Ok(state
        .playback_session(&video_id, prefer_sabr, fresh)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
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
    Ok(state
        .channel_media_page(&owner.0, &channel_id, tab, &next_page)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
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
#[post(
    "/api/v1/auth/guest",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    session: crate::auth::SessionAuth
)]
pub async fn create_guest_account() -> Result<Account> {
    let account = state
        .accounts()
        .create_guest()
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?;
    crate::auth::sign_in_session(&session.0, &account);
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
#[post(
    "/api/v1/auth/sign-in",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    session: crate::auth::SessionAuth
)]
pub async fn sign_in_to_account(credentials: Credentials) -> Result<Account> {
    let account = state
        .accounts()
        .sign_in(&credentials.email, &credentials.password)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?;
    session.0.logout_user();
    crate::auth::sign_in_session(&session.0, &account);
    Ok(account)
}

/// Drop this device's session. Other devices keep theirs.
#[post(
    "/api/v1/auth/sign-out",
    session: crate::auth::SessionAuth
)]
pub async fn sign_out_of_account() -> Result<()> {
    session.0.logout_user();
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
