use std::{time::{Instant, Duration}, marker::PhantomData};

use crate::{generic_enum, prelude::Either};
use crate::crate_private::Sealed;

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