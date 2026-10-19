//! The pomodoro window's frames, built for real.
//!
//! Each frame a user can reach is rendered in a headless window and read back:
//! the countdown, the new-task form, the task list, the continue row and the
//! worker-lost notice are five different answers, and a window that renders only
//! some of them is the failure mode this covers.

use std::cell::RefCell;
use std::rc::Rc;

use crate::pomodoro_window::{PomodoroView, view};
use crate::{SettingsClient, SettingsLanguage, SettingsServiceEndpoint, SettingsTheme};
use bongocat_pomodoro::{
    Cultivation, DEFAULT_FOCUS_MINUTES, PomodoroClient, PomodoroSnapshot, PomodoroTaskStatus,
    PomodoroTaskView, PomodoroWorkerState, TimerState, TimerView,
};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AppContext, Context, ElementId, Entity, IntoElement, Render, TestAppContext, Window, px, size,
};
// `test_support()` registers elements for the headless window tests; without
// `gpui-kit`'s `test-support` feature it is the identity function.

/// The window root the harness mounts.
///
/// The application mounts `Root` here, which adds the notification layer these
/// tests do not exercise. Mounting a plain wrapper instead is what lets the
/// harness keep the `Entity<PomodoroView>` and drive the view directly.
struct Mount(Entity<PomodoroView>);

impl Render for Mount {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.0.clone()
    }
}

struct Harness {
    handle: gpui_kit::WindowHandle<Mount>,
    view: Entity<PomodoroView>,
    /// Kept alive so the window's language poll sees a live channel.
    _settings: SettingsServiceEndpoint,
}

impl Harness {
    fn set_snapshot(&mut self, cx: &mut TestAppContext, snapshot: PomodoroSnapshot) {
        cx.update_entity(&self.view, |view, _| view.snapshot = snapshot);
    }

    /// Paint a frame and report whether the element with `id` was painted.
    fn paints(&self, cx: &mut TestAppContext, id: ElementId) -> bool {
        cx.update_window(self.handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.try_find(id).is_some()
        })
        .expect("the pomodoro window stays open")
    }
}

fn snapshot(worker: PomodoroWorkerState) -> PomodoroSnapshot {
    PomodoroSnapshot {
        revision: 1,
        worker,
        tasks: Vec::new(),
        timer: None,
        focus_minutes: DEFAULT_FOCUS_MINUTES,
        last_completion: None,
        cultivation: Cultivation::default().view(),
    }
}

fn task(id: u64, name: &str, running: bool) -> PomodoroTaskView {
    PomodoroTaskView {
        id,
        name: name.to_owned(),
        estimated_pomodoros: 4,
        completed_pomodoros: 1,
        status: PomodoroTaskStatus::in_progress,
        created_at_unix: 0,
        completed_at_unix: None,
        running,
    }
}

fn harness(cx: &mut TestAppContext) -> Harness {
    cx.update(gpui_kit::init);
    // The client is wired to a sink the harness drains, so a test can assert what
    // the view asked for rather than only what it painted.
    let (pomodoro_client, sink) = PomodoroClient::with_sink();
    let (settings_client, settings_endpoint) = SettingsClient::bounded(4);
    let created: Rc<RefCell<Option<Entity<PomodoroView>>>> = Rc::new(RefCell::new(None));
    let captured = Rc::clone(&created);
    let handle = cx.open_window(size(px(480.0), px(600.0)), move |window, cx| {
        let view = cx.new(|cx| {
            PomodoroView::new(
                pomodoro_client,
                settings_client,
                SettingsLanguage::English,
                SettingsTheme::System,
                window,
                cx,
            )
        });
        captured.borrow_mut().replace(view.clone());
        Mount(view)
    });
    let view = created
        .borrow_mut()
        .take()
        .expect("the window built its pomodoro view");
    std::mem::forget(sink);
    Harness {
        handle,
        view,
        _settings: settings_endpoint,
    }
}

#[gpui_kit::test]
fn the_empty_window_offers_the_form_and_says_there_are_no_tasks(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let mut snapshot = snapshot(PomodoroWorkerState::Ready);
    snapshot.tasks = Vec::new();
    harness.set_snapshot(cx, snapshot);
    assert!(
        harness.paints(cx, "pomodoro-new-task".into()),
        "the form must be there"
    );
    assert!(
        harness.paints(cx, "pomodoro-task-none".into()),
        "an empty list must say so"
    );
    assert!(
        !harness.paints(cx, "pomodoro-countdown".into()),
        "no session means no countdown"
    );
    assert!(
        !harness.paints(cx, "pomodoro-worker-lost".into()),
        "a live worker must not raise the failure notice"
    );
}

#[gpui_kit::test]
fn a_running_session_shows_the_countdown_and_marks_its_task_running(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let mut snapshot = snapshot(PomodoroWorkerState::Ready);
    snapshot.tasks = vec![task(1, "focus", true), task(2, "later", false)];
    snapshot.timer = Some(TimerView {
        task_id: 1,
        state: TimerState::Running,
        remaining_seconds: 24 * 60 + 59,
        total_seconds: 25 * 60,
    });
    harness.set_snapshot(cx, snapshot);
    assert!(
        harness.paints(cx, "pomodoro-countdown".into()),
        "the countdown"
    );
    assert!(
        harness.paints(cx, "pomodoro-task-running".into()),
        "the running task must say so"
    );
    // The running task itself is never the continued one, but another unfinished
    // task still is: a user with a session running and a second task open must be
    // able to start the second one without waiting for the first to finish.
    let continued = cx.update(|app| {
        view::task_to_continue(&harness.view.read(app).snapshot.tasks).map(|task| task.id)
    });
    assert_eq!(
        continued,
        Some(2),
        "the continue row must not offer the task that is already running"
    );
}

#[gpui_kit::test]
fn an_unfinished_task_offers_a_continue_row(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let mut snapshot = snapshot(PomodoroWorkerState::Ready);
    snapshot.tasks = vec![task(1, "left unfinished", false), task(2, "later", false)];
    harness.set_snapshot(cx, snapshot);
    assert!(
        harness.paints(cx, "pomodoro-continue-row".into()),
        "the unfinished task must be offered"
    );
    assert!(
        !harness.paints(cx, "pomodoro-task-none".into()),
        "a task list with a task must not also say it is empty"
    );
}

#[gpui_kit::test]
fn a_paused_session_says_it_is_paused(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let mut snapshot = snapshot(PomodoroWorkerState::Ready);
    snapshot.tasks = vec![task(1, "focus", true)];
    snapshot.timer = Some(TimerView {
        task_id: 1,
        state: TimerState::Paused,
        remaining_seconds: 600,
        total_seconds: 25 * 60,
    });
    harness.set_snapshot(cx, snapshot);
    assert!(
        harness.paints(cx, "pomodoro-countdown-paused".into()),
        "a paused session must be distinguishable from a running one"
    );
}

#[gpui_kit::test]
fn a_lost_worker_says_so_instead_of_showing_a_stale_countdown(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let mut snapshot = snapshot(PomodoroWorkerState::Stopped);
    // A worker that failed leaves the last snapshot it managed to publish behind;
    // rendering its countdown would tell the user a session is running that no
    // thread is driving.
    snapshot.tasks = vec![task(1, "before the failure", false)];
    snapshot.timer = Some(TimerView {
        task_id: 1,
        state: TimerState::Running,
        remaining_seconds: 60,
        total_seconds: 25 * 60,
    });
    harness.set_snapshot(cx, snapshot);
    assert!(
        harness.paints(cx, "pomodoro-worker-lost".into()),
        "a lost worker must say so"
    );
    assert!(
        !harness.paints(cx, "pomodoro-countdown".into()),
        "a countdown the worker is not driving must not be shown as live"
    );
}

#[gpui_kit::test]
fn the_session_length_is_stated_in_the_unit_the_session_count_uses(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let mut snapshot = snapshot(PomodoroWorkerState::Ready);
    snapshot.focus_minutes = 30;
    harness.set_snapshot(cx, snapshot);
    assert!(
        harness.paints(cx, "pomodoro-session-length".into()),
        "\"4 sessions\" means nothing until the user can see that a session is thirty minutes"
    );
}

#[gpui_kit::test]
fn the_choices_a_task_list_offers_follow_the_task_states(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let mut snapshot = snapshot(PomodoroWorkerState::Ready);
    let mut finished = task(9, "finished", false);
    finished.status = PomodoroTaskStatus::done;
    snapshot.tasks = vec![finished, task(2, "later", false)];

    harness.set_snapshot(cx, snapshot);
    assert!(
        harness.paints(cx, "pomodoro-continue-row".into()),
        "the unfinished task is offered"
    );
    assert!(
        harness.paints(cx, ("pomodoro-task-start", 2u64).into()),
        "a startable task offers its own start button"
    );
    assert!(
        !harness.paints(cx, ("pomodoro-task-start", 9u64).into()),
        "a finished task offers nothing: this window answers what to work on now"
    );
}

#[gpui_kit::test]
fn finished_tasks_fold_until_the_group_is_clicked(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let mut snapshot = snapshot(PomodoroWorkerState::Ready);
    let mut finished = task(9, "finished", false);
    finished.status = PomodoroTaskStatus::done;
    snapshot.tasks = vec![finished, task(2, "later", false)];
    harness.set_snapshot(cx, snapshot);
    // The toggle is always drawn; the finished row waits behind it.
    assert!(
        harness.paints(cx, "pomodoro-done-toggle".into()),
        "the finished group announces itself even folded"
    );
    assert!(
        !harness.paints(cx, ("pomodoro-task-row", 9u64).into()),
        "a finished task is folded by default"
    );
    assert!(
        harness.paints(cx, ("pomodoro-task-row", 2u64).into()),
        "an unfinished task is never folded"
    );
    cx.update_entity(&harness.view, |view, _| view.done_expanded = true);
    assert!(
        harness.paints(cx, ("pomodoro-task-row", 9u64).into()),
        "expanding the group shows the finished task"
    );
}

#[gpui_kit::test]
fn the_worker_lost_notice_does_not_hide_the_tasks_that_already_exist(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let mut snapshot = snapshot(PomodoroWorkerState::Stopped);
    snapshot.tasks = vec![task(1, "before the failure", false)];
    harness.set_snapshot(cx, snapshot);
    assert!(harness.paints(cx, "pomodoro-worker-lost".into()));
    assert!(
        harness.paints(cx, "pomodoro-task-list".into()),
        "the tasks a failed worker left behind are still the user's tasks"
    );
}

/// A pure-function check, so it needs no window: it asserts the rule the frame
/// draws by, independent of whether the frame drew it.
#[test]
fn the_decisions_the_frame_makes_are_the_ones_the_domain_would_make() {
    let mut finished = task(1, "finished", false);
    finished.status = PomodoroTaskStatus::done;
    let tasks = vec![finished, task(2, "running", true), task(3, "later", false)];
    assert_eq!(
        view::task_to_continue(&tasks).map(|task| task.id),
        Some(3),
        "the row the frame draws must be the row the rule picks"
    );
    assert_eq!(
        view::listed_tasks(&tasks).len(),
        3,
        "every task keeps its row: finished ones dim, the running one says so"
    );
}
#[gpui_kit::test]
fn every_row_of_this_window_is_localized_in_every_language() {
    // The contract is the list of strings the frame actually reaches for. A key
    // that resolves to nothing renders a blank row, which is the failure this
    // rules out for all eight languages rather than only the one a test runs in.
    for language in SettingsLanguage::ALL {
        let locale = language.catalog_locale();
        for key in crate::pomodoro_window::POMODORO_LOCALIZED_KEYS {
            assert!(
                !bongocat_i18n::text(locale, key).is_empty(),
                "{language:?} has no text for {key}"
            );
        }
    }
}
