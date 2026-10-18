//! The pomodoro settings page: a task list with an inline editor, a month
//! calendar of finished focus sessions, and a two-axis trend chart under it.
//!
//! Both surfaces read the same [`PomodoroClient`] the rest of the app uses, so a
//! task created in the picker or finished by a countdown shows up here without
//! this page being told. Toggling a task's done state, editing one, and
//! deleting one publish the same commands the task window would, so there is a
//! single writer for those facts and the settings list never drifts from the
//! picker.
//!
//! The page is one custom [`gpui_kit::component::setting::SettingItem`] element
//! so the list and the calendar can own the full content width instead of
//! sharing it with a label column.

use super::*;
use crate::pomodoro_window::view::{DEFAULT_EXPECTED_SESSIONS, sessions_from_typed};
use bongocat_i18n::{format_text, text};
use bongocat_pomodoro::{
    CalendarDay, PomodoroCommand, PomodoroTaskStatus, PomodoroTaskView, sanitize_task_name,
};
use gpui_kit::component::Sizable as _;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::{ContextMenuExt, PopupMenuItem};
use gpui_kit::{PathBuilder, canvas, fill};

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
    let searching = !needle.is_empty();

    // The search covers every task, finished ones included; the fold below
    // only governs the browse view. Creation order, descending — the same
    // stable order the picker uses, which completing a pomodoro never shuffles.
    let mut matching: Vec<&PomodoroTaskView> = snapshot
        .tasks
        .iter()
        .filter(|task| needle.is_empty() || task.name.to_lowercase().contains(&needle))
        .collect();
    matching.sort_by_key(|task| std::cmp::Reverse(task.created_at_unix));
    let had_matches = !matching.is_empty();
    let (active, finished): (Vec<&PomodoroTaskView>, Vec<&PomodoroTaskView>) = matching
        .into_iter()
        .partition(|task| task.status != PomodoroTaskStatus::done);

    // The search box is part of the frame itself, never conditional on what
    // the list below it shows: a filter that matches nothing used to take the
    // field away with it, leaving "no tasks" as text the user could not search
    // their way out of.
    let search_input = view.read(cx).pomodoro_search_input.clone();
    let mut panel = div()
        .id("pomodoro-task-list")
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .id("pomodoro-task-search")
                .w_full()
                .child(Input::new(&search_input).w_full()),
        );

    // The editor sits above the rows and above the empty state both: creating
    // the very first task is exactly when there are no rows to show.
    if let Some(target) = view.read(cx).pomodoro_editor {
        panel = panel.child(task_editor(view, locale, tokens, target, cx));
    }

    if !had_matches {
        return panel
            .child(
                div()
                    .id("pomodoro-task-list-empty")
                    .text_color(tokens.muted)
                    .child(text(locale, "pomodoro.settings.no_tasks")),
            )
            .into_any_element();
    }

    panel = panel.children(
        active
            .iter()
            .map(|task| task_row(view, locale, tokens, task)),
    );

    if !finished.is_empty() {
        let finished_count = finished.len();
        // A search overrides the fold: its matches are what the user asked to
        // see, whatever group they sit in.
        let expanded = searching || view.read(cx).pomodoro_done_expanded;
        let arrow = if expanded { "▾" } else { "▸" };
        panel = panel.child(
            div()
                .id("pomodoro-done-toggle")
                .flex()
                .flex_row()
                .items_center()
                .gap_1()
                .cursor_pointer()
                .child(text(locale, arrow))
                .child(format_text(
                    locale,
                    "pomodoro.settings.done_group",
                    &[("count", finished_count.to_string())],
                ))
                .on_click({
                    let view = view.clone();
                    move |_, _, cx| {
                        view.update(cx, |view, cx| {
                            view.pomodoro_done_expanded = !view.pomodoro_done_expanded;
                            cx.notify();
                        });
                    }
                }),
        );
        if expanded {
            panel = panel.children(
                finished
                    .iter()
                    .map(|task| task_row(view, locale, tokens, task)),
            );
        }
    }

    panel.into_any_element()
}

/// One task row: a done checkbox, the name, and its session progress as
/// tomatoes — one pale icon per estimated session with a solid one stacked over
/// each finished session, plus extra solid icons past the estimate.
///
/// A right-click anywhere on the row opens the task menu: edit the name and
/// the plan, or delete the task with its history. Both publish the same
/// commands the picker window would, so there is one writer per fact.
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
    // Past the cap the row shows an ellipsis instead of a dozen tomatoes, and
    // the slots cover the larger of the two counts, so finishing more than
    // planned still reads as extra sessions rather than a re-based estimate.
    let overflowing = task.estimated_pomodoros.max(task.completed_pomodoros)
        > u32::try_from(MAX_TOMATO_ICONS).unwrap_or(u32::MAX);
    let (slots, filled) = tomato_slots(task.estimated_pomodoros, task.completed_pomodoros);
    // The menu labels are resolved here rather than inside the menu builder:
    // the builder is stored for the life of the element, and the language it
    // renders in is the one the row was built with.
    let edit_label = text(locale, "pomodoro.settings.task_edit");
    let delete_label = text(locale, "pomodoro.settings.task_delete");

    div()
        .id(format!("pomodoro-task-row-{task_id}"))
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .py_1()
        .context_menu({
            let view = view.clone();
            move |menu, _window, _cx| {
                let edit_view = view.clone();
                let delete_view = view.clone();
                menu.item(
                    PopupMenuItem::new(edit_label)
                        .icon(Icon::new(gpui_kit::assets::IconName::SquarePen))
                        .on_click(move |_, window, cx| {
                            edit_view.update(cx, |view, cx| {
                                view.open_pomodoro_task_editor(
                                    PomodoroEditorTarget::Existing(task_id),
                                    window,
                                    cx,
                                );
                            });
                        }),
                )
                .separator()
                .item(
                    PopupMenuItem::new(delete_label)
                        .icon(Icon::new(gpui_kit::assets::IconName::Trash))
                        .on_click(move |_, _, cx| {
                            delete_view.update(cx, |view, cx| {
                                // The domain drops the task, its history and a
                                // session running for it in one write, so the
                                // row and the calendar lose it together.
                                let _ = view
                                    .pomodoro
                                    .try_publish(PomodoroCommand::DeleteTask { task_id });
                                view.poll_pomodoro(cx);
                            });
                        }),
                )
            }
        })
        .child(
            Checkbox::new(format!("pomodoro-task-done-{task_id}"))
                .checked(done)
                .on_change({
                    let view = view.clone();
                    move |checked, _window, cx| {
                        // Turning a task done is the one direction that
                        // celebrates; un-checking it again stays silent.
                        if *checked {
                            bongocat_platform::play_task_done_sound();
                        }
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
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(2.0))
                        .children((0..slots).map(|index| tomato_icon(index < filled)))
                        .when(overflowing, |this| {
                            this.child(div().text_xs().text_color(tokens.muted).child("…"))
                        }),
                )
                .child(div().text_xs().text_color(tokens.muted).child(format_text(
                    locale,
                    "pomodoro.task.created_at",
                    &[(
                        "time",
                        crate::pomodoro_window::view::format_local_timestamp(
                            task.created_at_unix,
                            bongocat_platform::local_utc_offset_seconds(),
                        ),
                    )],
                )))
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

/// The inline task editor: the two fields a task is made of, and the buttons
/// that store or discard them.
///
/// One surface serves both directions — creating a task and editing one — and
/// the only difference is the title and where the submit publishes. The name
/// field submits on Enter, so a task can be created without the pointer after
/// the first click; the submit button stays disabled while the name is empty,
/// which is the same rule the domain follows when it refuses an unnamed task.
fn task_editor(
    view: &Entity<SettingsView>,
    locale: &str,
    tokens: Tokens,
    target: PomodoroEditorTarget,
    cx: &mut App,
) -> impl IntoElement {
    let editing = matches!(target, PomodoroEditorTarget::Existing(_));
    let title = text(
        locale,
        if editing {
            "pomodoro.settings.task_editor_title"
        } else {
            "pomodoro.settings.new_task"
        },
    );
    let submit_label = text(
        locale,
        if editing {
            "pomodoro.settings.save"
        } else {
            "pomodoro.picker.create"
        },
    );
    let name_input = view.read(cx).pomodoro_editor_name.clone();
    let sessions_input = view.read(cx).pomodoro_editor_sessions.clone();
    // The domain refuses an unnamed task, so the button says so before the
    // command is ever published: the same rule creation follows, read off the
    // same sanitiser rather than a second opinion about what a name is.
    let can_submit = !sanitize_task_name(&name_input.read(cx).value()).is_empty();

    div()
        .id("pomodoro-task-editor")
        .flex()
        .flex_col()
        .gap_2()
        .p_2()
        .rounded_md()
        .border_1()
        .border_color(tokens.border)
        .child(div().font_semibold().child(title))
        .child(
            div()
                .flex()
                .flex_row()
                .gap_2()
                .items_end()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(tokens.muted)
                                .child(text(locale, "pomodoro.picker.task_name")),
                        )
                        .child(Input::new(&name_input).w_full()),
                )
                .child(
                    div()
                        .w(px(96.0))
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(tokens.muted)
                                .child(text(locale, "pomodoro.picker.estimated_sessions")),
                        )
                        .child(Input::new(&sessions_input).w_full()),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap_2()
                .justify_end()
                .child(
                    Button::new("pomodoro-editor-cancel")
                        .ghost()
                        .small()
                        .label(text(locale, "pomodoro.settings.cancel"))
                        .on_click({
                            let view = view.clone();
                            move |_, _, cx| {
                                view.update(cx, |view, cx| view.close_pomodoro_task_editor(cx));
                            }
                        }),
                )
                .child(
                    Button::new("pomodoro-editor-submit")
                        .small()
                        .label(submit_label)
                        .disabled(!can_submit)
                        .on_click({
                            let view = view.clone();
                            move |_, _, cx| {
                                view.update(cx, |view, cx| view.submit_pomodoro_task_editor(cx));
                            }
                        }),
                ),
        )
}

impl SettingsView {
    /// Open the task editor, seeding the fields from the task being edited or
    /// from the defaults for a new one.
    ///
    /// The editor is one at a time and belongs to the view: the rows are
    /// rebuilt every frame and the fields have to survive that — the same
    /// reason the model rename keeps its draft on the view. Opening it also
    /// returns the page to the list, so the fields are never open on the
    /// calendar subview that does not render them.
    pub(super) fn open_pomodoro_task_editor(
        &mut self,
        target: PomodoroEditorTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pomodoro_view = PomodoroSettingsView::List;
        let (name, sessions) = match target {
            PomodoroEditorTarget::New => (String::new(), DEFAULT_EXPECTED_SESSIONS),
            PomodoroEditorTarget::Existing(task_id) => self
                .pomodoro
                .snapshot()
                .tasks
                .iter()
                .find(|task| task.id == task_id)
                .map(|task| (task.name.clone(), task.estimated_pomodoros))
                .unwrap_or((String::new(), DEFAULT_EXPECTED_SESSIONS)),
        };
        self.pomodoro_editor_name
            .update(cx, |input, cx| input.set_value(name, window, cx));
        self.pomodoro_editor_sessions.update(cx, |input, cx| {
            input.set_value(sessions.to_string(), window, cx)
        });
        self.pomodoro_editor = Some(target);
        cx.notify();
    }

    /// Close the editor without storing anything.
    pub(super) fn close_pomodoro_task_editor(&mut self, cx: &mut Context<Self>) {
        if self.pomodoro_editor.take().is_some() {
            cx.notify();
        }
    }

    /// Store the editor's fields and close it.
    ///
    /// A new task publishes the same `CreateTask` the picker window does; an
    /// edit publishes `UpdateTask`, which rewrites the name and the plan while
    /// the finished sessions stay where they are. An empty name is refused by
    /// the domain exactly as creation refuses it — the guard here keeps the
    /// editor closed in that case rather than leaving a blank row behind.
    pub(super) fn submit_pomodoro_task_editor(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.pomodoro_editor.take() else {
            return;
        };
        let name = sanitize_task_name(&self.pomodoro_editor_name.read(cx).value());
        let estimated_pomodoros =
            sessions_from_typed(&self.pomodoro_editor_sessions.read(cx).value());
        if name.is_empty() {
            cx.notify();
            return;
        }
        let command = match target {
            PomodoroEditorTarget::New => PomodoroCommand::CreateTask {
                name,
                estimated_pomodoros,
            },
            PomodoroEditorTarget::Existing(task_id) => PomodoroCommand::UpdateTask {
                task_id,
                name,
                estimated_pomodoros,
            },
        };
        let _ = self.pomodoro.try_publish(command);
        self.poll_pomodoro(cx);
    }
}

/// The most session icons one task row draws before the rest are summarised as
/// an ellipsis: a row of tomatoes is a shape, not a bar chart, and past this
/// many the shape is gone anyway.
const MAX_TOMATO_ICONS: usize = 6;

/// The outlined tomato icon: a session the task estimates but has not finished.
const POMODORO_OUTLINE_ICON: &str = "pomodoro/tomato-outline.png";
/// The filled tomato icon: a session the task has finished.
const POMODORO_FILLED_ICON: &str = "pomodoro/tomato-filled.png";

/// How many icon slots a task's progress draws, and how many of them are solid.
///
/// The slots cover the larger of the two counts, so finishing more than planned
/// still shows the extra sessions rather than silently re-basing the row. The
/// filled count never exceeds the slots, and neither exceeds
/// [`MAX_TOMATO_ICONS`] — past that the caller shows the ellipsis instead.
fn tomato_slots(estimated: u32, completed: u32) -> (usize, usize) {
    let cap = u32::try_from(MAX_TOMATO_ICONS).unwrap_or(u32::MAX);
    let slots = estimated.max(completed).min(cap) as usize;
    let filled = completed.min(cap) as usize;
    (slots, filled)
}

/// One tomato icon: the product's own artwork, a filled fruit for a finished
/// session and the outlined one for a session that is only estimated.
///
/// A finished slot reads as the solid fruit standing over its outlined
/// estimate, so the two never compete for the pixel. The icons ship beside
/// the executable (or in the repository's resources during a build-tree run),
/// which [`bongocat_platform::bundled_resource`] resolves.
fn tomato_icon(filled: bool) -> impl IntoElement {
    img(bongocat_platform::bundled_resource(if filled {
        POMODORO_FILLED_ICON
    } else {
        POMODORO_OUTLINE_ICON
    }))
    .w(px(14.0))
    .h(px(14.0))
    .flex_shrink_0()
}

/// The month calendar of finished focus sessions, with a trend chart under it.
fn calendar_panel(
    view: &Entity<SettingsView>,
    locale: &str,
    tokens: Tokens,
    cx: &mut App,
) -> gpui_kit::AnyElement {
    let offset = bongocat_platform::local_utc_offset_seconds();
    let (year, month) = view.read(cx).pomodoro_display_month;
    let days = view.read(cx).pomodoro.day_totals(offset);
    let activities = view.read(cx).pomodoro.day_activities(offset);
    let grid = bongocat_pomodoro::month_grid(year, month);
    let today = CalendarDay::from_unix(now_unix(), offset);
    let selected_day = view.read(cx).pomodoro_selected_day;

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

    // Each cell carries only the day's finished-session count, summed from the
    // same per-task activities the strip under the grid lists; the names moved
    // out of the cells so a long one can wrap instead of crowding a date.
    let cells = grid.cells.iter().enumerate().map(|(index, day)| {
        let day_pomodoros = activities
            .get(day)
            .map(|day_activities| {
                day_activities.iter().fold(0u32, |total, activity| {
                    total.saturating_add(activity.pomodoros)
                })
            })
            .unwrap_or(0);
        calendar_cell(
            view,
            tokens,
            day,
            index,
            &grid,
            day_pomodoros,
            today,
            selected_day,
        )
    });

    let month_title = format_text(
        locale,
        "pomodoro.settings.month_label",
        &[("year", year.to_string()), ("month", format!("{month:02}"))],
    );

    // The task names behind the displayed month, each listed once in the order
    // it first appears. The cells carry counts only, so this strip answers
    // "what was this month spent on": a long name wraps with the others, and a
    // single one truncates rather than widening the row past the window.
    let mut month_task_names: Vec<String> = Vec::new();
    for (index, day) in grid.cells.iter().enumerate() {
        let in_month =
            index >= grid.leading_days && index < grid.leading_days + grid.days_in_month as usize;
        if !in_month {
            continue;
        }
        if let Some(day_activities) = activities.get(day) {
            for activity in day_activities {
                if !month_task_names.contains(&activity.task_name) {
                    month_task_names.push(activity.task_name.clone());
                }
            }
        }
    }
    let month_tasks_panel = (!month_task_names.is_empty()).then(|| {
        div()
            .id("pomodoro-month-tasks")
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(tokens.muted)
                    .child(text(locale, "pomodoro.settings.month_tasks")),
            )
            .child(
                div()
                    .id("pomodoro-month-task-names")
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap_1()
                    .children(
                        month_task_names
                            .into_iter()
                            .enumerate()
                            .map(|(index, name)| {
                                div()
                                    .id(format!("pomodoro-month-task-{index}"))
                                    .px_1()
                                    .rounded_sm()
                                    .text_xs()
                                    .text_color(tokens.muted)
                                    .max_w(px(180.0))
                                    .min_w_0()
                                    .truncate()
                                    .child(name)
                            }),
                    ),
            )
    });

    // The picked day's own tasks, above the month strip: the same "name ×N"
    // read the cells carried before the count-only pass, now answering "what
    // did the day I clicked actually spend on". A day without sessions says
    // so in words instead of showing an empty box, and a future day can never
    // be picked, so an empty state here means a quiet day, not a bug.
    let day_detail_panel = selected_day.map(|day| {
        let day_activities = activities.get(&day);
        let civil = day.to_civil();
        let title = format!(
            "{:02}-{:02} {}",
            civil.month,
            civil.day,
            text(locale, "pomodoro.settings.day_tasks")
        );
        div()
            .id("pomodoro-day-tasks")
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .font_semibold()
                    .text_color(tokens.text)
                    .child(title),
            )
            .child(
                div()
                    .id("pomodoro-day-task-names")
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap_1()
                    .children(day_activities.into_iter().flatten().map(|activity| {
                        div()
                            .id(format!("pomodoro-day-task-{}", activity.task_name))
                            .px_1()
                            .rounded_sm()
                            .text_xs()
                            .text_color(tokens.muted)
                            .child(format!("{} ×{}", activity.task_name, activity.pomodoros))
                    })),
            )
            .when(day_activities.is_none_or(|list| list.is_empty()), |this| {
                this.child(
                    div()
                        .text_xs()
                        .text_color(tokens.muted)
                        .child(text(locale, "pomodoro.settings.day_tasks_empty")),
                )
            })
    });

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
        .children(day_detail_panel)
        .children(month_tasks_panel)
        .child(
            div()
                .id("pomodoro-month-summary")
                .text_xs()
                .text_color(tokens.muted)
                .child(summary),
        )
        .child(trend_chart(view, locale, tokens, cx))
        .into_any_element()
}

/// The trend chart under the calendar: a span toggle, a plot with two vertical
/// axes, and the date ticks under it.
///
/// The two series carry different units — focus minutes (sessions x the session
/// length) on the left axis, tasks marked finished on the right — so each keeps
/// its own scale from zero up, and a flat week reads as flat on both instead of
/// two shapes that both fill the plot. Days without activity are zero-height
/// points, which is what makes a flat week read as flat instead of disappearing.
fn trend_chart(
    view: &Entity<SettingsView>,
    locale: &str,
    tokens: Tokens,
    cx: &mut App,
) -> gpui_kit::AnyElement {
    let days_span = view.read(cx).pomodoro_chart_days;
    let offset = bongocat_platform::local_utc_offset_seconds();
    let snapshot = view.read(cx).pomodoro.snapshot();
    let totals = view.read(cx).pomodoro.day_totals(offset);
    let focus_minutes = snapshot.focus_minutes;
    let today = CalendarDay::from_unix(now_unix(), offset);
    // The window ends on the picked day when one is selected and still in the
    // past, so clicking a calendar cell rewinds the whole chart to end on it;
    // an absent selection — or a stale pick that midnight has since passed —
    // ends on today, which is what the page opens with.
    let end_day = chart_end_day(view.read(cx).pomodoro_selected_day, today);

    // Oldest day first, so the polyline runs left to right into the end day.
    // Every finished task lands on exactly one local day, matched by the same
    // offset rule the totals use.
    let mut minutes: Vec<u32> = Vec::with_capacity(days_span);
    let mut finished_tasks: Vec<u32> = Vec::with_capacity(days_span);
    let mut day_labels: Vec<String> = Vec::with_capacity(days_span);
    for back in (0..days_span).rev() {
        let day = CalendarDay::from_day_number(end_day.day_number() - back as i64);
        let pomodoros = totals.get(&day).map(|total| total.pomodoros).unwrap_or(0);
        minutes.push(pomodoros * focus_minutes);
        finished_tasks.push(
            u32::try_from(
                snapshot
                    .tasks
                    .iter()
                    .filter(|task| {
                        task.completed_at_unix
                            .is_some_and(|at| CalendarDay::from_unix(at, offset) == day)
                    })
                    .count(),
            )
            .unwrap_or(u32::MAX),
        );
        let civil = day.to_civil();
        day_labels.push(format!("{:02}-{:02}", civil.month, civil.day));
    }

    let count = days_span.max(1);
    // One ceiling per series, each starting at zero: the axis a reader trusts is
    // the one whose zero sits on the baseline, and a nice ceiling keeps the tick
    // labels round numbers rather than the exact maximum.
    let minutes_max = nice_axis_max(minutes.iter().copied().max().unwrap_or(0));
    let finished_max = nice_axis_max(finished_tasks.iter().copied().max().unwrap_or(0));
    let ticks = date_tick_indices(count);
    let tick_labels: Vec<String> = ticks
        .iter()
        .map(|&index| day_labels[index].clone())
        .collect();
    let tick_spans = date_tick_spans(count, &ticks);

    let chart_height = px(150.0);
    // The two series carry different units — focus minutes on the left axis,
    // finished tasks on the right — so each keeps its own scale from zero up.
    // Red for minutes and blue for tasks is what tells them apart at a
    // glance, in the plot and in the legend under the title alike; theme
    // tokens would read the pair as one hierarchy instead of two series.
    let minutes_color = MINUTES_SERIES_COLOR;
    let finished_color = FINISHED_SERIES_COLOR;
    let border_color = tokens.border;

    let plot = canvas(
        move |_bounds, _, _| TrendPlot {
            minutes,
            finished: finished_tasks,
            count,
            date_ticks: ticks,
            minutes_max,
            finished_max,
            minutes_color,
            finished_color,
            border_color,
        },
        move |bounds, plot, window, _| {
            // The axis columns reserve AXIS_UNIT_HEIGHT + AXIS_UNIT_GAP for their
            // unit label, then spread three tick slots over the rest; the
            // gridlines sit at those slots' centres, which is what keeps a tick
            // label and its line at the same height.
            // `bounds` and every paint call below are in WINDOW coordinates:
            // the x side reads `bounds.left()` directly, so the y side must
            // start from `bounds.top()` too — a bare local offset would paint
            // the whole plot near the window's top edge, straight over the
            // calendar above the chart.
            let tick_top = bounds.top() + AXIS_UNIT_HEIGHT + AXIS_UNIT_GAP;
            let tick_area = bounds.size.height - AXIS_UNIT_HEIGHT - AXIS_UNIT_GAP;
            let gridline_y = |slot: usize| tick_top + tick_area * (slot as f32 + 0.5) / 3.0;
            let top_y = gridline_y(0);
            let mid_y = gridline_y(1);
            let bottom_y = gridline_y(2);
            let left = bounds.left();
            let right = bounds.right();
            let width = bounds.size.width;

            // The frame the two series sit in: a box reads as axes with zero
            // labels, where a lone baseline reads as decoration.
            for (x, y, w, h) in [
                (left, top_y, width, px(1.0)),
                (left, bottom_y, width, px(1.0)),
                (left, top_y, px(1.0), bottom_y - top_y),
                (right - px(1.0), top_y, px(1.0), bottom_y - top_y),
            ] {
                window.paint_quad(fill(
                    Bounds::new(point(x, y), size(w, h)),
                    plot.border_color,
                ));
            }
            // A halfway gridline, so both scales read as numbers rather than as
            // decoration.
            window.paint_quad(fill(
                Bounds::new(point(left, mid_y), size(width, px(1.0))),
                plot.border_color,
            ));

            let step = width / plot.count as f32;
            let point_x = |index: usize| left + step * index as f32 + step / 2.0;

            // A tick under every labelled day, drawn at the data point itself so
            // the date below and the point above share an x.
            for &index in &plot.date_ticks {
                let x = point_x(index);
                window.paint_quad(fill(
                    Bounds::new(point(x, bottom_y), size(px(1.0), px(3.0))),
                    plot.border_color,
                ));
            }

            for (series, axis_max, color) in [
                (plot.minutes, plot.minutes_max, plot.minutes_color),
                (plot.finished, plot.finished_max, plot.finished_color),
            ] {
                let mut builder = PathBuilder::stroke(px(2.0));
                for (index, value) in series.iter().enumerate() {
                    // Each series scales against its own axis: a value at its
                    // ceiling sits on the top gridline, never above the frame.
                    let height = ((*value as f32) / axis_max).clamp(0.0, 1.0);
                    let y = top_y + (bottom_y - top_y) * (1.0 - height);
                    let point = point(point_x(index), y);
                    if index == 0 {
                        builder.move_to(point);
                    } else {
                        builder.line_to(point);
                    }
                }
                if let Ok(path) = builder.build() {
                    window.paint_path(path, color);
                }
            }
        },
    )
    // The canvas owns no content size of its own, so without an explicit
    // size it lays out at zero and every quad and path above paints into an
    // invisible bounds — the axes (plain text) survive, the plot does not.
    .size_full();

    let toggle_chip = |label: &'static str, days: usize, id: &'static str| {
        let active = days_span == days;
        let view = view.clone();
        div()
            .id(id)
            .px_2()
            .py_1()
            .rounded_md()
            .cursor_pointer()
            .text_xs()
            .when(active, |this| {
                this.bg(tokens.accent).text_color(tokens.canvas)
            })
            .when(!active, |this| this.text_color(tokens.muted))
            .child(text(locale, label))
            .on_click(move |_, _, cx| {
                view.update(cx, |view, cx| {
                    view.pomodoro_chart_days = days;
                    cx.notify();
                });
            })
    };

    div()
        .id("pomodoro-trend")
        .flex()
        .flex_col()
        .gap_2()
        .pt_2()
        .child(
            div()
                .id("pomodoro-trend-header")
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .child(toggle_chip(
                            "pomodoro.settings.chart_days_7",
                            7,
                            "pomodoro-trend-7",
                        ))
                        .child(toggle_chip(
                            "pomodoro.settings.chart_days_30",
                            30,
                            "pomodoro-trend-30",
                        )),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .text_xs()
                        // Each swatch carries its own series colour, the same
                        // one the plot paints with, so the legend and the
                        // lines can never drift apart. The swatch is wrapped
                        // in its own div because a `text_color` set on the
                        // row would recolour the label beside it too.
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_1()
                                .child(div().text_color(MINUTES_SERIES_COLOR).child("▬"))
                                .child(text(locale, "pomodoro.settings.chart_minutes")),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_1()
                                .child(div().text_color(FINISHED_SERIES_COLOR).child("▬"))
                                .child(text(locale, "pomodoro.settings.chart_done")),
                        ),
                ),
        )
        .child(
            div()
                .id("pomodoro-trend-plot")
                .flex()
                .flex_row()
                .gap_1()
                .h(chart_height)
                .child(axis_column(
                    "pomodoro.settings.axis_minutes",
                    minutes_max,
                    locale,
                    tokens,
                ))
                .child(div().flex_1().h_full().child(plot))
                .child(axis_column(
                    "pomodoro.settings.axis_tasks",
                    finished_max,
                    locale,
                    tokens,
                )),
        )
        .child(
            div()
                .id("pomodoro-trend-labels")
                .flex()
                .flex_row()
                .text_xs()
                .text_color(tokens.muted)
                // The date row spans the plot area only: the leading and trailing
                // spacers stand in for the two axis columns, so a date sits under
                // its own point rather than under an axis.
                .child(div().w(AXIS_WIDTH + AXIS_GAP).flex_shrink_0().child(""))
                .child(
                    div().flex_1().flex().flex_row().children(
                        tick_spans
                            .into_iter()
                            .zip(tick_labels)
                            .map(|(span, label)| {
                                div()
                                    .flex_grow(span as f32)
                                    .flex_shrink_0()
                                    .text_center()
                                    .child(label)
                            }),
                    ),
                )
                .child(div().w(AXIS_WIDTH + AXIS_GAP).flex_shrink_0().child("")),
        )
        .into_any_element()
}

/// The height of the unit label row above an axis' tick slots.
const AXIS_UNIT_HEIGHT: Pixels = px(14.0);
/// The gap between an axis' unit label and its first tick slot.
const AXIS_UNIT_GAP: Pixels = px(4.0);
/// The width of one vertical axis column.
const AXIS_WIDTH: Pixels = px(30.0);
/// The gap between an axis column and the plot. The date row's spacers use the
/// same measure so a date centres on its point rather than on an axis.
const AXIS_GAP: Pixels = px(4.0);

/// The trend chart's focus-minutes series: red.
///
/// A fixed pair rather than theme tokens: "accent" and "muted" read as one
/// hierarchy, and the chart's whole job is to keep two units apart.
const MINUTES_SERIES_COLOR: Hsla = gpui_kit::hsla(0.0, 0.78, 0.55, 1.0);

/// The trend chart's finished-tasks series: blue.
const FINISHED_SERIES_COLOR: Hsla = gpui_kit::hsla(0.60, 0.72, 0.55, 1.0);

/// Both series the trend plot paints, each with its own axis ceiling.
///
/// The values travel into the paint pass unnormalized because their units
/// differ: scaling each to its own maximum would draw two shapes that both fill
/// the plot whatever the numbers, which is exactly the read the axes correct.
struct TrendPlot {
    minutes: Vec<u32>,
    finished: Vec<u32>,
    count: usize,
    date_ticks: Vec<usize>,
    minutes_max: f32,
    finished_max: f32,
    minutes_color: Hsla,
    finished_color: Hsla,
    border_color: Hsla,
}

/// One vertical axis of the trend plot: the unit on top, then the ceiling, the
/// halfway value and zero spread over the three tick slots the canvas draws its
/// gridlines at. The reserved unit height and the gap are shared with the canvas
/// through [`AXIS_UNIT_HEIGHT`] and [`AXIS_UNIT_GAP`].
fn axis_column(
    unit_key: &'static str,
    axis_max: f32,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let tick = |value: f32| {
        div().flex_1().flex().items_center().justify_center().child(
            div()
                .text_xs()
                .text_color(tokens.muted)
                .child(axis_tick_label(value)),
        )
    };
    div()
        .w(AXIS_WIDTH)
        .flex_shrink_0()
        .flex()
        .flex_col()
        .gap(AXIS_GAP)
        .child(
            div()
                .h(AXIS_UNIT_HEIGHT)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_xs()
                        .text_color(tokens.muted)
                        .child(text(locale, unit_key)),
                ),
        )
        .child(tick(axis_max))
        .child(tick(axis_max / 2.0))
        .child(tick(0.0))
}

/// The ceiling a series' axis is drawn to: the next nice number — 1, 2 or 5
/// times a power of ten — at or above the series maximum, and never below one.
///
/// A ceiling of the exact maximum puts the top tick at the very edge of the
/// plot and reads as clipped; a nice one leaves headroom and round labels.
fn nice_axis_max(max: u32) -> f32 {
    let max = f64::from(max.max(1));
    let exponent = max.log10().floor();
    let fraction = max / 10f64.powf(exponent);
    let nice = if fraction <= 1.0 {
        1.0
    } else if fraction <= 2.0 {
        2.0
    } else if fraction <= 5.0 {
        5.0
    } else {
        10.0
    };
    (nice * 10f64.powf(exponent)) as f32
}

/// How a tick value reads: whole numbers without a decimal tail, halves with
/// exactly one — "25" and "12.5", never "12.50".
fn axis_tick_label(value: f32) -> String {
    let rounded = value.round();
    if (value - rounded).abs() < f32::EPSILON {
        format!("{rounded:.0}")
    } else {
        format!("{value:.1}")
    }
}

/// Which day indices carry a date tick: every day over a short span, every fifth
/// day plus the last over a long one. Thirty "10-16" labels do not fit under the
/// plot; six of them do, and the last day is always one of them.
fn date_tick_indices(count: usize) -> Vec<usize> {
    if count <= 8 {
        return (0..count).collect();
    }
    let mut ticks: Vec<usize> = (0..count).step_by(5).collect();
    if ticks.last().copied() != Some(count - 1) {
        ticks.push(count - 1);
    }
    ticks
}

/// How much of the date row each tick's label takes: the span between the
/// midpoints to its neighbours, so a label centres on its own day's point
/// rather than on an even division of the row.
fn date_tick_spans(count: usize, ticks: &[usize]) -> Vec<usize> {
    ticks
        .iter()
        .enumerate()
        .map(|(position, &tick)| {
            let left = if position == 0 {
                0
            } else {
                (ticks[position - 1] + tick + 1) / 2
            };
            let right = if position + 1 == ticks.len() {
                count
            } else {
                (tick + ticks[position + 1] + 1) / 2
            };
            right.saturating_sub(left).max(1)
        })
        .collect()
}

/// One calendar cell: the day number and the number of focus sessions finished
/// that day — the count, never the task names, which moved to the strip under
/// the grid where a long name can wrap instead of crowding a cell. Days outside
/// the displayed month are dimmed and today is highlighted.
///
/// A cell is a button: clicking one selects that day (a future day refuses,
/// since nothing can have happened there yet), the selection draws a ring, and
/// clicking the selected cell again clears it back to "chart ends today".
fn calendar_cell(
    view: &Entity<SettingsView>,
    tokens: Tokens,
    day: &CalendarDay,
    index: usize,
    grid: &bongocat_pomodoro::MonthGrid,
    pomodoros: u32,
    today: CalendarDay,
    selected: Option<CalendarDay>,
) -> gpui_kit::AnyElement {
    let in_month =
        index >= grid.leading_days && index < grid.leading_days + grid.days_in_month as usize;
    let civil = day.to_civil();
    let is_today = *day == today;
    let is_selected = selected == Some(*day);

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
        .gap(px(2.0))
        .cursor_pointer()
        .when(in_month, |this| this.text_color(tokens.text))
        .when(!in_month, |this| this.text_color(tokens.muted))
        .when(is_today, |this| {
            this.bg(tokens.accent).text_color(tokens.canvas)
        })
        // The selected ring rides on top of the today highlight, so it uses
        // the canvas colour there to stay visible against the accent fill.
        .when(is_selected && !is_today, |this| {
            this.border_1().border_color(tokens.accent)
        })
        .when(is_selected && is_today, |this| {
            this.border_1().border_color(tokens.canvas)
        })
        .child(div().text_xs().child(civil.day.to_string()))
        // The day's finished-session count, kept visually below the day number
        // and set apart from it by weight and colour: a zero day shows nothing
        // rather than a zero that reads as an achievement.
        .when(pomodoros > 0, |this| {
            this.child(
                div()
                    .text_xs()
                    .font_semibold()
                    .text_color(if is_today {
                        tokens.canvas
                    } else {
                        tokens.accent
                    })
                    .child(format!("×{pomodoros}")),
            )
        })
        .on_click({
            let view = view.clone();
            let day = *day;
            move |_, _, cx| {
                // A future day cannot have sessions yet, so clicking one is a
                // no-op rather than a selection that shows an empty strip.
                if day > today {
                    return;
                }
                view.update(cx, |view, cx| {
                    // Clicking the selected day again clears the selection,
                    // so the chart slides back to ending on today.
                    view.pomodoro_selected_day = if view.pomodoro_selected_day == Some(day) {
                        None
                    } else {
                        Some(day)
                    };
                    cx.notify();
                });
            }
        })
        .into_any_element()
}

/// The day the trend chart's window ends on.
///
/// The picked day when it is still in the past — a click on the calendar
/// rewinds the whole chart to end there — and today otherwise: either nothing
/// is picked, or midnight has passed a pick that used to be today.
fn chart_end_day(selected: Option<CalendarDay>, today: CalendarDay) -> CalendarDay {
    selected.filter(|day| *day <= today).unwrap_or(today)
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

    #[test]
    fn a_series_ceiling_is_a_nice_number_at_or_above_the_maximum() {
        assert_eq!(nice_axis_max(0), 1.0);
        assert_eq!(nice_axis_max(1), 1.0);
        assert_eq!(nice_axis_max(4), 5.0);
        assert_eq!(nice_axis_max(7), 10.0);
        assert_eq!(nice_axis_max(25), 50.0);
        assert_eq!(nice_axis_max(30), 50.0);
        assert_eq!(nice_axis_max(100), 100.0);
        assert_eq!(nice_axis_max(120), 200.0);
    }

    #[test]
    fn a_tick_label_drops_the_decimal_tail_of_a_whole_number() {
        assert_eq!(axis_tick_label(0.0), "0");
        assert_eq!(axis_tick_label(25.0), "25");
        assert_eq!(axis_tick_label(12.5), "12.5");
    }

    #[test]
    fn a_short_span_labels_every_day_and_a_long_one_every_fifth_plus_the_last() {
        assert_eq!(date_tick_indices(7), (0..7).collect::<Vec<_>>());
        assert_eq!(
            date_tick_indices(30),
            vec![0, 5, 10, 15, 20, 25, 29],
            "thirty days do not fit thirty labels under the plot"
        );
        assert_eq!(date_tick_indices(1), vec![0]);
    }

    #[test]
    fn tick_spans_cover_the_row_exactly_once() {
        for count in [7, 30] {
            let ticks = date_tick_indices(count);
            let spans = date_tick_spans(count, &ticks);
            assert_eq!(spans.len(), ticks.len(), "{count} days");
            assert_eq!(
                spans.iter().sum::<usize>(),
                count,
                "{count} days should be covered exactly once"
            );
            assert!(
                spans.iter().all(|&span| span >= 1),
                "{count} days produced a zero-width label"
            );
        }
    }

    #[test]
    fn tomato_slots_cover_the_larger_count_and_cap_the_row() {
        // An untouched task shows its whole estimate, pale.
        assert_eq!(tomato_slots(4, 0), (4, 0));
        // A finished one shows the estimate, all solid.
        assert_eq!(tomato_slots(4, 4), (4, 4));
        // Past the estimate the row grows to the extra sessions...
        assert_eq!(tomato_slots(3, 6), (6, 6));
        // ...but never past the cap, where the ellipsis takes over.
        assert_eq!(tomato_slots(3, 10), (MAX_TOMATO_ICONS, MAX_TOMATO_ICONS));
        assert_eq!(tomato_slots(20, 0), (MAX_TOMATO_ICONS, 0));
    }

    #[test]
    fn the_chart_window_ends_on_the_picked_day_while_it_is_still_past() {
        let today = CalendarDay::from_day_number(20_000);
        let yesterday = CalendarDay::from_day_number(19_999);
        let tomorrow = CalendarDay::from_day_number(20_001);
        // Nothing picked: the chart ends on today, which is what the page
        // opens with.
        assert_eq!(chart_end_day(None, today), today);
        // A picked past day rewinds the window to end on it.
        assert_eq!(chart_end_day(Some(yesterday), today), yesterday);
        // A stale pick that midnight has passed — or a future day a click
        // refused — falls back to today rather than showing an empty window.
        assert_eq!(chart_end_day(Some(today), today), today);
        assert_eq!(chart_end_day(Some(tomorrow), today), today);
    }
}
