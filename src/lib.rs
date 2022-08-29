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

///Module to hold [`board::Board`] struct
pub mod board;
///Module to hold [`cacher::Cacher`] struct
pub mod cacher;
///Module to hold chess-related utils
pub mod chess;
///Module to hold [`either::Either`]
pub mod either;
///Module to hold Error Extension traits
pub mod error_ext;
///Module to hold the [`list_refresher::ListRefresher`] struct
pub mod list_refresher;
///Module to deal with JSON responses from the server - [`server_interface::JSONMove`], [`server_interface::JSONPiece`], and [`server_interface::JSONPieceList`]
pub mod server_interface;
///Module to hold structs which deal with time
pub mod time_based_structs;
///Module to hold a container for the board for ease of use and utility functions
pub mod board_container;
///Module to hold coordinates
pub mod coords;

#[macro_use]
extern crate tracing;

#[macro_use]
extern crate anyhow;

///Module to hold trait private contents
pub(crate) mod crate_private {
    ///Trait that library users can't implement
    pub trait Sealed {}
}
