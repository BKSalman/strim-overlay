use leptos::*;

#[cfg(feature = "ssr")]
pub mod ssr {
    use std::collections::VecDeque;

    use crate::{Event, Message as OverlayMessage, ServerPlayer};
    use axum::extract::{ws::Message, State};
    use leptos::*;

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
                                    OverlayMessage::SetPosition {
                                        player_idx,
                                        new_position,
                                    } => {
                                        let mut lock = state.players.write().await;
                                        let (i, player) = lock
                                            .iter_mut()
                                            .enumerate()
                                            .find(|(i, _p)| *i == player_idx)
                                            .unwrap();
                                        player.position = new_position;

                                        let event = Event::PositionUpdated {
                                            player_idx: i,
                                            new_position: player.position.clone(),
                                        };

                                        // notify other clients
                                        let _ = state.broadcaster.send((socket_id, event.clone()));

                                        // NOTE: this might be better to keep commented out to make the client experience a bit better
                                        // let event = bincode::serialize(&event).unwrap();
                                        // let _ = socket.send(Message::Binary(event)).await;
                                    }
                                    OverlayMessage::NewPlayer { src_url, file_type, position, width, height } => {
                                        add_new_player(socket_id,
                                            state.broadcaster.clone(),
                                            src_url, file_type, position, width,
                                            height, &mut socket,
                                            state.players.clone()
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
                                    OverlayMessage::SetSize { player_idx, width, height } => {
                                        let mut players = state.players.write().await;
                                        let (i, player) = players.iter_mut().enumerate().find(|(i, _p)| *i == player_idx).unwrap();

                                        player.width = width;
                                        player.height = height;

                                        let event = Event::SizeUpdated {
                                            player_idx: i,
                                            new_width: player.width,
                                            new_height: player.height,
                                        };

                                        let _ = state.broadcaster.send((socket_id, event.clone()));

                                        let event = bincode::serialize(&event).unwrap();

                                        let _ = socket.send(Message::Binary(event)).await;
                                    },
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
        height: i32,
        socket: &mut axum::extract::ws::WebSocket,
        players: std::sync::Arc<tokio::sync::RwLock<VecDeque<ServerPlayer>>>,
    ) -> anyhow::Result<()> {
        if file_type != "video/webm" && file_type != "image/gif" {
            return Ok(());
        }

        let player = ServerPlayer {
            url: src_url,
            file_type,
            position,
            width,
            height,
        };
        logging::log!("adding new player: {}", player.file_type);

        let event = Event::NewPlayer(player.clone());

        let _ = broadcaster.send((socket_id, event.clone()));

        let event = bincode::serialize(&event).unwrap();
        let _ = socket.send(Message::Binary(event)).await;
        players.write().await.push_back(player);

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
