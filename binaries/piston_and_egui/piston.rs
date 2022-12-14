use crate::{
    game::ChessGame,
    pixel_size_consts::{BOARD_S, LEFT_BOUND, RIGHT_BOUND},
};
use anyhow::Context;
use async_chess_client::{
    prelude::ErrorExt, util::time_based_structs::memcache::MemoryTimedCacher,
};
use piston_window::{
    Button, Key, MouseButton, MouseCursorEvent, PistonWindow, PressEvent, RenderEvent, UpdateEvent,
    Window, WindowSettings,
};
use serde::{Deserialize, Serialize};

///Configuration for the Piston window
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct PistonConfig {
    ///The game id
    pub id: u32,
    ///The width/height of the window
    pub res: u32,
}

///Starts up a piston window using the given [`PistonConfig`]
#[tracing::instrument(skip(pc))]
pub fn piston_main(pc: PistonConfig) {
    let mut win: PistonWindow = WindowSettings::new("Async Chess", [pc.res, pc.res])
        .exit_on_esc(true)
        .resizable(true)
        .build()
        .map_err(|e| anyhow!("{e}"))
        .context("making window")
        .unwrap_log_error();
    // win.set_ups(5);

    let mut game = ChessGame::new(&mut win, pc.id)
        .context("new chess game")
        .unwrap_log_error();

    game.update_list(true).context("initial update").error();

    let mut mouse_pos = (0.0, 0.0);
    let mut time_since_last_frame = 0.0;
    let mut cached_dt = MemoryTimedCacher::<_, 100>::default();
    let mut is_flipped = false;

    while let Some(e) = win.next() {
        let window_scale = win.size().height / BOARD_S;

        if time_since_last_frame == 0.0 || cached_dt.is_empty() {
            debug!(fps=%(1.0 / time_since_last_frame), cached_fps=%(1.0 / cached_dt.average_f64()));
        }

        if let Some(r) = e.render_args() {
            time_since_last_frame = r.ext_dt;
            cached_dt.add(r.ext_dt);

            win.draw_2d(&e, |c, g, _device| {
                game.render(c, g, mouse_pos, window_scale, is_flipped)
                    .context("rendering")
                    .error();
            });
        }

        if let Some(_u) = e.update_args() {
            game.update_list(false).context("on update args").error();
        }

        if let Some(pa) = e.press_args() {
            let mut update_now = false;

            match pa {
                Button::Keyboard(kb) => {
                    info!(?kb, "Keyboard Input");

                    match kb {
                        Key::C => {
                            //Clear
                            game.restart_board().context("restart on c key").error();
                            update_now = true;
                        },
                        Key::F =>  is_flipped = !is_flipped,
                        _ => {}
                    }
                }
                Button::Mouse(mb) => {
                    if mb == MouseButton::Right {
                        game.clear_mouse_input();
                    } else if mp_valid(mouse_pos, window_scale) {
                        game.mouse_input(to_board_pixels(mouse_pos, window_scale), window_scale)
                            .context("dealing with mouse input")
                            .error();
                        update_now = true;
                    }
                }
                _ => {}
            }

            game.update_list(update_now)
                .with_context(|| format!("update on input update_now: {update_now}"))
                .error();
        }

        e.mouse_cursor(|p| {
            if is_flipped {
                mouse_pos = (p[0], (BOARD_S * window_scale) - p[1]);
            } else {
                mouse_pos = (p[0], p[1]);
            }
        });
    }

    info!("Finishing and cleaning up");
    game.exit().context("clearing up").error();
}

///Checks whether or not the mouse is on the board
///
/// Must always be called BEFORE [`to_board_pixels`]
pub fn mp_valid(raw_mp: (f64, f64), window_scale: f64) -> bool {
    raw_mp.0 > LEFT_BOUND * window_scale
        && raw_mp.0 < RIGHT_BOUND * window_scale
        && raw_mp.1 > LEFT_BOUND * window_scale
        && raw_mp.1 < RIGHT_BOUND * window_scale
}

///Converts window pixels to board pixels
///
/// Must always be called AFTER [`mp_valid`]
pub fn to_board_pixels(raw_mouse_pos: (f64, f64), window_scale: f64) -> (f64, f64) {
    (
        raw_mouse_pos.0 - LEFT_BOUND * window_scale,
        raw_mouse_pos.1 - LEFT_BOUND * window_scale,
    )
}
