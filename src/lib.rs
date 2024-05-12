use indexmap::IndexMap;
use leptos::LeptosOptions;
use leptos::RwSignal;
use serde::{Deserialize, Serialize};

pub mod app;
pub mod control_page;
pub mod error_template;
pub mod home_page;
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
    pub name: RwSignal<String>,
    pub url: RwSignal<String>,
    pub file_type: RwSignal<String>,
    pub position: RwSignal<Position>,
    pub width: RwSignal<i32>,
    /// None means this should keep the aspect ratio of the player and set the height to auto
    pub height: RwSignal<Option<i32>>,
    pub is_selected: RwSignal<bool>,
    pub horizontal_flip: RwSignal<bool>,
}

impl From<ServerPlayer> for Player {
    fn from(value: ServerPlayer) -> Self {
        Self {
            name: RwSignal::new(value.name),
            url: RwSignal::new(value.url),
            file_type: RwSignal::new(value.file_type),
            position: RwSignal::new(value.position),
            width: RwSignal::new(value.width),
            height: RwSignal::new(value.height),
            is_selected: RwSignal::new(false),
            horizontal_flip: RwSignal::new(value.horizontal_flip),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerPlayer {
    pub name: String,
    pub url: String,
    pub file_type: String,
    pub position: Position,
    pub width: i32,
    /// None means this should keep the aspect ratio of the player and set the height to auto
    pub height: Option<i32>,
    horizontal_flip: bool,
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
    pub players: Arc<RwLock<IndexMap<String, ServerPlayer>>>,
    #[cfg(feature = "ssr")]
    pub broadcaster: tokio::sync::broadcast::Sender<(u32, Event)>,
    pub leptos_options: LeptosOptions,
    #[cfg(feature = "ssr")]
    pub routes: Vec<RouteListing>,
}

/// Messages from frontend to backend
#[derive(Debug, Serialize, Deserialize)]
pub enum Message {
    Ping,
    Authorize(String),
    SetPosition {
        player_name: String,
        new_position: Position,
    },
    SetSize {
        player_name: String,
        width: i32,
        height: Option<i32>,
    },
    GetAllPlayers,
    NewPlayer {
        name: String,
        src_url: String,
        file_type: String,
        position: Position,
        width: i32,
        height: Option<i32>,
    },
    DeletePlayer {
        player_name: String,
    },
    MovePlayerUp {
        player_name: String,
    },
    MovePlayerDown {
        player_name: String,
    },
    FlipPlayerHorizontally {
        player_name: String,
        is_flipped: bool,
    },
}

/// Events from backend to frontend
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Event {
    Pong,
    AllPlayers(IndexMap<String, ServerPlayer>),
    NewPlayer(ServerPlayer),
    PositionUpdated {
        player_name: String,
        new_position: Position,
    },
    SizeUpdated {
        player_name: String,
        new_width: i32,
        new_height: Option<i32>,
    },
    PlayerDeleted {
        player_name: String,
    },
    PlayerMovedDown {
        player_name: String,
    },
    PlayerMovedUp {
        player_name: String,
    },
    FlipPlayerHorizontally {
        player_name: String,
        is_flipped: bool,
    },
}
