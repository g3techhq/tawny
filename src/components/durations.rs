//! On-screen runtime hydration, shared by every page that shows cached cards.
//!
//! YouTube's subscription RSS carries no duration, so `feed_entry_video` stores
//! 0 - there is nothing to store. That is fine for a thumbnail, and fatal for a
//! duration filter: an unknown length is not short, medium, or long, so a chip
//! silently drops the row. The refresh backfill works through the subscription
//! backlog, but it can only ever reach uploads from followed channels, and a
//! playlist is the one place a reader collects videos from channels they do not
//! follow.
//!
//! So each page asks for exactly the rows it is displaying. The server validates
//! and caps the request; the cards re-render as the lengths arrive.

use crate::{api::hydrate_video_durations, models::Video, state::AppState};
use dioxus::prelude::*;
use std::collections::HashSet;

/// How far past the visible page to look when a duration filter is on.
///
/// Some resolved videos will not belong to the selected bucket, so filling only
/// the visible count would leave the page short. The server caps one request at
/// the same size.
pub const DURATION_LOOKAHEAD: usize = 48;

/// Ids of the leading `window` videos that still have no runtime.
///
/// Call this *before* a duration filter runs. After it, the rows with no length
/// are already gone, and they could never acquire the metadata that would let
/// them enter the filter in the first place.
pub fn duration_candidates<'a>(
    videos: impl IntoIterator<Item = &'a Video>,
    window: usize,
) -> Vec<String> {
    videos
        .into_iter()
        .take(window)
        .filter(|video| !video.is_live && video.duration_seconds == 0)
        .take(DURATION_LOOKAHEAD)
        .map(|video| video.id.clone())
        .collect()
}

/// Request the lengths in `candidates`, once each per visit.
///
/// Videos the extractor cannot resolve come back absent rather than as a zero,
/// so remembering what was asked is also what stops a permanently unresolvable
/// row from being retried on every render.
pub fn use_duration_hydration(candidates: Vec<String>) {
    let app_state = use_context::<AppState>();
    let mut requested = use_signal(HashSet::<String>::new);

    // Reruns whenever the candidates change: a filter change, the next page.
    use_effect(use_reactive!(|candidates| {
        let pending = {
            let already_requested = requested.peek();
            candidates
                .iter()
                .filter(|video_id| !already_requested.contains(*video_id))
                .cloned()
                .collect::<Vec<_>>()
        };
        if pending.is_empty() {
            return;
        }
        requested.write().extend(pending.iter().cloned());
        spawn(async move {
            if let Ok(videos) = hydrate_video_durations(pending).await {
                app_state.merge_video_metadata(videos);
            }
        });
    }));
}
