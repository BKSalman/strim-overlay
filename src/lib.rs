#[cfg(feature = "ssr")]
use axum::extract::FromRef;
#[cfg(feature = "ssr")]
use leptos_router::RouteListing;
use std::collections::VecDeque;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub url: RwSignal<String>,
    pub position: RwSignal<Position>,
}

impl From<ServerPlayer> for Player {
    fn from(value: ServerPlayer) -> Self {
        Self {
            url: RwSignal::new(value.url),
            position: RwSignal::new(value.position),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerPlayer {
    pub url: String,
    pub position: Position,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct Position {
    x: i32,
    y: i32,
}

impl Position {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "ssr", derive(FromRef))]
pub struct AppState {
    #[cfg(feature = "ssr")]
    pub players: Arc<RwLock<VecDeque<ServerPlayer>>>,
    pub leptos_options: LeptosOptions,
    #[cfg(feature = "ssr")]
    pub routes: Vec<RouteListing>,
}

/// Messages from frontend to backend
#[derive(Debug, Serialize, Deserialize)]
pub enum Message {
    SetPosition {
        player_idx: usize,
        new_position: Position,
    },
    GetAllPlayers,
    NewPlayer {
        src_url: String,
        position: Position,
    },
}

/// Events from backend to frontend
#[derive(Debug, Serialize, Deserialize)]
pub enum Event {
    AllPlayers(VecDeque<ServerPlayer>),
    NewPlayer {
        src_url: String,
        position: Position,
    },
    PositionUpdated {
        player_idx: usize,
        new_position: Position,
    },
}
