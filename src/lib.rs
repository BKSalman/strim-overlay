use std::collections::VecDeque;

use leptos::LeptosOptions;
use leptos::RwSignal;
use serde::{Deserialize, Serialize};

pub mod app;
pub mod error_template;
pub mod server;

cfg_if::cfg_if! {
    if #[cfg(feature = "ssr")] {
        use tokio::sync::RwLock;
        use std::sync::Arc;
        use leptos_router::RouteListing;
        use axum::extract::FromRef;

        pub mod fileserv;
    }
}

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
    pub file_type: RwSignal<String>,
    pub position: RwSignal<Position>,
    pub width: RwSignal<i32>,
    pub height: RwSignal<i32>,
}

impl From<ServerPlayer> for Player {
    fn from(value: ServerPlayer) -> Self {
        Self {
            url: RwSignal::new(value.url),
            file_type: RwSignal::new(value.file_type),
            position: RwSignal::new(value.position),
            width: RwSignal::new(value.width),
            height: RwSignal::new(value.height),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerPlayer {
    pub url: String,
    pub file_type: String,
    pub position: Position,
    pub width: i32,
    pub height: i32,
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
    #[cfg(feature = "ssr")]
    pub broadcaster: tokio::sync::broadcast::Sender<(u32, Event)>,
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
    SetSize {
        player_idx: usize,
        width: i32,
        height: i32,
    },
    GetAllPlayers,
    NewPlayer {
        src_url: String,
        file_type: String,
        position: Position,
        width: i32,
        height: i32,
    },
}

/// Events from backend to frontend
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Event {
    AllPlayers(VecDeque<ServerPlayer>),
    NewPlayer(ServerPlayer),
    PositionUpdated {
        player_idx: usize,
        new_position: Position,
    },
    SizeUpdated {
        player_idx: usize,
        new_width: i32,
        new_height: i32,
    },
}
