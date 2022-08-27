use crate::piston::{mp_valid, to_board_pixels};
use anyhow::{Context as _, Result};
use async_chess_client::{
    board::{Board, Coords},
    cacher::{Cacher, TILE_S},
    error_ext::{ErrorExt, ToAnyhowErr},
    list_refresher::{BoardMessage, ListRefresher, MessageToGame, MessageToWorker, MoveOutcome},
    server_interface::{no_connection_list, JSONMove},
};
use graphics::DrawState;
use piston_window::{clear, rectangle::square, Context, G2d, Image, PistonWindow, Transformed};
use std::sync::mpsc::TryRecvError;

///Struct to hold Game of Chess
pub struct ChessGame {
    ///The id of the game being played
    id: u32,
    ///The cacher of all the assets
    cache: Cacher,
    ///The Chess Board
    board: Board,
    ///The coordinates of the piece last pressed. Used for selected sprite location.
    last_pressed: Option<Coords>,
    ///The coordinates before - useful for rolling back invalid moves.
    ex_last_pressed: Option<Coords>,
    ///The refresher for making server requests
    refresher: ListRefresher,
}
impl ChessGame {
    ///Create a new `ChessGame`
    ///
    /// # Errors
    /// - Can fail if the cacher incorrectly populates
    pub fn new(win: &mut PistonWindow, id: u32) -> Result<Self> {
        Ok(Self {
            id,
            cache: Cacher::new(win).context("making cacher")?,
            board: Board::default(),
            refresher: ListRefresher::new(id),
            last_pressed: None,
            ex_last_pressed: None,
        })
    }

    // #[tracing::instrument(skip(self, ctx, graphics, _device))]
    ///Renders out the `ChessBoard` to the screen
    ///
    /// # Errors
    /// - Can fail if piece sprites aren't found in the [`Cacher`]. However, will still render all other sprites
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
                .cache
                .get("board_alt.png")
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
                    self.cache
                        .get("highlight.png")
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
                if let Some(piece) = self.board[(col, row).try_into().unwrap_log_error()] {
                    match self.cache.get(&piece.to_file_name()) {
                        Err(e) => {
                            errs.push(e.context(format!(
                                "cacher doesn't contain: {:?} at ({col}, {row})",
                                piece.to_file_name()
                            )));
                        }
                        Ok(tex) => {
                            let x = f64::from(col) * (TILE_S + 2.0) * window_scale;
                            let y = f64::from(row) * (TILE_S + 2.0) * window_scale;
                            let image = Image::new().rect(square(x, y, TILE_S * window_scale));

                            let mut draw =
                                || image.draw(tex, &DrawState::default(), trans, graphics);

                            if let Some((lp_x, lp_y)) = self.last_pressed.map(Into::into) {
                                if lp_x == col as u32 && lp_y == row as u32 {
                                    let tx = self.cache.get("selected.png").context("Unable to find \"selected.png\" - check your assets folder").unwrap_log_error();
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
                    match self.cache.get(&piece.to_file_name()) {
                        Ok(tex) => {
                            let s = TILE_S * window_scale / 1.5;
                            let image =
                                Image::new().rect(square(raw_x - s / 2.0, raw_y - s / 2.0, s));
                            image.draw(tex, &DrawState::default(), t, graphics);
                        }
                        Err(e) => {
                            errs.push(e.context(format!(
                                "Cacher doesn't contain: {} at ({:?} floating)",
                                piece.to_file_name(),
                                lp
                            )));
                        }
                    }
                } else {
                    self.last_pressed = None;
                }
            }
        }

        if !errs.is_empty() {
            bail!("{errs:?}");
        }

        Ok(())
    }

    ///Handles mouse input
    ///
    /// # Errors
    /// - Can fail if there is an error sending the message to the [`ListRefresher`]
    #[tracing::instrument(skip(self))]
    pub fn mouse_input(&mut self, mouse_pos: (f64, f64), mult: f64) -> Result<()> {
        match std::mem::take(&mut self.last_pressed) {
            None => {
                let lp_x = to_board_coord(mouse_pos.0, mult);
                let lp_y = to_board_coord(mouse_pos.1, mult);

                let coord = (lp_x, lp_y).try_into()?;

                if self.board.piece_exists_at_location(coord) {
                    self.last_pressed = Some(coord);
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

                self.refresher
                    .send_msg(MessageToWorker::MakeMove(JSONMove::new(
                        self.id,
                        lp.x(),
                        lp.y(),
                        current_press.0,
                        current_press.1,
                    )))
                    .context("sending a message to the worker re moving")?;

                self.ex_last_pressed = Some(lp);
            }
        }

        Ok(())
    }

    ///Updates the board using messages from the [`ListRefresher`]
    ///
    /// Should be called ASAP after instantiating game, and often afterwards.
    ///
    /// # Errors:
    /// - Can fail if an error sending a message to the [`ListRefresher`]
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
                    std::process::exit(1);
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

    ///Sends a message to the [`ListRefresher`] to clear the board for a new game.
    ///
    /// # Errors:
    /// - If there is an error sending the message
    #[tracing::instrument(skip(self))]
    pub fn restart_board(&mut self) -> Result<()> {
        self.refresher
            .send_msg(MessageToWorker::RestartBoard)
            .context("sending restart msg to board")
    }

    ///Sends a message to the [`ListRefresher`] to tell the server we're done
    ///
    /// # Errors:
    /// - If there is an error sending the message
    #[tracing::instrument(skip(self))]
    pub fn exit(self) -> Result<()> {
        self.refresher
            .send_msg(MessageToWorker::InvalidateKill)
            .context("sending invalidatekill msg to board")
    }

    ///Clears the mouse input - means that a different piece can be selected.
    pub fn clear_mouse_input(&mut self) {
        self.last_pressed = None;
        self.ex_last_pressed = None;
    }
}

///Converts a pixel to a board coordinate, assuming that the mouse cursor is on the board
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn to_board_coord(p: f64, mult: f64) -> u32 {
    (p / ((TILE_S + 2.0) * mult)).floor() as u32
}
