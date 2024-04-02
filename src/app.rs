use std::collections::VecDeque;

use crate::{
    error_template::{AppError, ErrorTemplate},
    Event, Message, Player, Position,
};
use leptos::{html::Div, *};
use leptos_meta::*;
use leptos_router::*;
use leptos_use::{
    core::ConnectionReadyState, use_element_size, use_websocket, UseElementSizeReturn,
    UseWebsocketReturn,
};
use std::rc::Rc;

#[derive(Clone)]
pub struct WebsocketContext {
    pub message: Signal<Option<Vec<u8>>>,
    send: Rc<dyn Fn(Vec<u8>)>, // use Rc to make it easily cloneable
    pub ready_state: Signal<ConnectionReadyState>,
}

impl WebsocketContext {
    pub fn new(
        message: Signal<Option<Vec<u8>>>,
        send: Rc<dyn Fn(Vec<u8>)>,
        ready_state: Signal<ConnectionReadyState>,
    ) -> Self {
        Self {
            message,
            send,
            ready_state,
        }
    }

    // create a method to avoid having to use parantheses around the field
    #[inline(always)]
    pub fn send(&self, message: Vec<u8>) {
        (self.send)(message)
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    let UseWebsocketReturn {
        message_bytes,
        send_bytes,
        ready_state,
        ..
    } = use_websocket("ws://localhost:3030/ws");

    provide_context(WebsocketContext::new(
        message_bytes,
        Rc::new(send_bytes.clone()),
        ready_state,
    ));

    view! {
        // injects a stylesheet into the document <head>
        // id=leptos means cargo-leptos will hot-reload this stylesheet
        <Stylesheet id="leptos" href="/pkg/strim-overlay.css"/>

        // sets the document title
        <Title text="Alo"/>

        // content for this welcome page
        <Router fallback=|| {
            let mut outside_errors = Errors::default();
            outside_errors.insert_with_default_key(AppError::NotFound);
            view! {
                <ErrorTemplate outside_errors/>
            }
            .into_view()
        }>
            <main>
                <Routes>
                    <Route path="" view=HomePage/>
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn HomePage() -> impl IntoView {
    let websocket = expect_context::<WebsocketContext>();
    websocket.send(bincode::serialize(&Message::GetAllPlayers).unwrap());

    let (src_url, set_src_url) = create_signal(String::new());

    let new_player = {
        let websocket = websocket.clone();
        move |src_url, x, y| {
            websocket.send(
                bincode::serialize(&Message::NewPlayer {
                    src_url,
                    position: Position::new(x, y),
                })
                .unwrap(),
            );
        }
    };

    let get_all_players = {
        let websocket = websocket.clone();
        move || {
            websocket.send(bincode::serialize(&Message::GetAllPlayers).unwrap());
        }
    };

    view! {
        <h1>{move || format!("State: {}", websocket.ready_state.get())}</h1>
        <button on:click={
            let new_player = new_player.clone();
            move |_| new_player(src_url(), 100, 100)
        }>"New player"</button>
        <input on:input=move |event| {
            let value = event_target_value(&event);
            logging::log!("changed: {value}");
            set_src_url(value);
        } value=move || src_url()/>
        <button on:click={
            let new_player = new_player.clone();
            move |_| new_player(String::from("sugoi.webm"), 200, 200)
        }>"New player 200"</button>
        <button on:click=move |_| get_all_players()
        >"All players"</button>
        <Players/>
    }
}

#[component]
fn Players() -> impl IntoView {
    let owner = leptos::Owner::current().expect("there should be an owner");
    let (players, set_players) = create_signal(VecDeque::<Player>::new());
    let (move_click, set_move_click) = create_signal(false);
    let (resize_click, set_resize_click) = create_signal(false);

    let websocket = expect_context::<WebsocketContext>();

    {
        let websocket = websocket.clone();
        create_effect(move |_| match websocket.ready_state.get() {
            ConnectionReadyState::Open => {
                websocket.send(bincode::serialize(&Message::GetAllPlayers).unwrap());
                logging::log!("sending GetAllPlayers");
            }
            _ => {}
        });
    }

    create_effect(move |_| {
        if let Some(message) = websocket.message.get() {
            match bincode::deserialize::<Event>(&message).unwrap() {
                Event::AllPlayers(incoming_players) => {
                    leptos::with_owner(owner, || {
                        let local_players = incoming_players
                            .into_iter()
                            .map(|p| Player {
                                url: RwSignal::new(p.url),
                                position: RwSignal::new(p.position),
                            })
                            .collect();
                        // logging::log!("local players: {local_players:?}");
                        set_players.set(local_players);
                    });
                }
                Event::NewPlayer { src_url, position } => {
                    leptos::with_owner(owner, || {
                        let player = Player {
                            url: RwSignal::new(src_url),
                            position: RwSignal::new(position),
                        };
                        set_players.update(|players| {
                            players.push_back(player);
                            // logging::log!("players after adding new player: {players:?}");
                        });
                    });
                }
                Event::PositionUpdated {
                    player_idx,
                    new_position,
                } => set_players.update(|players| {
                    let (_, player) = players
                        .iter_mut()
                        .enumerate()
                        .find(|(i, _p)| *i == player_idx)
                        .unwrap();

                    player.position.set(new_position);
                    // logging::log!("updated player: {player:?}");
                }),
            }
        }
    });

    let set_position = move |player_idx: usize, x: i32, y: i32| {
        let message = Message::SetPosition {
            player_idx,
            new_position: Position { x, y },
        };
        websocket.send(bincode::serialize(&message).unwrap());
    };

    let div = create_node_ref::<Div>();

    let UseElementSizeReturn { width, height } = use_element_size(div);

    let (initial_xy, set_initial_xy) = create_signal((0., 0.));
    let (initial_size, set_initial_size) = create_signal((200., 200.));
    let (current_size, set_current_size) = create_signal((200., 200.));

    view! {
          <For
            each=move || players().into_iter().enumerate()
            key=|(i, _)| *i
            children=move |(i, player): (usize, Player)| {
              view! {
                <div
                on:mousedown=move |event| {
                    if event.ctrl_key() {
                        event.prevent_default();
                        set_resize_click(true);
                        set_initial_size((width.get_untracked(), height.get_untracked()));
                        set_initial_xy((event.client_x() as f64, event.client_y() as f64));
                    } else {
                        set_move_click(true);
                    }
                }
                on:mousemove={
                    let set_position = set_position.clone();
                    move |event| {
                    if move_click() {
                        player.position.update(|pos| {
                            pos.x += event.movement_x();
                            pos.y += event.movement_y();
                            set_position(i, pos.x, pos.y);
                        });
                    } else if resize_click() {
                        set_current_size.update(|current_size| {
                            let initial = initial_size();
                            current_size.0 = initial.0 + event.client_x() as f64 - initial_xy().0;
                            current_size.1 = initial.1 + event.client_y() as f64 - initial_xy().1;
                        });
                        logging::log!("{:?}", current_size.get_untracked());
                    }
                }}
                on:mouseup=move |_event| {
                    set_move_click(false);
                    set_resize_click(false);
                }
                on:mouseleave=move |_| {
                    set_move_click(false);
                    set_resize_click(false);
                }
                style="position: absolute; box-sizing: border-box;"
                style:left=move || format!("{}px", player.position.get().x)
                style:top=move || format!("{}px", player.position.get().y)
                // style:width=move || format!("{}px", current_size().0)
                // style:height=move || format!("{}px", current_size().1)
                style:border= move || {
                    if resize_click() || move_click() {
                        format!("3px solid black")
                    } else {
                        String::new()
                    }
                }
                node_ref=div
                ><video style="width: 100%; height: 100%;" autoplay loop src=player.url.get()></video></div>
              }
            }
          />
    }
}
