#[cfg(windows)]
use anyhow::anyhow;
use anyhow::{Context, Result};
#[cfg(unix)]
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

fn temporary_path(path: &Path) -> Result<PathBuf> {
    let parent = path
        .parent()
        .context("Persistence path has no parent directory")?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("Persistence path has no UTF-8 file name")?;
    Ok(parent.join(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        Uuid::new_v4()
    )))
}

/// Write a small state file without exposing a partially-written destination.
///
/// The data is written and synced through a unique sibling file first. Unix
/// then replaces the destination atomically. Windows uses a rollback-safe
/// backup because `rename` cannot replace an existing destination there.
pub fn atomic_write(path: &Path, contents: &[u8], unix_mode: Option<u32>) -> Result<()> {
    #[cfg(windows)]
    let _ = unix_mode;

    let parent = path
        .parent()
        .context("Persistence path has no parent directory")?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("Failed to create {}", parent.display()))?;

    let temp = temporary_path(path)?;
    let result = (|| -> Result<()> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        if let Some(mode) = unix_mode {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(mode);
        }

        let mut file = options
            .open(&temp)
            .with_context(|| format!("Failed to create {}", temp.display()))?;
        file.write_all(contents)
            .with_context(|| format!("Failed to write {}", temp.display()))?;
        file.sync_all()
            .with_context(|| format!("Failed to sync {}", temp.display()))?;
        drop(file);

        replace_destination(&temp, path)?;

        #[cfg(unix)]
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .with_context(|| format!("Failed to sync {}", parent.display()))?;
        Ok(())
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

#[cfg(not(windows))]
fn replace_destination(temp: &Path, destination: &Path) -> Result<()> {
    std::fs::rename(temp, destination).with_context(|| {
        format!(
            "Failed to replace {} with {}",
            destination.display(),
            temp.display()
        )
    })
}

#[cfg(windows)]
fn replace_destination(temp: &Path, destination: &Path) -> Result<()> {
    let backup = destination.with_extension(format!("bak-{}", Uuid::new_v4()));
    let had_destination = destination.exists();
    if had_destination {
        std::fs::rename(destination, &backup).with_context(|| {
            format!(
                "Failed to stage existing persistence file {}",
                destination.display()
            )
        })?;
    }

    match std::fs::rename(temp, destination) {
        Ok(()) => {
            if had_destination {
                if let Err(cleanup_error) = std::fs::remove_file(&backup) {
                    let rollback = std::fs::remove_file(destination)
                        .and_then(|_| std::fs::rename(&backup, destination));
                    return match rollback {
                        Ok(()) => Err(cleanup_error).with_context(|| {
                            format!(
                                "Failed to remove staged persistence backup {}; update was rolled back",
                                backup.display()
                            )
                        }),
                        Err(rollback_error) => Err(anyhow!(
                            "Failed to remove staged backup {} ({cleanup_error}) and failed to restore it ({rollback_error})",
                            backup.display()
                        )),
                    };
                }
            }
            Ok(())
        }
        Err(error) => {
            if had_destination {
                let _ = std::fs::rename(&backup, destination);
            }
            Err(error).with_context(|| {
                format!(
                    "Failed to replace persistence file {}",
                    destination.display()
                )
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_creates_new_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("new-state.json");

        atomic_write(&path, b"created", Some(0o600)).unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"created");
    }

    #[test]
    fn atomic_write_replaces_existing_file_and_cleans_temporary_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state.json");
        std::fs::write(&path, b"old").unwrap();

        atomic_write(&path, b"new", Some(0o600)).unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        let leftovers = std::fs::read_dir(directory.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp"))
            .count();
        assert_eq!(leftovers, 0);
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_applies_restrictive_mode_at_creation() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("secret");
        atomic_write(&path, b"token", Some(0o600)).unwrap();

        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
