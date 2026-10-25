//! The attributes and the derived figures: what growth buys, where
//! the free points go, and the two power lines every fight reads.

use super::*;
use bongocat_pomodoro::{AptitudeHerb, Derived, DerivedBreakdown, Fighter};

/// The four attributes, what each source contributes, and the free points the
/// levels have paid.
///
/// A level pays one point the user places by hand, so the panel is also the one
/// place that spends them: the point is offered on the attribute itself rather
/// than in a separate editor, because "where does this point go" is answered by
/// looking at the four numbers it could raise.
pub(super) fn attributes_panel(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let free = cultivation.free_points;
    let chips = Attribute::ALL.into_iter().map(|attribute| {
        let mut chip = div()
            .id(format!("cultivation-attribute-{}", attribute.index()))
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(tokens.overlay)
            .child(
                div()
                    .text_xs()
                    .text_color(tokens.muted)
                    .child(text(locale, attribute.name_key())),
            )
            .child(
                div()
                    .text_xs()
                    .child(attribute.of(cultivation.attributes).to_string()),
            );
        if free > 0 {
            chip = chip.child(allocate_button(
                view,
                attribute,
                1,
                "cultivation.attribute.add",
                locale,
            ));
        }
        // Once ten or more points are banked, offer a ten-at-a-time button beside
        // the single one so the user is not forced to click ten times.
        if free >= 10 {
            chip = chip.child(allocate_button(
                view,
                attribute,
                10,
                "cultivation.attribute.add_ten",
                locale,
            ));
        }
        // The explanation hangs off the whole chip rather than off the name: the
        // name is a handful of glyphs wide, and a hint that only shows when the
        // pointer lands exactly on them reads as a hint that does not work.
        let name = text(locale, attribute.name_key());
        let explanation = text(locale, attribute.desc_key());
        let hint_tokens = tokens;
        HoverCard::new(format!("cultivation-attribute-hint-{}", attribute.index()))
            .trigger(chip.test_support())
            .open_delay(Duration::from_millis(120))
            .content(move |_, _, _| {
                attribute_hint(attribute.index(), name, explanation, hint_tokens)
            })
    });
    // Every source the total is made of, and then the total itself.
    //
    // The panel used to print five of them and stop, and the five did not add up
    // to the number in the corner: the points the user had placed by hand were
    // missing, the gear's outright combat numbers were missing, and the pairing
    // bonus the two schools pay was counted once per source instead of once over
    // the whole body. A breakdown that does not reach its own total is worse than
    // none — it reads as a bug in the arithmetic rather than in the line — so
    // every part the domain knows about is printed, a residual 配对 line carries
    // whatever the split cannot see, and the row ends on the total it equals.
    let breakdown = cultivation.power_breakdown;
    let parts = [
        ("cultivation.power_level", breakdown.level),
        ("cultivation.power_allocated", breakdown.allocated),
        ("cultivation.power_equipment", breakdown.equipment),
        (
            "cultivation.power_equipment_bonus",
            breakdown.equipment_bonus,
        ),
        ("cultivation.power_crop", breakdown.crop),
        ("cultivation.power_art", breakdown.art),
        ("cultivation.power_art_bonus", breakdown.art_bonus),
        ("cultivation.power_synergy", breakdown.synergy),
    ]
    .into_iter()
    .map(|(key, value)| format_text(locale, key, &[("value", value.to_string())]))
    .collect::<Vec<_>>()
    .join(" · ");
    let parts_line = format!(
        "{} ＝ {}",
        parts,
        format_text(
            locale,
            "cultivation.power_total",
            &[("value", breakdown.total.to_string())],
        ),
    );
    // 外功 is not part of the sum: it is what a blow lands for, not what the body
    // is worth, so it is priced on its own line rather than folded into a total
    // it would have broken.
    let external_line = format_text(
        locale,
        "cultivation.power_external",
        &[("value", breakdown.external.to_string())],
    );

    let panel = div()
        .id("cultivation-attributes")
        .flex()
        .flex_col()
        .gap_1()
        .child(section_title(locale, "cultivation.attributes", tokens))
        .child(div().flex().flex_row().flex_wrap().gap_2().children(chips));
    // Only shown while a point is waiting: a line that reads "0 points" is a
    // permanent reminder of nothing.
    let panel = if free == 0 {
        panel
    } else {
        panel.child(
            div()
                .id("cultivation-free-points")
                .text_xs()
                .text_color(tokens.accent)
                .child(format_text(
                    locale,
                    "cultivation.free_points",
                    &[("points", free.to_string())],
                )),
        )
    };
    panel.child(
        div()
            .id("cultivation-power-breakdown")
            .flex()
            .flex_col()
            .gap_0p5()
            .text_xs()
            .text_color(tokens.muted)
            .child(div().child(parts_line))
            .child(
                div()
                    .id("cultivation-power-external-line")
                    .child(external_line)
                    .test_support(),
            )
            .test_support(),
    )
}

/// The seven numbers the four attributes turn into, and the two schools they
/// fight in.
///
/// Shown as read-only chips rather than rows with a "+" beside them: these are
/// what the four already bought, and a point can only be spent on the four
/// themselves. The hint line says so, because a panel full of numbers with no
/// button on them otherwise reads as unfinished.
pub(super) fn derived_panel(
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let stats = CombatStats::of_derived(cultivation.derived);
    let mut rows = derived_rows(&stats, locale);
    // 潜质 rides along: it is shown with the rest, and it is the one number
    // here a fight does not use — which is why it is appended rather than
    // folded into the eight.
    rows.push((
        "cultivation.aptitude",
        format_text(
            locale,
            "cultivation.aptitude",
            &[("value", super::tenths_text(cultivation.aptitude_tenths))],
        ),
    ));
    // The arithmetic behind each number, matched to the number by its catalog
    // key: the chip that prints the value also prints the sum that produced
    // it, so the two cannot be read as two different numbers.
    // 潜质 has no attribute recipe behind it — it is born on its own band and
    // raised by herbs — so it carries no breakdown: a formula reading "根骨 × 0"
    // would be a sum that explains nothing.
    let breakdowns: std::collections::HashMap<&'static str, &DerivedBreakdown> = cultivation
        .derived_breakdowns
        .iter()
        .filter(|breakdown| breakdown.key != "cultivation.aptitude")
        .map(|breakdown| (breakdown.key, breakdown))
        .collect();
    let chips = rows.into_iter().map(|(key, line)| {
        let breakdown = breakdowns.get(key).copied();
        derived_chip(key, line, breakdown, locale, tokens)
    });
    div()
        .id("cultivation-derived")
        .flex()
        .flex_col()
        .gap_1()
        .child(section_title(locale, "cultivation.derived", tokens))
        .child(div().flex().flex_row().flex_wrap().gap_2().children(chips))
        .child(
            div()
                .text_xs()
                .text_color(tokens.muted)
                .child(text(locale, "cultivation.derived_hint")),
        )
}

/// The eight shown numbers a fighter walks in with.
///
/// The page reads them off a [`Derived`], the fight report off a [`Fighter`],
/// and the two have to agree number for number: the popup shows what the fight
/// actually used, so both read the same eight through the same rows rather than
/// through a second table that could drift from the page's.
pub(in crate::window) struct CombatStats {
    pub attack: u32,
    pub health: u32,
    pub inner: u32,
    pub spell: u32,
    pub resist: u32,
    pub speed: u32,
    /// 命中, in parts per thousand.
    pub hit: u32,
    /// 闪避, in parts per thousand.
    pub evade: u32,
}

impl CombatStats {
    /// The eight numbers a set of attributes is worth.
    pub(super) fn of_derived(derived: Derived) -> Self {
        Self {
            attack: derived.attack,
            health: derived.health,
            inner: derived.inner,
            spell: derived.spell,
            resist: derived.resist,
            speed: derived.speed,
            hit: derived.hit,
            evade: derived.evade,
        }
    }

    /// The eight numbers a fighter stood up with.
    pub(in crate::window) fn of_fighter(fighter: &Fighter) -> Self {
        Self {
            attack: fighter.attack,
            health: fighter.health,
            inner: fighter.inner,
            spell: fighter.spell,
            resist: fighter.resist,
            speed: fighter.speed,
            hit: fighter.hit,
            evade: fighter.evade,
        }
    }
}

/// The eight numbers as the page says them, each with the catalog key it is
/// read through.
///
/// The keys are spelled out rather than built from a name: the catalog test
/// reads the source for the keys it checks, and a key only a `format!` could
/// produce would look unused to it. 命中 and 闪避 are held in parts per
/// thousand and printed as percentages, the way the fight reads them.
pub(in crate::window) fn derived_rows(
    stats: &CombatStats,
    locale: &str,
) -> Vec<(&'static str, String)> {
    [
        ("cultivation.attack", stats.attack),
        ("cultivation.health", stats.health),
        ("cultivation.inner", stats.inner),
        ("cultivation.spell", stats.spell),
        ("cultivation.resist", stats.resist),
        ("cultivation.speed", stats.speed),
        ("cultivation.hit", stats.hit),
        ("cultivation.evade", stats.evade),
    ]
    .into_iter()
    .map(|(key, value)| {
        (
            key,
            format_text(locale, key, &[("value", value.to_string())]),
        )
    })
    .collect()
}

/// One derived number: its name and its value together, as the catalog line
/// spells them.
///
/// The hover carries the arithmetic that produced the number — the base, the
/// attribute it reads, what one point of that attribute adds, and the ceiling
/// it was held to — so the value beside it can be checked by hand rather than
/// taken on trust. The numbers come from the domain's own breakdown, which is
/// computed by the same table the fight reads, so the sum on the card is the
/// sum that ran.
fn derived_chip(
    key: &'static str,
    line: String,
    breakdown: Option<&DerivedBreakdown>,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    // A chip with no button on it reads as a number the page forgot how to change,
    // so each carries the sentence that says where it comes from. The strings are
    // read out here because the hover body outlives this call and cannot borrow
    // `locale`.
    let title = text(locale, "cultivation.hint.derived_title");
    // 潜质 is the one number here a fight never reads, so the sentence that says
    // "it moves when the four move" is the wrong sentence for it: what it
    // actually decides is how often a 功法 attempt fails, and the card says so.
    let body = if key == "cultivation.aptitude" {
        text(locale, "cultivation.hint.aptitude_hint")
    } else {
        text(locale, "cultivation.hint.derived_hint_each")
    };
    // The arithmetic, spelled out with the very numbers the domain computed.
    let formula = breakdown.map(|breakdown| derived_formula(breakdown, locale));
    let baseline = breakdown.map(|breakdown| derived_baseline_line(breakdown, locale));
    let chip = div()
        // The key carries its own `cultivation.` prefix; the element id drops
        // it so the tree reads `cultivation-derived-attack`.
        .id(format!(
            "cultivation-derived-{}",
            key.trim_start_matches("cultivation.")
        ))
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .px_2()
        .py_1()
        .rounded_md()
        .bg(tokens.overlay)
        .child(div().text_xs().text_color(tokens.text).child(line));
    let hover = HoverCard::new(format!(
        "cultivation-hint-{}",
        key.trim_start_matches("cultivation.derived.")
    ))
    .trigger(chip.test_support())
    .open_delay(Duration::from_millis(120));
    match formula {
        Some(formula) => {
            let formula = formula.clone();
            let baseline = baseline.unwrap_or_default();
            hover
                .content(move |_, _, _| {
                    derived_hint_body(
                        format!("cultivation-hint-{key}-body"),
                        title,
                        body,
                        formula.clone(),
                        baseline.clone(),
                        tokens,
                    )
                })
                .into_any_element()
        }
        None => hover
            .content(move |_, _, _| {
                hint_body_with_title(format!("cultivation-hint-{key}-body"), title, body, tokens)
            })
            .into_any_element(),
    }
}

/// The arithmetic behind one derived number, as a line the user can check.
///
/// Spelled out with the numbers themselves rather than with a rule: "基础 50 +
/// 根骨 787 × 8 = 6346" can be added up on the page, and "由属性换算" cannot. A
/// number held to a ceiling says so, because a number that stops growing and
/// does not say why reads as a bug.
///
/// Every word in it is a catalog entry, including the attribute's own name: a
/// formula that spells `cultivation.attribute.root` is a formula that leaked a
/// key onto the page. 命中 and 闪避 are the two per-mille numbers, and they are
/// printed in percent with the percent sign, so the sum on the card is in the
/// same unit as the chip it explains.
pub(in crate::window) fn derived_formula(breakdown: &DerivedBreakdown, locale: &str) -> String {
    let figure = |value: u32| figure_value(value, breakdown.per_mille);
    let unit = if breakdown.per_mille { "%" } else { "" };
    // What gear and 功法 added outright rides in the sum when they paid any:
    // the arithmetic the card prints is the arithmetic that ran, and a number
    // that moved without the sum saying so would be a sum the user cannot
    // check.
    let formula = if breakdown.bonus > 0 {
        format_text(
            locale,
            "cultivation.formula.line_bonus",
            &[
                ("base", figure(breakdown.base)),
                (
                    "attribute",
                    text(locale, breakdown.attribute.name_key()).to_owned(),
                ),
                ("value", figure(breakdown.attribute_value.max(0) as u32)),
                ("per", figure(breakdown.per_point)),
                ("bonus", format!("{}{unit}", figure(breakdown.bonus))),
                ("result", format!("{}{unit}", figure(breakdown.result))),
            ],
        )
    } else {
        format_text(
            locale,
            "cultivation.formula.line",
            &[
                ("base", figure(breakdown.base)),
                (
                    "attribute",
                    text(locale, breakdown.attribute.name_key()).to_owned(),
                ),
                ("value", figure(breakdown.attribute_value.max(0) as u32)),
                ("per", figure(breakdown.per_point)),
                ("result", format!("{}{unit}", figure(breakdown.result))),
            ],
        )
    };
    match breakdown.cap {
        Some(cap) if breakdown.result >= cap => format_text(
            locale,
            "cultivation.formula.capped",
            &[
                ("formula", formula),
                ("cap", format!("{}{unit}", figure(cap))),
            ],
        ),
        _ => formula,
    }
}

/// A derived number as the page prints it: a plain count, or one decimal of a
/// percent for the two per-mille numbers.
fn figure_value(value: u32, per_mille: bool) -> String {
    if !per_mille {
        return value.to_string();
    }
    let tenths = value / 10;
    let tenth = value % 10;
    if tenth == 0 {
        tenths.to_string()
    } else {
        format!("{tenths}.{tenth}")
    }
}

/// A figure with its unit on the end, for a line that prints one on its own
/// rather than inside the formula's sentence.
fn figure_with_unit(value: u32, per_mille: bool) -> String {
    let figure = figure_value(value, per_mille);
    if per_mille {
        format!("{figure}%")
    } else {
        figure
    }
}

/// The number against what the cultivator's own level is worth, with the
/// difference spelled out.
///
/// The reference is not a second table: it is the same arithmetic run over the
/// attributes the level itself carries — nothing worn, nothing trained, nothing
/// allocated — so "等级 40 的基础是 X，你是 Y" is a sentence that can be checked
/// against the nine chips beside it. The sign is printed rather than implied,
/// because "比同级基础多/少" is the question the line answers: `+120` is above a
/// naked body of this level, `-40` is under it, and `±0` is exactly on it.
pub(in crate::window) fn derived_baseline_line(
    breakdown: &DerivedBreakdown,
    locale: &str,
) -> String {
    let delta = i64::from(breakdown.result) - i64::from(breakdown.baseline);
    let signed = match delta {
        0 => "±0".to_owned(),
        delta if delta > 0 => format!(
            "+{}",
            figure_with_unit(
                u32::try_from(delta).unwrap_or(u32::MAX),
                breakdown.per_mille
            )
        ),
        delta => format!(
            "-{}",
            figure_with_unit(
                u32::try_from(delta.unsigned_abs()).unwrap_or(u32::MAX),
                breakdown.per_mille
            )
        ),
    };
    format_text(
        locale,
        "cultivation.formula.baseline",
        &[
            ("level", breakdown.level.to_string()),
            (
                "baseline",
                figure_with_unit(breakdown.baseline, breakdown.per_mille),
            ),
            (
                "result",
                figure_with_unit(breakdown.result, breakdown.per_mille),
            ),
            ("delta", signed),
        ],
    )
}

/// A hover body carrying the arithmetic behind a number.
///
/// The formula is its own line rather than part of the paragraph: it is a sum
/// to be checked, and a sum buried in a sentence is a sum nobody checks.
fn derived_hint_body(
    id: impl Into<gpui_kit::ElementId>,
    title: &'static str,
    body: &'static str,
    formula: String,
    baseline: String,
    tokens: Tokens,
) -> impl IntoElement {
    div()
        .id(id.into())
        .flex()
        .flex_col()
        .gap_1()
        .w(px(260.0))
        .child(div().text_xs().text_color(tokens.text).child(title))
        .child(
            div()
                .text_xs()
                .text_color(tokens.accent)
                .whitespace_normal()
                .child(formula),
        )
        // What the same level would stand at with nothing worn or trained, and
        // which way this build is off it. Its own line because it is a
        // comparison to read, not a sentence to skim.
        .child(
            div()
                .id("cultivation-hint-baseline")
                .text_xs()
                .text_color(tokens.text)
                .whitespace_normal()
                .child(baseline),
        )
        .child(
            div()
                .text_xs()
                .text_color(tokens.muted)
                .whitespace_normal()
                .child(body),
        )
        .test_support()
}

/// A "+1" / "+10" allocation button for one attribute.
fn allocate_button(
    view: &Entity<SettingsView>,
    attribute: Attribute,
    points: u32,
    label_key: &'static str,
    locale: &str,
) -> impl IntoElement {
    Button::new(format!(
        "cultivation-allocate-{}-{points}",
        attribute.index()
    ))
    .ghost()
    .small()
    .label(text(locale, label_key))
    .on_click({
        let view = view.clone();
        move |_, _, cx| {
            view.update(cx, |view, cx| {
                let _ = view
                    .pomodoro
                    .try_publish(PomodoroCommand::AllocateAttribute { attribute, points });
                view.poll_pomodoro(cx);
            });
        }
    })
}

/// A hover body carrying a heading and a paragraph.
///
/// The heading is what the chip does not already say. A school's chip is two bare
/// numbers, so the card has to name them; a discipline's title already names its
/// own row, so the card only adds the sentence that separates the layer.
fn hint_body_with_title(
    id: impl Into<gpui_kit::ElementId>,
    title: &'static str,
    body: &'static str,
    tokens: Tokens,
) -> impl IntoElement {
    div()
        .id(id.into())
        .flex()
        .flex_col()
        .gap_1()
        .w(px(240.0))
        .child(div().text_xs().text_color(tokens.text).child(title))
        .child(
            div()
                .text_xs()
                .text_color(tokens.muted)
                .whitespace_normal()
                .child(body),
        )
        .test_support()
}

/// The name and the explanation an attribute's chip shows on hover.
///
/// The card is an element rather than a one-line tooltip string because the
/// explanation is a sentence: it says which school the attribute feeds and which
/// partner it is scored against, and that does not fit under a pointer.
fn attribute_hint(
    index: usize,
    name: &'static str,
    explanation: &'static str,
    tokens: Tokens,
) -> impl IntoElement {
    div()
        .id(format!("cultivation-attribute-hint-body-{index}"))
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
                .child(explanation),
        )
        .test_support()
}

/// 潜质 and the three things that move it.
///
/// One panel rather than three chips scattered around, because the number and
/// its only three sources are one decision: a user reading "潜质 4.7" has to be
/// able to see, in the same glance, that the way up is a herb and that every
/// hundred is a gate a pill opens.
pub(super) fn aptitude_panel(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let herbs = AptitudeHerb::ALL
        .into_iter()
        .zip(cultivation.aptitude_herbs)
        .map(|(herb, held)| {
            let mut chip = div()
                .id(format!("cultivation-herb-{}", herb_index(herb)))
                .flex()
                .flex_row()
                .items_center()
                .gap_1()
                .px_2()
                .py_1()
                .rounded_md()
                .bg(tokens.overlay)
                .child(
                    div()
                        .text_xs()
                        .text_color(tokens.muted)
                        .child(text(locale, herb.name_key())),
                )
                .child(div().text_xs().child(format!("×{held}")));
            // Only the two herbs can be fed: a 洗髓丹 is spent by the move that
            // crosses an even hundred, so a button of its own would be a button
            // that has nothing to do until the herb beside it is pressed.
            if herb.gain_tenths().is_some() {
                chip = chip.child(
                    Button::new(format!("cultivation-use-herb-{}", herb_index(herb)))
                        .ghost()
                        .small()
                        .label(text(locale, "cultivation.aptitude_use"))
                        .disabled(held == 0)
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
                        }),
                );
            }
            chip.test_support()
        });
    // The number, the three items and the sentence that explains them are one
    // row rather than four: this panel sits under a long column of others, and a
    // block of its own height would push the fight and the walk off the fold.
    // The explanation hangs off the number instead of taking a line of its own.
    let hint_title = text(locale, "cultivation.aptitude_title");
    let hint_body = text(locale, "cultivation.aptitude_hint");
    div()
        .id("cultivation-aptitude")
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(
                    HoverCard::new("cultivation-hint-aptitude")
                        .trigger(
                            div()
                                .id("cultivation-aptitude-value")
                                .text_xs()
                                .child(format_text(
                                    locale,
                                    "cultivation.aptitude",
                                    &[("value", super::tenths_text(cultivation.aptitude_tenths))],
                                ))
                                .test_support(),
                        )
                        .open_delay(Duration::from_millis(120))
                        .content(move |_, _, _| {
                            hint_body_with_title(
                                "cultivation-hint-aptitude-body",
                                hint_title,
                                hint_body,
                                tokens,
                            )
                        })
                        .into_any_element(),
                )
                .children(herbs),
        )
}

/// Where a 潜质 item sits in the panel's row: the enum's own order.
fn herb_index(herb: AptitudeHerb) -> usize {
    AptitudeHerb::ALL
        .iter()
        .position(|candidate| *candidate == herb)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bongocat_pomodoro::PLAYER_NAME;

    /// The bug this guards: the chip keys already carry their `cultivation.`
    /// prefix, and prefixing them a second time made the lookup miss and every
    /// chip render its own key — a panel reading `cultivation.cultivation.attack`.
    #[test]
    fn a_derived_chip_says_the_number_rather_than_the_key() {
        let stats = CombatStats::of_derived(Derived {
            attack: 1_234,
            ..Derived::default()
        });
        let rows = derived_rows(&stats, "zh-CN");
        let (key, line) = &rows[0];
        assert_eq!(*key, "cultivation.attack");
        assert!(line.contains("1234"), "the chip carries the number: {line}");
        assert!(
            !line.contains("cultivation."),
            "and never the key it was read through: {line}"
        );
    }

    /// The comparison line says which way the build is off what its level alone
    /// pays, with the sign in front: `+` above, `-` below, `±0` on it, and the
    /// two per-mille numbers in percent like every other line on the page.
    #[test]
    fn the_baseline_line_carries_the_sign_of_the_gap() {
        let breakdown = |result: u32, baseline: u32, per_mille: bool| DerivedBreakdown {
            key: "cultivation.attack",
            attribute: Attribute::Strength,
            attribute_value: 100,
            base: 10,
            per_point: 3,
            cap: None,
            computed: result,
            result,
            bonus: 0,
            level: 40,
            baseline,
            per_mille,
        };
        let above = derived_baseline_line(&breakdown(310, 250, false), "zh-CN");
        assert!(
            above.contains("+60"),
            "250 of level 40 against a build of 310: {above}"
        );
        assert!(
            above.contains("40") && above.contains("250"),
            "the line names the level and its base: {above}"
        );
        let below = derived_baseline_line(&breakdown(250, 310, false), "zh-CN");
        assert!(below.contains("-60"), "310 against 250: {below}");
        let even = derived_baseline_line(&breakdown(310, 310, false), "zh-CN");
        assert!(even.contains("±0"), "the same number twice: {even}");
        // 命中 is held in parts per thousand, so the gap is printed in percent.
        let percent = derived_baseline_line(&breakdown(800, 1_000, true), "en-US");
        assert!(percent.contains("-20%"), "800‰ against 1000‰: {percent}");
        assert!(
            percent.contains("100%"),
            "and the level's own base reads 100%: {percent}"
        );
    }

    /// The popup reads a fighter through the same rows the page reads its
    /// derived numbers through, so the two cannot drift apart.
    #[test]
    fn a_fighter_reads_the_same_eight_numbers() {
        let fighter = Fighter {
            name: PLAYER_NAME.to_owned(),
            health: 900,
            attack: 300,
            inner: 120,
            spell: 240,
            resist: 160,
            speed: 40,
            hit: 800,
            evade: 100,
            art: None,
        };
        let stats = CombatStats::of_fighter(&fighter);
        assert_eq!(stats.health, 900);
        let rows = derived_rows(&stats, "zh-CN");
        assert_eq!(rows.len(), 8, "the eight the fight uses");
        assert!(rows[1].1.contains("900"), "生命值 reads its number");
        // 命中 and 闪避 are held in parts per thousand and printed as percent.
        assert!(
            rows[6].1.contains("80"),
            "命中 reads as percent: {}",
            rows[6].1
        );
        assert!(
            rows[7].1.contains("10"),
            "闪避 reads as percent: {}",
            rows[7].1
        );
    }
}
