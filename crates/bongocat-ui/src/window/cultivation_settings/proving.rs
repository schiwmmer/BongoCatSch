//! 试炼: the fight a finished pomodoro pays for.
//!
//! The panel sits directly under the walk out of the door, because the two are
//! the same kind of decision — go and find something — and differ only in what
//! pays for the trip: a walk is rationed by time, a 试炼 by the training that
//! banked the chance.
//!
//! The button answers to the bank rather than to a clock. The click is always
//! offered: a 试炼 with nothing banked, or the cultivator still out on the
//! road or at a job, is refused by the domain and answered with a toast — the
//! panel does not keep its own copy of the reason, so the click that is
//! offered and the one that is refused can never disagree.

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
    // The click is always wired: whether the road is still out, the job still
    // in hand, or the bank empty, the domain answers with the refusal and the
    // page shows it as a toast. A 试炼 is one door with three reasons, and the
    // panel does not guess which one the click will meet.
    let mut button = Button::new("cultivation-proving-attempt")
        .small()
        .label(text(locale, "cultivation.proving.attempt"));
    button = button.on_click({
        let view = view.clone();
        move |_, _, cx| {
            view.update(cx, |view, cx| {
                view.ask_cultivation(CultivationDuelSource::Proving, PomodoroCommand::Prove, cx);
            });
        }
    });
    // The count is always read against the cap, whether the bank is full, part
    // spent, or empty: "0 / 10" is the honest answer to "can I go?".
    let status = format_text(
        locale,
        "cultivation.proving.chances",
        &[
            ("left", cultivation.proving_tokens.to_string()),
            ("total", cultivation.proving_token_cap.to_string()),
        ],
    );
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
                        .text_color(tokens.muted)
                        .child(status)
                        .test_support(),
                )
                .child(button),
        )
        .test_support()
}
