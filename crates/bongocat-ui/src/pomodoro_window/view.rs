//! The pomodoro view: the state behind the window, and what it asks the worker
//! to do.
//!
//! The view owns two inputs — the name and the expected session count — because
//! both belong to "a task I am about to create" rather than to anything the
//! worker stores. Everything else it reads: the countdown, the task list and the
//! completion history all come from the snapshot, so a task created in another
//! surface appears here without this window being told.
//!
//! The decisions the view makes are free functions over the snapshot rather than
//! methods, so they can be tested without a window: "which task does continue
//! mean here" and "which tasks are startable" are rules about the data, not
//! about a view.

use super::*;
use crate::SettingsLanguage;
use bongocat_pomodoro::{
    MAX_FOCUS_MINUTES, MIN_FOCUS_MINUTES, PomodoroClient, PomodoroCommand, PomodoroSnapshot,
    PomodoroTaskStatus, PomodoroTaskView,
};
use gpui_kit::component::input::InputState;
use gpui_kit::{AppContext, Entity, FocusHandle, Focusable, Window};

/// The expected-session count a new task starts with.
///
/// Four is a round number that fits an ordinary piece of work and does not
/// commit the user before they have said anything: the field is editable, so the
/// default only has to be a starting point rather than a recommendation.
pub(super) const DEFAULT_EXPECTED_SESSIONS: u32 = 4;

/// The task the "continue" row would pick up.
///
/// "Unfinished" decides it: a task the user ticked off is not something to pick
/// up for them, and a task that is already running is already the current one.
/// Choosing by that rule means the row can never disagree with what the button
/// says it does.
pub(super) fn task_to_continue(tasks: &[PomodoroTaskView]) -> Option<&PomodoroTaskView> {
    tasks
        .iter()
        .find(|task| !task.running && task.status == PomodoroTaskStatus::in_progress)
}

/// The tasks this window lists.
///
/// Finished tasks are excluded: this window answers "what am I working on now",
/// and a finished task is not an answer. A running task stays listed so its row
/// can say so.
pub(super) fn startable_tasks(tasks: &[PomodoroTaskView]) -> Vec<&PomodoroTaskView> {
    tasks
        .iter()
        .filter(|task| task.status == PomodoroTaskStatus::in_progress)
        .collect()
}

/// The countdown, when there is one to show.
///
/// `None` when the worker is gone rather than when nothing is running: the two
/// need different copy, and rendering "no session" for a worker that failed would
/// send the user looking for a task to start instead of at the log.
pub(super) fn countdown_of(snapshot: &PomodoroSnapshot) -> Option<bongocat_pomodoro::TimerView> {
    if snapshot.worker == bongocat_pomodoro::PomodoroWorkerState::Stopped {
        return None;
    }
    snapshot.timer
}

/// Whether the worker that owns the timer is gone.
pub(super) fn worker_is_lost(snapshot: &PomodoroSnapshot) -> bool {
    snapshot.worker == bongocat_pomodoro::PomodoroWorkerState::Stopped
}

/// The session length, always inside the range the worker accepts.
///
/// A reported length outside the range is clamped rather than rendered: the
/// window shows what the worker will actually run, and a document written by
/// another build cannot make the countdown disagree with its own label.
pub(super) fn focus_minutes(snapshot: &PomodoroSnapshot) -> u32 {
    snapshot
        .focus_minutes
        .clamp(MIN_FOCUS_MINUTES, MAX_FOCUS_MINUTES)
}

/// The expected-session count a typed field reads as.
///
/// A field left empty, or filled with something that is not a number, reads as
/// the default rather than as zero: a session count of zero would create a task
/// that is finished the moment it is ticked.
pub(super) fn sessions_from_typed(typed: &str) -> u32 {
    typed
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|count| *count > 0)
        .unwrap_or(DEFAULT_EXPECTED_SESSIONS)
}

/// The window content.
pub struct PomodoroView {
    pub(super) client: PomodoroClient,
    pub(super) settings_client: SettingsClient,
    /// The display language, kept in sync by [`Self::start_settings_polling`].
    pub(super) language: SettingsLanguage,
    /// The appearance the product is configured with, seeded at open so the first
    /// frame already matches instead of clearing the override the settings window
    /// installed.
    pub(super) appearance_theme: SettingsTheme,
    pub(super) snapshot: PomodoroSnapshot,
    pub(super) applied_theme: Option<SettingsTheme>,
    /// The revision the rendered snapshot came from, so a poll that found no
    /// change costs one integer comparison.
    pub(super) observed_revision: u64,
    pub(super) name_input: Entity<InputState>,
    pub(super) sessions_input: Entity<InputState>,
    pub(super) primary_focus: FocusHandle,
}

impl Focusable for PomodoroView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.primary_focus.clone()
    }
}

impl PomodoroView {
    pub(super) fn new(
        client: PomodoroClient,
        settings_client: SettingsClient,
        language: SettingsLanguage,
        appearance_theme: SettingsTheme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let snapshot = client.snapshot();
        let observed_revision = snapshot.revision;
        let locale = language.catalog_locale();
        let name_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(text(locale, "pomodoro.picker.task_name_placeholder"))
        });
        let sessions_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(DEFAULT_EXPECTED_SESSIONS.to_string())
                .placeholder(text(locale, "pomodoro.picker.estimated_sessions"))
        });
        Self {
            client,
            settings_client,
            language,
            appearance_theme,
            snapshot,
            applied_theme: None,
            observed_revision,
            name_input,
            sessions_input,
            primary_focus: cx.focus_handle().tab_index(1).tab_stop(true),
        }
    }

    /// The name as it will be stored.
    ///
    /// The domain sanitises it too; doing it here as well means the field can
    /// never hold a name the domain would refuse, which is the difference between
    /// a create button that does nothing and one that creates a task.
    pub(super) fn pending_name(&self, cx: &App) -> String {
        bongocat_pomodoro::sanitize_task_name(&self.name_input.read(cx).value())
    }

    /// The expected-session count as it will be stored.
    pub(super) fn pending_sessions(&self, cx: &App) -> u32 {
        sessions_from_typed(&self.sessions_input.read(cx).value())
    }

    /// Whether the create button would do anything.
    pub(super) fn can_create(&self, cx: &App) -> bool {
        !self.pending_name(cx).is_empty() && !worker_is_lost(&self.snapshot)
    }

    /// Create the task.
    ///
    /// The first session is not started here: the worker has not published an id
    /// for the task yet, and guessing one would either start nothing or start the
    /// wrong task. The row in the list is the start button, which appears as soon
    /// as the id is real.
    pub(super) fn create_task(&mut self, cx: &mut Context<Self>) {
        if !self.can_create(cx) {
            return;
        }
        let _ = self.client.try_publish(PomodoroCommand::CreateTask {
            name: self.pending_name(cx),
            estimated_pomodoros: self.pending_sessions(cx),
        });
        cx.notify();
    }

    /// Start a task that already exists.
    pub(super) fn start_task(&mut self, task_id: u64, cx: &mut Context<Self>) {
        let _ = self
            .client
            .try_publish(PomodoroCommand::StartTask { task_id });
        cx.notify();
    }

    /// Re-read the countdown and the task list.
    ///
    /// The countdown moves every second, so the poll is short; the snapshot is
    /// only taken when the revision moved, which is what makes an idle window
    /// cheap rather than a per-tick clone of the whole list.
    pub(super) fn poll_state(&mut self, cx: &mut Context<Self>) {
        let revision = self.client.revision();
        if revision == self.observed_revision {
            return;
        }
        self.observed_revision = revision;
        self.snapshot = self.client.snapshot();
        cx.notify();
    }

    /// Follow the countdown.
    pub(super) fn start_polling(&self, cx: &mut Context<Self>) {
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            loop {
                executor.timer(TIMER_POLL_INTERVAL).await;
                if this.update(cx, |view, cx| view.poll_state(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// Follow the settings snapshot for the language and the appearance.
    pub(super) fn start_settings_polling(&self, cx: &mut Context<Self>) {
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let mut last_revision: Option<u64> = None;
            loop {
                executor.timer(SETTINGS_POLL_INTERVAL).await;
                let client = match this.update(cx, |view, _| view.settings_client.clone()) {
                    Ok(client) => client,
                    Err(_) => break,
                };
                let Ok(revision) = client.read_snapshot_revision().await else {
                    continue;
                };
                if last_revision == Some(revision) {
                    continue;
                }
                let Ok(snapshot) = client.read_snapshot().await else {
                    continue;
                };
                // A released view answers an error here, which is how the loop
                // ends: the window was closed.
                if this
                    .update(cx, |view, cx| {
                        let language_changed = view.language != snapshot.resolved_language;
                        let theme_changed = view.appearance_theme != snapshot.appearance_theme;
                        if language_changed || theme_changed {
                            view.language = snapshot.resolved_language;
                            view.appearance_theme = snapshot.appearance_theme;
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
                last_revision = Some(revision);
            }
        })
        .detach();
    }

    /// Apply the product appearance, once per change.
    pub(super) fn sync_component_theme(
        &mut self,
        theme: SettingsTheme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.applied_theme == Some(theme) {
            return;
        }
        apply_component_theme(theme, window, cx);
        self.applied_theme = Some(theme);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bongocat_pomodoro::{DEFAULT_FOCUS_MINUTES, PomodoroWorkerState, TimerState, TimerView};

    fn task(id: u64, status: PomodoroTaskStatus, running: bool) -> PomodoroTaskView {
        PomodoroTaskView {
            id,
            name: format!("task {id}"),
            estimated_pomodoros: 4,
            completed_pomodoros: 1,
            status,
            created_at_unix: 0,
            completed_at_unix: None,
            running,
        }
    }

    fn snapshot(worker: PomodoroWorkerState, timer: Option<TimerView>) -> PomodoroSnapshot {
        PomodoroSnapshot {
            revision: 1,
            worker,
            tasks: Vec::new(),
            timer,
            focus_minutes: DEFAULT_FOCUS_MINUTES,
            last_completion: None,
        }
    }

    fn running_timer() -> TimerView {
        TimerView {
            task_id: 1,
            state: TimerState::Running,
            remaining_seconds: 60,
            total_seconds: 1_500,
        }
    }

    #[test]
    fn the_continued_task_is_the_first_unfinished_one_that_is_not_already_running() {
        let tasks = vec![
            task(1, PomodoroTaskStatus::done, false),
            task(2, PomodoroTaskStatus::in_progress, true),
            task(3, PomodoroTaskStatus::in_progress, false),
            task(4, PomodoroTaskStatus::in_progress, false),
        ];
        assert_eq!(task_to_continue(&tasks).map(|task| task.id), Some(3));
    }

    #[test]
    fn there_is_nothing_to_continue_when_everything_is_finished_or_running() {
        let finished = vec![
            task(1, PomodoroTaskStatus::done, false),
            task(2, PomodoroTaskStatus::in_progress, true),
        ];
        assert!(task_to_continue(&finished).is_none());
        assert!(task_to_continue(&[]).is_none());
    }

    #[test]
    fn the_picker_lists_unfinished_tasks_only() {
        let tasks = vec![
            task(1, PomodoroTaskStatus::done, false),
            task(2, PomodoroTaskStatus::in_progress, true),
            task(3, PomodoroTaskStatus::in_progress, false),
        ];
        assert_eq!(
            startable_tasks(&tasks)
                .iter()
                .map(|task| task.id)
                .collect::<Vec<_>>(),
            vec![2, 3],
            "a running task stays listed so its row can show that it is running"
        );
    }

    #[test]
    fn a_stopped_worker_reports_no_countdown_and_says_so() {
        let lost = snapshot(PomodoroWorkerState::Stopped, Some(running_timer()));
        assert!(worker_is_lost(&lost));
        assert_eq!(countdown_of(&lost), None);
        let live = snapshot(PomodoroWorkerState::Ready, Some(running_timer()));
        assert!(!worker_is_lost(&live));
        assert_eq!(countdown_of(&live), Some(running_timer()));
    }

    #[test]
    fn the_focus_length_is_clamped_into_the_range_the_worker_accepts() {
        for reported in [0, 1, 25, 999, u32::MAX] {
            let mut snapshot = snapshot(PomodoroWorkerState::Ready, None);
            snapshot.focus_minutes = reported;
            assert!(
                (MIN_FOCUS_MINUTES..=MAX_FOCUS_MINUTES).contains(&focus_minutes(&snapshot)),
                "a reported length of {reported} was read as {}",
                focus_minutes(&snapshot)
            );
        }
    }

    #[test]
    fn an_unusable_session_count_reads_as_the_default() {
        assert_eq!(sessions_from_typed(""), DEFAULT_EXPECTED_SESSIONS);
        assert_eq!(sessions_from_typed("   "), DEFAULT_EXPECTED_SESSIONS);
        assert_eq!(sessions_from_typed("abc"), DEFAULT_EXPECTED_SESSIONS);
        assert_eq!(sessions_from_typed("0"), DEFAULT_EXPECTED_SESSIONS);
        assert_eq!(sessions_from_typed("-2"), DEFAULT_EXPECTED_SESSIONS);
        assert_eq!(sessions_from_typed(" 7 "), 7);
        assert_eq!(sessions_from_typed("300"), 300);
    }
}
