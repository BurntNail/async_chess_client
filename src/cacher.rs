use find_folder::Search::ParentsThenKids;
use piston_window::{Filter, Flip, G2dTexture, PistonWindow, Texture, TextureSettings};
use std::collections::HashMap;
use std::path::PathBuf;
use crate::chess::ChessPiece;

pub struct Cacher {
	path: PathBuf,
	assets: HashMap<String, G2dTexture>,
}

impl Cacher {
	pub fn new() -> Result<Self, find_folder::Error> {
		let path = ParentsThenKids(3, 3)
			.for_folder("assets")?;
		Ok(Cacher {
			path,
			assets: HashMap::new(),
		})
	}
	pub fn new_and_populate (win: &mut PistonWindow) -> Result<Self, find_folder::Error> {
		let mut s = Self::new()?;
		s.populate(win);
		Ok(s)
	}

	pub fn get(&self, p: &str) -> Option<&G2dTexture> {
		self.assets.get(p)
	}

	fn insert(&mut self, p: &str, win: &mut PistonWindow) {
		let path = self.path.join(p);
		let ts = TextureSettings::new().filter(Filter::Linear);

		match Texture::from_path(&mut win.create_texture_context(), p, Flip::None, &ts) {
			Ok(tex) =>  {
				self.assets.insert(p.to_string(), tex);
			},
			Err(e) => {
				error!("Unable to find texture: {e}");
			}
		}
	}

	pub fn populate(&mut self, win: &mut PistonWindow) {
		for variant in ChessPiece::all_variants() {
			self.insert(&variant.to_file_name(), win);
		}
		self.insert("board_alt.png", win);
	}
}
