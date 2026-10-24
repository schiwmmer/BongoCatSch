//! Seeds and plots: the stacks, the four garden beds, and the harvest.

use super::*;

/// What one harvest of this seed pays, in the attribute it raises.
///
/// The domain owns the number; the page only spells it. Both the shelf and the
/// satchel read it, so the figure a buyer sees is the figure the plot pays.
fn harvest_line(locale: &str, attribute: Attribute, tier: u8, grade: SeedGrade) -> String {
    format_text(
        locale,
        "cultivation.seed.harvest_line",
        &[
            ("attribute", text(locale, attribute.name_key()).to_string()),
            ("value", harvest_points(tier, grade).to_string()),
        ],
    )
}

/// Everything a seed is worth knowing before it is planted: what it pays when
/// it comes up, and how long that takes.
///
/// One string shared by the shelf and the satchel, so the herb cannot be
/// described two ways depending on where the user happened to look at it.
pub(super) fn seed_facts(locale: &str, attribute: Attribute, tier: u8, grade: SeedGrade) -> String {
    format!(
        "{} · {}",
        harvest_line(locale, attribute, tier, grade),
        growing_line(locale, grow_seconds(tier)),
    )
}

/// The line under a seed's row: what it pays, what a sale would bring, and —
/// on the shelf and in the satchel — how long it would take to grow.
fn seed_detail(
    view: &Entity<SettingsView>,
    key: (Attribute, u8, SeedGrade),
    line: String,
    tokens: Tokens,
) -> gpui_kit::AnyElement {
    div()
        .id(format!(
            "cultivation-seed-detail-{}-{}-{}",
            key.0.index(),
            key.1,
            key.2.index()
        ))
        .w_full()
        .text_xs()
        .text_color(tokens.muted)
        .child(line)
        .test_support()
        .on_click({
            let view = view.clone();
            move |_, _, cx| {
                view.update(cx, |view, cx| {
                    view.cultivation_open_seed = None;
                    cx.notify();
                });
            }
        })
        .into_any_element()
}

/// The seeds waiting to be planted.
pub(super) fn seed_panel(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    open_seed: Option<(Attribute, u8, SeedGrade)>,
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
        let key = (seed.attribute, seed.tier, seed.grade);
        let label = format!(
            "{} ×{}",
            seed_label(
                locale,
                seed.attribute,
                seed.tier,
                seed.grade,
                seed.name.as_deref()
            ),
            seed.count
        );
        let grade_paint = seed_grade_color(seed.grade);
        let mut button = Button::new(format!(
            "cultivation-plant-{}-{}-{}",
            seed.attribute.index(),
            seed.tier,
            seed.grade.index()
        ))
        .ghost()
        .small()
        .label(text(locale, "cultivation.seed.plant"));
        if !plots_full {
            button = button.on_click({
                let view = view.clone();
                let attribute = seed.attribute;
                let tier = seed.tier;
                let grade = seed.grade;
                move |_, _, cx| {
                    view.update(cx, |view, cx| {
                        let _ = view.pomodoro.try_publish(PomodoroCommand::PlantSeed {
                            attribute,
                            tier,
                            grade,
                        });
                        view.poll_pomodoro(cx);
                    });
                }
            });
        }
        // The name is the click: it says what this herb is worth before money
        // is spent on it or a plot is given up for it. The buttons beside it
        // stay one-click, so asking a question never costs an action.
        let name = div()
            .id(format!(
                "cultivation-seed-name-{}-{}-{}",
                seed.attribute.index(),
                seed.tier,
                seed.grade.index()
            ))
            .flex_1()
            .min_w_0()
            .truncate()
            .text_xs()
            .text_color(grade_paint)
            .child(label)
            .test_support()
            .on_click({
                let view = view.clone();
                move |_, _, cx| {
                    view.update(cx, |view, cx| {
                        view.cultivation_open_seed = if view.cultivation_open_seed == Some(key) {
                            None
                        } else {
                            Some(key)
                        };
                        cx.notify();
                    });
                }
            });
        let mut row = div()
            .id(format!(
                "cultivation-seed-{}-{}-{}",
                seed.attribute.index(),
                seed.tier,
                seed.grade.index()
            ))
            .flex()
            .flex_col()
            .gap_1()
            .py_1()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(name)
                    .child(button)
                    // One seed at a time: a stack of ten is nine seeds and one of
                    // them, and which one the user wants to keep is not a question
                    // a single "sell all" can ask.
                    .child(
                        Button::new(format!(
                            "cultivation-sell-seed-{}-{}-{}",
                            seed.attribute.index(),
                            seed.tier,
                            seed.grade.index()
                        ))
                        .ghost()
                        .small()
                        .label(sell_label(locale, seed.sell_price))
                        .on_click({
                            let view = view.clone();
                            let attribute = seed.attribute;
                            let tier = seed.tier;
                            let grade = seed.grade;
                            move |_, _, cx| {
                                view.update(cx, |view, cx| {
                                    let _ = view.pomodoro.try_publish(PomodoroCommand::SellSeed {
                                        attribute,
                                        tier,
                                        grade,
                                    });
                                    view.poll_pomodoro(cx);
                                });
                            }
                        }),
                    ),
            );
        if open_seed == Some(key) {
            row = row.child(seed_detail(
                view,
                key,
                format!(
                    "{} · {}",
                    seed_facts(locale, seed.attribute, seed.tier, seed.grade),
                    format_text(
                        locale,
                        "cultivation.shop.detail_sell",
                        &[("stones", seed.sell_price.to_string())],
                    ),
                ),
                tokens,
            ));
        }
        row
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
    open_plot: Option<usize>,
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
            row = row.child(plot_cell(
                view,
                index,
                plot,
                open_plot == Some(index),
                now,
                locale,
                tokens,
            ));
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
    open: bool,
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
    let Some(crop) = plot.crop.as_ref() else {
        return cell
            .child(
                div()
                    .text_xs()
                    .text_color(tokens.muted)
                    .child(text(locale, "cultivation.garden.empty")),
            )
            .into_any_element();
    };
    // The name is the click: what is in the ground can be asked what it will
    // pay without giving up the harvest to find out.
    cell = cell.child(
        div()
            .id(format!("cultivation-plot-name-{index}"))
            .text_xs()
            .truncate()
            .child(seed_label(
                locale,
                crop.attribute,
                crop.tier,
                crop.grade,
                crop.name.as_deref(),
            ))
            .text_color(seed_grade_color(crop.grade))
            .test_support()
            .on_click({
                let view = view.clone();
                move |_, _, cx| {
                    view.update(cx, |view, cx| {
                        view.cultivation_open_plot = if view.cultivation_open_plot == Some(index) {
                            None
                        } else {
                            Some(index)
                        };
                        cx.notify();
                    });
                }
            }),
    );
    if open {
        cell = cell.child(
            div()
                .id(format!("cultivation-plot-detail-{index}"))
                .text_xs()
                .text_color(tokens.muted)
                .child(harvest_line(locale, crop.attribute, crop.tier, crop.grade))
                .test_support(),
        );
    }
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
