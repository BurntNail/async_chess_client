use crate::{
    cacher::{Cacher, BOARD_S, TILE_S},
    chess::ChessPiece,
    server_interface::{JSONMove, JSONPieceList},
};
use graphics::DrawState;
use piston_window::{
    clear, rectangle::square, Context, G2d, GfxDevice, Image, PistonWindow, Size, Transformed,
};
use reqwest::Client;
use std::sync::RwLock;

pub struct ChessGame {
    id: u32,
    c: Cacher,
    // requests: HashMap<RequestType, Promise<reqwest::Result<String>>>,
    cached_pieces: RwLock<Vec<Option<ChessPiece>>>,
    last_pressed: Option<(u32, u32)>,
    client: Client,
}
impl ChessGame {
    pub fn new(win: &mut PistonWindow) -> Result<Self, find_folder::Error> {
        Ok(Self {
            id: 420, //TODO: opening menu, user chooses, maybe quick and dirty egui which launches the piston stuff
            c: Cacher::new_and_populate(win)?,
            // requests: Default::default(),
            cached_pieces: RwLock::new(vec![None; 64]),
            last_pressed: None,
            client: Client::new(),
        })
    }

    pub fn render(
        &mut self,
        size: Size,
        ctx: Context,
        graphics: &mut G2d,
        _device: &mut GfxDevice,
        mouse_coords: Option<(u32, u32)>,
    ) {
        let window_scale = size.height / BOARD_S;

        clear([0.0; 4], graphics);
        let t = ctx.transform;
        {
            let image = Image::new().rect(square(0.0, 0.0, 256.0 * window_scale));
            image.draw(
                self.c.get("board_alt.png").unwrap(),
                &DrawState::default(),
                t,
                graphics,
            );
        }

        let trans = t.trans(41.0 * window_scale, 41.0 * window_scale);

        {
            if let Some((px, py)) = mouse_coords {
                let x = f64::from(px) * (TILE_S + 2.0) * window_scale;
                let y = f64::from(py) * (TILE_S + 2.0) * window_scale;
                let image = Image::new().rect(square(x, y, 20.0 * window_scale));

                image.draw(
                    self.c.get("highlight.png").unwrap(),
                    &DrawState::default(),
                    trans,
                    graphics,
                );
            }
        }

        match self.cached_pieces.read() {
            Ok(lock) => {
                for col in 0..8 {
                    for row in 0..8 {
                        let idx = row * 8 + col;
                        if let Some(piece) = lock[idx] {
                            match self.c.get(&piece.to_file_name()) {
                                None => {
                                    error!("Cacher doesn't contain: {}", piece.to_file_name());
                                }
                                Some(tex) => {
                                    let x = col as f64 * (TILE_S + 2.0) * window_scale;
                                    let y = row as f64 * (TILE_S + 2.0) * window_scale;
                                    // let trans: Matrix2d = trans.trans(x, y); //TODO: account for scaling lol
                                    let image =
                                        Image::new().rect(square(x, y, 20.0 * window_scale));

                                    image.draw(tex, &DrawState::default(), trans, graphics);

                                    if let Some((lp_x, lp_y)) = self.last_pressed {
                                        if lp_x == col as u32 && lp_y == row as u32 {
                                            image.draw(
                                                self.c.get("selected.png").unwrap(),
                                                &DrawState::default(),
                                                trans,
                                                graphics,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error!("Unable to read vec: {e}");
            }
        }
    }

    pub async fn mouse_input(&mut self, mouse_pos: (f64, f64), size: Size) {
        let mult = size.height / BOARD_S;

        match std::mem::take(&mut self.last_pressed) {
            None => {
                let lp_x = to_board_coord(mouse_pos.0, mult);
                let lp_y = to_board_coord(mouse_pos.1, mult);

                match self.cached_pieces.read() {
                    Ok(lock) => {
                        if lock.get(lp_y as usize * 8 + lp_x as usize).is_some() {
                            self.last_pressed = Some((lp_x, lp_y));
                        }
                    }
                    Err(err) => error!("Unable to read cached pieces: {err}"),
                }
            }
            Some(lp) => {
                //Deal with second press
                let current_press = {
                    let lp_x = to_board_coord(mouse_pos.0, mult);
                    let lp_y = to_board_coord(mouse_pos.1, mult);
                    (lp_x, lp_y)
                };

                info!("Dealing with a move from {lp:?} to {current_press:?}");

                let rsp = self
                    .client
                    .post("http://109.74.205.63:12345/movepiece")
                    .json(&JSONMove::new(
                        self.id,
                        lp.0,
                        lp.1,
                        current_press.0,
                        current_press.1,
                    ))
                    .send()
                    .await;
                match rsp {
                    Ok(response) => {
                        info!("Update from server on moving: {:?}", response.text().await);
                    }
                    Err(e) => {
                        error!("Error in input response {e}");
                    }
                }
            }
        }
    }

    ///Should be called ASAP after instantiating game, and often afterwards
    pub async fn update_list(&mut self) -> Result<(), reqwest::Error> {
        let result = self
            .client
            .get(format!("http://109.74.205.63:12345/games/{}", self.id))
            .send()
            .await?
            //     .text()
            //     .await?;
            // info!("Got {result} from server");
            .json::<JSONPieceList>()
            .await;
        match result {
            Ok(jpl) => match self.cached_pieces.write() {
                Ok(mut lock) => {
                    *lock = jpl.to_game_list();
                }
                Err(e) => {
                    error!("Unable to populate due to {e}");
                }
            },
            Err(e) => error!("Unable to parse result to a valid JSONPieceList: {e}"),
        }

        Ok(())
    }

    pub async fn restart_board(&mut self) -> Result<(), reqwest::Error> {
        let rsp = self
            .client
            .post("http://109.74.205.63:12345/newgame")
            .body(self.id.to_string())
            .send()
            .await?;
        info!("Update from server on restarting: {:?}", rsp.text().await);
        Ok(())
    }

    pub fn clear_mouse_input(&mut self) {
        self.last_pressed = None;
    }
}

pub fn to_board_coord(p: f64, mult: f64) -> u32 {
    let tile_size = (TILE_S + 2.0) * mult;
    (p / tile_size).floor() as u32
}
