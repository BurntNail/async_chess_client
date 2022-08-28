use crate::{
    chess::{ChessPiece, ChessPieceKind},
    error_ext::{ErrorExt, ToAnyhowNotErr},
    server_interface::{JSONMove, JSONPieceList},
};
use anyhow::{Context, Result};
use std::{
    fmt::Debug,
    ops::{Index, IndexMut},
};

///Utility type to hold a set of [`u32`] coordinates in an `(x, y)` format
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Coords(u32, u32);

impl Debug for Coords {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Coords")
            .field("x", &self.0)
            .field("y", &self.1)
            .finish()
    }
}

impl TryFrom<(i32, i32)> for Coords {
    type Error = anyhow::Error;

    fn try_from((x, y): (i32, i32)) -> Result<Self, Self::Error> {
        if x < 0 {
            bail!("x < 0")
        }
        if x > 7 {
            bail!("x > 7")
        }
        if y < 0 {
            bail!("y < 0")
        }
        if y > 7 {
            bail!("y > 7")
        }

        #[allow(clippy::cast_sign_loss)]
        Ok(Self(x as u32, y as u32)) //conversion works as all checked above
    }
}
impl TryFrom<(u32, u32)> for Coords {
    type Error = anyhow::Error;

    fn try_from((x, y): (u32, u32)) -> Result<Self, Self::Error> {
        if x > 7 {
            bail!("x > 7")
        }
        if y > 7 {
            bail!("y > 7")
        }

        Ok(Self(x as u32, y as u32)) //conversion works as all checked above
    }
}

impl From<Coords> for (u32, u32) {
    fn from(c: Coords) -> Self {
        (c.0, c.1)
    }
}

impl Coords {
    ///Provides an index with which to index a 1D array using the 2D coords, assuming there are 8 rows per column
    #[must_use]
    pub fn to_usize(&self) -> usize {
        (self.1 * 8 + self.0) as usize
    }
    ///Provides the X part of the coordinate
    #[must_use]
    pub const fn x(&self) -> u32 {
        self.0
    }
    ///Provides the Y part of the coordinate
    #[must_use]
    pub const fn y(&self) -> u32 {
        self.1
    }
}

///Struct to hold a Chess Board
pub struct Board {
    ///1D vector to hold all of the [`ChessPiece`]s - where the index of each piece is `y * 8 + x`
    ///
    ///`None` signifies no piece, and `Some` signifies a piece
    pieces: [Option<ChessPiece>; 64],
    ///Used to hold the contents and details of the previous move, in case the move was invalid
    ///
    ///Holds the move made, the piece taken, and what the original kind was
    previous: Option<(JSONMove, Option<ChessPiece>, ChessPieceKind)>,
}

impl Default for Board {
    fn default() -> Self {
        Self {
            pieces: [None; 64],
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
            .get(index.to_usize())
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
            .get_mut(index.to_usize())
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
    #[must_use]
    pub fn piece_exists_at_location(&self, coords: Coords) -> bool {
        matches!(self.pieces.get(coords.to_usize()), Some(Some(_)))
    }
}
