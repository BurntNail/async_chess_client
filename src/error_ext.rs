use anyhow::Result;
use std::{any::Any, fmt::Display, sync::LockResult};

///Extension trait for errors to quickly do things
pub trait ErrorExt<T> {
    ///If `Err` write to [`warn!`]
    fn warn(self);
    ///If `Err` write to [`error!`]
    fn error(self);
    ///If `Err` write to [`error!`] and [`std::process::exit`] with code 1
    fn error_exit(self);
    ///If `Err` write to [`eprintln!`] and [`std::process::exit`] with code 1
    fn eprint_exit(self);
    ///If `Err` write to [`error!`] and [`std::process::exit`] with code 1, else return `Ok` value
    fn unwrap_log_error(self) -> T;
}

macro_rules! to_anyhow_trait {
    ($($name:ident => $doc:expr),+) => {
        $(
            #[doc=$doc]
            pub trait $name<T> {
                ///Converter function to [`anyhow::Result`]
                fn ae (self) -> anyhow::Result<T>;
            }
        )+
    };
}
to_anyhow_trait!(
    ToAnyhowErr => "Trait to turn [`std::error::Error`] to [`anyhow::Error`]",
    ToAnyhowNotErr => "Trait to turn non-errors (like [`Option`]) to [`anyhow::Error`]",
    ToAnyhowPoisonErr => "Trait to turn `Box<dyn Any + Send + 'static>` to [`anyhow::Error`]",
    ToAnyhowThreadErr => "Trait to turn [`std::sync::LockResult`] to [`anyhow::Result`]"
);
//To avoid overlapping trait bounds

impl<T, E: Display> ErrorExt<T> for Result<T, E> {
    fn warn(self) {
        if let Err(e) = self {
            warn!(%e);
        }
    }

    fn error(self) {
        if let Err(e) = self {
            error!(%e);
        }
    }

    fn error_exit(self) {
        if let Err(e) = self {
            error!(%e, "Fatal Error");
            std::process::exit(1);
        }
    }

    fn eprint_exit(self) {
        if let Err(e) = self {
            eprintln!("Fatal Error: {e}");
            std::process::exit(1);
        }
    }

    fn unwrap_log_error(self) -> T {
        match self {
            Ok(o) => o,
            Err(e) => {
                error!(%e, "Fatal Error on unwrap");
                std::process::exit(1);
            }
        }
    }
}

impl<T> ToAnyhowNotErr<T> for Option<T> {
    fn ae(self) -> Result<T> {
        match self {
            Some(s) => Ok(s),
            None => Err(anyhow!("empty option")),
        }
    }
}

impl<T, E: std::error::Error + Send + Sync + 'static> ToAnyhowErr<T> for std::result::Result<T, E> {
    fn ae(self) -> Result<T> {
        self.map_err(|e| anyhow::Error::new(e))
    }
}
impl<T> ToAnyhowThreadErr<T> for std::result::Result<T, Box<dyn Any + Send + 'static>> {
    fn ae(self) -> Result<T> {
        self.map_err(|_| anyhow!("Error joining thread"))
    }
}
impl<T> ToAnyhowPoisonErr<T> for LockResult<T> {
    fn ae(self) -> Result<T> {
        self.map_err(|e| anyhow!("{}", e))
    }
}
