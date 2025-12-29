use std::fmt;
use std::io;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Index(String),
    Search(String),
    NotIndexed(String),
    Cancelled,
    Dbus(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "IO error: {}", e),
            Error::Index(s) => write!(f, "Index error: {}", s),
            Error::Search(s) => write!(f, "Search error: {}", s),
            Error::NotIndexed(s) => write!(f, "Not indexed: {}", s),
            Error::Cancelled => write!(f, "Operation cancelled"),
            Error::Dbus(s) => write!(f, "D-Bus error: {}", s),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Index(e.to_string())
    }
}

impl From<zbus::Error> for Error {
    fn from(e: zbus::Error) -> Self {
        Error::Dbus(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
