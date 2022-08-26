use std::{
    error::Error as SError,
    fmt::{Debug, Formatter},
};
use strum::{Display, EnumIter, IntoEnumIterator};

///Enum with all of the chess piece kinds
#[derive(EnumIter, Display, Copy, Clone, PartialEq, Eq)]
pub enum ChessPieceKind {
    Bishop,
    Knight,
    Pawn,
    Queen,
    King,
    Rook,
}

///Enum to hold errors for chess piece kinds
#[derive(Debug, Display)]
pub enum ChessPieceKindParseError {
    ///Failed to find a match
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

///Struct to hold a chess piece
#[derive(Copy, Clone)]
pub struct ChessPiece {
    ///The kind of the chess piece
    pub kind: ChessPieceKind,
    ///Whether or not this is a white piece.
    pub is_white: bool,
}
impl ChessPiece {
    ///Gets all of the variants of a [`ChessPiece`] - each of the variants of [`ChessPieceKind`] with one black and one white
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

    ///Converts a [`ChessPiece`] to a file name
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
