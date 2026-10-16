//! How the pomodoro window looks.
//!
//! Three frames, in the order the questions arrive: the countdown if one is
//! running, the new-task form, and the tasks that can be started. A frame that
//! would be empty says so rather than rendering a gap.

use super::*;
use crate::window::Tokens;
use bongocat_i18n::{format_text, text};
use bongocat_pomodoro::TimerState;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::Input;
use gpui_kit::{IntoElement, WeakEntity, Window, div, px};
// `test_support()` registers elements for the headless window tests; without
// `gpui-kit`'s `test-support` feature it is the identity function.
use gpui_kit::base::TestSupportExt as _;

impl Render for PomodoroView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        if viewport.width <= px(0.) || viewport.height <= px(0.) {
            return div().size_full().into_any_element();
        }
        let locale = self.language.catalog_locale();
        window.set_window_title(text(locale, "pomodoro.window.title"));
        self.sync_component_theme(self.appearance_theme, window, cx);
        let tokens = Tokens::from_theme(cx);
        let view = cx.entity().downgrade();

        div()
            .id("pomodoro-content")
            .size_full()
            .flex()
            .flex_col()
            .gap_4()
            .p_4()
            .text_color(tokens.text)
            .children(self.render_worker_lost(locale, tokens))
            .children(self.render_countdown(locale, tokens))
            .child(self.render_new_task(locale, tokens, cx, &view))
            .child(self.render_session_length(locale, tokens))
            .child(self.render_task_list(locale, tokens, &view))
            .into_any_element()
    }
}

impl PomodoroView {
    /// What the window says when the worker that owns the timer is gone.
    ///
    /// It is stated rather than left silent: a window that lists no tasks and no
    /// countdown while refusing every click would read as an empty task list, and
    /// the user would go looking for the task they thought they had created.
    fn render_worker_lost(&self, locale: &str, tokens: Tokens) -> Vec<gpui_kit::AnyElement> {
        if !view::worker_is_lost(&self.snapshot) {
            return Vec::new();
        }
        vec![
            div()
                .id("pomodoro-worker-lost")
                .text_sm()
                .text_color(tokens.danger)
                .child(text(locale, "pomodoro.worker_lost"))
                .test_support()
                .into_any_element(),
        ]
    }

    /// The countdown, as `mm:ss`, plus whether it is paused.
    fn render_countdown(&self, locale: &str, tokens: Tokens) -> Vec<gpui_kit::AnyElement> {
        let Some(timer) = view::countdown_of(&self.snapshot) else {
            return Vec::new();
        };
        let minutes = timer.remaining_seconds / 60;
        let seconds = timer.remaining_seconds % 60;
        let mut frames = vec![
            div()
                .id("pomodoro-countdown")
                .flex()
                .items_center()
                .justify_center()
                .py_3()
                .rounded_md()
                .border_1()
                .border_color(tokens.border)
                .child(
                    div()
                        .id("pomodoro-countdown-value")
                        // A monospaced digit column is what makes a changing number
                        // readable: a proportional font reflows the whole label every
                        // second as its width changes.
                        .text_3xl()
                        .text_color(tokens.accent)
                        .child(format!("{minutes:02}:{seconds:02}")),
                )
                .test_support()
                .into_any_element(),
        ];
        if timer.state == TimerState::Paused {
            frames.push(
                div()
                    .id("pomodoro-countdown-paused")
                    .text_sm()
                    .text_color(tokens.muted)
                    .child(text(locale, "pomodoro.task.paused"))
                    .test_support()
                    .into_any_element(),
            );
        }
        frames
    }

    /// The session length, as a line under the new-task form.
    ///
    /// It is stated rather than left implied because the expected-session count is
    /// counted in this unit: "4 sessions" means nothing until the user can see
    /// that a session is twenty-five minutes.
    fn render_session_length(&self, locale: &str, tokens: Tokens) -> gpui_kit::AnyElement {
        div()
            .id("pomodoro-session-length")
            .text_xs()
            .text_color(tokens.muted)
            .child(format_text(
                locale,
                "pomodoro.picker.session_length",
                &[("minutes", view::focus_minutes(&self.snapshot).to_string())],
            ))
            .test_support()
            .into_any_element()
    }

    /// The new-task form: a name, an expected session count, and a create button
    /// that is disabled rather than silent when the form cannot be submitted.
    fn render_new_task(
        &self,
        locale: &str,
        tokens: Tokens,
        cx: &App,
        view: &WeakEntity<PomodoroView>,
    ) -> gpui_kit::AnyElement {
        let can_create = !self.pending_name(cx).is_empty() && !view::worker_is_lost(&self.snapshot);
        // The handler outlives this frame, so it owns its own handle rather than
        // borrowing the one this render was given.
        let create_view = view.clone();
        div()
            .id("pomodoro-new-task")
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .text_color(tokens.muted)
                    .child(text(locale, "pomodoro.picker.new_task")),
            )
            .child(
                div()
                    .id("pomodoro-name-field")
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(tokens.muted)
                            .child(text(locale, "pomodoro.picker.task_name")),
                    )
                    .child(Input::new(&self.name_input).w_full())
                    .test_support(),
            )
            .child(
                div()
                    .id("pomodoro-sessions-field")
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(tokens.muted)
                            .child(text(locale, "pomodoro.picker.estimated_sessions")),
                    )
                    .child(Input::new(&self.sessions_input).w_full())
                    .test_support(),
            )
            .child(
                Button::new("pomodoro-create")
                    .label(text(locale, "pomodoro.picker.create"))
                    .with_variant(ButtonVariant::Primary)
                    .disabled(!can_create)
                    .on_click(move |_, _, cx| {
                        let _ = create_view.update(cx, |view, cx| view.create_task(cx));
                    }),
            )
            .test_support()
            .into_any_element()
    }

    /// The tasks, most recently active first, each with a done checkbox.
    ///
    /// The "continue" row is the same list with one task promoted, not a
    /// replacement for it: a user who has four tasks open needs to see the other
    /// three to be able to choose one of those instead. So the promoted row is
    /// drawn above the list and every task still gets its own row below it.
    fn render_task_list(
        &self,
        locale: &str,
        tokens: Tokens,
        view: &WeakEntity<PomodoroView>,
    ) -> gpui_kit::AnyElement {
        let heading = div()
            .id("pomodoro-task-section")
            .text_sm()
            .text_color(tokens.muted)
            .child(text(locale, "pomodoro.task.section"));
        let tasks = view::listed_tasks(&self.snapshot.tasks);
        if tasks.is_empty() {
            return div()
                .id("pomodoro-task-list")
                .flex()
                .flex_col()
                .gap_2()
                .child(heading)
                .child(
                    div()
                        .id("pomodoro-task-none")
                        .text_sm()
                        .text_color(tokens.muted)
                        .child(text(locale, "pomodoro.task.none"))
                        .test_support(),
                )
                .test_support()
                .into_any_element();
        }
        let running_row = self.render_running_task(locale, tokens);
        let continue_row = view::task_to_continue(&self.snapshot.tasks).map(|continued| {
            let task_id = continued.id;
            let button_view = view.clone();
            div()
                .id("pomodoro-continue-row")
                .flex()
                .flex_col()
                .gap_1()
                .pb_2()
                .child(
                    div()
                        .id("pomodoro-continue-label")
                        .text_xs()
                        .text_color(tokens.muted)
                        .child(text(locale, "pomodoro.picker.continue_last"))
                        .test_support(),
                )
                .child(
                    Button::new("pomodoro-continue")
                        .label(continued.name.clone())
                        .on_click(move |_, window, cx| {
                            let _ = button_view.update(cx, |view, cx| view.start_task(task_id, cx));
                            // The session is running on the cat; the picker has
                            // done its job and gets out of the way.
                            window.remove_window();
                        }),
                )
                .test_support()
                .into_any_element()
        });
        let rows: Vec<gpui_kit::AnyElement> = tasks
            .into_iter()
            .map(|task| self.render_task_row(locale, tokens, &task, view))
            .collect();
        div()
            .id("pomodoro-task-list")
            .flex()
            .flex_col()
            .gap_1()
            .min_h_0()
            .overflow_y_scroll()
            .child(heading)
            .children(running_row)
            .children(continue_row)
            .children(rows)
            .test_support()
            .into_any_element()
    }

    /// The task that owns the live session, pinned above the scrolling list.
    ///
    /// It is pinned rather than left in the list because the list scrolls: a
    /// running task the user has to scroll to find is the one thing in this
    /// window they most need to be able to see.
    fn render_running_task(&self, locale: &str, tokens: Tokens) -> Option<gpui_kit::AnyElement> {
        let running = self.snapshot.tasks.iter().find(|task| task.running)?;
        Some(
            div()
                .id("pomodoro-running-task")
                .flex()
                .flex_col()
                .gap_1()
                .pb_2()
                .child(
                    div()
                        .id("pomodoro-task-running")
                        .text_xs()
                        .text_color(tokens.accent)
                        .child(format_text(
                            locale,
                            "pomodoro.task.running_now",
                            &[("name", running.name.clone())],
                        ))
                        .test_support(),
                )
                .child(
                    div()
                        .id(("pomodoro-task-progress", running.id))
                        .text_xs()
                        .text_color(tokens.muted)
                        .child(format_text(
                            locale,
                            "pomodoro.task.progress",
                            &[
                                ("completed", running.completed_pomodoros.to_string()),
                                ("estimated", running.estimated_pomodoros.to_string()),
                            ],
                        )),
                )
                .test_support()
                .into_any_element(),
        )
    }

    /// One task: a done checkbox, its name, when it was created, its progress,
    /// and either "running" or a start button.
    ///
    /// The checkbox publishes the same done command the settings page uses, so
    /// a task can be ticked off right where it is started. A running task's
    /// checkbox is disabled — its done state belongs to the session, and
    /// abandoning is the way a running task ends.
    fn render_task_row(
        &self,
        locale: &str,
        tokens: Tokens,
        task: &bongocat_pomodoro::PomodoroTaskView,
        view: &WeakEntity<PomodoroView>,
    ) -> gpui_kit::AnyElement {
        let done = task.status == bongocat_pomodoro::PomodoroTaskStatus::done;
        let progress = format_text(
            locale,
            "pomodoro.task.progress",
            &[
                ("completed", task.completed_pomodoros.to_string()),
                ("estimated", task.estimated_pomodoros.to_string()),
            ],
        );
        let created = format_text(
            locale,
            "pomodoro.task.created_at",
            &[(
                "time",
                view::format_local_timestamp(
                    task.created_at_unix,
                    bongocat_platform::local_utc_offset_seconds(),
                ),
            )],
        );
        let checkbox_view = view.clone();
        let checkbox_task_id = task.id;
        let checkbox = Checkbox::new(("pomodoro-task-done", task.id))
            .checked(done)
            .disabled(task.running)
            .on_change(move |checked, _, cx| {
                let _ = checkbox_view.update(cx, |view, cx| {
                    view.set_task_done(checkbox_task_id, *checked, cx)
                });
            });
        let trailing: gpui_kit::AnyElement = if task.running {
            div()
                .id(("pomodoro-task-running", task.id))
                .text_xs()
                .text_color(tokens.accent)
                .child(text(locale, "pomodoro.task.running"))
                .test_support()
                .into_any_element()
        } else if !done {
            let view = view.clone();
            let task_id = task.id;
            Button::new(("pomodoro-task-start", task.id))
                .label(text(locale, "pomodoro.task.start"))
                .on_click(move |_, window, cx| {
                    let _ = view.update(cx, |view, cx| view.start_task(task_id, cx));
                    // The session is running on the cat; the picker has
                    // done its job and gets out of the way.
                    window.remove_window();
                })
                .into_any_element()
        } else {
            // A finished task carries no action: the checkbox already says its
            // state, and starting it again is what the "continue" flow is for.
            div().into_any_element()
        };
        div()
            .id(("pomodoro-task-row", task.id))
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .py_2()
            .border_b_1()
            .border_color(tokens.border)
            .child(checkbox)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .text_color(if done { tokens.muted } else { tokens.text })
                            .child(task.name.clone()),
                    )
                    .child(
                        div()
                            .id(("pomodoro-task-progress", task.id))
                            .text_xs()
                            .text_color(tokens.muted)
                            .child(progress),
                    )
                    .child(
                        div()
                            .id(("pomodoro-task-created", task.id))
                            .text_xs()
                            .text_color(tokens.muted)
                            .child(created),
                    ),
            )
            .child(trailing)
            .test_support()
            .into_any_element()
    }
}
