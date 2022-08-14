use crate::piston::PistonConfig;
use directories::ProjectDirs;
use eframe::App;
use serde_json::to_string;
use std::{
    fs::{create_dir_all, OpenOptions},
    io::Write,
};

#[tracing::instrument]
pub fn egui_main() {
    eframe::run_native(
        "Async Chess Configurator",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Box::new(AsyncChessLauncher::new())),
    );
}

#[derive(Debug)]
struct AsyncChessLauncher {
    id: String,
    res: String,
}

impl AsyncChessLauncher {
    pub fn new() -> Self {
        Self {
            id: "0".into(),
            res: "600".into(),
        }
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

            if ui.button("Start Game.").clicked() {
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
        write_conf_to_file(pc);
    }
}

#[tracing::instrument]
fn write_conf_to_file(pc: PistonConfig) {
    std::thread::spawn(move || {
        info!(?pc, "Writing config to disk");

        match to_string(&pc) {
            Ok(st) => match ProjectDirs::from("com", "jackmaguire", "async_chess") {
                Some(cd) => {
                    let path = cd.config_dir();

                    match create_dir_all(path) {
                        Ok(_) => {
                            let path = path.join("config.json");

                            let oo = OpenOptions::new()
                                .create(true)
                                .write(true)
                                .open(&path)
                                .map_err(|ioe| ioe.kind());

                            match oo {
                                Ok(mut f) => {
                                    if let Err(e) = write!(f, "{}", st.as_str()) {
                                        error!(%st, %e, "Error writing to file");
                                    }
                                }
                                Err(e) => {
                                    error!(?path, error_kind=?e, "Unable to create file");
                                }
                            }
                        }
                        Err(e) => error!(%e, "Unable to create directory"),
                    }
                }
                None => error!("Unable to find project dirs"),
            },
            Err(e) => error!(config=?pc, %e, "Unable to get string repr through sj"),
        }
    });
}
