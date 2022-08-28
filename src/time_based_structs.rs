use crate::{
    crate_private::Sealed,
    either::Either,
    error_ext::{ErrorExt, ToAnyhowPoisonErr},
};
use anyhow::Context;
use std::{
    fmt::{Debug, Display},
    marker::PhantomData,
    mem::MaybeUninit,
    ops::{AddAssign, Div},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
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
                false
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

///Provides any number of unit structs that implement a unit type
macro_rules! generic_enum {
    (($trait_name:ident -> $trait_docs:literal) => $(($unit_struct_name:ident -> $docs:literal)),+) => {
        pub trait $trait_name : Sealed {}

        $(
            #[doc=$trait_docs]
            #[derive(Copy, Clone, Debug)]
            pub struct $unit_struct_name;
            impl Sealed for $unit_struct_name {}
            impl $trait_name for $unit_struct_name {}
        )+
    };
}

generic_enum!((DoOnIntervalMode -> "Trait for how `DoOnInterval` should update the timer") => (GiveUpdaters -> "Give updaters that update the timer when they are dropped"), (UpdateOnCheck -> "Update the timer when if we can do the action when we check"));

///Timer struct to only allow actions to be performed on an interval
#[derive(Debug)]
pub struct DoOnInterval<MODE: DoOnIntervalMode> {
    ///When the action was last done
    last_did: Instant,
    ///Gap between doing actions
    gap: Duration,
    ///Whether or not an instance of [`DOIUpdate`] exists pointing to this right now. Only used in [`GiveUpdaters`]
    updater_exists: bool,

    ///`PhantomData` to make sure mode isn't optimised away
    _pd: PhantomData<MODE>,
}

impl<MODE: DoOnIntervalMode> DoOnInterval<MODE> {
    ///Creates a new `DoOnInterval` using the duration given
    #[must_use]
    pub fn new(gap: Duration) -> Self {
        Self {
            last_did: Instant::now() - gap * 2,
            gap,
            updater_exists: false,
            _pd: PhantomData,
        }
    }
}

impl DoOnInterval<GiveUpdaters> {
    ///Checks whether or not we can do the action, using the timer and checking whether any instances of [`DOIUpdate`] currently exist
    ///
    /// Returns `None` is we can't, and `Some` if we can. Make sure to bind the [`DOIUpdate`] to allow the [`Drop::drop`] impl to run correctly.
    pub fn get_updater(&mut self) -> Option<DOIUpdate> {
        if !self.updater_exists && self.last_did.elapsed() > self.gap {
            self.updater_exists = true;
            Some(DOIUpdate(self))
        } else {
            None
        }
    }

    ///Turns a [`GiveUpdaters`] to an [`UpdateOnCheck`]. Can return the original [`GiveUpdaters`] if an updater currently exists
    #[must_use]
    pub fn to_update_on_check(
        self,
    ) -> Either<DoOnInterval<GiveUpdaters>, DoOnInterval<UpdateOnCheck>> {
        if self.updater_exists {
            Either::Left(self)
        } else {
            let nu = DoOnInterval {
                last_did: self.last_did,
                gap: self.gap,
                updater_exists: false,
                _pd: PhantomData,
            };
            Either::Right(nu)
        }
    }
}
impl DoOnInterval<UpdateOnCheck> {
    ///Checks whether or not enough time has elapsed. If so, updates the timer and returns true, else returns false.
    ///
    ///If the action takes a while, it is reccomended to call `update_timer`
    pub fn can_do(&mut self) -> bool {
        if self.last_did.elapsed() > self.gap {
            self.last_did = Instant::now();
            true
        } else {
            false
        }
    }

    ///Updates the timer.
    pub fn update_timer(&mut self) {
        self.last_did = Instant::now();
    }

    ///Turns a [`UpdateOnCheck`] to a [`GiveUpdaters`]
    #[must_use]
    pub fn to_give_updaters(self) -> DoOnInterval<GiveUpdaters> {
        DoOnInterval {
            last_did: self.last_did,
            gap: self.gap,
            updater_exists: false,
            _pd: PhantomData,
        }
    }
}

///Struct to update [`DoOnInterval`] when the action finishes.
pub struct DOIUpdate<'a>(&'a mut DoOnInterval<GiveUpdaters>);
impl Drop for DOIUpdate<'_> {
    fn drop(&mut self) {
        self.0.last_did = Instant::now();
        self.0.updater_exists = false;
    }
}

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
