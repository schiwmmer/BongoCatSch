//! Interaction counting and its persistence.
//!
//! One keyboard key press or one mouse button press is one interaction; gamepad
//! buttons, cursor motion and reconciliation traffic do not count. The macOS
//! input worker records each accepted down edge into session counters, flushes
//! them onto the persisted totals with a time-based throttle, and a surface that
//! wants the lifetime count reads the totals.
//!
//! The counters persist here as their own record, and they also feed the
//! cultivation ladder: the app registers an [`set_input_hook`] that batches raw
//! interactions into growth, so a keystroke or click creeps the ladder up by one
//! while a burst is folded into a single durable command instead of one document
//! write per key. The platform crate counts; what counts are worth is the
//! pomodoro layer's decision, which is why the hook exists rather than a hard
//! dependency.
//!
//! The persisted document is its own file (`interaction-stats.json`) rather
//! than a field of `config.json`, so the interaction history cannot be reset
//! by the configuration's "latest valid backup" recovery path and does not
//! widen the configuration schema. It follows the same environment separation
//! as every other persisted file: the caller hands in the path, which comes
//! from the build's [`bongocat_config::StorageLayout`]. A document whose
//! `schema_version` is not the current one is refused and treated as empty, the
//! same strict-version rule the configuration uses; the next flush overwrites
//! it with the current version.

use std::{
    io,
    path::{Path, PathBuf},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use bongocat_storage::{create_private_dir_all, write_private_atomic};
use serde::{Deserialize, Serialize};

/// The version this crate writes and the only one it reads back.
pub const INTERACTION_STATS_SCHEMA_VERSION: u32 = 1;

/// How long a dirty session waits before its deltas are flushed to disk. The
/// input worker calls [`flush_if_due`] on every run-loop slice, so the
/// throttle — not the caller cadence — bounds the write frequency.
const FLUSH_INTERVAL: Duration = Duration::from_secs(30);

/// The persisted document shape. New fields must default when absent so an
/// older document written by an earlier build keeps loading (the same
/// compatibility rule the configuration follows).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedStats {
    pub schema_version: u32,
    #[serde(default)]
    pub key_interactions: u64,
    #[serde(default)]
    pub mouse_interactions: u64,
}

/// Refuse anything that is not the current schema version instead of
/// interpreting it: the reader cannot know what an unknown version meant, and
/// silently adopting one would overwrite a document another build wrote on
/// purpose. The caller starts from empty totals and the next flush replaces
/// the file with the current version.
pub fn read_stats(path: &Path) -> PersistedStats {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return PersistedStats::default();
        }
        Err(_) => return PersistedStats::default(),
    };
    let parsed: PersistedStats = match serde_json::from_slice(&bytes) {
        Ok(parsed) => parsed,
        Err(_) => return PersistedStats::default(),
    };
    if parsed.schema_version != INTERACTION_STATS_SCHEMA_VERSION {
        return PersistedStats::default();
    }
    parsed
}

/// Replace the document atomically, creating the parent directory when it does
/// not exist yet (the first flush may run before anything else has created the
/// environment root).
pub fn write_stats(path: &Path, stats: &PersistedStats) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        create_private_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec(stats).expect("interaction stats serialize");
    write_private_atomic(path, &bytes)
}

struct Shared {
    path: PathBuf,
    persisted: Mutex<PersistedStats>,
    session_key: AtomicU64,
    session_mouse: AtomicU64,
    dirty: AtomicBool,
    last_flush: Mutex<Option<Instant>>,
}

static STATS: OnceLock<Shared> = OnceLock::new();

/// A closure run on every recorded keyboard or mouse interaction. The app
/// registers it to forward raw activity into the cultivation growth, which is a
/// pomodoro-layer concern the platform crate must not name directly. The hook
/// fires on the input worker, so it has to be `Send + Sync`.
static INPUT_HOOK: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

/// A closure run whenever the interaction counters are flushed, so a consumer can
/// drain its own per-input counters at the same cadence. Without it, activity
/// credited in [`INPUT_HOOK`] but not yet batched would be stranded when input
/// stops; the flush drains whatever remains.
static INPUT_FLUSH_HOOK: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

/// Register the closure invoked on every keyboard or mouse interaction.
///
/// The first call wins; later calls are ignored. Intended to be called once at
/// startup, before any input arrives.
pub fn set_input_hook(hook: Box<dyn Fn() + Send + Sync>) {
    let _ = INPUT_HOOK.set(hook);
}

/// Register the closure invoked when the interaction counters flush.
///
/// The first call wins; later calls are ignored.
pub fn set_input_flush_hook(hook: Box<dyn Fn() + Send + Sync>) {
    let _ = INPUT_FLUSH_HOOK.set(hook);
}

/// Install the persistence path and load whatever document is already there.
///
/// The first call wins; later calls are ignored so a re-started input service
/// cannot reload the file over counters the session has already accumulated.
/// A load failure keeps empty totals: the file is re-created on the next
/// flush, which is the same "empty document, keep going" behaviour the
/// configuration's recovery path ends at.
pub fn init(path: PathBuf) {
    let persisted = read_stats(&path);
    let _ = STATS.set(Shared {
        path,
        persisted: Mutex::new(persisted),
        session_key: AtomicU64::new(0),
        session_mouse: AtomicU64::new(0),
        dirty: AtomicBool::new(false),
        last_flush: Mutex::new(None),
    });
}

/// Record one keyboard interaction (one real key down).
pub fn record_key() {
    if let Some(shared) = STATS.get() {
        shared.session_key.fetch_add(1, Ordering::Relaxed);
        shared.dirty.store(true, Ordering::Release);
    }
    if let Some(hook) = INPUT_HOOK.get() {
        hook();
    }
}

/// Record one mouse interaction (one mouse button down).
pub fn record_mouse() {
    if let Some(shared) = STATS.get() {
        shared.session_mouse.fetch_add(1, Ordering::Relaxed);
        shared.dirty.store(true, Ordering::Release);
    }
    if let Some(hook) = INPUT_HOOK.get() {
        hook();
    }
}

/// The lifetime totals: persisted plus this session's unflushed deltas.
///
/// Returns zeros before [`init`], which is what an input service that started
/// without a storage layout (a test, a preview binary) wants: counting works
/// in memory, nothing is persisted, nothing is reported as history.
pub fn totals() -> (u64, u64, u64) {
    let Some(shared) = STATS.get() else {
        return (0, 0, 0);
    };
    let persisted = *shared
        .persisted
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let key = persisted
        .key_interactions
        .saturating_add(shared.session_key.load(Ordering::Relaxed));
    let mouse = persisted
        .mouse_interactions
        .saturating_add(shared.session_mouse.load(Ordering::Relaxed));
    (key, mouse, key.saturating_add(mouse))
}

/// Fold the session deltas into the persisted totals and write the document.
fn flush(shared: &Shared) -> io::Result<()> {
    let mut persisted = shared
        .persisted
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let session_key = shared.session_key.swap(0, Ordering::Relaxed);
    let session_mouse = shared.session_mouse.swap(0, Ordering::Relaxed);
    persisted.key_interactions = persisted.key_interactions.saturating_add(session_key);
    persisted.mouse_interactions = persisted.mouse_interactions.saturating_add(session_mouse);
    persisted.schema_version = INTERACTION_STATS_SCHEMA_VERSION;
    match write_stats(&shared.path, &persisted) {
        Ok(()) => {
            shared.dirty.store(false, Ordering::Release);
            *shared
                .last_flush
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Instant::now());
            Ok(())
        }
        Err(error) => {
            // The deltas were folded into the totals before the write failed,
            // so they stay counted; put them back as session deltas so the
            // next attempt carries them again.
            persisted.key_interactions = persisted
                .key_interactions
                .saturating_sub(session_key.min(persisted.key_interactions));
            persisted.mouse_interactions = persisted
                .mouse_interactions
                .saturating_sub(session_mouse.min(persisted.mouse_interactions));
            shared.session_key.fetch_add(session_key, Ordering::Relaxed);
            shared
                .session_mouse
                .fetch_add(session_mouse, Ordering::Relaxed);
            shared.dirty.store(true, Ordering::Release);
            Err(error)
        }
    }
}

/// Write through when the throttle interval has elapsed. Runs on the input
/// worker's run-loop slice; a failed write is reported to the caller but the
/// deltas stay queued, so the next due attempt carries them.
pub fn flush_if_due() {
    if let Some(hook) = INPUT_FLUSH_HOOK.get() {
        hook();
    }
    let Some(shared) = STATS.get() else {
        return;
    };
    if !shared.dirty.load(Ordering::Acquire) {
        return;
    }
    let due = {
        let last = shared
            .last_flush
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        last.is_none_or(|last| last.elapsed() >= FLUSH_INTERVAL)
    };
    if due {
        let _ = flush(shared);
    }
}

/// Write through immediately, ignoring the throttle. Called when the input
/// service stops, which is the shutdown-order point every exit path passes
/// through.
pub fn flush_now() {
    if let Some(hook) = INPUT_FLUSH_HOOK.get() {
        hook();
    }
    let Some(shared) = STATS.get() else {
        return;
    };
    if shared.dirty.load(Ordering::Acquire) {
        let _ = flush(shared);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_document_reads_as_empty() {
        let directory = tempfile::tempdir().expect("tempdir");
        assert_eq!(
            read_stats(&directory.path().join("absent.json")),
            PersistedStats::default()
        );
    }

    #[test]
    fn the_written_document_round_trips() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("interaction-stats.json");
        write_stats(
            &path,
            &PersistedStats {
                schema_version: INTERACTION_STATS_SCHEMA_VERSION,
                key_interactions: 12,
                mouse_interactions: 34,
            },
        )
        .expect("write");
        assert_eq!(
            read_stats(&path),
            PersistedStats {
                schema_version: INTERACTION_STATS_SCHEMA_VERSION,
                key_interactions: 12,
                mouse_interactions: 34,
            }
        );
    }

    /// A document whose fields were written by an earlier build without some
    /// of them still loads: the new fields default instead of failing the
    /// read, and the totals they carry are simply zero.
    #[test]
    fn an_older_document_with_missing_fields_still_loads() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("interaction-stats.json");
        std::fs::write(
            &path,
            format!(
                r#"{{"schema_version":{INTERACTION_STATS_SCHEMA_VERSION},"key_interactions":7}}"#
            ),
        )
        .expect("seed");
        assert_eq!(
            read_stats(&path).mouse_interactions,
            0,
            "the missing field defaults"
        );
        assert_eq!(read_stats(&path).key_interactions, 7);
    }

    #[test]
    fn an_unsupported_schema_version_is_refused_as_empty() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("interaction-stats.json");
        std::fs::write(
            &path,
            r#"{"schema_version":2,"key_interactions":7,"mouse_interactions":8}"#,
        )
        .expect("seed");
        assert_eq!(read_stats(&path), PersistedStats::default());
    }

    #[test]
    fn a_corrupt_document_is_refused_as_empty() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("interaction-stats.json");
        std::fs::write(&path, b"{not json").expect("seed");
        assert_eq!(read_stats(&path), PersistedStats::default());
    }

    /// A write that fails mid-flush must not lose the deltas it folded in:
    /// the totals go back to the session counters so the next attempt carries
    /// them again.
    #[test]
    fn a_failed_flush_keeps_the_session_deltas() {
        let shared = Shared {
            // `/dev/null` is a device file, so creating the parent directory
            // under it always fails, which is what drives the flush into its
            // error path.
            path: PathBuf::from("/dev/null/interaction-stats.json"),
            persisted: Mutex::new(PersistedStats::default()),
            session_key: AtomicU64::new(3),
            session_mouse: AtomicU64::new(4),
            dirty: AtomicBool::new(true),
            last_flush: Mutex::new(None),
        };
        assert!(flush(&shared).is_err());
        assert_eq!(
            shared.session_key.load(Ordering::Relaxed),
            3,
            "the key deltas were returned to the session"
        );
        assert_eq!(
            shared.session_mouse.load(Ordering::Relaxed),
            4,
            "the mouse deltas were returned to the session"
        );
        assert_eq!(
            shared.persisted.lock().expect("persisted").key_interactions,
            0,
            "the failed write did not leave totals behind"
        );
    }
}
