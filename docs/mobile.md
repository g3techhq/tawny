# Android, iOS and desktop

The native apps are the same Rust code built with the `mobile` or `desktop`
feature: Dioxus renders into a system webview, and the app talks to the server
over HTTP. The player's media element runs in that webview, with the same
Shaka transport as the web.

```bash
just dev-android          # emulator or a connected phone
just dev-ios              # simulator (macOS only)
just dev-desktop
```

## The backend address

A native build has no page origin, so the server it talks to is compiled in
(`SERVER_URL`) and can be changed in Settings. `src/server_url.rs` reads the
choice before `dioxus::launch`, because Dioxus 0.7 keeps the first server URL
it is given; changing it reloads the app. The player's transport fetches from
the webview directly, so it is told the chosen backend too
(`playback_server_url` in `player.rs`).

## Android

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

## Sessions

Native builds initialize the Dioxus cookie runtime before launch
(`dioxus-cookie`), so the session cookie stays outside application state. See
[authentication.md](authentication.md).

## Builds

**Build Mobile** (`.github/workflows/build-mobile.yml`) produces the Android
and iOS artifacts; see [deployment.md](deployment.md#native-mobile-artifacts).
