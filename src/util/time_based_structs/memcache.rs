use super::do_on_interval::UpdateOnCheck;
use crate::prelude::DoOnInterval;
use std::{
    fmt::Debug,
    mem::MaybeUninit,
    ops::{AddAssign, Div},
    time::Duration,
};

///Struct to hold a list of items that only get updated on a [`DoOnInterval`], with a circular cache that overwrites the oldest items if there isn't any free space.
#[derive(Debug)]
pub struct MemoryTimedCacher<T, const N: usize> {
    ///Holds all the data
    data: [MaybeUninit<T>; N],
    ///Marks whether or not data has been written ever
    data_ever_written: bool,
    ///Marks whether or not the array is full of data - useful for after it wraps around
    full: bool,
    ///Holds the index of the last data written in
    index: usize,

    ///Holds a timer in case we only want to write data on intervals rather than whenever `add` is called
    timer: Option<DoOnInterval<UpdateOnCheck>>,
}

impl<T: Copy, const N: usize> Default for MemoryTimedCacher<T, N> {
    fn default() -> Self {
        trace!(size=%N, mem_size=%std::mem::size_of::<[Option<T>; N]>(), "Making memcache struct");
        Self {
            data: [MaybeUninit::uninit(); N],
            data_ever_written: false,
            full: false,
            index: 0,
            timer: Some(DoOnInterval::new(Duration::from_millis(50))),
        }
    }
}

impl<T: Debug + Copy, const N: usize> MemoryTimedCacher<T, N> {
    ///Creates a blank Memory Cacher
    #[must_use]
    pub fn new(t: Option<DoOnInterval<UpdateOnCheck>>) -> Self {
        Self {
            timer: t,
            ..Default::default()
        }
    }

    ///Adds an element to the list on the following conditions:
    /// - there are no elements
    /// - there is a [`DoOnInterval`] timer, and we can use it
    ///
    /// # Safety
    /// We check that there is data at the index before we drop the data at the old index
    pub fn add(&mut self, t: T) {
        let can = !self.data_ever_written
            || if let Some(t) = &mut self.timer {
                t.can_do()
            } else {
                true
            };

        if can {
            if self.data_ever_written {
                unsafe { self.data[self.index].assume_init_drop() };
            } else {
                self.data_ever_written = true;
            }

            self.data[self.index].write(t);
            self.index = (self.index + 1) % N;

            if self.index == N - 1 {
                self.full = true;
            }

            if let Some(t) = &mut self.timer {
                t.update_timer();
            }
        }
    }

    ///Gets all of the elements, with order unimportant
    ///
    /// # Safety
    /// We double check there is data beforehand using the `index` variable and the `full` variable
    pub fn get_all(&self) -> Vec<T> {
        if !self.data_ever_written {
            //no elements yet
            return vec![];
        }

        let end_index = if self.full { N - 1 } else { self.index };

        self.data[0..end_index]
            .iter()
            .copied()
            .map(|opt| unsafe { opt.assume_init_read() })
            .collect()
    }

    ///Returns whether or not the list is empty
    pub fn is_empty(&self) -> bool {
        !self.data_ever_written
    }
}

///Creates an average function for an {integer} type
macro_rules! average_impl {
    ($($t:ty => $name:ident),+) => {
        $(
            impl<T, const N: usize> MemoryTimedCacher<T, N>
            where
                T: Div<$t> + AddAssign + Default + Clone + Copy + Debug,
                T::Output: Default,
            {
                ///Function to get the average of the items in the list
                pub fn $name(&self) -> T::Output {
                    if self.is_empty() {
                        return T::Output::default();
                    }

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
///Creates an average function for a {float} type
macro_rules! average_fp_impl {
    ($($t:ty => $name:ident),+) => {
        $(
            impl<T, const N: usize> MemoryTimedCacher<T, N>
            where
                T: Div<$t> + AddAssign + Default + Clone + Copy + Debug + Default,
                T::Output: Default
            {
                ///Function to get the average of the items in the list
                pub fn $name(&self) -> T::Output {
                    if self.is_empty() {
                        return T::Output::default();
                    }

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
