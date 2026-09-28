# YouTube

Where Tawny's catalog comes from: direct extraction with rustypipe (`src/server/youtube.rs`, `browse.rs`), official RSS and WebSub (`feed.rs`, `websub.rs`). No Piped instance is involved.

No Piped instance — public or self-hosted — is required or contacted. Tawny extracts public YouTube search results, channel tabs, RSS feeds, video metadata, comments, captions, and player data directly. Search coalesces concurrent requests and caches identical query/filter pairs for five minutes. Results are normalized and deduplicated in SurrealDB. Video details use a two-hour server cache with stale fallback; ephemeral stream URLs and PO tokens are never persisted.

## Discovery source contract

Direct YouTube extraction is the only remote source. It supports real video/channel search, search continuations, channel Videos/Shorts/Live pages, channel continuations, RSS, video details, comments and comment continuations, captions, recommendations, player streams, and PO-token generation when `rustypipe-botguard` is available. Concurrent searches are serialized and identical query/filter results are cached for five minutes to prevent type-ahead or multi-client request bursts from reaching YouTube.

Chapters come from YouTube's structured markers when present; when they are absent Tawny parses timestamps out of the description so description-only chapters still work.

Results are normalized into the models in `models.rs` and deduplicated before caching. Existing subscription/watch state is preserved when metadata refreshes. Extraction failures—including an upstream extractor panic—are contained and fall through to the stale server cache and then to what the device last saw, so a YouTube-side change degrades to cached data rather than an error page.

## Feed ingestion and WebSub

The 15-minute worker uses a tiered policy rather than fully extracting every subscription. When WebSub is active it rotates lightweight official Atom checks through the 48 channels with the oldest `last_polled_at`; without that accelerated source it checks every channel with 32-way bounded concurrency. Full Videos/Shorts/Live extraction is reserved for channel pages and small subscription changes. Bulk subscription imports use bounded RSS instead of hundreds of full extractions. Demo channel IDs are skipped.

If `TAWNY_PUBLIC_URL` or `TAWNY_WEBSUB_CALLBACK_URL` is a public HTTPS address, subscribing also requests a YouTube WebSub lease. The callback verifies the hub challenge/topic/token, validates `X-Hub-Signature` with HMAC-SHA1, immediately stores the upload hint, and enriches only the notified video using player metadata for duration, live state, and Shorts classification. Subscription requests are persisted so restarts do not re-register the whole library; leases older than three days are renewed with 12-way bounded concurrency. RSS reconciliation remains the recovery mechanism.

Feed ingestion is designed for large libraries. When WebSub is active the server rotates lightweight official RSS checks through the 48 stalest channels instead of fully extracting every subscription; without that accelerated source, RSS covers the complete library with bounded concurrency. Full Videos/Shorts/Live extraction happens when a channel is opened or an individual channel is newly subscribed. Atom timestamps and relative labels such as `2 hours ago` are normalized into one chronological sort key, and the client re-sorts cached feed entries on render for migration safety.

For push updates, deploy with a public HTTPS `TAWNY_PUBLIC_URL` (or explicit `TAWNY_WEBSUB_CALLBACK_URL`). Subscribing requests a YouTube WebSub lease; signed callbacks insert the upload immediately and enrich only that video. Lease requests are persisted, renewed after three days, and processed with bounded concurrency. The 15-minute RSS reconciliation worker remains as a recovery path for missed callbacks.

## Checking against YouTube

One server test talks to YouTube and is ignored by default:

```sh
cargo test --no-default-features --features server live_youtube_search_subscribe_channel_and_feed_round_trip -- --ignored --nocapture --test-threads=1
```
