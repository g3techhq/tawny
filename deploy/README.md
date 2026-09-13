# Tawny deployment

Tawny is a Rust full-stack server plus three supporting containers. GitHub
Actions publishes two image tags each for the two images Tawny itself owns,
after a successful build on `main`:

- `ghcr.io/g3techhq/tawny:latest` and `:sha-<commit>`
- `ghcr.io/g3techhq/tawny-extractor:latest` and `:sha-<commit>`

The sidecar is published too, not just the app: `compose.yaml` at the repository
root builds it from `docker/extractor`, so a pull-only deployment with only the
app image would have nothing to extract streams with.

It is named for the job rather than for yt-dlp, which is one of several things
inside it: yt-dlp itself, a Node runtime, the bgutil PO-token plugin, and a
small HTTP adapter exposing the two operations Tawny needs.

The other two services come from upstream images and need nothing published:
SurrealDB, and the bgutil PO-token provider.

## One-time GitHub setup

1. Merge the deployment workflow and let its first `main` run finish.
2. In the GitHub organization, open **Packages**, then each of **tawny** and
   **tawny-extractor**.
3. Open **Package settings** and change the package visibility to **Public**.

Public GHCR packages can be pulled without credentials. Package visibility is
separate from repository visibility, so verify this once after the first images
are published. If you keep them private, the Portainer host needs
`docker login ghcr.io` with a token carrying `read:packages`.

## Portainer stack

1. In Portainer, open the target environment and select **Stacks** → **Add stack**.
2. Give the stack a name such as `tawny`.
3. Choose **Web editor** and paste the contents of
   [`compose.yaml`](compose.yaml), or choose **Git repository** and point
   Portainer at this repository with `deploy/compose.yaml` as the Compose path.
4. Set the stack environment variables below.
5. Deploy the stack.

### Environment

Two variables have no default, and the stack refuses to start without them
rather than doing something worse quietly:

| Variable | Why it has no default |
| --- | --- |
| `SURREALDB_PASSWORD` | A deployed database with the password `root` is worse than a stack that will not start. |
| `TAWNY_PUBLIC_URL` | The origin browsers reach this instance on. Playback proxy URLs and WebSub lease callbacks are built from it, so a wrong value surfaces as broken playback rather than as a configuration error. |

Optional:

| Variable | Default | Notes |
| --- | --- | --- |
| `TAWNY_IMAGE_TAG` | `latest` | Pin to `sha-<commit>` to hold a known-good build. |
| `TAWNY_PORT` | `8080` | Host port. Only Tawny publishes one. |
| `TAWNY_WEBSUB_CALLBACK_URL` | derived | Set when the public URL is not where YouTube should call back. |
| `TAWNY_WEBSUB_SECRET` | generated | Generated and kept under the data volume if unset, which is fine as long as that volume survives. |
| `YTDLP_CONCURRENCY` | `2` | Simultaneous extractions. |

Unlike the development `compose.yaml`, only Tawny publishes a port. SurrealDB,
the PO-token provider and the extractor reach each other over the stack's network;
development binds them to `127.0.0.1` only so a host `dx serve` can reach them,
which is not a deployment need.

## Updating

`pull_policy: always` checks for a newer image when the stack is redeployed or a
container is recreated. A restart policy does not, by itself, replace a running
container when a new image is pushed - use Portainer's stack webhook or another
updater to trigger a redeploy after GitHub publishes.

From a shell instead:

```sh
docker compose -f deploy/compose.yaml pull
docker compose -f deploy/compose.yaml up -d
```

## State

Two named volumes hold everything worth keeping:

- `surrealdb-data` - the catalog, and every account's library
- `tawny-data` - extractor caches and the generated WebSub secret

Back up `surrealdb-data`. Losing `tawny-data` costs a re-warmed cache and a new
WebSub secret, which resubscribes on the next renewal.

## Reverse proxy

Put TLS in front of the published port. `TAWNY_PUBLIC_URL` must be the public
HTTPS origin: WebSub stays disabled on a loopback URL, and playback proxy URLs
are built from it.
