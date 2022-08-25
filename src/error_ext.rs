use anyhow::Result;
use std::fmt::{Debug, Display};

pub trait ErrorExt<T> {
    fn warn(self);
    fn error(self);
    fn error_exit(self);
    fn eprint_exit(self);
    fn unwrap_log_error(self) -> T;
}

pub trait ToAnyhowDisplay<T> {
    fn to_ae_display(self) -> Result<T>;
}
pub trait ToAnyhowDebug<T> {
    fn to_ae_debug(self) -> Result<T>;
}
pub trait ToAnyhow<T> {
    fn to_ae_debug(self) -> Result<T>;
    fn to_ae_display(self) -> Result<T>;
}

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

impl<T, E: Display> ToAnyhowDisplay<T> for Result<T, E> {
    fn to_ae_display(self) -> Result<T> {
        self.map_err(|e| anyhow!("{e}"))
    }
}
impl<T, E: Debug> ToAnyhowDebug<T> for Result<T, E> {
    fn to_ae_debug(self) -> Result<T> {
        self.map_err(|e| anyhow!("{e:?}"))
    }
}
impl<T, E: Display + Debug> ToAnyhow<T> for Result<T, E> {
    fn to_ae_debug(self) -> Result<T> {
        self.map_err(|e| anyhow!("{e}"))
    }

    fn to_ae_display(self) -> Result<T> {
        self.map_err(|e| anyhow!("{e:?}"))
    }
}
impl<T> ToAnyhow<T> for Option<T> {
    fn to_ae_debug(self) -> Result<T> {
        match self {
            Some(s) => Ok(s),
            None => Err(anyhow!("empty option")),
        }
    }

    fn to_ae_display(self) -> Result<T> {
        self.to_ae_debug()
    }
}
