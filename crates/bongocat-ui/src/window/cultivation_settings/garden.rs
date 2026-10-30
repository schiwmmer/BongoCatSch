//! Seeds and plots: the stacks, the four garden beds, and the harvest.

use super::*;

/// How many seed stacks the satchel lists before the toggle takes over.
///
/// Four, in two rows of two: a seed row is a name, an odds line and two
/// buttons, and a page that is already a column of panels cannot spend eight
/// rows on the satchel. A gardener who holds more than four kinds is a click
/// away from all of them.
const VISIBLE_SEED_COUNT: usize = 4;

/// How many seed stacks one row of the satchel draws.
///
/// Two to a row: a seed needs its name, its odds and its two buttons, and one
/// to a row spends twice the height on the same four facts.
const SEEDS_PER_ROW: usize = 2;

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

/// The 种植 and 卖 keys as one row of fixed width.
///
/// The two buttons are laid out at the two ends of a row of fixed width, so
/// the row is the same width whatever the price is and a column of seeds
/// reads as one column of buttons. Without the fixed width, a 卖 key that
/// carries a four-digit price pushes its 种植 key further left than a
/// neighbour carrying a two-digit one — the two keys of the same column no
/// longer line up, and the satchel reads as scattered rather than tabular.
fn seed_action_row(plant: impl IntoElement, sell: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .w_40()
        .child(plant)
        .child(sell)
}

/// The success rate, with how long the crop would take to come up beside it.
///
/// The two facts read on one line so a gardener comparing two seeds sees both
/// the odds and the wait at a glance: the rate answers "is it worth planting
/// this bed?", the time answers "how long is the plot committed?". The growing
/// time is the domain's own — the same figure the open detail and a planted
/// plot show — so a seed cannot promise a wait the planting will not take.
fn seed_odds_line(
    id: String,
    odds_line: String,
    locale: &str,
    tier: u8,
    tokens: Tokens,
) -> gpui_kit::AnyElement {
    // The growing time keeps its own id so a render test can prove it is drawn
    // next to the rate rather than trusting the rate line's width.
    let growing_id = format!("{id}-growing");
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .text_xs()
        .text_color(tokens.muted)
        .child(odds_line)
        .child(
            div()
                .id(growing_id)
                .test_support()
                .child(growing_line(locale, grow_seconds(tier))),
        )
        .test_support()
        .into_any_element()
}

/// Which seed a row names — and which open-detail slot it owns.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum SeedKey {
    /// A crop seed: one attribute, one tier, one grade.
    Attr(Attribute, u8, SeedGrade),
    /// A 潜质 herb seed: which herb, at which grade.
    Herb(AptitudeHerb, SeedGrade),
}

/// The line under a seed's row: what it pays, what a sale would bring, and —
/// on the shelf and in the satchel — how long it would take to grow.
fn seed_detail(
    view: &Entity<SettingsView>,
    key: SeedKey,
    line: String,
    tokens: Tokens,
) -> gpui_kit::AnyElement {
    let detail_id = match &key {
        SeedKey::Attr(attribute, tier, grade) => {
            format!(
                "cultivation-seed-detail-{}-{}-{}",
                attribute.index(),
                tier,
                grade.index()
            )
        }
        SeedKey::Herb(herb, grade) => {
            format!(
                "cultivation-seed-detail-herb-{}-{}",
                herb.name_key(),
                grade.index()
            )
        }
    };
    div()
        .id(detail_id)
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

/// One 潜质 herb seed waiting to be planted.
///
/// A herb seed is a seed in the ways that matter — it occupies a plot, grows
/// at the herb's own tier, and comes home as a harvest — but it is planted by
/// herb and grade rather than attribute and tier, sells the same way, and
/// names itself from the herb's catalog rather than an attribute's. So it is
/// its own row, a sibling of the attribute-seed rows below.
fn herb_row(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    seed: &SeedView,
    plots_full: bool,
    open_seed: Option<SeedKey>,
    locale: &str,
    tokens: Tokens,
) -> gpui_kit::AnyElement {
    let herb = seed
        .herb
        .expect("herb_row is only ever called on a 潜质 herb seed");
    let grade = seed.grade;
    let key = SeedKey::Herb(herb, grade);
    let label = format!(
        "{} ×{}",
        herb_seed_label(locale, herb, seed.tier, grade),
        seed.count
    );
    let grade_paint = seed_grade_color(grade);
    let mut button = Button::new(format!(
        "cultivation-plant-herb-{}-{}",
        herb.name_key(),
        grade.index()
    ))
    .ghost()
    .small()
    .label(text(locale, "cultivation.seed.plant"));
    if !plots_full {
        button = button.on_click({
            let view = view.clone();
            move |_, _, cx| {
                view.update(cx, |view, cx| {
                    let _ = view
                        .pomodoro
                        .try_publish(PomodoroCommand::PlantHerbSeed { herb, grade });
                    view.poll_pomodoro(cx);
                });
            }
        });
    }
    let name = div()
        .id(format!(
            "cultivation-seed-name-herb-{}-{}",
            herb.name_key(),
            grade.index()
        ))
        .w_full()
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
    let percent = (plant_permille(cultivation.garden_rank, seed.tier, seed.grade) + 5) / 10;
    let odds_line = format_text(
        locale,
        "cultivation.garden.plant_odds",
        &[("percent", percent.to_string())],
    );
    let named = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .child(name)
        .child(seed_odds_line(
            format!(
                "cultivation-seed-odds-herb-{}-{}",
                herb.name_key(),
                grade.index()
            ),
            odds_line,
            locale,
            seed.tier,
            tokens,
        ));
    let mut row = div()
        .id(format!(
            "cultivation-seed-herb-{}-{}",
            herb.name_key(),
            grade.index()
        ))
        .flex_1()
        .min_w_0()
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
                .child(named)
                .child(seed_action_row(
                    button,
                    Button::new(format!(
                        "cultivation-sell-seed-herb-{}-{}",
                        herb.name_key(),
                        grade.index()
                    ))
                    .ghost()
                    .small()
                    .label(sell_label(locale, seed.sell_price))
                    .on_click({
                        let view = view.clone();
                        move |_, _, cx| {
                            view.update(cx, |view, cx| {
                                let _ = view
                                    .pomodoro
                                    .try_publish(PomodoroCommand::SellHerbSeed { herb, grade });
                                view.poll_pomodoro(cx);
                            });
                        }
                    }),
                )),
        );
    if open_seed == Some(key) {
        row = row.child(seed_detail(
            view,
            key,
            format!(
                "{} · {}",
                super::shop::herb_effect(herb, Some(grade), locale),
                format_text(
                    locale,
                    "cultivation.shop.detail_sell",
                    &[("stones", seed.sell_price.to_string())],
                ),
            ),
            tokens,
        ));
    }
    row.into_any_element()
}

/// The seeds waiting to be planted.
///
/// The title carries the whole satchel — every seed in every stack — and the
/// list under it shows five stacks at a time, the lowest grade first. A satchel
/// of twenty stacks is a wall on a page that is already a ladder, and the cheap
/// stacks are the ones a garden can actually bring up; the rest are one click
/// away.
pub(super) fn seed_panel(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    open_seed: Option<SeedKey>,
    open: bool,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let total: u32 = cultivation
        .seeds
        .iter()
        .fold(0, |sum, seed| sum.saturating_add(seed.count));
    let panel = div()
        .id("cultivation-seeds")
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(section_title(locale, "cultivation.seeds.title", tokens))
                .child(
                    div()
                        .id("cultivation-seeds-total")
                        .text_xs()
                        .text_color(tokens.muted)
                        .child(format_text(
                            locale,
                            "cultivation.seeds.total",
                            &[("count", total.to_string())],
                        ))
                        .test_support(),
                ),
        );
    if cultivation.seeds.is_empty() {
        return panel.child(
            div()
                .text_xs()
                .text_color(tokens.muted)
                .child(text(locale, "cultivation.seeds.empty")),
        );
    }
    let plots_full = cultivation.plots.iter().all(|plot| plot.crop.is_some());
    // Lowest grade first, then the cheapest bed: the domain hands the satchel
    // over highest-first because that is how the shelf prices it, and this panel
    // is asking a different question — what can be planted today.
    let mut stacks: Vec<&SeedView> = cultivation.seeds.iter().collect();
    stacks.sort_by(|left, right| {
        left.grade.cmp(&right.grade).then_with(|| {
            left.tier
                .cmp(&right.tier)
                .then_with(|| left.herb.is_some().cmp(&right.herb.is_some()))
                .then_with(|| left.attribute.index().cmp(&right.attribute.index()))
        })
    });
    let stacks_total = stacks.len();
    let shown = if open {
        stacks_total
    } else {
        stacks_total.min(VISIBLE_SEED_COUNT)
    };
    // Two to a row, and a row of two is the unit the fold counts in: the last
    // row is whole whether four or five are showing, so nothing here has to
    // decide what a half-empty row looks like.
    let rows: Vec<Vec<&SeedView>> = stacks
        .into_iter()
        .take(shown)
        .collect::<Vec<_>>()
        .chunks(SEEDS_PER_ROW)
        .map(|chunk| chunk.to_vec())
        .collect();
    let mut panel = panel.children(rows.into_iter().enumerate().map(|(row_index, row)| {
        div()
            .id(format!("cultivation-seed-row-{row_index}"))
            .flex()
            .flex_row()
            .items_start()
            .gap_2()
            .test_support()
            .children(row.into_iter().map(|seed| {
                // A 潜质 herb seed is its own row: planted and sold by herb and grade,
                // and named from the herb rather than an attribute. The attribute rows
                // below keep their own shape.
                if seed.herb.is_some() {
                    return herb_row(
                        view,
                        cultivation,
                        seed,
                        plots_full,
                        open_seed,
                        locale,
                        tokens,
                    );
                }
                let key = SeedKey::Attr(seed.attribute, seed.tier, seed.grade);
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
                    .w_full()
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
                                view.cultivation_open_seed =
                                    if view.cultivation_open_seed == Some(key) {
                                        None
                                    } else {
                                        Some(key)
                                    };
                                cx.notify();
                            });
                        }
                    });
                // The odds, under every seed's name and before anything is planted: this
                // garden, this herb, right now. A success rate the user can read makes
                // planting a decision rather than a purchase; the number is the domain's
                // own, so the page cannot promise odds the planting will not get.
                let percent =
                    (plant_permille(cultivation.garden_rank, seed.tier, seed.grade) + 5) / 10;
                let odds_line = format_text(
                    locale,
                    "cultivation.garden.plant_odds",
                    &[("percent", percent.to_string())],
                );
                let named = div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(name)
                    .child(seed_odds_line(
                        format!(
                            "cultivation-seed-odds-{}-{}-{}",
                            seed.attribute.index(),
                            seed.tier,
                            seed.grade.index()
                        ),
                        odds_line,
                        locale,
                        seed.tier,
                        tokens,
                    ));
                let mut row = div()
                    .id(format!(
                        "cultivation-seed-{}-{}-{}",
                        seed.attribute.index(),
                        seed.tier,
                        seed.grade.index()
                    ))
                    // Half the row: a seed is a name, an odds line and two buttons, and
                    // two to a row is what the satchel spends its height on.
                    .flex_1()
                    .min_w_0()
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
                            .child(named)
                            .child(seed_action_row(
                                button,
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
                                            let _ = view.pomodoro.try_publish(
                                                PomodoroCommand::SellSeed {
                                                    attribute,
                                                    tier,
                                                    grade,
                                                },
                                            );
                                            view.poll_pomodoro(cx);
                                        });
                                    }
                                }),
                            )),
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
                row.into_any_element()
            }))
    }));
    if stacks_total > VISIBLE_SEED_COUNT {
        let key = if open {
            "cultivation.seeds.show_less"
        } else {
            "cultivation.seeds.show_all"
        };
        let label = if open {
            text(locale, key).to_string()
        } else {
            format_text(locale, key, &[("count", stacks_total.to_string())])
        };
        panel = panel.child(
            div()
                .id("cultivation-seeds-toggle")
                .text_xs()
                .text_color(tokens.accent)
                .child(label)
                .test_support()
                .on_click({
                    let view = view.clone();
                    move |_, _, cx| {
                        view.update(cx, |view, cx| {
                            view.cultivation_seeds_open = !view.cultivation_seeds_open;
                            cx.notify();
                        });
                    }
                }),
        );
    }
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

/// The garden's own ladder: how well it is tended, what the next rung asks for,
/// and what it costs.
///
/// Above the plots rather than beside them, because every percentage further
/// down the page is read against this number — the same seed lines up
/// differently here than it did one rank ago, and knowing why is the difference
/// between odds displayed and odds understood.
fn garden_rank_row(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
) -> gpui_kit::AnyElement {
    let mut row = div()
        .id("cultivation-garden-rank")
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .child(div().text_xs().text_color(tokens.muted).child(format_text(
            locale,
            "cultivation.garden.rank",
            &[("rank", cultivation.garden_rank.to_string())],
        )))
        .test_support();
    let Some(price) = cultivation.garden_upgrade_cost else {
        return row
            .child(
                div()
                    .text_xs()
                    .text_color(tokens.muted)
                    .child(text(locale, "cultivation.garden.rank_top")),
            )
            .into_any_element();
    };
    row = row
        .child(div().text_xs().text_color(tokens.muted).child(format_text(
            locale,
            "cultivation.garden.xp",
            &[
                (
                    "have",
                    compact_number(u64::try_from(cultivation.garden_xp).unwrap_or(0)),
                ),
                (
                    "need",
                    compact_number(u64::try_from(cultivation.garden_xp_needed).unwrap_or(0)),
                ),
            ],
        )))
        .child(
            Button::new("cultivation-upgrade-garden")
                .ghost()
                .small()
                .label(format_text(
                    locale,
                    "cultivation.garden.upgrade",
                    &[("price", compact_number(u64::from(price)))],
                ))
                .on_click({
                    let view = view.clone();
                    move |_, _, cx| {
                        view.update(cx, |view, cx| {
                            let _ = view.pomodoro.try_publish(PomodoroCommand::UpgradeGarden);
                            view.poll_pomodoro(cx);
                        });
                    }
                }),
        );
    row.into_any_element()
}

/// The garden: the plots four to a row — empty, growing, or ready to pick —
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
        .child(garden_rank_row(view, cultivation, locale, tokens))
        .test_support();
    for (row_index, chunk) in cultivation.plots.chunks(4).enumerate() {
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

/// One plot as a cell a row of four can hold: the herb's name on top, and
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
    // A bed past its halfway mark may be holding a dead crop. That is the whole
    // point of the reveal — the gardener learns at the midpoint, not under the
    // seed — so the cell says so and offers the one thing a dead bed is good
    // for: getting out of the way.
    let dead = plot.is_crop_dead();
    // The name is the click: what is in the ground can be asked what it will
    // pay without giving up the harvest to find out.
    cell = cell.child(
        div()
            .id(format!("cultivation-plot-name-{index}"))
            .text_xs()
            .truncate()
            .child(match crop.herb {
                // A herb crop is named by its herb: its `attribute` is the
                // don't-care a herb seed carries, and printing it would call a
                // bed of 悟心草 a bed of 力量.
                Some(herb) => herb_seed_label(locale, herb, crop.tier, crop.grade),
                None => seed_label(
                    locale,
                    crop.attribute,
                    crop.tier,
                    crop.grade,
                    crop.name.as_deref(),
                ),
            })
            .text_color(if dead {
                tokens.muted
            } else {
                seed_grade_color(crop.grade)
            })
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
    if dead {
        return cell
            .child(
                div()
                    .id(format!("cultivation-plot-withered-{index}"))
                    .text_xs()
                    .text_color(tokens.muted)
                    .child(text(locale, "cultivation.garden.withered"))
                    .test_support(),
            )
            .child(
                Button::new(format!("cultivation-dig-{index}"))
                    .ghost()
                    .small()
                    .label(text(locale, "cultivation.garden.dig"))
                    .on_click({
                        let view = view.clone();
                        move |_, _, cx| {
                            view.update(cx, |view, cx| {
                                let _ = view
                                    .pomodoro
                                    .try_publish(PomodoroCommand::DigPlot { plot: index });
                                view.poll_pomodoro(cx);
                            });
                        }
                    }),
            )
            .into_any_element();
    }
    if open {
        cell = cell.child(
            div()
                .id(format!("cultivation-plot-detail-{index}"))
                .text_xs()
                .text_color(tokens.muted)
                .child(match crop.herb {
                    // What a herb crop pays is 潜质, not an attribute point: the
                    // satchel's own line, so a bed cannot promise a number the
                    // harvest will not pay.
                    Some(herb) => super::shop::herb_effect(herb, Some(crop.grade), locale),
                    None => harvest_line(locale, crop.attribute, crop.tier, crop.grade),
                })
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
