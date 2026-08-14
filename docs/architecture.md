# Tawny architecture

## Shape of the system

```text
YouTube Innertube/RSS ──► source normalization
          │                       │
YouTube WebSub ──────────► SurrealDB/RocksDB ◄──── library sync
                                  │
                           Dioxus server functions
                                  │
                         Tawny clients + local snapshot
                                  │
                       playback proxy/embed fallback
```

Tawny is self-contained. The Dioxus server does everything a Piped backend would do — search, feeds, channels, video details, comments, captions, and stream resolution — by extracting from YouTube directly. There is no external companion service to deploy, and no Piped or third-party instance is contacted.

The client owns interaction state, watch progress, swipe destinations, and an immediately available cached snapshot. The server owns subscription polling and the canonical multi-device data model. Playback resolution is isolated behind its own module because YouTube delivery behavior changes independently from feed and library behavior.

## Client cache

The client persists `LibrarySnapshot` and `AppSettings` in the platform WebView's local storage. Dioxus uses that WebView surface on web, desktop, Android, and iOS, giving all six targets the same behavior with no platform-specific database bridge. The server separately caches normalized video details for two hours in SurrealDB and retains stale details for offline fallback.

The cache follows stale-while-revalidate semantics:

1. render the local snapshot immediately;
2. request the server snapshot in the background;
3. push a newer local library revision or accept the newer server revision;
4. keep the local snapshot if the server is unavailable.

Local storage is appropriate for the initial bounded feed. Before offline downloads, large histories, transcripts, or thousands of cached videos are enabled, migrate the same repository boundary to IndexedDB on web and a native store on mobile/desktop. SurrealDB 3 supports `kv-indxdb` in browser builds and SurrealKV in native builds, so an embedded-Surreal implementation remains a viable follow-up once its mobile footprint is measured.

## Server and SurrealDB

When `SURREALDB_HOST` is absent, the server opens an embedded SurrealDB RocksDB database beneath the platform data directory (overridable with `TAWNY_DATA_DIR`). `mem://` remains available explicitly for tests or disposable sessions. The schema models:

- channels and videos;
- users and subscription relations;
- playlists and ordered playlist-item relations;
- subscription groups and their channel membership;
- normalized video-detail cache entries.

The 15-minute worker uses a tiered policy rather than fully extracting every subscription. When WebSub is active it rotates lightweight official Atom checks through the 48 channels with the oldest `last_polled_at`; without that accelerated source it checks every channel with 32-way bounded concurrency. Full Videos/Shorts/Live extraction is reserved for channel pages and small subscription changes. Bulk subscription imports use bounded RSS instead of hundreds of full extractions. Demo channel IDs are skipped.

If `TAWNY_PUBLIC_URL` or `TAWNY_WEBSUB_CALLBACK_URL` is a public HTTPS address, subscribing also requests a YouTube WebSub lease. The callback verifies the hub challenge/topic/token, validates `X-Hub-Signature` with HMAC-SHA1, immediately stores the upload hint, and enriches only the notified video using player metadata for duration, live state, and Shorts classification. Subscription requests are persisted so restarts do not re-register the whole library; leases older than three days are renewed with 12-way bounded concurrency. RSS reconciliation remains the recovery mechanism.

Playlist, subscription, watch-state, queue, and history mutations now use optimistic local writes followed by revision-checked SurrealDB snapshot sync. The sync is serialized server-side; a stale or equal client revision receives the current server snapshot instead of overwriting it.

The next server milestone is user authentication and per-account records. Multi-user and long-offline deployments should graduate from whole-library revisions to an operation log so independent edits can merge instead of choosing one complete snapshot.

## Discovery source contract

Direct YouTube extraction is the only remote source. It supports real video/channel search, search continuations, channel Videos/Shorts/Live pages, channel continuations, RSS, video details, comments and comment continuations, captions, recommendations, player streams, and PO-token generation when `rustypipe-botguard` is available. Concurrent searches are serialized and identical query/filter results are cached for five minutes to prevent type-ahead or multi-client request bursts from reaching YouTube.

Chapters come from YouTube's structured markers when present; when they are absent Tawny parses timestamps out of the description so description-only chapters still work.

Results are normalized into the models in `models.rs` and deduplicated before caching. Existing subscription/watch state is preserved when metadata refreshes. Extraction failures—including an upstream extractor panic—are contained and fall through to the stale server cache and then the device snapshot, so a YouTube-side change degrades to cached data rather than an error page.

## Playback model

`playback_session` resolves streams in-process and returns a `PlaybackSession` (see `models.rs`) over the server function boundary. Nothing external is contacted for resolution.

A `PlaybackSource` carries a `protocol` of `Sabr`, `Hls`, `Dash`, `Progressive`, or `EmbedFallback`. Adaptive sources carry a `tracks` list holding at least one indexed `Video` and one indexed `Audio` representation, and normally every available quality so the client can select automatically. Each track keeps its codec, bitrate, duration, resolution, and DASH initialization/index byte ranges; range ends are inclusive. PO tokens are obtained per video when `rustypipe-botguard` is configured. Playback sources and PO tokens are deliberately kept out of the persistent cache because they are short-lived and request-bound.

Stream URLs come from `yt-dlp` when it is available (`TAWNY_YTDLP_BIN`, else `yt-dlp` on PATH), joined to the extractor's stream layout by itag. This split exists because the two halves fail independently: rustypipe can only reach YouTube's iOS client, whose googlevideo URLs are gated to roughly 6 MiB before returning 403, while yt-dlp uses the `android_vr` client, whose URLs carry no such gate. Byte ranges, codecs, languages, and sizes still come from the extractor — an itag identifies one specific transcode, so the two clients hand out different URLs for the same file. Sizes are compared per itag and a mismatch keeps the extractor's URL rather than pointing ranges at the wrong bytes. Without yt-dlp installed, playback still resolves but is subject to the gate.

Source order is direct YouTube player extraction ranked by `playback_source_priority` — adaptive track sets first, then HLS, progressive, and plain manifests. The privacy-enhanced YouTube embed is **not** an automatic fallback: when every direct source fails, the player surfaces the transport's own error with a retry action, and the embed is offered only as an explicit user choice.

The player is mounted in the shared application shell, expands above video details, and switches to a fixed mini-player above bottom navigation on other routes without replacing the media element. Non-embed sources and every adaptive track are registered as short-lived same-origin proxy targets. The proxy forwards byte ranges and configured request headers, streams response bodies, bounds connect and per-chunk read time so a wedged upstream surfaces as an error instead of an indefinite stall, and rewrites HLS playlists plus DASH `BaseURL` elements so child requests stay same-origin. Expired proxy registrations are pruned as new sessions are created.

The client generates an in-memory DASH manifest for extracted representation sets and sends it to the packaged Shaka transport. Shaka handles Media Source capability negotiation, automatic bitrate selection, buffering, seeking, DASH/HLS manifests, and retry recovery. Progressive playback and native HLS are retained for platforms without Media Source support. On a fatal request, the transport records the current position, resolves fresh signed sources, and loads the renewed primary at that position before trying another protocol. The position is also retained across a final Dioxus media-element remount.

`Sabr` sources are accepted when the URL exposes a playable DASH/HLS relay or when the application installs a `window.TawnySabrAdapter`; raw YouTube UMP remains isolated behind that explicit adapter because it is not a media URL. The native adapter must follow the current stateful SABR contract rather than treating the endpoint like an ordinary segment URL: increment `rn` on every request, use the requested segment start as `playerTimeMs`, advertise selected formats and buffered ranges, carry playback cookies and SABR contexts forward, obey server backoff and redirects, and renew the per-session PO token when protection status requires it. The parsed UMP media headers and payloads then feed the platform media-buffer implementation without exposing Googlevideo URLs to the UI.

## Feature roadmap

The current slice covers the main navigation and interaction model. LibreTube parity should be added in these bounded layers:

1. authentication and operation-log sync;
2. comment replies and richer search suggestions/filters;
3. native SABR/UMP adapter and background playback controls;
4. SponsorBlock, Return YouTube Dislike, DeArrow, and live chat;
5. downloads and a larger database-backed offline cache;
6. import/export, notification controls, and privacy settings.

Keeping those integrations behind source/provider traits prevents volatile YouTube details from spreading through the Dioxus UI.
