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
        if self.timer.do_check() || self.data[0].is_none() {
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

        let end_index = if self.index == N - 1 {
            N - 1
        } else if self.data[self.index + 1].is_some() {
            N - 1
        } else {
            self.index
        };

        self.data[0..end_index]
            .into_iter()
            .cloned()
            .map(Option::unwrap)
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
}

impl DoOnInterval {
    pub fn new(gap: Duration) -> Self {
        Self {
            last_did: Instant::now(),
            gap,
        }
    }

    pub fn do_check(&mut self) -> bool {
        if self.last_did.elapsed() > self.gap {
            self.last_did = Instant::now();

            true
        } else {
            false
        }
    }
}

pub struct ScopedTimer {
    msg: String,
    start_time: Instant,
}

impl ScopedTimer {
    pub fn new(msg: String) -> Self {
        Self {
            msg,
            start_time: Instant::now(),
        }
    }
}

impl Drop for ScopedTimer {
    fn drop(&mut self) {
        info!(time_taken=?self.start_time.elapsed(), msg=%self.msg);
    }
}
