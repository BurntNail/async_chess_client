use std::fmt::{Debug, Formatter};

///Enum which can represent one of two values
///
///The Same as an `(Option<A>, Option<B>)` where one [`Option`] must always be [`Option::Some`] and the other must be [`Option::None`]
pub enum Either<L, R> {
    ///The First variant of [`Either`]
    Left(L),
    ///The second variant of [`Either`]
    Right(R),
}

impl<L, R> Either<L, R> {
    ///Constructor for [`Either::Left`] which uses [`Into::into`]
    #[allow(dead_code)]
    pub fn l(a: impl Into<L>) -> Self {
        Self::Left(a.into())
    }

    ///Constructor for [`Either::Right`] which uses [`Into::into`]
    #[allow(dead_code)]
    pub fn r(b: impl Into<R>) -> Self {
        Self::Right(b.into())
    }
}

impl<L> Either<L, L> {
    ///If `L` == `R` then this function will return an `L` - useful for when the [`Either`] side signifies something, but always returns the same type.
    #[allow(dead_code)]
    #[allow(clippy::missing_const_for_fn)] //Cannot be const as destructors cannot be const - Github error 8874
    pub fn to_normal(self) -> L {
        match self {
            Self::Left(l) => l,
            Self::Right(r) => r,
        }
    }
}

impl<L: Debug, R: Debug> Debug for Either<L, R> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("Either");
        match self {
            Self::Left(l) => debug.field("Left", l),
            Self::Right(r) => debug.field("Right", r),
        }
        .finish()
    }
}

impl<L: Clone, R: Clone> Clone for Either<L, R> {
    fn clone(&self) -> Self {
        match self {
            Self::Left(l) => Self::Left(l.clone()),
            Self::Right(r) => Self::Right(r.clone()),
        }
    }
}
