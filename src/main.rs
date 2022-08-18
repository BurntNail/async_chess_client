#![warn(clippy::all, clippy::pedantic)]
#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::use_self
)]

mod cacher;
mod chess;
mod egui_launcher;
mod game;
mod memory_cacher;
mod piston;
mod server_interface;

#[macro_use]
extern crate tracing;

pub use color_eyre::eyre::eyre;
use color_eyre::{install, Report};
use directories::ProjectDirs;
use egui_launcher::egui_main;
use piston::{piston_main, PistonConfig};
use serde_json::from_str;
use std::env::{args, set_var, var};
use tokio::fs::read_to_string;
use tracing_subscriber::{prelude::*, EnvFilter, Registry};
use tracing_tree::HierarchicalLayer;

#[tokio::main]
async fn main() {
    if let Err(e) = setup_logging_tracing().await {
        println!("Unable to setup logging/tracing: {e}");
        std::process::exit(1);
    }

    info!("Thanks to Devil's Workshop for the Chess Assets!");

    start().await;
}

#[tracing::instrument]
async fn setup_logging_tracing() -> Result<(), Report> {
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

    install()?;

    Ok(())
}

#[tracing::instrument]
async fn start() {
    let user_wants_conf = args()
        .nth(1)
        .and_then(|s| s.chars().next())
        .map_or(false, |c| c == 'c');

    info!(%user_wants_conf, a=?args());

    let uc = match read_config().await {
        Ok(c) => {
            if user_wants_conf {
                Some(c)
            } else {
                info!("Running Async Chess");
                return piston_main(c).await;
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
async fn read_config() -> Result<PistonConfig, Report> {
    match ProjectDirs::from("com", "jackmaguire", "async_chess") {
        Some(cd) => {
            let path = cd.config_dir().join("config.json");
            match read_to_string(&path).await {
                Ok(cntnts) => match from_str::<PistonConfig>(&cntnts) {
                    Ok(pc) => Ok(pc),
                    Err(e) => Err(eyre!("Error reading {cntnts:?}: {e}")),
                },
                Err(e) => Err(eyre!("Error reading {path:?}: {e}")),
            }
        }
        None => Err(eyre!("Unable to find project dirs")),
    }
}
