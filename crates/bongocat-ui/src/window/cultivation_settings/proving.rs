//! 试炼: the fight a finished pomodoro pays for.
//!
//! The panel sits directly under the walk out of the door, because the two are
//! the same kind of decision — go and find something — and differ only in what
//! pays for the trip: a walk is rationed by time, a 试炼 by the training that
//! banked the chance.
//!
//! The button answers to the bank rather than to a clock. A click with nothing
//! banked is not offered: the count beside it says zero, and a button that
//! silently did nothing would read as broken rather than as empty.

use super::explore::cooldown_line;
use super::*;

/// The 试炼 panel, under the walk.
///
/// One line saying how many chances are banked, one button, and the sentence
/// that says what a chance buys, behind the title's hover. The count is drawn
/// against the cap — "3 / 10" — rather than as a bare number: the ceiling is the
/// rule that makes spending a chance a decision.
pub(super) fn proving_panel(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    // Three walls, and the domain answers which one: still on the road, still at
    // a job, or no chance banked. The page reads the answer rather than keeping
    // its own copy of the rule, so the click that is offered and the one that is
    // refused can never disagree.
    let blocked = cultivation.proving_blocked;
    let ready = blocked.is_none();
    let mut button = Button::new("cultivation-proving-attempt")
        .small()
        .label(text(locale, "cultivation.proving.attempt"));
    if ready {
        button = button.on_click({
            let view = view.clone();
            move |_, _, cx| {
                view.update(cx, |view, cx| {
                    view.ask_cultivation(
                        CultivationDuelSource::Proving,
                        PomodoroCommand::Prove,
                        cx,
                    );
                });
            }
        });
    }
    let status = match blocked {
        None => format_text(
            locale,
            "cultivation.proving.chances",
            &[
                ("left", cultivation.proving_tokens.to_string()),
                ("total", cultivation.proving_token_cap.to_string()),
            ],
        ),
        // Out of chances: the count beside it already says zero, so the line
        // says what one buys rather than repeating the number.
        Some(ProvingBlocked::Spent) => text(locale, "cultivation.proving.refuse_empty").to_string(),
        // On the road: the countdown is the sentence here, the way the walk
        // panel shows it, so the two panels read the same clock.
        Some(ProvingBlocked::Out) => cooldown_line(
            cultivation.exploration_ready_at,
            "cultivation.proving.away",
            locale,
        ),
        // At a job: the same refusal the board prints, since it is the same job.
        Some(ProvingBlocked::Working) => {
            text(locale, "cultivation.jobs.refuse_working").to_string()
        }
    };
    let color = if ready { tokens.text } else { tokens.muted };
    // The rules read beside the title rather than behind it.
    div()
        .id("cultivation-proving")
        .flex()
        .flex_col()
        .gap_1()
        .child(section_header(
            locale,
            "cultivation.proving.title",
            "cultivation.proving.hint",
            tokens,
        ))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .id("cultivation-proving-status")
                        .text_xs()
                        .text_color(color)
                        .child(status)
                        .test_support(),
                )
                .child(button),
        )
        .test_support()
}
