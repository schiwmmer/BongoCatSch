//! 打工: the board of odd jobs, and the one being worked.
//!
//! A walk pays in goods and a job pays in money, and the board is where the two
//! are weighed against each other: three jobs, each with a grade, a length and a
//! wage, and one of them can be taken. Everything on a row is a number the
//! domain already decided — the page only says what it was told.
//!
//! The wage is one figure rather than a range, so the row can be judged before
//! it is taken; 潜质 is the only thing that moves it after the fact, and the line
//! under the board says what the last job actually came home with.

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
    // The board is a function of the day, so it can be drawn straight from the
    // snapshot the worker published: no draft, no local selection. The one thing
    // the page adds is the re-roll button, and it is priced by the domain rather
    // than by the page — a free re-roll would make a ten-thousandth something a
    // user could simply wait out.
    // The title row carries the re-roll: the toll sits beside the button, both
    // at the row's right edge, so the board's refresh is the first thing a user
    // reads after the title rather than a footnote under the last job.
    let header = div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .child(div().flex_1().min_w_0().child(section_header(
            locale,
            "cultivation.jobs.title",
            "cultivation.jobs.hint",
            tokens,
        )))
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
        );
    let mut panel = div()
        .id("cultivation-jobs")
        .flex()
        .flex_col()
        .gap_1()
        .child(header)
        .children(work_lines(cultivation, locale, tokens));
    let rows = view.clone();
    for job in &cultivation.jobs {
        panel = panel.child(job_row(&rows, job, locale, tokens));
    }
    panel.test_support()
}

/// The job in hand, or what the last one came home with.
///
/// The wage in flight is the figure the board printed, because that is the only
/// honest thing to say about work not yet done: 潜质 has not had its turn yet.
/// What the last one actually paid — windfall and all — is the line that says the
/// crit is real, so it takes the place the job used to hold.
fn work_lines(
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
) -> Vec<gpui_kit::AnyElement> {
    let mut lines: Vec<gpui_kit::AnyElement> = Vec::new();
    if let Some(run) = cultivation.work {
        lines.push(
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
                        ("stones", run.offer().stones().to_string()),
                    ],
                ))
                .test_support()
                .into_any_element(),
        );
        lines.push(
            div()
                .id("cultivation-jobs-due")
                .text_xs()
                .text_color(tokens.muted)
                .child(cooldown_line(
                    Some(run.ends_unix),
                    "cultivation.jobs.due",
                    locale,
                ))
                .test_support()
                .into_any_element(),
        );
    } else if let Some(paid) = cultivation.last_work {
        lines.push(
            div()
                .id("cultivation-jobs-paid")
                .text_xs()
                .text_color(tokens.muted)
                .child(format_text(
                    locale,
                    "cultivation.jobs.paid",
                    &[
                        ("stones", paid.stones.to_string()),
                        ("multiplier", paid.multiplier.to_string()),
                    ],
                ))
                .test_support()
                .into_any_element(),
        );
    }
    lines
}

/// One job: what it is, how long it takes, what it pays, and the button that
/// takes it.
fn job_row(
    view: &Entity<SettingsView>,
    job: &bongocat_pomodoro::JobView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    // The grade is the coloured word at the head of the row, so the line beside
    // it says the three numbers that are not the grade.
    let label = format_text(
        locale,
        "cultivation.jobs.row",
        &[
            ("tier", job.tier.to_string()),
            ("hours", job.hours.to_string()),
            ("stones", job.stones.to_string()),
        ],
    );
    let slot = job.slot;
    // A job the cultivator is already on is not open: the board keeps the row
    // so the page can say 接取中 where the take button would be, and a second
    // click would only be refused.
    let action = if job.working {
        div()
            .id(format!("cultivation-job-taken-{}", job.slot))
            .text_xs()
            .text_color(tokens.muted)
            .child(text(locale, "cultivation.jobs.taken"))
            .test_support()
            .into_any_element()
    } else {
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
            })
            .into_any_element()
    };
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
        .child(action)
        .test_support()
}
