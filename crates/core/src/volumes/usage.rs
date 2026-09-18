//! Size, used and available bytes for a mounted filesystem, matching `df`.

use crate::error::{Error, Result};
use crate::types::Usage;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FsStats {
    pub size: u64,
    pub usage: Usage,
}

pub fn stats_for(path: &Path) -> Result<FsStats> {
    let vfs = rustix::fs::statvfs(path).map_err(|errno| Error::Statvfs {
        path: PathBuf::from(path),
        source: std::io::Error::from(errno),
    })?;
    let frsize = vfs.f_frsize;
    let size = vfs.f_blocks.saturating_mul(frsize);
    let used = vfs.f_blocks.saturating_sub(vfs.f_bfree).saturating_mul(frsize);
    let available = vfs.f_bavail.saturating_mul(frsize);
    Ok(FsStats {
        size,
        usage: Usage { used, available },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tempdir_reports_consistent_numbers() {
        let dir = tempfile::tempdir().unwrap();
        let stats = stats_for(dir.path()).unwrap();
        assert!(stats.size > 0);
        assert!(stats.usage.used + stats.usage.available <= stats.size);
    }

    #[test]
    fn missing_path_is_a_statvfs_error_carrying_the_path() {
        let err = stats_for(Path::new("/definitely/not/here")).unwrap_err();
        match err {
            Error::Statvfs { path, .. } => assert_eq!(path, PathBuf::from("/definitely/not/here")),
            other => panic!("unexpected error {other:?}"),
        }
    }
}
