mod api;
mod app;
mod cache;
mod components;
mod models;
#[cfg(feature = "server")]
mod server;
mod state;

use app::App;

#[cfg(not(feature = "server"))]
const SERVER_URL: Option<&str> = option_env!("SERVER_URL");

#[cfg(feature = "server")]
fn main() {
    dioxus::serve(|| async move {
        use dioxus::server::axum::Extension;
        use dioxus::server::axum::routing::get;

        let state = server::AppServerState::initialize()
            .await
            .expect("initialize Tawny server");
        server::spawn_subscription_poller(state.clone());

        Ok(dioxus::server::router(App)
            .route(
                "/api/v1/playback/proxy/{token}",
                get(server::playback_proxy),
            )
            .route(
                "/api/v1/websub/youtube",
                get(server::youtube_websub_verify).post(server::youtube_websub_notification),
            )
            .layer(Extension(state)))
    });
}

#[cfg(not(feature = "server"))]
fn main() {
    g3_ui::init_auto_mode();
    dioxus::fullstack::set_server_url(SERVER_URL.unwrap_or("http://localhost:8080"));
    dioxus::launch(App);
}
