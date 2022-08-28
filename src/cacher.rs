use crate::{
    error_ext::{ErrorExt, ToAnyhowNotErr},
    time_based_structs::ScopedTimer,
};
use anyhow::{Context, Result};
use find_folder::Search::ParentsThenKids;
use piston_window::{
    Filter, Flip, G2dTexture, G2dTextureContext, PistonWindow, Texture, TextureSettings,
};
use std::{collections::HashMap, path::PathBuf};

///The size in pixels of the length/width of a chess piece sprite
pub const TILE_S: f64 = 20.0;
///The size in pixels of the length/width of the chess board sprite
pub const BOARD_S: f64 = 256.0;

///Struct to hold a cache of [`G2dTexture`]s
pub struct Cacher {
    ///Base path for the assets
    base_path: PathBuf,
    ///HashMap of paths to textures
    assets: HashMap<String, G2dTexture>,
    ///Context for textures from window
    tc: G2dTextureContext,
}

impl Cacher {
    ///Function to create a new empty cache.
    ///
    /// # Errors
    /// Can fail if it can't find the assets folder
    pub fn new(win: &mut PistonWindow) -> Result<Self> {
        let path = ParentsThenKids(2, 2)
            .for_folder("assets")
            .context("Finding the assets folder")?;
        Ok(Self {
            base_path: path,
            assets: HashMap::new(),
            tc: win.create_texture_context(),
        })
    }

    ///Gets a [`G2dTexture`] from the cache. Returns [`None`] if there is no asset with that path.
    ///
    /// # Errors
    /// - Unable to find the texture using [`Texture::from_path`]
    pub fn get(&mut self, p: &str) -> Result<&G2dTexture> {
        self.insert(p).map(|_| {
            self.assets
                .get(p)
                .ae()
                .context("getting asset that exists")
                .unwrap_log_error()
        })
    }

    ///Inserts a new asset into the cache from the path given - should just be like `'icon.png'`, as all files should be in the `'assets/'` folder
    ///
    /// # Errors
    /// - Unable to find the texture using [`Texture::from_path`]
    fn insert(&mut self, p: &str) -> Result<()> {
        if self.assets.contains_key(p) {
            return Ok(());
        }

        info!("Inserting {p}");
        let _st = ScopedTimer::new(format!("Geting {p}"));

        let path = self.base_path.join(p);
        let ts = TextureSettings::new().filter(Filter::Nearest);

        match Texture::from_path(&mut self.tc, path, Flip::None, &ts) {
            Ok(tex) => {
                self.assets.insert(p.to_string(), tex);
                Ok(())
            }
            Err(e) => Err(anyhow!("Unable to find texture: {e}")),
        }
    }
}
