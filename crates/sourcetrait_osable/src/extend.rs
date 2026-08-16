use crate::*;

pub(crate) trait OsableIntoUtf8 {
    fn into_utf8(self) -> OsableResult<String>;
}

impl OsableIntoUtf8 for CString {
    fn into_utf8(self) -> OsableResult<String> {
        self.into_string().map_err(|_| OsableError::Utf8)
    }
} 

impl OsableIntoUtf8 for &CStr {
    fn into_utf8(self) -> OsableResult<String> {
        self.to_str()
            .map(|v| v.to_owned())
            .map_err(|_| OsableError::Utf8)
    }
} 

impl OsableIntoUtf8 for PathBuf {
    fn into_utf8(self) -> OsableResult<String> {
        self.into_string().map_err(|_| OsableError::Utf8)
    }
} 

pub(crate) trait OsableFromUtf8: Sized {
    fn from_utf8<S: Into<String>>(s: S) -> OsableResult<Self>;
}

impl OsableFromUtf8 for CString {
    fn from_utf8<S: Into<String>>(s: S) -> OsableResult<Self> {
        Self::new(s.into()).map_err(|_| OsableError::CString)
    }
}
