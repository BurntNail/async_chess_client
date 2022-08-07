use crate::chess::{ChessPiece, ChessPieceKind};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct JSONPieceList(Vec<JSONPiece>);

#[derive(Deserialize, Debug)]
pub struct JSONPiece {
    pub x: i32,
    pub y: i32,
    pub kind: String,
    pub is_white: bool,
}

impl JSONPieceList {
    ///# Panics:
    ///Has the ability to panic, but if the server follows specs, should be fine
    pub fn to_game_list(self) -> Vec<Option<ChessPiece>> {
        let mut v = vec![None; 8 * 8];
        for p in self.0 {
            if p.x < 0 || p.y < 0 {
                continue;
            }

            //PANIC: have checked above for > 0
            v[8 * p.y as usize + p.x as usize] = Some(ChessPiece {
                kind: ChessPieceKind::try_from(p.kind).expect("Server messed up"),
                is_white: p.is_white,
            });
        }

        v
    }
}

#[derive(Serialize)]
pub struct JSONMove {
    pub id: u32,
    pub x: u32,
    pub y: u32,
    pub nx: u32,
    pub ny: u32
}

impl JSONMove {
    pub fn new (id: u32, x: u32, y: u32, nx: u32, ny: u32) -> Self {
        Self {
            id,x,y,nx,ny
        }
    }
}