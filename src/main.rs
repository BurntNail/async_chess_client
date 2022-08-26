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
    clippy::too_many_lines
)]

///Module to hold [`Board`] struct
mod board;
///Module to hold [`Cacher`] struct
mod cacher;
///Module to hold chess-related utils
mod chess;
///Module to deal with configurator - [`AsyncChessLauncher`]
mod egui_launcher;
///Module to hold [`Either`]
mod either;
///Module to hold Error Extension traits
mod error_ext;
///Module to hold the [`ChessGame`] struct and deal with its logic
mod game;
///Module to hold the [`ListRefresher`] struct
mod list_refresher;
///Module to hold windowing/rendering logic for the [`ChessGame`]
mod piston;
///Module to deal with JSON responses from the server - [`JSONMove`], [`JSONPiece`], and [`JSONPieceList`]
mod server_interface;
///Module to hold structs which deal with time
mod time_based_structs;

#[macro_use]
extern crate tracing;

#[macro_use]
extern crate anyhow;

use crate::error_ext::ErrorExt;
use anyhow::Result;
use anyhow::{Context, Error};
use directories::ProjectDirs;
use egui_launcher::egui_main;
use error_ext::ToAnyhowNotErr;
use piston::{piston_main, PistonConfig};
use serde_json::from_str;
use std::env::{args, set_var, var};
use std::fs::read_to_string;
use tracing_subscriber::{prelude::*, EnvFilter, Registry};
use tracing_tree::HierarchicalLayer;

fn main() {
    setup_logging_tracing().eprint_exit();

    info!("Thanks to Devil's Workshop for the Chess Assets!");

    start();
}

///Function to setup all of the logging and tracing for the program
///
/// - Firstly, it sets the environment variables `RUST_LIB_BACKTRACE` to `1` and `RUST_LOG` to `info`
/// - Then it sets up an Environment tracing logger with Tracing Tree
///
/// # Errors
/// Can return an error if the tracing [`Registry`] fails to initialise, and this happens when:
/// > `This method returns an error if a global default subscriber has already been set, or if a log logger has already been set (when the "tracing-log" feature is enabled).`
#[tracing::instrument]
fn setup_logging_tracing() -> Result<(), Error> {
    for (k, v) in &[("RUST_LIB_BACKTRACE", "1"), ("RUST_LOG", "info")] {
        if var(k).is_err() {
            println!("Setting {k} to {v}");
            set_var(k, v);
        }
    }

    Registry::default()
        .with(EnvFilter::builder().from_env()?)
        .with(
            HierarchicalLayer::new(1)
                .with_targets(true)
                .with_bracketed_fields(true)
                .with_verbose_entry(true)
                .with_ansi(true), // .with_filter(Level::INFO.into())
        )
        .try_init()?;

    Ok(())
}

///Function to run the game.
///
/// - It checks whether or not the conf argument was passed, and if so it starts up the [`egui_main`] which launches an [`AsyncChessLauncher`]
/// - If not, then it checks if a configuration exists (and is valid), and if so it starts up the [`piston_main`] with the found configuration.
/// - If not, then it goes for the [`egui_main`]
///
/// When launching [`egui_main`] an Optional [`PistonConfig`] is passed in, and if it is `Some`, then the default values in the window are set to that of the [`PistonConfig`]
#[tracing::instrument]
fn start() {
    let user_wants_conf = args()
        .nth(1)
        .and_then(|s| s.chars().next())
        .map_or(false, |c| c == 'c');

    let uc = match read_config() {
        Ok(c) => Some(c),
        Err(e) => {
            error!(%e, "Error in config");
            None
        }
    };
    info!(%user_wants_conf, ?uc);

    if let Some(uc) = uc {
        if !user_wants_conf {
            piston_main(uc);
            return;
        }
    }

    egui_main(uc);
}

///Function to read in the config
///
/// Reads in the configuration path from `("com", "jackmaguire", "async_chess")` with [`ProjectDirs`] using the `config_dir` and a filename of `config.json`
///
/// # Errors
/// All Errors take the form of [`anyhow::Error`], with a relevant [`anyhow::Context`]
///
/// Can return an error if:
/// - Cannot find [`ProjectDirs`] - the [`Option`] is turned to a [`anyhow::Result`]
/// - Cannot read in the contents of the path using [`read_to_string`]
/// - Cannot parse the contents using [`from_str`] into a [`PistonConfig`]
#[tracing::instrument]
fn read_config() -> Result<PistonConfig> {
    let conf_path = ProjectDirs::from("com", "jackmaguire", "async_chess")
        .ae()
        .context("finding project dirs")?
        .config_dir()
        .join("config.json");
    let cntnts =
        read_to_string(&conf_path).with_context(|| format!("reading path {conf_path:?}"))?;
    from_str::<PistonConfig>(&cntnts).with_context(|| format!("reading contents {cntnts}"))
}
