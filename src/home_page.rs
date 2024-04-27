use std::collections::VecDeque;

use leptos::*;
use leptos_use::core::ConnectionReadyState;

use crate::{
    app::{handle_websocket_message, WebsocketContext},
    Message, Player,
};

#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <Players/>
    }
}

#[component]
fn Players() -> impl IntoView {
    let owner = leptos::Owner::current().expect("there should be an owner");
    let (players, set_players) = create_signal(VecDeque::<Player>::new());
    let websocket = expect_context::<WebsocketContext>();

    {
        let websocket = websocket.clone();
        create_effect(move |_| {
            if let ConnectionReadyState::Open = websocket.ready_state.get() {
                websocket.send(bincode::serialize(&Message::GetAllPlayers).unwrap());
            }
        });
    }

    {
        let websocket = websocket.clone();
        create_effect(move |_| {
            if let ConnectionReadyState::Open = websocket.ready_state.get() {
                handle_websocket_message(websocket.clone(), owner, set_players.clone());
            }
        });
    }

    {
        let websocket = websocket.clone();
        create_effect(move |_| {
            if let ConnectionReadyState::Closed = websocket.ready_state.get() {
                websocket.open();
            }
        });
    }

    view! {
        <For
            each=move || players().into_iter().enumerate()
            key=|(i, _)| *i
            children=move |(_i, player): (usize, Player)| {
                view! {
                    <div
                        style="position: absolute; z-index: 2; box-sizing: border-box;"
                        style:left=move || {
                            format!("{}px", player.position.get().x)
                        }

                        style:top=move || {
                            format!("{}px", player.position.get().y)
                        }

                        style:width=move || format!("{}px", player.width.get())
                        style:height=move || {
                            if let Some(height) = player.height.get() {
                                format!("{}px", height)
                            } else {
                                String::from("auto")
                            }
                        }
                    >
                        {move || {
                            let file_type = player.file_type.get();
                            if file_type.starts_with("video") {
                                view! {
                                    <video
                                        style="width: 100%; height: 100%;"
                                        autoplay
                                        loop
                                        src=player.url.get()
                                    ></video>
                                }
                                    .into_view()
                            } else if file_type.starts_with("image") {
                                view! {
                                    <img
                                        style="width: 100%; height: 100%;"
                                        autoplay
                                        loop
                                        src=player.url.get()
                                    />
                                }
                                    .into_view()
                            } else {
                                view! {}.into_view()
                            }
                        }}

                    </div>
                }
            }
        />
    }
}
