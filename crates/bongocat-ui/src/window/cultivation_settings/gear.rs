//! Equipment and satchel: the four slots, the rest of the inventory,
//! and selling a piece that is not worn.

use super::*;

/// The four slots and whatever is worn in each.
pub(super) fn equipment_panel(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
    open_item: Option<u64>,
) -> impl IntoElement {
    let rows = EquipSlot::ALL.into_iter().map(|slot| {
        let worn = cultivation.equipped[slot.index()].clone();
        let label = worn.as_ref().map_or_else(
            || text(locale, "cultivation.slot.empty").to_string(),
            |item| item_label(locale, item),
        );
        let mut line = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .py_1()
            .child(
                div()
                    .w(px(56.0))
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(tokens.muted)
                    .child(text(locale, slot.name_key())),
            )
            .child(item_label_element(view, worn.as_ref(), label, tokens));
        if worn.is_some() {
            line = line.child(
                Button::new(format!("cultivation-unequip-{}", slot.index()))
                    .ghost()
                    .small()
                    .label(text(locale, "cultivation.item.unequip"))
                    .on_click({
                        let view = view.clone();
                        move |_, _, cx| {
                            view.update(cx, |view, cx| {
                                let _ = view
                                    .pomodoro
                                    .try_publish(PomodoroCommand::UnequipSlot { slot });
                                view.poll_pomodoro(cx);
                            });
                        }
                    }),
            );
            // The wager rides beside the worn piece too: what is on the body is
            // what fights, so it is what the user wants starred.
            if let Some(item) = worn.as_ref() {
                line = line.children(star_button(view, item, locale));
            }
        }
        let mut row = div()
            .id(format!("cultivation-slot-{}", slot.index()))
            .flex()
            .flex_col()
            .gap_1()
            .child(line);
        if let Some(item) = worn.as_ref()
            && open_item == Some(item.id)
        {
            row = row.child(item_detail(item, locale, tokens));
        }
        row
    });

    div()
        .id("cultivation-equipment")
        .flex()
        .flex_col()
        .gap_1()
        .child(section_title(locale, "cultivation.equipment", tokens))
        .children(rows)
}

/// The satchel: everything owned and not worn.
pub(super) fn inventory_panel(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
    open_item: Option<u64>,
) -> impl IntoElement {
    let mut panel = div()
        .id("cultivation-inventory")
        .flex()
        .flex_col()
        .gap_1()
        .child(section_title(locale, "cultivation.inventory.title", tokens))
        .children(consumable_rows(view, cultivation, locale, tokens));
    if cultivation.inventory.is_empty() {
        return panel.child(
            div()
                .text_xs()
                .text_color(tokens.muted)
                .child(text(locale, "cultivation.inventory.empty")),
        );
    }
    // Newest first: a fresh drop is the one the user is looking for, and the
    // list is bounded so a long-lived save cannot push the garden off the page.
    let rows = cultivation.inventory.iter().take(8).map(|item| {
        let line = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .py_1()
            .child(item_label_element(
                view,
                Some(item),
                item_label(locale, item),
                tokens,
            ))
            .child(
                Button::new(format!("cultivation-equip-{}", item.id))
                    .ghost()
                    .small()
                    .label(text(locale, "cultivation.item.equip"))
                    .on_click({
                        let view = view.clone();
                        let item_id = item.id;
                        move |_, _, cx| {
                            view.update(cx, |view, cx| {
                                let _ = view
                                    .pomodoro
                                    .try_publish(PomodoroCommand::EquipItem { item_id });
                                view.poll_pomodoro(cx);
                            });
                        }
                    }),
            )
            .child(
                Button::new(format!("cultivation-sell-gear-{}", item.id))
                    .ghost()
                    .small()
                    .label(sell_label(locale, item.sell_price))
                    .on_click({
                        let view = view.clone();
                        let item_id = item.id;
                        move |_, _, cx| {
                            view.update(cx, |view, cx| {
                                let _ = view
                                    .pomodoro
                                    .try_publish(PomodoroCommand::SellEquipment { item_id });
                                view.poll_pomodoro(cx);
                            });
                        }
                    }),
            )
            // The wager beside the sale: what a star costs and what the piece
            // sells for are the two numbers the row is chosen between.
            .children(star_button(view, item, locale));
        let mut row = div()
            .id(format!("cultivation-inventory-{}", item.id))
            .flex()
            .flex_col()
            .gap_1()
            .child(line);
        if open_item == Some(item.id) {
            row = row.child(item_detail(item, locale, tokens));
        }
        row
    });
    panel = panel.children(rows);
    panel
}

/// The 潜质 consumables the satchel carries, and only the ones it carries.
///
/// The 潜质 panel used to keep all three in a row of its own, with a 服用 button
/// beside each. A herb is fed the moment it is found now, so what is left is a
/// pill and the gate ahead of it: one button that spends that gate's own number
/// of pills to carry 潜质 up to the next even hundred. A herb only appears here
/// when the body could not take it — the band was full, or the gate had no pill
/// — and then the button is the one the page no longer carries.
///
/// A count of zero shows nothing at all: the satchel lists what is in it, and a
/// permanent "洗髓丹 ×0" row is an empty shelf talking about an item the user
/// does not have.
fn consumable_rows(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
) -> Vec<gpui_kit::AnyElement> {
    let mut rows: Vec<gpui_kit::AnyElement> = Vec::new();
    for (index, herb) in AptitudeHerb::ALL.into_iter().enumerate() {
        let held = cultivation.aptitude_herbs[index];
        if held == 0 {
            continue;
        }
        let name = format!("{} ×{held}", text(locale, herb.name_key()));
        // One button per plant: a pill opens the gate ahead at that gate's
        // price, a herb goes into the body. Both are the same control, so the
        // row that draws them does not have to know which one it is holding.
        let button = if herb == AptitudeHerb::Marrow {
            let gate = cultivation.marrow_gate_cost;
            let label = match gate {
                Some(cost) => format_text(
                    locale,
                    "cultivation.herb.use_pill",
                    &[("pills", cost.to_string())],
                ),
                None => text(locale, "cultivation.aptitude_refuse_capped").to_string(),
            };
            Button::new("cultivation-use-marrow")
                .ghost()
                .small()
                .label(label)
                .disabled(gate.is_none())
                .on_click({
                    let view = view.clone();
                    move |_, _, cx| {
                        view.update(cx, |view, cx| {
                            let _ = view.pomodoro.try_publish(PomodoroCommand::UseMarrowPill);
                            view.poll_pomodoro(cx);
                        });
                    }
                })
        } else {
            Button::new(format!("cultivation-use-held-herb-{index}"))
                .ghost()
                .small()
                .label(text(locale, "cultivation.aptitude_use"))
                .on_click({
                    let view = view.clone();
                    move |_, _, cx| {
                        view.update(cx, |view, cx| {
                            let _ = view
                                .pomodoro
                                .try_publish(PomodoroCommand::UseAptitudeHerb { herb });
                            view.poll_pomodoro(cx);
                        });
                    }
                })
        };
        rows.push(
            div()
                .id(format!("cultivation-held-herb-{index}"))
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .py_1()
                .child(div().text_xs().text_color(tokens.text).child(name))
                .child(button)
                .test_support()
                .into_any_element(),
        );
    }
    rows
}
