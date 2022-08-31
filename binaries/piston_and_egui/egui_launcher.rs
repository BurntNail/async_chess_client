use anyhow::{Context, Result};
use async_chess_client::{prelude::ErrorExt, util::error_ext::ToAnyhowNotErr};
use directories::ProjectDirs;
use eframe::{egui, App};
use serde_json::to_string;
use std::{
    fs::{create_dir_all},
};

use crate::piston::PistonConfig;

///Function to start up an [`AsyncChessLauncher`] using [`eframe::run_native`]
#[tracing::instrument]
pub fn egui_main(uc: Option<PistonConfig>) {
    eframe::run_native(
        "Async Chess Configurator",
        eframe::NativeOptions::default(),
        Box::new(move |_cc| Box::new(AsyncChessLauncher::new(uc))),
    );
}

///Struct to run the Egui Configurator.
///
/// Holds Strings as that is what egui line-edits take
#[derive(Debug)]
struct AsyncChessLauncher {
    ///The game ID
    id: String,
    ///The width/height of the to-be-opened window
    res: String,
}

impl Default for AsyncChessLauncher {
    fn default() -> Self {
        Self {
            id: "0".into(),
            res: "600".into(),
        }
    }
}

impl AsyncChessLauncher {
    ///Function to create a new `AsyncChessLauncher`.
    ///
    ///If `start_uc` is [`Some`], then it uses those values, and if not then it uses the [`AsyncChessLauncher::default`] values - `id: 0, res: 600`
    pub fn new(start_uc: Option<PistonConfig>) -> Self {
        start_uc
            .map(|PistonConfig { id, res }| Self {
                id: id.to_string(),
                res: res.to_string(),
            })
            .unwrap_or_default()
    }
}

impl App for AsyncChessLauncher {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Asynchronous Chess!");
            ui.label("To play, enter the configuration and press start game, then re-open the app");
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Game ID: ");
                ui.text_edit_singleline(&mut self.id);

                if self.id.parse::<u32>().is_err() {
                    self.id.clear();
                }
            });
            ui.horizontal(|ui| {
                ui.label("Screen Width/Height: ");
                ui.text_edit_singleline(&mut self.res);

                if self.res.parse::<u32>().is_err() {
                    self.res.clear();
                }
            });

            ui.separator();

            if ui.button("Save and Exit.").clicked() {
                frame.quit();
            }
        });
    }

    #[tracing::instrument]
    fn on_exit(&mut self, gl: &eframe::glow::Context) {
        let pc = PistonConfig {
            //PANICS - we parse ^
            id: self.id.parse().unwrap(),
            res: self.res.parse().unwrap(),
        };

        std::thread::spawn(move || {
            write_conf_to_file(pc).error();
        });
    }
}

///Writes the given [`PistonConfig`] to a file.
///
/// # Errors
/// - Fail to get [`ProjectDirs`]
/// - Fail to [`create_dir_all`] on the config directory
/// - Fail to convert the [`PistonConfig`] to JSON with [`to_string`]
/// - Fail to open the file using the [`OpenOptions`]
/// - Fail to write to the file using [`write!`]
#[tracing::instrument]
fn write_conf_to_file(pc: PistonConfig) -> Result<()> {
    info!(?pc, "Writing config to disk");

    let cd = ProjectDirs::from("com", "jackmaguire", "async_chess")
        .ae()
        .context("getting project dirs")?;
    let cd = cd.config_dir(); //to avoid dropping temporary refs
    create_dir_all(cd).context("creating config directory")?;
    let path = cd.join("config.json");

    let st = to_string(&pc).with_context(|| format!("turning {pc:?} to string"))?;

    std::fs::write(&path, st).context("Write to file")
}
