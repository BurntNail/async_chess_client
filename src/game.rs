use crate::{
    cacher::{Cacher, TILE_S},
    list_refresher::{ListRefresher, MessageToGame, MessageToWorker},
    piston::{mp_valid, to_board_pixels},
    server_interface::{Board, JSONMove},
};
use anyhow::{Context as _, Result};
use graphics::DrawState;
use piston_window::{clear, rectangle::square, Context, G2d, Image, PistonWindow, Transformed};
use reqwest::StatusCode;
use std::sync::{
    mpsc::{SendError, TryRecvError},
    Arc, RwLock,
};

pub struct ChessGame {
    id: u32,
    c: Cacher,
    cached_pieces: Arc<RwLock<Board>>,
    last_pressed: Option<(u32, u32)>,
    ex_last_pressed: Option<(u32, u32)>,
    refresher: ListRefresher,
}
impl ChessGame {
    pub fn new(win: &mut PistonWindow, id: u32) -> Result<Self> {
        let cps = Arc::new(RwLock::new(vec![None; 64]));
        Ok(Self {
            id,
            c: Cacher::new_and_populate(win).context("making cacher and populating it")?,
            cached_pieces: cps.clone(),
            refresher: ListRefresher::new(cps, id),
            last_pressed: None,
            ex_last_pressed: None,
        })
    }

    // #[tracing::instrument(skip(self, ctx, graphics, _device))]
    pub fn render(
        &mut self,
        ctx: Context,
        graphics: &mut G2d,
        raw_mouse_coords: (f64, f64),
        window_scale: f64,
    ) -> Result<()> {
        let board_coords = if mp_valid(raw_mouse_coords, window_scale) {
            let bps = to_board_pixels(raw_mouse_coords, window_scale);
            Some((
                to_board_coord(bps.0, window_scale),
                to_board_coord(bps.1, window_scale),
            ))
        } else {
            None
        };

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
            if let Some((px, py)) = board_coords {
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
                                    errs.push(anyhow!(
                                        "Cacher doesn't contain: {} at ({col}, {row})",
                                        piece.to_file_name()
                                    ));
                                }
                                Some(tex) => {
                                    let x = f64::from(col) * (TILE_S + 2.0) * window_scale;
                                    let y = f64::from(row) * (TILE_S + 2.0) * window_scale;
                                    let image =
                                        Image::new().rect(square(x, y, TILE_S * window_scale));

                                    let mut draw =
                                        || image.draw(tex, &DrawState::default(), trans, graphics);

                                    if let Some((lp_x, lp_y)) = self.last_pressed {
                                        if lp_x == col as u32 && lp_y == row as u32 {
                                            image.draw(
                                                self.c.get("selected.png").expect("Unable to find selected.png - check your assets folder"),
                                                &DrawState::default(),
                                                trans,
                                                graphics,
                                            );
                                        } else {
                                            draw();
                                        }
                                    } else {
                                        draw();
                                    }
                                }
                            }
                        }
                    }
                }

                {
                    let (raw_x, raw_y) = raw_mouse_coords;
                    if let Some((lp_x, lp_y)) = self.last_pressed {
                        if let Some(piece) = lock[(lp_y * 8 + lp_x) as usize] {
                            if let Some(tex) = self.c.get(&piece.to_file_name()) {
                                let s = TILE_S * window_scale / 1.5;
                                let image =
                                    Image::new().rect(square(raw_x - s / 2.0, raw_y - s / 2.0, s));
                                image.draw(tex, &DrawState::default(), t, graphics);
                            } else {
                                errs.push(anyhow!(
                                    "Cacher doesn't contain: {} at ({lp_x}, {lp_y} floating)",
                                    piece.to_file_name()
                                ));
                            }
                        } else {
                            warn!("no piece at last pressed");
                        }
                    }
                }

                if !errs.is_empty() {
                    bail!("{errs:?}");
                }
            }
            Err(e) => {
                bail!("Unable to read vec: {e}");
            }
        }

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub fn mouse_input(&mut self, mouse_pos: (f64, f64), mult: f64) {
        match std::mem::take(&mut self.last_pressed) {
            None => {
                let lp_x = to_board_coord(mouse_pos.0, mult);
                let lp_y = to_board_coord(mouse_pos.1, mult);

                match self.cached_pieces.read() {
                    Ok(lock) => {
                        if matches!(lock.get(lp_y as usize * 8 + lp_x as usize), Some(Some(_))) {
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

                if let Err(e) = self
                    .refresher
                    .send_msg(MessageToWorker::MakeMove(JSONMove::new(
                        self.id,
                        lp.0,
                        lp.1,
                        current_press.0,
                        current_press.1,
                    )))
                {
                    warn!(%e, "Error sending message to worker re move");
                }
                self.ex_last_pressed = Some(lp);
            }
        }
    }

    ///Should be called ASAP after instantiating game, and often afterwards
    // #[tracing::instrument(skip(self))]
    #[allow(irrefutable_let_patterns)]
    pub fn update_list(&mut self, ignore_timer: bool) -> Result<(), SendError<MessageToWorker>> {
        match self.refresher.try_recv() {
            Ok(msg) => {
                if let MessageToGame::Response(rsp) = msg {
                    match rsp {
                        Ok(response) => {
                            info!(update=?response.text(), "Update from server on moving");
                            self.ex_last_pressed = None;
                        }
                        Err(e) => {
                            if let Some(sc) = e.status() {
                                if sc == StatusCode::PRECONDITION_FAILED {
                                    error!("Invalid move");
                                    self.last_pressed = self.ex_last_pressed;
                                } else {
                                    error!(%e, %sc, "Error in input response");
                                }
                            } else {
                                error!(%e, "Error in input response");
                            }
                        }
                    }
                }
            }
            Err(e) => {
                if e != TryRecvError::Empty {
                    error!(%e, "Try recv error from worker");
                }
            }
        }

        self.refresher.send_msg(if ignore_timer {
            MessageToWorker::UpdateNOW
        } else {
            MessageToWorker::UpdateList
        })
    }

    #[tracing::instrument(skip(self))]
    pub fn restart_board(&mut self) -> Result<()> {
        self.refresher
            .send_msg(MessageToWorker::RestartBoard)
            .context("sending restart msg to board")
    }

    #[tracing::instrument(skip(self))]
    pub fn exit(self) -> Result<()> {
        self.refresher
            .send_msg(MessageToWorker::InvalidateKill)
            .context("sending invalidatekill msg to board")
    }

    pub fn clear_mouse_input(&mut self) {
        self.last_pressed = None;
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn to_board_coord(p: f64, mult: f64) -> u32 {
    (p / ((TILE_S + 2.0) * mult)).floor() as u32
}
