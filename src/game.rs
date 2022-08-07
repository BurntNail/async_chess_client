use crate::{
    cacher::{Cacher, BOARD_S, TILE_S},
    chess::ChessPiece,
    server_interface::JSONPieceList,
};
use piston_window::{clear, image, Context, G2d, GfxDevice, PistonWindow, Size, Transformed};
use std::sync::RwLock;
use reqwest::{Client};
use crate::server_interface::JSONMove;

pub struct ChessGame {
    id: u32,
    c: Cacher,
    // requests: HashMap<RequestType, Promise<reqwest::Result<String>>>,
    cached_pieces: RwLock<Vec<Option<ChessPiece>>>,
    last_pressed: Option<(u32, u32)>,
    client: Client
}
impl ChessGame {
    pub fn new(win: &mut PistonWindow) -> Result<Self, find_folder::Error> {
        Ok(Self {
            id: 420, //TODO: opening menu, user chooses
            c: Cacher::new_and_populate(win)?,
            // requests: Default::default(),
            cached_pieces: RwLock::new(vec![None; 64]),
            last_pressed: None,
            client: Client::new(),
        })
    }

    pub fn render(
        &mut self,
        _size: Size,
        ctx: Context,
        graphics: &mut G2d,
        _device: &mut GfxDevice,
    ) {
        clear([0.0; 4], graphics);
        {
            let t = ctx.transform;
            image(self.c.get("board_alt.png").unwrap(), t, graphics);

            let t = t.trans(42.0, 42.0);

            match self.cached_pieces.read() {
                Ok(lock) => {
                    for col in 0..8 {
                        for row in 0..8 {
                            let idx = row * 8 + col;
                            if let Some(piece) = lock[idx] {
                                match self.c.get(&piece.to_file_name()) {
                                    None => error!("Cacher doesn't contain: {}", piece.to_file_name()),
                                    Some(tex) => {
                                        let x = col as f64 * (TILE_S + 2.0);
                                        let y = row as f64 * (TILE_S + 2.0);
                                        let trans = t.trans(x, y); //TODO: account for scaling lol
                                        image(tex, trans, graphics);
                                    }
                                }
                            }
                        }
                    }
                },
                Err(e) => {
                    error!("Unable to read vec: {e}");
                }
            }
        }
    }

    pub async fn input(&mut self, mouse_pos: (f64, f64), size: Size) {
        let to_board_coord = |p: f64| {
            let mult = size.height / BOARD_S;
            let tile_size = (TILE_S + 2.0) * mult;
            (p / tile_size).floor() as u32
        };

        match self.last_pressed {
            None => {
                let lp_x = to_board_coord(mouse_pos.0);
                let lp_y = to_board_coord(mouse_pos.1);
                self.last_pressed = Some((lp_x, lp_y));
            }
            Some(lp) => {
                //Deal with second press
                let current_press = {
                    let lp_x = to_board_coord(mouse_pos.0);
                    let lp_y = to_board_coord(mouse_pos.1);
                    (lp_x, lp_y)
                };

                info!("Dealing with a move from {lp:?} to {current_press:?}");

                let rsp = self.client
                    .post("http://109.74.205.63:12345/movepiece")
                    .json(&JSONMove::new(self.id, lp.0, lp.1, current_press.0, current_press.1))
                    .send()
                    .await;
                match rsp {
                    Ok(response) => {
                        info!("Update from server: {:?}", response.text().await)
                    }
                    Err(e) => {
                        error!("Error in input response {e}");
                    }
                }

                self.last_pressed = None;
            }
        }
    }

    ///Should be called ASAP after instantiating game, and after input
    pub async fn update_list(&mut self) -> Result<(), reqwest::Error> {
        match self.cached_pieces.write() {
            Ok(mut lock) => {
                *lock = self.client.get(format!("http://109.74.205.63:12345/games/{}", self.id))
                    .send()
                    .await?
                    .json::<JSONPieceList>()
                    .await?
                    .to_game_list();
            }
            Err(e) => {
                error!("Unable to populate due to {e}")
            }
        }

        Ok(())
    }

    pub fn clear_input(&mut self) {
        self.last_pressed = None;
    }
}
