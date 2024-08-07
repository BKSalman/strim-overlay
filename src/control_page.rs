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
    MediaType, Message, Player, Position,
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
        let delta = (event.delta_y() / 1000.).abs() as f32;
        if event.delta_y() > 0. {
            set_canvas_zoom.update(|current_zoom| {
                if *current_zoom - delta > 0. {
                    *current_zoom -= delta;
                }
            });
        } else {
            set_canvas_zoom.update(|current_zoom| {
                *current_zoom += delta;
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
            <div style:cursor=move || {
                if canvas_move_click() { "grabbing" } else if space_pressed() { "grab" } else { "" }
            }>

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
                            match player.media_type {
                                crate::MediaType::Text => {
                                    view! {
                                        <div
                                            style="width: 100%; height: 100%;"
                                            style:font-size=move || {
                                                format!("{}px", (player.width.get() / 5) as f32 * canvas_zoom())
                                            }
                                        >
                                            <span>{move || player.data.get()}</span>
                                        </div>
                                    }
                                        .into_view()
                                }
                                crate::MediaType::Image => {
                                    view! {
                                        <img
                                            style="width: 100%; height: 100%;"
                                            style:outline=move || {
                                                if player.is_selected.get() {
                                                    "3px solid black"
                                                } else {
                                                    ""
                                                }
                                            }

                                            autoplay
                                            loop
                                            src=player.data.get()
                                        />
                                    }
                                        .into_view()
                                }
                                crate::MediaType::Video => {
                                    view! {
                                        <video
                                            style="width: 100%; height: 100%;"
                                            style:outline=move || {
                                                if player.is_selected.get() {
                                                    "3px solid black"
                                                } else {
                                                    ""
                                                }
                                            }

                                            autoplay
                                            loop
                                            src=player.data.get()
                                        ></video>
                                    }
                                        .into_view()
                                }
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
    let (channel, set_channel) = create_signal(String::from("sadmadladsalman"));
    let (show_stream_player, set_show_stream_player) = create_signal(true);
    let (interactive_stream_player, set_interactive_stream_player) = create_signal(false);

    let new_player = {
        let websocket = websocket.clone();
        move |name, data, media_type, x, y, width, height| {
            websocket.send(
                bincode::serialize(&Message::NewMedia {
                    name,
                    data,
                    media_type,
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

                                let media_type = if file.type_().starts_with("video") {
                                    MediaType::Video
                                } else {
                                    MediaType::Image
                                };

                                new_player(file.name(), src, media_type, 100, 100, 200, None);
                            }
                        });
                    }
                }
            }
        }
    };

    #[cfg(not(debug_assertions))]
    let iframe_parent = "overlay.bksalman.com";
    #[cfg(debug_assertions)]
    let iframe_parent = "localhost";

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
        }>"Refresh"</button>
        <div
            style="z-index: -5000; outline: 3px solid black; position: absolute;"
            style:width=move || format!("{}px", screen_size().width as f32 * canvas_zoom())
            style:height=move || format!("{}px", screen_size().height as f32 * canvas_zoom())
            style:left=move || format!("{}px", canvas_position().x as f32 * canvas_zoom())
            style:top=move || format!("{}px", canvas_position().y as f32 * canvas_zoom())
        >
            <Show when=move || show_stream_player()>
                <iframe
                    style="pointer-events: none; opacity: 60%;"
                    style:pointer-events=move || {
                        if !interactive_stream_player() { "none" } else { "" }
                    }

                    src=move || {
                        format!(
                            "https://player.twitch.tv/?channel={}&parent={iframe_parent}&muted=true&autoplay=true",
                            channel(),
                        )
                    }

                    height="100%"
                    width="100%"
                    autoplay
                    muted
                ></iframe>
            </Show>
        </div>

        <div style="height: 100vh; width: 20vw; background: #535594; position: absolute; left: 0; top: 0; z-index: 5000; margin: 0; padding: 0; box-sizing: border-box; opacity: 90%;">
            <StreamPlayerSettings
                show_stream_player
                set_show_stream_player
                interactive_stream_player
                set_interactive_stream_player
                channel
                set_channel
            />

            <hr/>

            <ScreenBorder screen_size set_screen_size/>

            <hr/>

            <NewText screen_size/>
            <PlayersList players/>
        </div>
    }
}

#[component]
fn NewText(screen_size: ReadSignal<ScreenSize>) -> impl IntoView {
    let websocket = expect_context::<WebsocketContext>();
    let (show_input, set_show_input) = create_signal(false);
    let (text_content, set_text_content) = create_signal(String::new());

    let send_new_text = move || {
        let message = Message::NewMedia {
            name: text_content(),
            data: text_content(),
            media_type: MediaType::Text,
            position: Position {
                x: 0,
                y: screen_size().height,
            },
            width: 200,
            height: Some(200),
        };
        let message = bincode::serialize(&message).unwrap();
        websocket.send(message);
    };

    view! {
        <div>
            <button on:click=move |_| {
                set_show_input(true);
            }>"Create text"</button>
            <div style:display=move || if show_input() { "" } else { "none" }>
                <input
                    on:change=move |event| set_text_content(event_target_value(&event))
                    prop:value=move || text_content()
                />
                <button on:click=move |_| {
                    set_show_input(false);
                    if text_content().is_empty() {
                        return;
                    }
                    logging::log!("{}", text_content());
                    send_new_text();
                    set_text_content.update(|s| s.clear());
                }>

                    "Add"
                </button>
            </div>
        </div>
    }
}

#[component]
fn StreamPlayerSettings(
    show_stream_player: ReadSignal<bool>,
    set_show_stream_player: WriteSignal<bool>,
    interactive_stream_player: ReadSignal<bool>,
    set_interactive_stream_player: WriteSignal<bool>,
    channel: ReadSignal<String>,
    set_channel: WriteSignal<String>,
) -> impl IntoView {
    view! {
        <div>
            <div style="display: flex; justify-content: space-around">
                <div>
                    <label for="show-channel-player">"Show player"</label>
                    <input
                        id="show-channel-player"
                        type="checkbox"
                        checked=move || show_stream_player()
                        on:change=move |event| {
                            set_show_stream_player(event_target_checked(&event));
                        }
                    />

                </div>

                <div>
                    <label
                        for="interactive-channel-player"
                        title="When this is enabled the scroll won't work when the mouse is over the player"
                    >
                        "Interactive player"
                    </label>
                    <input
                        title="When this is enabled the scroll won't work when the mouse is over the player"
                        id="interactive-channel-player"
                        type="checkbox"
                        checked=move || interactive_stream_player()
                        on:change=move |event| {
                            set_interactive_stream_player(event_target_checked(&event));
                        }
                    />

                </div>

            </div>
            <div>
                <label for="channel">"Channel: "</label>
                <input
                    id="channel"
                    value=move || channel()
                    on:input=move |event| {
                        let value = event_target_value(&event);
                        set_channel(value);
                    }
                />

            </div>
        </div>
    }
}

#[component]
fn ScreenBorder(
    screen_size: ReadSignal<ScreenSize>,
    set_screen_size: WriteSignal<ScreenSize>,
) -> impl IntoView {
    view! {
        <div>
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

#[component]
fn PlayersList(players: ReadSignal<IndexMap<String, Player>>) -> impl IntoView {
    let websocket = expect_context::<WebsocketContext>();
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
        <ul style="width: 100%; margin: 0; padding: 0; box-sizing: border-box;">
            <For
                each=move || players().into_iter()
                key=|(name, _)| name.clone()
                children=move |(name, player): (String, Player)| {
                    view! {
                        <li
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

                            style="display: flex; align-items: center; justify-content: space-between; list-style: none; width: 100%; margin: 0; padding: 0; box-sizing: border-box;"
                            style:border=move || {
                                if player.is_selected.get() { "3px solid black" } else { "" }
                            }
                        >

                            <span
                                title=name.clone()
                                style="overflow: hidden; white-space: nowrap; text-overflow: ellipsis;"
                            >
                                {name.clone()}
                            </span>
                            <div style="display: flex; align-items: center; flex-shrink: 0; height: 1.5rem;">
                                <button
                                    on:click={
                                        let move_up = move_up.clone();
                                        let name = name.clone();
                                        move |_e| move_up(name.clone())
                                    }

                                    title="Move media up"
                                    style="height: 100%;"
                                >
                                    "↑"
                                </button>
                                <button
                                    on:click={
                                        let move_down = move_down.clone();
                                        let name = name.clone();
                                        move |_e| move_down(name.clone())
                                    }

                                    title="Move media down"
                                    style="height: 100%;"
                                >
                                    "↓"
                                </button>
                                <button
                                    on:click={
                                        let flip = flip.clone();
                                        let name = name.clone();
                                        move |_e| {
                                            player
                                                .horizontal_flip
                                                .update(|is_flipped| *is_flipped = !*is_flipped);
                                            flip(name.clone(), player.horizontal_flip.get());
                                        }
                                    }

                                    title="Flip media horizontally"
                                    style="height: 100%;"
                                >
                                    "↔"
                                </button>
                                <button
                                    on:click={
                                        let delete = delete.clone();
                                        let name = name.clone();
                                        move |_e| delete(name.clone())
                                    }

                                    title="Remove media"
                                    style="height: 100%;"
                                >
                                    "🗑"
                                </button>
                            </div>
                        </li>
                    }
                }
            />

        </ul>
    }
}
