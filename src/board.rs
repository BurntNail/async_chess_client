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

///Utility type to hold a set of [`i8`] coordinates in an `(x, y)` format
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Coords(i8, i8);

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
        if x == -1 && y == -1 {
            return Ok(Self(-1, -1));
        }

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

        Ok(Self(x as i8, y as i8)) //conversion works as all checked above
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

        Ok(Self(x as i8, y as i8)) //conversion works as all checked above
    }
}

impl From<Coords> for (i8, i8) {
    fn from(c: Coords) -> Self {
        (c.0, c.1)
    }
}

impl Coords {
    ///Provides an index with which to index a 1D array using the 2D coords, assuming there are 8 rows per column
    #[must_use]
    pub fn to_usize(&self) -> Option<usize> {
        if self.0 == -1 || self.1 == -1 {
            None
        } else {
            Some((self.1 * 8 + self.0) as usize)
        }
    }
    ///Provides the X part of the coordinate
    #[must_use]
    pub const fn x(&self) -> i8 {
        self.0
    }
    ///Provides the Y part of the coordinate
    #[must_use]
    pub const fn y(&self) -> i8 {
        self.1
    }
}

///Struct to hold a Chess Board
pub struct Board {
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
}

impl Default for Board {
    fn default() -> Self {
        Self {
            pieces: [None; 64],
            taken: Vec::with_capacity(32),
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
            .get(index.to_usize().ae().context("index piece").unwrap_log_error())
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
            .get_mut(index.to_usize().ae().context("index piece").unwrap_log_error())
            .ae()
            .with_context(|| format!("Getting position mutably from {index:?}"))
            .unwrap_log_error()
    }
}

//TODO: Make this into a generic for Board<MakeMove> vs Board<DealWithMove>
impl Board {
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
    pub fn make_move(&mut self, m: JSONMove) {
        if self.previous.is_some() {
            Err::<(), _>("Move made without clearing").unwrap_log_error();
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
        } else {
            Err::<(), _>("undo move without move to undo").unwrap_log_error()
        }
    }

    ///Clears out the cache
    /// 
    /// # Panics
    /// Can panic if there wasn't a move made beforehand
    pub fn move_worked(&mut self, taken: bool) {
        if taken {
            let (_, p, _) = std::mem::take(&mut self.previous).ae().context("taking previous").unwrap_log_error();
            if let Some(p) = p {
                self.taken.push(p);
            }
        } else {
            self.previous = None;
        }
    }



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
    pub fn get_taken (&self) -> Vec<ChessPiece> {
        self.taken.clone()
    }
}
