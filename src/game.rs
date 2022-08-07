use crate::{
    cacher::{Cacher, BOARD_S, TILE_S},
    chess::ChessPiece,
    from_server::JSONPieceList,
};
use piston_window::{clear, image, Context, G2d, GfxDevice, PistonWindow, Size};
use std::sync::RwLock;

pub struct ChessGame {
    id: u32,
    c: Cacher,
    // requests: HashMap<RequestType, Promise<reqwest::Result<String>>>,
    cached_pieces: RwLock<Vec<Option<ChessPiece>>>,
    last_pressed: Option<(u32, u32)>,
}
impl ChessGame {
    pub fn new(win: &mut PistonWindow) -> Result<Self, find_folder::Error> {
        Ok(Self {
            id: 420, //TODO: opening menu, user chooses
            c: Cacher::new_and_populate(win)?,
            // requests: Default::default(),
            cached_pieces: RwLock::new(vec![None; 64]),
            last_pressed: None,
        })
    }

    pub fn render(
        &mut self,
        size: Size,
        ctx: Context,
        graphics: &mut G2d,
        _device: &mut GfxDevice,
    ) {
        clear([0.0; 4], graphics);
        {
            let t = ctx.transform;
            image(self.c.get("board_alt.png").unwrap(), t, graphics);
        }
    }

    pub async fn input(&mut self, mouse_pos: (f64, f64), size: Size) {
        match self.last_pressed {
            None => {
                let lp_x = (mouse_pos.0 / (TILE_S * (size.height / BOARD_S))) as u32;
                let lp_y = (mouse_pos.1 / (TILE_S * (size.height / BOARD_S))) as u32;
                self.last_pressed = Some((lp_x, lp_y));
            }
            Some(lp) => {
                //Deal with second press
                let current_press = {
                    let lp_x = (mouse_pos.0 / (TILE_S * (size.height / BOARD_S))) as u32;
                    let lp_y = (mouse_pos.1 / (TILE_S * (size.height / BOARD_S))) as u32;
                    (lp_x, lp_y)
                };

                info!("Dealing with a move from {lp:?} to {current_press:?}");

                self.last_pressed = None;
            }
        }
    }

    ///Should be called ASAP after instantiating game
    pub async fn populate(&mut self) -> Result<(), reqwest::Error> {
        match self.cached_pieces.write() {
            Ok(mut lock) => {
                *lock = reqwest::get(format!("http://109.74.205.63:12345/games/{}", self.id))
                    .await?
                    .json::<JSONPieceList>()
                    .await?
                    .to_game_list()
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
