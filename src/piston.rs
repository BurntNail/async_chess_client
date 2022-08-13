use crate::{
    cacher::BOARD_S,
    game::{to_board_coord, ChessGame},
};
use color_eyre::Report;
use piston_window::{
    Button, EventLoop, Key, MouseButton, MouseCursorEvent, PistonWindow, PressEvent, RenderEvent,
    UpdateEvent, Window, WindowSettings,
};
use serde::{Deserialize, Serialize};
use crate::eyre;

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct PistonConfig {
    pub id: u32,
    pub res: u32,
}

#[tracing::instrument]
pub async fn piston_main(pc: PistonConfig) -> Result<(), Report> {

    let mut win: PistonWindow = WindowSettings::new("Async Chess", [pc.res, pc.res])
        .exit_on_esc(true)
        .resizable(true)
        .build()
        .unwrap_or_else(|e| {
            error!("Error making window: {e}");
            std::process::exit(1);
        });
    win.set_ups(5);

    let mut game = ChessGame::new(&mut win, pc.id).map_err(|e| eyre!("Error making game: {e}"))?;

    game.update_list().await.map_err(|e| eyre!("Error on initial update: {e}"))?;

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
                game.render(size, c, g, device, mp).unwrap_or_else(|e| {
                    error!("Error rendering: {e}");
                });
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

    Ok(())
}
