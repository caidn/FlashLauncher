use serde::{Serialize, Serializer};

/// Every command returns this on failure. It reaches the UI as a plain
/// message string, which is all the UI does with it.
#[derive(Debug)]
pub struct AppError(pub String);

pub type AppResult<T> = Result<T, AppError>;

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for AppError {}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

macro_rules! from_err {
    ($($t:ty),*) => {$(
        impl From<$t> for AppError {
            fn from(e: $t) -> Self {
                AppError(e.to_string())
            }
        }
    )*};
}

from_err!(
    rusqlite::Error,
    std::io::Error,
    crate::swf::SwfError,
    tauri::Error,
    serde_json::Error
);

impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        AppError(s.to_string())
    }
}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError(s)
    }
}
