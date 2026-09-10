# Docker Compose

Tawny has one Compose file for both development dependencies and a complete
server deployment. The default profile intentionally leaves the Tawny app out
because Dioxus builds it on the host during development.

## What is included

| Service | Development host address | Purpose |
| --- | --- | --- |
| `surrealdb` | `http://127.0.0.1:8001` | Persistent library, feed, sync, and cache data |
| `pot-provider` | `http://127.0.0.1:4416` | Video-bound bgutil PO-token generation |
| `yt-dlp` | `http://127.0.0.1:8090` | yt-dlp, its provider plugin, Node, and the EJS challenge runtime |
| `tawny` | `http://127.0.0.1:8080` | Production-profile Dioxus full-stack server and web client |

The published dependency ports bind to loopback only. Containers use their
Compose service names internally. The Tawny port is public so a reverse proxy
can route to it; use HTTPS at the proxy for WebSub callbacks and remote apps.

## Development

Create the local configuration once and start the dependencies:

```sh
cp .env.example .env
docker compose up -d --build
docker compose ps
```

On PowerShell, use `Copy-Item .env.example .env` for the first command. Then run
the appropriate Dioxus command separately, for example `dx serve --android`.
The checked-in example points the host Tawny server at SurrealDB port 8001 and
the yt-dlp service on port 8090. No yt-dlp, Python plugin, Node provider, or
SurrealDB installation is required on the host.

Useful checks:

```sh
docker compose logs -f yt-dlp pot-provider surrealdb
curl http://127.0.0.1:8090/health
curl http://127.0.0.1:4416/ping
```

## Production

At minimum, replace these `.env` values before exposing the server:

```dotenv
SURREALDB_PASSWORD=a-long-random-password
TAWNY_PUBLIC_URL=https://tawny.example
TAWNY_WEBSUB_CALLBACK_URL=https://tawny.example/api/v1/websub/youtube
TAWNY_WEBSUB_SECRET=a-random-64-character-secret
```

Build and run the complete stack:

```sh
docker compose --profile production up -d --build
docker compose --profile production ps
```

The Tawny image is multi-stage: Node installs the pinned browser transport,
Rust/Dioxus builds the full-stack web release, and only the release output and
runtime libraries enter the final image. The source tree's sibling development
crate patches live in `.cargo/config.toml`, which `.dockerignore` keeps out of
the image. `g3-ui`, `g3-route-transitions` and `g3-native-plugins` are git
dependencies pinned to tags - none of the three is published to crates.io - so a
standalone clone builds without repositories elsewhere on the machine.

Place a TLS reverse proxy in front of port 8080. `TAWNY_PUBLIC_URL` must be the
public HTTPS origin for proxy URLs and WebSub lease callbacks. If the explicit
callback variable is omitted, Tawny derives it from a non-loopback public URL.

## Deploying from GHCR

The compose file above builds from source, which a home lab has no reason to do.
`.github/workflows/publish-images.yml` builds the two images a deployment needs
and pushes them to GHCR:

- `ghcr.io/g3techhq/tawny` - the server and its web bundle
- `ghcr.io/g3techhq/tawny-yt-dlp` - the extraction sidecar, which `compose.yaml`
  otherwise builds locally

Run it from the Actions tab ("Publish images"), or push a `v*` tag. A manual run
publishes `edge` plus the commit SHA and accepts an optional extra tag; a tag
push publishes that version. Only `linux/amd64` is built - the Tawny image
compiles Rust and a WASM bundle, and doing that for arm64 under QEMU takes hours.

Deploy with `compose.deploy.yaml`, which pulls both images and builds nothing.
In Portainer, add it as a stack from this repository (or paste its contents) and
set the environment variables there:

```dotenv
SURREALDB_PASSWORD=a-long-random-password
TAWNY_PUBLIC_URL=https://tawny.example
TAWNY_IMAGE_TAG=edge
```

The first two have no defaults on purpose - the stack refuses to start without
them rather than running with the password `root` or building playback URLs
against the wrong origin. `TAWNY_IMAGE_TAG` defaults to `edge`; pin it to a
version tag once you are cutting releases.

Unlike `compose.yaml`, only Tawny publishes a port. SurrealDB, the PO-token
provider and the yt-dlp sidecar reach each other over the compose network and
are not exposed - development binds them to `127.0.0.1` only so a host
`dx serve` can reach them.

To update, pull and recreate:

```sh
docker compose -f compose.deploy.yaml pull
docker compose -f compose.deploy.yaml up -d
```

If the images are private, the deploying host needs `docker login ghcr.io` with
a token carrying `read:packages` first. Making the packages public in the
repository's package settings avoids that.

## State, updates, and backups

`tawny_surrealdb-data` contains the SurrealDB RocksDB files and
`tawny_tawny-data` contains extractor caches and the generated WebSub secret.
Compose does not delete either volume during ordinary stop, restart, or image
updates. Do not use `docker compose down -v` unless the library should be
erased.

Versions are pinned through `.env` defaults (`SURREALDB_VERSION`,
`YTDLP_VERSION`, and `POT_PROVIDER_VERSION`). Update one pin at a time, rebuild,
and verify playback before deploying it:

```sh
docker compose build yt-dlp
docker compose --profile production build tawny
docker compose --profile production up -d
```

SurrealDB defaults to a 2 GB container memory ceiling so RocksDB does not size
its block cache against all memory assigned to Docker Desktop. Override
`SURREALDB_MEMORY_LIMIT` for a larger server after measuring the workload.

For a consistent backup, stop the stack and archive both named volumes with a
container-based volume backup tool. Restore into empty volumes using the same
image versions, then start the stack and verify `/api/v1/health`.

## Legacy host mode

When `TAWNY_YTDLP_SERVICE_URL` is absent, Tawny can still run a binary from
`TAWNY_YTDLP_BIN` (or `PATH`) and can point that binary at
`TAWNY_PO_TOKEN_PROVIDER_URL`. This exists for diagnostics and migration; the
Compose sidecar is the supported self-contained setup.
