use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const APP_NAME: &str = "aisub2api";
pub const LEGACY_APP_NAME: &str = "claude-proxy";

const LOG_ROTATION_FILES: usize = 5;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct StateMigration {
    pub config_dir_migrated: bool,
    pub cache_dir_migrated: bool,
    pub config_dir_conflict: bool,
    pub cache_dir_conflict: bool,
    pub runtime_files_migrated: usize,
}

impl StateMigration {
    pub fn migrated(&self) -> bool {
        self.config_dir_migrated || self.cache_dir_migrated || self.runtime_files_migrated > 0
    }

    pub fn has_conflicts(&self) -> bool {
        self.config_dir_conflict || self.cache_dir_conflict
    }
}

pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|path| path.join(APP_NAME))
}

pub fn cache_dir() -> Option<PathBuf> {
    dirs::cache_dir().map(|path| path.join(APP_NAME))
}

pub fn migrate_legacy_state() -> io::Result<StateMigration> {
    let mut migration = StateMigration::default();

    if let Some(base_dir) = dirs::config_dir() {
        let legacy = base_dir.join(LEGACY_APP_NAME);
        let canonical = base_dir.join(APP_NAME);
        let outcome = migrate_directory(&legacy, &canonical)?;
        migration.config_dir_migrated = outcome.migrated;
        migration.config_dir_conflict = outcome.conflict;
        if canonical.exists() {
            migration.runtime_files_migrated = migrate_runtime_file_names(&canonical)?;
        }
    }

    if let Some(base_dir) = dirs::cache_dir() {
        let legacy = base_dir.join(LEGACY_APP_NAME);
        let canonical = base_dir.join(APP_NAME);
        let outcome = migrate_directory(&legacy, &canonical)?;
        migration.cache_dir_migrated = outcome.migrated;
        migration.cache_dir_conflict = outcome.conflict;
    }

    Ok(migration)
}

#[derive(Debug, Default, PartialEq, Eq)]
struct DirectoryMigration {
    migrated: bool,
    conflict: bool,
}

fn migrate_directory(legacy: &Path, canonical: &Path) -> io::Result<DirectoryMigration> {
    if !legacy.exists() {
        return Ok(DirectoryMigration::default());
    }
    if canonical.exists() {
        return Ok(DirectoryMigration {
            migrated: false,
            conflict: true,
        });
    }
    if let Some(parent) = canonical.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(legacy, canonical)?;
    Ok(DirectoryMigration {
        migrated: true,
        conflict: false,
    })
}

fn migrate_runtime_file_names(config_dir: &Path) -> io::Result<usize> {
    let mut migrated = 0;
    migrated += migrate_named_file(
        &config_dir.join(format!("{LEGACY_APP_NAME}.pid")),
        &config_dir.join(format!("{APP_NAME}.pid")),
    )? as usize;

    for directory in [config_dir.to_path_buf(), config_dir.join("logs")] {
        migrated += migrate_named_file(
            &directory.join(format!("{LEGACY_APP_NAME}.log")),
            &directory.join(format!("{APP_NAME}.log")),
        )? as usize;
        for index in 1..=LOG_ROTATION_FILES {
            migrated += migrate_named_file(
                &directory.join(format!("{LEGACY_APP_NAME}.log.{index}")),
                &directory.join(format!("{APP_NAME}.log.{index}")),
            )? as usize;
        }
    }

    Ok(migrated)
}

fn migrate_named_file(legacy: &Path, canonical: &Path) -> io::Result<bool> {
    if !legacy.exists() || canonical.exists() {
        return Ok(false);
    }
    fs::rename(legacy, canonical)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn migrates_legacy_directory_without_overwriting_canonical_state() {
        let root = tempdir().unwrap();
        let legacy = root.path().join(LEGACY_APP_NAME);
        let canonical = root.path().join(APP_NAME);
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("config.toml"), "legacy").unwrap();

        let outcome = migrate_directory(&legacy, &canonical).unwrap();

        assert_eq!(
            outcome,
            DirectoryMigration {
                migrated: true,
                conflict: false,
            }
        );
        assert!(!legacy.exists());
        assert_eq!(
            fs::read_to_string(canonical.join("config.toml")).unwrap(),
            "legacy"
        );

        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("config.toml"), "second").unwrap();
        let outcome = migrate_directory(&legacy, &canonical).unwrap();
        assert_eq!(
            outcome,
            DirectoryMigration {
                migrated: false,
                conflict: true,
            }
        );
        assert_eq!(
            fs::read_to_string(canonical.join("config.toml")).unwrap(),
            "legacy"
        );
    }

    #[test]
    fn migrates_runtime_file_names_after_directory_rename() {
        let root = tempdir().unwrap();
        let logs = root.path().join("logs");
        fs::create_dir_all(&logs).unwrap();
        fs::write(root.path().join("claude-proxy.pid"), "123").unwrap();
        fs::write(root.path().join("claude-proxy.log"), "root").unwrap();
        fs::write(logs.join("claude-proxy.log.1"), "rotated").unwrap();

        let migrated = migrate_runtime_file_names(root.path()).unwrap();

        assert_eq!(migrated, 3);
        assert_eq!(
            fs::read_to_string(root.path().join("aisub2api.pid")).unwrap(),
            "123"
        );
        assert_eq!(
            fs::read_to_string(root.path().join("aisub2api.log")).unwrap(),
            "root"
        );
        assert_eq!(
            fs::read_to_string(logs.join("aisub2api.log.1")).unwrap(),
            "rotated"
        );
    }
}
