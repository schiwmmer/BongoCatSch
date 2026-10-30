//! The resource economy: the purse, the three sites, the two trades and the
//! four buildings — the half of the page that turns a finished pomodoro's
//! spare time into the ore and timber the forge and the estate need.
//!
//! The panel is one column of rows, the way every panel on this page is: the
//! purse's four counts in one line, the three sites as three buttons, the two
//! trades as two rows, and the four buildings as four rows with their upgrade
//! buttons. Nothing here owns state — a gathering is one click whose result is
//! already in the snapshot.

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
        .child(gather_row(view, locale))
        .child(trade_row(resources.woodcut, GatherSkill::Woodcut, locale))
        .child(trade_row(resources.stonecut, GatherSkill::Stonecut, locale))
        .child(building_row(
            view,
            resources.workshop.clone(),
            Building::Workshop,
            locale,
        ))
        .child(building_row(
            view,
            resources.warehouse.clone(),
            Building::Warehouse,
            locale,
        ))
        .child(building_row(
            view,
            resources.smithy.clone(),
            Building::Smithy,
            locale,
        ))
        .child(building_row(
            view,
            resources.dojo.clone(),
            Building::Dojo,
            locale,
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

/// The three sites, as three buttons: each one publishes its gathering.
fn gather_row(view: &Entity<SettingsView>, locale: &str) -> impl IntoElement {
    let mut row = div()
        .id("cultivation-resources-sites")
        .flex()
        .flex_row()
        .gap_2();
    for site in [GatherSite::Forest, GatherSite::Mountain, GatherSite::Mine] {
        row = row.child(
            Button::new(format!("cultivation-gather-{site:?}"))
                .ghost()
                .small()
                .label(text(locale, site.key()))
                .on_click({
                    let view = view.clone();
                    move |_, _, cx| {
                        view.update(cx, |view, cx| {
                            let _ = view
                                .pomodoro
                                .try_publish(PomodoroCommand::CollectResources { site });
                            view.poll_pomodoro(cx);
                        });
                    }
                }),
        );
    }
    row
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
/// rank left to buy.
fn building_row(
    view: &Entity<SettingsView>,
    building: BuildingView,
    kind: Building,
    locale: &str,
) -> impl IntoElement {
    let mut row = div()
        .id(format!("cultivation-building-{kind:?}"))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_2()
        .child(div().text_sm().child(format_text(
            locale,
            "cultivation.resources.building",
            &[
                ("building", text(locale, kind.key()).to_string()),
                ("level", building.level.to_string()),
            ],
        )));
    if let Some(cost) = building.upgrade_cost.as_ref() {
        row = row.child(
            Button::new(format!("cultivation-upgrade-{kind:?}"))
                .ghost()
                .small()
                .label(cost_line(locale, cost))
                .on_click({
                    let view = view.clone();
                    move |_, _, cx| {
                        view.update(cx, |view, cx| {
                            let _ = view
                                .pomodoro
                                .try_publish(PomodoroCommand::UpgradeBuilding { building: kind });
                            view.poll_pomodoro(cx);
                        });
                    }
                }),
        );
    }
    row
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
