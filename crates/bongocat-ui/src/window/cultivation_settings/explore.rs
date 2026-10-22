//! Breakthrough and exploration: the two cooldown-gated buttons and
//! what each of them pays.

use super::*;

/// The guardian at the end of the realm, and the one button that fights it.
///
/// The button answers to the cooldown rather than to the power line: the fight
/// is what decides a breakthrough, and a cultivator who is behind on paper has
/// won one before. The power comparison stays on the page because it is what
/// the report reads back, and because knowing where you stand is half of
/// deciding to spend the attempt.
pub(super) fn breakthrough_panel(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let panel = div()
        .id("cultivation-breakthrough")
        .flex()
        .flex_col()
        .gap_1()
        .child(section_title(
            locale,
            "cultivation.breakthrough.title",
            tokens,
        ));
    let Some(guardian) = cultivation.guardian else {
        let line = if cultivation.realm == Realm::Immortal && cultivation.level == MAX_LEVEL {
            text(locale, "cultivation.breakthrough.max")
        } else {
            text(locale, "cultivation.capped")
        };
        return panel.child(div().text_xs().text_color(tokens.muted).child(line));
    };
    let enough = cultivation.power >= guardian.power;
    let hint_key = if enough {
        "cultivation.breakthrough.ready"
    } else {
        "cultivation.breakthrough.weak"
    };
    let mut button = Button::new("cultivation-breakthrough-attempt")
        .small()
        .label(text(locale, "cultivation.breakthrough.attempt"));
    if cultivation.breakthrough_ready {
        button = button.on_click({
            let view = view.clone();
            move |_, _, cx| {
                view.update(cx, |view, cx| {
                    view.ask_cultivation(
                        CultivationDuelSource::Breakthrough,
                        PomodoroCommand::Breakthrough,
                        cx,
                    );
                });
            }
        });
    }
    let mut panel = panel
        .child(div().text_xs().text_color(tokens.muted).child(format_text(
            locale,
            "cultivation.breakthrough.guardian",
            &[
                (
                    "stage",
                    text(locale, guardian.target.name_key()).to_string(),
                ),
                ("power", guardian.power.to_string()),
            ],
        )))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .text_color(tokens.muted)
                        .child(text(locale, hint_key)),
                )
                .child(button),
        );
    // A loss holds the attempt for twelve hours. The countdown is the honest
    // thing to show: a disabled button with no reason beside it reads as broken.
    if !cultivation.breakthrough_ready {
        panel = panel.child(
            div()
                .id("cultivation-breakthrough-held")
                .text_xs()
                .text_color(tokens.danger)
                .child(cooldown_line(
                    cultivation.breakthrough_ready_at,
                    "cultivation.breakthrough.held",
                    locale,
                ))
                .test_support(),
        );
    }
    panel
}

/// The walk out of the door: one every ten hours, whatever it comes to.
///
/// Gated by the cooldown alone — the thing being rationed is the player's
/// attention, not a cost — and the countdown says when the next one is owed.
/// The button is offered whether or not one is owed: a walk that is not owed
/// says so, and a button that quietly disappeared would leave the user
/// wondering whether the page was broken or the walk was.
pub(super) fn exploration_panel(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let button = Button::new("cultivation-explore-attempt")
        .small()
        .label(text(locale, "cultivation.explore.attempt"))
        .on_click({
            let view = view.clone();
            move |_, _, cx| {
                view.update(cx, |view, cx| {
                    view.ask_cultivation(
                        CultivationDuelSource::Exploration,
                        PomodoroCommand::Explore,
                        cx,
                    );
                });
            }
        });
    let status = if cultivation.exploration_ready {
        text(locale, "cultivation.explore.ready").to_string()
    } else {
        cooldown_line(
            cultivation.exploration_ready_at,
            "cultivation.explore.held",
            locale,
        )
    };
    div()
        .id("cultivation-exploration")
        .flex()
        .flex_col()
        .gap_1()
        .child(section_title(locale, "cultivation.explore.title", tokens))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .id("cultivation-exploration-status")
                        .text_xs()
                        .text_color(tokens.muted)
                        .child(status)
                        .test_support(),
                )
                .child(button),
        )
}

/// The hours and minutes left on a cooldown, as one line.
///
/// A cooldown is read from the instant it ends, which the view already carries;
/// the page owns the clock, so the subtraction happens here rather than in the
/// domain. An ended cooldown reads as zero rather than as a negative, and a
/// cooldown that was never started falls back to the bare key.
fn cooldown_line(owed_at: Option<i64>, key: &'static str, locale: &str) -> String {
    let Some(owed_at) = owed_at else {
        return text(locale, key).to_string();
    };
    let left = owed_at.saturating_sub(now_unix()).max(0);
    let hours = left / 3_600;
    let minutes = (left % 3_600) / 60;
    format_text(
        locale,
        key,
        &[
            ("hours", hours.to_string()),
            ("minutes", minutes.to_string()),
        ],
    )
}
