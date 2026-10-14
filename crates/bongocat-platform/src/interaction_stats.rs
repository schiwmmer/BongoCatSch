//! Interaction counting, level growth and its persistence.
//!
//! One keyboard key press or one mouse button press is one interaction; gamepad
//! buttons, cursor motion and reconciliation traffic do not count. The macOS
//! input worker records each accepted down edge into session counters, flushes
//! them onto the persisted totals with a time-based throttle, and the overlay
//! reads the totals to draw the hover stats HUD.
//!
//! The persisted document is its own file (`interaction-stats.json`) rather
//! than a field of `config.json`, so the interaction history cannot be reset
//! by the configuration's "latest valid backup" recovery path and does not
//! widen the configuration schema. It follows the same environment separation
//! as every other persisted file: the caller hands in the path, which comes
//! from the build's [`bongocat_config::StorageLayout`].
//!
//! Level growth is a pure function of the lifetime total, so every caller —
//! the HUD, a future settings page, the tests — reads the same anchors from
//! [`LEVEL_ANCHORS`] and the same interpolation from [`threshold_for`]. A
//! document whose `schema_version` is not the current one is refused and
//! treated as empty, the same strict-version rule the configuration uses; the
//! next flush overwrites it with the current version.

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

/// The highest level the curve reaches.
pub const MAX_LEVEL: u16 = 1000;

/// The anchor points of the level curve: `(level, total the level opens at)`.
///
/// The original ten-level table is stretched a hundredfold: every historic
/// level `k` becomes anchor level `100 * k` carrying the same threshold, and
/// the levels between two anchors interpolate linearly, so the curve's shape
/// (gentle at the start, rare at the top) is preserved exactly. The last
/// segment extends the previous one's slope — 1,000 interactions per level —
/// to level 1000, which opens at 300,000.
const LEVEL_ANCHORS: [(u16, u64); 11] = [
    (1, 0),
    (100, 100),
    (200, 500),
    (300, 2_000),
    (400, 5_000),
    (500, 10_000),
    (600, 30_000),
    (700, 60_000),
    (800, 100_000),
    (900, 200_000),
    (1000, 300_000),
];

/// The lifetime interaction count `level` opens at. Levels outside
/// `1..=MAX_LEVEL` are clamped.
pub fn threshold_for(level: u16) -> u64 {
    let level = level.clamp(1, MAX_LEVEL);
    for window in LEVEL_ANCHORS.windows(2) {
        let (start_level, start_total) = window[0];
        let (end_level, end_total) = window[1];
        if level <= end_level {
            let level_span = u64::from(end_level - start_level);
            let total_span = end_total - start_total;
            return start_total + total_span * u64::from(level - start_level) / level_span;
        }
    }
    LEVEL_ANCHORS[LEVEL_ANCHORS.len() - 1].1
}

/// The level a lifetime interaction total has reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionLevel {
    pub level: u16,
    /// The total this level opens at; progress is measured from here.
    pub floor: u64,
    /// The total the next level opens at; `None` at the level cap.
    pub next: Option<u64>,
}

/// The level `total` interactions have reached. The anchors partition the
/// curve; within the segment the anchor's band brackets, the level is the
/// linear interpolation inverted, so the answer is the highest level whose
/// [`threshold_for`] is at or below the total.
pub fn level_for(total: u64) -> InteractionLevel {
    let mut index = 0;
    for (position, &(_, anchor_total)) in LEVEL_ANCHORS.iter().enumerate() {
        if anchor_total <= total {
            index = position;
        } else {
            break;
        }
    }
    let (start_level, start_total) = LEVEL_ANCHORS[index];
    let last = index == LEVEL_ANCHORS.len() - 1;
    if last && total >= start_total {
        return InteractionLevel {
            level: start_level,
            floor: start_total,
            next: None,
        };
    }
    let (end_level, end_total) = LEVEL_ANCHORS[index + 1];
    let total_span = end_total - start_total;
    let progressed = total.saturating_sub(start_total);
    // Invert the interpolation exactly: the answer is the largest `into`
    // whose floor threshold is at or below the total, which is the ceiling
    // of the raw ratio minus one. Plain flooring would miss the level whose
    // threshold rounds down (level 2 opens at 1 interaction, yet a floored
    // inverse reports 1 for that total).
    let level_span = u64::from(end_level - start_level);
    let into = (level_span * progressed + level_span - 1) / total_span;
    let level = start_level + into as u16;
    InteractionLevel {
        level,
        floor: threshold_for(level),
        next: Some(threshold_for(level + 1)),
    }
}

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
}

/// Record one mouse interaction (one mouse button down).
pub fn record_mouse() {
    if let Some(shared) = STATS.get() {
        shared.session_mouse.fetch_add(1, Ordering::Relaxed);
        shared.dirty.store(true, Ordering::Release);
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
    fn levels_follow_the_threshold_curve_at_both_sides_of_every_anchor() {
        assert_eq!(level_for(0).level, 1);
        assert_eq!(level_for(99).level, 99);
        assert_eq!(level_for(100).level, 100);
        assert_eq!(
            level_for(499).level,
            199,
            "399 of 400 through the 100→200 band"
        );
        assert_eq!(level_for(500).level, 200);
        assert_eq!(
            level_for(1_999).level,
            299,
            "1,499 of 1,500 through the 200→300 band"
        );
        assert_eq!(level_for(2_000).level, 300);
        assert_eq!(level_for(200_000).level, 900);
        assert_eq!(level_for(300_000).level, 1000);
        assert_eq!(level_for(u64::MAX).level, MAX_LEVEL);
        assert_eq!(level_for(u64::MAX).next, None);
    }

    #[test]
    fn thresholds_interpolate_inside_the_anchor_bands() {
        assert_eq!(threshold_for(1), 0);
        assert_eq!(threshold_for(100), 100);
        assert_eq!(
            threshold_for(101),
            104,
            "500-100 over 100 levels is 4 a level"
        );
        assert_eq!(threshold_for(200), 500);
        assert_eq!(threshold_for(950), 250_000);
        assert_eq!(threshold_for(1000), 300_000);
        assert_eq!(threshold_for(1001), 300_000, "levels above the cap clamp");
        assert_eq!(threshold_for(0), 0);
    }

    /// The curve must be invertible and monotonically non-decreasing, or the
    /// HUD would show a level whose floor overshoots its total.
    #[test]
    fn every_level_inverts_from_its_own_threshold() {
        let mut previous = 0;
        for level in 1..=MAX_LEVEL {
            let threshold = threshold_for(level);
            assert!(threshold >= previous, "threshold regressed at {level}");
            assert_eq!(
                level_for(threshold).level,
                level,
                "inversion failed at {level}"
            );
            let between = level_for(threshold);
            assert!(between.floor <= threshold, "floor overshoots at {level}");
            previous = threshold;
        }
    }

    #[test]
    fn each_level_reports_the_floor_it_measures_progress_from() {
        let level = level_for(5_000);
        assert_eq!(level.floor, 5_000);
        assert_eq!(
            level.next,
            Some(5_050),
            "10,000-5,000 over 100 levels is 50 a level"
        );
        let first = level_for(0);
        assert_eq!(first.floor, 0);
        assert_eq!(first.next, Some(1));
        let capped = level_for(300_000);
        assert_eq!(capped.floor, 300_000);
        assert_eq!(capped.next, None);
    }

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
