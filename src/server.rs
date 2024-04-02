use leptos::*;

#[cfg(feature = "ssr")]
pub mod ssr {
    use crate::{Event, Message as OverlayMessage, ServerPlayer};
    use axum::extract::{ws::Message, State};
    use leptos::*;

    use crate::AppState;

    pub async fn websocket(
        State(state): State<AppState>,
        ws: axum::extract::WebSocketUpgrade,
    ) -> axum::response::Response {
        ws.on_upgrade(move |ws| handle_socket(ws, state))
    }

    async fn handle_socket(mut socket: axum::extract::ws::WebSocket, state: AppState) {
        let mut broadcast_receiver = state.broadcaster.subscribe();
        let mut skip_event = None;
        loop {
            tokio::select! {
                Ok(event) = broadcast_receiver.recv() => {
                    #[allow(unused)]
                    if matches!(skip_event, Some(ref event)) {
                        skip_event = None;
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

                                        let _ = state.broadcaster.send(event.clone());
                                        skip_event = Some(event.clone());

                                        let event = bincode::serialize(&event).unwrap();
                                        let _ = socket.send(Message::Binary(event)).await;
                                    }
                                    OverlayMessage::NewPlayer { src_url, position } => {
                                        if src_url == "sugoi.webm" {
                                            let player = ServerPlayer {
                                                url: src_url,
                                                position,
                                            };

                                            logging::log!("adding new player {player:?}");

                                            let event = Event::NewPlayer {
                                                src_url: player.url.clone(),
                                                position: player.position.clone(),
                                            };

                                            let _ = state.broadcaster.send(event.clone());
                                            skip_event = Some(event.clone());

                                            let event = bincode::serialize(&event).unwrap();
                                            let _ = socket.send(Message::Binary(event)).await;
                                            state.players.write().await.push_back(player);
                                            continue;
                                        }
                                        if let Ok(url) = url::Url::parse(&src_url) {
                                            let src_url = match url.host_str() {
                                                Some("twitch.tv")
                                                | Some("www.twitch.tv")
                                                | Some("clips.twitch.tv") => {
                                                    logging::log!("received twitch clip");

                                                    twitch_clip_src_url(url).await
                                                }
                                                _ => {
                                                    logging::log!(
                                                        "not a supported URL: {:?}",
                                                        url.host_str()
                                                    );
                                                    None
                                                }
                                            };

                                            let Some(src_url) = src_url else {
                                                continue;
                                            };

                                            logging::log!("src url: {src_url}");

                                            let player = ServerPlayer {
                                                url: src_url,
                                                position,
                                            };

                                            logging::log!("adding new player {player:?}");

                                            let event = Event::NewPlayer {
                                                src_url: player.url.clone(),
                                                position: player.position.clone(),
                                            };

                                            let _ = state.broadcaster.send(event.clone());
                                            skip_event = Some(event.clone());

                                            let event = bincode::serialize(&event)
                                            .unwrap();
                                            let _ = socket.send(Message::Binary(event)).await;
                                            state.players.write().await.push_back(player);
                                        }
                                    }
                                    OverlayMessage::GetAllPlayers => {
                                        logging::log!("Received request for all players");
                                        let event = bincode::serialize(&Event::AllPlayers(
                                            state.players.read().await.clone(),
                                        ))
                                        .unwrap();
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

    async fn twitch_clip_src_url(url: url::Url) -> Option<String> {
        let Some(clip_id) = (match url.path_segments() {
            Some(segments) => segments.last(),
            None => return None,
        }) else {
            return None;
        };
        logging::log!("clip id: {clip_id}");
        let client = reqwest::Client::default();
        let body = serde_json::json!({
            "operationName": "VideoAccessToken_Clip",
            "variables": {
                "slug": clip_id
            },
            "extensions": {
                "persistedQuery": {
                    "version": 1,
                    "sha256Hash": "36b89d2507fce29e5ca551df756d27c1cfe079e2609642b4390aa4c35796eb11"
                }
            }
        });
        let response = match client
            .post("https://gql.twitch.tv/gql")
            // FIXME: this is shamelessly stolen from https://github.com/lay295/TwitchDownloader/blob/8144d31ffbd048b9a0ef09a1f8343b185cb9412b/TwitchDownloaderCore/TwitchHelper.cs#L142
            .header("Client-ID", "kimne78kx3ncx6brgo4mv6wki5h1ko")
            .json(&body)
            .send()
            .await
            .unwrap()
            .json::<serde_json::Value>()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                logging::error!("faild to get clip access token: {e}");
                return None;
            }
        };
        let Some(data) = response.get("data") else {
            logging::error!("no data");
            return None;
        };
        let Some(clip) = data.get("clip") else {
            logging::error!("no clip");
            return None;
        };
        let Some(mut video_qualities) = (match clip.get("videoQualities") {
            Some(vq) => vq.as_array().map(|a| a.to_owned()),
            None => {
                logging::error!("Clip has no video qualities, deleted possibly?");
                return None;
            }
        }) else {
            return None;
        };
        video_qualities.sort_by_key(|q| q["quality"].as_str().unwrap().to_string());
        let Some(playback_access_token) = clip.get("playbackAccessToken") else {
            logging::error!("Invalid Clip, deleted possibly?");
            return None;
        };
        let download_link = match video_qualities.iter().next() {
            Some(vq) => vq["sourceURL"].as_str().unwrap().to_string(),
            None => {
                logging::error!("no video qualities");
                return None;
            }
        };
        logging::log!("download link: {download_link}");
        let form = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("token", playback_access_token["value"].as_str().unwrap())
            .finish();
        let src_url = format!(
            "{download_link}?sig={signature}&{form}",
            signature = playback_access_token["signature"].as_str().unwrap(),
        );
        Some(src_url)
    }
}

#[server(IncrementCounter, "/api/counter")]
pub async fn increment_counter() -> Result<String, ServerFnError> {
    let state = use_context::<AppState>().unwrap();
    use crate::AppState;
    use leptos::*;

    Ok(format!("{state:?}"))
}
