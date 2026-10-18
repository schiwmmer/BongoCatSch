//! The scenarios the verification plan names, written as one walk through the
//! public surface rather than as a test per module.
//!
//! The unit tests inside each module cover the edges; these cover the sequence
//! the product promises: create a task, run a session to zero, see the count go
//! up, tick the task off, untick it, and find everything still there after a
//! restart.

use std::time::Duration;

use crate::{
    CalendarDay, CalendarDayTotal, DEFAULT_FOCUS_MINUTES, MIN_FOCUS_MINUTES, PomodoroCommand,
    PomodoroState, PomodoroTaskStatus, Timer, month_grid, month_totals, sanitize_task_name,
};

fn create(state: &mut PomodoroState, name: &str, estimated: u32) -> u64 {
    state.apply(
        PomodoroCommand::CreateTask {
            name: name.into(),
            estimated_pomodoros: estimated,
        },
        1_000,
    );
    state.document().tasks.len() as u64
}

#[test]
fn the_whole_task_lifecycle_a_user_walks_through() {
    let mut state = PomodoroState::empty();
    let task_id = create(&mut state, "write the quarterly summary", 4);
    assert_eq!(task_id, 1);
    assert_eq!(state.focus_minutes(), DEFAULT_FOCUS_MINUTES);

    // Three sessions, each run to zero.
    for session in 0..3 {
        state.apply(PomodoroCommand::StartTask { task_id }, 1_000);
        let remaining = state.timer().remaining_seconds();
        assert!(
            remaining > 0,
            "session {session} started with time on the clock"
        );
        let completion = state
            .advance(Duration::from_secs(u64::from(remaining)), 2_000)
            .expect("the session finished");
        assert_eq!(completion.task_id, task_id);
        assert_eq!(completion.sequence, session + 1);
        assert_eq!(state.timer(), Timer::Idle);
    }
    let snapshot = state.snapshot(1);
    assert_eq!(snapshot.tasks[0].completed_pomodoros, 3);
    assert_eq!(snapshot.tasks[0].estimated_pomodoros, 4);

    // One more session, interrupted.
    state.apply(PomodoroCommand::StartTask { task_id }, 3_000);
    state.advance(Duration::from_secs(60), 3_060);
    state.apply(PomodoroCommand::Pause, 3_060);
    state.apply(PomodoroCommand::Resume, 3_100);
    state.apply(PomodoroCommand::Abandon, 3_100);
    assert_eq!(state.snapshot(2).tasks[0].completed_pomodoros, 3);

    // Tick the task off and back.
    state.apply(
        PomodoroCommand::SetTaskDone {
            task_id,
            done: true,
        },
        4_000,
    );
    assert_eq!(state.snapshot(3).tasks[0].status, PomodoroTaskStatus::done);
    state.apply(
        PomodoroCommand::SetTaskDone {
            task_id,
            done: false,
        },
        5_000,
    );
    let snapshot = state.snapshot(4);
    assert_eq!(snapshot.tasks[0].status, PomodoroTaskStatus::in_progress);
    assert_eq!(snapshot.tasks[0].completed_pomodoros, 3);
}

#[test]
fn a_paused_session_survives_a_long_absence() {
    let mut state = PomodoroState::empty();
    let task_id = create(&mut state, "deep work", 1);
    state.apply(PomodoroCommand::StartTask { task_id }, 1_000);
    state.advance(Duration::from_secs(120), 1_120);
    state.apply(PomodoroCommand::Pause, 1_120);
    let frozen = state.timer().remaining_seconds();
    // A pause is a promise that time away does not count.
    assert!(state.advance(Duration::from_secs(3_600), 4_720).is_none());
    state.apply(PomodoroCommand::Resume, 4_720);
    assert_eq!(state.timer().remaining_seconds(), frozen);
    assert!(
        state
            .advance(Duration::from_secs(u64::from(frozen)), 4_720)
            .is_some()
    );
}

#[test]
fn the_month_view_shows_what_the_sessions_recorded() {
    let mut state = PomodoroState::empty();
    let first = create(&mut state, "first task", 2);
    let second = create(&mut state, "second task", 1);
    // Two sessions on 2026-10-14 and one on 2026-10-15, in UTC seconds.
    let day_14 = 20_000_i64 * 86_400;
    state.record_completion_for_test(first, day_14 + 3_600);
    state.record_completion_for_test(first, day_14 + 5_400);
    state.record_completion_for_test(second, day_14 + 7_200);
    state.record_completion_for_test(second, day_14 + 86_400);

    let days = state.day_totals(0);
    let totals = month_totals(&days);
    assert_eq!(totals.pomodoros, 4);
    // Three sessions on the first day belong to two tasks, the fourth to one.
    assert_eq!(totals.tasks, 3);

    // The grid has a cell for every day that carries a count, in the month the
    // counts belong to. The timestamps above are day numbers 20000 and 20001,
    // which the grid is asked for by date.
    let first_day = CalendarDay::from_day_number(20_000).to_civil();
    let grid = month_grid(first_day.year, first_day.month);
    let populated = grid
        .cells
        .iter()
        .filter(|day| {
            days.get(day)
                .is_some_and(|total: &CalendarDayTotal| total.pomodoros > 0)
        })
        .count();
    assert_eq!(populated, 2, "both recorded days must have a cell");
}

#[test]
fn a_day_total_splits_sessions_between_tasks_on_the_same_day() {
    let mut state = PomodoroState::empty();
    let first = create(&mut state, "first", 3);
    let second = create(&mut state, "second", 3);
    let day = 20_000_i64 * 86_400;
    state.record_completion_for_test(first, day);
    state.record_completion_for_test(first, day + 60);
    state.record_completion_for_test(second, day + 120);
    let days = state.day_totals(0);
    let total = days
        .get(&CalendarDay::from_day_number(20_000))
        .copied()
        .expect("the day");
    assert_eq!(total.pomodoros, 3);
    assert_eq!(total.tasks, 2);
}

#[test]
fn a_restart_keeps_the_history_and_drops_the_countdown() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("pomodoro-tasks.json");

    let mut state = PomodoroState::load(&path);
    let task_id = create(&mut state, "survives a restart", 2);
    state.apply(PomodoroCommand::StartTask { task_id }, 1_000);
    state.advance(Duration::from_secs(1_500), 1_500);
    // One session done, and the next one left running when the machine goes
    // away: the document must keep the first and drop the second.
    assert_eq!(state.timer(), Timer::Idle);
    state.apply(PomodoroCommand::StartTask { task_id }, 2_000);
    state.advance(Duration::from_secs(60), 2_060);
    assert!(state.timer().state().is_running());
    crate::document::write_document(&path, state.document()).expect("write the document");

    let reloaded = PomodoroState::load(&path);
    let snapshot = reloaded.snapshot(1);
    assert_eq!(snapshot.tasks.len(), 1);
    assert_eq!(snapshot.tasks[0].name, "survives a restart");
    assert_eq!(snapshot.tasks[0].completed_pomodoros, 1);
    assert_eq!(snapshot.timer, None, "a countdown is not restored");
    assert!(snapshot.last_completion.is_none());

    // Starting again from the task gives a whole session, not the leftover.
    let mut reloaded = reloaded;
    reloaded.apply(PomodoroCommand::StartTask { task_id }, 2_000);
    assert_eq!(
        reloaded.timer().remaining_seconds(),
        DEFAULT_FOCUS_MINUTES * 60
    );
}

#[test]
fn the_session_length_survives_a_restart() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("pomodoro-tasks.json");
    let mut state = PomodoroState::load(&path);
    state.apply(PomodoroCommand::SetFocusMinutes { minutes: 45 }, 1_000);
    crate::document::write_document(&path, state.document()).expect("write the document");
    assert_eq!(PomodoroState::load(&path).focus_minutes(), 45);
}

#[test]
fn an_out_of_range_session_length_is_refused_rather_than_clamped_silently() {
    let mut state = PomodoroState::empty();
    state.apply(PomodoroCommand::SetFocusMinutes { minutes: 0 }, 1_000);
    assert_eq!(state.focus_minutes(), MIN_FOCUS_MINUTES);
}

#[test]
fn a_name_pasted_from_a_terminal_cannot_break_a_row() {
    assert_eq!(sanitize_task_name("  spaced\u{200b}name\t"), "spacedname");
    assert_eq!(sanitize_task_name("two  spaces"), "two spaces");
    assert_eq!(
        sanitize_task_name(&"n".repeat(500)).chars().count(),
        crate::MAX_TASK_NAME_CHARS
    );
    assert!(sanitize_task_name("\n\r").is_empty());
}

#[test]
fn an_edit_rewrites_the_plan_and_keeps_the_history() {
    let mut state = PomodoroState::empty();
    let task_id = create(&mut state, "write notes", 4);
    // One finished session, run to zero the way the product runs one, so the
    // history the edit has to keep is the real thing: the counter and the
    // calendar's timestamps both.
    state.apply(PomodoroCommand::StartTask { task_id }, 1_000);
    let remaining = state.timer().remaining_seconds();
    state
        .advance(Duration::from_secs(u64::from(remaining)), 2_000)
        .expect("the session finished");

    state.apply(
        PomodoroCommand::UpdateTask {
            task_id,
            name: "write the notes".into(),
            estimated_pomodoros: 2,
        },
        1_000,
    );
    let task = &state.snapshot(1).tasks[0];
    assert_eq!(task.name, "write the notes");
    assert_eq!(task.estimated_pomodoros, 2);
    // The finished sessions are history: an edit is about the plan, never
    // about the work already done — neither the counter nor the timestamps
    // the month view counts.
    assert_eq!(task.completed_pomodoros, 1);
    let totals = state.day_totals(0);
    assert_eq!(
        totals.values().map(|total| total.pomodoros).sum::<u32>(),
        1,
        "the calendar keeps the finished day"
    );

    // A name the user cannot read is refused rather than stored, so the row
    // keeps the name it had.
    state.apply(
        PomodoroCommand::UpdateTask {
            task_id,
            name: "   ".into(),
            estimated_pomodoros: 9,
        },
        2_000,
    );
    let task = &state.snapshot(2).tasks[0];
    assert_eq!(task.name, "write the notes");
    assert_eq!(task.estimated_pomodoros, 2);

    // An edit addressed at a task that is not there changes nothing.
    let before = state.snapshot(3).tasks.len();
    state.apply(
        PomodoroCommand::UpdateTask {
            task_id: 999,
            name: "ghost".into(),
            estimated_pomodoros: 1,
        },
        3_000,
    );
    assert_eq!(state.snapshot(4).tasks.len(), before);
}

#[test]
fn deleting_a_task_removes_it_its_history_and_its_running_session() {
    let mut state = PomodoroState::empty();
    let first = create(&mut state, "first", 2);
    let second = create(&mut state, "second", 2);
    let day = 20_000 * 86_400;
    state.record_completion_for_test(first, day);
    state.record_completion_for_test(second, day + 60);
    // A session is running for the task about to be deleted.
    state.apply(PomodoroCommand::StartTask { task_id: first }, 1_000);

    state.apply(PomodoroCommand::DeleteTask { task_id: first }, 2_000);

    let snapshot = state.snapshot(1);
    assert_eq!(snapshot.tasks.len(), 1, "the other task stays");
    assert_eq!(snapshot.tasks[0].id, second);
    // The countdown belonged to the deleted task, so it goes with it rather
    // than completing into nothing.
    assert_eq!(state.timer(), Timer::Idle);
    // The day loses only the deleted task's session, not its neighbour's.
    let total = state
        .day_totals(0)
        .get(&CalendarDay::from_day_number(20_000))
        .copied()
        .expect("the day");
    assert_eq!(total.pomodoros, 1);
    assert_eq!(total.tasks, 1);
}
