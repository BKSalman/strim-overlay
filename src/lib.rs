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
pub enum MediaType {
    Text,
    Image,
    Video,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub name: RwSignal<String>,
    pub data: RwSignal<String>,
    pub media_type: MediaType,
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
            data: RwSignal::new(value.data),
            media_type: value.media_type,
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
    pub data: String,
    pub media_type: MediaType,
    pub position: Position,
    pub width: i32,
    /// None means this should keep the aspect ratio of the player and set the height to auto
    pub height: Option<i32>,
    horizontal_flip: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    x: i32,
    y: i32,
}

impl Position {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

impl std::ops::Sub for Position {
    type Output = Position;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::Output {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl std::ops::Add for Position {
    type Output = Position;

    fn add(self, rhs: Self) -> Self::Output {
        Self::Output {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl std::ops::Mul<f32> for Position {
    type Output = Position;

    fn mul(self, rhs: f32) -> Self::Output {
        Self::Output {
            x: (self.x as f32 * rhs) as i32,
            y: (self.y as f32 * rhs) as i32,
        }
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
    NewMedia {
        name: String,
        data: String,
        media_type: MediaType,
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
