#![forbid(unsafe_code)]

#[cfg(all(feature = "production", feature = "storage-test-injection"))]
compile_error!("storage-test-injection cannot be enabled for Production builds");

use bongocat_audio::MotionAudioShutdownError;
use bongocat_config::{BuildEnvironment, ConfigError, PlatformStorageError, WindowStateError};
use bongocat_model::{CommittedModel, ModelError, ModelId};
use bongocat_model_store::ModelStoreError;
use bongocat_pomodoro::PomodoroShutdownError;
use bongocat_runtime::{
    ExpressionIdError, MotionIdError, RuntimeCommandFailure, SendError, ShutdownError,
};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

mod app_log;
mod application;
mod config_projection;
mod diagnostics_export;
mod import_progress;
mod model_identity;
mod model_input;
mod model_listing;
mod model_titles;
mod settings;
mod shortcut_config;
mod shortcuts;
mod startup_permission;
#[cfg(test)]
mod tests;
mod update;

pub use app_log::{
    ApplicationLogCode, ApplicationLogComponent, ApplicationLogContext, ApplicationLogDiagnostics,
    ApplicationLogError, ApplicationLogEvent, ApplicationLogEventCounts, ApplicationLogHandle,
    ApplicationLogLevel, ApplicationPanicHook, CoreLogDiagnostics,
};
pub use application::Application;
pub(crate) use config_projection::{
    random_behavior_mode_from_runtime, random_behavior_mode_to_runtime_settings,
    settings_logging_from_config,
};
pub use settings::{
    ApplicationSettingsService, DockIconCapability, SettingsServiceJoinError, StatusIconCapability,
    TaskbarIconCapability,
};
pub use shortcuts::application_shortcut_dispatcher;
pub use startup_permission::ensure_startup_permission;
pub use update::{ApplicationUpdateService, UpdateServiceError, restart_required_after_install};

#[cfg(test)]
pub(crate) use tests::repository_preset_root;

/// Work the settings worker hands to the thread that owns the product's windows.
///
/// Two things the application has to do can only happen on that thread: showing the
/// settings window a global shortcut asked for, and rendering a just-imported model
/// into its own cover. The worker cannot do either — the shortcut service is a
/// platform thread of its own, and a cover capture creates a native window — so it
/// raises a signal and the GPUI thread drains it. Both fields are shared handles, so
/// every clone observes the same work.
#[derive(Clone, Default)]
pub struct ApplicationMainThreadSignals {
    open_settings: Arc<AtomicBool>,
    cover_captures: Arc<Mutex<VecDeque<CoverCaptureRequest>>>,
}

impl ApplicationMainThreadSignals {
    pub fn request_open_settings(&self) {
        self.open_settings.store(true, Ordering::Release);
    }

    pub fn take_open_settings_request(&self) -> bool {
        self.open_settings.swap(false, Ordering::AcqRel)
    }

    /// Queue `model` to be rendered into its own cover by the GPUI thread.
    ///
    /// Nothing is written here: the caller has only just installed the model, and
    /// the cover it ships is left in place until a capture replaces it.
    pub fn request_model_cover_capture(
        &self,
        key: bongocat_ui_protocol::SettingsModelKey,
        model: CommittedModel,
    ) {
        self.cover_captures
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push_back(CoverCaptureRequest {
                key,
                model: Arc::new(model),
            });
    }

    /// Take the captures queued since the last call, oldest first.
    pub fn take_model_cover_captures(&self) -> Vec<CoverCaptureRequest> {
        self.cover_captures
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .drain(..)
            .collect()
    }
}

/// One installed model whose cover has to be rendered from the model itself.
///
/// The model travels with the request because loading it is the settings worker's
/// work: the GPUI thread gets something it can render, not an id it would have to
/// re-open the store for.
pub struct CoverCaptureRequest {
    key: bongocat_ui_protocol::SettingsModelKey,
    model: Arc<CommittedModel>,
}

impl CoverCaptureRequest {
    /// The model as the settings protocol names it, for writing the cover back.
    pub fn key(&self) -> &bongocat_ui_protocol::SettingsModelKey {
        &self.key
    }

    pub fn model(&self) -> &Arc<CommittedModel> {
        &self.model
    }
}

const COMMAND_CAPACITY: usize = 64;
const AUDIO_COMMAND_CAPACITY: usize = 16;
const RUNTIME_TIMEOUT: Duration = Duration::from_secs(2);
pub const PRODUCT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// PNG file signature, checked before a user-chosen image replaces a model's
/// cover. Only the signature is verified: the cover is display artwork for the
/// settings catalog, so a PNG that no decoder can read is a wrong picture, not
/// a broken model.
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

#[cfg(feature = "production")]
pub const BUILD_ENVIRONMENT: BuildEnvironment = BuildEnvironment::Production;

#[cfg(not(feature = "production"))]
pub const BUILD_ENVIRONMENT: BuildEnvironment = BuildEnvironment::Development;

/// Whether this build can actually check for and install updates.
///
/// Updates require a Production channel and a provisioned release signing key;
/// a Development build never installs a release artifact. This is the
/// availability fact only — the system menu additionally keeps the entry
/// clickable in Development builds, because the update window is where the
/// "development build" explanation lives.
pub fn update_check_available() -> bool {
    bongocat_update::UpdateRuntime::for_current_build(
        BUILD_ENVIRONMENT,
        PRODUCT_VERSION,
        bongocat_update::UpdateDiagnosticsTracker::default(),
    )
    .is_available()
}

#[derive(Debug, thiserror::Error)]
pub enum ApplicationError {
    #[error("storage setup failed: {0}")]
    PlatformStorage(PlatformStorageError),
    #[error("configuration failed: {0}")]
    Config(ConfigError),
    #[error("model preparation failed: {0}")]
    Model(ModelError),
    #[error("model store failed: {0}")]
    ModelStore(ModelStoreError),
    #[error("motion id failed: {0}")]
    MotionId(MotionIdError),
    #[error("expression id failed: {0}")]
    ExpressionId(ExpressionIdError),
    #[error("preset model cannot be deleted: {}", .0.as_str())]
    PresetModelDeletion(ModelId),
    /// The model a request names is not in its catalog or store.
    ///
    /// Both origins report this the same way: a preset a build no longer ships
    /// and an installed package the user removed by hand are the same fact to
    /// the request that named either one.
    #[error("model was not found: {}", .0.as_str())]
    ModelNotFound(ModelId),
    #[error("model title is not usable")]
    ModelTitleInvalid,
    #[error("model cover must be a PNG image within the size limit")]
    ModelCoverInvalid,
    #[error("runtime command failed: {0}")]
    RuntimeCommand(SendError),
    #[error("runtime command {} failed: {:?}", .0.sequence, .0.code)]
    RuntimeCommandFailed(RuntimeCommandFailure),
    #[error("runtime did not publish the requested revision")]
    RuntimeDidNotPublish,
    #[error("runtime did not prepare the requested render model")]
    RuntimeDidNotPrepareModel,
    #[error("application render consumer is unavailable")]
    RenderConsumerUnavailable,
    #[error("shutdown failed: {0}")]
    Shutdown(ShutdownError),
    #[error("motion audio shutdown failed: {0}")]
    MotionAudioShutdown(MotionAudioShutdownError),
    #[error("pomodoro shutdown failed: {0}")]
    PomodoroShutdown(PomodoroShutdownError),
    #[error("shutdown failed: {0}")]
    ShutdownAggregate(ApplicationShutdownError),
    #[error("application logging failed: {0}")]
    ApplicationLog(ApplicationLogError),
    #[error("configuration rollback failed: {0}")]
    ConfigRollback(ConfigError),
    #[error("window state failed: {0}")]
    WindowState(WindowStateError),
}

impl ApplicationError {
    pub(crate) const fn stable_code(&self) -> &'static str {
        match self {
            Self::PlatformStorage(_) => "platform_storage_failed",
            Self::Config(_) | Self::ConfigRollback(_) => "config_failed",
            Self::Model(_) => "model_preparation_failed",
            // The store's own code, not a folded "something in the store
            // failed": twelve different refusals reach this arm, and a log
            // that cannot tell a lock from a rejected package sends the next
            // bug report no further than the reporter's own guess.
            Self::ModelStore(error) => error.code.as_str(),
            Self::MotionId(_) => "motion_id_invalid",
            Self::ExpressionId(_) => "expression_id_invalid",
            Self::PresetModelDeletion(_) => "preset_model_deletion_rejected",
            Self::ModelNotFound(_) => "model_not_found",
            Self::ModelTitleInvalid => "model_title_invalid",
            Self::ModelCoverInvalid => "model_cover_invalid",
            Self::RuntimeCommand(_) => "runtime_transport_failed",
            Self::RuntimeCommandFailed(_) => "runtime_command_failed",
            Self::RuntimeDidNotPublish => "runtime_snapshot_timeout",
            Self::RuntimeDidNotPrepareModel => "runtime_model_prepare_timeout",
            Self::RenderConsumerUnavailable => "render_consumer_unavailable",
            Self::Shutdown(_) => "runtime_shutdown_failed",
            Self::MotionAudioShutdown(_) => "audio_shutdown_failed",
            Self::PomodoroShutdown(_) => "pomodoro_shutdown_failed",
            Self::ShutdownAggregate(_) => "shutdown_aggregate_failed",
            Self::ApplicationLog(_) => "application_log_failed",
            Self::WindowState(_) => "window_state_failed",
        }
    }
}

/// One service that failed to stop in time or panicked.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShutdownFailure {
    /// The service's stable name, as the logs and the error code use it.
    pub service: &'static str,
    pub message: String,
}

/// Two or more services that failed to stop, each named.
///
/// Shutdown never hides one failure behind another. A single failure keeps its
/// own error variant, so a log names the cause precisely; the aggregate exists
/// for the case where more than one service is stuck, and it is a list rather
/// than a struct of fields so the next worker service does not change its shape.
#[derive(Clone, Debug, Eq, PartialEq, Default, thiserror::Error)]
#[error("{}", ShutdownFailure::joined(.failures))]
pub struct ApplicationShutdownError {
    pub failures: Vec<ShutdownFailure>,
}

impl ShutdownFailure {
    fn joined(failures: &[ShutdownFailure]) -> String {
        failures
            .iter()
            .map(|failure| format!("{}: {}", failure.service, failure.message))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

/// Combine the runtime and every worker service into one shutdown outcome.
///
/// The runtime's stopped snapshot is what the caller asked for, so it is
/// returned whenever the runtime succeeded. A single service failure keeps its
/// own error variant; two or more are reported together so neither is lost
/// behind the other.
fn combine_shutdown_results<T>(
    runtime_result: Result<T, ShutdownError>,
    audio_result: Result<(), MotionAudioShutdownError>,
    pomodoro_result: Result<(), PomodoroShutdownError>,
) -> Result<T, ApplicationError> {
    let audio = audio_result.err();
    let pomodoro = pomodoro_result.err();
    let mut workers = Vec::new();
    if let Some(error) = &audio {
        workers.push(ShutdownFailure {
            service: "audio",
            message: error.to_string(),
        });
    }
    if let Some(error) = &pomodoro {
        workers.push(ShutdownFailure {
            service: "pomodoro",
            message: error.to_string(),
        });
    }
    match runtime_result {
        Ok(stopped) => match (workers.len(), audio, pomodoro) {
            // Nothing failed.
            (0, _, _) => Ok(stopped),
            // One worker failed and the runtime stopped: report that one
            // precisely rather than burying it in a list of one.
            (1, Some(audio), None) => Err(ApplicationError::MotionAudioShutdown(audio)),
            (1, None, Some(pomodoro)) => Err(ApplicationError::PomodoroShutdown(pomodoro)),
            // Two workers, or a worker plus a runtime failure.
            _ => Err(ApplicationError::ShutdownAggregate(
                ApplicationShutdownError { failures: workers },
            )),
        },
        Err(runtime) => {
            if workers.is_empty() {
                // Only the runtime failed: it keeps its own code, which is what
                // the log has always reported for a stuck worker.
                return Err(ApplicationError::Shutdown(runtime));
            }
            let mut failures = Vec::with_capacity(workers.len() + 1);
            failures.push(ShutdownFailure {
                service: "runtime",
                message: runtime.to_string(),
            });
            failures.extend(workers);
            Err(ApplicationError::ShutdownAggregate(
                ApplicationShutdownError { failures },
            ))
        }
    }
}

impl From<PlatformStorageError> for ApplicationError {
    fn from(error: PlatformStorageError) -> Self {
        Self::PlatformStorage(error)
    }
}

impl From<ConfigError> for ApplicationError {
    fn from(error: ConfigError) -> Self {
        Self::Config(error)
    }
}

impl From<WindowStateError> for ApplicationError {
    fn from(error: WindowStateError) -> Self {
        Self::WindowState(error)
    }
}

impl From<ModelError> for ApplicationError {
    fn from(error: ModelError) -> Self {
        Self::Model(error)
    }
}

impl From<MotionIdError> for ApplicationError {
    fn from(error: MotionIdError) -> Self {
        Self::MotionId(error)
    }
}

impl From<ExpressionIdError> for ApplicationError {
    fn from(error: ExpressionIdError) -> Self {
        Self::ExpressionId(error)
    }
}

impl From<ModelStoreError> for ApplicationError {
    fn from(error: ModelStoreError) -> Self {
        Self::ModelStore(error)
    }
}

impl From<ApplicationLogError> for ApplicationError {
    fn from(error: ApplicationLogError) -> Self {
        Self::ApplicationLog(error)
    }
}
