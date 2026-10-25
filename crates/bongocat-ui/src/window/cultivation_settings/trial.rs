//! 锁妖塔: the tower climbed at home, a handful of floors a day.
//!
//! The page calls it 锁妖塔; the code and the catalog still call it `trial`,
//! the name it was built under. The other half of the courtyard — the 试炼 a
//! finished pomodoro pays for — is [`super::proving`], and the two are
//! deliberately apart: this one is a frontier climbed on the day's attempts,
//! that one is a fight bought with banked training.

use super::*;

/// The 锁妖塔, under the walk out of the door.
///
/// The panel is the walk's twin on purpose: one line saying what the next climb
/// is worth, one button, and a status beside it saying whether the tower is
/// open. What it adds is the frontier — the floor the save is standing on — so
/// the button is never a number the user has to remember from yesterday.
///
/// The button is always drawn, and goes quiet with the reason printed beside it
/// rather than disappearing: a tower that closed itself without a word would
/// leave the user wondering whether the page was broken or the day was spent.
/// The click is only wired while a floor is owed, the way the breakthrough
/// button answers to its cooldown — the gate is the domain's, and the page does
/// not offer what it already knows will be refused.
pub(super) fn trial_panel(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let next = cultivation.trial_next_floor;
    let mut button = Button::new("cultivation-trial-attempt")
        .small()
        .label(text(locale, "cultivation.trial.attempt"));
    if cultivation.trial_blocked.is_none() {
        button = button.on_click({
            let view = view.clone();
            move |_, _, cx| {
                view.update(cx, |view, cx| {
                    view.ask_cultivation(CultivationDuelSource::Trial, PomodoroCommand::Tower, cx);
                });
            }
        });
    }
    // At the top of the tower the floor line already says so, and a second copy
    // of the sentence beside the button would be the page repeating itself.
    let status = match cultivation.trial_blocked {
        None => format_text(
            locale,
            "cultivation.trial.attempts_left",
            &[
                ("left", cultivation.trial_attempts_left.to_string()),
                ("total", TRIAL_ATTEMPTS_PER_DAY.to_string()),
            ],
        ),
        Some(TrialBlocked::Top) => String::new(),
        Some(blocked) => text(locale, blocked.key()).to_string(),
    };
    let color = if cultivation.trial_blocked.is_none() {
        tokens.muted
    } else {
        tokens.danger
    };
    // The rules of the tower read beside the title rather than behind it.
    let mut panel = div()
        .id("cultivation-trial")
        .flex()
        .flex_col()
        .gap_1()
        .child(section_header(
            locale,
            "cultivation.trial.title",
            "cultivation.trial.hint",
            tokens,
        ))
        // Marked so a frame can be asked whether the tower is on the page.
        .test_support();
    // What the next climb is: the floor and the warden waiting on it. Once the
    // tower is behind the save there is no warden to name, and the line says so
    // instead of printing the top floor's number one more time.
    let floor_line = if cultivation.trial_blocked == Some(TrialBlocked::Top) {
        text(locale, "cultivation.trial.top").to_string()
    } else {
        format_text(
            locale,
            "cultivation.trial.floor",
            &[
                ("floor", next.to_string()),
                (
                    "power",
                    compact_number(u64::from(cultivation.trial_warden_power)),
                ),
            ],
        )
    };
    panel = panel
        .child(
            div()
                .id("cultivation-trial-floor")
                .text_xs()
                .text_color(tokens.muted)
                .child(floor_line)
                .test_support(),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .id("cultivation-trial-status")
                        .text_xs()
                        .text_color(color)
                        .child(status)
                        .test_support(),
                )
                .child(button),
        );
    // The climb behind the save, when there is one: the floor is the number the
    // user is working on, so the one under it belongs on the panel rather than
    // in a popup that has already closed.
    if cultivation.trial_floor > 0 && cultivation.trial_blocked != Some(TrialBlocked::Top) {
        panel = panel.child(
            div()
                .id("cultivation-trial-cleared")
                .text_xs()
                .text_color(tokens.muted)
                .child(format_text(
                    locale,
                    "cultivation.trial.cleared",
                    &[("floor", cultivation.trial_floor.to_string())],
                ))
                .test_support(),
        );
    }
    panel
}
