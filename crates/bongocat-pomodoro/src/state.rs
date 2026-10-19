//! The state machine: what a command does to the tasks and the live session.
//!
//! [`PomodoroState`] is a plain value with no clock and no IO. Every transition
//! takes the current wall time as an argument, and the countdown is advanced by
//! handing the state an explicit elapsed [`Duration`]. That is what lets the
//! tests cover a twenty-five minute session in microseconds and still cover the
//! case that matters most — a wake-up that arrives two seconds late — without
//! the worker having to be involved.

use std::{collections::BTreeMap, time::Duration};

use crate::{
    CalendarDay, CalendarDayTotal, CalendarTaskActivity, Cultivation, CultivationView, EquipSlot,
    MAX_FOCUS_MINUTES, MAX_TASK_NAME_CHARS, MIN_FOCUS_MINUTES,
    document::{Document, TaskRecord, TaskStatus, read_document},
};

/// Whether a task is still being worked on or has been finished.
pub use crate::document::TaskStatus as PomodoroTaskStatus;

/// Everything a settings surface or a menu needs about one task.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PomodoroTaskView {
    pub id: u64,
    pub name: String,
    pub estimated_pomodoros: u32,
    pub completed_pomodoros: u32,
    pub status: PomodoroTaskStatus,
    pub created_at_unix: i64,
    /// When the task was last marked finished.
    pub completed_at_unix: Option<i64>,
    /// Whether this task owns the running or paused session.
    pub running: bool,
}

/// The live session, as a surface renders it.
///
/// `remaining_seconds` is meaningful in both the running and the paused case: a
/// paused countdown still has time left, and hiding it would make a paused
/// session indistinguishable from an abandoned one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimerView {
    pub task_id: u64,
    pub state: TimerState,
    pub remaining_seconds: u32,
    pub total_seconds: u32,
}

/// The totals a month view shows next to the grid.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CalendarTotals {
    pub pomodoros: u32,
    pub tasks: u32,
}

/// Whether there is a session, and whether it is counting down.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TimerState {
    /// No session at all.
    #[default]
    Idle,
    /// Counting down towards zero.
    Running,
    /// Frozen at [`TimerState::running_seconds`].
    Paused,
}

impl TimerState {
    /// Whether the countdown is moving.
    pub const fn is_running(self) -> bool {
        matches!(self, Self::Running)
    }
}

/// The one live session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Timer {
    Idle,
    Running {
        task_id: u64,
        remaining_seconds: u32,
    },
    Paused {
        task_id: u64,
        remaining_seconds: u32,
    },
}

impl Timer {
    /// The task this session belongs to, if there is one.
    pub fn task_id(&self) -> Option<u64> {
        match *self {
            Self::Idle => None,
            Self::Running { task_id, .. } | Self::Paused { task_id, .. } => Some(task_id),
        }
    }

    /// The time left, in either the running or the paused case.
    pub fn remaining_seconds(&self) -> u32 {
        match *self {
            Self::Idle => 0,
            Self::Running {
                remaining_seconds, ..
            }
            | Self::Paused {
                remaining_seconds, ..
            } => remaining_seconds,
        }
    }

    /// The state a surface renders.
    pub fn state(&self) -> TimerState {
        match *self {
            Self::Idle => TimerState::Idle,
            Self::Running { .. } => TimerState::Running,
            Self::Paused { .. } => TimerState::Paused,
        }
    }

    /// How long the session runs when it starts.
    pub fn view(&self, total_seconds: u32) -> Option<TimerView> {
        match *self {
            Self::Idle => None,
            Self::Running {
                task_id,
                remaining_seconds,
            }
            | Self::Paused {
                task_id,
                remaining_seconds,
            } => Some(TimerView {
                task_id,
                state: self.state(),
                remaining_seconds,
                total_seconds,
            }),
        }
    }
}

/// Everything a surface reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PomodoroSnapshot {
    pub revision: u64,
    /// Whether the worker behind this snapshot is running.
    ///
    /// A caller renders an empty timer rather than a broken one when the
    /// worker is gone, which is why the state travels with the data rather than
    /// only with the diagnostics.
    pub worker: crate::service::PomodoroWorkerState,
    pub tasks: Vec<PomodoroTaskView>,
    pub timer: Option<TimerView>,
    pub focus_minutes: u32,
    pub last_completion: Option<PomodoroCompletion>,
    /// The cultivation ladder: the realm, the attributes, the gear, the garden
    /// and the guardian. It travels with the snapshot so a surface that draws the
    /// timer also draws what the sessions have bought.
    pub cultivation: CultivationView,
}

/// One finished session, and the identity a surface uses to react once.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PomodoroCompletion {
    pub sequence: u64,
    pub task_id: u64,
    pub at_unix: i64,
}

/// What a caller asks the state machine to do.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PomodoroCommand {
    /// Create a task with a display name and how many focus sessions the user
    /// expects it to take.
    CreateTask {
        name: String,
        estimated_pomodoros: u32,
    },
    /// Start a fresh focus session for a task.
    StartTask { task_id: u64 },
    /// Freeze the remaining time. Only meaningful while running.
    Pause,
    /// Continue a paused countdown from the time it stopped at.
    Resume,
    /// End the session without counting it. Only meaningful while running or
    /// paused.
    Abandon,
    /// Mark a task finished or unfinished. Both directions are one variant:
    /// a checkbox is a position, not two buttons.
    SetTaskDone { task_id: u64, done: bool },
    /// Change how long one focus session lasts.
    SetFocusMinutes { minutes: u32 },
    /// Rewrite a task's name and how many focus sessions it expects. The
    /// sessions it has already finished are the history and never move: an
    /// edit is about the plan, not about the work already done.
    UpdateTask {
        task_id: u64,
        name: String,
        estimated_pomodoros: u32,
    },
    /// Remove a task, its progress and its history.
    DeleteTask { task_id: u64 },
    /// Plant one owned seed in the first empty plot.
    PlantSeed {
        attribute: crate::Attribute,
        tier: u8,
    },
    /// Pick a ripe crop out of a plot.
    HarvestPlot { plot: usize },
    /// Wear one owned item in its slot.
    EquipItem { item_id: u64 },
    /// Take whatever is worn in a slot off.
    UnequipSlot { slot: EquipSlot },
    /// Fight the guardian standing at the end of the current realm.
    Breakthrough,
}

impl PomodoroCommand {
    /// Whether the command would change anything at all.
    ///
    /// A command that would not is still applied and still bumps the revision,
    /// because a caller that asked for something deserves an answer; this is
    /// only used to decide whether the document has to be written back.
    fn changes_the_document(&self) -> bool {
        match self {
            Self::StartTask { .. } | Self::Pause | Self::Resume | Self::Abandon => false,
            Self::CreateTask { .. } | Self::SetTaskDone { .. } | Self::SetFocusMinutes { .. } => {
                true
            }
            Self::UpdateTask { .. } | Self::DeleteTask { .. } => true,
            // Every cultivation command is a durable move: a planted seed, a
            // harvest, a worn item and a breakthrough all outlive the session.
            Self::PlantSeed { .. }
            | Self::HarvestPlot { .. }
            | Self::EquipItem { .. }
            | Self::UnequipSlot { .. }
            | Self::Breakthrough => true,
        }
    }
}

/// The whole domain state, and every transition over it.
#[derive(Clone, Debug)]
pub struct PomodoroState {
    document: Document,
    timer: Timer,
    /// The session length in seconds, so a running timer and the document
    /// cannot disagree about how long a session runs.
    session_seconds: u32,
    /// The count of sessions that have finished since this state was loaded.
    completions: u64,
    last_completion: Option<PomodoroCompletion>,
    /// The ladder the sessions feed, kept beside the document in the shape it
    /// normalizes to. [`PomodoroState::document`] mirrors it back, so the one
    /// atomic write carries both halves.
    cultivation: Cultivation,
    /// The UTC offset that decides which local day a completion lands on. It is
    /// the caller's offset, because reading the machine's needs a platform call
    /// this crate does not make.
    utc_offset_seconds: i32,
}

impl PomodoroState {
    /// An empty state, as a fresh install sees it.
    pub fn empty() -> Self {
        Self::empty_at(0)
    }

    /// An empty state for a caller with a UTC offset.
    pub fn empty_at(utc_offset_seconds: i32) -> Self {
        Self::from_document(Document::default(), utc_offset_seconds)
    }

    /// The state a stored document produces.
    ///
    /// The timer starts idle whatever the document says, because the document
    /// cannot hold one: a countdown restored on a later launch would be a
    /// deadline that passed while the application was not running.
    pub fn load(path: &std::path::Path) -> Self {
        Self::load_at(path, 0)
    }

    /// The state a stored document produces, for a caller that knows its offset.
    pub fn load_at(path: &std::path::Path, utc_offset_seconds: i32) -> Self {
        Self::from_document(read_document(path), utc_offset_seconds)
    }

    fn from_document(document: Document, utc_offset_seconds: i32) -> Self {
        let focus_minutes = clamp_focus_minutes(document.focus_minutes);
        let cultivation = Cultivation::new(document.cultivation.clone());
        Self {
            session_seconds: focus_minutes * 60,
            document: Document {
                focus_minutes,
                cultivation: cultivation.document().clone(),
                ..document
            },
            timer: Timer::Idle,
            completions: 0,
            last_completion: None,
            cultivation,
            utc_offset_seconds,
        }
    }

    /// The document as it would be written back.
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// The focus session length in minutes.
    pub fn focus_minutes(&self) -> u32 {
        self.document.focus_minutes
    }

    /// The live session.
    pub fn timer(&self) -> Timer {
        self.timer
    }

    /// Whether the document changed since it was last written.
    ///
    /// The worker asks this after every command so a pause or a resume — which
    /// touch nothing on disk — does not cost a write.
    pub fn document_is_dirty(&self, written: &Document) -> bool {
        &self.document != written
    }

    /// Apply one command.
    pub fn apply(&mut self, command: PomodoroCommand, now_unix: i64) {
        match command {
            PomodoroCommand::CreateTask {
                name,
                estimated_pomodoros,
            } => self.create_task(name, estimated_pomodoros, now_unix),
            PomodoroCommand::StartTask { task_id } => self.start_task(task_id),
            PomodoroCommand::Pause => self.pause(),
            PomodoroCommand::Resume => self.resume(),
            PomodoroCommand::Abandon => self.abandon(),
            PomodoroCommand::SetTaskDone { task_id, done } => {
                self.set_task_done(task_id, done, now_unix)
            }
            PomodoroCommand::SetFocusMinutes { minutes } => self.set_focus_minutes(minutes),
            PomodoroCommand::UpdateTask {
                task_id,
                name,
                estimated_pomodoros,
            } => self.update_task(task_id, name, estimated_pomodoros),
            PomodoroCommand::DeleteTask { task_id } => self.delete_task(task_id),
            PomodoroCommand::PlantSeed { attribute, tier } => {
                self.cultivation.plant(attribute, tier, now_unix);
            }
            PomodoroCommand::HarvestPlot { plot } => {
                self.cultivation.harvest(plot, now_unix);
            }
            PomodoroCommand::EquipItem { item_id } => {
                self.cultivation.equip(item_id);
            }
            PomodoroCommand::UnequipSlot { slot } => {
                self.cultivation.unequip(slot);
            }
            PomodoroCommand::Breakthrough => {
                self.cultivation.breakthrough();
            }
        }
        self.sync_cultivation();
    }

    /// Mirror the ladder back into the document, so the one write that follows
    /// a command covers it.
    fn sync_cultivation(&mut self) {
        if &self.document.cultivation != self.cultivation.document() {
            self.document.cultivation = self.cultivation.document().clone();
        }
    }

    /// The cultivation ladder, for a caller that wants it without the snapshot.
    pub fn cultivation(&self) -> &Cultivation {
        &self.cultivation
    }

    /// Charge every whole local day without a session that has not been charged
    /// yet. Returns whether anything was taken.
    ///
    /// The worker calls this on every tick so a machine left running for a week
    /// charges the missed days as they end, and a machine launched after a week
    /// away charges them on the first tick after loading.
    pub fn apply_absence(&mut self, now_unix: i64) -> bool {
        let today = CalendarDay::from_unix(now_unix, self.utc_offset_seconds);
        if self.cultivation.apply_absence(today).is_none() {
            return false;
        }
        self.sync_cultivation();
        true
    }

    /// Whether the command would write anything to the document.
    pub fn command_writes_the_document(command: &PomodoroCommand) -> bool {
        command.changes_the_document()
    }

    fn create_task(&mut self, name: String, estimated_pomodoros: u32, now_unix: i64) {
        let name = sanitize_task_name(&name);
        if name.is_empty() {
            // A row the user cannot read is worse than no row at all, so an
            // unnamed task is refused rather than created as a blank line.
            return;
        }
        let id = self.document.next_task_id();
        self.document
            .tasks
            .push(TaskRecord::new(id, name, estimated_pomodoros, now_unix));
    }

    fn start_task(&mut self, task_id: u64) {
        if !self.document.tasks.iter().any(|task| task.id == task_id) {
            return;
        }
        // Starting over an existing session abandons it. The abandoned work is
        // deliberately not counted: the user pointed at a different task, and
        // counting a session nobody finished would make the history a record of
        // intent rather than of work.
        self.timer = Timer::Running {
            task_id,
            remaining_seconds: self.session_seconds,
        };
    }

    fn pause(&mut self) {
        let Timer::Running {
            task_id,
            remaining_seconds,
        } = self.timer
        else {
            return;
        };
        self.timer = Timer::Paused {
            task_id,
            remaining_seconds,
        };
    }

    fn resume(&mut self) {
        let Timer::Paused {
            task_id,
            remaining_seconds,
        } = self.timer
        else {
            return;
        };
        // A session paused with nothing left resumes as finished on the next
        // tick rather than counting a whole session from zero.
        self.timer = Timer::Running {
            task_id,
            remaining_seconds,
        };
    }

    fn abandon(&mut self) {
        if self.timer.task_id().is_some() {
            self.timer = Timer::Idle;
        }
    }

    fn set_task_done(&mut self, task_id: u64, done: bool, now_unix: i64) {
        let Some(task) = self
            .document
            .tasks
            .iter_mut()
            .find(|task| task.id == task_id)
        else {
            return;
        };
        task.set_done(done, now_unix);
    }

    fn set_focus_minutes(&mut self, minutes: u32) {
        let minutes = clamp_focus_minutes(minutes);
        self.document.focus_minutes = minutes;
        self.session_seconds = minutes.saturating_mul(60);
    }

    /// Rewrite a task's name and its expected session count.
    ///
    /// The name follows the rule creation follows: a field the user cannot
    /// read is refused rather than stored, so an edit that ends on an empty
    /// name leaves the row exactly as it was. The finished sessions are
    /// history and never move — only the plan is rewritten.
    fn update_task(&mut self, task_id: u64, name: String, estimated_pomodoros: u32) {
        let Some(task) = self
            .document
            .tasks
            .iter_mut()
            .find(|task| task.id == task_id)
        else {
            return;
        };
        let name = sanitize_task_name(&name);
        if name.is_empty() {
            return;
        }
        task.name = name;
        task.estimated_pomodoros = estimated_pomodoros;
    }

    /// Remove a task and everything recorded against it.
    ///
    /// A session running for the deleted task is abandoned with it: the
    /// timer holds only an id, and a countdown belonging to a task the
    /// document no longer has would complete into nothing. The completion
    /// timestamps go with the task, so the month calendar loses its
    /// sessions in the same write rather than keeping days that name a
    /// task nobody can find.
    fn delete_task(&mut self, task_id: u64) {
        if self.timer.task_id() == Some(task_id) {
            self.timer = Timer::Idle;
        }
        self.document.tasks.retain(|task| task.id != task_id);
    }

    /// Fold elapsed time into a running countdown.
    ///
    /// Returns the completion when the session reached zero. Elapsed time is
    /// taken as one amount rather than subtracted once per wake-up, so a wake-up
    /// that arrives late shortens the remaining time instead of making the
    /// countdown run slow.
    pub fn advance(&mut self, elapsed: Duration, now_unix: i64) -> Option<PomodoroCompletion> {
        let Timer::Running {
            task_id,
            remaining_seconds,
        } = self.timer
        else {
            return None;
        };
        let elapsed_seconds = u32::try_from(elapsed.as_secs()).unwrap_or(u32::MAX);
        let remaining_seconds = remaining_seconds.saturating_sub(elapsed_seconds);
        if remaining_seconds > 0 {
            self.timer = Timer::Running {
                task_id,
                remaining_seconds,
            };
            return None;
        }
        let completion = self.complete(task_id, now_unix);
        if completion.is_some() {
            self.record_cultivation_session(now_unix);
        }
        completion
    }

    /// Pay the ladder for one finished session: its growth, whatever absence was
    /// owed before it, and the one roll a session makes.
    fn record_cultivation_session(&mut self, now_unix: i64) {
        let today = CalendarDay::from_unix(now_unix, self.utc_offset_seconds);
        self.cultivation
            .on_session_completed(now_unix, today, self.document.focus_minutes);
        self.sync_cultivation();
    }

    /// The session length in seconds.
    pub fn session_seconds(&self) -> u32 {
        self.session_seconds
    }

    /// Count a finished session and end it.
    fn complete(&mut self, task_id: u64, now_unix: i64) -> Option<PomodoroCompletion> {
        if let Some(task) = self
            .document
            .tasks
            .iter_mut()
            .find(|task| task.id == task_id)
        {
            task.completed_pomodoros = task.completed_pomodoros.saturating_add(1);
            task.completion_timestamps.push(now_unix);
            // The task's completion date follows its last session: a task
            // ticked off before one more session ran on it is dated by that
            // session, the same rule the checkbox path follows.
            if task.status == TaskStatus::done {
                task.completed_at_unix = Some(now_unix);
            }
        }
        self.timer = Timer::Idle;
        self.completions = self.completions.saturating_add(1);
        let completion = PomodoroCompletion {
            sequence: self.completions,
            task_id,
            at_unix: now_unix,
        };
        self.last_completion = Some(completion);
        Some(completion)
    }

    /// How many sessions have finished since this state was loaded.
    pub fn completion_count(&self) -> u64 {
        self.completions
    }

    /// Read the snapshot a surface renders.
    ///
    /// The worker state travels with the data rather than only with the
    /// diagnostics: a caller that renders a timer needs to know whether there is
    /// a worker behind it in the same read it takes the task list from.
    pub fn snapshot(&self, revision: u64) -> PomodoroSnapshot {
        let running_task = self.timer.task_id();
        PomodoroSnapshot {
            revision,
            worker: crate::service::PomodoroWorkerState::Ready,
            tasks: self
                .document
                .tasks
                .iter()
                .map(|task| task.view(running_task == Some(task.id)))
                .collect(),
            timer: self.timer.view(self.session_seconds),
            focus_minutes: self.document.focus_minutes,
            last_completion: self.last_completion,
            cultivation: self.cultivation.view(),
        }
    }

    /// The session count and task count per local day, for a month view.
    pub fn day_totals(&self, utc_offset_seconds: i32) -> BTreeMap<CalendarDay, CalendarDayTotal> {
        let mut days: BTreeMap<CalendarDay, BTreeMap<u64, u32>> = BTreeMap::new();
        for task in &self.document.tasks {
            for at_unix in &task.completion_timestamps {
                *days
                    .entry(CalendarDay::from_unix(*at_unix, utc_offset_seconds))
                    .or_default()
                    .entry(task.id)
                    .or_insert(0) += 1;
            }
        }
        days.into_iter()
            .map(|(day, tasks)| {
                let total = CalendarDayTotal {
                    pomodoros: tasks.values().copied().sum(),
                    // The map is keyed by task id, so its length is the number
                    // of distinct tasks that contributed to this day.
                    tasks: u32::try_from(tasks.len()).unwrap_or(u32::MAX),
                };
                (day, total)
            })
            .collect()
    }

    /// The tasks behind each day's finished sessions, for the calendar cells
    /// that name what a day was spent on.
    ///
    /// The same local-day bucketing as [`Self::day_totals`], one entry per task
    /// per day with its session count; within a day the tasks are ordered by
    /// sessions finished, then by name, so a cell reads the same way every time.
    pub fn day_activities(
        &self,
        utc_offset_seconds: i32,
    ) -> BTreeMap<CalendarDay, Vec<CalendarTaskActivity>> {
        let mut days: BTreeMap<CalendarDay, BTreeMap<u64, CalendarTaskActivity>> = BTreeMap::new();
        for task in &self.document.tasks {
            for at_unix in &task.completion_timestamps {
                let day = CalendarDay::from_unix(*at_unix, utc_offset_seconds);
                let entry = days
                    .entry(day)
                    .or_default()
                    .entry(task.id)
                    .or_insert_with(|| CalendarTaskActivity {
                        task_name: task.name.clone(),
                        pomodoros: 0,
                    });
                entry.pomodoros += 1;
            }
        }
        days.into_iter()
            .map(|(day, tasks)| {
                let mut activities: Vec<CalendarTaskActivity> = tasks.into_values().collect();
                activities.sort_by(|a, b| {
                    b.pomodoros
                        .cmp(&a.pomodoros)
                        .then_with(|| a.task_name.cmp(&b.task_name))
                });
                (day, activities)
            })
            .collect()
    }
}

impl PomodoroState {
    /// Record a completion without driving the countdown.
    ///
    /// Only the tests need this: a month view is read for days that already
    /// happened, and running a whole countdown per historical session would make
    /// the aggregation tests slow for no extra coverage.
    #[cfg(test)]
    pub(crate) fn record_completion_for_test(&mut self, task_id: u64, at_unix: i64) {
        self.document
            .tasks
            .iter_mut()
            .find(|task| task.id == task_id)
            .expect("a task to complete")
            .completion_timestamps
            .push(at_unix);
    }
}

impl TaskRecord {
    /// The read model one task row renders from.
    pub(crate) fn view(&self, running: bool) -> PomodoroTaskView {
        PomodoroTaskView {
            id: self.id,
            name: self.name.clone(),
            estimated_pomodoros: self.estimated_pomodoros,
            completed_pomodoros: self.completed_pomodoros,
            status: self.status,
            created_at_unix: self.created_at_unix,
            completed_at_unix: self.completed_at_unix,
            running,
        }
    }
}

/// Clamp a requested session length into the supported range.
pub fn clamp_focus_minutes(minutes: u32) -> u32 {
    minutes.clamp(MIN_FOCUS_MINUTES, MAX_FOCUS_MINUTES)
}

/// The display name a task is stored under.
///
/// Control characters are dropped rather than escaped so a name pasted from a
/// terminal cannot break the row's single-line layout, and the result is
/// trimmed and bounded.
pub fn sanitize_task_name(value: &str) -> String {
    let filtered: String = value
        .chars()
        .filter(|character| !character.is_control() && !is_invisible(*character))
        .collect();
    filtered
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_TASK_NAME_CHARS)
        .collect()
}

/// Whether a character is one a user never meant to type into a task name.
///
/// Zero-width characters are the case that matters: they survive a paste and a
/// trim, and a name that renders as `spacedname` while being stored as
/// `spaced<ZWSP>name` is a row that cannot be searched for by what it shows.
fn is_invisible(character: char) -> bool {
    // The general categories of format and unassigned code points, which is
    // where the zero-width family lives. They are checked by value because the
    // std library exposes no category predicate.
    matches!(character as u32,
        0x00AD                                  // soft hyphen
        | 0x200B..=0x200F                       // zero-width space through RTL mark
        | 0x202A..=0x202E                       // bidi embedding controls
        | 0x2060..=0x2064                       // word joiner and invisible operators
        | 0x2066..=0x206F                       // bidi isolates and deprecated formats
        | 0xFEFF                                // zero-width no-break space
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DEFAULT_FOCUS_MINUTES;

    #[test]
    fn a_fresh_state_has_no_tasks_and_no_session() {
        let state = PomodoroState::empty();
        let snapshot = state.snapshot(1);
        assert!(snapshot.tasks.is_empty());
        assert_eq!(snapshot.timer, None);
        assert_eq!(snapshot.focus_minutes, DEFAULT_FOCUS_MINUTES);
    }

    #[test]
    fn a_created_task_is_in_progress_with_nothing_completed() {
        let mut state = PomodoroState::empty();
        state.apply(
            PomodoroCommand::CreateTask {
                name: "write the report".into(),
                estimated_pomodoros: 4,
            },
            1_000,
        );
        let snapshot = state.snapshot(1);
        assert_eq!(snapshot.tasks.len(), 1);
        assert_eq!(snapshot.tasks[0].name, "write the report");
        assert_eq!(snapshot.tasks[0].estimated_pomodoros, 4);
        assert_eq!(snapshot.tasks[0].completed_pomodoros, 0);
        assert_eq!(snapshot.tasks[0].status, PomodoroTaskStatus::in_progress);
        assert_eq!(snapshot.tasks[0].id, 1);
    }

    #[test]
    fn an_unnamed_task_is_refused() {
        let mut state = PomodoroState::empty();
        state.apply(
            PomodoroCommand::CreateTask {
                name: "   \t\n ".into(),
                estimated_pomodoros: 1,
            },
            1_000,
        );
        assert!(state.snapshot(1).tasks.is_empty());
    }

    #[test]
    fn a_started_task_countdown_holds_the_session_length() {
        let mut state = PomodoroState::empty();
        state.apply(
            PomodoroCommand::CreateTask {
                name: "focus".into(),
                estimated_pomodoros: 1,
            },
            1_000,
        );
        state.apply(PomodoroCommand::StartTask { task_id: 1 }, 1_000);
        let snapshot = state.snapshot(1);
        let timer = snapshot.timer.expect("a running session");
        assert_eq!(timer.remaining_seconds, 25 * 60);
        assert_eq!(timer.total_seconds, 25 * 60);
        assert!(snapshot.tasks[0].running);
    }

    #[test]
    fn pausing_freezes_the_remaining_time_and_resuming_continues_it() {
        let mut state = PomodoroState::empty();
        state.apply(
            PomodoroCommand::CreateTask {
                name: "focus".into(),
                estimated_pomodoros: 1,
            },
            1_000,
        );
        state.apply(PomodoroCommand::StartTask { task_id: 1 }, 1_000);
        assert!(state.advance(Duration::from_secs(30), 1_030).is_none());
        state.apply(PomodoroCommand::Pause, 1_030);
        let paused = state.snapshot(2).timer.expect("a paused session");
        assert_eq!(paused.state, TimerState::Paused);
        assert_eq!(paused.remaining_seconds, 25 * 60 - 30);
        // Time passing while paused must not move the countdown.
        assert!(state.advance(Duration::from_secs(600), 1_630).is_none());
        assert_eq!(
            state
                .snapshot(3)
                .timer
                .expect("still paused")
                .remaining_seconds,
            25 * 60 - 30
        );
        state.apply(PomodoroCommand::Resume, 1_630);
        assert_eq!(
            state
                .snapshot(4)
                .timer
                .expect("running again")
                .remaining_seconds,
            25 * 60 - 30
        );
    }

    #[test]
    fn abandoning_ends_the_session_without_counting_it() {
        let mut state = PomodoroState::empty();
        state.apply(
            PomodoroCommand::CreateTask {
                name: "focus".into(),
                estimated_pomodoros: 2,
            },
            1_000,
        );
        state.apply(PomodoroCommand::StartTask { task_id: 1 }, 1_000);
        state.advance(Duration::from_secs(60), 1_060);
        state.apply(PomodoroCommand::Abandon, 1_060);
        let snapshot = state.snapshot(2);
        assert_eq!(snapshot.timer, None);
        assert_eq!(snapshot.tasks[0].completed_pomodoros, 0);
        // The abandoned session left no completion behind, so the month view
        // has nothing to show for it.
        assert!(state.day_totals(0).is_empty());
    }

    #[test]
    fn a_finished_session_counts_once_and_records_its_time() {
        let mut state = PomodoroState::empty();
        state.apply(
            PomodoroCommand::CreateTask {
                name: "focus".into(),
                estimated_pomodoros: 2,
            },
            1_000,
        );
        state.apply(PomodoroCommand::StartTask { task_id: 1 }, 1_000);
        let completion = state
            .advance(Duration::from_secs(25 * 60), 1_500)
            .expect("the session finished");
        assert_eq!(completion.sequence, 1);
        assert_eq!(completion.task_id, 1);
        assert_eq!(completion.at_unix, 1_500);
        let snapshot = state.snapshot(2);
        assert_eq!(snapshot.tasks[0].completed_pomodoros, 1);
        assert_eq!(snapshot.timer, None);
        assert_eq!(snapshot.last_completion, Some(completion));
    }

    #[test]
    fn a_late_wake_up_shortens_the_countdown_rather_than_slowing_it() {
        let mut state = PomodoroState::empty();
        state.apply(
            PomodoroCommand::CreateTask {
                name: "focus".into(),
                estimated_pomodoros: 1,
            },
            1_000,
        );
        state.apply(PomodoroCommand::StartTask { task_id: 1 }, 1_000);
        // A wake-up five seconds late must cost five seconds of countdown.
        assert!(state.advance(Duration::from_secs(6), 1_006).is_none());
        assert_eq!(
            state
                .snapshot(2)
                .timer
                .expect("still running")
                .remaining_seconds,
            25 * 60 - 6
        );
        // And the overshoot on the final tick must still complete the session.
        state.advance(Duration::from_secs(25 * 60 - 1), 1_500);
        assert_eq!(state.snapshot(3).timer, None);
    }

    #[test]
    fn two_completions_in_the_same_second_stay_distinguishable() {
        let mut state = PomodoroState::empty();
        state.apply(
            PomodoroCommand::CreateTask {
                name: "focus".into(),
                estimated_pomodoros: 3,
            },
            1_000,
        );
        let mut sequences = Vec::new();
        for _ in 0..2 {
            state.apply(PomodoroCommand::StartTask { task_id: 1 }, 1_500);
            let completion = state
                .advance(Duration::from_secs(25 * 60), 1_500)
                .expect("the session finished");
            sequences.push(completion.sequence);
        }
        assert_eq!(sequences, vec![1, 2]);
        assert_eq!(state.snapshot(1).tasks[0].completed_pomodoros, 2);
    }

    #[test]
    fn a_task_can_be_finished_and_unfinished_again() {
        let mut state = PomodoroState::empty();
        state.apply(
            PomodoroCommand::CreateTask {
                name: "focus".into(),
                estimated_pomodoros: 1,
            },
            1_000,
        );
        state.apply(
            PomodoroCommand::SetTaskDone {
                task_id: 1,
                done: true,
            },
            2_000,
        );
        let finished = state.snapshot(2);
        assert_eq!(finished.tasks[0].status, PomodoroTaskStatus::done);
        assert_eq!(finished.tasks[0].completed_at_unix, Some(2_000));
        state.apply(
            PomodoroCommand::SetTaskDone {
                task_id: 1,
                done: false,
            },
            3_000,
        );
        let reopened = state.snapshot(3);
        assert_eq!(reopened.tasks[0].status, PomodoroTaskStatus::in_progress);
        assert_eq!(reopened.tasks[0].completed_at_unix, None);
        // Unfinishing a task does not erase the sessions it completed.
        assert_eq!(reopened.tasks[0].completed_pomodoros, 0);
    }

    #[test]
    fn checking_a_task_off_dates_it_to_its_last_session_not_the_click() {
        let mut state = PomodoroState::empty();
        state.apply(
            PomodoroCommand::CreateTask {
                name: "focus".into(),
                estimated_pomodoros: 2,
            },
            1_000,
        );
        // One session finishes at 2_000; the user only confirms the task is
        // finished much later, at 9_000.
        state.apply(PomodoroCommand::StartTask { task_id: 1 }, 1_500);
        state
            .advance(Duration::from_secs(25 * 60), 2_000)
            .expect("the session finished");
        state.apply(
            PomodoroCommand::SetTaskDone {
                task_id: 1,
                done: true,
            },
            9_000,
        );
        let finished = state.snapshot(2);
        assert_eq!(finished.tasks[0].status, PomodoroTaskStatus::done);
        assert_eq!(
            finished.tasks[0].completed_at_unix,
            Some(2_000),
            "the completion date is the day the work happened, not the day the box was ticked"
        );
    }

    #[test]
    fn a_session_finishing_on_a_done_task_moves_its_completion_date() {
        let mut state = PomodoroState::empty();
        state.apply(
            PomodoroCommand::CreateTask {
                name: "focus".into(),
                estimated_pomodoros: 3,
            },
            1_000,
        );
        state.apply(
            PomodoroCommand::SetTaskDone {
                task_id: 1,
                done: true,
            },
            1_200,
        );
        // One more session runs against the finished task; its completion date
        // follows the session rather than staying at the checkbox click.
        state.apply(PomodoroCommand::StartTask { task_id: 1 }, 1_500);
        state
            .advance(Duration::from_secs(25 * 60), 4_000)
            .expect("the session finished");
        assert_eq!(state.snapshot(1).tasks[0].completed_at_unix, Some(4_000));
    }

    #[test]
    fn starting_another_task_replaces_the_running_session_without_counting_it() {
        let mut state = PomodoroState::empty();
        for name in ["first", "second"] {
            state.apply(
                PomodoroCommand::CreateTask {
                    name: name.into(),
                    estimated_pomodoros: 1,
                },
                1_000,
            );
        }
        state.apply(PomodoroCommand::StartTask { task_id: 1 }, 1_000);
        state.advance(Duration::from_secs(60), 1_060);
        state.apply(PomodoroCommand::StartTask { task_id: 2 }, 1_060);
        let snapshot = state.snapshot(2);
        assert_eq!(snapshot.timer.expect("the new session").task_id, 2);
        assert_eq!(snapshot.tasks[0].completed_pomodoros, 0);
        assert!(!snapshot.tasks[0].running);
        assert!(snapshot.tasks[1].running);
    }

    #[test]
    fn commands_for_a_task_that_does_not_exist_are_ignored() {
        let mut state = PomodoroState::empty();
        state.apply(PomodoroCommand::StartTask { task_id: 99 }, 1_000);
        state.apply(
            PomodoroCommand::SetTaskDone {
                task_id: 99,
                done: true,
            },
            1_000,
        );
        assert_eq!(state.snapshot(1).timer, None);
        assert!(state.snapshot(1).tasks.is_empty());
    }

    #[test]
    fn pause_and_abandon_do_nothing_without_a_session() {
        let mut state = PomodoroState::empty();
        state.apply(PomodoroCommand::Pause, 1_000);
        state.apply(PomodoroCommand::Resume, 1_000);
        state.apply(PomodoroCommand::Abandon, 1_000);
        assert_eq!(state.snapshot(1).timer, None);
    }

    #[test]
    fn the_session_length_is_clamped_and_applies_to_the_next_session() {
        let mut state = PomodoroState::empty();
        state.apply(PomodoroCommand::SetFocusMinutes { minutes: 0 }, 1_000);
        assert_eq!(state.focus_minutes(), MIN_FOCUS_MINUTES);
        state.apply(PomodoroCommand::SetFocusMinutes { minutes: 999 }, 1_000);
        assert_eq!(state.focus_minutes(), MAX_FOCUS_MINUTES);
        state.apply(PomodoroCommand::SetFocusMinutes { minutes: 30 }, 1_000);
        state.apply(
            PomodoroCommand::CreateTask {
                name: "focus".into(),
                estimated_pomodoros: 1,
            },
            1_000,
        );
        state.apply(PomodoroCommand::StartTask { task_id: 1 }, 1_000);
        assert_eq!(
            state.snapshot(2).timer.expect("running").total_seconds,
            30 * 60
        );
    }

    #[test]
    fn only_the_document_changing_commands_ask_for_a_write() {
        assert!(!PomodoroState::command_writes_the_document(
            &PomodoroCommand::Pause
        ));
        assert!(!PomodoroState::command_writes_the_document(
            &PomodoroCommand::Resume
        ));
        assert!(!PomodoroState::command_writes_the_document(
            &PomodoroCommand::Abandon
        ));
        assert!(PomodoroState::command_writes_the_document(
            &PomodoroCommand::CreateTask {
                name: "focus".into(),
                estimated_pomodoros: 1
            }
        ));
        assert!(PomodoroState::command_writes_the_document(
            &PomodoroCommand::SetFocusMinutes { minutes: 30 }
        ));
    }

    #[test]
    fn the_day_totals_group_sessions_by_task() {
        let mut state = PomodoroState::empty();
        for name in ["a", "b"] {
            state.apply(
                PomodoroCommand::CreateTask {
                    name: name.into(),
                    estimated_pomodoros: 3,
                },
                1_000,
            );
        }
        // Task 1 finishes twice on one day; task 2 once on the same day and
        // once the next day.
        state.record_completion_for_test(1, 20_000 * 86_400);
        state.record_completion_for_test(1, 20_000 * 86_400 + 3_600);
        state.record_completion_for_test(2, 20_000 * 86_400 + 7_200);
        state.record_completion_for_test(2, 20_001 * 86_400);
        let days = state.day_totals(0);
        let first = days
            .get(&CalendarDay::from_day_number(20_000))
            .expect("the first day");
        assert_eq!(first.pomodoros, 3);
        assert_eq!(first.tasks, 2);
        let second = days
            .get(&CalendarDay::from_day_number(20_001))
            .expect("the second day");
        assert_eq!(second.pomodoros, 1);
        assert_eq!(second.tasks, 1);
    }

    /// The calendar cells name the tasks a day was spent on, with each task's
    /// session count for that day, busiest task first.
    #[test]
    fn the_day_activities_name_each_task_and_its_session_count() {
        let mut state = PomodoroState::empty();
        for name in ["a", "b"] {
            state.apply(
                PomodoroCommand::CreateTask {
                    name: name.into(),
                    estimated_pomodoros: 3,
                },
                1_000,
            );
        }
        state.record_completion_for_test(1, 20_000 * 86_400);
        state.record_completion_for_test(1, 20_000 * 86_400 + 3_600);
        state.record_completion_for_test(2, 20_000 * 86_400 + 7_200);
        let days = state.day_activities(0);
        let first = days
            .get(&CalendarDay::from_day_number(20_000))
            .expect("the first day");
        assert_eq!(first.len(), 2, "two tasks contributed to the day");
        assert_eq!(
            first[0],
            CalendarTaskActivity {
                task_name: "a".into(),
                pomodoros: 2
            },
            "the busiest task leads"
        );
        assert_eq!(first[1].task_name, "b");
        assert_eq!(first[1].pomodoros, 1);
        assert!(
            !days.contains_key(&CalendarDay::from_day_number(20_001)),
            "a day without completions has no activities"
        );
    }
}
