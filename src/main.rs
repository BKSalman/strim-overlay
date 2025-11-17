use indexmap::IndexMap;
use strim_overlay::server::ssr::websocket;
use tower_http::compression::CompressionLayer;

cfg_if::cfg_if! {
    if #[cfg(feature = "ssr")] {
        use axum::{
            routing::{get},
            Router,
        };
        use strim_overlay::shell;
        use leptos_axum::{generate_route_list, LeptosRoutes};
        use leptos::config::get_configuration;
        use strim_overlay::{AppState};

        #[tokio::main]
        async fn main() {
            let conf = get_configuration(None).unwrap();
            let leptos_options = conf.leptos_options;
            let addr = leptos_options.site_addr;
            let routes = generate_route_list(strim_overlay::app::App);

            let (sender, _receiver) = tokio::sync::broadcast::channel::<(u32, strim_overlay::Event)>(1024);

            let state = AppState {
                leptos_options,
                players: std::sync::Arc::new(tokio::sync::RwLock::new(IndexMap::new())),
                broadcaster: sender,
            };

            let app = Router::new()
                .route("/ws", get(websocket))
                 .leptos_routes(&state, routes, {
                    let leptos_options = state.leptos_options.clone();
                    move || shell(leptos_options.clone())
                })
                .route_layer(CompressionLayer::new().gzip(true))
                .fallback(leptos_axum::file_and_error_handler::<AppState, _>(shell))
                .with_state(state);

            let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
            tracing::info!("listening on http://{}", &addr);
            axum::serve(listener, app.into_make_service())
                .await
                .unwrap();
        }
    } else if #[cfg(not(feature = "ssr"))] {
        #[cfg(not(feature = "ssr"))]
        pub fn main() {
            // no client-side main function
            // unless we want this to work with e.g., Trunk for a purely client-side app
            // see lib.rs for hydration function instead
        }
    }
}
