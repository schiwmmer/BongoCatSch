//! The resource economy: the purse, the three sites, the two trades and the
//! four buildings — the half of the page that turns a finished pomodoro's
//! spare time into the ore and timber the forge and the estate need.
//!
//! The panel is one column of rows, the way every panel on this page is: the
//! purse's four counts in one line, the three sites as three buttons, the two
//! trades as two rows, and the four buildings as four rows with their upgrade
//! buttons. Nothing here owns state — a gathering is one click whose result is
//! already in the snapshot.

use super::explore::cooldown_line;
use super::*;

/// The panel: the purse, the sites to gather at, and the buildings to raise.
pub(super) fn resources_panel(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
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
        .child(mode_row(view, cultivation.session_mode, locale))
        .child(bag_row(resources, locale))
        .child(expedition_status(resources, locale))
        .child(expedition_rows(view, resources, locale))
        .child(trade_row(resources.woodcut, GatherSkill::Woodcut, locale))
        .child(trade_row(resources.stonecut, GatherSkill::Stonecut, locale))
        .child(building_row(
            view,
            resources.workshop.clone(),
            Building::Workshop,
            locale,
            tokens,
        ))
        .child(building_row(
            view,
            resources.warehouse.clone(),
            Building::Warehouse,
            locale,
            tokens,
        ))
        .child(building_row(
            view,
            resources.smithy.clone(),
            Building::Smithy,
            locale,
            tokens,
        ))
        .child(building_row(
            view,
            resources.dojo.clone(),
            Building::Dojo,
            locale,
            tokens,
        ))
        .test_support()
}

/// The three session modes, as three buttons: what the next finished pomodoro
/// pays. The armed one is disabled, the way a selected 功法 is — a mode already
/// chosen is not offered again.
fn mode_row(view: &Entity<SettingsView>, current: SessionMode, locale: &str) -> impl IntoElement {
    let mut row = div()
        .id("cultivation-resources-modes")
        .flex()
        .flex_row()
        .gap_2();
    for mode in [SessionMode::Study, SessionMode::Woodcut, SessionMode::Mine] {
        let armed = mode == current;
        row = row.child(
            Button::new(format!("cultivation-mode-{mode:?}"))
                .ghost()
                .small()
                .label(text(locale, mode.key()))
                .disabled(armed)
                .on_click({
                    let view = view.clone();
                    move |_, _, cx| {
                        view.update(cx, |view, cx| {
                            let _ = view
                                .pomodoro
                                .try_publish(PomodoroCommand::SelectSessionMode { mode });
                            view.poll_pomodoro(cx);
                        });
                    }
                }),
        );
    }
    row
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

/// The gathering expedition under way, as one line: the site, the length and
/// the time left. Nothing while the cultivator is home.
fn expedition_status(resources: &ResourcesView, locale: &str) -> impl IntoElement {
    let Some(out) = resources.expedition.as_ref() else {
        return div().id("cultivation-expedition-status").test_support();
    };
    let key = if out.early {
        "cultivation.expedition.returning_in"
    } else {
        "cultivation.expedition.eta"
    };
    let eta = cooldown_line(Some(out.ends_unix), key, locale);
    div()
        .id("cultivation-expedition-status")
        .text_xs()
        .child(format_text(
            locale,
            "cultivation.expedition.status",
            &[
                ("site", text(locale, out.site.key()).to_string()),
                ("hours", out.hours.to_string()),
                ("eta", eta),
            ],
        ))
        .test_support()
}

/// The three sites, each with its three lengths: one expedition is one outing,
/// so clicking a length sets the cultivator out for that many hours and the
/// yield is settled on the clock. The lengths are closed while an expedition is
/// under way — one at a time — and a surface that knows the cultivator is out
/// says so above the rows.
fn expedition_rows(
    view: &Entity<SettingsView>,
    resources: &ResourcesView,
    locale: &str,
) -> impl IntoElement {
    let out = resources.expedition.is_some();
    let mut column = div()
        .id("cultivation-resources-sites")
        .flex()
        .flex_col()
        .gap_1();
    for site in [GatherSite::Forest, GatherSite::Mountain, GatherSite::Mine] {
        let mut row = div()
            .id(format!("cultivation-expedition-{site:?}"))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap_2()
            .child(div().text_sm().child(text(locale, site.key())));
        let mut chips = div().flex().flex_row().gap_1();
        for hours in ExplorationHours::ALL {
            chips = chips.child(
                Button::new(format!("cultivation-expedition-{site:?}-{}", hours.hours()))
                    .ghost()
                    .small()
                    .label(format_text(
                        locale,
                        "cultivation.expedition.hours",
                        &[("hours", hours.hours().to_string())],
                    ))
                    .disabled(out)
                    .on_click({
                        let view = view.clone();
                        move |_, _, cx| {
                            view.update(cx, |view, cx| {
                                let _ = view
                                    .pomodoro
                                    .try_publish(PomodoroCommand::StartExpedition { site, hours });
                                view.poll_pomodoro(cx);
                            });
                        }
                    }),
            );
        }
        row = row.child(chips);
        column = column.child(row);
    }
    column
}

/// One trade: its level, and the xp banked toward the next one.
fn trade_row(trade: TradeView, skill: GatherSkill, locale: &str) -> impl IntoElement {
    div()
        .id(format!("cultivation-trade-{skill:?}"))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_2()
        .child(div().text_sm().child(text(locale, skill.key())))
        .child(div().text_xs().child(format_text(
            locale,
            "cultivation.resources.trade",
            &[
                ("level", trade.level.to_string()),
                ("xp", trade.xp.to_string()),
                ("next", trade.xp_to_next.to_string()),
            ],
        )))
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
        );
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
                                let _ = view.pomodoro.try_publish(PomodoroCommand::UpgradeBuilding {
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
                                view.pomodoro.try_publish(PomodoroCommand::UpgradeBuildingMany {
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
