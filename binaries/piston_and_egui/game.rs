use crate::{
    piston::{mp_valid, to_board_pixels},
    pixel_size_consts::{BOARD_S, BOARD_TILE_S, LEFT_BOUND_PADDING, RIGHT_BOUND, TILE_S},
};
use anyhow::{Context as _, Result};
use async_chess_client::{
    chess::board::{board::Board, board_container::BoardContainer},
    net::{
        list_refresher::{
            BoardMessage, ListRefresher, MessageToGame, MessageToWorker, MoveOutcome,
        },
        server_interface::{no_connection_list, JSONMove},
    },
    prelude::{Coords, Either, ErrorExt, ScopedTimer},
    util::{cacher::Cacher, error_ext::ToAnyhowErr},
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
    board: BoardContainer,
    ///The coordinates of the piece last pressed. Used for selected sprite location.
    last_pressed: Coords,
    ///The coordinates before - useful for rolling back invalid moves.
    ex_last_pressed: Coords,
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
            board: BoardContainer::default(),
            refresher: ListRefresher::new(id),
            last_pressed: Coords::OffBoard,
            ex_last_pressed: Coords::OffBoard,
        })
    }

    ///Handles mouse input
    ///
    /// # Errors
    /// - Can fail if there is an error sending the message to the [`ListRefresher`]
    #[tracing::instrument(skip(self))]
    pub fn mouse_input(&mut self, mouse_pos: (f64, f64), mult: f64) -> Result<()> {
        match std::mem::take(&mut self.last_pressed) {
            Coords::OffBoard => {
                let lp_x = to_board_coord(mouse_pos.0, mult);
                let lp_y = to_board_coord(mouse_pos.1, mult);

                let coord = (lp_x, lp_y).try_into()?;

                if self.board.piece_exists_at_location(coord) {
                    self.last_pressed = coord;
                }
            }
            Coords::OnBoard(x, y) => {
                //Deal with second press
                let current_press = {
                    let lp_x = to_board_coord(mouse_pos.0, mult);
                    let lp_y = to_board_coord(mouse_pos.1, mult);
                    (lp_x, lp_y)
                };

                info!(last_pos=?(x, y), new_pos=?current_press, "Starting moving");

                self.refresher
                    .send_msg(MessageToWorker::MakeMove(JSONMove::new(
                        self.id,
                        u32::from(x),
                        u32::from(y),
                        current_press.0,
                        current_press.1,
                    )))
                    .context("sending a message to the worker re moving")?;

                self.ex_last_pressed = Coords::OnBoard(x, y);
            }
        }

        Ok(())
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
        self.last_pressed = Coords::OffBoard;
        self.ex_last_pressed = Coords::OffBoard;
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
            let image = Image::new().rect(square(0.0, 0.0, BOARD_S * window_scale));
            let tex = self
                .cache
                .get("board_alt.png")
                .context("getting hightlight.png")
                .unwrap_log_error();
            image.draw(tex, &DrawState::default(), t, graphics);
        }

        let trans = t.trans(
            LEFT_BOUND_PADDING * window_scale,
            LEFT_BOUND_PADDING * window_scale,
        );

        {
            if let Some((px, py)) = board_coords {
                let x = f64::from(px) * BOARD_TILE_S * window_scale;
                let y = f64::from(py) * BOARD_TILE_S * window_scale;
                let image = Image::new().rect(square(x, y, TILE_S * window_scale));

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

        for col in 0..8_u8 {
            for row in 0..8_u8 {
                if let Some(piece) = self.board[(col, row).into()] {
                    match self.cache.get(&piece.to_file_name()) {
                        Err(e) => {
                            errs.push(e.context(format!(
                                "cacher doesn't contain: {:?} at ({col}, {row})",
                                piece.to_file_name()
                            )));
                        }
                        Ok(tex) => {
                            let x = f64::from(col) * BOARD_TILE_S * window_scale;
                            let y = f64::from(row) * BOARD_TILE_S * window_scale;
                            let image = Image::new().rect(square(x, y, TILE_S * window_scale));

                            let mut draw =
                                || image.draw(tex, &DrawState::default(), trans, graphics);

                            if let Coords::OnBoard(lp_x, lp_y) = self.last_pressed {
                                if lp_x == col && lp_y == row {
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
            ///Size in pixels for pieces which have been taken
            const TAKEN_TILE_SIZE: f64 = TILE_S * 0.75;
            ///Starting Y for Taken tiles, such that when all pieces are taken, it it centred
            const START_Y: f64 = (BOARD_S - (TAKEN_TILE_SIZE * 16.0)) / 2.0; //16 pieces

            let mut pieces = self.board.get_taken();
            pieces.sort();

            let white_trans = t.trans(TAKEN_TILE_SIZE * window_scale, START_Y * window_scale);
            let black_trans = t.trans(
                (RIGHT_BOUND + TAKEN_TILE_SIZE) * window_scale,
                START_Y * window_scale,
            );

            
            let mut white_dy = 0.0;
            let mut black_dy = 0.0;

            for p in pieces {
                match self.cache.get(&p.to_file_name()) {
                    Err(e) => errs
                        .push(e.context(format!("cacher doesn't contain: {:?}", p.to_file_name()))),
                    Ok(tex) => {
                        if p.is_white {
                            let img = Image::new().rect(square(
                                0.0,
                                white_dy * window_scale,
                                TAKEN_TILE_SIZE * window_scale,
                            ));
                            white_dy += TAKEN_TILE_SIZE;
                            img.draw(tex, &DrawState::default(), white_trans, graphics);
                        } else {
                            let img = Image::new().rect(square(
                                0.0,
                                black_dy * window_scale,
                                TAKEN_TILE_SIZE * window_scale,
                            ));
                            black_dy += TAKEN_TILE_SIZE;
                            img.draw(tex, &DrawState::default(), black_trans, graphics);
                        }
                    }
                }
            }
        }

        {
            let (raw_x, raw_y) = raw_mouse_coords;
            if self.last_pressed.is_on_board() {
                if let Some(piece) = self.board[self.last_pressed] {
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
                                self.last_pressed
                            )));
                        }
                    }
                } else {
                    self.last_pressed = Coords::OffBoard;
                }
            }
        }

        if !errs.is_empty() {
            bail!("{errs:?}");
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
                        if let Either::Left(bo) = self.board.clone() {
                            self.board = Either::Right(bo.make_move(m));
                        } else {
                            bail!("need move update before can do: {m:?}");
                        }
                    }
                    BoardMessage::Move(outcome) => {
                        if let Either::Right(bo) = self.board.clone() {
                            match outcome {
                                MoveOutcome::Worked(taken) => {
                                    self.board = Either::Left(bo.move_worked(taken))
                                }
                                MoveOutcome::Invalid | MoveOutcome::CouldntProcessMove => {
                                    info!("Resetting pieces");
                                    self.board = Either::Left(bo.undo_move());
                                }
                            }
                        } else {
                            bail!("need move to update with outcome: {outcome:?}");
                        }
                    }
                    BoardMessage::NoConnectionList => {
                        self.board = Either::Left(no_connection_list())
                    }
                    BoardMessage::NewList(l) => self.board = Either::Left(Board::new_json(l)?),
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
}

///Converts a pixel to a board coordinate, assuming that the mouse cursor is on the board
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn to_board_coord(p: f64, mult: f64) -> u32 {
    (p / (BOARD_TILE_S * mult)).floor() as u32
}
