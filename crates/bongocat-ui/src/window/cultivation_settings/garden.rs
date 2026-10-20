//! Seeds and plots: the stacks, the four garden beds, and the harvest.

use super::*;

/// The seeds waiting to be planted.
pub(super) fn seed_panel(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let mut panel = div()
        .id("cultivation-seeds")
        .flex()
        .flex_col()
        .gap_1()
        .child(section_title(locale, "cultivation.seeds.title", tokens));
    if cultivation.seeds.is_empty() {
        return panel.child(
            div()
                .text_xs()
                .text_color(tokens.muted)
                .child(text(locale, "cultivation.seeds.empty")),
        );
    }
    let plots_full = cultivation.plots.iter().all(|plot| plot.crop.is_some());
    let rows = cultivation.seeds.iter().take(8).map(|seed| {
        let label = format!(
            "{} ×{}",
            seed_label(locale, seed.attribute, seed.tier, seed.name.as_deref()),
            seed.count
        );
        let mut button = Button::new(format!(
            "cultivation-plant-{}-{}",
            seed.attribute.index(),
            seed.tier
        ))
        .ghost()
        .small()
        .label(text(locale, "cultivation.seed.plant"));
        if !plots_full {
            button = button.on_click({
                let view = view.clone();
                let attribute = seed.attribute;
                let tier = seed.tier;
                move |_, _, cx| {
                    view.update(cx, |view, cx| {
                        let _ = view
                            .pomodoro
                            .try_publish(PomodoroCommand::PlantSeed { attribute, tier });
                        view.poll_pomodoro(cx);
                    });
                }
            });
        }
        div()
            .id(format!(
                "cultivation-seed-{}-{}",
                seed.attribute.index(),
                seed.tier
            ))
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .py_1()
            .child(div().flex_1().min_w_0().truncate().text_xs().child(label))
            .child(button)
            // One seed at a time: a stack of ten is nine seeds and one of them,
            // and which one the user wants to keep is not a question a single
            // "sell all" can ask.
            .child(
                Button::new(format!(
                    "cultivation-sell-seed-{}-{}",
                    seed.attribute.index(),
                    seed.tier
                ))
                .ghost()
                .small()
                .label(sell_label(locale, seed.sell_price))
                .on_click({
                    let view = view.clone();
                    let attribute = seed.attribute;
                    let tier = seed.tier;
                    move |_, _, cx| {
                        view.update(cx, |view, cx| {
                            let _ = view
                                .pomodoro
                                .try_publish(PomodoroCommand::SellSeed { attribute, tier });
                            view.poll_pomodoro(cx);
                        });
                    }
                }),
            )
    });
    panel = panel.children(rows);
    if plots_full {
        panel = panel.child(
            div()
                .text_xs()
                .text_color(tokens.muted)
                .child(text(locale, "cultivation.garden.full")),
        );
    }
    panel
}

/// The four plots: empty, growing, or ready to pick.
pub(super) fn garden_panel(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let now = now_unix();
    let rows = cultivation.plots.iter().enumerate().map(|(index, plot)| {
        let mut row = div()
            .id(format!("cultivation-plot-{index}"))
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .py_1();
        match plot.crop {
            None => {
                row = row.child(
                    div()
                        .flex_1()
                        .text_xs()
                        .text_color(tokens.muted)
                        .child(text(locale, "cultivation.garden.empty")),
                );
            }
            Some(crop) => {
                let label = seed_label(locale, crop.attribute, crop.tier, None);
                let remaining = crop.ready_at_unix.saturating_sub(now).max(0);
                row = row.child(div().flex_1().min_w_0().truncate().text_xs().child(label));
                if remaining == 0 {
                    row = row
                        .child(
                            div()
                                .text_xs()
                                .text_color(tokens.accent)
                                .child(text(locale, "cultivation.garden.ready")),
                        )
                        .child(
                            Button::new(format!("cultivation-harvest-{index}"))
                                .small()
                                .label(text(locale, "cultivation.garden.harvest"))
                                .on_click({
                                    let view = view.clone();
                                    move |_, _, cx| {
                                        view.update(cx, |view, cx| {
                                            let _ = view.pomodoro.try_publish(
                                                PomodoroCommand::HarvestPlot { plot: index },
                                            );
                                            view.poll_pomodoro(cx);
                                        });
                                    }
                                }),
                        );
                } else {
                    let minutes = ((remaining + 59) / 60).max(1);
                    row = row.child(div().text_xs().text_color(tokens.muted).child(format_text(
                        locale,
                        "cultivation.garden.growing",
                        &[("minutes", minutes.to_string())],
                    )));
                }
            }
        }
        row
    });

    div()
        .id("cultivation-garden")
        .flex()
        .flex_col()
        .gap_1()
        .child(section_title(locale, "cultivation.garden.title", tokens))
        .children(rows)
}
