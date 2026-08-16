use crate::*;

pub type OsableResult<T> = Result<T, OsableError>;

#[derive(Debug, snafu::Snafu)]
pub enum OsableError {
    Utf8,
    CString,
    #[cfg(target_family = "unix")]
    Libc {
        source: io::Error,
    },
    NotFound,
}

impl OsableError {
    #[cfg(target_family = "unix")]
    #[inline]
    pub fn libc_last() -> Self {
        Self::Libc { source: io::Error::last_os_error() }
    }

    #[cfg(target_family = "unix")]
    #[inline]
    pub fn libc(raw: i32) -> Self {
        Self::Libc { source: io::Error::from_raw_os_error(raw) }
    }

    #[cfg(target_family = "unix")]
    #[inline]
    pub fn err_libc_last<T>() -> OsableResult<T> {
        Err(Self::libc_last())
    }

    #[cfg(target_family = "unix")]
    #[inline]
    pub fn err_libc<T>(raw: i32) -> OsableResult<T> {
        Err(Self::libc(raw))
    }
}
