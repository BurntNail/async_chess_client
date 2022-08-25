use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct MemoryTimedCacher<T, const N: usize> {
    data: [Option<T>; N],
    index: usize,

    timer: DoOnInterval,
}

impl<T: Copy, const N: usize> Default for MemoryTimedCacher<T, N> {
    fn default() -> Self {
        trace!(size=%N, mem_size=%std::mem::size_of::<[Option<T>; N]>(), "Making memcache struct");
        Self {
            data: [None; N],
            index: 0,
            timer: DoOnInterval::new(Duration::from_millis(50)),
        }
    }
}

impl<T: Clone + std::fmt::Debug, const N: usize> MemoryTimedCacher<T, N> {
    pub fn add(&mut self, t: T) {
        let doiu = self.timer.can_do();
        if doiu.is_some() || self.data[0].is_none() {
            self.data[self.index] = Some(t);
            self.index = (self.index + 1) % N;
        }
    }

    ///Order not preserved
    pub fn get_all(&self) -> Vec<T> {
        if self.data[0].is_none() {
            //no elements yet
            return vec![];
        }

        let end_index =
            if self.index == N - 1 || matches!(self.data.get(self.index + 1), Some(Some(_))) {
                N - 1
            } else {
                self.index
            };

        self.data[0..end_index]
            .iter()
            .cloned()
            .map(|opt| opt.expect("LOGIC ERROR IN TBS"))
            .collect()
    }
}

impl<T: Into<f64> + Clone + std::fmt::Debug, const N: usize> MemoryTimedCacher<T, N> {
    pub fn average(&self) -> f64 {
        let mut total = 0.0;
        let mut count = 0.0;

        for el in self.get_all().into_iter().map(Into::into) {
            total += el;
            count += 1.0;
        }

        total / count
    }
}

#[derive(Debug)]
pub struct DoOnInterval {
    last_did: Instant,
    gap: Duration,
    updater_exists: bool,
}

impl DoOnInterval {
    pub fn new(gap: Duration) -> Self {
        Self {
            last_did: Instant::now() - Duration::from_secs(1),
            gap,
            updater_exists: false,
        }
    }

    pub fn can_do(&mut self) -> Option<DOIUpdate> {
        if !self.updater_exists && self.last_did.elapsed() > self.gap {
            self.updater_exists = true;
            Some(DOIUpdate(self))
        } else {
            None
        }
    }
}

pub struct DOIUpdate<'a>(&'a mut DoOnInterval);
impl Drop for DOIUpdate<'_> {
    fn drop(&mut self) {
        self.0.last_did = Instant::now();
        self.0.updater_exists = false;
    }
}

pub struct ScopedTimer {
    msg: String,
    start_time: Instant,
}

impl ScopedTimer {
    pub fn new(msg: impl Into<String>) -> Self {
        Self {
            msg: msg.into(),
            start_time: Instant::now(),
        }
    }
}

impl Drop for ScopedTimer {
    fn drop(&mut self) {
        info!(time_taken=?self.start_time.elapsed(), msg=%self.msg);
    }
}
