use std::{
    error::Error as SError,
    fmt::{Debug, Formatter},
};
use strum::{Display, EnumIter, IntoEnumIterator};

#[derive(EnumIter, Display, Copy, Clone, PartialEq, Eq)]
pub enum ChessPieceKind {
    Bishop,
    Knight,
    Pawn,
    Queen,
    King,
    Rook,
}

#[derive(Debug, Display)]
pub enum ChessPieceKindParseError {
    FailedMatch(String),
}

impl SError for ChessPieceKindParseError {}

impl TryFrom<String> for ChessPieceKind {
    type Error = ChessPieceKindParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let value = value.trim().to_lowercase();
        match value.as_str() {
            "bishop" => Ok(Self::Bishop),
            "knight" => Ok(Self::Knight),
            "pawn" => Ok(Self::Pawn),
            "queen" => Ok(Self::Queen),
            "king" => Ok(Self::King),
            "rook" => Ok(Self::Rook),
            _ => Err(ChessPieceKindParseError::FailedMatch(value)),
        }
    }
}

#[derive(Copy, Clone)]
pub struct ChessPiece {
    pub kind: ChessPieceKind,
    pub is_white: bool,
}
impl ChessPiece {
    pub fn all_variants() -> Vec<Self> {
        let mut v = Vec::with_capacity(12);

        for el in ChessPieceKind::iter() {
            v.push(Self {
                kind: el,
                is_white: false,
            });
            v.push(Self {
                kind: el,
                is_white: true,
            });
        }

        v
    }

    pub fn to_file_name(self) -> String {
        format!(
            "{}_{}.png",
            if self.is_white { "white" } else { "black" },
            self.kind.to_string().to_lowercase()
        )
    }
}

impl Debug for ChessPiece {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChessPiece")
            .field("kind", &self.kind.to_string())
            .field("is_white", &self.is_white)
            .finish()
    }
}
