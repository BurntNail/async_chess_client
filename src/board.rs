use std::ops::{Index, IndexMut};
use anyhow::{Result, Context};
use crate::{chess::{ChessPiece, ChessPieceKind}, server_interface::{JSONMove, JSONPieceList}, error_ext::{ToAnyhowNotErr, ErrorExt}};

pub type Coords = (u32, u32);

pub struct Board {
    pieces: Vec<Option<ChessPiece>>,
    previous: Option<(JSONMove, Option<ChessPiece>, bool)> //move made, piece taken (at nx and ny), original piece was queen
}

impl Default for Board {
    fn default() -> Self {
        Self { pieces: vec![None; 64], previous: Default::default() }
    }
}

impl Index<Coords> for Board {
    type Output = Option<ChessPiece>;

    fn index(&self, index: Coords) -> &Self::Output {
        self.pieces.get(u32_to_idx(index)).ae().with_context(|| format!("Getting position from {index:?}")).unwrap_log_error()
    }
}

impl IndexMut<Coords> for Board {
    fn index_mut(&mut self, index: Coords) -> &mut Self::Output {
        self.pieces.get_mut(u32_to_idx(index)).ae().with_context(|| format!("Getting position mutably from {index:?}")).unwrap_log_error()
    }
}

impl Board {
    pub fn new_json (board: JSONPieceList) -> Result<Self> {
        Ok(Self {
            pieces: board.into_game_list()?,
            ..Default::default()
        })
    }

    #[tracing::instrument(skip(self))]
    pub fn make_move (&mut self, m: JSONMove) {
        let current = self[m.current_coords()];
        let was_queen = current.map_or(false, |p| p.kind == ChessPieceKind::Queen);
        self.previous = Some((m, current, was_queen));

        self[m.new_coords()] = std::mem::take(&mut self[m.current_coords()]);
    }

    pub fn undo_move (&mut self) {
        if let Some((m, taken, was_queen)) = std::mem::take(&mut self.previous) {
            self[m.current_coords()] = self[m.new_coords()];
            self[m.new_coords()] = taken;

            if was_queen {
                if let Some(to_be_queen) = self.pieces.get_mut(u32_to_idx(m.current_coords())) {
                    if let Some(to_be_queen) = to_be_queen {
                        to_be_queen.kind = ChessPieceKind::Queen;
                    } else {
                        error!("Piece no longer present in board")
                    }
                } else {
                    error!("Piece no longer present at index")
                }
            }
        }
    }

    pub fn move_worked (&mut self) {
        self.previous = None;
    }

    pub fn piece_exists_at_location (&self, coords: Coords) -> bool {
        matches!(
            self.pieces.get(u32_to_idx(coords)),
            Some(Some(_))
        )
    }

}


const fn u32_to_idx ((x, y): Coords) -> usize {
    (y * 8 + x) as usize
}