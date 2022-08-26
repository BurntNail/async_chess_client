use crate::{
    chess::{ChessPiece, ChessPieceKind},
    error_ext::{ErrorExt, ToAnyhowNotErr},
    server_interface::{JSONMove, JSONPieceList},
};
use anyhow::{Context, Result};
use std::ops::{Index, IndexMut};

//TODO: turn Coords to struct
//TODO: turn pieces to array

///Utility type to hold a set of [`u32`] coordinates in an `(x, y)` format
pub type Coords = (u32, u32);

///Struct to hold a Chess Board
pub struct Board {
    ///1D vector to hold all of the [`ChessPiece`]s - where the index of each piece is `y * 8 + x`
    ///
    ///`None` signifies no piece, and `Some` signifies a piece
    pieces: Vec<Option<ChessPiece>>,
    ///Used to hold the contents and details of the previous move, in case the move was invalid
    ///
    ///Holds the move made, the piece taken, and what the original kind was
    previous: Option<(JSONMove, Option<ChessPiece>, ChessPieceKind)>,
}

impl Default for Board {
    fn default() -> Self {
        Self {
            pieces: vec![None; 64],
            previous: None,
        }
    }
}

impl Index<Coords> for Board {
    type Output = Option<ChessPiece>;

    ///Function to index the pieces
    ///
    /// # Panics
    /// Can panic if the coords are out-of-bounds, but very unlikely
    fn index(&self, index: Coords) -> &Self::Output {
        self.pieces
            .get(u32_to_idx(index))
            .ae()
            .with_context(|| format!("Getting position from {index:?}"))
            .unwrap_log_error()
    }
}

impl IndexMut<Coords> for Board {
    ///Function to mutably index the pieces
    ///
    /// # Panics
    /// Can panic if the coords are out-of-bounds, but very unlikely
    fn index_mut(&mut self, index: Coords) -> &mut Self::Output {
        self.pieces
            .get_mut(u32_to_idx(index))
            .ae()
            .with_context(|| format!("Getting position mutably from {index:?}"))
            .unwrap_log_error()
    }
}

impl Board {
    ///Create a new board from a [`JSONPieceList`], using `JSONPieceList::into_game_list`
    ///
    /// # Errors
    /// If `into_game_list` fails, this will return that error.
    ///
    /// `into_game_list` can fail if any pieces are out-of-bounds, or there are collisions
    pub fn new_json(board: JSONPieceList) -> Result<Self> {
        Ok(Self {
            pieces: board.into_game_list()?,
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
    /// - Can panic if the move is OOB, or there is no piece at the current location
    #[tracing::instrument(skip(self))]
    pub fn make_move(&mut self, m: JSONMove) {
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
        let nu = &mut self[m.new_coords()];
        *nu = old_current;

        if let Some(p) = nu {
            //rather than unwrap to get a mutable reference
            if (p.is_white && m.ny == 0) || (!p.is_white && m.ny == 7) {
                p.kind = ChessPieceKind::Queen;
            }
        }
    }

    ///Undos the most recent move
    ///
    /// # Errors
    /// Can return an error if there is no longer a piece at the coordinates the piece was moved to
    pub fn undo_move(&mut self) {
        if let Some((m, taken, old_kind)) = std::mem::take(&mut self.previous) {
            self[m.current_coords()] = self[m.new_coords()];
            self[m.new_coords()] = taken;

            if let Some(piece) = &mut self[m.current_coords()] {
                piece.kind = old_kind;
            }
        }
    }

    ///Clears out the cache
    pub fn move_worked(&mut self) {
        self.previous = None;
    }

    ///Checks whether or not a piece exists at a given set of coordinates
    pub fn piece_exists_at_location(&self, coords: Coords) -> bool {
        matches!(self.pieces.get(u32_to_idx(coords)), Some(Some(_)))
    }
}

///Converts a set of [`Coords`] to a [`usize`] for indexing
pub const fn u32_to_idx((x, y): Coords) -> usize {
    (y * 8 + x) as usize
}
