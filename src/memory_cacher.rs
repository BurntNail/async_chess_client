use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct MemoryTimedCacher<T, const N: usize> {
    data: [Option<T>; N],
    index: usize,

    last_cached: Instant,
    time_between_caches: Duration,
}

impl<T: Copy, const N: usize> Default for MemoryTimedCacher<T, N> {
    fn default() -> Self {
        Self {
            data: [None; N],
            index: 0,
            last_cached: Instant::now(),
            time_between_caches: Duration::from_millis(100),
        }
    }
}

impl<T: Clone + std::fmt::Debug, const N: usize> MemoryTimedCacher<T, N> {
    pub fn add(&mut self, t: T) {
        if self.last_cached.elapsed() >= self.time_between_caches || self.data[0].is_none() {
            self.last_cached = Instant::now();

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
