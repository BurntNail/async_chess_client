use crate::{
    board::{Board, Coords},
    chess::{ChessPiece, ChessPieceKind},
    error_ext::{ErrorExt, ToAnyhowNotErr},
};
use anyhow::{Context, Error, Result};
use serde::{Deserialize, Serialize};

///Unit struct to hold a vector of [`JSONPiece`]s.
#[derive(Deserialize, Debug, Default)]
pub struct JSONPieceList(pub Vec<JSONPiece>);

///A piece in JSON representation
#[derive(Deserialize, Debug)]
pub struct JSONPiece {
    ///The x position
    pub x: i32,
    ///The y position
    pub y: i32,
    ///The kind of the piece as a String
    pub kind: String,
    ///Whether or not the piece is white
    pub is_white: bool,
}

impl TryInto<Board> for JSONPieceList {
    type Error = Error;

    fn try_into(self) -> Result<Board, Self::Error> {
        Board::new_json(self)
    }
}

impl JSONPieceList {
    ///Converts into a true board for the [`chess::Board`].
    ///
    /// # Errors
    /// Can return an error for any collisions or if the pieces are out of bounds
    ///
    /// # Panics
    /// Has the ability to panic, but if the server follows specs, should be fine
    #[allow(clippy::cast_sign_loss)]
    pub fn into_game_list(self) -> Result<[Option<ChessPiece>; 64]> {
        let mut v = [None; 8 * 8];
        for p in self.0 {
            let current = v
                .get_mut(Coords::try_from((p.x, p.y))?.to_usize())
                .ae()
                .context("getting index from vector in into_game_list")?;

            if current.is_some() {
                bail!("Collision at ({}, {})", p.x, p.y);
            }

            *current = Some(ChessPiece {
                kind: ChessPieceKind::try_from(p.kind)?,
                is_white: p.is_white,
            });
        }

        Ok(v)
    }
}

///Returns a Board that says Uh Oh.
///
/// # Panics:
/// - Shouldn't if list is correct, but might if the list is invalid and fails [`JSONPieceList::into_game_list`]
#[must_use]
pub fn no_connection_list() -> Board {
    let p = |x, y| JSONPiece {
        x,
        y,
        is_white: (x + y) % 2 == 1, //why not
        kind: "rook".into(),
    };
    let list = vec![
        p(0, 0),
        p(2, 0),
        p(5, 0),
        p(7, 0),
        p(0, 1),
        p(2, 1),
        p(5, 1),
        p(6, 1),
        p(7, 1),
        p(0, 2),
        p(1, 2),
        p(2, 2),
        p(5, 2),
        p(7, 2),
        p(0, 5),
        p(1, 5),
        p(2, 5),
        p(5, 5),
        p(7, 5),
        p(0, 6),
        p(2, 6),
        p(5, 6),
        p(6, 6),
        p(7, 6),
        p(0, 7),
        p(1, 7),
        p(2, 7),
        p(5, 7),
        p(7, 7),
    ];

    Board::new_json(JSONPieceList(list))
        .context("turning ncl to board")
        .unwrap_log_error()
}

///JSON repr of a chess move
#[derive(Serialize, Debug, PartialEq, Eq, Clone, Copy)]
pub struct JSONMove {
    ///Game ID
    pub id: u32,
    ///Starting X position
    pub x: u32,
    ///Starting Y position
    pub y: u32,
    ///X position to be moved to
    pub nx: u32,
    ///Y position to be moved to
    pub ny: u32,
}

impl JSONMove {
    ///Creates a new `JSONMove`
    #[must_use]
    pub const fn new(id: u32, x: u32, y: u32, nx: u32, ny: u32) -> Self {
        Self { id, x, y, nx, ny }
    }

    ///Gets the starting coordinates as a [`Coords`]
    #[must_use]
    pub fn current_coords(&self) -> Coords {
        (self.x, self.y).try_into().unwrap_log_error()
    }
    ///Gets the finishing coordinates as a [`Coords`]
    #[must_use]
    pub fn new_coords(&self) -> Coords {
        (self.nx, self.ny).try_into().unwrap_log_error()
    }
}
