#![warn(clippy::all, clippy::pedantic, clippy::derivable_impls)]
#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::use_self,
    clippy::too_many_lines,
    clippy::needless_pass_by_value
)]

mod cacher;
mod chess;
mod egui_launcher;
mod game;
mod list_refresher;
mod piston;
mod server_interface;
mod time_based_structs;
mod error_ext;

#[macro_use]
extern crate tracing;

#[macro_use]
extern crate anyhow;

use anyhow::{Error, Context};
use directories::ProjectDirs;
use egui_launcher::egui_main;
use error_ext::ToAnyhow;
use piston::{piston_main, PistonConfig};
use serde_json::from_str;
use std::env::{args, set_var, var};
use std::fs::read_to_string;
use tracing_subscriber::{prelude::*, EnvFilter, Registry};
use tracing_tree::HierarchicalLayer;
use anyhow::Result;
use crate::error_ext::ErrorExt;

fn main() {
    setup_logging_tracing().eprint_exit();

    info!("Thanks to Devil's Workshop for the Chess Assets!");

    start();
}

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

#[tracing::instrument]
fn start() {
    let user_wants_conf = args()
        .nth(1)
        .and_then(|s| s.chars().next())
        .map_or(false, |c| c == 'c');

    info!(%user_wants_conf, a=?args());

    let uc = match read_config() {
        Ok(c) => {
            if user_wants_conf {
                Some(c)
            } else {
                info!("Running Async Chess");
                piston_main(c);
                return;
            }
        }
        Err(e) => {
            error!(%e, "Error finding config");
            None
        }
    };

    info!("Running EGUI Config");
    egui_main(uc);
}

#[tracing::instrument]
fn read_config() -> Result<PistonConfig> {
    let conf_path = ProjectDirs::from("com", "jackmaguire", "async_chess")
        .to_ae_display().context("finding project dirs")?.config_dir().join("config.json");
    let cntnts = read_to_string(&conf_path).with_context(|| format!("reading path {conf_path:?}"))?;
    from_str::<PistonConfig>(&cntnts).with_context(|| format!("reading contents {cntnts}"))
}
