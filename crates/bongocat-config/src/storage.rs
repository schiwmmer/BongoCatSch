//! Where this build's data lives.
//!
//! Development and Production share one shape and never one root: the layout is
//! derived from the bundle id and the environment, so a Development build cannot
//! read, write or lock Production data and a Production build cannot inherit a
//! Development preference.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildEnvironment {
    Development,
    Production,
}

impl BuildEnvironment {
    pub const fn directory_name(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Production => "production",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageLayout {
    pub environment: BuildEnvironment,
    pub root: PathBuf,
    pub config: PathBuf,
    pub window_state: PathBuf,
    pub models: PathBuf,
    /// The user side of the models the build ships.
    ///
    /// A preset's package lives inside the application bundle — a signed
    /// `.app` on macOS — which the product may not write to. The artwork that
    /// replaces a preset's cover is kept here instead, in the same per-model
    /// shape a package uses (`<id>/resources/cover.png`), so the settings page
    /// reads a cover the same way whichever origin it came from.
    pub model_overrides: PathBuf,
    /// The lifetime interaction counters (one file, one document).
    ///
    /// It is deliberately not a `config.json` field: the interaction history
    /// must survive the configuration's backup-recovery path untouched, so it
    /// lives beside it as its own document.
    pub interaction_stats: PathBuf,
    /// The pomodoro tasks and their completion history (one file, one document).
    ///
    /// It is a sibling of the interaction counters rather than a `config.json`
    /// field for the same reason: the focus history has to survive the
    /// configuration's backup-recovery path untouched. The running countdown is
    /// deliberately absent from it — see ADR-0090.
    pub pomodoro_tasks: PathBuf,
    pub backups: PathBuf,
    pub logs: PathBuf,
    pub updates: PathBuf,
    pub locks: PathBuf,
}

impl StorageLayout {
    pub fn under_application_root(
        application_root: impl AsRef<Path>,
        environment: BuildEnvironment,
    ) -> Self {
        let root = application_root.as_ref().join(environment.directory_name());
        Self {
            environment,
            config: root.join("config.json"),
            window_state: root.join(WINDOW_STATE_FILE_NAME),
            models: root.join("models"),
            model_overrides: root.join("model-overrides"),
            interaction_stats: root.join("interaction-stats.json"),
            pomodoro_tasks: root.join("pomodoro-tasks.json"),
            backups: root.join("backups"),
            logs: root.join("logs"),
            updates: root.join("updates"),
            locks: root.join("locks"),
            root,
        }
    }

    pub fn under(base: impl AsRef<Path>, environment: BuildEnvironment) -> Self {
        Self::under_application_root(base.as_ref().join(BUNDLE_ID), environment)
    }

    pub(crate) fn create_directories(&self) -> io::Result<()> {
        create_private_dir_all(&self.root)?;
        for directory in [
            &self.models,
            &self.model_overrides,
            &self.backups,
            &self.logs,
            &self.updates,
            &self.locks,
        ] {
            create_private_dir_all(directory)?;
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PlatformStorageError {
    #[error("platform data directory is unavailable")]
    DataDirectoryUnavailable,
}

pub fn platform_layout(
    environment: BuildEnvironment,
) -> Result<StorageLayout, PlatformStorageError> {
    let root = dirs::data_dir()
        .ok_or(PlatformStorageError::DataDirectoryUnavailable)?
        .join(BUNDLE_ID);
    Ok(StorageLayout::under_application_root(root, environment))
}

