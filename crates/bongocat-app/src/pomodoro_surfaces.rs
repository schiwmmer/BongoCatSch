//! What the app does when a focus session finishes, and what a right-click on
//! the countdown offers.
//!
//! Both live here because both are product actions rather than drawing: the
//! overlay draws the countdown and plays the performance, and it reports a click
//! or a completion; deciding that a click means "pause" and that a completion
//! means "play a chime" is the application's job.

use super::*;

use bongocat_pomodoro::PomodoroCommand;

/// The sequence of the completion whose chime has already been played.
///
/// The sound is played from the frame loop, which runs many times a second, so
/// the sequence that has been played is what stops a session finishing between
/// two polls from chiming once per frame.
#[cfg(target_os = "macos")]
static PLAYED_COMPLETION_SEQUENCE: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// Play the chime for a completion, once per sequence.
///
/// The sound is decoration: a machine with no audio device, a busy output device
/// or a denied audio permission all end the same way, and none of them should
/// stop the performance or the recorded completion that accompany it.
#[cfg(target_os = "macos")]
pub(crate) fn play_pomodoro_complete_sound(_cx: &mut App, sequence: u64) {
    use std::sync::atomic::Ordering;

    if PLAYED_COMPLETION_SEQUENCE.load(Ordering::Acquire) >= sequence {
        return;
    }
    // The sequence is recorded before the sound is asked for, not after: a device
    // that cannot play would otherwise be retried every frame for as long as the
    // completion stayed in the snapshot.
    PLAYED_COMPLETION_SEQUENCE.store(sequence, Ordering::Release);
    bongocat_platform::play_pomodoro_complete_sound();
}

/// Play the chime for a completion on a platform that has no countdown.
///
/// Windows has no pomodoro surface at all, so there is no session that could
/// finish here; the function exists so the frame loop's call site does not need
/// a platform branch.
#[cfg(not(target_os = "macos"))]
pub(crate) fn play_pomodoro_complete_sound(_cx: &mut App, _sequence: u64) {}

/// What a right-click on the countdown offers.
///
/// It is a list of descriptions rather than an enum of commands so the menu is
/// built from it and the mapping back to a timer command cannot drift from the
/// rows the user actually sees.
#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PomodoroMenuEntry {
    pub(crate) id: &'static str,
    pub(crate) label_key: &'static str,
}

/// What a running countdown offers.
#[cfg(target_os = "macos")]
pub(crate) const PAUSE_ROW: PomodoroMenuEntry = PomodoroMenuEntry {
    id: "bongocat.pomodoro-pause",
    label_key: "pomodoro.timer.pause",
};

/// What a paused countdown offers.
///
/// Abandoning is one row below resuming because a paused session is already
/// interrupted: throwing it away is a decision the user is in the middle of
/// making, not an accident one click away from a common action.
#[cfg(target_os = "macos")]
pub(crate) const RESUME_AND_ABANDON_ROWS: [PomodoroMenuEntry; 2] = [
    PomodoroMenuEntry {
        id: "bongocat.pomodoro-resume",
        label_key: "pomodoro.timer.resume",
    },
    PomodoroMenuEntry {
        id: "bongocat.pomodoro-abandon",
        label_key: "pomodoro.timer.abandon",
    },
];

/// The rows a right-click offers, given the countdown's state.
///
/// A running countdown offers only a pause. Offering to abandon a session the
/// user did not mean to touch would put a destructive row next to a common one.
#[cfg(target_os = "macos")]
pub(crate) fn pomodoro_menu_entries(request: OverlayPomodoroMenuRequest) -> Vec<PomodoroMenuEntry> {
    match request {
        OverlayPomodoroMenuRequest::OfferPause => vec![PAUSE_ROW],
        OverlayPomodoroMenuRequest::OfferResumeAndAbandon => RESUME_AND_ABANDON_ROWS.to_vec(),
    }
}

/// The timer command a chosen action means.
#[cfg(target_os = "macos")]
pub(crate) const fn command_for(action: SystemMenuAction) -> Option<PomodoroCommand> {
    match action {
        SystemMenuAction::PausePomodoro => Some(PomodoroCommand::Pause),
        SystemMenuAction::ResumePomodoro => Some(PomodoroCommand::Resume),
        SystemMenuAction::AbandonPomodoro => Some(PomodoroCommand::Abandon),
        _ => None,
    }
}

/// Send the timer a command, reporting a queue that would not take it.
///
/// A pomodoro command is a click on a button, so a full queue means the user is
/// clicking faster than the worker folds in time rather than anything wrong; the
/// click is still recorded as a failure so it shows up in the log rather than
/// silently doing nothing.
#[cfg(target_os = "macos")]
pub(crate) fn send_pomodoro_command(cx: &mut App, command: PomodoroCommand) -> Result<(), String> {
    cx.try_global::<ProductCoordinator>()
        .map(|coordinator| coordinator.pomodoro_client.clone())
        .ok_or_else(|| "pomodoro owner is unavailable".to_owned())?
        .try_publish(command)
        .map_err(|error| error.to_string())
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn a_running_countdown_offers_only_a_pause() {
        assert_eq!(
            pomodoro_menu_entries(OverlayPomodoroMenuRequest::OfferPause),
            [PAUSE_ROW]
        );
    }

    #[test]
    fn a_paused_countdown_offers_resume_and_abandon() {
        assert_eq!(
            pomodoro_menu_entries(OverlayPomodoroMenuRequest::OfferResumeAndAbandon),
            RESUME_AND_ABANDON_ROWS
        );
    }

    #[test]
    fn the_running_menu_does_not_offer_a_destructive_row() {
        // Abandoning throws away a session's work. It would sit one row from
        // Pause, so it must not appear there at all.
        assert!(
            !pomodoro_menu_entries(OverlayPomodoroMenuRequest::OfferPause)
                .iter()
                .any(|row| row.id == "bongocat.pomodoro-abandon"),
            "a running countdown must not offer abandon"
        );
    }

    #[test]
    fn every_row_maps_to_an_action_and_every_action_to_a_command() {
        for row in [
            PAUSE_ROW,
            RESUME_AND_ABANDON_ROWS[0],
            RESUME_AND_ABANDON_ROWS[1],
        ] {
            assert!(!row.label_key.is_empty(), "{:?} has no label", row.id);
        }
        assert_eq!(
            command_for(SystemMenuAction::PausePomodoro),
            Some(PomodoroCommand::Pause)
        );
        assert_eq!(
            command_for(SystemMenuAction::ResumePomodoro),
            Some(PomodoroCommand::Resume)
        );
        assert_eq!(
            command_for(SystemMenuAction::AbandonPomodoro),
            Some(PomodoroCommand::Abandon)
        );
        // An action the menu never offers has no command, so a stray id cannot
        // be turned into something destructive.
        assert_eq!(command_for(SystemMenuAction::Quit), None);
        assert_eq!(command_for(SystemMenuAction::StartPomodoro), None);
    }

    #[test]
    fn the_rows_of_each_menu_are_distinct() {
        let running = pomodoro_menu_entries(OverlayPomodoroMenuRequest::OfferPause);
        let paused = pomodoro_menu_entries(OverlayPomodoroMenuRequest::OfferResumeAndAbandon);
        assert!(
            running.iter().all(|row| !paused.contains(row)),
            "a running countdown must not offer what only a paused one does"
        );
    }
}
