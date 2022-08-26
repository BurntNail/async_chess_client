use crate::{
    board::{Board, Coords},
    cacher::{Cacher, TILE_S},
    error_ext::{ErrorExt, ToAnyhowErr, ToAnyhowNotErr},
    list_refresher::{BoardMessage, ListRefresher, MessageToGame, MessageToWorker, MoveOutcome},
    piston::{mp_valid, to_board_pixels},
    server_interface::{no_connection_list, JSONMove},
};
use anyhow::{Context as _, Result};
use graphics::DrawState;
use piston_window::{clear, rectangle::square, Context, G2d, Image, PistonWindow, Transformed};
use std::sync::mpsc::TryRecvError;

pub struct ChessGame {
    id: u32,
    c: Cacher,
    board: Board,
    last_pressed: Option<Coords>,
    ex_last_pressed: Option<Coords>,
    refresher: ListRefresher,
}
impl ChessGame {
    pub fn new(win: &mut PistonWindow, id: u32) -> Result<Self> {
        Ok(Self {
            id,
            c: Cacher::new_and_populate(win).context("making cacher and populating it")?,
            board: Board::default(),
            refresher: ListRefresher::new(id),
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
            let tex = self
                .c
                .get("board_alt.png")
                .ae()
                .context("getting hightlight.png")
                .unwrap_log_error();
            image.draw(tex, &DrawState::default(), t, graphics);
        }

        let trans = t.trans(41.0 * window_scale, 41.0 * window_scale);

        {
            if let Some((px, py)) = board_coords {
                let x = f64::from(px) * (TILE_S + 2.0) * window_scale;
                let y = f64::from(py) * (TILE_S + 2.0) * window_scale;
                let image = Image::new().rect(square(x, y, 20.0 * window_scale));

                image.draw(
                    self.c
                        .get("highlight.png")
                        .ae()
                        .context("getting hightlight.png")
                        .unwrap_log_error(),
                    &DrawState::default(),
                    trans,
                    graphics,
                );
            }
        }
        let mut errs = vec![];

        for col in 0..8_u32 {
            for row in 0..8_u32 {
                if let Some(piece) = self.board[(col, row)] {
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
                            let image = Image::new().rect(square(x, y, TILE_S * window_scale));

                            let mut draw =
                                || image.draw(tex, &DrawState::default(), trans, graphics);

                            if let Some((lp_x, lp_y)) = self.last_pressed {
                                if lp_x == col as u32 && lp_y == row as u32 {
                                    let tx = self.c.get("selected.png").ae().context("Unable to find \"selected.png\" - check your assets folder").unwrap_log_error();
                                    image.draw(tx, &DrawState::default(), trans, graphics);
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
            if let Some(lp) = self.last_pressed {
                if let Some(piece) = self.board[lp] {
                    if let Some(tex) = self.c.get(&piece.to_file_name()) {
                        let s = TILE_S * window_scale / 1.5;
                        let image = Image::new().rect(square(raw_x - s / 2.0, raw_y - s / 2.0, s));
                        image.draw(tex, &DrawState::default(), t, graphics);
                    } else {
                        errs.push(anyhow!(
                            "Cacher doesn't contain: {} at ({}, {} floating)",
                            piece.to_file_name(),
                            lp.0,
                            lp.1
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

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub fn mouse_input(&mut self, mouse_pos: (f64, f64), mult: f64) {
        match std::mem::take(&mut self.last_pressed) {
            None => {
                let lp_x = to_board_coord(mouse_pos.0, mult);
                let lp_y = to_board_coord(mouse_pos.1, mult);

                if self.board.piece_exists_at_location((lp_x, lp_y)) {
                    self.last_pressed = Some((lp_x, lp_y));
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
    pub fn update_list(&mut self, ignore_timer: bool) -> Result<()> {
        match self.refresher.try_recv() {
            Ok(msg) => match msg {
                MessageToGame::UpdateBoard(msg) => match msg {
                    BoardMessage::TmpMove(m) => {
                        self.board.make_move(m);
                    }
                    BoardMessage::Move(outcome) => match outcome {
                        MoveOutcome::Worked => self.board.move_worked(),
                        MoveOutcome::Invalid | MoveOutcome::ReqwestFailed => {
                            self.board.undo_move();
                            info!("Resetting pieces");
                        }
                    },
                    BoardMessage::NoConnectionList => self.board = no_connection_list(),
                    BoardMessage::NewList(l) => self.board = Board::new_json(l)?,
                    BoardMessage::UseExisting => {}
                },
            },
            Err(e) => {
                if e != TryRecvError::Empty {
                    error!(%e, "Try recv error from worker");
                }
            }
        }

        self.refresher
            .send_msg(if ignore_timer {
                MessageToWorker::UpdateNOW
            } else {
                MessageToWorker::UpdateList
            })
            .ae()
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
