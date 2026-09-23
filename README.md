# Tawny

Tawny is a calm, local-first YouTube client built with Dioxus and `g3-ui`. The same client targets Android, iOS, web, macOS, Windows, and Linux, with a SurrealDB-backed server that watches subscribed channels for new uploads.

Tawny is designed as a self-hosted replacement for a LibreTube plus Piped deployment. The Dioxus backend performs search, feeds, channels, video details, comments, captions, and stream orchestration directly against YouTube; its deployment-local yt-dlp and PO-token sidecars are included in this repository, and no public Piped instance is contacted.

This repository currently contains a working local-first vertical slice:

- cached subscription feed with All, Videos, Shorts, and Live filters;
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
- responsive mobile, web, and desktop navigation using `g3-ui`;
- server-side playback resolution for SABR, HLS, DASH, progressive streams, and per-video PO tokens.

The seeded library is intentional: it keeps the first launch useful while the real library hydrates. Library actions update the device cache immediately and then write the newer revision to SurrealDB; if another client already has a newer revision, its server snapshot wins.

## Development

```sh
npm install
docker compose up -d --build
dx serve
```

The default Compose project starts only Tawny's server dependencies:

- SurrealDB 3.2 with a persistent SurrealKV named volume, published on `127.0.0.1:8001`;
- a pinned bgutil PO-token provider, published on `127.0.0.1:4416`;
- a pinned yt-dlp sidecar with its plugin and Node challenge runtime, published on `127.0.0.1:8090`.

Tawny itself is deliberately omitted from the default profile, so `dx serve`, `dx serve --android`, and the other Dioxus development targets continue to own the app build and port 8080. Copy `.env.example` to `.env` once; its host-facing URLs already match this layout. Nothing needs to be installed in `%APPDATA%/yt-dlp`, and the host does not need a yt-dlp executable or provider plugin.

For a server deployment, configure the public URL, database password, and WebSub secret in `.env`, then include the production profile:

```sh
docker compose --profile production up -d --build
```

That command builds and runs Tawny plus the same three dependencies. The production image builds the Dioxus full-stack web bundle, includes the project-local Shaka asset, stores app state in `tawny-data`, and talks to the other containers over the Compose network. See [docs/docker.md](docs/docker.md) for configuration, health checks, updates, and volume backups.

To deploy without building, use [`compose.prod.yml`](compose.prod.yml), which pulls both images from GHCR instead. Copy [`.env.prod.template`](.env.prod.template) to `.env.prod`, or paste the same variables into a Portainer stack. See [docs/deploy.md](docs/deploy.md).

The pinned Shaka Player package supplies the cross-platform DASH/HLS Media Source transport. Dioxus packages its compiled browser runtime as a local app asset; playback never depends on a third-party CDN.

Tawny's server connects to the SurrealDB endpoint in `SURREALDB_HOST`. The checked-in Compose stack supplies a persistent SurrealDB container for development and production, while server tests use an in-memory database. `TAWNY_DATA_DIR` controls Tawny's extractor cache and other app-owned server data. See [database/README.md](database/README.md) for scoped local SurrealKit operator commands.

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

Playback is resolved server-side from direct YouTube extraction, which preserves every indexed audio/video representation including codec, bitrate, duration, quality, and DASH initialization/index byte ranges.

Stream URLs come from the Compose-managed yt-dlp service and are matched to the extractor's streams by itag. The sidecar owns yt-dlp, its bgutil plugin, and the Node runtime used for YouTube JavaScript challenges; it requests video-bound GVS PO tokens from the adjacent provider. Raw media URLs are replaced with short-lived same-origin proxy URLs before reaching the client. The player turns the representation set into a local DASH manifest and uses its packaged adaptive engine for automatic quality selection, buffering, seeking, retry recovery, DASH, and HLS. A fatal media request snapshots the current timestamp and renews the active playback session first, then tries protocol fallbacks, without replaying from zero.

### PO-token provider setup

There is no separate host installation step. `docker compose up -d --build` starts the provider and the extractor service together, and the extractor health check verifies both the installed plugin and the provider's `/ping` endpoint. Development Tawny uses `TAWNY_YTDLP_SERVICE_URL=http://127.0.0.1:8090`; production Compose replaces that with the internal `http://extractor:8080` address. `TAWNY_YTDLP_BIN` and `TAWNY_PO_TOKEN_PROVIDER_URL` remain supported only as a legacy non-Compose fallback.

The privacy-enhanced YouTube embed is never substituted automatically. If every direct source fails, the player reports the transport's actual error and offers a retry; switching to the embed is an explicit user action.

The media element lives in the shared app shell: it is full-width on a video route and changes into a compact bar above bottom navigation while the user explores the rest of the app, without being unmounted. Playback speed, captions, chapter seeking, and picture-in-picture operate on that same element. Set `TAWNY_BOTGUARD_BIN` to a compatible `rustypipe-botguard` executable for PO-token-backed browser-client streams; the extractor caches session tokens while content-bound data stays ephemeral. A `Sabr` source is played through a playable DASH/HLS bridge or a registered `TawnySabrAdapter`; raw UMP parsing and BotGuard execution remain behind that adapter boundary rather than being mislabeled as ordinary media playback.

Tawny is not affiliated with or endorsed by YouTube. YouTube trademarks belong to their respective owners.
