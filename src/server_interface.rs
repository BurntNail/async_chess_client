use crate::{chess::{ChessPiece, ChessPieceKind}, board::{Board, Coords}};
use crate::error_ext::{ErrorExt, ToAnyhowNotErr};
use anyhow::Result;
use anyhow::{Context, Error};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug, Default)]
pub struct JSONPieceList(pub Vec<JSONPiece>);

#[derive(Deserialize, Debug)]
pub struct JSONPiece {
    pub x: i32,
    pub y: i32,
    pub kind: String,
    pub is_white: bool,
}

impl TryInto<Board> for JSONPieceList {
    type Error = Error;

    fn try_into(self) -> Result<Board, Self::Error> {
        Board::new_json(self)
    }
}

impl JSONPieceList {
    ///# Panics:
    ///Has the ability to panic, but if the server follows specs, should be fine
    #[allow(clippy::cast_sign_loss)]
    pub fn into_game_list(self) -> Result<Vec<Option<ChessPiece>>> {
        let mut v = vec![None; 8 * 8];
        for p in self.0.into_iter().filter(|p| p.x != -1 && p.y != -1) {
            let idx = (8 * p.y + p.x) as usize;
            let current = v.get_mut(idx).ae().context("getting index from vector in into_game_list")?;

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

    //TODO: Change this to read from JSON in data dir
    //TODO: Make a JSON Chess Editor

    Board::new_json(JSONPieceList(list)).context("turning ncl to board").unwrap_log_error()
}

#[derive(Serialize, Debug, PartialEq, Eq, Clone, Copy)]
pub struct JSONMove {
    pub id: u32,
    pub x: u32,
    pub y: u32,
    pub nx: u32,
    pub ny: u32,
}

impl JSONMove {
    pub const fn new(id: u32, x: u32, y: u32, nx: u32, ny: u32) -> Self {
        Self { id, x, y, nx, ny }
    }

    pub const fn current_coords (&self) -> Coords {
        (self.x, self.y)
    }
    pub const fn new_coords (&self) -> Coords {
        (self.nx, self.ny)
    }
}
