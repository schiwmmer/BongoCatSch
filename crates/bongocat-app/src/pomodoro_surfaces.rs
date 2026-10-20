//! What the app does when a focus session finishes, and what a right-click on
//! the countdown offers.
//!
//! Both live here because both are product actions rather than drawing: the
//! overlay draws the countdown and plays the performance, and it reports a click
//! or a completion; deciding that a click means "pause" and that a completion
//! means "play a chime" is the application's job.

use super::*;

#[cfg(target_os = "macos")]
use bongocat_pomodoro::CultivationView;
use bongocat_pomodoro::PomodoroCommand;

/// The copy the forehead badge and the hover HUD draw, composed here.
///
/// The overlay is a renderer of text, not a catalog reader: it draws whatever
/// string it was handed. Composing that string is the application's job for the
/// same reason the pomodoro menu is — the catalog, the language and the ladder
/// all belong to the product, and a panel that read them itself would give two
/// surfaces two chances to disagree about which realm the user is in.
///
/// The badge gets the realm and layer alone, because it rides the forehead and
/// has room for nothing else. The HUD gets the same line followed by the growth
/// behind it — the growth banked, a slash, and the growth still owed before the
/// next layer opens — over the bar that growth fills. A right-click on the model
/// is meant to answer "how far have I climbed and how far to the next step", so
/// the pomodoro counts are deliberately left out.
///
/// The two growth figures are the pair on one line rather than a total on one
/// line and a remainder on the next: the banked figure is the total, so printing
/// it twice would only be printing the same number in two shapes.
#[cfg(target_os = "macos")]
pub(crate) fn cultivation_copy(
    cultivation: &CultivationView,
    locale: &str,
) -> bongocat_overlay::OverlayCultivationText {
    let stage = bongocat_i18n::text(locale, cultivation.realm.name_key()).to_owned();
    let layer = cultivation.layer.to_string();
    let badge = bongocat_i18n::format_text(
        locale,
        "stats.badge",
        &[
            ("stage", stage.clone()),
            ("layer", layer.clone()),
            ("rank", cultivation.rank.to_string()),
        ],
    );
    // A capped realm has no next layer: the bar is full because the guardian,
    // not the growth, is what stands in the way. Reporting the fraction of the
    // cap here would draw a bar that can never fill.
    let (progress, permille) = match cultivation.next {
        Some(next) => {
            let span = next.saturating_sub(cultivation.floor).max(1);
            let into = cultivation
                .growth
                .saturating_sub(cultivation.floor)
                .min(span);
            let permille = (into.saturating_mul(1_000) / span).min(1_000);
            let progress = bongocat_i18n::format_text(
                locale,
                "stats.hud.progress",
                &[
                    (
                        "value",
                        bongocat_i18n::compact_number(u64::from(cultivation.growth)),
                    ),
                    (
                        "remaining",
                        bongocat_i18n::compact_number(u64::from(
                            next.saturating_sub(cultivation.growth),
                        )),
                    ),
                    ("percent", (permille / 10).to_string()),
                ],
            );
            (progress, permille as u16)
        }
        None => {
            let progress = bongocat_i18n::format_text(
                locale,
                "stats.hud.total",
                &[(
                    "value",
                    bongocat_i18n::compact_number(u64::from(cultivation.growth)),
                )],
            );
            (progress, 1_000)
        }
    };
    let realm = bongocat_i18n::format_text(
        locale,
        "stats.hud.realm",
        &[
            ("stage", stage),
            ("layer", layer),
            ("rank", cultivation.rank.to_string()),
        ],
    );
    bongocat_overlay::OverlayCultivationText {
        badge,
        hud: format!("{realm} · {progress}"),
        hud_progress_permille: permille,
    }
}

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

    #[test]
    fn a_fresh_cultivator_is_named_at_the_first_layer_of_the_first_realm() {
        let view = bongocat_pomodoro::Cultivation::default().view();
        let copy = cultivation_copy(&view, "zh-CN");
        let stage = bongocat_i18n::text("zh-CN", view.realm.name_key());
        assert!(
            copy.badge.starts_with(stage) && copy.badge.contains('1'),
            "badge {:?} does not name {stage} at layer 1",
            copy.badge
        );
        // Nothing has been grown yet, so the bar is empty and the HUD still has
        // a next layer to promise.
        assert_eq!(copy.hud_progress_permille, 0);
        assert!(
            copy.hud.contains(stage),
            "hud {:?} does not name the realm",
            copy.hud
        );
    }

    #[test]
    fn half_a_layer_reports_half_the_bar_and_says_so() {
        let mut view = bongocat_pomodoro::Cultivation::default().view();
        let next = view.next.expect("a fresh cultivator has a next layer");
        let span = next - view.floor;
        view.growth = view.floor + span / 2;
        let copy = cultivation_copy(&view, "zh-CN");
        // Integer halves of an odd span may round down by one part per thousand.
        assert!(
            (499..=500).contains(&copy.hud_progress_permille),
            "half a layer drew {} permille",
            copy.hud_progress_permille
        );
        // The number in the text is the bar's own fraction, not a second
        // computation that could disagree with it.
        let percent = format!("{}%", copy.hud_progress_permille / 10);
        assert!(
            copy.hud.contains(&percent),
            "hud {:?} does not report the {percent} the bar draws",
            copy.hud
        );
    }

    #[test]
    fn a_capped_realm_fills_the_bar_rather_than_drawing_an_unreachable_layer() {
        let mut view = bongocat_pomodoro::Cultivation::default().view();
        view.next = None;
        let copy = cultivation_copy(&view, "zh-CN");
        assert_eq!(copy.hud_progress_permille, 1_000);
    }

    /// The hover panel carries the growth banked and the growth still owed, as a
    /// pair on one line: the banked figure is the lifetime total, so a second
    /// line repeating it would be the same number in two shapes.
    #[test]
    fn the_hover_panel_reports_the_growth_banked_and_the_growth_to_go() {
        let mut view = bongocat_pomodoro::Cultivation::default().view();
        let next = view.next.expect("a fresh cultivator has a next layer");
        let span = next - view.floor;
        view.growth = view.floor + span / 3;
        let copy = cultivation_copy(&view, "zh-CN");
        let pair = format!(
            "{} / {}",
            bongocat_i18n::compact_number(u64::from(view.growth)),
            bongocat_i18n::compact_number(u64::from(next - view.growth)),
        );
        assert!(
            copy.hud.contains(&pair),
            "hud {:?} does not report {pair}",
            copy.hud
        );
        assert!(
            !copy.hud.contains('\n'),
            "hud {:?} is more than one line",
            copy.hud
        );
        // Two cultivators at the same realm but with different growth read their
        // own line, while the badge still names the realm only.
        let mut other = view.clone();
        other.growth = view.floor + span * 2 / 3;
        let other_copy = cultivation_copy(&other, "zh-CN");
        assert_ne!(copy.hud, other_copy.hud);
        assert_eq!(
            copy.badge, other_copy.badge,
            "the badge names the realm only"
        );
    }

    /// A growth figure long enough to push the bar off the panel is printed in
    /// thousands, and the number after the slash is what is still owed rather
    /// than the layer's ceiling.
    #[test]
    fn a_long_growth_figure_is_printed_in_thousands_beside_what_is_owed() {
        let mut view = bongocat_pomodoro::Cultivation::default().view();
        view.floor = 54_000;
        view.growth = 54_683;
        view.next = Some(55_000);
        let copy = cultivation_copy(&view, "zh-CN");
        assert!(
            copy.hud.contains("54.7K / 317"),
            "hud {:?} does not read 54.7K / 317",
            copy.hud
        );
    }
}
