use axum::{
    body::Body as AxumBody,
    extract::{Path, Request},
    response::IntoResponse,
};
use indexmap::IndexMap;
use leptos_axum::handle_server_fns_with_context;
use strim_overlay::server::ssr::websocket;
use tower_http::compression::CompressionLayer;

cfg_if::cfg_if! {
    if #[cfg(feature = "ssr")] {
        use axum::{
            routing::{get, post},
            Router,
        };
        use leptos::*;
        use leptos_axum::{generate_route_list, LeptosRoutes};
        use strim_overlay::{AppState, fileserv::file_and_error_handler};

        async fn server_fn_handler(
            // State(_app_state): State<AppState>,
            path: Path<String>,
            request: Request<AxumBody>,
        ) -> impl IntoResponse {
            leptos::logging::log!("{:?}", path);

            handle_server_fns_with_context(
                move || {
                    // provide_context(app_state.count.clone());
                },
                request,
            )
            .await
        }

        #[tokio::main]
        async fn main() {
            let conf = get_configuration(None).await.unwrap();
            let leptos_options = conf.leptos_options;
            let addr = leptos_options.site_addr;
            let routes = generate_route_list(strim_overlay::app::App);

            let (sender, _receiver) = tokio::sync::broadcast::channel::<(u32, strim_overlay::Event)>(1024);

            let state = AppState {
                routes: routes.clone(),
                leptos_options,
                players: std::sync::Arc::new(tokio::sync::RwLock::new(IndexMap::new())),
                broadcaster: sender,
            };

            let app = Router::new()
                .route(
                    "/api/*fn_name",
                    post(server_fn_handler).get(server_fn_handler),
                )
                .route("/ws", get(websocket))
                .leptos_routes(&state, routes, strim_overlay::app::App)
                .route_layer(CompressionLayer::new().gzip(true))
                .fallback(file_and_error_handler)
                .with_state(state);

            let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
            logging::log!("listening on http://{}", &addr);
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
