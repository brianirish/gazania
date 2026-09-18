//! zinnia core: drives, volumes, and later scanning, health and benchmarks.
//! No GTK or GLib dependency lives here.

pub mod bench;
pub mod error;
pub mod format;
pub mod health;
pub mod scan;
pub mod types;
pub mod volumes;

pub use error::{Error, Result};
pub use types::{Drive, MountPoint, Transport, Usage, Volume};
