//! The pomodoro settings page: a read-only task list with completion toggles,
//! and a month calendar of finished focus sessions.
//!
//! Both surfaces read the same [`PomodoroClient`] the rest of the app uses, so a
//! task created in the picker or finished by a countdown shows up here without
//! this page being told. Toggling a task's done state publishes the same command
//! the task window would, so there is a single writer for that fact and the
//! settings list never drifts from the picker.
//!
//! The page is one custom [`gpui_kit::component::setting::SettingItem`] element
//! so the list and the calendar can own the full content width instead of
//! sharing it with a label column.

use super::*;
use bongocat_i18n::{format_text, text};
use bongocat_pomodoro::{CalendarDay, PomodoroCommand, PomodoroTaskStatus, PomodoroTaskView};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::Input;

/// Build the search keywords so the page is reachable from the settings search
/// box even though its title lives under the `pomodoro` namespace.
pub(super) fn keywords(language: SettingsLanguage) -> Vec<SharedString> {
    SettingsNavigationPage::Pomodoro.search_keywords(
        language,
        [
            "pomodoro.settings.search_placeholder",
            "pomodoro.settings.tab_list",
            "pomodoro.settings.tab_calendar",
            "pomodoro.settings.no_tasks",
        ],
    )
}

/// The whole pomodoro page body, rendered as one custom settings element.
pub(super) fn content(
    view: Entity<SettingsView>,
    _options: &RenderOptions,
    _window: &mut Window,
    cx: &mut App,
) -> gpui_kit::AnyElement {
    let language = view.read(cx).display_language();
    let locale = language.catalog_locale();
    let tokens = Tokens::from_theme(cx);
    let active_view = view.read(cx).pomodoro_view;

    div()
        .id("pomodoro-settings")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(toggle_row(&view, locale, tokens, active_view))
        .child(match active_view {
            PomodoroSettingsView::List => list_panel(&view, locale, tokens, cx),
            PomodoroSettingsView::Calendar => calendar_panel(&view, locale, tokens, cx),
        })
        .into_any_element()
}

/// The List / Calendar switch.
fn toggle_row(
    view: &Entity<SettingsView>,
    locale: &str,
    tokens: Tokens,
    active_view: PomodoroSettingsView,
) -> impl IntoElement {
    div()
        .id("pomodoro-view-toggle")
        .flex()
        .flex_row()
        .gap_2()
        .child(toggle_chip(
            view,
            locale,
            tokens,
            PomodoroSettingsView::List,
            active_view,
            "pomodoro.settings.tab_list",
        ))
        .child(toggle_chip(
            view,
            locale,
            tokens,
            PomodoroSettingsView::Calendar,
            active_view,
            "pomodoro.settings.tab_calendar",
        ))
}

fn toggle_chip(
    view: &Entity<SettingsView>,
    locale: &str,
    tokens: Tokens,
    target: PomodoroSettingsView,
    active_view: PomodoroSettingsView,
    label_key: &'static str,
) -> impl IntoElement {
    let active = target == active_view;
    let id = match target {
        PomodoroSettingsView::List => "pomodoro-view-list",
        PomodoroSettingsView::Calendar => "pomodoro-view-calendar",
    };
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_md()
        .cursor_pointer()
        .when(active, |this| {
            this.bg(tokens.accent).text_color(tokens.canvas)
        })
        .when(!active, |this| this.text_color(tokens.muted))
        .child(text(locale, label_key))
        .on_click({
            let view = view.clone();
            move |_event, _window, cx| {
                view.update(cx, |view, cx| {
                    view.pomodoro_view = target;
                    cx.notify();
                });
            }
        })
}

/// The task list: a search box and the tasks, each a checkbox that toggles done.
fn list_panel(
    view: &Entity<SettingsView>,
    locale: &str,
    tokens: Tokens,
    cx: &mut App,
) -> gpui_kit::AnyElement {
    let snapshot = view.read(cx).pomodoro.snapshot();
    let search = view
        .read(cx)
        .pomodoro_search_input
        .read(cx)
        .value()
        .to_string();
    let needle = search.trim().to_lowercase();

    let mut tasks: Vec<&PomodoroTaskView> = snapshot
        .tasks
        .iter()
        .filter(|task| needle.is_empty() || task.name.to_lowercase().contains(&needle))
        .collect();
    // Keep the picker's order, but finished tasks sink to the bottom so the work
    // in progress stays where the eye lands first.
    tasks.sort_by_key(|task| task.status == PomodoroTaskStatus::done);

    if tasks.is_empty() {
        return div()
            .id("pomodoro-task-list-empty")
            .text_color(tokens.muted)
            .child(text(locale, "pomodoro.settings.no_tasks"))
            .into_any_element();
    }

    let search_input = view.read(cx).pomodoro_search_input.clone();
    let rows = tasks
        .into_iter()
        .map(|task| task_row(view, locale, tokens, task));

    div()
        .id("pomodoro-task-list")
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .id("pomodoro-task-search")
                .w_full()
                .child(Input::new(&search_input).w_full()),
        )
        .children(rows)
        .into_any_element()
}

/// One task row: a done checkbox, the name, and its session progress.
fn task_row(
    view: &Entity<SettingsView>,
    locale: &str,
    tokens: Tokens,
    task: &PomodoroTaskView,
) -> impl IntoElement {
    let task_id = task.id;
    let done = task.status == PomodoroTaskStatus::done;
    let name = task.name.clone();
    let running = task.running;
    let progress = format_text(
        locale,
        "pomodoro.task.progress",
        &[
            ("completed", task.completed_pomodoros.to_string()),
            ("estimated", task.estimated_pomodoros.to_string()),
        ],
    );

    div()
        .id(format!("pomodoro-task-row-{task_id}"))
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .py_1()
        .child(
            Checkbox::new(format!("pomodoro-task-done-{task_id}"))
                .checked(done)
                .on_change({
                    let view = view.clone();
                    move |checked, _window, cx| {
                        view.update(cx, |view, cx| {
                            let _ = view.pomodoro.try_publish(PomodoroCommand::SetTaskDone {
                                task_id,
                                done: *checked,
                            });
                            view.poll_pomodoro(cx);
                        });
                    }
                }),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(div().child(name))
                .child(div().text_xs().text_color(tokens.muted).child(progress))
                .when(running, |this| {
                    this.child(
                        div()
                            .text_xs()
                            .text_color(tokens.accent)
                            .child(text(locale, "pomodoro.task.running")),
                    )
                }),
        )
}

/// The month calendar of finished focus sessions.
fn calendar_panel(
    view: &Entity<SettingsView>,
    locale: &str,
    tokens: Tokens,
    cx: &mut App,
) -> gpui_kit::AnyElement {
    let offset = bongocat_platform::local_utc_offset_seconds();
    let (year, month) = view.read(cx).pomodoro_display_month;
    let days = view.read(cx).pomodoro.day_totals(offset);
    let grid = bongocat_pomodoro::month_grid(year, month);
    let today = CalendarDay::from_unix(now_unix(), offset);

    let totals = bongocat_pomodoro::month_totals(&days);
    let summary = format_text(
        locale,
        "pomodoro.settings.month_summary",
        &[
            ("sessions", totals.pomodoros.to_string()),
            ("tasks", totals.tasks.to_string()),
        ],
    );

    let weekday_labels: Vec<String> = text(locale, "pomodoro.settings.weekdays")
        .split(',')
        .map(|label| label.trim().to_string())
        .collect();

    let cells = grid
        .cells
        .iter()
        .enumerate()
        .map(|(index, day)| calendar_cell(tokens, day, index, &grid, &days, today));

    let month_title = format_text(
        locale,
        "pomodoro.settings.month_label",
        &[("year", year.to_string()), ("month", format!("{month:02}"))],
    );

    div()
        .id("pomodoro-calendar")
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .id("pomodoro-calendar-header")
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    Button::new("pomodoro-month-prev")
                        .ghost()
                        .label("‹")
                        .tooltip(text(locale, "pomodoro.settings.prev_month"))
                        .on_click({
                            let view = view.clone();
                            move |_event, _window, cx| {
                                view.update(cx, |view, cx| {
                                    view.pomodoro_display_month =
                                        shift_month(view.pomodoro_display_month, -1);
                                    cx.notify();
                                });
                            }
                        }),
                )
                .child(div().font_semibold().child(month_title))
                .child(
                    Button::new("pomodoro-month-next")
                        .ghost()
                        .label("›")
                        .tooltip(text(locale, "pomodoro.settings.next_month"))
                        .on_click({
                            let view = view.clone();
                            move |_event, _window, cx| {
                                view.update(cx, |view, cx| {
                                    view.pomodoro_display_month =
                                        shift_month(view.pomodoro_display_month, 1);
                                    cx.notify();
                                });
                            }
                        }),
                ),
        )
        .child(
            div()
                .id("pomodoro-weekday-header")
                .grid()
                .grid_cols(7)
                .gap_1()
                .children(weekday_labels.into_iter().map(|label| {
                    div()
                        .text_center()
                        .text_xs()
                        .text_color(tokens.muted)
                        .child(label)
                })),
        )
        .child(
            div()
                .id("pomodoro-calendar-grid")
                .grid()
                .grid_cols(7)
                .gap_1()
                .children(cells),
        )
        .child(
            div()
                .id("pomodoro-month-summary")
                .text_xs()
                .text_color(tokens.muted)
                .child(summary),
        )
        .into_any_element()
}

/// One calendar cell, dimming days outside the displayed month and surfacing the
/// finished-session count for days that have one.
fn calendar_cell(
    tokens: Tokens,
    day: &CalendarDay,
    index: usize,
    grid: &bongocat_pomodoro::MonthGrid,
    days: &std::collections::BTreeMap<CalendarDay, bongocat_pomodoro::CalendarDayTotal>,
    today: CalendarDay,
) -> impl IntoElement {
    let in_month =
        index >= grid.leading_days && index < grid.leading_days + grid.days_in_month as usize;
    let civil = day.to_civil();
    let count = days.get(day).map(|total| total.pomodoros).unwrap_or(0);
    let is_today = *day == today;

    div()
        .id(format!(
            "pomodoro-cell-{}-{}-{}",
            civil.year, civil.month, civil.day
        ))
        .p_1()
        .rounded_md()
        .flex()
        .flex_col()
        .items_center()
        .when(in_month, |this| this.text_color(tokens.text))
        .when(!in_month, |this| this.text_color(tokens.muted))
        .when(is_today, |this| {
            this.bg(tokens.accent).text_color(tokens.canvas)
        })
        .child(div().text_xs().child(civil.day.to_string()))
        .when(count > 0, |this| {
            this.child(
                div()
                    .text_xs()
                    .text_color(if is_today {
                        tokens.canvas
                    } else {
                        tokens.accent
                    })
                    .child(count.to_string()),
            )
        })
}

/// The current Unix second, used to mark today on the calendar.
fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

/// Move a `(year, month)` pair by a whole number of months, clamped to the valid
/// 1..=12 month range. A delta that crosses a year boundary wraps both fields.
fn shift_month((year, month): (i32, u32), delta: i32) -> (i32, u32) {
    let total = year * 12 + (month as i32 - 1) + delta;
    let shifted_year = total.div_euclid(12);
    let shifted_month = (total.rem_euclid(12) + 1) as u32;
    (shifted_year, shifted_month)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn month_navigation_wraps_across_years_in_both_directions() {
        assert_eq!(shift_month((2026, 1), -1), (2025, 12));
        assert_eq!(shift_month((2026, 12), 1), (2027, 1));
        assert_eq!(shift_month((2026, 10), 0), (2026, 10));
        assert_eq!(shift_month((2026, 10), -10), (2025, 12));
        assert_eq!(shift_month((2026, 10), 14), (2027, 12));
    }

    #[test]
    fn month_navigation_keeps_a_valid_month() {
        for (year, month) in [(2026, 1), (2026, 12), (2000, 2)] {
            for delta in -30..=30 {
                let (_shifted_year, shifted_month) = shift_month((year, month), delta);
                assert!(
                    (1..=12).contains(&shifted_month),
                    "delta {delta} from {year}-{month:02} produced month {shifted_month}"
                );
            }
        }
    }
}
