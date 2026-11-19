//! The resource economy: the real-time tasks, the purse, the two trades and the
//! four buildings — the half of the page that turns the cultivator's attention
//! into the ore and timber the forge and the estate need.
//!
//! Gathering is a task, not an outing: 伐木 and 采石 start a run that pays a
//! tick at a time (`docs/0103` §4), so there is no length to pick here and no
//! "out gathering" line to draw. What is left is packed rather than stacked,
//! unlike the panels whose content is a sentence per row: the purse's four counts
//! in one line, the two trades on one line, and the four buildings two to a row.

use super::*;

/// The panel: the tasks to start, the purse, and the buildings to raise.
///
/// Packed rather than stacked. Every line here used to own a row of its own,
/// which made the panel the longest on the page for the least information: the
/// three trades were one line each, and the four buildings one row each. The
/// information is the same now, packed into the rows that were mostly empty —
/// the two trades share one line, and the buildings sit two to a row.
pub(super) fn resources_panel(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    tier: GatherTier,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let resources = &cultivation.resources;
    div()
        .id("cultivation-resources")
        .flex()
        .flex_col()
        .gap_1()
        .child(section_title(locale, "cultivation.resources.title", tokens))
        .child(trades_line(resources, locale))
        .child(task_row(view, cultivation, tier, locale, tokens))
        .child(seclusion_row(view, cultivation, locale, tokens))
        .child(bag_row(resources, locale))
        .child(building_grid(view, resources, locale, tokens))
        .test_support()
}

/// The real-time task area (`docs/0103` §4): the task under way, and the three
/// a click can start.
///
/// A session mode decides what a *finished pomodoro* pays; a task is started
/// and stopped by hand and pays on the clock. The old 修习/伐木/采石 row that
/// used to sit beside this one is gone: it armed the next session, which is
/// what the 番茄钟 window's own selector is for, and a second copy of it here
/// read as a second way to start the same work.
fn task_row(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    tier: GatherTier,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let mut panel = div()
        .id("cultivation-task")
        .flex()
        .flex_col()
        .gap_1()
        .child(section_title(locale, "cultivation.task.title", tokens));
    panel = match cultivation.task.as_ref() {
        Some(task) => panel
            .child(task_status_line(task, now_unix_millis(), locale, tokens))
            .child(task_stop_button(view, locale)),
        None => panel.child(
            div()
                .id("cultivation-task-idle")
                .text_xs()
                .text_color(tokens.muted)
                .child(text(locale, "cultivation.task.idle"))
                .test_support(),
        ),
    };
    panel
        .child(task_start_row(view, cultivation, tier, locale, tokens))
        .test_support()
}

/// The task under way as one line and the bar filling beneath it: what it is,
/// where it works, how many ticks the clock has already paid, how long one tick
/// is, and how far the tick under way has run.
///
/// The tick count and the tempo are both read off the task rather than
/// recomputed from the clock: the domain is the only thing that knows which
/// ticks it has settled, and a page that divided the elapsed seconds itself
/// would drift from the save the moment a tick was capped or a relaunch landed
/// mid-tick. The bar is the other half of that: the domain settles whole ticks
/// and says nothing in between, so the part of the tick already spent is read
/// off the task's own `start_unix` and the wall clock, the way the page's
/// countdowns are.
fn task_status_line(
    task: &ActiveTask,
    now_ms: i64,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let kind = text(locale, task.kind.key()).to_string();
    let ticks = task.ticks_done.to_string();
    let seconds = task.tick_seconds.to_string();
    let line = match task.site {
        Some(site) => format_text(
            locale,
            "cultivation.task.current_gather",
            &[
                ("kind", kind),
                ("site", text(locale, site.key()).to_string()),
                ("tier", text(locale, task.tier.key()).to_string()),
                ("ticks", ticks),
                ("seconds", seconds),
            ],
        ),
        None => format_text(
            locale,
            "cultivation.task.current_plain",
            &[("kind", kind), ("ticks", ticks), ("seconds", seconds)],
        ),
    };
    div()
        .id("cultivation-task-status")
        .flex()
        .flex_col()
        .gap_0p5()
        .text_xs()
        .text_color(tokens.accent)
        .child(line)
        .child(progress_bar(
            "cultivation-task-progress",
            task.progress_permille(now_ms),
            tokens,
        ))
        .test_support()
}

/// The one button that ends the task under way.
fn task_stop_button(view: &Entity<SettingsView>, locale: &str) -> impl IntoElement {
    Button::new("cultivation-task-stop")
        .ghost()
        .small()
        .label(text(locale, "cultivation.task.stop"))
        .on_click({
            let view = view.clone();
            move |_, _, cx| {
                view.update(cx, |view, cx| {
                    let _ = view.pomodoro.try_publish(PomodoroCommand::CultStopTask);
                    view.poll_pomodoro(cx);
                });
            }
        })
}

/// The three tasks a click can start, at the tier the chips name.
///
/// The tier is chosen once for both gathering tasks: the real choice is "how
/// deep", not a per-button decision. 伐木 works the forest and 采石 the
/// mountain, so the site is the kind's own rather than a second picker — a
/// 伐木 task in the mountain is a task the design does not define. The 矿洞's
/// dual-skill point has no [`TaskKind`] of its own yet, so it is not offered
/// here.
///
/// A task already running disables the three: the slot is one, and a second
/// start would only be refused. The running task's own stop button is the way
/// out, and it is drawn directly above.
fn task_start_row(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    tier: GatherTier,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let running = cultivation.task.is_some();
    let chips = GatherTier::ALL.into_iter().map(|choice| {
        let selected = choice == tier;
        let mut chip = div()
            .id(format!("cultivation-task-tier-{choice:?}"))
            .px_2()
            .py_1()
            .rounded_md()
            .text_xs()
            .child(text(locale, choice.key()))
            .on_click({
                let view = view.clone();
                move |_, _, cx| {
                    view.update(cx, |view, cx| {
                        view.cultivation_task_tier = choice;
                        cx.notify();
                    });
                }
            })
            .test_support();
        if selected {
            chip = chip
                .bg(tokens.overlay)
                .text_color(tokens.accent)
                .font_semibold();
        } else {
            chip = chip.text_color(tokens.muted);
        }
        chip
    });
    // One builder for the three starts, so the command each click publishes is
    // spelled once: the kind names the task, the site is the kind's own, and
    // the tier is the chip the user lit.
    let start_button = |kind: TaskKind, site: Option<GatherSite>, id: &'static str| {
        let mut button = Button::new(id)
            .ghost()
            .small()
            .label(text(locale, kind.key()))
            .disabled(running);
        button = button.on_click({
            let view = view.clone();
            move |_, _, cx| {
                view.update(cx, |view, cx| {
                    let _ = view.pomodoro.try_publish(PomodoroCommand::CultStartTask {
                        kind,
                        site,
                        tier: Some(tier),
                        grade: None,
                        // None of the three starts here is a 打工: the board
                        // takes those, and the trade a job was taken at is
                        // what `accept_job` passes. Carried as a field so a
                        // future start that is a job does not have to grow
                        // the variant.
                        job_kind: None,
                    });
                    view.poll_pomodoro(cx);
                });
            }
        });
        button
    };
    div()
        .id("cultivation-task-starts")
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_1()
        .children(chips)
        .child(start_button(
            TaskKind::Meditate,
            None,
            "cultivation-task-start-meditate",
        ))
        .child(start_button(
            TaskKind::Woodcut,
            Some(GatherSite::Forest),
            "cultivation-task-start-woodcut",
        ))
        .child(start_button(
            TaskKind::Stonecut,
            Some(GatherSite::Mountain),
            "cultivation-task-start-stonecut",
        ))
        .test_support()
}

/// The 闭关 under way, as its own row beside the task (`docs/0103` §4).
///
/// 闭关 is not a task and does not take the slot, so it is drawn apart from the
/// task's start buttons rather than as one of them: a user who can gather and
/// sit at the same time would otherwise be offered a button that silently
/// refuses. The progress is the sit's own tick count against the 250 a full sit
/// runs, read off the run rather than recomputed.
fn seclusion_row(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let total = bongocat_pomodoro::SECLUSION_TICKS.to_string();
    let mut row = div()
        .id("cultivation-seclusion")
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_2();
    match cultivation.seclusion.as_ref() {
        Some(run) => {
            row = row.child(
                div()
                    .id("cultivation-seclusion-progress")
                    .text_xs()
                    .text_color(tokens.accent)
                    .child(format_text(
                        locale,
                        "cultivation.seclusion.progress",
                        &[("ticks", run.ticks_done.to_string()), ("total", total)],
                    ))
                    .test_support(),
            );
            row = row.child(seclusion_button(
                view,
                "cultivation-seclusion-stop",
                "cultivation.seclusion.stop",
                PomodoroCommand::CultStopSeclusion,
                locale,
            ));
        }
        None => {
            row = row.child(
                div()
                    .id("cultivation-seclusion-progress")
                    .text_xs()
                    .text_color(tokens.muted)
                    .child(text(locale, "cultivation.seclusion.title"))
                    .test_support(),
            );
            row = row.child(seclusion_button(
                view,
                "cultivation-seclusion-start",
                "cultivation.seclusion.start",
                PomodoroCommand::CultStartSeclusion,
                locale,
            ));
        }
    }
    row.test_support()
}

/// One 闭关 button: the command is the only thing the two states differ by.
fn seclusion_button(
    view: &Entity<SettingsView>,
    id: &'static str,
    label_key: &'static str,
    command: PomodoroCommand,
    locale: &str,
) -> impl IntoElement {
    Button::new(id)
        .ghost()
        .small()
        .label(text(locale, label_key))
        .on_click({
            let view = view.clone();
            move |_, _, cx| {
                view.update(cx, |view, cx| {
                    let _ = view.pomodoro.try_publish(command.clone());
                    view.poll_pomodoro(cx);
                });
            }
        })
}

/// The purse: one line, each resource's count against its ceiling.
fn bag_row(resources: &ResourcesView, locale: &str) -> impl IntoElement {
    let mut parts = Vec::new();
    for resource in Resource::ALL {
        let count = resources.bag.get(resource);
        parts.push(format_text(
            locale,
            "cultivation.resources.bag_entry",
            &[
                ("resource", text(locale, resource.key()).to_string()),
                ("count", count.to_string()),
                ("cap", resources.cap.to_string()),
            ],
        ));
    }
    div()
        .id("cultivation-resources-bag")
        .text_xs()
        .child(parts.join(" · "))
}

/// The two trades on one line: each level and the xp banked toward the next.
fn trades_line(resources: &ResourcesView, locale: &str) -> impl IntoElement {
    let trade = |trade: TradeView, skill: GatherSkill| {
        format_text(
            locale,
            "cultivation.resources.trade",
            &[
                ("skill", text(locale, skill.key()).to_string()),
                ("level", trade.level.to_string()),
                ("xp", trade.xp.to_string()),
                ("next", trade.xp_to_next.to_string()),
            ],
        )
    };
    div()
        .id("cultivation-trades")
        .text_xs()
        .child(format!(
            "{} · {}",
            trade(resources.woodcut, GatherSkill::Woodcut),
            trade(resources.stonecut, GatherSkill::Stonecut)
        ))
        .test_support()
}

/// The four buildings, two to a row.
///
/// The ladder is long — the 仓库 alone runs to fifty ranks — and four full-width
/// rows of "name — price — ×5" spent the page's height on two words and a
/// number each. Two per row keeps the name, the rank and both buttons and halves
/// the height.
fn building_grid(
    view: &Entity<SettingsView>,
    resources: &ResourcesView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let cell = |building: BuildingView, kind: Building| {
        div()
            .flex_1()
            .min_w_0()
            .child(building_row(view, building, kind, locale, tokens))
    };
    div()
        .id("cultivation-buildings")
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .flex()
                .flex_row()
                .gap_3()
                .child(cell(resources.workshop.clone(), Building::Workshop))
                .child(cell(resources.warehouse.clone(), Building::Warehouse)),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap_3()
                .child(cell(resources.smithy.clone(), Building::Smithy))
                .child(cell(resources.dojo.clone(), Building::Dojo)),
        )
        .test_support()
}

/// One building: its name and level, and the upgrade button while it has a
/// rank left to buy. The name is a hover: a building that does something a
/// surface cannot see would read as a label, so the hint says what each one is
/// worth per level.
fn building_row(
    view: &Entity<SettingsView>,
    building: BuildingView,
    kind: Building,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let name = text(locale, kind.key()).to_string();
    let level = building.level.to_string();
    let max = kind.max_level().to_string();
    let tip_key = match kind {
        Building::Workshop => "cultivation.building_tip.workshop",
        Building::Warehouse => "cultivation.building_tip.warehouse",
        Building::Smithy => "cultivation.building_tip.smithy",
        Building::Dojo => "cultivation.building_tip.dojo",
    };
    // The hover's name and body are both static strings (a translation-table
    // lookup), so the content closure owns them as `&'static str` — nothing
    // borrowed from this call's `locale` escapes the function.
    let hint_name = text(locale, kind.key());
    let hint_body = text(locale, tip_key);
    let mut row = div()
        .id(format!("cultivation-building-{kind:?}"))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_2()
        .child(
            HoverCard::new(format!("cultivation-building-hint-{kind:?}"))
                .trigger(
                    div()
                        .id(format!("cultivation-building-name-{kind:?}"))
                        .text_sm()
                        .child(format_text(
                            locale,
                            "cultivation.resources.building",
                            &[("building", name), ("level", level), ("max", max)],
                        ))
                        .test_support(),
                )
                .open_delay(Duration::from_millis(120))
                .content(move |_, _, _| building_hint(kind, hint_name, hint_body, tokens)),
        )
        .test_support();
    if building.upgrade_cost.is_some() {
        let mut buttons = div().flex().flex_row().gap_1();
        if let Some(cost) = building.upgrade_cost.as_ref() {
            buttons = buttons.child(
                Button::new(format!("cultivation-upgrade-{kind:?}"))
                    .ghost()
                    .small()
                    .label(cost_line(locale, cost))
                    .on_click({
                        let view = view.clone();
                        move |_, _, cx| {
                            view.update(cx, |view, cx| {
                                let _ =
                                    view.pomodoro.try_publish(PomodoroCommand::UpgradeBuilding {
                                        building: kind,
                                    });
                                view.poll_pomodoro(cx);
                            });
                        }
                    }),
            );
        }
        // A long ladder — the 仓库's fifty — is a chore one rank at a time, so
        // the same click can buy up to five ranks; the state stops early when
        // the purse runs out or the building tops out.
        buttons = buttons.child(
            Button::new(format!("cultivation-upgrade-five-{kind:?}"))
                .ghost()
                .small()
                .label(text(locale, "cultivation.resources.upgrade_five"))
                .on_click({
                    let view = view.clone();
                    move |_, _, cx| {
                        view.update(cx, |view, cx| {
                            let _ =
                                view.pomodoro
                                    .try_publish(PomodoroCommand::UpgradeBuildingMany {
                                        building: kind,
                                        times: 5,
                                    });
                            view.poll_pomodoro(cx);
                        });
                    }
                }),
        );
        row = row.child(buttons);
    }
    row
}

/// What one building is worth per level, as a hover's body: the name and the
/// effect, the way an attribute's hint says its name and its explanation.
fn building_hint(
    kind: Building,
    name: &'static str,
    body: &'static str,
    tokens: Tokens,
) -> impl IntoElement {
    div()
        .id(format!("cultivation-building-hint-body-{kind:?}"))
        .flex()
        .flex_col()
        .gap_1()
        .w(px(240.0))
        .child(div().text_xs().text_color(tokens.text).child(name))
        .child(
            div()
                .text_xs()
                .text_color(tokens.muted)
                .whitespace_normal()
                .child(body),
        )
        .test_support()
}

/// A price, spelled out the way a 升星's is: the 灵石 wager and the tribute.
fn cost_line(locale: &str, cost: &Cost) -> String {
    format_text(
        locale,
        "cultivation.resources.cost",
        &[
            ("stones", cost.stones.to_string()),
            ("tribute", resource_bag_text(locale, &cost.resources)),
        ],
    )
}
