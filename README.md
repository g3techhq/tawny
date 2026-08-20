# Tawny

Tawny is a calm, local-first YouTube client built with Dioxus and `g3_ui`. The same client targets Android, iOS, web, macOS, Windows, and Linux, with a SurrealDB-backed server that watches subscribed channels for new uploads.

Tawny is a complete replacement for a LibreTube plus Piped deployment. The Dioxus backend performs every job a Piped instance would — search, feeds, channels, video details, comments, captions, and stream resolution — by extracting from YouTube directly. There is no companion backend to run and no third-party instance is contacted.

This repository currently contains a working local-first vertical slice:

- cached subscription feed with all/unwatched/today filters;
- real YouTube search for videos and channels with continuation paging, request coalescing, and a five-minute result cache;
- full channel pages with Videos, Shorts, Live, refresh, and continuation paging;
- subscription management with direct extraction, official RSS fallback, and YouTube WebSub callbacks;
- local playlists, creation, detail views, and offline persistence;
- revisioned playlist, subscription, queue, history, and watch-state sync;
- configurable swipe-left and swipe-right playlist destinations;
- channel pages, a persistent play queue, watch history, and a full video action sheet;
- a persistent player that expands on video pages and becomes a mini-player above navigation elsewhere, with one-tap speed controls;
- subscription groups with one-tap feed filters;
- direct YouTube video details, comments, captions, chapters, recommendations, and expiring playback sources;
- cached descriptions, chapter jumps, captions, comments, and related videos;
- same-origin ranged media/caption proxying, HLS/DASH manifest rewriting, adaptive quality, buffered seeking, and session retry;
- responsive mobile, web, and desktop navigation using `g3_ui`;
- in-process playback resolution for SABR, HLS, DASH, progressive streams, and per-video PoTokens.

The seeded library is intentional: it keeps the first launch useful while the real library hydrates. Library actions update the device cache immediately and then write the newer revision to SurrealDB; if another client already has a newer revision, its server snapshot wins.

## Development

```sh
npm install
dx serve
```

The pinned Shaka Player package supplies the cross-platform DASH/HLS Media Source transport. Dioxus packages its compiled browser runtime as a local app asset; playback never depends on a third-party CDN.

Tawny defaults to an embedded, persistent SurrealDB RocksDB database in the platform data directory. Set `TAWNY_DATA_DIR` to override that location, `SURREALDB_HOST=mem://` for disposable development data, or the `SURREALDB_*` variables for a separate database deployment.

No Piped instance — public or self-hosted — is required or contacted. Tawny extracts public YouTube search results, channel tabs, RSS feeds, video metadata, comments, captions, and player data directly. Search coalesces concurrent requests and caches identical query/filter pairs for five minutes. Results are normalized and deduplicated in SurrealDB. Video details use a two-hour server cache with stale fallback; ephemeral stream URLs and PO tokens are never persisted.

Feed ingestion is designed for large libraries. When WebSub is active the server rotates lightweight official RSS checks through the 48 stalest channels instead of fully extracting every subscription; without that accelerated source, RSS covers the complete library with bounded concurrency. Full Videos/Shorts/Live extraction happens when a channel is opened or an individual channel is newly subscribed. Atom timestamps and relative labels such as `2 hours ago` are normalized into one chronological sort key, and the client re-sorts cached feed entries on render for migration safety.

### Running on Android

For the Android emulator, run the normal Dioxus command:

```bash
dx serve --android
```

The Android client and development media proxy default to
`http://127.0.0.1:8080`, which matches Dioxus's generated network-security
policy and the emulator's `adb reverse` mapping. Playback URLs, captions, and
session refreshes are all rebased to that API origin inside the Android
WebView. The media proxy also supplies the CORS, private-network, resource, and
range headers needed by Shaka.

Do not substitute `localhost`: Android's policy entries match literal
hostnames, and only `127.0.0.1` is permitted for cleartext development
traffic. Set `SERVER_URL` and `TAWNY_PUBLIC_URL` explicitly when the client and
server use a different reachable HTTPS address, such as a physical device
connecting to a home-lab server.

For push updates, deploy with a public HTTPS `TAWNY_PUBLIC_URL` (or explicit `TAWNY_WEBSUB_CALLBACK_URL`). Subscribing requests a YouTube WebSub lease; signed callbacks insert the upload immediately and enrich only that video. Lease requests are persisted, renewed after three days, and processed with bounded concurrency. The 15-minute RSS reconciliation worker remains as a recovery path for missed callbacks.

Useful verification commands:

```sh
cargo check --features web
cargo check --no-default-features --features server
cargo test --no-default-features --features server
cargo test --no-default-features --features server live_youtube_search_subscribe_channel_and_feed_round_trip -- --ignored --nocapture --test-threads=1
dx build --platform web
```

See [docs/architecture.md](docs/architecture.md) for the data flow, cache strategy, playback model, and implementation roadmap.

## Current playback behavior

Playback is resolved in-process from direct YouTube extraction, which preserves every indexed audio/video representation including codec, bitrate, duration, quality, and DASH initialization/index byte ranges.

Stream URLs come from a local `yt-dlp` binary when one is available (`TAWNY_YTDLP_BIN`, otherwise `yt-dlp` on PATH), matched to the extracted streams by itag. Current YouTube media URLs increasingly require a video-bound GVS PO token; without one they can return 403 partway through the file even though extraction initially succeeds. For reliable playback, install a yt-dlp PO-token provider plugin, run its bgutil HTTP provider, and set `TAWNY_PO_TOKEN_PROVIDER_URL` to its base URL (the provider's usual local value is `http://127.0.0.1:4416`). Tawny then asks yt-dlp for an mweb URL bound to the provider token. Without a configured provider, yt-dlp's default client remains a best-effort fallback. Raw media URLs are replaced with short-lived same-origin proxy URLs before reaching the client. The player turns the representation set into a local DASH manifest and uses its packaged adaptive engine for automatic quality selection, buffering, seeking, retry recovery, DASH, and HLS. A fatal media request snapshots the current timestamp and renews the active playback session first, then tries protocol fallbacks, without replaying from zero.

### PO-token provider setup

The provider is a server-side companion process, not an Android/iOS dependency. Tawny checks its `/ping` endpoint before each extraction and only selects the token-backed mweb client while the provider is healthy. If it is offline, the server logs that state and keeps yt-dlp's default best-effort client.

1. Install the [`bgutil-ytdlp-pot-provider` plugin](https://github.com/Brainicism/bgutil-ytdlp-pot-provider). A pip/pipx yt-dlp installation can use `python -m pip install -U bgutil-ytdlp-pot-provider`. For a standalone `yt-dlp.exe`, download `bgutil-ytdlp-pot-provider.zip` from the latest provider release and place the zip in `%APPDATA%\yt-dlp\plugins\`.
2. Run the HTTP provider. With Docker: `docker run --name bgutil-provider --restart unless-stopped -d --init -p 4416:4416 brainicism/bgutil-ytdlp-pot-provider:latest`. The provider repository also documents a native Node.js/Deno setup.
3. Set `TAWNY_PO_TOKEN_PROVIDER_URL=http://127.0.0.1:4416` in the Tawny server's environment and restart the server. If Tawny runs in a container, use the provider's container/service address instead of loopback.
4. Verify the plugin with `yt-dlp -v https://www.youtube.com/watch?v=dQw4w9WgXcQ`; its debug output should list an external `bgutil:http` PO-token provider.

The privacy-enhanced YouTube embed is never substituted automatically. If every direct source fails, the player reports the transport's actual error and offers a retry; switching to the embed is an explicit user action.

The media element lives in the shared app shell: it is full-width on a video route and changes into a compact bar above bottom navigation while the user explores the rest of the app, without being unmounted. Playback speed, captions, chapter seeking, and picture-in-picture operate on that same element. Set `TAWNY_BOTGUARD_BIN` to a compatible `rustypipe-botguard` executable for PO-token-backed browser-client streams; the extractor caches session tokens while content-bound data stays ephemeral. A `Sabr` source is played through a playable DASH/HLS bridge or a registered `TawnySabrAdapter`; raw UMP parsing and BotGuard execution remain behind that adapter boundary rather than being mislabeled as ordinary media playback.

Tawny is not affiliated with or endorsed by YouTube. YouTube trademarks belong to their respective owners.
