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
        loop {
            match socket.recv().await {
                Some(Ok(Message::Binary(bytes))) => {
                    match bincode::deserialize::<OverlayMessage>(&bytes) {
                        Ok(message) => {
                            match message {
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

                                    let event = bincode::serialize(&Event::PositionUpdated {
                                        player_idx: i,
                                        new_position: player.position.clone(),
                                    })
                                    .unwrap();
                                    let _ = socket.send(Message::Binary(event)).await;
                                }
                                OverlayMessage::NewPlayer { src_url, position } => {
                                    // TODO: get the video from the `src_url`
                                    if let Ok(url) = url::Url::parse(&src_url) {
                                        logging::log!("received twitch clip");

                                        if !matches!(url.host_str(), Some("twitch.tv"))
                                            && !matches!(url.host_str(), Some("www.twitch.tv"))
                                            && !matches!(url.host_str(), Some("clips.twitch.tv"))
                                        {
                                            logging::log!(
                                                "not a twitch link: {:?}",
                                                url.host_str()
                                            );
                                            continue;
                                        }

                                        let Some(clip_id) = (match url.path_segments() {
                                            Some(segments) => segments.last(),
                                            None => continue,
                                        }) else {
                                            continue;
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
                                                logging::error!(
                                                    "faild to get clip access token: {e}"
                                                );
                                                continue;
                                            }
                                        };

                                        let Some(data) = response.get("data") else {
                                            logging::error!("no data");
                                            continue;
                                        };

                                        let Some(clip) = data.get("clip") else {
                                            logging::error!("no clip");
                                            continue;
                                        };

                                        let Some(mut video_qualities) =
                                            (match clip.get("videoQualities") {
                                                Some(vq) => vq.as_array().map(|a| a.to_owned()),
                                                None => {
                                                    logging::error!(
                                                "Clip has no video qualities, deleted possibly?"
                                            );
                                                    continue;
                                                }
                                            })
                                        else {
                                            continue;
                                        };

                                        video_qualities.sort_by_key(|q| {
                                            q["quality"].as_str().unwrap().to_string()
                                        });

                                        let Some(playback_access_token) =
                                            clip.get("playbackAccessToken")
                                        else {
                                            logging::error!("Invalid Clip, deleted possibly?");
                                            continue;
                                        };

                                        let download_link = match video_qualities.iter().next() {
                                            Some(vq) => {
                                                vq["sourceURL"].as_str().unwrap().to_string()
                                            }
                                            None => {
                                                logging::error!("no video qualities");
                                                continue;
                                            }
                                        };

                                        logging::log!("download link: {download_link}");

                                        let form =
                                            url::form_urlencoded::Serializer::new(String::new())
                                                .append_pair(
                                                    "token",
                                                    playback_access_token["value"]
                                                        .as_str()
                                                        .unwrap(),
                                                )
                                                .finish();

                                        let src_url = format!(
                                            "{download_link}?sig={signature}&{form}",
                                            signature = playback_access_token["signature"]
                                                .as_str()
                                                .unwrap(),
                                        );
                                        logging::log!("src url: {src_url}");

                                        let player = ServerPlayer {
                                            url: src_url,
                                            position,
                                        };

                                        logging::log!("adding new player {player:?}");

                                        let event = bincode::serialize(&Event::NewPlayer {
                                            src_url: player.url.clone(),
                                            position: player.position.clone(),
                                        })
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
                            }
                        }
                        Err(e) => logging::error!("{e}"),
                    }
                }
                Some(Ok(Message::Close(close_frame))) => {
                    logging::log!("Closing websocket: {close_frame:?}");
                    break;
                }
                _ => {
                    continue;
                }
            }
        }
    }
}

#[server(IncrementCounter, "/api/counter")]
pub async fn increment_counter() -> Result<String, ServerFnError> {
    let state = use_context::<AppState>().unwrap();
    use crate::AppState;
    use leptos::*;

    Ok(format!("{state:?}"))
}
