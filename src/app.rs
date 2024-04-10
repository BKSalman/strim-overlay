use std::collections::VecDeque;

use crate::{
    error_template::{AppError, ErrorTemplate},
    Event, Message, Player, Position,
};
use base64::Engine;
use leptos::{
    ev::MouseEvent,
    html::{Div, Input},
    *,
};
use leptos_meta::*;
use leptos_router::*;
use leptos_use::{
    core::ConnectionReadyState,
    storage::use_local_storage,
    use_element_size, use_event_listener, use_websocket, use_window,
    utils::{FromToStringCodec, JsonCodec},
    UseElementSizeReturn, UseWebsocketReturn,
};
use serde::{Deserialize, Serialize};
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
    } = use_websocket("ws://127.0.0.1:3030/ws");

    provide_context(WebsocketContext::new(
        message_bytes,
        Rc::new(send_bytes.clone()),
        ready_state,
    ));

    view! {
        <Stylesheet id="leptos" href="/pkg/strim-overlay.css"/>

        // sets the document title
        <Title text="Alo"/>

        // content for this welcome page
        <Router fallback=|| {
            let mut outside_errors = Errors::default();
            outside_errors.insert_with_default_key(AppError::NotFound);
            view! { <ErrorTemplate outside_errors/> }.into_view()
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

    let (show_menu, set_show_menu, _) = use_local_storage::<bool, FromToStringCodec>("show_menu");

    let _ = use_event_listener(use_window(), leptos::ev::contextmenu, move |event| {
        event.prevent_default();
    });

    let _ = use_event_listener(use_window(), leptos::ev::keyup, move |event| {
        if event.key() == "Escape" {
            set_show_menu.update(|show| *show = !*show);
        }
    });

    let (canvas_move_click, set_canvas_move_click) = create_signal(false);
    let (canvas_position, set_canvas_position) = create_signal((0, 0));

    let _ = use_event_listener(use_window(), leptos::ev::mousedown, move |event| {
        // 1 = middle mouse button, aka. wheel button
        if event.button() == 1 {
            event.prevent_default();
            set_canvas_move_click(true);
        }
    });

    let _ = use_event_listener(use_window(), leptos::ev::mousemove, move |event| {
        if canvas_move_click() {
            event.prevent_default();
            set_canvas_position.update(|current_pos| {
                current_pos.0 += event.movement_x();
                current_pos.1 += event.movement_y();
            });
        }
    });

    let _ = use_event_listener(use_window(), leptos::ev::mouseup, move |event| {
        if event.button() == 1 {
            event.prevent_default();
            set_canvas_move_click(false);
        }
    });

    let _ = use_event_listener(use_window(), leptos::ev::mouseleave, move |event| {
        event.prevent_default();
        set_canvas_move_click(false);
    });

    view! {
        {move || {
            if show_menu() {
                view! { <Menu canvas_position/> }.into_view()
            } else {
                view! {}.into_view()
            }
        }}

        <Players canvas_position/>
    }
}

#[component]
fn Players(canvas_position: ReadSignal<(i32, i32)>) -> impl IntoView {
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

    {
        let websocket = websocket.clone();
        create_effect(move |_| {
            handle_websocket_message(websocket.clone(), owner, set_players.clone());
        });
    }

    let (ctrl_pressed, set_ctrl_pressed) = create_signal(false);

    let _ = use_event_listener(use_window(), leptos::ev::keydown, move |event| {
        if event.ctrl_key() {
            set_ctrl_pressed(true);
        }
    });

    let _ = use_event_listener(use_window(), leptos::ev::keyup, move |event| {
        if !event.ctrl_key() {
            set_ctrl_pressed(false);
        }
    });

    let set_position = {
        let websocket = websocket.clone();
        move |player_idx: usize, x: i32, y: i32| {
            let message = Message::SetPosition {
                player_idx,
                new_position: Position { x, y },
            };
            websocket.send(bincode::serialize(&message).unwrap());
        }
    };

    let send_set_size = move |player_idx: usize, width: i32, height: Option<i32>| {
        let message = Message::SetSize {
            player_idx,
            width,
            height,
        };
        websocket.send(bincode::serialize(&message).unwrap());
    };

    let div = create_node_ref::<Div>();

    let UseElementSizeReturn { width, height } = use_element_size(div);

    let (initial_xy, set_initial_xy) = create_signal((0., 0.));
    let (initial_size, set_initial_size) = create_signal((200., 200.));

    let move_mouse = move |width: RwSignal<i32>,
                           height: RwSignal<Option<i32>>,
                           position: RwSignal<Position>,
                           i: usize,
                           event: leptos::ev::MouseEvent| {
        event.prevent_default();

        let set_position = set_position.clone();
        let send_set_size = send_set_size.clone();
        event.prevent_default();
        if move_click() {
            position.update(|pos| {
                pos.x += event.movement_x();
                pos.y += event.movement_y();
                set_position(i, pos.x, pos.y);
            });
        } else if resize_click() {
            let initial = initial_size();
            width.update(|current_width| {
                *current_width = (initial.0 + event.client_x() as f64 - initial_xy().0) as i32;
            });
            height.update(|current_height| {
                if ctrl_pressed() {
                    // None means this should keep the aspect ratio of the player and set the height to auto
                    *current_height = None;
                } else {
                    *current_height =
                        Some((initial.1 + event.client_y() as f64 - initial_xy().1) as i32);
                }
            });
            send_set_size(i, width.get_untracked(), height.get_untracked());
        }
    };

    // TODO: add canvas zoom
    // for now the browser zoom is not a bad solution
    // let _ = use_event_listener(use_window(), leptos::ev::wheel, move |event| {
    //     if event.delta_y() > 0. {
    //         set_canvas_zoom.update_untracked(|current_zoom| {
    //             *current_zoom += 0.01;
    //         });
    //     } else {
    //         set_canvas_zoom.update_untracked(|current_zoom| {
    //             *current_zoom -= 0.01;
    //         });
    //     }
    // });

    let start_moving_player = move |event: MouseEvent| {
        event.prevent_default();

        if event.button() == 0 {
            set_move_click(true);
        } else if event.button() == 2 {
            // 2 = right click
            set_resize_click(true);
            set_initial_size((width.get_untracked(), height.get_untracked()));
            set_initial_xy((event.client_x() as f64, event.client_y() as f64));
        }
    };

    view! {
        <For
            each=move || players().into_iter().enumerate()
            key=|(i, _)| *i
            children=move |(i, player): (usize, Player)| {
                view! {
                    <div
                        on:mousedown=start_moving_player
                        on:mousemove={
                            let move_mouse = move_mouse.clone();
                            move |event| {
                                move_mouse(player.width, player.height, player.position, i, event)
                            }
                        }

                        on:mouseup=move |_event| {
                            set_move_click(false);
                            set_resize_click(false);
                        }

                        on:mouseleave=move |_| {
                            set_move_click(false);
                            set_resize_click(false);
                        }

                        style="position: absolute; z-index: 2; box-sizing: border-box;"
                        style:left=move || {
                            format!("{}px", player.position.get().x + canvas_position().0)
                        }

                        style:top=move || {
                            format!("{}px", player.position.get().y + canvas_position().1)
                        }

                        style:width=move || format!("{}px", player.width.get())
                        style:height=move || {
                            if let Some(height) = player.height.get() {
                                format!("{}px", height)
                            } else {
                                String::from("auto")
                            }
                        }

                        style:border=move || {
                            if resize_click() || move_click() {
                                format!("3px solid black")
                            } else {
                                String::new()
                            }
                        }

                        node_ref=div
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

#[component]
fn Menu(canvas_position: ReadSignal<(i32, i32)>) -> impl IntoView {
    let websocket = expect_context::<WebsocketContext>();
    let (screen_size, set_screen_size, _) =
        use_local_storage::<ScreenSize, JsonCodec>("screen_width");

    let (src_url, set_src_url) = create_signal(String::new());

    let new_player = {
        let websocket = websocket.clone();
        move |src_url, file_type, x, y, width, height| {
            websocket.send(
                bincode::serialize(&Message::NewPlayer {
                    src_url,
                    file_type,
                    position: Position::new(x, y),
                    width,
                    height,
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

    let input_element: NodeRef<Input> = create_node_ref();
    let on_file_submit = {
        let new_player = new_player.clone();
        move || {
            if let Some(files) = input_element.get().unwrap().files() {
                for i in 0..files.length() {
                    if let Some(file) = files.item(i) {
                        if file.type_() != "video/webm" && file.type_() != "image/gif" {
                            continue;
                        }
                        let new_player = new_player.clone();
                        spawn_local(async move {
                            if let Ok(file_data) =
                                wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await
                            {
                                let data =
                                    wasm_bindgen_futures::js_sys::Uint8Array::new(&file_data)
                                        .to_vec();
                                let base64 =
                                    base64::engine::general_purpose::STANDARD.encode(&data);

                                let src = if file.type_() == "video/webm" {
                                    format!("data:video/webm;base64,{base64}")
                                } else if file.type_() == "image/gif" {
                                    format!("data:image/gif;base64,{base64}")
                                } else {
                                    return;
                                };

                                new_player(src, file.type_(), 100, 100, 500, None);
                            }
                        });
                    }
                }
            }
        }
    };

    view! {
        <h1>{move || format!("State: {}", websocket.ready_state.get())}</h1>
        <button on:click={
            let on_file_submit = on_file_submit.clone();
            move |_event| on_file_submit()
        }>"New player"</button>
        <input type="file" accept="video/webm,image/gif" node_ref=input_element/>
        <input
            on:input=move |event| {
                let value = event_target_value(&event);
                set_src_url(value);
            }

            value=move || src_url()
        />
        <button on:click={
            let get_all_players = get_all_players.clone();
            move |_| get_all_players()
        }>"All players"</button>
        <div
            style="z-index: -5000; border: 3px solid black; position: absolute;"
            style:width=move || format!("{}px", screen_size().width)
            style:height=move || format!("{}px", screen_size().height)
            style:left=move || format!("{}px", canvas_position().0)
            style:top=move || format!("{}px", canvas_position().1)
        >
            <p>"Screen border"</p>
            <label for="height">"Height"</label>
            <input
                id="height"
                type="number"
                value=move || screen_size().height
                on:input=move |event| {
                    set_screen_size
                        .update(|size| {
                            size.height = event_target_value(&event).parse::<i32>().unwrap();
                        })
                }
            />

            <label for="width">"Width"</label>
            <input
                id="width"
                type="number"
                value=move || screen_size().width
                on:input=move |event| {
                    set_screen_size
                        .update(|size| {
                            size.width = event_target_value(&event).parse::<i32>().unwrap();
                        })
                }
            />

        </div>
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct ScreenSize {
    width: i32,
    height: i32,
}

impl Default for ScreenSize {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
        }
    }
}

fn handle_websocket_message(
    websocket: WebsocketContext,
    owner: Owner,
    set_players: WriteSignal<VecDeque<Player>>,
) {
    if let Some(message) = websocket.message.get() {
        match bincode::deserialize::<Event>(&message).unwrap() {
            Event::AllPlayers(incoming_players) => {
                leptos::with_owner(owner, || {
                    let local_players = incoming_players
                        .into_iter()
                        .map(|p| Player::from(p))
                        .collect();
                    set_players.set(local_players);
                });
            }
            Event::NewPlayer(player) => {
                leptos::with_owner(owner, || {
                    let player = Player::from(player);
                    set_players.update(|players| {
                        players.push_back(player);
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
            }),
            Event::SizeUpdated {
                player_idx,
                new_width,
                new_height,
            } => set_players.update(|players| {
                let (_, player) = players
                    .iter_mut()
                    .enumerate()
                    .find(|(i, _p)| *i == player_idx)
                    .unwrap();

                player.width.set(new_width);
                player.height.set(new_height);
            }),
        }
    }
}
