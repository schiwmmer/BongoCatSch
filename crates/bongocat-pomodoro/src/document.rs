//! The persisted document: the tasks and the history, with no live session.
//!
//! It is a separate file rather than a `config.json` field for the reason the
//! interaction counters have: the focus history must survive the
//! configuration's backup-recovery path untouched, so it lives beside it as its
//! own document with its own version.
//!
//! Every field except `schema_version` defaults when absent, so a document
//! written by an earlier build keeps loading. Adding a field is therefore a
//! `#[serde(default)]` and a test that reads the older shape.

use std::{fs, io, path::Path};

use bongocat_storage::{create_private_dir_all, write_private_atomic};
use serde::{Deserialize, Serialize};

/// The version this crate writes and the only one it reads back.
pub const POMODORO_TASKS_SCHEMA_VERSION: u32 = 1;

/// Whether a task is still being worked on or has been finished.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(non_camel_case_types)]
pub enum TaskStatus {
    /// The default so a task created before the status existed is not read as
    /// finished: an absent field must never close work the user did.
    #[default]
    in_progress,
    done,
}

/// One task, as stored.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct TaskRecord {
    pub id: u64,
    pub name: String,
    /// How many focus sessions the user expects this task to take.
    pub estimated_pomodoros: u32,
    pub completed_pomodoros: u32,
    #[serde(default)]
    pub status: TaskStatus,
    pub created_at_unix: i64,
    #[serde(default)]
    pub completed_at_unix: Option<i64>,
    /// When each focus session finished, oldest first.
    ///
    /// The monthly calendar is derived from these rather than from a second
    /// document, so a completion cannot be recorded in one place and forgotten
    /// in the other.
    #[serde(default)]
    pub completion_timestamps: Vec<i64>,
}

impl TaskRecord {
    /// A new task, as stored: in progress, nothing completed.
    pub(crate) fn new(id: u64, name: String, estimated_pomodoros: u32, now_unix: i64) -> Self {
        Self {
            id,
            name,
            estimated_pomodoros,
            completed_pomodoros: 0,
            status: TaskStatus::in_progress,
            created_at_unix: now_unix,
            completed_at_unix: None,
            completion_timestamps: Vec::new(),
        }
    }

    /// Mark the task finished or unfinished.
    ///
    /// Both directions clear or set the completion timestamp together with the
    /// status, so the task list and the calendar never disagree about whether
    /// a task is done.
    ///
    /// The completion time is the moment the task's *last focus session*
    /// finished, not the moment the checkbox was clicked: ticking the box is a
    /// confirmation that may happen minutes or days after the work did. A task
    /// finished without ever completing a session falls back to the click, so
    /// the field is still always set when the status says done.
    pub(crate) fn set_done(&mut self, done: bool, now_unix: i64) {
        self.status = if done {
            TaskStatus::done
        } else {
            TaskStatus::in_progress
        };
        self.completed_at_unix = done.then(|| {
            self.completion_timestamps
                .last()
                .copied()
                .unwrap_or(now_unix)
        });
    }
}

/// The whole document: one file, one version.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Document {
    pub schema_version: u32,
    #[serde(default)]
    pub tasks: Vec<TaskRecord>,
    /// How long one focus session lasts.
    #[serde(default)]
    pub focus_minutes: u32,
    /// How many focus sessions have ever finished, across every task.
    ///
    /// A task's own counter is deleted with the task; this one is not, so the
    /// total a surface shows counts the work actually done rather than the work
    /// still recorded. A document written before the counter existed derives it
    /// from the timestamps it does hold, so no history is lost on load.
    #[serde(default)]
    pub total_completions: u32,
    /// The cultivation ladder the sessions feed.
    ///
    /// It lives in this document rather than a file of its own so a finished
    /// session and the growth it paid are written by the same atomic replace: a
    /// crash between two files could otherwise count the session and drop its
    /// reward. A document written before the ladder existed reads as a
    /// cultivator who has just started.
    #[serde(default)]
    pub cultivation: crate::CultivationDocument,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            schema_version: POMODORO_TASKS_SCHEMA_VERSION,
            tasks: Vec::new(),
            focus_minutes: crate::DEFAULT_FOCUS_MINUTES,
            total_completions: 0,
            cultivation: crate::CultivationDocument::default(),
        }
    }
}

impl Document {
    /// The document as written back out: the version is always the current one,
    /// so a load that fell back to empty heals on the next write.
    pub(crate) fn normalized(mut self) -> Self {
        self.schema_version = POMODORO_TASKS_SCHEMA_VERSION;
        if self.focus_minutes == 0 {
            self.focus_minutes = crate::DEFAULT_FOCUS_MINUTES;
        }
        // An earlier build kept only the per-task timestamps, so the lifetime
        // total is recovered from them once rather than starting at zero and
        // throwing away history the user already earned.
        let recorded = self.tasks.iter().fold(0u32, |total, task| {
            total
                .saturating_add(u32::try_from(task.completion_timestamps.len()).unwrap_or(u32::MAX))
        });
        self.total_completions = self.total_completions.max(recorded);
        self
    }

    /// The next free task id.
    ///
    /// Derived from what is stored rather than kept as a counter, so a document
    /// that lost its last task still cannot hand out an id twice.
    pub(crate) fn next_task_id(&self) -> u64 {
        self.tasks
            .iter()
            .map(|task| task.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
    }
}

/// Read the persisted document, or an empty one when it cannot be read.
///
/// A document whose `schema_version` is not the current one is refused and
/// treated as empty, the same strict-version rule the configuration follows:
/// this crate cannot know what an unknown version meant, and adopting one would
/// overwrite a document a future build wrote on purpose.
pub(crate) fn read_document(path: &Path) -> Document {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Document::default(),
        Err(_) => return Document::default(),
    };
    let document: Document = match serde_json::from_slice(&bytes) {
        Ok(document) => document,
        Err(_) => return Document::default(),
    };
    if document.schema_version != POMODORO_TASKS_SCHEMA_VERSION {
        return Document::default();
    }
    document.normalized()
}

/// Replace the document atomically, creating the parent directory when the
/// first write runs before anything else has created the environment root.
pub(crate) fn write_document(path: &Path, document: &Document) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        create_private_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec(document).expect("the pomodoro document serializes");
    write_private_atomic(path, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn an_absent_document_is_empty_and_current() {
        let document = read_document(&PathBuf::from("/nowhere/pomodoro-tasks.json"));
        assert_eq!(document, Document::default());
        assert_eq!(document.schema_version, POMODORO_TASKS_SCHEMA_VERSION);
        assert_eq!(document.focus_minutes, crate::DEFAULT_FOCUS_MINUTES);
    }

    #[test]
    fn a_document_from_another_version_is_refused() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let path = directory.path().join("pomodoro-tasks.json");
        fs::write(
            &path,
            br#"{"schema_version":99,"tasks":[{"id":1,"name":"kept","estimated_pomodoros":2,"completed_pomodoros":1,"created_at_unix":10}]}"#,
        )
        .expect("write the future document");
        assert_eq!(read_document(&path), Document::default());
    }

    #[test]
    fn a_document_without_the_newer_fields_still_loads() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let path = directory.path().join("pomodoro-tasks.json");
        // Every field except the version and the id/name/estimates is absent,
        // which is the shape the first build wrote.
        fs::write(
            &path,
            br#"{"schema_version":1,"tasks":[{"id":4,"name":"old","estimated_pomodoros":3,"completed_pomodoros":0,"created_at_unix":7}]}"#,
        )
        .expect("write the older document");
        let document = read_document(&path);
        assert_eq!(document.tasks.len(), 1);
        assert_eq!(document.tasks[0].status, TaskStatus::in_progress);
        assert_eq!(document.tasks[0].completed_at_unix, None);
        assert!(document.tasks[0].completion_timestamps.is_empty());
        assert_eq!(document.focus_minutes, crate::DEFAULT_FOCUS_MINUTES);
    }

    #[test]
    fn a_document_round_trips_through_the_atomic_write() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let path = directory.path().join("nested").join("pomodoro-tasks.json");
        let mut document = Document::default();
        document
            .tasks
            .push(TaskRecord::new(1, "write notes".into(), 4, 1_700_000_000));
        document.tasks[0].completed_pomodoros = 2;
        document.tasks[0].completion_timestamps = vec![1_700_000_060, 1_700_003_060];
        document.total_completions = 2;
        write_document(&path, &document).expect("write the document");
        assert_eq!(read_document(&path), document);
    }

    #[test]
    fn the_next_id_follows_the_highest_one_stored() {
        let mut document = Document::default();
        document.tasks.push(TaskRecord::new(9, "kept".into(), 1, 0));
        assert_eq!(document.next_task_id(), 10);
        document.tasks.clear();
        assert_eq!(document.next_task_id(), 1);
    }

    #[test]
    fn unfinishing_a_task_clears_the_timestamp() {
        let mut task = TaskRecord::new(1, "done".into(), 1, 0);
        task.set_done(true, 42);
        assert_eq!(task.completed_at_unix, Some(42));
        task.set_done(false, 99);
        assert_eq!(task.status, TaskStatus::in_progress);
        assert_eq!(task.completed_at_unix, None);
    }

    #[test]
    fn finishing_a_task_dates_it_to_its_last_session_not_the_click() {
        let mut task = TaskRecord::new(1, "done".into(), 3, 0);
        task.completion_timestamps = vec![500, 900, 1_400];
        // The checkbox is clicked long after the work happened.
        task.set_done(true, 9_000);
        assert_eq!(task.completed_at_unix, Some(1_400));

        // A task finished without any session keeps the click as its date.
        let mut untouched = TaskRecord::new(2, "instant".into(), 1, 0);
        untouched.set_done(true, 9_000);
        assert_eq!(untouched.completed_at_unix, Some(9_000));
    }
}
