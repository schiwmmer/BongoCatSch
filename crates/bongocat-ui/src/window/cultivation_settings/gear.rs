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
        .child(section_title(locale, "cultivation.inventory.title", tokens));
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
            );
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
