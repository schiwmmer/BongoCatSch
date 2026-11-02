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
) -> Option<gpui_kit::AnyElement> {
    // No guardian, no panel. A hint about breaking through belongs on the page
    // only while there is a wall to break: printed under a realm still being
    // climbed it was a line the user read every day and could do nothing about,
    // and the banner at the top of the page now carries the one case that is
    // worth raising a voice for.
    let guardian = cultivation.guardian.as_ref()?;
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
    // Marked so a frame can be asked whether the fight is on the page.
    Some(panel.test_support().into_any_element())
}

/// The one line the page raises its voice for: the realm is full.
///
/// Drawn at the very top of the page, above the recent events, and in the
/// body's own size one step up rather than the small type every other line
/// uses: it is the only prompt on the page about a thing the user cannot buy
/// their way past, and a hint that has to be looked for is not a hint. It shows
/// when — and only when — the realm is full; a save still climbing is told
/// nothing, because there is nothing yet to tell it.
pub(super) fn breakthrough_banner(
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
) -> Option<gpui_kit::AnyElement> {
    if !cultivation.realm_full {
        return None;
    }
    let line = match &cultivation.guardian {
        Some(guardian) => format_text(
            locale,
            "cultivation.breakthrough.banner",
            &[(
                "stage",
                text(locale, guardian.target.name_key()).to_string(),
            )],
        ),
        // The last realm is full and there is nothing past it: the banner says
        // so rather than promising a door that is not there.
        None => text(locale, "cultivation.breakthrough.max").to_string(),
    };
    Some(
        div()
            .id("cultivation-breakthrough-banner")
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .rounded_md()
            .bg(tokens.overlay)
            .text_base()
            .font_semibold()
            .text_color(tokens.accent)
            .whitespace_normal()
            .child(line)
            .test_support()
            .into_any_element(),
    )
}

/// The walk out of the door, at whichever of the three lengths is chosen.
///
/// The road pays when the walk comes home, so the panel has two faces: at home
/// it offers the three lengths and the button that sets out; out on the road it
/// draws the countdown to the walk's return and the one button that calls it
/// back. A walk that is called back pays nothing — the road is not walked — and
/// the trip home is a third of the time already spent, which is the price of
/// changing one's mind.
///
/// The three lengths are the walk's one lever. A longer walk finds more —
/// every chance it rolls is multiplied by the chip's own figure — and keeps the
/// cultivator out for longer, so the choice is how finely the day's rolls are
/// cut rather than which chip is better. The chosen length is drawn as the
/// selected chip and the countdown below it is that walk's own.
pub(super) fn exploration_panel(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    chosen: ExplorationHours,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let at_home = cultivation.exploration_ready;
    // An expedition is not a walk, but it closes the road the same way: the
    // page draws the walk as closed rather than offering to "return" a walk
    // that was never taken. The gathering's own return lives on the status
    // line, which is where every outing is called home.
    let out_gathering = cultivation.resources.expedition.is_some();
    let mut button = if at_home {
        Button::new("cultivation-explore-attempt")
            .small()
            .label(text(locale, "cultivation.explore.attempt"))
    } else if out_gathering {
        Button::new("cultivation-explore-attempt")
            .small()
            .label(text(locale, "cultivation.explore.attempt"))
            .disabled(true)
    } else {
        let label_key = if cultivation.walk_early {
            "cultivation.explore.returning"
        } else {
            "cultivation.explore.return"
        };
        Button::new("cultivation-explore-return")
            .small()
            .label(text(locale, label_key))
    };
    if at_home {
        button = button.on_click({
            let view = view.clone();
            move |_, _, cx| {
                view.update(cx, |view, cx| {
                    view.ask_cultivation(
                        CultivationDuelSource::Exploration,
                        PomodoroCommand::Explore {
                            hours: ExplorationHours::from_hours(chosen.hours()),
                        },
                        cx,
                    );
                });
            }
        });
    } else if !out_gathering && !cultivation.walk_early {
        // A walk already on its way home is drawn, not offered: a second call
        // would only rewrite a return trip already under way, and a button that
        // looks live but does nothing reads as broken.
        button = button.on_click({
            let view = view.clone();
            move |_, _, cx| {
                view.update(cx, |view, cx| {
                    let _ = view.pomodoro.try_publish(PomodoroCommand::ReturnEarly);
                    cx.notify();
                });
            }
        });
    }
    let status = if at_home {
        text(locale, "cultivation.explore.ready").to_string()
    } else if out_gathering {
        text(locale, "cultivation.explore.busy_gathering").to_string()
    } else {
        let key = if cultivation.walk_early {
            "cultivation.explore.returning_in"
        } else {
            "cultivation.explore.eta"
        };
        cooldown_line(cultivation.exploration_ready_at, key, locale)
    };
    let chips = ExplorationHours::ALL.into_iter().map(move |hours| {
        let selected = hours == chosen;
        let mut chip = div()
            .id(format!("cultivation-explore-hours-{}", hours.hours()))
            .px_2()
            .py_1()
            .rounded_md()
            .text_xs()
            .child(format_text(
                locale,
                "cultivation.explore.hours",
                &[
                    ("hours", hours.hours().to_string()),
                    ("mult", multiplier_text(hours)),
                ],
            ))
            .on_click({
                let view = view.clone();
                move |_, _, cx| {
                    view.update(cx, |view, cx| {
                        view.cultivation_explore_hours = hours.hours();
                        cx.notify();
                    });
                }
            })
            .test_support();
        chip = if selected {
            chip.bg(tokens.overlay)
                .text_color(tokens.accent)
                .font_semibold()
        } else {
            chip.text_color(tokens.muted)
        };
        chip
    });
    // The rules of the road read beside the title rather than behind it: the page
    // used to hide four lines of prose under a hover nobody thought to seek, and
    // a rule nobody reads is a rule the walk keeps breaking.
    div()
        .id("cultivation-exploration")
        .flex()
        .flex_col()
        .gap_1()
        .child(section_header(
            locale,
            "cultivation.explore.title",
            "cultivation.explore.hint",
            tokens,
        ))
        .child(div().flex().flex_row().gap_2().children(chips))
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

/// A length's multiplier as the chip prints it: "1.0", "2.1", "3.2".
fn multiplier_text(hours: ExplorationHours) -> String {
    format!("{:.1}", hours.multiplier_permille() as f32 / 1_000.0)
}

/// The hours and minutes left on a cooldown, as one line.
///
/// A cooldown is read from the instant it ends, which the view already carries;
/// the page owns the clock, so the subtraction happens here rather than in the
/// domain. An ended cooldown reads as zero rather than as a negative, and a
/// cooldown that was never started falls back to the bare key.
pub(super) fn cooldown_line(owed_at: Option<i64>, key: &'static str, locale: &str) -> String {
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
