use crate::{
    control_page::ControlPage,
    error_template::{AppError, ErrorTemplate},
    home_page::HomePage,
    Event, Player,
};
use indexmap::IndexMap;
use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use leptos_use::{core::ConnectionReadyState, use_websocket, UseWebsocketReturn};
use std::rc::Rc;

#[derive(Clone)]
pub struct WebsocketContext {
    pub message: Signal<Option<Vec<u8>>>,
    send: Rc<dyn Fn(Vec<u8>)>, // use Rc to make it easily cloneable
    open: Rc<dyn Fn()>,
    pub ready_state: Signal<ConnectionReadyState>,
}

impl WebsocketContext {
    pub fn new(
        message: Signal<Option<Vec<u8>>>,
        send: Rc<dyn Fn(Vec<u8>)>,
        open: Rc<dyn Fn()>,
        ready_state: Signal<ConnectionReadyState>,
    ) -> Self {
        Self {
            message,
            send,
            open,
            ready_state,
        }
    }

    // create a method to avoid having to use parantheses around the field
    #[inline(always)]
    pub fn send(&self, message: Vec<u8>) {
        (self.send)(message)
    }

    // create a method to avoid having to use parantheses around the field
    #[inline(always)]
    pub fn open(&self) {
        (self.open)()
    }
}

#[derive(Clone)]
pub struct BaseUrl(String);

impl core::fmt::Display for BaseUrl {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    let (ws_url, set_ws_url) = create_signal(String::new());

    create_effect(move |_| {
        let location = window().location();

        let protocol = location.protocol().unwrap();
        let base_ws_url = format!(
            "{}//{}",
            if protocol == "http:" { "ws:" } else { "wss:" },
            location.host().unwrap()
        );

        set_ws_url.set_untracked(base_ws_url);
    });

    let UseWebsocketReturn {
        message_bytes,
        send_bytes,
        ready_state,
        open,
        ..
    } = use_websocket(&format!("{}/ws", ws_url.get_untracked()));

    provide_context(WebsocketContext::new(
        message_bytes,
        Rc::new(send_bytes.clone()),
        Rc::new(open.clone()),
        ready_state,
    ));

    view! {
        <Stylesheet id="leptos" href="/pkg/strim-overlay.css"/>

        <Title text="Alo"/>

        <Router fallback=|| {
            let mut outside_errors = Errors::default();
            outside_errors.insert_with_default_key(AppError::NotFound);
            view! { <ErrorTemplate outside_errors/> }.into_view()
        }>
            <main>
                <Routes>
                    <Route path="" view=HomePage/>
                    <Route path="/control" view=ControlPage/>
                </Routes>
            </main>
        </Router>
    }
}

pub fn handle_websocket_message(
    websocket: WebsocketContext,
    owner: Owner,
    set_players: WriteSignal<IndexMap<String, Player>>,
) {
    if let Some(message) = websocket.message.get() {
        match bincode::deserialize::<Event>(&message).unwrap() {
            Event::AllPlayers(incoming_players) => {
                leptos::with_owner(owner, || {
                    let local_players = incoming_players
                        .into_iter()
                        .map(|(n, p)| (n, Player::from(p)))
                        .collect();
                    set_players.set(local_players);
                });
            }
            Event::NewPlayer(player) => {
                leptos::with_owner(owner, || {
                    let player = Player::from(player);
                    set_players.update(|players| {
                        players.insert(player.name.get_untracked(), player);
                    });
                });
            }
            Event::PositionUpdated {
                player_name,
                new_position,
            } => set_players.update(|players| {
                if let Some(player) = players.get_mut(&player_name) {
                    player.position.set(new_position);
                }
            }),
            Event::SizeUpdated {
                player_name,
                new_width,
                new_height,
            } => set_players.update(|players| {
                if let Some(player) = players.get_mut(&player_name) {
                    player.width.set(new_width);
                    player.height.set(new_height);
                }
            }),
            Event::PlayerDeleted { player_name } => set_players.update(|players| {
                players.shift_remove(&player_name);
            }),
            Event::PlayerMovedUp { player_name } => set_players.update(|players| {
                logging::log!("moving {player_name} up");
                if let Some(s) = players.get_index_of(&player_name) {
                    players.swap_indices(s, s - 1);
                }
            }),
            Event::PlayerMovedDown { player_name } => set_players.update(|players| {
                logging::log!("moving {player_name} down");
                if let Some(s) = players.get_index_of(&player_name) {
                    players.swap_indices(s, s + 1);
                }
            }),
            Event::FlipPlayerHorizontally {
                player_name,
                is_flipped,
            } => set_players.update(|players| {
                if let Some(player) = players.get_mut(&player_name) {
                    player
                        .horizontal_flip
                        .update(|flipped| *flipped = is_flipped);
                }
            }),
            Event::Pong => {}
        }
    }
}
