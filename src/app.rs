use std::collections::VecDeque;

use crate::{
    error_template::{AppError, ErrorTemplate},
    server::increment_counter,
    Count, Player, Position,
};
use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use leptos_server_signal::create_server_signal;

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    leptos_server_signal::provide_websocket("ws://localhost:3030/ws").unwrap();

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
    let (players, set_players) = create_signal(VecDeque::<Player>::new());
    set_players.update(|players| {
        players.push_front(Player {
            id: 0,
            file: RwSignal::new("sugoi.webm".into()),
            position: RwSignal::new(Position::new(0, 0)),
        });
    });
    set_players.update(|players| {
        players.push_front(Player {
            id: 1,
            file: RwSignal::new("sugoi-1.webm".into()),
            position: RwSignal::new(Position::new(100, 100)),
        });
    });

    let count = create_server_signal::<Count>("counter");

    view! {
        <button on:click=move |_| {
            spawn_local(async {
                increment_counter().await;
            });
        }>"Click me"</button>
        <div>"Count: "{move || count().value}</div>
        <Players players set_players/>
    }
}

#[component]
fn Players(
    players: ReadSignal<VecDeque<Player>>,
    set_players: WriteSignal<VecDeque<Player>>,
    #[prop(default = RwSignal::new(false))] mouse_clicked: RwSignal<bool>,
) -> impl IntoView {
    view! {
          <For
            // a function that returns the items we're iterating over; a signal is fine
            each=players
            // a unique key for each item
            key=|player| player.id
            // renders each item to a view
            children=move |player: Player| {
              view! {
                <div
                on:mousedown=move |_event| {
                    mouse_clicked.set(true);
                    set_players.update(|players| {
                        let idx = players.iter().position(|p| p.id == player.id).unwrap();
                        let player = players.remove(idx).unwrap();
                        players.push_back(player);
                    });
                }
                on:mousemove=move |event| {
                    if mouse_clicked.get() {
                        set_players.update(|players| {
                                let player = players.iter_mut().find(|p| p.id == player.id).unwrap();
                                player.position.update(|p| {
                                    p.x += event.movement_x();
                                    p.y += event.movement_y();
                                });
                         });
                    }
                }
                on:mouseup=move |_event| {
                    mouse_clicked.set(false);
                }
                on:mouseleave=move |_| {
                    mouse_clicked.set(false);
                }
                style="position: absolute;"
                style:left=move || format!("{}px", player.position.get().x)
                style:top=move || format!("{}px", player.position.get().y)
                ><video autoplay loop src=player.file.get().to_string_lossy().to_string()></video></div>
              }
            }
          />
    }
}
