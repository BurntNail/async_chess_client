use anyhow::Result;
use std::{
    sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
    time::{Duration, Instant},
};

pub trait RwLockExt<T> {
    fn write_timeout(&self, to: Duration) -> Result<RwLockWriteGuard<'_, T>>;
    fn read_timeout(&self, to: Duration) -> Result<RwLockReadGuard<'_, T>>;
}

impl<T> RwLockExt<T> for RwLock<T> {
    fn write_timeout(&self, to: Duration) -> Result<RwLockWriteGuard<T>> {
        let start = Instant::now();
        while start.elapsed() < to {
            if let Ok(l) = self.try_write() {
                return Ok(l);
            }
        }

        bail!("timeout");
    }

    fn read_timeout(&self, to: Duration) -> Result<RwLockReadGuard<T>> {
        let start = Instant::now();
        while start.elapsed() < to {
            if let Ok(l) = self.try_read() {
                return Ok(l);
            }
        }

        bail!("timeout");
    }
}
