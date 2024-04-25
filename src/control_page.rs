use std::collections::VecDeque;

use base64::Engine;
use leptos::{
    ev::MouseEvent,
    html::{Div, Input},
    *,
};
use leptos_use::{
    core::ConnectionReadyState, storage::use_local_storage, use_element_size, use_event_listener,
    use_window, utils::JsonCodec, UseElementSizeReturn,
};
use serde::{Deserialize, Serialize};

use crate::{
    app::{handle_websocket_message, WebsocketContext},
    server::is_authorized,
    Message, Player, Position,
};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct ScreenSize {
    width: i32,
    height: i32,
}

impl Default for ScreenSize {
    fn default() -> Self {
        Self {
            width: 2560,
            height: 1440,
        }
    }
}

#[component]
pub fn ControlPage() -> impl IntoView {
    let websocket = expect_context::<WebsocketContext>();
    websocket.send(bincode::serialize(&Message::GetAllPlayers).unwrap());

    // auth (I guess)
    let (authorized, set_authorized) = create_signal(false);
    let (access_token, set_access_token, _) =
        use_local_storage::<Option<String>, JsonCodec>("access_token");

    create_effect(move |_| {
        let location = use_window().as_ref().unwrap().location();
        if let Ok(hash) = location.hash() {
            if hash.is_empty() {
                return;
            }

            if let Some(token) = hash
                .split("&")
                .find_map(|s| s.strip_prefix("#access_token="))
            {
                set_access_token(Some(token.to_string()));
            }
            let _ = location.set_hash("");
        }
    });

    create_effect(move |_| {
        if let Some(access_token) = access_token() {
            spawn_local(async move {
                let is_mod = is_authorized(access_token).await.is_ok_and(|is| is);
                if is_mod {
                    set_authorized(true);
                } else {
                    set_authorized(false);
                }
            });
        }
    });

    let (show_menu, set_show_menu) = create_signal(false);

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
    let (canvas_zoom, set_canvas_zoom) = create_signal(1.0f32);

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
                current_pos.0 += (event.movement_x() as f32 / canvas_zoom()) as i32;
                current_pos.1 += (event.movement_y() as f32 / canvas_zoom()) as i32;
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

    let _ = use_event_listener(use_window(), leptos::ev::wheel, move |event| {
        if event.delta_y() > 0. {
            set_canvas_zoom.update(|current_zoom| {
                if *current_zoom - 0.1 > 0. {
                    *current_zoom -= 0.1;
                }
            });
        } else {
            set_canvas_zoom.update(|current_zoom| {
                *current_zoom += 0.1;
            });
        }
    });

    view! {
        <Show
            when=move || authorized()
            fallback=|| view! {
                <a href="https://id.twitch.tv/oauth2/authorize?response_type=token&client_id=48mas39k4vcamtq5fy33r7qegf13l9&redirect_uri=http://localhost:3030/control&scope=user%3Aread%3Amoderated_channels&force_verify=true">Authorize</a>
            }
        >
            {move || {
                if show_menu() {
                    view! { <Menu canvas_position canvas_zoom/> }.into_view()
                } else {
                    view! {}.into_view()
                }
            }}
            <Players canvas_position canvas_zoom ctrl_pressed/>
        </Show>
    }
}

#[component]
fn Players(
    canvas_position: ReadSignal<(i32, i32)>,
    canvas_zoom: ReadSignal<f32>,
    ctrl_pressed: ReadSignal<bool>,
) -> impl IntoView {
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
                pos.x += (event.movement_x() as f32 / canvas_zoom()) as i32;
                pos.y += (event.movement_y() as f32 / canvas_zoom()) as i32;
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
                            format!("{}px", (player.position.get().x + canvas_position().0) as f32 * canvas_zoom())
                        }

                        style:top=move || {
                            format!("{}px", (player.position.get().y + canvas_position().1) as f32 * canvas_zoom())
                        }

                        style:width=move || format!("{}px", player.width.get() as f32 * canvas_zoom())
                        style:height=move || {
                            if let Some(height) = player.height.get() {
                                format!("{}px", height as f32 * canvas_zoom())
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
fn Menu(canvas_position: ReadSignal<(i32, i32)>, canvas_zoom: ReadSignal<f32>) -> impl IntoView {
    let websocket = expect_context::<WebsocketContext>();
    let (screen_size, set_screen_size) = create_signal(ScreenSize::default());

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
                        if file.type_() != "video/webm" && !file.type_().starts_with("image") {
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

                                let src = format!("data:{};base64,{base64}", file.type_());

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
        <input type="file" accept="video/webm,image/*" node_ref=input_element/>
        <button on:click={
            let get_all_players = get_all_players.clone();
            move |_| get_all_players()
        }>"All players"</button>
        <div
            style="z-index: -5000; border: 3px solid black; position: absolute;"
            style:width=move || format!("{}px", screen_size().width as f32 * canvas_zoom())
            style:height=move || format!("{}px", screen_size().height as f32 * canvas_zoom())
            style:left=move || format!("{}px", canvas_position().0 as f32 * canvas_zoom())
            style:top=move || format!("{}px", canvas_position().1 as f32 * canvas_zoom())
        >
            <p>"Screen border"</p>
            <label for="height">"Height"</label>
            <input
                id="height"
                type="number"
                value=move || screen_size().height
                on:input=move |event| {
                    if let Ok(new_height) = event_target_value(&event).parse::<i32>() {
                        set_screen_size.update(|size| size.height = new_height);
                    }
                }
            />

            <label for="width">"Width"</label>
            <input
                id="width"
                type="number"
                value=move || screen_size().width
                on:input=move |event| {
                    if let Ok(new_width) = event_target_value(&event).parse::<i32>() {
                        set_screen_size.update(|size| size.width = new_width);
                    }
                }
            />
        </div>
    }
}
