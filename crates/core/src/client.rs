//! One system-bus connection reused for every udisks2 query.

use crate::error::Result;
use crate::health::{health_from, Health};
use crate::types::Drive;
use crate::volumes::raw::Snapshot;
use crate::volumes::watch::watch_owned;
use crate::volumes::{assemble, mountinfo, udisks, usage, Change};
use futures_lite::Stream;
use std::path::Path;
use std::pin::Pin;
use std::time::Duration;

/// udisks2 change events, boxed so they can live next to the `Client`.
pub type ChangeStream = Pin<Box<dyn Stream<Item = Change>>>;

const BACKOFF_START: Duration = Duration::from_secs(1);
const BACKOFF_CAP: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct Client {
    conn: zbus::Connection,
}

impl Client {
    pub async fn connect() -> Result<Self> {
        Ok(Self {
            conn: udisks::connect().await?,
        })
    }

    pub async fn snapshot(&self) -> Result<Snapshot> {
        udisks::snapshot(&self.conn).await
    }

    /// The full drive tree with mount options and statvfs usage.
    pub async fn drives(&self) -> Result<Vec<Drive>> {
        let mounts = mountinfo::read_system()?;
        let snapshot = self.snapshot().await?;
        let mut stats = |p: &Path| usage::stats_for(p).ok();
        Ok(assemble::assemble(&snapshot, &mounts, &mut stats))
    }

    /// Health for every drive. Assembles without mounts or statvfs, because
    /// only the drive-to-device mapping is needed.
    pub async fn health(&self) -> Result<Vec<Health>> {
        let snapshot = self.snapshot().await?;
        let drives =
            assemble::assemble(&snapshot, &[], &mut |_: &Path| -> Option<usage::FsStats> {
                None
            });
        Ok(health_from(&snapshot, &drives))
    }

    pub async fn changes(&self) -> Result<ChangeStream> {
        Ok(Box::pin(watch_owned(self.conn.clone()).await?))
    }
}

/// Delay before reconnect attempt `attempt` (0-based): 1 s, 2 s, 4 s and so
/// on, capped at 30 s.
pub fn backoff_delay(attempt: u32) -> Duration {
    let factor = 1u64 << attempt.min(16);
    Duration::from_secs(BACKOFF_START.as_secs().saturating_mul(factor)).min(BACKOFF_CAP)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_starts_at_one_second_and_doubles() {
        assert_eq!(backoff_delay(0), Duration::from_secs(1));
        assert_eq!(backoff_delay(1), Duration::from_secs(2));
        assert_eq!(backoff_delay(4), Duration::from_secs(16));
    }

    #[test]
    fn backoff_is_capped_at_thirty_seconds_without_overflowing() {
        assert_eq!(backoff_delay(5), Duration::from_secs(30));
        assert_eq!(backoff_delay(40), Duration::from_secs(30));
        assert_eq!(backoff_delay(u32::MAX), Duration::from_secs(30));
    }
}
