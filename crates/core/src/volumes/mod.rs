//! Drives and volumes: enumeration, mount info, usage.

pub mod assemble;
pub mod fallback;
pub mod mountinfo;
pub mod raw;
pub mod udisks;
pub mod usage;

use crate::error::{Error, Result};
use crate::types::Drive;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Udisks2,
    MountinfoFallback,
}

#[derive(Debug, Clone)]
pub struct VolumesReport {
    pub source: Source,
    /// Why udisks2 could not be used, in fallback mode.
    pub fallback_reason: Option<String>,
    pub drives: Vec<Drive>,
}

/// Enumerate drives and volumes. Uses udisks2 when reachable, else mountinfo.
pub async fn list_volumes() -> Result<VolumesReport> {
    let mounts = mountinfo::read_system()?;
    let mut stats = |p: &Path| usage::stats_for(p).ok();

    let via_udisks: Result<Vec<Drive>> = async {
        let conn = udisks::connect().await?;
        let snap = udisks::snapshot(&conn).await?;
        Ok(assemble::assemble(&snap, &mounts, &mut stats))
    }
    .await;

    match via_udisks {
        Ok(drives) => Ok(VolumesReport { source: Source::Udisks2, fallback_reason: None, drives }),
        Err(e @ (Error::DbusUnavailable(_) | Error::Dbus(_))) => Ok(VolumesReport {
            source: Source::MountinfoFallback,
            fallback_reason: Some(e.to_string()),
            drives: fallback::assemble_fallback(&mounts, &mut stats),
        }),
        Err(e) => Err(e),
    }
}
