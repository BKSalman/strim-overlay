use indexmap::IndexMap;
use leptos::prelude::*;
use leptos_use::{core::ConnectionReadyState, use_interval_fn};

use crate::{
    Message, Player,
    app::{WebsocketContext, handle_websocket_message},
};

#[component]
pub fn HomePage() -> impl IntoView {
    view! { <Players/> }
}

#[component]
fn Players() -> impl IntoView {
    // let owner = leptos::Owner::current().expect("there should be an owner");
    let (players, set_players) = signal(IndexMap::<String, Player>::new());
    let websocket = expect_context::<WebsocketContext>();

    {
        let websocket = websocket.clone();
        use_interval_fn(
            move || {
                websocket.send(&bincode::serialize(&Message::Ping).unwrap());
            },
            5000,
        );
    }
    {
        let websocket = websocket.clone();
        Effect::new(move |_| {
            if let ConnectionReadyState::Open = websocket.ready_state.get() {
                websocket.send(&bincode::serialize(&Message::GetAllPlayers).unwrap());
            }
        });
    }

    {
        let websocket = websocket.clone();
        Effect::new(move |_| {
            if let ConnectionReadyState::Open = websocket.ready_state.get() {
                handle_websocket_message(websocket.clone(), set_players.clone());
            }
        });
    }

    {
        let websocket = websocket.clone();
        Effect::new(move |_| {
            if let ConnectionReadyState::Closed = websocket.ready_state.get() {
                websocket.open();
            }
        });
    }

    view! {
        <For
            each=move || players().into_iter().rev()
            key=|(name, _)| name.clone()
            children=move |(_name, player): (String, Player)| {
                view! {
                    <div
                        style="position: absolute; z-index: 2; box-sizing: border-box;"
                        style:left=move || { format!("{}px", player.position.get().x) }

                        style:top=move || { format!("{}px", player.position.get().y) }

                        style:width=move || format!("{}px", player.width.get())
                        style:height=move || {
                            if let Some(height) = player.height.get() {
                                format!("{}px", height)
                            } else {
                                String::from("auto")
                            }
                        }

                        style:transform=move || {
                            if player.horizontal_flip.get() { "scaleX(-1)" } else { "" }
                        }
                    >

                        {move || {
                            match player.media_type {
                                crate::MediaType::Text => {
                                    view! {
                                        <div
                                            style="width: 100%; height: 100%;"
                                            style:font-size=move || {
                                                format!("{}px", (player.width.get() / 5) as f32)
                                            }
                                        >
                                            <span>{move || player.data.get()}</span>
                                        </div>
                                    }
                                        .into_any()
                                }
                                crate::MediaType::Image => {
                                    view! {
                                        <img
                                            style="width: 100%; height: 100%;"
                                            src=player.data.get()
                                        />
                                    }
                                        .into_any()
                                }
                                crate::MediaType::Video => {
                                    view! {
                                        <video
                                            style="width: 100%; height: 100%;"
                                            autoplay
                                            loop
                                            src=player.data.get()
                                        ></video>
                                    }
                                        .into_any()
                                }
                            }
                        }}

                    </div>
                }
            }
        />
    }
}
