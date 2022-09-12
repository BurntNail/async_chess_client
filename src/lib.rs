#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::derivable_impls,
    clippy::missing_docs_in_private_items,
    // clippy::missing_doc_code_examples
)]
#![allow(
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::use_self,
    clippy::too_many_lines,
    clippy::needless_pass_by_value
)]
//! Async Chess Client
//!
//! Async for people playing not at the same time, not for using tokio

//TODO: add docu-examples

#![macro_use]
extern crate epac_utils;

///Module to hold all chess-related modules
pub mod chess;
///Module to hold all networking modules
pub mod net;

///Module to hold commonly used structs, enums and functions that should be in a prelude
pub mod prelude {
    pub use crate::{
        chess::{
            chess_piece::{ChessPiece, ChessPieceKind},
            coords::Coords,
        },
    };
    pub use anyhow::{Error, Result};
    pub use std::error::Error as SError;
}

///Module to hold trait private contents
pub(crate) mod crate_private {
    ///Trait that library users can't implement
    pub trait Sealed {}
}

#[macro_use]
extern crate tracing;

#[macro_use]
extern crate anyhow;
