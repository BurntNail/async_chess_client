use std::ops::{Index, IndexMut};
use epac_utils::either::Either;
use crate::prelude::{ChessPiece, Coords};
use super::board::{Board, CanMovePiece, NeedsMoveUpdate};

///Struct to hold board states for utility purposes
pub type BoardContainer = Either<Board<CanMovePiece>, Board<NeedsMoveUpdate>>;

impl Default for BoardContainer {
    fn default() -> Self {
        Self::Left(Board::default())
    }
}

///Macro for use with [`BoardContainer`] that just repeats board functions
macro_rules! method_on_original_ref {
    ($func_name:ident $func_return:ty => $($arg_name:ident $arg_type:ty),*) => {
        impl BoardContainer {
            #[must_use]
            pub fn $func_name (&self, $($arg_name: $arg_type)*) -> $func_return {
                match self {
                    Self::Left(l) => l.$func_name($($arg_name,)*),
                    Self::Right(l) => l.$func_name($($arg_name),*),
                }
            }
        }
    };
}
///Macro for use with [`BoardContainer`] that just repeats board functions
macro_rules! method_on_original_mut_ref {
    ($func_name:ident $func_return:ty => $($arg_name:ident $arg_type:ty),*) => {
        impl BoardContainer {
            pub fn $func_name (&mut self, $($arg_name: $arg_type)*) -> $func_return {
                match self {
                    Self::Left(l) => l.$func_name($($arg_name,)*),
                    Self::Right(l) => l.$func_name($($arg_name),*),
                }
            }
        }
    };
}

method_on_original_ref!(piece_exists_at_location bool => coords Coords);
method_on_original_mut_ref!(get_taken Vec<ChessPiece> => );

impl Index<Coords> for BoardContainer {
    type Output = Option<ChessPiece>;

    fn index(&self, index: Coords) -> &Self::Output {
        match self {
            Either::Left(b) => b.index(index),
            Either::Right(b) => b.index(index),
        }
    }
}

impl IndexMut<Coords> for BoardContainer {
    fn index_mut(&mut self, index: Coords) -> &mut Self::Output {
        match self {
            Either::Left(b) => b.index_mut(index),
            Either::Right(b) => b.index_mut(index),
        }
    }
}
