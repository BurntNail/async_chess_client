use crate::{
    chess::{ChessPiece, ChessPieceKind},
    eyre,
};
use color_eyre::Report;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug, Default)]
pub struct JSONPieceList(pub Vec<JSONPiece>);

#[derive(Deserialize, Debug)]
pub struct JSONPiece {
    pub x: u32,
    pub y: u32,
    pub kind: String,
    pub is_white: bool,
}

impl JSONPieceList {
    ///# Panics:
    ///Has the ability to panic, but if the server follows specs, should be fine
    #[allow(clippy::wrong_self_convention)]
    pub fn to_game_list(self) -> Result<Vec<Option<ChessPiece>>, Report> {
        let mut v = vec![None; 8 * 8];
        for p in self.0 {
            let idx = (8 * p.y + p.x) as usize;
            let current = v.get_mut(idx).expect("Jack has messed up his maths");

            if current.is_some() {
                return Err(eyre!("Collision at ({}, {})", p.x, p.y));
            }

            *current = Some(ChessPiece {
                kind: ChessPieceKind::try_from(p.kind)?,
                is_white: p.is_white,
            });
        }

        Ok(v)
    }

    pub fn no_connection_list() -> Vec<Option<ChessPiece>> {
        let p = |x, y| JSONPiece {
            x,
            y,
            is_white: false,
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

        JSONPieceList(list)
            .to_game_list()
            .expect("Error in list boi")
    }
}

#[derive(Serialize)]
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
}
