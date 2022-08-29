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
    pub fn l(a: impl Into<L>) -> Self {
        Self::Left(a.into())
    }

    ///Constructor for [`Either::Right`] which uses [`Into::into`]
    pub fn r(b: impl Into<R>) -> Self {
        Self::Right(b.into())
    }
}

impl<T> Either<T, T> {
    ///If `L` == `R` then this function will return an `L` - useful for when the [`Either`] side signifies something, but always returns the same type.
    #[allow(clippy::missing_const_for_fn)] //Cannot be const as destructors cannot be const - Github error 8874
    pub fn one_type(self) -> T {
        match self {
            Self::Left(l) => l,
            Self::Right(r) => r,
        }
    }
}

impl<L: Debug, R: Debug> Debug for Either<L, R> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Left(arg0) => f.debug_tuple("Left").field(arg0).finish(),
            Self::Right(arg0) => f.debug_tuple("Right").field(arg0).finish(),
        }
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
