#[cfg(feature = "ssr")]
use axum::extract::FromRef;
#[cfg(feature = "ssr")]
use leptos_router::RouteListing;
use std::path::PathBuf;
#[cfg(feature = "ssr")]
use std::sync::Arc;
#[cfg(feature = "ssr")]
use tokio::sync::RwLock;

use leptos::LeptosOptions;
use leptos::RwSignal;
use serde::{Deserialize, Serialize};

pub mod app;
pub mod error_template;
#[cfg(feature = "ssr")]
pub mod fileserv;
pub mod server;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::*;
    console_error_panic_hook::set_once();
    leptos::mount_to_body(App);
}

#[derive(Clone)]
pub struct Player {
    id: i32,
    file: RwSignal<PathBuf>,
    position: RwSignal<Position>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Position {
    x: i32,
    y: i32,
}

impl Position {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Count {
    pub value: i32,
}

#[derive(Debug, Clone)]
#[cfg_attr(derive, FromRef)]
pub struct AppState {
    #[cfg(feature = "ssr")]
    pub count: Arc<RwLock<leptos_server_signal::ServerSignal<Count>>>,
    pub leptos_options: LeptosOptions,
    #[cfg(feature = "ssr")]
    pub routes: Vec<RouteListing>,
}
