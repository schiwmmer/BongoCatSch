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

/// How long a crop still needs, in the unit that reads best.
///
/// Minutes inside the hour, hours inside the two days, days past that: a
/// third-tier crop is an afternoon and a tenth-tier one is a season, and
/// "4,320 分钟后成熟" says neither of those things.
fn growing_line(locale: &str, remaining: i64) -> String {
    let minutes = ((remaining + 59) / 60).max(1);
    if minutes < 60 {
        format_text(
            locale,
            "cultivation.garden.growing",
            &[("minutes", minutes.to_string())],
        )
    } else if minutes < 60 * 48 {
        let hours = (minutes + 59) / 60;
        format_text(
            locale,
            "cultivation.garden.growing_hours",
            &[("hours", hours.to_string())],
        )
    } else {
        let days = (minutes + 59) / (60 * 24);
        format_text(
            locale,
            "cultivation.garden.growing_days",
            &[("days", days.to_string())],
        )
    }
}

/// The garden: the plots three to a row — empty, growing, or ready to pick —
/// and, while the garden still has room to grow, the plot that 灵石 can buy.
pub(super) fn garden_panel(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let now = now_unix();
    let mut panel = div()
        .id("cultivation-garden")
        .flex()
        .flex_col()
        .gap_1()
        .child(section_title(locale, "cultivation.garden.title", tokens))
        .test_support();
    for (row_index, chunk) in cultivation.plots.chunks(3).enumerate() {
        let mut row = div()
            .id(format!("cultivation-plot-row-{row_index}"))
            .flex()
            .flex_row()
            .gap_2();
        for (offset, plot) in chunk.iter().enumerate() {
            let index = row_index * 3 + offset;
            row = row.child(plot_cell(view, index, plot, now, locale, tokens));
        }
        panel = panel.child(row);
    }
    // The next plot, while the garden still has one to sell: the price is the
    // domain's own, so the button cannot promise a number the purchase would
    // not charge.
    if let Some(price) = cultivation.next_plot_price {
        panel = panel.child(
            Button::new("cultivation-buy-plot")
                .ghost()
                .small()
                .label(format_text(
                    locale,
                    "cultivation.garden.buy_plot",
                    &[("price", price.to_string())],
                ))
                .on_click({
                    let view = view.clone();
                    move |_, _, cx| {
                        view.update(cx, |view, cx| {
                            let _ = view.pomodoro.try_publish(PomodoroCommand::BuyPlot);
                            view.poll_pomodoro(cx);
                        });
                    }
                }),
        );
    }
    panel
}

/// One plot as a cell a row of three can hold: the herb's name on top, and
/// what the plot is doing under it.
fn plot_cell(
    view: &Entity<SettingsView>,
    index: usize,
    plot: &PlotView,
    now: i64,
    locale: &str,
    tokens: Tokens,
) -> gpui_kit::AnyElement {
    let mut cell = div()
        .id(format!("cultivation-plot-{index}"))
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_1()
        .py_1()
        .test_support();
    let Some(crop) = plot.crop else {
        return cell
            .child(
                div()
                    .text_xs()
                    .text_color(tokens.muted)
                    .child(text(locale, "cultivation.garden.empty")),
            )
            .into_any_element();
    };
    cell = cell.child(div().text_xs().truncate().child(seed_label(
        locale,
        crop.attribute,
        crop.tier,
        None,
    )));
    let remaining = crop.ready_at_unix.saturating_sub(now).max(0);
    if remaining == 0 {
        cell = cell.child(
            Button::new(format!("cultivation-harvest-{index}"))
                .small()
                .label(text(locale, "cultivation.garden.harvest"))
                .on_click({
                    let view = view.clone();
                    move |_, _, cx| {
                        view.update(cx, |view, cx| {
                            let _ = view
                                .pomodoro
                                .try_publish(PomodoroCommand::HarvestPlot { plot: index });
                            view.poll_pomodoro(cx);
                        });
                    }
                }),
        );
    } else {
        cell = cell.child(
            div()
                .text_xs()
                .text_color(tokens.muted)
                .child(growing_line(locale, remaining)),
        );
    }
    cell.into_any_element()
}
