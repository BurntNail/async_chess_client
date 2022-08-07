mod cacher;
mod chess;
mod game;

#[macro_use]
extern crate tracing;

use piston_window::{PistonWindow, WindowSettings};
use crate::game::ChessGame;

#[tokio::main]
async fn main() {
    info!("Thanks to Devil's Workshop for the Chess Assets!");

    let mut win: PistonWindow = WindowSettings::new("Async Chess", [400, 400])
        .exit_on_esc(true)
        .resizable(true)
        .build()
        .unwrap_or_else(|e| {
            error!("Error making window: {e}");
            std::process::exit(1);
        });
    let mut game = ChessGame::new(&mut win);
}
