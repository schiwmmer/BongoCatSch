//! The worker thread that owns the state machine, and the client handles that
//! talk to it.
//!
//! The shape follows the motion audio service: a bounded command queue, a
//! worker that folds elapsed time into the countdown, a shared snapshot behind a
//! lock and a condition variable, and a shutdown that reports rather than
//! blocks forever.
//!
//! The countdown is driven by elapsed monotonic time rather than by subtracting
//! a second per wake-up. A worker that sleeps for a second and then decrements
//! accumulates the delay of every sleep and every scheduling hiccup, so a
//! twenty-five minute session would end minutes late after a few hours of
//! running. Measuring the gap between wake-ups instead makes a late wake-up
//! cost exactly the time it was late.

use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crate::{
    COMMAND_CAPACITY, CalendarDay, CalendarDayTotal, CalendarTaskActivity, PomodoroCommand,
    PomodoroSnapshot, PomodoroState, Timer, TimerState,
    document::{Document, write_document},
};

/// The longest a worker waits before looking at its own clock again.
///
/// A running session shortens this to the time left on its deadline, so the
/// worker wakes when it has something to do. The bound still matters: a session
/// that was paused and then abandoned leaves no deadline behind, and without it
/// the worker would sleep indefinitely.
const IDLE_POLL: Duration = Duration::from_secs(1);

/// What the worker is doing right now.
///
/// A worker that failed to start reports [`PomodoroWorkerState::Stopped`]
/// through its client, which is what lets a caller degrade to "no timer"
/// without guessing whether the worker is busy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PomodoroWorkerState {
    Starting,
    Ready,
    Stopping,
    Stopped,
}

/// Why the worker could not do something.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PomodoroErrorCode {
    /// The command queue is full, or the worker is gone.
    WorkerUnavailable,
    /// The document could not be written. The in-memory state still advanced;
    /// the next write carries it.
    DocumentWriteFailed,
}

/// A command that was not accepted, handed back so the caller can report it.
#[derive(Debug, Eq, PartialEq)]
pub enum PomodoroPublishError {
    /// The queue is full. A pomodoro command is a click, not a per-frame
    /// signal, so this means the user is clicking faster than the worker folds
    /// time in — harmless, and not worth blocking a UI thread over.
    QueueFull(PomodoroCommand),
    /// The worker is stopping or stopped.
    ServiceStopped(PomodoroCommand),
}

impl std::fmt::Display for PomodoroPublishError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QueueFull(_) => write!(formatter, "pomodoro command queue is full"),
            Self::ServiceStopped(_) => write!(formatter, "pomodoro service is stopped"),
        }
    }
}

impl std::error::Error for PomodoroPublishError {}

/// The worker thread could not be started.
#[derive(Debug, thiserror::Error)]
#[error("cannot start pomodoro worker: {0}")]
pub struct PomodoroStartError(pub(crate) io::Error);

/// The worker did not stop inside the budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PomodoroShutdownError {
    #[error("pomodoro shutdown timed out")]
    TimedOut,
    #[error("pomodoro worker panicked")]
    WorkerPanicked,
}

/// The worker's shared state.
struct SharedState {
    inner: Mutex<Inner>,
    changed: Condvar,
    shutdown_requested: AtomicBool,
}

struct Inner {
    worker_state: PomodoroWorkerState,
    domain: PomodoroState,
    /// The snapshot callers read. Republished from the domain state on every
    /// change, so a reader never has to hold two locks.
    snapshot: PomodoroSnapshot,
    /// The document as last written, so a command that changed nothing durable
    /// does not cost a write.
    written: Document,
    revision: u64,
    completed_sessions: u64,
    document_writes: u64,
    document_write_failures: u64,
    last_error: Option<PomodoroErrorCode>,
}

impl Inner {
    /// Republish the snapshot from the domain state.
    fn publish(&mut self) {
        self.revision = self.revision.saturating_add(1);
        // The cultivation cooldowns are read against the wall clock, so the
        // snapshot is built with one: a surface that polls gets the truth rather
        // than a cached answer from when the state last changed.
        self.snapshot = self.domain.snapshot_at(self.revision, now_unix());
        self.snapshot.worker = self.worker_state;
    }

    /// Write the document when the domain state has moved past what is on disk.
    ///
    /// A failed write is recorded and the state stays ahead of the file: the
    /// next write carries the same change, so a full disk or a read-only
    /// directory costs one missed write rather than the work it was recording.
    fn write_document(&mut self, path: &Path) {
        if !self.domain.document_is_dirty(&self.written) {
            return;
        }
        let document = self.domain.document().clone();
        match write_document(path, &document) {
            Ok(()) => {
                self.written = document;
                self.document_writes = self.document_writes.saturating_add(1);
            }
            Err(_) => {
                self.document_write_failures = self.document_write_failures.saturating_add(1);
                self.last_error = Some(PomodoroErrorCode::DocumentWriteFailed);
            }
        }
    }
}

/// The handle callers use to ask for things and read the state.
#[derive(Clone)]
pub struct PomodoroClient {
    sender: SyncSender<PomodoroCommand>,
    shared: Arc<SharedState>,
}

impl PomodoroClient {
    /// A client whose worker does not exist.
    ///
    /// Startup uses this instead of failing the application: a timer that
    /// cannot run leaves the cat, the counters and the input service working,
    /// and the diagnostics say why.
    pub fn unavailable() -> Self {
        let (sender, receiver) = mpsc::sync_channel(1);
        drop(receiver);
        let domain = PomodoroState::empty();
        let mut snapshot = domain.snapshot(0);
        // A client with no worker must not render as an idle one that is about
        // to work: a surface asking for the task list has to see the failure.
        snapshot.worker = PomodoroWorkerState::Stopped;
        Self {
            sender,
            shared: Arc::new(SharedState {
                inner: Mutex::new(Inner {
                    worker_state: PomodoroWorkerState::Stopped,
                    domain,
                    written: Document::default(),
                    revision: 0,
                    completed_sessions: 0,
                    document_writes: 0,
                    document_write_failures: 0,
                    last_error: Some(PomodoroErrorCode::WorkerUnavailable),
                    snapshot,
                }),
                changed: Condvar::new(),
                shutdown_requested: AtomicBool::new(true),
            }),
        }
    }

    /// Queue one command without waiting for room.
    pub fn try_publish(&self, command: PomodoroCommand) -> Result<(), PomodoroPublishError> {
        if self.shared.shutdown_requested.load(Ordering::Acquire) {
            self.note(PomodoroErrorCode::WorkerUnavailable);
            return Err(PomodoroPublishError::ServiceStopped(command));
        }
        match self.sender.try_send(command) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(command)) => {
                self.note(PomodoroErrorCode::WorkerUnavailable);
                Err(PomodoroPublishError::QueueFull(command))
            }
            Err(TrySendError::Disconnected(command)) => {
                self.note(PomodoroErrorCode::WorkerUnavailable);
                Err(PomodoroPublishError::ServiceStopped(command))
            }
        }
    }

    /// A client whose commands land in a channel the caller can read.
    ///
    /// No worker runs behind it, so nothing advances the countdown: it is the
    /// seam a surface's own tests use to assert what a view asked the timer to
    /// do, rather than only what it painted. The window reports
    /// [`PomodoroWorkerState::Ready`] because from a surface's point of view the
    /// commands it sent are accepted, and a test that asserted otherwise would be
    /// testing the sink rather than the surface.
    pub fn with_sink() -> (Self, Receiver<PomodoroCommand>) {
        let (sender, receiver) = mpsc::sync_channel(COMMAND_CAPACITY);
        let mut inner = Inner {
            worker_state: PomodoroWorkerState::Ready,
            domain: PomodoroState::empty(),
            written: Document::default(),
            revision: 0,
            completed_sessions: 0,
            document_writes: 0,
            document_write_failures: 0,
            last_error: None,
            snapshot: PomodoroState::empty().snapshot(0),
        };
        inner.snapshot.worker = PomodoroWorkerState::Ready;
        (
            Self {
                sender,
                shared: Arc::new(SharedState {
                    inner: Mutex::new(inner),
                    changed: Condvar::new(),
                    shutdown_requested: AtomicBool::new(false),
                }),
            },
            receiver,
        )
    }

    /// A client that serves one snapshot the caller built, with no worker.
    ///
    /// The twin of [`Self::with_sink`] for a surface that renders a state a test
    /// wants to see — a cultivator standing at a realm's cap with the attempt
    /// held, say. The page reads exactly that snapshot, and the commands it
    /// publishes still land in the sink the test can read.
    pub fn with_snapshot(snapshot: PomodoroSnapshot) -> (Self, Receiver<PomodoroCommand>) {
        let (client, sink) = Self::with_sink();
        let mut inner = client
            .shared
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        inner.snapshot = snapshot;
        drop(inner);
        (client, sink)
    }

    /// The state a surface renders.
    pub fn snapshot(&self) -> PomodoroSnapshot {
        self.shared
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .snapshot
            .clone()
    }

    /// The per-local-day session totals the month calendar draws.
    ///
    /// The worker owns the only copy of the document, so the history a calendar
    /// shows can only be read from here. The caller passes its own UTC offset
    /// because the day a completion lands on is a local fact, not a worker one —
    /// the same instant buckets into a different day east or west of Greenwich.
    pub fn day_totals(&self, utc_offset_seconds: i32) -> BTreeMap<CalendarDay, CalendarDayTotal> {
        self.shared
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .domain
            .day_totals(utc_offset_seconds)
    }

    /// The tasks behind each local day's finished sessions, for the calendar
    /// cells that name what a day was spent on. Same offset rule as
    /// [`Self::day_totals`]: the caller's UTC offset decides the bucketing.
    pub fn day_activities(
        &self,
        utc_offset_seconds: i32,
    ) -> BTreeMap<CalendarDay, Vec<CalendarTaskActivity>> {
        self.shared
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .domain
            .day_activities(utc_offset_seconds)
    }

    /// The snapshot revision, without cloning the snapshot.
    ///
    /// A poller asks this before reading the whole task list, which is what
    /// makes a per-second tick cheap when nothing has changed.
    pub fn revision(&self) -> u64 {
        self.shared
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .revision
    }

    /// The counts a caller reports.
    pub fn diagnostics(&self) -> PomodoroDiagnostics {
        let inner = self
            .shared
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        PomodoroDiagnostics {
            worker: inner.worker_state,
            revision: inner.revision,
            completed_sessions: inner.completed_sessions,
            document_writes: inner.document_writes,
            document_write_failures: inner.document_write_failures,
            last_error: inner.last_error,
        }
    }

    /// Whether the worker has been asked to stop.
    pub fn is_stopped(&self) -> bool {
        self.shared.shutdown_requested.load(Ordering::Acquire)
    }

    /// Wait until the snapshot revision passes `revision`, or the timeout.
    ///
    /// The overlay uses this to react to a completion as it happens instead of
    /// comparing revisions every frame.
    pub fn wait_for_revision(&self, revision: u64, timeout: Duration) -> Option<u64> {
        let deadline = Instant::now().checked_add(timeout)?;
        let mut inner = self
            .shared
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        loop {
            if inner.revision >= revision || inner.worker_state == PomodoroWorkerState::Stopped {
                return Some(inner.revision);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return None;
            }
            let (next, _) = self
                .shared
                .changed
                .wait_timeout(inner, remaining)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            inner = next;
        }
    }

    fn note(&self, error: PomodoroErrorCode) {
        self.with_inner(|inner| inner.last_error = Some(error));
    }

    fn with_inner(&self, update: impl FnOnce(&mut Inner)) {
        let mut inner = self
            .shared
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        update(&mut inner);
        self.shared.changed.notify_all();
    }
}

/// The counts a caller reports about the worker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PomodoroDiagnostics {
    pub worker: PomodoroWorkerState,
    pub revision: u64,
    pub completed_sessions: u64,
    pub document_writes: u64,
    pub document_write_failures: u64,
    pub last_error: Option<PomodoroErrorCode>,
}

/// The service that owns the worker thread.
pub struct PomodoroService {
    client: PomodoroClient,
    worker: Option<JoinHandle<()>>,
}

impl PomodoroService {
    /// Start the worker and load the document from `path`.
    pub fn start(path: PathBuf) -> Result<Self, PomodoroStartError> {
        Self::start_at(path, 0)
    }

    /// Start the worker for a caller that knows its UTC offset.
    ///
    /// The offset decides which local day a finished session lands on, and
    /// therefore which days count as missed: a completion at half past midnight
    /// in Shanghai belongs to the day that just started, not to yesterday in UTC.
    pub fn start_at(path: PathBuf, utc_offset_seconds: i32) -> Result<Self, PomodoroStartError> {
        let (sender, receiver) = mpsc::sync_channel(COMMAND_CAPACITY);
        let domain = PomodoroState::load_at(&path, utc_offset_seconds);
        let snapshot = domain.snapshot(0);
        let shared = Arc::new(SharedState {
            inner: Mutex::new(Inner {
                worker_state: PomodoroWorkerState::Starting,
                domain,
                written: Document::default(),
                revision: 0,
                completed_sessions: 0,
                document_writes: 0,
                document_write_failures: 0,
                last_error: None,
                snapshot,
            }),
            changed: Condvar::new(),
            shutdown_requested: AtomicBool::new(false),
        });
        let worker_shared = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name("bongocat-pomodoro".into())
            .spawn(move || run_worker(receiver, worker_shared, path))
            .map_err(PomodoroStartError)?;
        Ok(Self {
            client: PomodoroClient { sender, shared },
            worker: Some(worker),
        })
    }

    /// A handle that outlives the service.
    pub fn client(&self) -> PomodoroClient {
        self.client.clone()
    }

    /// Ask the worker to stop, wait for it, and join it.
    pub fn shutdown(mut self, timeout: Duration) -> Result<(), PomodoroShutdownError> {
        self.request_shutdown();
        match self.wait_until_stopped(timeout) {
            Ok(()) => self.join_worker(),
            Err(error @ PomodoroShutdownError::TimedOut) => {
                // A bounded shutdown must not fall through to `Drop`, whose
                // fallback join is only for normal owner destruction. The worker
                // already has the stop request and finishes on its own.
                self.worker.take();
                Err(error)
            }
            Err(error) => Err(error),
        }
    }

    fn request_shutdown(&self) {
        self.client
            .shared
            .shutdown_requested
            .store(true, Ordering::Release);
        self.client.shared.changed.notify_all();
    }

    fn wait_until_stopped(&self, timeout: Duration) -> Result<(), PomodoroShutdownError> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(PomodoroShutdownError::TimedOut)?;
        let mut inner = self
            .client
            .shared
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while inner.worker_state != PomodoroWorkerState::Stopped {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(PomodoroShutdownError::TimedOut);
            }
            let (next, result) = self
                .client
                .shared
                .changed
                .wait_timeout(inner, remaining)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            inner = next;
            if result.timed_out() && inner.worker_state != PomodoroWorkerState::Stopped {
                return Err(PomodoroShutdownError::TimedOut);
            }
        }
        Ok(())
    }

    fn join_worker(&mut self) -> Result<(), PomodoroShutdownError> {
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| PomodoroShutdownError::WorkerPanicked)?;
        }
        Ok(())
    }
}

impl Drop for PomodoroService {
    fn drop(&mut self) {
        if self.worker.is_none() {
            return;
        }
        self.request_shutdown();
        let _ = self.join_worker();
    }
}

/// The worker loop.
fn run_worker(receiver: Receiver<PomodoroCommand>, shared: Arc<SharedState>, path: PathBuf) {
    set_worker_state(&shared, PomodoroWorkerState::Ready);
    // The session the document was holding comes back before anything else: a
    // quit or an upgrade is not an abandon, and the countdown has been running
    // the whole time the application was not.
    resume_session(&shared, &path);
    // The wall clock a countdown is measured against is the gap between these
    // readings, not a per-tick subtraction, so a late wake-up costs exactly the
    // time it was late.
    let mut last_tick = Instant::now();
    // The seconds a previous wake-up could not hand over because they had not
    // finished yet. A wake-up that arrives 30 ms after the last one owes zero
    // whole seconds, and that 30 ms must survive to the next wake-up: typing or
    // moving the pointer publishes a command on every frame, which turns the
    // worker into a 30 ms loop, and dropping each fraction froze the countdown
    // for as long as the input lasted.
    let mut carried = Duration::ZERO;
    loop {
        if shared.shutdown_requested.load(Ordering::Acquire) {
            break;
        }
        let wait = wait_for(&shared);
        match receiver.recv_timeout(wait) {
            Ok(command) => process_command(&shared, &path, command),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(last_tick);
        last_tick = now;
        let elapsed = take_whole_seconds(elapsed, &mut carried);
        // Idle time is not a countdown: a fraction carried across a break would
        // be charged to the session that comes after it.
        if !tick(&shared, &path, elapsed) {
            carried = Duration::ZERO;
        }
    }
    set_worker_state(&shared, PomodoroWorkerState::Stopping);
    while receiver.try_recv().is_ok() {}
    // The countdown's own deadline is in the document — the instant it started,
    // not the seconds it had left — so there is nothing to write on the way out:
    // the next launch recomputes the remainder from that instant, and whatever
    // the last durable command changed was already written.
    set_worker_state(&shared, PomodoroWorkerState::Stopped);
}

/// How long the worker may sleep before folding in elapsed time.
///
/// A running session answers with the time left on its deadline, so the worker
/// wakes exactly when it has something to do; anything else waits for a command.
fn wait_for(shared: &Arc<SharedState>) -> Duration {
    let inner = shared
        .inner
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match inner.domain.timer() {
        Timer::Running {
            remaining_seconds, ..
        } => Duration::from_secs(u64::from(remaining_seconds)).min(IDLE_POLL),
        Timer::Paused { .. } | Timer::Idle => IDLE_POLL,
    }
}

/// The whole seconds in `elapsed`, with the fraction carried to the next call.
///
/// The countdown is stored in whole seconds, so a wake-up that leaves a fraction
/// of one behind has nothing to hand over — and throwing that fraction away is
/// what stops a clock that is woken many times a second. Carrying it means a
/// hundred 30 ms wake-ups still pay their three seconds.
fn take_whole_seconds(elapsed: Duration, carried: &mut Duration) -> Duration {
    let total = elapsed.saturating_add(*carried);
    let whole = Duration::from_secs(total.as_secs());
    *carried = total.saturating_sub(whole);
    whole
}

/// Pick up the session the document was holding.
///
/// The seconds between the last write and now belong to that session, so they
/// are charged here rather than on a tick: a countdown that came due while the
/// application was closed has to settle at launch, and one that is still running
/// has to come back with what is actually left of it.
fn resume_session(shared: &Arc<SharedState>, path: &Path) {
    let mut inner = shared
        .inner
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let completed = inner.domain.resume_live_session(now_unix()).is_some();
    if completed {
        inner.completed_sessions = inner.completed_sessions.saturating_add(1);
    }
    inner.publish();
    inner.write_document(path);
    drop(inner);
    shared.changed.notify_all();
}

fn process_command(shared: &Arc<SharedState>, path: &Path, command: PomodoroCommand) {
    let mut inner = shared
        .inner
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    inner.domain.apply(command, now_unix());
    inner.publish();
    inner.write_document(path);
    drop(inner);
    shared.changed.notify_all();
}

/// Fold elapsed time into a running countdown.
///
/// Returns whether a session was running: a paused or idle timer answers `false`
/// so the caller knows the time it just measured belongs to nobody.
fn tick(shared: &Arc<SharedState>, path: &Path, elapsed: Duration) -> bool {
    let mut inner = shared
        .inner
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let now = now_unix();
    // The absence charge is evaluated on every tick, not only when a session
    // finishes: a machine left running overnight has to charge the day that
    // ended while nobody was looking, and a machine launched after a week away
    // charges it on the first tick after loading.
    if inner.domain.apply_absence(now) {
        inner.publish();
        inner.write_document(path);
    }
    // A walk pays when it comes home, not when it is set out on, so the same
    // tick that charges the day has to notice a road that has been walked.
    if inner.domain.apply_walk_return(now) {
        inner.publish();
        inner.write_document(path);
    }
    // And so does a job: it is hours of the cultivator's day, paid on the clock.
    if inner.domain.apply_work_return(now) {
        inner.publish();
        inner.write_document(path);
    }
    // A bed says how it is doing at its halfway mark rather than under the
    // seed: the same tick finds it late or finds it on time, and pays once.
    if inner.domain.apply_garden_reveal(now) {
        inner.publish();
        inner.write_document(path);
    }
    // The day opens once, whichever tick is the first of it: the morning event
    // is the ladder's one unprompted move, so it hangs off the same tick rather
    // than off a click the user might never make.
    if inner.domain.apply_daily_event(now) {
        inner.publish();
        inner.write_document(path);
    }
    if inner.domain.timer().state() != TimerState::Running {
        drop(inner);
        shared.changed.notify_all();
        return false;
    }
    let completed = inner.domain.advance(elapsed, now).is_some();
    if completed {
        inner.completed_sessions = inner.completed_sessions.saturating_add(1);
    }
    // The remaining time is on screen, so a countdown that moved has to be
    // republished even though it changed nothing on disk.
    inner.publish();
    if completed {
        inner.write_document(path);
    }
    drop(inner);
    shared.changed.notify_all();
    true
}

fn set_worker_state(shared: &Arc<SharedState>, worker: PomodoroWorkerState) {
    let mut inner = shared
        .inner
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    inner.worker_state = worker;
    inner.publish();
    drop(inner);
    shared.changed.notify_all();
}

/// Seconds since the Unix epoch, or zero when the system clock predates it.
///
/// A clock that cannot answer a countdown is not worth failing over: the
/// recorded timestamp only orders completions for the calendar.
fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DEFAULT_FOCUS_MINUTES;

    const TIMEOUT: Duration = Duration::from_secs(5);

    /// A worker woken faster than once a second still owes its seconds: input
    /// publishes a command on every frame, and the countdown must not stop for
    /// as long as the user keeps typing or moving the pointer.
    #[test]
    fn sub_second_wake_ups_add_up_instead_of_being_dropped() {
        let mut carried = Duration::ZERO;
        let mut paid = Duration::ZERO;
        for _ in 0..100 {
            paid += take_whole_seconds(Duration::from_millis(30), &mut carried);
        }
        assert_eq!(paid, Duration::from_secs(3), "3000 ms of 30 ms wake-ups");
        assert_eq!(carried, Duration::ZERO);

        // A hundred wake-ups at 10 ms leave nothing behind either, and a
        // fraction smaller than a second is kept until it completes one.
        let mut carried = Duration::ZERO;
        assert_eq!(
            take_whole_seconds(Duration::from_millis(400), &mut carried),
            Duration::ZERO
        );
        assert_eq!(carried, Duration::from_millis(400));
        assert_eq!(
            take_whole_seconds(Duration::from_millis(700), &mut carried),
            Duration::from_secs(1)
        );
        assert_eq!(carried, Duration::from_millis(100));
    }

    fn wait_for(client: &PomodoroClient, predicate: impl Fn(&PomodoroSnapshot) -> bool) {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            if predicate(&client.snapshot()) {
                return;
            }
            assert!(Instant::now() < deadline, "the worker did not get there");
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn an_unavailable_client_reports_a_stopped_service() {
        let client = PomodoroClient::unavailable();
        assert!(client.is_stopped());
        assert_eq!(
            client.try_publish(PomodoroCommand::Pause),
            Err(PomodoroPublishError::ServiceStopped(PomodoroCommand::Pause))
        );
        assert_eq!(
            client.diagnostics().last_error,
            Some(PomodoroErrorCode::WorkerUnavailable)
        );
        assert_eq!(client.snapshot().worker, PomodoroWorkerState::Stopped);
        assert!(client.snapshot().tasks.is_empty());
        assert_eq!(client.snapshot().focus_minutes, DEFAULT_FOCUS_MINUTES);
    }

    #[test]
    fn a_started_worker_reports_ready_and_accepts_a_new_task() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let path = directory.path().join("pomodoro-tasks.json");
        let service = PomodoroService::start(path).expect("pomodoro service");
        let client = service.client();
        wait_for(&client, |snapshot| {
            snapshot.worker == PomodoroWorkerState::Ready
        });
        client
            .try_publish(PomodoroCommand::CreateTask {
                name: "focus".into(),
                estimated_pomodoros: 2,
            })
            .expect("create the task");
        wait_for(&client, |snapshot| {
            snapshot.tasks.iter().any(|t| t.name == "focus")
        });
        assert!(
            client.snapshot().tasks.iter().any(|t| t.name == "focus"),
            "the created task is present alongside any seeded daily tasks"
        );
        service.shutdown(TIMEOUT).expect("clean shutdown");
        assert_eq!(client.diagnostics().worker, PomodoroWorkerState::Stopped);
    }

    #[test]
    fn a_created_task_reaches_the_document_and_a_reload_finds_it() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let path = directory.path().join("pomodoro-tasks.json");
        let service = PomodoroService::start(path.clone()).expect("pomodoro service");
        let client = service.client();
        client
            .try_publish(PomodoroCommand::CreateTask {
                name: "persisted".into(),
                estimated_pomodoros: 1,
            })
            .expect("create the task");
        client
            .try_publish(PomodoroCommand::SetFocusMinutes { minutes: 30 })
            .expect("change the session length");
        wait_for(&client, |snapshot| {
            !snapshot.tasks.is_empty() && snapshot.focus_minutes == 30
        });
        service.shutdown(TIMEOUT).expect("clean shutdown");
        let reloaded = PomodoroState::load(&path);
        let persisted = reloaded
            .document()
            .tasks
            .iter()
            .find(|t| t.name == "persisted")
            .expect("the persisted task survives the reload");
        assert_eq!(persisted.name, "persisted");
        assert_eq!(reloaded.focus_minutes(), 30);
        // The countdown is not in the document: a reload starts with no session.
        assert_eq!(reloaded.timer(), Timer::Idle);
    }

    #[test]
    fn a_paused_session_does_not_count_down() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let path = directory.path().join("pomodoro-tasks.json");
        let service = PomodoroService::start(path).expect("pomodoro service");
        let client = service.client();
        client
            .try_publish(PomodoroCommand::CreateTask {
                name: "focus".into(),
                estimated_pomodoros: 1,
            })
            .expect("create the task");
        wait_for(&client, |snapshot| {
            snapshot.tasks.iter().any(|t| t.name == "focus")
        });
        let focus_id = client
            .snapshot()
            .tasks
            .iter()
            .find(|t| t.name == "focus")
            .expect("the focus task exists")
            .id;
        client
            .try_publish(PomodoroCommand::StartTask { task_id: focus_id })
            .expect("start the session");
        wait_for(&client, |snapshot| snapshot.timer.is_some());
        client
            .try_publish(PomodoroCommand::Pause)
            .expect("pause the session");
        wait_for(&client, |snapshot| {
            snapshot
                .timer
                .is_some_and(|timer| timer.state == TimerState::Paused)
        });
        let frozen = client
            .snapshot()
            .timer
            .expect("a paused session")
            .remaining_seconds;
        thread::sleep(Duration::from_millis(1_200));
        assert_eq!(
            client
                .snapshot()
                .timer
                .expect("still paused")
                .remaining_seconds,
            frozen,
            "a paused countdown must not move"
        );
        service.shutdown(TIMEOUT).expect("clean shutdown");
    }

    #[test]
    fn a_running_session_publishes_a_lower_remaining_time() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let path = directory.path().join("pomodoro-tasks.json");
        let service = PomodoroService::start(path).expect("pomodoro service");
        let client = service.client();
        client
            .try_publish(PomodoroCommand::CreateTask {
                name: "focus".into(),
                estimated_pomodoros: 1,
            })
            .expect("create the task");
        wait_for(&client, |snapshot| {
            snapshot.tasks.iter().any(|t| t.name == "focus")
        });
        let focus_id = client
            .snapshot()
            .tasks
            .iter()
            .find(|t| t.name == "focus")
            .expect("the focus task exists")
            .id;
        client
            .try_publish(PomodoroCommand::StartTask { task_id: focus_id })
            .expect("start the session");
        wait_for(&client, |snapshot| snapshot.timer.is_some());
        let started = client
            .snapshot()
            .timer
            .expect("a running session")
            .remaining_seconds;
        wait_for(&client, |snapshot| {
            snapshot
                .timer
                .is_some_and(|timer| timer.remaining_seconds < started)
        });
        service.shutdown(TIMEOUT).expect("clean shutdown");
    }
}
