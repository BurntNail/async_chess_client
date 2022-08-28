#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::derivable_impls,
    clippy::missing_docs_in_private_items
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

///Module to hold [`Board`] struct
pub mod board;
///Module to hold [`Cacher`] struct
pub mod cacher;
///Module to hold chess-related utils
pub mod chess;
///Module to hold [`Either`]
pub mod either;
///Module to hold Error Extension traits
pub mod error_ext;
///Module to hold the [`ListRefresher`] struct
pub mod list_refresher;
///Module to deal with JSON responses from the server - [`JSONMove`], [`JSONPiece`], and [`JSONPieceList`]
pub mod server_interface;
///Module to hold structs which deal with time
pub mod time_based_structs;

#[macro_use]
extern crate tracing;

#[macro_use]
extern crate anyhow;

///Module to hold trait private contents
pub(crate) mod crate_private {
    ///Trait that library users can't implement
    pub trait Sealed {}
}
