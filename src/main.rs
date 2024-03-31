use axum::{
    body::Body as AxumBody,
    extract::{Path, Request},
    response::{IntoResponse, Response},
};

async fn server_fn_handler(
    State(app_state): State<AppState>,
    path: Path<String>,
    request: Request<AxumBody>,
) -> impl IntoResponse {
    leptos::logging::log!("{:?}", path);

    handle_server_fns_with_context(
        move || {
            provide_context(app_state.count.clone());
        },
        request,
    )
    .await
}

async fn leptos_routes_handler(
    State(app_state): State<AppState>,
    req: Request<AxumBody>,
) -> Response {
    let handler = leptos_axum::render_route_with_context(
        app_state.leptos_options.clone(),
        app_state.routes.clone(),
        move || {
            provide_context(app_state.count.clone());
        },
        App,
    );
    handler(req).await.into_response()
}

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use std::sync::Arc;
    use tokio::sync::RwLock;

    use axum::{
        routing::{get, post},
        Router,
    };
    use leptos::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use leptos_server_signal::ServerSignal;
    use strim_overlay::{app::*, AppState};
    use strim_overlay::{fileserv::file_and_error_handler, Count};

    // Setting get_configuration(None) means we'll be using cargo-leptos's env values
    // For deployment these variables are:
    // <https://github.com/leptos-rs/start-axum#executing-a-server-on-a-remote-machine-without-the-toolchain>
    // Alternately a file can be specified such as Some("Cargo.toml")
    // The file would need to be included with the executable when moved to deployment
    let conf = get_configuration(None).await.unwrap();
    let leptos_options = conf.leptos_options;
    let addr = leptos_options.site_addr;
    let routes = generate_route_list(App);

    let count = ServerSignal::<Count>::new("counter").unwrap();

    let count = Arc::new(RwLock::new(count));

    let state = AppState {
        count,
        routes: routes.clone(),
        leptos_options,
    };
    // build our application with a route
    let app = Router::new()
        .route(
            "/api/*fn_name",
            post(server_fn_handler).get(server_fn_handler),
        )
        .route("/ws", get(websocket))
        .leptos_routes_with_handler(routes, get(leptos_routes_handler))
        // .leptos_routes(&leptos_options, routes, App)
        .fallback(file_and_error_handler)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    logging::log!("listening on http://{}", &addr);
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // no client-side main function
    // unless we want this to work with e.g., Trunk for a purely client-side app
    // see lib.rs for hydration function instead
}

#[cfg(feature = "ssr")]
pub async fn websocket(
    State(state): State<AppState>,
    ws: axum::extract::WebSocketUpgrade,
) -> axum::response::Response {
    use leptos::*;

    logging::log!("{state:?}");
    ws.on_upgrade(move |ws| handle_socket(state.count.clone(), ws))
}

#[cfg(feature = "ssr")]
use axum::extract::State;
use leptos::provide_context;
use leptos_axum::handle_server_fns_with_context;
use strim_overlay::{app::App, AppState};
#[cfg(feature = "ssr")]
use strim_overlay::{AppState, Count};

#[cfg(feature = "ssr")]
use std::sync::Arc;
#[cfg(feature = "ssr")]
use tokio::sync::RwLock;

#[cfg(feature = "ssr")]
async fn handle_socket(
    count: Arc<RwLock<leptos_server_signal::ServerSignal<Count>>>,
    mut socket: axum::extract::ws::WebSocket,
) {
    use std::time::Duration;

    loop {
        tokio::time::sleep(Duration::from_millis(1000)).await;
        let mut count = count.write().await;
        let result = count.with(&mut socket, |count| count.value += 1).await;
        if result.is_err() {
            break;
        }
    }
}
