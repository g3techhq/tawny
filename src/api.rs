use crate::models::{
    Account, AuthSession, ChannelDetails, ChannelMediaPage, CommentsPage, Credentials,
    FeedRefreshResult, LibrarySnapshot, LibraryUserState, PlaybackSession, SearchResults,
    SponsorSegment, VideoDetails,
};
use dioxus::prelude::*;

/// Resolve the caller's account, or refuse.
///
/// Every endpoint below that reads or writes library data goes through this.
/// There is deliberately no fallback owner: an unauthenticated request gets an
/// error, not somebody else's subscriptions.
#[cfg(feature = "server")]
async fn owner_of(
    state: &crate::server::AppServerState,
    headers: &dioxus::fullstack::HeaderMap,
) -> Result<String, ServerFnError> {
    let token = crate::server::bearer_token(headers);
    state
        .accounts()
        .authenticate(&token)
        .await
        .map(|account| account.id)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[get(
    "/api/v1/library",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    headers: dioxus::fullstack::HeaderMap
)]
pub async fn get_library() -> Result<LibrarySnapshot> {
    let owner = owner_of(&state, &headers).await?;
    Ok(state
        .library_snapshot(&owner)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

#[post(
    "/api/v1/library/sync",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    headers: dioxus::fullstack::HeaderMap
)]
pub async fn sync_library(snapshot: LibrarySnapshot) -> Result<LibrarySnapshot> {
    let owner = owner_of(&state, &headers).await?;
    Ok(state
        .sync_library(&owner, snapshot)
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
    headers: dioxus::fullstack::HeaderMap
)]
pub async fn push_library_state(user_state: LibraryUserState) -> Result<u64> {
    let owner = owner_of(&state, &headers).await?;
    Ok(state
        .apply_user_state(&owner, user_state)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

#[get(
    "/api/v1/search?query&filter",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    headers: dioxus::fullstack::HeaderMap
)]
pub async fn search_catalog(query: String, filter: String) -> Result<SearchResults> {
    let owner = owner_of(&state, &headers).await?;
    Ok(state
        .search_catalog(&owner, &query, &filter)
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
    "/api/v1/playback/{video_id}?prefer_sabr",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>
)]
pub async fn resolve_playback(video_id: String, prefer_sabr: bool) -> Result<PlaybackSession> {
    Ok(state
        .playback_session(&video_id, prefer_sabr)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

#[get(
    "/api/v1/videos/{video_id}",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    headers: dioxus::fullstack::HeaderMap
)]
pub async fn get_video_details(video_id: String) -> Result<VideoDetails> {
    let owner = owner_of(&state, &headers).await?;
    Ok(state
        .video_details(&owner, &video_id)
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
    headers: dioxus::fullstack::HeaderMap
)]
pub async fn get_channel_details(channel_id: String) -> Result<ChannelDetails> {
    let owner = owner_of(&state, &headers).await?;
    Ok(state
        .channel_details(&owner, &channel_id)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

#[get(
    "/api/v1/channels/{channel_id}/media?tab&next_page",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    headers: dioxus::fullstack::HeaderMap
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
    let owner = owner_of(&state, &headers).await?;
    Ok(state
        .channel_media_page(&owner, &channel_id, tab, &next_page)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

#[post(
    "/api/v1/feed/refresh",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    headers: dioxus::fullstack::HeaderMap
)]
pub async fn refresh_subscription_feed() -> Result<FeedRefreshResult> {
    let owner = owner_of(&state, &headers).await?;
    Ok(state
        .refresh_feed(&owner)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

// ---------------------------------------------------------------------------
// Accounts
//
// The session token travels in an `Authorization: Bearer` header rather than a
// cookie, because a packaged client points at an arbitrary origin - often plain
// http on a LAN - where a cookie would need `SameSite=None; Secure` and working
// CORS credentials, and would simply not arrive. The client installs the header
// once through `dioxus::fullstack::set_request_headers`.
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
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>
)]
pub async fn create_guest_account() -> Result<AuthSession> {
    Ok(state
        .accounts()
        .create_guest()
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

/// Promote the calling guest into a real account, keeping its library.
#[post(
    "/api/v1/auth/register",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    headers: dioxus::fullstack::HeaderMap
)]
pub async fn register_account(credentials: Credentials) -> Result<AuthSession> {
    let token = crate::server::bearer_token(&headers);
    Ok(state
        .accounts()
        .register(&token, &credentials.email, &credentials.password)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

/// Sign in to an existing account, abandoning whatever session was held.
#[post(
    "/api/v1/auth/sign-in",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>
)]
pub async fn sign_in_to_account(credentials: Credentials) -> Result<AuthSession> {
    Ok(state
        .accounts()
        .sign_in(&credentials.email, &credentials.password)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

/// Drop this device's session. Other devices keep theirs.
#[post(
    "/api/v1/auth/sign-out",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    headers: dioxus::fullstack::HeaderMap
)]
pub async fn sign_out_of_account() -> Result<()> {
    let token = crate::server::bearer_token(&headers);
    Ok(state
        .accounts()
        .sign_out(&token)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

/// Who the caller is, used on launch to decide whether a stored token is still
/// worth anything before the app renders as though it were.
#[get(
    "/api/v1/auth/account",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>,
    headers: dioxus::fullstack::HeaderMap
)]
pub async fn current_account() -> Result<Account> {
    let token = crate::server::bearer_token(&headers);
    Ok(state
        .accounts()
        .authenticate(&token)
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
    headers: dioxus::fullstack::HeaderMap
)]
pub async fn get_sponsor_segments(
    video_id: String,
    categories: String,
) -> Result<Vec<SponsorSegment>> {
    owner_of(&state, &headers).await?;
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
