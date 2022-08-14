use crate::{
    cacher::{Cacher, BOARD_S, TILE_S},
    eyre,
    server_interface::{Board, JSONMove, JSONPieceList},
};
use color_eyre::Report;
use graphics::DrawState;
use piston_window::{
    clear, rectangle::square, Context, G2d, GfxDevice, Image, PistonWindow, Size, Transformed,
};
use reqwest::{Client, ClientBuilder};
use std::sync::RwLock;

pub struct ChessGame {
    id: u32,
    c: Cacher,
    cached_pieces: RwLock<Board>,
    last_pressed: Option<(u32, u32)>,
    client: Client,
    no_connection_at_last_refresh: bool,
}
impl ChessGame {
    pub fn new(win: &mut PistonWindow, id: u32) -> Result<Self, Report> {
        Ok(Self {
            id,
            c: Cacher::new_and_populate(win)?,
            cached_pieces: RwLock::new(vec![None; 64]),
            last_pressed: None,
            client: ClientBuilder::default()
                .user_agent("J/AsyncChess")
                .build()?,
            no_connection_at_last_refresh: false,
        })
    }

    // #[tracing::instrument(skip(self, ctx, graphics, _device))]
    pub fn render(
        &mut self,
        size: Size,
        ctx: Context,
        graphics: &mut G2d,
        _device: &mut GfxDevice,
        mouse_coords: Option<(f64, f64)>,
    ) -> Result<(), Report> {
        let window_scale = size.height / BOARD_S;
        let mouse_coords = mouse_coords.map(|(x, y)| {
            (
                to_board_coord(x, window_scale),
                to_board_coord(y, window_scale),
            )
        });

        clear([0.0; 4], graphics);
        let t = ctx.transform;
        {
            let image = Image::new().rect(square(0.0, 0.0, 256.0 * window_scale));
            image.draw(
                self.c.get("board_alt.png").unwrap(),
                &DrawState::default(),
                t,
                graphics,
            );
        }

        let trans = t.trans(41.0 * window_scale, 41.0 * window_scale);

        {
            if let Some((px, py)) = mouse_coords {
                let x = f64::from(px) * (TILE_S + 2.0) * window_scale;
                let y = f64::from(py) * (TILE_S + 2.0) * window_scale;
                let image = Image::new().rect(square(x, y, 20.0 * window_scale));

                image.draw(
                    self.c.get("highlight.png").unwrap(),
                    &DrawState::default(),
                    trans,
                    graphics,
                );
            }
        }

        match self.cached_pieces.read() {
            Ok(lock) => {
                let mut errs = vec![];

                for col in 0..8_u32 {
                    for row in 0..8_u32 {
                        let idx = row * 8 + col;
                        if let Some(piece) = lock[idx as usize] {
                            match self.c.get(&piece.to_file_name()) {
                                None => {
                                    errs.push(eyre!(
                                        "Cacher doesn't contain: {} at ({col}, {row})",
                                        piece.to_file_name()
                                    ));
                                }
                                Some(tex) => {
                                    let x = f64::from(col) * (TILE_S + 2.0) * window_scale;
                                    let y = f64::from(row) * (TILE_S + 2.0) * window_scale;
                                    let image =
                                        Image::new().rect(square(x, y, 20.0 * window_scale));

                                    image.draw(tex, &DrawState::default(), trans, graphics);

                                    if let Some((lp_x, lp_y)) = self.last_pressed {
                                        if lp_x == col as u32 && lp_y == row as u32 {
                                            image.draw(
                                                self.c.get("selected.png").unwrap(),
                                                &DrawState::default(),
                                                trans,
                                                graphics,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if !errs.is_empty() {
                    return Err(eyre!("{errs:?}"));
                }
            }
            Err(e) => {
                return Err(eyre!("Unable to read vec: {e}"));
            }
        }

        Ok(())
    }

    #[tracing::instrument(skip(self, mouse_pos))]
    pub async fn mouse_input(&mut self, mouse_pos: (f64, f64), mult: f64) {
        match std::mem::take(&mut self.last_pressed) {
            None => {
                let lp_x = to_board_coord(mouse_pos.0, mult);
                let lp_y = to_board_coord(mouse_pos.1, mult);

                match self.cached_pieces.read() {
                    Ok(lock) => {
                        if lock.get(lp_y as usize * 8 + lp_x as usize).is_some() {
                            self.last_pressed = Some((lp_x, lp_y));
                        }
                    }
                    Err(err) => error!(%err, "Unable to read cached pieces"),
                }
            }
            Some(lp) => {
                //Deal with second press
                let current_press = {
                    let lp_x = to_board_coord(mouse_pos.0, mult);
                    let lp_y = to_board_coord(mouse_pos.1, mult);
                    (lp_x, lp_y)
                };

                info!(last_pos=?lp, new_pos=?current_press, "Starting moving");

                let rsp = self
                    .client
                    .post("http://109.74.205.63:12345/movepiece")
                    .json(&JSONMove::new(
                        self.id,
                        lp.0,
                        lp.1,
                        current_press.0,
                        current_press.1,
                    ))
                    .send()
                    .await;

                match rsp {
                    Ok(response) => {
                        info!(update=?response.text().await, "Update from server on moving");
                        //TODO: communicate to user
                    }
                    Err(e) => {
                        error!(%e, "Error in input response");
                    }
                }
            }
        }
    }

    ///Should be called ASAP after instantiating game, and often afterwards
    // #[tracing::instrument(skip(self))]
    pub async fn update_list(&mut self) -> Result<(), Report> {
        let result_rsp = self
            .client
            .get(format!("http://109.74.205.63:12345/games/{}", self.id))
            .send()
            .await;

        let (mut list, ret) = match result_rsp {
            Ok(rsp) => {
                let jpl = rsp.error_for_status()?.json::<JSONPieceList>().await?;

                self.no_connection_at_last_refresh = false;
                (Some(jpl.into_game_list()?), Ok(()))
            }
            Err(e) => {
                //Only for reqwest server errors (hopefully)
                let l = if self.no_connection_at_last_refresh {
                    None
                } else {
                    self.no_connection_at_last_refresh = true;
                    Some(JSONPieceList::no_connection_list())
                };
                (l, Err(eyre!("Reqwest Error: {e}")))
            }
        }; //moved away to fix await errors with holding the lock

        match self.cached_pieces.write() {
            Ok(mut lock) => {
                if let Some(l) = list.take() {
                    *lock = l;
                }
                ret
            }
            Err(e) => Err(eyre!("Unable to populate due to {e}")),
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn restart_board(&mut self) -> Result<(), reqwest::Error> {
        let rsp = self
            .client
            .post("http://109.74.205.63:12345/newgame")
            .body(self.id.to_string())
            .send()
            .await?;
        info!(update=?rsp.text().await, "Update from server on restarting");
        Ok(())
    }

    pub fn clear_mouse_input(&mut self) {
        self.last_pressed = None;
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn to_board_coord(p: f64, mult: f64) -> u32 {
    let tile_size = (TILE_S + 2.0) * mult;
    (p / tile_size).floor() as u32
}
