use std::fmt::Debug;
use std::{
    ops::{AddAssign, Div},
    time::{Duration, Instant},
};

#[derive(Debug)]
pub struct MemoryTimedCacher<T, const N: usize> {
    data: [Option<T>; N],
    index: usize,

    timer: Option<DoOnInterval>,
}

impl<T: Copy, const N: usize> Default for MemoryTimedCacher<T, N> {
    fn default() -> Self {
        trace!(size=%N, mem_size=%std::mem::size_of::<[Option<T>; N]>(), "Making memcache struct");
        Self {
            data: [None; N],
            index: 0,
            timer: Some(DoOnInterval::new(Duration::from_millis(50))),
        }
    }
}

impl<T: Debug + Copy, const N: usize> MemoryTimedCacher<T, N> {
    pub fn new(t: Option<DoOnInterval>) -> Self {
        Self {
            data: [None; N],
            index: 0,
            timer: t,
        }
    }

    pub fn add(&mut self, t: T) {
        if self.data[0].is_none() {
            self.data[self.index] = Some(t);
            self.index = (self.index + 1) % N;
        } else if let Some(doi) = &mut self.timer {
            let doiu = doi.can_do();
            if doiu.is_some() {
                self.data[self.index] = Some(t);
                self.index = (self.index + 1) % N;
            }
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
            .copied()
            .map(|opt| opt.expect("LOGIC ERROR IN TBS"))
            .collect()
    }
}

macro_rules! average_impl {
    ($($t:ty => $name:ident),+) => {
        $(
            impl<T, const N: usize> MemoryTimedCacher<T, N>
            where
                T: Div<$t> + AddAssign + Default + Clone + Copy + Debug,
            {
                #[allow(dead_code)]
                pub fn $name(&self) -> T::Output {
                    let mut total = T::default();
                    let mut count = 0;
    
                    for el in self.get_all().into_iter() {
                        total += el;
                        count += 1;
                    }
    
                    total / count
                }
            }
        )+
    };
}
macro_rules! average_fp_impl {
    ($($t:ty => $name:ident),+) => {
        $(
            impl<T, const N: usize> MemoryTimedCacher<T, N>
            where
                T: Div<$t> + AddAssign + Default + Clone + Copy + Debug,
            {
                #[allow(dead_code)]
                pub fn $name(&self) -> T::Output {
                    let mut total = T::default();
                    let mut count = 0.0;
    
                    for el in self.get_all().into_iter() {
                        total += el;
                        count += 1.0;
                    }
    
                    total / count
                }
            }
        )+
    };
}

average_impl!(u8 => average_u8, u16 => average_u16, u32 => average_u32, u64 => average_u64, u128 => average_u128, i8 => average_i8, i16 => average_i16, i32 => average_i32, i64 => average_i64, i128 => average_i128);
average_fp_impl!(f32 => average_f32, f64 => average_f64);

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

pub struct ScopedToListTimer<'a, const N: usize>(&'a mut MemoryTimedCacher<Duration, N>, Instant);

impl<'a, const N: usize> ScopedToListTimer<'a, N> {
    pub fn new(t: &'a mut MemoryTimedCacher<Duration, N>) -> Self {
        Self(t, Instant::now())
    }
}

impl<'a, const N: usize> Drop for ScopedToListTimer<'a, N> {
    fn drop(&mut self) {
        self.0.add(self.1.elapsed());
    }
}

// pub trait Number {
//     fn zero() -> Self;
//     fn add_one(&self) -> Self;
//     fn addassign_one(&mut self);
// }

// macro_rules! number_impl {
//     ($($t:ty),+) => {
//         $(
//             impl Number for $t {
//                 fn zero () -> Self {
//                     0
//                 }

//                 fn add_one (&self) -> Self {
//                     self + 1
//                 }

//                 fn addassign_one (&mut self) {
//                     *self += 1
//                 }
//             }
//         )+
//     };
// }
// macro_rules! number_fp_impl {
//     ($($t:ty),+) => {
//         $(
//             impl Number for $t {
//                 fn zero () -> Self {
//                     0.0
//                 }

//                 fn add_one (&self) -> Self {
//                     self + 1.0
//                 }

//                 fn addassign_one (&mut self) {
//                     *self += 1.0
//                 }
//             }
//         )+
//     };
// }

// number_impl!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128);
// number_fp_impl!(f32, f64);
//TODO: get this to work for the average bit
