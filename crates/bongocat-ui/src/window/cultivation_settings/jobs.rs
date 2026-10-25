//! 打工: the board of odd jobs, and the one being worked.
//!
//! A walk pays in goods and a job pays in money, and the board is where the two
//! are weighed against each other: three jobs, each with a grade, a length and a
//! purse, and one of them can be taken. Everything on a row is a number the
//! domain already decided — the page only says what it was told.

use super::explore::cooldown_line;
use super::*;

/// The board, and the job in flight if there is one.
///
/// The board is a function of the day, so it can be drawn straight from the
/// snapshot the worker published: no draft, no local selection. The one thing
/// the page adds is the re-roll button, and it is priced by the domain rather
/// than by the page — a free re-roll would make a ten-thousandth something a
/// user could simply wait out.
pub(super) fn jobs_panel(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let panel = div()
        .id("cultivation-jobs")
        .flex()
        .flex_col()
        .gap_1()
        .child(section_title(locale, "cultivation.jobs.title", tokens));
    let panel = if let Some(run) = cultivation.work {
        panel
            .child(
                div()
                    .id("cultivation-jobs-working")
                    .text_xs()
                    .text_color(tokens.muted)
                    .child(format_text(
                        locale,
                        "cultivation.jobs.working",
                        &[
                            ("hours", run.hours.to_string()),
                            ("grade", text(locale, run.grade.name_key()).to_string()),
                            ("stones", purse_text(run.offer().stones())),
                        ],
                    ))
                    .test_support(),
            )
            .child(
                div()
                    .id("cultivation-jobs-due")
                    .text_xs()
                    .text_color(tokens.muted)
                    .child(cooldown_line(
                        Some(run.ends_unix),
                        "cultivation.jobs.due",
                        locale,
                    ))
                    .test_support(),
            )
    } else {
        panel.child(
            div()
                .text_xs()
                .text_color(tokens.muted)
                .child(text(locale, "cultivation.jobs.hint")),
        )
    };
    let rows = view.clone();
    let mut panel = panel;
    for job in &cultivation.jobs {
        panel = panel.child(job_row(&rows, job, locale));
    }
    panel
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(div().text_xs().text_color(tokens.muted).child(format_text(
                    locale,
                    "cultivation.jobs.refresh_cost",
                    &[("stones", cultivation.job_refresh_cost.to_string())],
                )))
                .child(
                    Button::new("cultivation-jobs-refresh")
                        .small()
                        .label(text(locale, "cultivation.jobs.refresh"))
                        .on_click({
                            let view = view.clone();
                            move |_, _, cx| {
                                view.update(cx, |view, cx| {
                                    let _ = view.pomodoro.try_publish(PomodoroCommand::RefreshJobs);
                                    cx.notify();
                                });
                            }
                        }),
                ),
        )
        .test_support()
}

/// One job: what it is, how long it takes, what it pays, and the button that
/// takes it.
fn job_row(
    view: &Entity<SettingsView>,
    job: &bongocat_pomodoro::JobView,
    locale: &str,
) -> impl IntoElement {
    // The grade is the coloured word at the head of the row, so the line beside
    // it says the three numbers that are not the grade.
    let label = format_text(
        locale,
        "cultivation.jobs.row",
        &[
            ("tier", job.tier.to_string()),
            ("hours", job.hours.to_string()),
            ("stones", purse_text(job.stones)),
        ],
    );
    let slot = job.slot;
    div()
        .id(format!("cultivation-job-{}", job.slot))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_2()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .font_semibold()
                        .text_color(grade_color(job.grade))
                        .child(text(locale, job.grade.name_key())),
                )
                .child(div().text_xs().child(label)),
        )
        .child(
            Button::new(format!("cultivation-job-take-{}", job.slot))
                .small()
                .label(text(locale, "cultivation.jobs.accept"))
                .on_click({
                    let view = view.clone();
                    move |_, _, cx| {
                        view.update(cx, |view, cx| {
                            let _ = view
                                .pomodoro
                                .try_publish(PomodoroCommand::AcceptJob { slot });
                            cx.notify();
                        });
                    }
                }),
        )
        .test_support()
}

/// A purse as the page says it: one number when it is one number, a range when
/// it is a range.
fn purse_text(purse: (u32, u32)) -> String {
    if purse.0 == purse.1 {
        purse.0.to_string()
    } else {
        format!("{}~{}", purse.0, purse.1)
    }
}
