mod cacher;
mod chess;
mod from_server;
mod game;

#[macro_use]
extern crate tracing;

use crate::game::ChessGame;
use piston_window::{
    Button, MouseButton, MouseCursorEvent, PistonWindow, PressEvent, RenderEvent, Window,
    WindowSettings,
};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() {
    {
        let sub = FmtSubscriber::builder()
            .with_max_level(Level::DEBUG)
            .finish();
        tracing::subscriber::set_global_default(sub).expect("Setting sub failed!");
    }

    info!("Thanks to Devil's Workshop for the Chess Assets!");

    let mut win: PistonWindow = WindowSettings::new("Async Chess", [256, 256])
        .exit_on_esc(true)
        .resizable(true)
        .build()
        .unwrap_or_else(|e| {
            error!("Error making window: {e}");
            std::process::exit(1);
        });

    let mut game = ChessGame::new(&mut win).unwrap_or_else(|e| {
        error!("Error making game: {e}");
        std::process::exit(1);
    });
    game.populate().await.unwrap_or_else(|err| {
        error!("Unable to populate game: {err}");
        std::process::exit(1);
    });

    // let mut mouse_pos = (0.0, 0.0);
    // while let Some(e) = win.next() {
    //     let size = win.size();
    //
    //     if let Some(_r) = e.render_args() {
    //         win.draw_2d(&e, |c, g, device| {
    //             game.render(size, c, g, device);
    //         });
    //     }
    //
    //     if let Some(Button::Mouse(mb)) = e.press_args() {
    //         if mb == MouseButton::Right {
    //             game.clear_input();
    //         } else {
    //             let inp = (mouse_pos.0, mouse_pos.1);
    //             game.input(inp, size).await;
    //         }
    //     }
    //
    //     e.mouse_cursor(|p| mouse_pos = (p[0], p[1]));
    // }
}
