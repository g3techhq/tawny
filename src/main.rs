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
// Both features re-export the same `dioxus-desktop` crate.
#[cfg(all(feature = "desktop", not(feature = "mobile")))]
use dioxus::desktop::Config as NativeConfig;
#[cfg(feature = "mobile")]
use dioxus::mobile::Config as NativeConfig;

#[cfg(feature = "server")]
fn main() {
    let _ = dotenvy::dotenv();
    dioxus::serve(|| async move {
        use std::sync::Arc;

        use axum_session::{SessionConfig, SessionLayer, SessionStore};
        use axum_session_auth::{AuthConfig, AuthSessionLayer};
        use dioxus::server::axum::extract::DefaultBodyLimit;
        use dioxus::server::axum::routing::get;
        use dioxus::server::axum::{Extension, Router};

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

        let db = Arc::clone(&state.db);
        let session_config = SessionConfig::default().with_cookie_path("/");
        let session_store = SessionStore::new(
            Some(auth::SurrealSessionPool::new(Arc::clone(&db))),
            session_config,
        )
        .await
        .expect("create Tawny session store");
        let auth_config = AuthConfig::<String>::default();
        let health_state = state.clone();

        Ok(dioxus::server::router(App)
            .route(
                "/api/v1/playback/proxy/{token}",
                get(server::playback_proxy).options(server::playback_proxy_options),
            )
            .route(
                "/api/v1/websub/youtube",
                get(server::youtube_websub_verify).post(server::youtube_websub_notification),
            )
            .layer(DefaultBodyLimit::max(LIBRARY_SYNC_BODY_LIMIT))
            .layer(Extension(state))
            .layer(
                AuthSessionLayer::<
                    auth::SessionUser,
                    String,
                    auth::SurrealSessionPool<surrealdb::engine::any::Any>,
                    Arc<surrealdb::Surreal<surrealdb::engine::any::Any>>,
                >::new(Some(db))
                .with_config(auth_config),
            )
            .layer(SessionLayer::new(session_store))
            // Probes should not allocate or load a viewer session.
            .merge(
                Router::new()
                    .route("/api/v1/health", get(server::health))
                    .layer(Extension(health_state)),
            ))
    });
}

#[cfg(not(feature = "server"))]
fn main() {
    #[cfg(any(feature = "desktop", feature = "mobile"))]
    dioxus_cookie::init();
    g3_ui::init_auto_mode();
    // Before `launch`, not after: `set_server_url` keeps only its first value.
    // The cookie runtime is initialized above so the first request can restore
    // its session. See `config` for why the URL cannot come from the usual
    // persistence helper.
    config::install();
    #[cfg(any(feature = "desktop", feature = "mobile"))]
    dioxus::LaunchBuilder::new()
        .with_cfg(NativeConfig::new().with_custom_head(app::native_head()))
        .launch(App);
    #[cfg(not(any(feature = "desktop", feature = "mobile")))]
    dioxus::launch(App);
}
