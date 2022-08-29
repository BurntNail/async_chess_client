use std::{
    fmt::Display,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::Context;

use super::memcache::MemoryTimedCacher;
use crate::{prelude::ErrorExt, util::error_ext::ToAnyhowPoisonErr};

///Struct to time how long actions in a given scope last.
pub struct ScopedTimer {
    ///The message to print to the logs
    msg: String,
    ///When the action starts
    start_time: Instant,
}

impl ScopedTimer {
    ///Function to create a new `ScopedTimer` and start the timer
    pub fn new(msg: impl Display) -> Self {
        Self {
            msg: msg.to_string(),
            start_time: Instant::now(),
        }
    }
}

impl Drop for ScopedTimer {
    fn drop(&mut self) {
        info!(time_taken=?self.start_time.elapsed(), msg=%self.msg);
    }
}

///Same as [`ScopedTimer`], but updates a [`MemoryTimedCacher`] rather than adding to logs
pub struct ScopedToListTimer<'a, const N: usize>(&'a mut MemoryTimedCacher<Duration, N>, Instant);

impl<'a, const N: usize> ScopedToListTimer<'a, N> {
    ///Creates a new `ScopedToListTimer`, and starts the timer
    pub fn new(t: &'a mut MemoryTimedCacher<Duration, N>) -> Self {
        Self(t, Instant::now())
    }
}

impl<'a, const N: usize> Drop for ScopedToListTimer<'a, N> {
    fn drop(&mut self) {
        self.0.add(self.1.elapsed());
    }
}

///Thread-safe version of [`ScopedToListTimer`] that uses [`Arc`] and [`Mutex`] over `&mut`
pub struct ThreadSafeScopedToListTimer<const N: usize>(
    Arc<Mutex<MemoryTimedCacher<Duration, N>>>,
    Instant,
);

impl<const N: usize> ThreadSafeScopedToListTimer<N> {
    ///Creates a new `ThreadSafeScopedToListTimer`, and starts the timer
    #[must_use]
    pub fn new(t: Arc<Mutex<MemoryTimedCacher<Duration, N>>>) -> Self {
        Self(t, Instant::now())
    }
}

impl<const N: usize> Drop for ThreadSafeScopedToListTimer<N> {
    fn drop(&mut self) {
        let elapsed = self.1.elapsed();
        let mut lock = self
            .0
            .lock()
            .ae()
            .context("locking memtimercache for timer")
            .unwrap_log_error();
        lock.add(elapsed);
    }
}
