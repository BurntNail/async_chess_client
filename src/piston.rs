use crate::{cacher::BOARD_S, game::ChessGame};
use piston_window::{
    Button, Key, MouseButton, MouseCursorEvent, PistonWindow, PressEvent, RenderEvent, UpdateEvent,
    Window, WindowSettings,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct PistonConfig {
    pub id: u32,
    pub res: u32,
}

#[tracing::instrument(skip(pc))]
pub async fn piston_main(pc: PistonConfig) {
    let mut win: PistonWindow = WindowSettings::new("Async Chess", [pc.res, pc.res])
        .exit_on_esc(true)
        .resizable(true)
        .build()
        .unwrap_or_else(|e| {
            error!(%e, "Error making window");
            std::process::exit(1);
        });
    // win.set_ups(5);

    let mut game =
        ChessGame::new(&mut win, pc.id).unwrap_or_else(|e| panic!("Error making game: {e}"));

    if let Err(e) = game.update_list().await {
        error!(%e, "Error on initial update");
    }

    let mut mouse_pos = (0.0, 0.0);
    while let Some(e) = win.next() {
        let size = win.size();

        if let Some(_r) = e.render_args() {
            let window_scale = size.height / BOARD_S;
            let mp = if mp_valid(mouse_pos, window_scale) {
                Some(to_board_pixels(mouse_pos, window_scale))
            } else {
                None
            };

            win.draw_2d(&e, |c, g, device| {
                game.render(size, c, g, device, mp).unwrap_or_else(|e| {
                    error!(%e, "Error rendering");
                });
            });
        }

        if let Some(_u) = e.update_args() {
            game.update_list().await.unwrap_or_else(|err| {
                error!(%err, "Unable to re-update list");
            });
        }

        if let Some(pa) = e.press_args() {
            match pa {
                Button::Keyboard(kb) => {
                    info!(?kb, "Keyboard Input");
                    if kb == Key::C {
                        //Clear
                        game.restart_board().await.unwrap_or_else(|err| {
                            error!(%err, "Unable to restart board");
                        });
                    }
                }
                Button::Mouse(mb) => {
                    let window_scale = size.height / BOARD_S;

                    if mb == MouseButton::Right {
                        game.clear_mouse_input();
                    } else if mp_valid(mouse_pos, window_scale) {
                        game.mouse_input(to_board_pixels(mouse_pos, window_scale), window_scale)
                            .await;
                    }
                }
                _ => {}
            }

            game.update_list().await.unwrap_or_else(|err| {
                error!(%err, "Unable to re-update list");
            });
        }

        e.mouse_cursor(|p| mouse_pos = (p[0], p[1]));
    }
}

#[allow(clippy::nonminimal_bool)]
fn mp_valid(mouse_pos: (f64, f64), window_scale: f64) -> bool {
    mouse_pos.0 > 40.0 * window_scale
        || mouse_pos.0 < 216.0 * window_scale
        || mouse_pos.1 > 40.0 * window_scale
        || mouse_pos.1 < 216.0 * window_scale
}
fn to_board_pixels(raw_mouse_pos: (f64, f64), window_scale: f64) -> (f64, f64) {
    (
        raw_mouse_pos.0 - 40.0 * window_scale,
        raw_mouse_pos.1 - 40.0 * window_scale,
    )
}
