//! 打工: the board of odd jobs, and the one being worked.
//!
//! A walk pays in goods and a job pays in money, and the board is where the two
//! are weighed against each other: three jobs, each with a trade, a grade, a
//! length and a wage, and one of them can be taken. Everything on a row is a
//! number the domain already decided — the page only says what it was told.
//!
//! The wage is one figure rather than a range, so the row can be judged before
//! it is taken. What the trade itself decides is the lasting gain: the row says
//! the attribute the job asks for, the requirement the realm puts on it, and the
//! chance the cultivator meets it — read off their own numbers, so the board is
//! a decision about the cultivator as well as about the money.

use super::explore::cooldown_line;
use super::*;
use bongocat_pomodoro::JobKind;

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
/// honest thing to say about work not yet done: the trade has not had its turn
/// yet. What the last one actually paid — and what the trade left behind — is
/// the line that says the gain is real, so it takes the place the job used to
/// hold.
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
                        ("kind", kind_name(run.kind, locale)),
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
        let mut line = format_text(
            locale,
            "cultivation.jobs.paid_base",
            &[
                ("kind", kind_name(paid.kind, locale)),
                ("stones", paid.stones.to_string()),
                ("hours", paid.hours.to_string()),
                ("grade", text(locale, paid.grade.name_key()).to_string()),
                ("rate", (paid.success_rate / 10).to_string()),
            ],
        );
        if let Some((slot, points)) = paid.gain {
            line.push_str(&format_text(
                locale,
                "cultivation.jobs.gain",
                &[
                    ("points", points.to_string()),
                    ("attribute", text(locale, slot.key()).to_string()),
                ],
            ));
        }
        lines.push(
            div()
                .id("cultivation-jobs-paid")
                .text_xs()
                .text_color(tokens.muted)
                .child(line)
                .test_support()
                .into_any_element(),
        );
    }
    lines
}

/// One job: its trade, what it asks, what it pays, and the button that takes it.
fn job_row(
    view: &Entity<SettingsView>,
    job: &bongocat_pomodoro::JobView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    // The grade is the coloured word at the head of the row, and the line beside
    // it names the trade and everything the domain already decided: what it
    // asks, the chance the cultivator meets it, what the trade pays when it
    // does, and the wage — so the board is read as one line and the row is no
    // taller than it was when a job was just money.
    let parts = [
        kind_name(job.kind, locale),
        requirement_line(job, locale),
        format_text(
            locale,
            "cultivation.jobs.success",
            &[("rate", (job.success_rate / 10).to_string())],
        ),
        reward_line(job, locale),
        format_text(
            locale,
            "cultivation.jobs.row",
            &[
                ("tier", job.tier.to_string()),
                ("hours", job.hours.to_string()),
                ("stones", job.stones.to_string()),
            ],
        ),
    ];
    let label = parts.join(" · ");
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
                .min_w_0()
                .child(
                    div()
                        .text_xs()
                        .font_semibold()
                        .text_color(grade_color(job.grade))
                        .child(text(locale, job.grade.name_key())),
                )
                .child(div().min_w_0().text_xs().truncate().child(label)),
        )
        .child(action)
        .test_support()
}

/// The name of a trade: the attribute it trains, or the two a mixed one does.
fn kind_name(kind: JobKind, locale: &str) -> String {
    let (first, second) = kind.slots();
    match second {
        Some(second) => format_text(
            locale,
            "cultivation.jobs.kind_pair",
            &[
                ("first", text(locale, first.key()).to_string()),
                ("second", text(locale, second.key()).to_string()),
            ],
        ),
        None => format_text(
            locale,
            "cultivation.jobs.kind_single",
            &[("attribute", text(locale, first.key()).to_string())],
        ),
    }
}

/// What the job asks of the cultivator, for the attribute(s) it trains.
fn requirement_line(job: &bongocat_pomodoro::JobView, locale: &str) -> String {
    let (first, second) = job.kind.slots();
    match second {
        Some(second) => format_text(
            locale,
            "cultivation.jobs.require_pair",
            &[
                ("first", text(locale, first.key()).to_string()),
                ("a", job.requirement.to_string()),
                ("second", text(locale, second.key()).to_string()),
                ("b", job.requirement_second.unwrap_or(0).to_string()),
            ],
        ),
        None => format_text(
            locale,
            "cultivation.jobs.require_single",
            &[
                ("attribute", text(locale, first.key()).to_string()),
                ("value", job.requirement.to_string()),
            ],
        ),
    }
}

/// What the trade pays when the gain comes up: one point to as many as the grade.
fn reward_line(job: &bongocat_pomodoro::JobView, locale: &str) -> String {
    let (first, second) = job.kind.slots();
    let high = job.grade.index() as u32 + 1;
    let low = 1u32;
    match second {
        Some(second) => format_text(
            locale,
            "cultivation.jobs.reward_pair",
            &[
                ("low", low.to_string()),
                ("high", high.to_string()),
                ("first", text(locale, first.key()).to_string()),
                ("second", text(locale, second.key()).to_string()),
            ],
        ),
        None => format_text(
            locale,
            "cultivation.jobs.reward_single",
            &[
                ("low", low.to_string()),
                ("high", high.to_string()),
                ("attribute", text(locale, first.key()).to_string()),
            ],
        ),
    }
}
