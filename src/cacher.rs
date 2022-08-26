use crate::{chess::ChessPiece, time_based_structs::ScopedTimer};
use anyhow::{Context, Result};
use find_folder::Search::ParentsThenKids;
use piston_window::{Filter, Flip, G2dTexture, PistonWindow, Texture, TextureSettings};
use std::{collections::HashMap, path::PathBuf};

//TODO: Check if we can create a texture context at constructor-time, rather than always needing the [`PistonWindow`] mut ref

///The size in pixels of the length/width of a chess piece sprite
pub const TILE_S: f64 = 20.0;
///The size in pixels of the length/width of the chess board sprite
pub const BOARD_S: f64 = 256.0;

#[derive(Debug)]
///Struct to hold a cache of [`G2dTexture`]s
pub struct Cacher {
    base_path: PathBuf,
    assets: HashMap<String, G2dTexture>,
}

impl Cacher {
    ///Function to create a new empty cache.
    ///
    /// # Errors
    /// Can fail if it can't find the assets folder
    pub fn new() -> Result<Self> {
        let path = ParentsThenKids(2, 2)
            .for_folder("assets")
            .context("Finding the assets folder")?;
        Ok(Self {
            base_path: path,
            assets: HashMap::new(),
        })
    }

    ///Creates a new blank cache, and populates it. Needs a mutable reference to the [`PistonWindow`]
    ///
    /// # Errors
    /// - Can't find the assets folder
    /// - Can't populate the cache
    pub fn new_and_populate(win: &mut PistonWindow) -> Result<Self> {
        let mut s = Self::new()?;
        s.populate(win)?;
        Ok(s)
    }

    ///Gets a [`G2dTexture`] from the cache. Returns [`None`] if there is no asset with that path.
    pub fn get(&self, p: &str) -> Option<&G2dTexture> {
        self.assets.get(p)
    }

    ///Inserts a new asset into the cache from the path given - should just be like `'icon.png'`, as all files should be in the `'assets/'` folder
    ///
    /// # Errors
    /// - Unable to find the texture using [`Texture::from_path`]
    fn insert(&mut self, p: &str, win: &mut PistonWindow) -> Result<()> {
        let path = self.base_path.join(p);
        let ts = TextureSettings::new().filter(Filter::Linear);

        match Texture::from_path(&mut win.create_texture_context(), path, Flip::None, &ts) {
            Ok(tex) => {
                self.assets.insert(p.to_string(), tex);
                Ok(())
            }
            Err(e) => Err(anyhow!("Unable to find texture: {e}")),
        }
    }

    ///Populates the entire cache
    ///
    /// # Errors
    /// - Unable to find the correct files in the `'assets/'` directory
    #[tracing::instrument(skip(self, win), fields(s_len=self.assets.len(), path=?self.base_path))]
    pub fn populate(&mut self, win: &mut PistonWindow) -> Result<()> {
        let _st = ScopedTimer::new("Populating");

        for variant in ChessPiece::all_variants() {
            self.insert(&variant.to_file_name(), win)
                .with_context(|| format!("Unable to find variant {variant:?}"))?;
        }

        for extra in &["board_alt.png", "highlight.png", "selected.png"] {
            self.insert(extra, win)
                .with_context(|| format!("Unable to find extra {extra}"))?;
        }

        Ok(())
    }
}
