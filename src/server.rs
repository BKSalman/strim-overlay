use leptos::*;

#[server]
pub async fn is_authorized(access_token: String) -> Result<bool, ServerFnError> {
    use super::server::ssr::TwitchResponse;

    let client = reqwest::Client::new();
    let res = client
        .get("https://api.twitch.tv/helix/users")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Client-Id", "48mas39k4vcamtq5fy33r7qegf13l9")
        .send()
        .await?
        .json::<TwitchResponse>()
        .await?;

    let data = &res.data[0];

    if data.get("login").unwrap().as_str().unwrap() == "sadmadladsalman" {
        return Ok(true);
    }

    let id = data.get("id").unwrap().as_str().unwrap();

    let res = client
        .get(format!(
            "https://api.twitch.tv/helix/moderation/channels?user_id={}&first=100",
            id
        ))
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Client-Id", "48mas39k4vcamtq5fy33r7qegf13l9")
        .send()
        .await?
        .json::<TwitchResponse>()
        .await?;

    Ok(res
        .data
        .iter()
        .any(|broadcaster| broadcaster.get("broadcaster_login").unwrap() == "sadmadladsalman"))
}

#[cfg(feature = "ssr")]
pub mod ssr {
    use crate::{server::is_authorized, Event, Message as OverlayMessage, ServerPlayer};
    use axum::extract::{ws::Message, State};
    use indexmap::IndexMap;
    use leptos::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    pub(crate) struct TwitchResponse {
        pub data: Vec<serde_json::Map<String, serde_json::Value>>,
    }

    use crate::AppState;

    fn next_id() -> u32 {
        static mut CURRENT_ID: u32 = 0;

        unsafe {
            let current_id = CURRENT_ID;

            CURRENT_ID += 1;

            return current_id;
        }
    }

    pub async fn websocket(
        State(state): State<AppState>,
        ws: axum::extract::WebSocketUpgrade,
    ) -> axum::response::Response {
        ws.on_upgrade(move |ws| handle_socket(ws, state))
    }

    async fn handle_socket(mut socket: axum::extract::ws::WebSocket, state: AppState) {
        let socket_id = next_id();
        let mut broadcast_receiver = state.broadcaster.subscribe();
        let mut authorized = false;
        loop {
            tokio::select! {
                Ok((sender_id, event)) = broadcast_receiver.recv() => {
                    if sender_id == socket_id {
                        continue;
                    }
                    let event = bincode::serialize(&event).unwrap();
                    let _ = socket.send(Message::Binary(event)).await;
                }
                Some(message) = socket.recv() => {
                    match message {
                        Ok(Message::Binary(bytes)) => {
                            match bincode::deserialize::<OverlayMessage>(&bytes) {
                                Ok(message) => match message {
                                    OverlayMessage::Authorize(access_token) => {
                                        #[cfg(debug_assertions)]
                                        logging::log!("received access token {access_token}");
                                        if is_authorized(access_token).await.is_ok_and(|a| a) {
                                            #[cfg(debug_assertions)]
                                            logging::log!("is authorized");
                                            authorized = true;
                                        } else {
                                            logging::log!("not authorized");
                                        }
                                    },
                                    OverlayMessage::SetPosition {
                                        player_name,
                                        new_position,
                                    } => {
                                        if !authorized {
                                            logging::log!("SetPosition: not authorized");
                                            continue;
                                        }

                                        let mut lock = state.players.write().await;
                                        let Some(player) = lock.get_mut(&player_name) else {
                                            continue;
                                        };
                                        player.position = new_position;

                                        let event = Event::PositionUpdated {
                                            player_name: player.name.clone(),
                                            new_position: player.position.clone(),
                                        };

                                        // notify other clients
                                        let _ = state.broadcaster.send((socket_id, event.clone()));

                                        // NOTE: this might be better to keep commented out to make the client experience a bit better
                                        // let event = bincode::serialize(&event).unwrap();
                                        // let _ = socket.send(Message::Binary(event)).await;
                                    }
                                    OverlayMessage::NewPlayer { name, src_url, file_type, position, width, height } => {
                                        if !authorized {
                                            logging::log!("NewPlayer not authorized");
                                            continue;
                                        }
                                        add_new_player(socket_id,
                                            state.broadcaster.clone(),
                                            src_url, file_type, position, width,
                                            height, &mut socket,
                                            state.players.clone(),
                                            name,
                                        ).await.unwrap()
                                    },
                                    OverlayMessage::GetAllPlayers => {
                                        logging::log!("Received request for all players");
                                        let event = bincode::serialize(&Event::AllPlayers(
                                            state.players.read().await.clone(),
                                        ))
                                        .unwrap();
                                        let _ = socket.send(Message::Binary(event)).await;
                                    }
                                    OverlayMessage::SetSize { player_name, width, height } => {
                                        if !authorized {
                                            logging::log!("SetSize not authorized");
                                            continue;
                                        }
                                        let mut players = state.players.write().await;
                                        let Some(player) = players.get_mut(&player_name) else {
                                            continue;
                                        };

                                        player.width = width;
                                        player.height = height;

                                        let event = Event::SizeUpdated {
                                            player_name: player.name.clone(),
                                            new_width: player.width,
                                            new_height: player.height,
                                        };

                                        let _ = state.broadcaster.send((socket_id, event.clone()));

                                        // let event = bincode::serialize(&event).unwrap();

                                        // let _ = socket.send(Message::Binary(event)).await;
                                    },
                                    OverlayMessage::DeletePlayer { player_name } => {
                                        if !authorized {
                                            logging::log!("DeletePlayer not authorized");
                                            continue;
                                        }
                                        let mut players = state.players.write().await;
                                        if players.shift_remove(&player_name).is_some() {
                                            let event = Event::PlayerDeleted { player_name };

                                            let _ = state.broadcaster.send((socket_id, event.clone()));

                                            let event = bincode::serialize(&event).unwrap();
                                            let _ = socket.send(Message::Binary(event)).await;
                                        }
                                    },
                                    OverlayMessage::MovePlayerUp { player_name } => {
                                        if !authorized {
                                            logging::log!("MovePlayerUp not authorized");
                                            continue;
                                        }
                                        let mut players = state.players.write().await;
                                        if let Some(s) = players.get_index_of(&player_name) {
                                            if s > 0 {
                                                logging::log!("moving {player_name} up");
                                                players.swap_indices(s, s - 1);
                                                let event = Event::PlayerMovedUp { player_name };

                                                let _ = state.broadcaster.send((socket_id, event.clone()));

                                                let event = bincode::serialize(&event).unwrap();
                                                let _ = socket.send(Message::Binary(event)).await;
                                            }
                                        }
                                    },
                                    OverlayMessage::MovePlayerDown { player_name } => {
                                        if !authorized {
                                            logging::log!("MovePlayerDown not authorized");
                                            continue;
                                        }
                                        let mut players = state.players.write().await;
                                        if let Some(s) = players.get_index_of(&player_name) {
                                            if players.len() > s + 1 {
                                                logging::log!("moving {player_name} down");
                                                players.swap_indices(s, s + 1);

                                                let event = Event::PlayerMovedDown { player_name };

                                                let _ = state.broadcaster.send((socket_id, event.clone()));

                                                let event = bincode::serialize(&event).unwrap();
                                                let _ = socket.send(Message::Binary(event)).await;
                                            }
                                        }
                                    },
                                    OverlayMessage::FlipPlayerHorizontally { player_name, is_flipped } => {
                                        if !authorized {
                                            logging::log!("FlipPlayerHorizontally not authorized");
                                            continue;
                                        }
                                        let mut players = state.players.write().await;
                                        let Some(player) = players.get_mut(&player_name) else {
                                            continue;
                                        };

                                        player.horizontal_flip = is_flipped;

                                        let event = Event::FlipPlayerHorizontally { player_name, is_flipped };

                                        let _ = state.broadcaster.send((socket_id, event.clone()));
                                    }
                                    OverlayMessage::Ping => {
                                        #[cfg(debug_assertions)]
                                        logging::log!("socket: {socket_id} ping");
                                        let event = bincode::serialize(&Event::Pong).unwrap();
                                        let _ = socket.send(Message::Binary(event)).await;
                                    }
                                },
                                Err(e) => logging::error!("{e}"),
                            }
                        },
                        Ok(Message::Close(close_frame)) => {
                            logging::log!("Closing websocket: {close_frame:?}");
                            break;
                        }
                        _ => break,
                    }
                }
            }
        }
    }

    async fn add_new_player(
        socket_id: u32,
        broadcaster: tokio::sync::broadcast::Sender<(u32, Event)>,
        src_url: String,
        file_type: String,
        position: crate::Position,
        width: i32,
        height: Option<i32>,
        socket: &mut axum::extract::ws::WebSocket,
        players: std::sync::Arc<tokio::sync::RwLock<IndexMap<String, ServerPlayer>>>,
        name: String,
    ) -> anyhow::Result<()> {
        if file_type != "video/webm" && !file_type.starts_with("image") {
            return Ok(());
        }

        let duplicated_count = players
            .read()
            .await
            .iter()
            .filter(|(p_name, _)| p_name.starts_with(&name))
            .count();

        logging::log!("duplicates: {duplicated_count}");

        let name = if duplicated_count > 0 {
            let mut stripped = name
                .strip_suffix(&format!("-{duplicated_count}"))
                .map(|s| s.to_string())
                .unwrap_or(name);
            stripped.push_str(&format!("-{}", duplicated_count + 1));
            stripped
        } else {
            name
        };

        let player = ServerPlayer {
            name,
            url: src_url,
            file_type,
            position,
            width,
            height,
            horizontal_flip: false,
        };
        logging::log!("adding new player: {}", player.file_type);

        players
            .write()
            .await
            .insert(player.name.clone(), player.clone());

        let event = Event::NewPlayer(player);

        let _ = broadcaster.send((socket_id, event.clone()));

        let event = bincode::serialize(&event).unwrap();
        let _ = socket.send(Message::Binary(event)).await;

        Ok(())
    }
}

#[server(IncrementCounter, "/api/counter")]
pub async fn increment_counter() -> Result<String, ServerFnError> {
    let state = use_context::<AppState>().unwrap();
    use crate::AppState;
    use leptos::*;

    Ok(format!("{state:?}"))
}
