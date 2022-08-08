#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::multiple_crate_versions,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::use_self,
    clippy::nonminimal_bool
)]

mod cacher;
mod chess;
mod game;
mod server_interface;

#[macro_use]
extern crate tracing;

use crate::{
    cacher::BOARD_S,
    game::{to_board_coord, ChessGame},
};
use piston_window::{
    Button, EventLoop, Key, MouseButton, MouseCursorEvent, PistonWindow, PressEvent, RenderEvent,
    UpdateEvent, Window, WindowSettings,
};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() {
    {
        let sub = FmtSubscriber::builder()
            .with_max_level(Level::INFO)
            .finish();
        tracing::subscriber::set_global_default(sub).expect("Setting sub failed!");
    }

    info!("Thanks to Devil's Workshop for the Chess Assets!");

    let mut win: PistonWindow = WindowSettings::new("Async Chess", [800, 800])
        .exit_on_esc(true)
        .resizable(true)
        .build()
        .unwrap_or_else(|e| {
            error!("Error making window: {e}");
            std::process::exit(1);
        });
    win.set_ups(25);

    let mut game = ChessGame::new(&mut win).unwrap_or_else(|e| {
        error!("Error making game: {e}");
        std::process::exit(1);
    });
    game.update_list().await.unwrap_or_else(|err| {
        error!("Unable to populate game: {err}");
        std::process::exit(1);
    });

    let mut mouse_pos = (0.0, 0.0);
    while let Some(e) = win.next() {
        let size = win.size();

        if let Some(_r) = e.render_args() {
            let window_scale = size.height / BOARD_S;
            let mp = if mouse_pos.0 < 40.0 * window_scale
                || mouse_pos.0 > 216.0 * window_scale
                || mouse_pos.0 < 40.0 * window_scale
                || mouse_pos.0 > 216.0 * window_scale
            {
                None
            } else {
                let inp: (f64, f64) = (
                    mouse_pos.0 - 40.0 * window_scale,
                    mouse_pos.1 - 40.0 * window_scale,
                );
                let px = to_board_coord(inp.0, window_scale);
                let py = to_board_coord(inp.1, window_scale);
                Some((px, py))
            };

            win.draw_2d(&e, |c, g, device| {
                game.render(size, c, g, device, mp);
            });
        }

        if let Some(_u) = e.update_args() {
            game.update_list().await.unwrap_or_else(|err| {
                error!("Unable to re-update list: {err}");
            });
        }

        if let Some(pa) = e.press_args() {
            match pa {
                Button::Keyboard(kb) => {
                    if kb == Key::C {
                        //Clear
                        game.restart_board().await.unwrap_or_else(|err| {
                            error!("Unable to restart board: {err}");
                        });
                        game.update_list().await.unwrap_or_else(|err| {
                            error!("Unable to re-update list: {err}");
                        });
                    }
                }
                Button::Mouse(mb) => {
                    let window_scale = size.height / BOARD_S;

                    if mb == MouseButton::Right {
                        game.clear_mouse_input();
                    } else if !(mouse_pos.0 < 40.0 * window_scale
                        || mouse_pos.0 > 216.0 * window_scale
                        || mouse_pos.0 < 40.0 * window_scale
                        || mouse_pos.0 > 216.0 * window_scale)
                    {
                        let inp = (
                            mouse_pos.0 - 40.0 * window_scale,
                            mouse_pos.1 - 40.0 * window_scale,
                        );
                        game.mouse_input(inp, size).await;
                    }

                    game.update_list().await.unwrap_or_else(|err| {
                        error!("Unable to re-update list: {err}");
                    });
                }
                _ => {}
            }
        }

        e.mouse_cursor(|p| mouse_pos = (p[0], p[1]));
    }
}
