use std::collections::HashMap;
use piston_window::PistonWindow;
use poll_promise::Promise;
use crate::cacher::Cacher;

pub enum RequestType {
	GetBoard,
	MovePiece,
	NewBoard,
}

pub struct ChessGame {
	id: u32,
	c: Cacher,
	requests: HashMap<RequestType, Promise<reqwest::Result<String>>>
}
impl ChessGame {
	pub fn new (win: &mut PistonWindow) -> Result<Self, find_folder::Error> {
		Ok(Self {
			id: 420, //TODO: opening menu, user chooses
			c: Cacher::new_and_populate(win)?,
			requests: Default::default()
		})
	}
}