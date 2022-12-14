use std::{
    marker::PhantomData,
    ops::{Index, IndexMut},
};
use anyhow::Context;
use epac_utils::error_ext::{ErrorExt, ToAnyhowNotErr};
use epac_utils::generic_enum;
use crate::{
    crate_private::Sealed,
    net::server_interface::{JSONMove, JSONPieceList},
    prelude::{ChessPiece, ChessPieceKind, Coords, Result},
};

generic_enum!(Sealed, (BoardMoveState -> "Holds the current state of moving pieces in the board to ensure no logic errors") => (CanMovePiece -> "The board can currently move a new piece"), (NeedsMoveUpdate -> "The board now needs an update on what happened to the piece it moved"));

///Struct to hold a Chess Board
#[derive(Clone, Debug)]
pub struct Board<STATE: BoardMoveState> {
    ///1D vector to hold all of the [`ChessPiece`]s - where the index of each piece is `y * 8 + x`
    ///
    ///`None` signifies no piece, and `Some` signifies a piece
    pieces: [Option<ChessPiece>; 64],

    ///vector to hold all the pieces which have been taken
    taken: Vec<ChessPiece>,

    ///Used to hold the contents and details of the previous move, in case the move was invalid
    ///
    ///Holds the move made, the piece taken, and what the original kind was
    previous: Option<(JSONMove, Option<ChessPiece>, ChessPieceKind)>,

    ///[`PhantomData`] to make sure `STATE` isn't optimised away
    _pd: PhantomData<STATE>,
}

impl Default for Board<CanMovePiece> {
    fn default() -> Self {
        Self {
            pieces: [None; 64],
            taken: Vec::with_capacity(32),
            previous: None,
            _pd: PhantomData,
        }
    }
}

impl<S: BoardMoveState> Index<Coords> for Board<S> {
    type Output = Option<ChessPiece>;

    ///Function to index the pieces
    ///
    /// # Panics
    /// Can panic if the coords are out-of-bounds, but very unlikely
    fn index(&self, index: Coords) -> &Self::Output {
        self.pieces
            .get(
                index
                    .to_usize()
                    .ae()
                    .context("index piece")
                    .unwrap_log_error(),
            )
            .unwrap_log_error_with_context(|| format!("Getting position from {index:?}"))
    }
}

impl<S: BoardMoveState> IndexMut<Coords> for Board<S> {
    ///Function to mutably index the pieces
    ///
    /// # Panics
    /// Can panic if the coords are out-of-bounds, but very unlikely
    fn index_mut(&mut self, index: Coords) -> &mut Self::Output {
        self.pieces
            .get_mut(
                index
                    .to_usize()
                    .ae()
                    .context("index piece")
                    .unwrap_log_error(),
            )
            .unwrap_log_error_with_context(|| format!("Getting position mutably from {index:?}"))
    }
}

//more like the rocket than the other examples
impl<STATE: BoardMoveState> Board<STATE> {
    ///Checks whether or not a piece exists at a given set of coordinates
    #[must_use]
    pub fn piece_exists_at_location(&self, coords: Coords) -> bool {
        if let Some(c) = coords.to_usize() {
            matches!(self.pieces.get(c), Some(Some(_)))
        } else {
            false
        }
    }

    ///Gets a clone of all the pieces which have been taken
    #[must_use]
    pub fn get_taken(&self) -> Vec<ChessPiece> {
        self.taken.clone()
    }
}

impl Board<CanMovePiece> {
    ///Create a new board from a [`JSONPieceList`], using `JSONPieceList::into_game_list`
    ///
    /// # Errors
    /// If `into_game_list` fails, this will return that error.
    ///
    /// `into_game_list` can fail if any pieces are out-of-bounds, or there are collisions
    pub fn new_json(board: JSONPieceList) -> Result<Self> {
        let (pieces, taken) = board.into_game_list()?;
        Ok(Self {
            pieces,
            taken,
            ..Default::default()
        })
    }

    ///Makes a move using a given [`JSONMove`]
    ///
    /// - Firstly, finds the piece to be taken, and sets the cache to the details of that piece
    /// - Then, sets the piece at the new location to the piece at the current location
    /// - Then, checks for pawn promotion, and possibly promotes the pawn
    ///
    /// # Panics
    /// - Can panic if the move is OOB, or there is no piece at the current location, or the last move wasn't cleared
    #[tracing::instrument(skip(self))]
    pub fn make_move(mut self, m: JSONMove) -> Board<NeedsMoveUpdate> {
        if self.previous.is_some() {
            Err::<(), _>(anyhow!("Move made without clearing")).unwrap_log_error();
        }

        self.previous = Some((
            m,
            self[m.new_coords()],
            self[m.current_coords()]
                .ae()
                .context("getting current piece")
                .unwrap_log_error()
                .kind,
        ));

        let old_current = std::mem::take(&mut self[m.current_coords()]);
        self[m.new_coords()] = old_current;

        if let Some(p) = &mut self[m.new_coords()] {
            //rather than unwrap to get a mutable reference
            if (p.is_white && m.ny == 0) || (!p.is_white && m.ny == 7) {
                p.kind = ChessPieceKind::Queen;
            }
        }

        Board {
            pieces: self.pieces,
            taken: self.taken,
            previous: self.previous,
            _pd: PhantomData,
        }
    }
}

impl Board<NeedsMoveUpdate> {
    ///Undos the most recent move
    ///
    /// # Errors
    /// Can return an error if there is no longer a piece at the coordinates the piece was moved to
    #[must_use]
    pub fn undo_move(mut self) -> Board<CanMovePiece> {
        if let Some((m, taken, old_kind)) = std::mem::take(&mut self.previous) {
            self[m.current_coords()] = self[m.new_coords()];
            self[m.new_coords()] = taken;

            if let Some(piece) = &mut self[m.current_coords()] {
                piece.kind = old_kind;
            }
        } else {
            Err::<(), _>(anyhow!("undo move without move to undo")).unwrap_log_error();
        }

        Board {
            pieces: self.pieces,
            taken: self.taken,
            previous: self.previous,
            _pd: PhantomData,
        }
    }

    ///Clears out the cache
    ///
    /// # Panics
    /// Can panic if there wasn't a move made beforehand
    #[must_use]
    pub fn move_worked(mut self, taken: bool) -> Board<CanMovePiece> {
        if taken {
            let (_, p, _) = std::mem::take(&mut self.previous)
                .ae()
                .context("taking previous")
                .unwrap_log_error();
            if let Some(p) = p {
                self.taken.push(p);
            }
        } else {
            self.previous = None;
        }

        Board {
            pieces: self.pieces,
            taken: self.taken,
            previous: self.previous,
            _pd: PhantomData,
        }
    }
}
