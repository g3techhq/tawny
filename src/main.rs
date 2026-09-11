mod api;
mod app;
#[cfg(feature = "server")]
mod auth;
mod cache;
mod components;
mod config;
#[cfg(feature = "server")]
mod database;
mod models;
#[cfg(feature = "server")]
mod server;
mod session;
mod state;
mod subscriptions_io;

use app::App;

#[cfg(feature = "server")]
fn main() {
    let _ = dotenvy::dotenv();
    dioxus::serve(|| async move {
        use dioxus::server::axum::Extension;
        use dioxus::server::axum::extract::DefaultBodyLimit;
        use dioxus::server::axum::routing::get;

        /// `/api/v1/library/sync` POSTs the entire library snapshot, which grows
        /// with the cache: at 11k cached videos it is already past axum's 2 MB
        /// default, and a rejected body surfaced as a 500 from a panicking
        /// `unwrap` inside dioxus-fullstack rather than a readable error.
        ///
        /// Ordinary edits now go to `/api/v1/library/state` instead and are
        /// kilobytes, so only first load and pull-to-refresh still send a full
        /// snapshot - but those two do, and they keep growing.
        const LIBRARY_SYNC_BODY_LIMIT: usize = 64 * 1024 * 1024;

        let state = server::AppServerState::initialize()
            .await
            .expect("initialize Tawny server");
        server::spawn_subscription_poller(state.clone());

        Ok(dioxus::server::router(App)
            .route("/api/v1/health", get(server::health))
            .route(
                "/api/v1/playback/proxy/{token}",
                get(server::playback_proxy).options(server::playback_proxy_options),
            )
            .route(
                "/api/v1/websub/youtube",
                get(server::youtube_websub_verify).post(server::youtube_websub_notification),
            )
            .layer(DefaultBodyLimit::max(LIBRARY_SYNC_BODY_LIMIT))
            .layer(Extension(state)))
    });
}

#[cfg(not(feature = "server"))]
fn main() {
    g3_ui::init_auto_mode();
    // Before `launch`, not after: `set_server_url` keeps only its first value,
    // and the stored session token has to be on the very first request the app
    // makes. See `config` for why neither can come from the usual persistence
    // helper.
    config::install();
    dioxus::launch(App);
}
