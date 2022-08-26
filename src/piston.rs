use crate::{
    cacher::BOARD_S,
    error_ext::{ErrorExt},
    game::ChessGame,
    time_based_structs::MemoryTimedCacher,
};
use anyhow::Context;
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

#[tracing::instrument(skip(pc), level = "debug")]
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

    game.update_list(true)
        .context("initial update")
        .error();

    let mut mouse_pos = (0.0, 0.0);
    let mut time_since_last_frame = 0.0;
    let mut cached_dt = MemoryTimedCacher::<_, 100>::default();

    while let Some(e) = win.next() {
        let size = win.size();
        debug!(fps=%(1.0 / time_since_last_frame), cached_fps=%(1.0 / cached_dt.average_f64()));

        if let Some(r) = e.render_args() {
            time_since_last_frame = r.ext_dt;
            cached_dt.add(r.ext_dt);

            let window_scale = size.height / BOARD_S;

            win.draw_2d(&e, |c, g, _device| {
                game.render(c, g, mouse_pos, window_scale)
                    .context("rendering")
                    .error();
            });
        }

        if let Some(_u) = e.update_args() {
            game.update_list(false)
                .context("on update args")
                .error();
        }

        if let Some(pa) = e.press_args() {
            let mut update_now = false;

            match pa {
                Button::Keyboard(kb) => {
                    info!(?kb, "Keyboard Input");
                    if kb == Key::C {
                        //Clear
                        game.restart_board().context("restart on c key").error();
                        update_now = true;
                    }
                }
                Button::Mouse(mb) => {
                    let window_scale = size.height / BOARD_S;

                    if mb == MouseButton::Right {
                        game.clear_mouse_input();
                    } else if mp_valid(mouse_pos, window_scale) {
                        game.mouse_input(to_board_pixels(mouse_pos, window_scale), window_scale);
                        update_now = true;
                    }
                }
                _ => {}
            }

            game.update_list(update_now)
                .with_context(|| format!("update on input update_now: {update_now}"))
                .error();
        }

        e.mouse_cursor(|p| mouse_pos = (p[0], p[1]));
    }

    info!("Finishing and cleaning up");
    game.exit().context("clearing up").error();
}

///Must always be called BEFORE [`to_board_pixels`]
pub fn mp_valid(mouse_pos: (f64, f64), window_scale: f64) -> bool {
    mouse_pos.0 > 40.0 * window_scale
        && mouse_pos.0 < 216.0 * window_scale
        && mouse_pos.1 > 40.0 * window_scale
        && mouse_pos.1 < 216.0 * window_scale
}

///Must always be called AFTER [`mp_valid`]
pub fn to_board_pixels(raw_mouse_pos: (f64, f64), window_scale: f64) -> (f64, f64) {
    (
        raw_mouse_pos.0 - 40.0 * window_scale,
        raw_mouse_pos.1 - 40.0 * window_scale,
    )
}
