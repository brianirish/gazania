use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("D-Bus is unavailable: {0}")]
    DbusUnavailable(String),
    #[error("D-Bus call failed: {0}")]
    Dbus(String),
    #[error("statvfs failed for {path}: {source}")]
    Statvfs {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("mountinfo parse error on line {line}: {reason}")]
    MountinfoParse { line: usize, reason: String },
}

pub type Result<T> = std::result::Result<T, Error>;
