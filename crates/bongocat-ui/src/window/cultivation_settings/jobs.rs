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
    // The row reads as one line: the trade's own name first — 炼体 is a name,
    // not a recipe — then the grade in its colour, then what the trade asks and
    // how far over or under the cultivator is, the chance it comes off, and the
    // wage. Each piece truncates on its own, so a narrow window clips the wage
    // rather than breaking the row in two.
    let grade_word = div()
        .text_xs()
        .font_semibold()
        .text_color(grade_color(job.grade))
        .child(text(locale, job.grade.name_key()));
    let details = div()
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .min_w_0()
        .child(
            div()
                .text_xs()
                .truncate()
                .child(kind_name(job.kind, locale)),
        )
        .child(grade_word)
        .child(
            div()
                .text_xs()
                .truncate()
                .child(requirement_line(job, locale)),
        )
        .child(div().text_xs().truncate().child(format_text(
            locale,
            "cultivation.jobs.success",
            &[("rate", (job.success_rate / 10).to_string())],
        )))
        .child(div().text_xs().truncate().child(format_text(
            locale,
            "cultivation.jobs.row",
            &[
                ("hours", job.hours.to_string()),
                ("stones", job.stones.to_string()),
            ],
        )));
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
        .child(details)
        .child(action)
        .test_support()
}

/// The name of a trade: what the board calls the work, distinct from the
/// attribute(s) it trains.
fn kind_name(kind: JobKind, locale: &str) -> String {
    text(locale, kind.name_key()).to_string()
}

/// What the job asks of the cultivator, for the attribute(s) it trains, and how
/// far over or under the requirement they are: a plus is a margin already
/// cleared, a minus is a shortfall, both in whole percents.
fn requirement_line(job: &bongocat_pomodoro::JobView, locale: &str) -> String {
    let (first, second) = job.kind.slots();
    match second {
        Some(second) => format_text(
            locale,
            "cultivation.jobs.require_pair",
            &[
                ("first", text(locale, first.key()).to_string()),
                ("a", margin(job.current_first, job.requirement)),
                ("second", text(locale, second.key()).to_string()),
                (
                    "b",
                    margin(
                        job.current_second.unwrap_or(0),
                        job.requirement_second.unwrap_or(0),
                    ),
                ),
            ],
        ),
        None => format_text(
            locale,
            "cultivation.jobs.require_single",
            &[
                ("attribute", text(locale, first.key()).to_string()),
                ("margin", margin(job.current_first, job.requirement)),
            ],
        ),
    }
}

/// How far the cultivator holds an attribute over or under what the job asks,
/// in whole percents, signed: `+5` is five percent clear, `-3` is three short.
fn margin(current: u32, requirement: u32) -> String {
    let ratio = u64::from(current).saturating_mul(1_000) / u64::from(requirement.max(1));
    let over = (ratio as i64 - 1_000) / 10;
    if over == 0 {
        "0".to_string()
    } else if over > 0 {
        format!("+{over}")
    } else {
        format!("{over}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_margin_is_signed_and_in_whole_percents() {
        // At the requirement the margin is zero, and it reads signed either
        // side of it, truncated to whole percents.
        assert_eq!(margin(100, 100), "0");
        assert_eq!(margin(105, 100), "+5");
        assert_eq!(margin(104, 100), "+4");
        assert_eq!(margin(95, 100), "-5");
        assert_eq!(margin(0, 100), "-100");
    }

    #[test]
    fn a_zero_requirement_is_read_as_one() {
        // A 黄阶 job asks the realm's baseline, which a low body can hold at
        // zero; the margin is still read rather than divided by nothing.
        assert_eq!(margin(5, 0), "+400");
    }
}
