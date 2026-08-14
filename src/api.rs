use crate::models::{
    ChannelDetails, ChannelMediaPage, CommentsPage, FeedRefreshResult, LibrarySnapshot,
    PlaybackSession, SearchResults, VideoDetails,
};
use dioxus::prelude::*;

#[get(
    "/api/v1/library",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>
)]
pub async fn get_library() -> Result<LibrarySnapshot> {
    Ok(state
        .library_snapshot()
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

#[post(
    "/api/v1/library/sync",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>
)]
pub async fn sync_library(snapshot: LibrarySnapshot) -> Result<LibrarySnapshot> {
    Ok(state
        .sync_library(snapshot)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

#[get(
    "/api/v1/search?query&filter",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>
)]
pub async fn search_catalog(query: String, filter: String) -> Result<SearchResults> {
    Ok(state
        .search_catalog(&query, &filter)
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
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>
)]
pub async fn get_video_details(video_id: String) -> Result<VideoDetails> {
    Ok(state
        .video_details(&video_id)
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
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>
)]
pub async fn get_channel_details(channel_id: String) -> Result<ChannelDetails> {
    Ok(state
        .channel_details(&channel_id)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

#[get(
    "/api/v1/channels/{channel_id}/media?tab&next_page",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>
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
        .channel_media_page(&channel_id, tab, &next_page)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}

#[post(
    "/api/v1/feed/refresh",
    state: dioxus::fullstack::extract::State<crate::server::AppServerState>
)]
pub async fn refresh_subscription_feed() -> Result<FeedRefreshResult> {
    Ok(state
        .refresh_feed()
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?)
}
