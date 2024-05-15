use base64::Engine;
use indexmap::IndexMap;
use leptos::{
    ev::MouseEvent,
    html::Input,
    leptos_dom::helpers::{location, location_hash},
    *,
};
use leptos_use::{
    core::ConnectionReadyState, storage::use_local_storage, use_event_listener, use_interval_fn,
    use_window, utils::JsonCodec,
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
    let (base_url, set_base_url) = create_signal(String::new());
    create_effect(move |_| {
        let location = window().location();

        set_base_url(location.origin().unwrap());
    });

    // auth (I guess)
    let (authorized, set_authorized) = create_signal(false);
    let (access_token, set_access_token, _) =
        use_local_storage::<Option<String>, JsonCodec>("access_token");

    create_effect(move |_| {
        if let Some(hash) = location_hash() {
            if hash.is_empty() {
                return;
            }

            if let Some(token) = hash
                .split("&")
                .find_map(|s| s.strip_prefix("access_token="))
            {
                set_access_token(Some(token.to_string()));
                let _ = location().set_hash("");
            }
        }
    });

    let websocket = expect_context::<WebsocketContext>();

    create_effect(move |_| {
        if let Some(access_token) = access_token() {
            spawn_local(async move {
                // authorize on client
                let authorized = is_authorized(access_token.clone()).await.is_ok_and(|is| is);
                if authorized {
                    logging::log!("authorized");
                    set_authorized(true);
                } else {
                    logging::log!("unauthorized");
                    set_authorized(false);
                }
            });
        }
    });
    {
        let websocket = websocket.clone();
        create_effect(move |_| {
            if let Some(access_token) = access_token() {
                // authorize on server
                let message = Message::Authorize(access_token);
                match websocket.ready_state.get() {
                    ConnectionReadyState::Open => {
                        websocket.send(bincode::serialize(&message).unwrap());
                    }
                    _ => {}
                }
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

    let (show_menu, set_show_menu) = create_signal(true);

    let _ = use_event_listener(use_window(), leptos::ev::contextmenu, move |event| {
        event.prevent_default();
    });

    let (canvas_move_click, set_canvas_move_click) = create_signal(false);
    let (canvas_position, set_canvas_position) = create_signal(Position { x: 0, y: 0 });
    let (canvas_zoom, set_canvas_zoom) = create_signal(1.0f32);
    let (space_pressed, set_space_pressed) = create_signal(false);
    let (ctrl_pressed, set_ctrl_pressed) = create_signal(false);

    let _ = use_event_listener(use_window(), leptos::ev::keyup, move |event| {
        if event.code() == "Escape" {
            set_show_menu.update(|show| *show = !*show);
        } else if event.code() == "Space" {
            set_space_pressed(false);
        } else if event.key() == "Control" {
            set_ctrl_pressed(false);
        }
    });

    let _ = use_event_listener(use_window(), leptos::ev::keydown, move |event| {
        if event.code() == "Space" {
            set_space_pressed(true);
        } else if event.key() == "Control" {
            set_ctrl_pressed(true);
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

    let _ = use_event_listener(use_window(), leptos::ev::mousedown, move |event| {
        // 1 = middle mouse button, aka. wheel button
        if event.button() == 1 {
            event.prevent_default();
            set_canvas_move_click(true);
        } else if event.button() == 0 && space_pressed() {
            event.prevent_default();
            set_canvas_move_click(true);
        }
    });

    let _ = use_event_listener(use_window(), leptos::ev::mousemove, move |event| {
        if canvas_move_click() {
            event.prevent_default();
            set_canvas_position.update(|current_pos| {
                current_pos.x += (event.movement_x() as f32 / canvas_zoom()) as i32;
                current_pos.y += (event.movement_y() as f32 / canvas_zoom()) as i32;
            });
        }
    });

    let _ = use_event_listener(use_window(), leptos::ev::mouseup, move |event| {
        if event.button() == 1 {
            event.prevent_default();
            set_canvas_move_click(false);
        } else if event.button() == 0 && space_pressed() {
            event.prevent_default();
            set_canvas_move_click(false);
        }
    });

    let _ = use_event_listener(use_window(), leptos::ev::mouseleave, move |event| {
        event.prevent_default();
        set_canvas_move_click(false);
    });

    let fallback_view = move || {
        let base_url = base_url.clone();
        view! {
            <a href=move || {
                format!(
                    "https://id.twitch.tv/oauth2/authorize?response_type=token&client_id=48mas39k4vcamtq5fy33r7qegf13l9&redirect_uri={}/control&scope=user%3Aread%3Amoderated_channels&force_verify=true",
                    base_url(),
                )
            }>Authorize</a>
        }
    };

    let (players, set_players) = create_signal(IndexMap::<String, Player>::new());

    view! {
        <Show when=move || authorized() fallback=fallback_view>
        <div
            style:cursor=move || {
                if canvas_move_click() { "grabbing" } else if space_pressed() { "grab" } else { "" }
            }
        >
            {move || {
                if show_menu() {
                    view! { <Menu players canvas_position canvas_zoom/> }.into_view()
                } else {
                    view! {}.into_view()
                }
            }}

            <Players players set_players canvas_position canvas_zoom ctrl_pressed authorized/>
        </div>
        </Show>
    }
}

#[component]
fn Players(
    players: ReadSignal<IndexMap<String, Player>>,
    set_players: WriteSignal<IndexMap<String, Player>>,
    canvas_position: ReadSignal<Position>,
    canvas_zoom: ReadSignal<f32>,
    ctrl_pressed: ReadSignal<bool>,
    authorized: ReadSignal<bool>,
) -> impl IntoView {
    let owner = leptos::Owner::current().expect("there should be an owner");
    let (move_click, set_move_click) = create_signal(false);
    let (resize_click, set_resize_click) = create_signal(false);

    let websocket = expect_context::<WebsocketContext>();
    {
        let websocket = websocket.clone();
        use_interval_fn(
            move || {
                websocket.send(bincode::serialize(&Message::Ping).unwrap());
            },
            5000,
        );
    }

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

    let send_set_position = {
        let websocket = websocket.clone();
        move |player_name: String, x: i32, y: i32| {
            let message = Message::SetPosition {
                player_name,
                new_position: Position { x, y },
            };
            if let ConnectionReadyState::Open = websocket.ready_state.get() {
                if authorized() {
                    websocket.send(bincode::serialize(&message).unwrap());
                }
            }
        }
    };

    let send_set_size = move |player_name: String, width: i32, height: Option<i32>| {
        let message = Message::SetSize {
            player_name,
            width,
            height,
        };
        if let ConnectionReadyState::Open = websocket.ready_state.get() {
            if authorized() {
                websocket.send(bincode::serialize(&message).unwrap());
            }
        }
    };

    let (prev_mouse_pos, set_prev_mouse_pos) = create_signal(Position { x: 0, y: 0 });

    let move_mouse = move |width: RwSignal<i32>,
                           height: RwSignal<Option<i32>>,
                           position: RwSignal<Position>,
                           name: RwSignal<String>,
                           event: leptos::ev::MouseEvent| {
        event.prevent_default();

        let send_set_position = send_set_position.clone();
        let send_set_size = send_set_size.clone();
        event.prevent_default();
        if move_click() {
            position.update(|pos| {
                let movement_x = event.x() - prev_mouse_pos().x;
                let movement_y = event.y() - prev_mouse_pos().y;
                pos.x += ((movement_x as f32) / canvas_zoom()) as i32;
                pos.y += ((movement_y as f32) / canvas_zoom()) as i32;
                send_set_position(name(), pos.x, pos.y);
                set_prev_mouse_pos(Position {
                    x: event.x(),
                    y: event.y(),
                });
            });
        } else if resize_click() {
            width.update(|current_width| {
                *current_width =
                    (*current_width as f32 + (event.movement_x() as f32 / canvas_zoom())) as i32;
            });
            height.update(|current_height| {
                if ctrl_pressed() {
                    // None means this should keep the aspect ratio of the player and set the height to auto
                    *current_height = None;
                } else {
                    *current_height = Some(
                        (current_height.unwrap_or(width()) as f32
                            + (event.movement_y() as f32 / canvas_zoom()))
                            as i32,
                    );
                }
            });
            send_set_size(name(), width.get_untracked(), height.get_untracked());
        }
    };

    view! {
        <For
            each=move || players().into_iter().rev()
            key=|(name, _)| name.clone()
            children=move |(_name, player): (String, Player)| {
                view! {
                    <div
                        on:mousedown=move |event: MouseEvent| {
                            event.prevent_default();

                            if event.button() == 0 {
                                set_move_click(true);
                                set_prev_mouse_pos(Position {
                                    x: event.x(),
                                    y: event.y(),
                                });
                            } else if event.button() == 2 {
                                // 2 = right click
                                set_resize_click(true);
                            }
                        }
                        on:mousemove={
                            let move_mouse = move_mouse.clone();
                            move |event| {
                                move_mouse(
                                    player.width,
                                    player.height,
                                    player.position,
                                    player.name,
                                    event,
                                )
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

                        style="position: absolute; z-index: 2;"
                        style:left=move || {
                            format!(
                                "{}px",
                                (player.position.get().x + canvas_position().x) as f32
                                    * canvas_zoom(),
                            )
                        }

                        style:top=move || {
                            format!(
                                "{}px",
                                (player.position.get().y + canvas_position().y) as f32
                                    * canvas_zoom(),
                            )
                        }

                        style:width=move || {
                            format!("{}px", player.width.get() as f32 * canvas_zoom())
                        }

                        style:height=move || {
                            if let Some(height) = player.height.get() {
                                format!("{}px", height as f32 * canvas_zoom())
                            } else {
                                String::from("auto")
                            }
                        }

                        style:outline=move || {
                            if resize_click() || move_click() { "3px solid black" } else { "" }
                        }

                        style:cursor=move || {
                            if resize_click() || move_click() { "move" } else { "" }
                        }

                        style:transform=move || {
                            if player.horizontal_flip.get() { "scaleX(-1)" } else { "" }
                        }
                    >

                        {move || {
                            let file_type = player.file_type.get();
                            if file_type.starts_with("video") {
                                view! {
                                    <video
                                        style="width: 100%; height: 100%;"
                                        style:outline= move || {
                                            if player.is_selected.get() {
                                                "3px solid black"
                                            } else {
                                                ""
                                            }
                                        }
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
                                        style:outline= move || {
                                            if player.is_selected.get() {
                                                "3px solid black"
                                            } else {
                                                ""
                                            }
                                        }
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
fn Menu(
    players: ReadSignal<IndexMap<String, Player>>,
    canvas_position: ReadSignal<Position>,
    canvas_zoom: ReadSignal<f32>,
) -> impl IntoView {
    let websocket = expect_context::<WebsocketContext>();
    let (screen_size, set_screen_size) = create_signal(ScreenSize::default());

    let new_player = {
        let websocket = websocket.clone();
        move |name, src_url, file_type, x, y, width, height| {
            websocket.send(
                bincode::serialize(&Message::NewPlayer {
                    name,
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

                                new_player(file.name(), src, file.type_(), 100, 100, 500, None);
                            }
                        });
                    }
                }
            }
        }
    };

    let delete = {
        let websocket = websocket.clone();
        move |player_name| {
            websocket.send(bincode::serialize(&Message::DeletePlayer { player_name }).unwrap());
        }
    };

    let move_up = {
        let websocket = websocket.clone();
        move |player_name| {
            websocket.send(bincode::serialize(&Message::MovePlayerUp { player_name }).unwrap());
        }
    };

    let move_down = {
        let websocket = websocket.clone();
        move |player_name| {
            websocket.send(bincode::serialize(&Message::MovePlayerDown { player_name }).unwrap());
        }
    };

    let flip = {
        let websocket = websocket.clone();
        move |player_name, is_flipped| {
            websocket.send(
                bincode::serialize(&Message::FlipPlayerHorizontally {
                    player_name,
                    is_flipped,
                })
                .unwrap(),
            );
        }
    };

    view! {
        <h1>{move || format!("State: {}", websocket.ready_state.get())}</h1>
        <button on:click={
            let on_file_submit = on_file_submit.clone();
            move |_event| on_file_submit()
        }>"Add"</button>
        <input type="file" accept="video/webm,image/*" node_ref=input_element/>
        <button on:click={
            let get_all_players = get_all_players.clone();
            move |_| get_all_players()
        }>"All players"</button>
        <div
            style="z-index: -5000; outline: 3px solid black; position: absolute;"
            style:width=move || format!("{}px", screen_size().width as f32 * canvas_zoom())
            style:height=move || format!("{}px", screen_size().height as f32 * canvas_zoom())
            style:left=move || format!("{}px", canvas_position().x as f32 * canvas_zoom())
            style:top=move || format!("{}px", canvas_position().y as f32 * canvas_zoom())
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

        <div style="height: 100vh; width: 20vw; background: #535594; position: absolute; left: 0; top: 0; z-index: 5000; margin: 0; padding: 0; box-sizing: border-box;">
            <ul style="width: 100%; margin: 0; padding: 0; box-sizing: border-box;">
                <For
                    each=move || players().into_iter()
                    key=|(name, _)| name.clone()
                    children=move |(name, player): (String, Player)| {
                        view! {
                            <li
                                style="display: flex; justify-content: space-between; list-style: none; width: 100%; margin: 0; padding: 0; box-sizing: border-box;"
                                style:border=move || {
                                    if player.is_selected.get() { "3px solid black" } else { "" }
                                }
                            >
                                <span style="overflow: hidden; white-space: nowrap; text-overflow: ellipsis;">{name.clone()}</span>
                                <div
                                    style="flex-shrink: 0;"
                                    on:click={
                                        let name = name.clone();
                                        move |_event| {
                                            players()
                                                .iter()
                                                .for_each(|(n, p)| {
                                                    if *n != name {
                                                        p.is_selected.set(false)
                                                    } else {
                                                        p.is_selected
                                                            .update(|selected| {
                                                                *selected = !*selected;
                                                            });
                                                    }
                                                });
                                        }
                                    }
                                >
                                    <button on:click={
                                        let move_up = move_up.clone();
                                        let name = name.clone();
                                        move |_e| move_up(name.clone())
                                    }>"↑"</button>
                                    <button on:click={
                                        let move_down = move_down.clone();
                                        let name = name.clone();
                                        move |_e| move_down(name.clone())
                                    }>"↓"</button>
                                    <button on:click={
                                        let delete = delete.clone();
                                        let name = name.clone();
                                        move |_e| delete(name.clone())
                                    }>"✕"</button>
                                    <button on:click={
                                        let flip = flip.clone();
                                        let name = name.clone();
                                        move |_e| {
                                            player.horizontal_flip.update(|is_flipped| *is_flipped = !*is_flipped);
                                            flip(name.clone(), player.horizontal_flip.get());
                                        }
                                    }>"↔"</button>
                                </div>
                            </li>
                        }
                    }
                />

            </ul>
        </div>
    }
}
