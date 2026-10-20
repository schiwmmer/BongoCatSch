//! The cultivation subview: the realm, the attributes it buys, the gear and the
//! herb garden that add to them, and the guardian at the end of the realm.
//!
//! Everything here is a read of [`bongocat_pomodoro::CultivationView`] plus the
//! commands the domain already accepts: the page renders what the worker
//! published and publishes what the user clicked, exactly as the task list does.
//! It owns no state of its own — no draft, no selection — because every control
//! is a single click whose result is already in the snapshot.
//!
//! The page is deliberately one column of rows rather than a grid: `gpui-kit`
//! has no sub-page routing (ADR-0091), so the whole ladder has to fit in one
//! scrolling body, and a row of "label — value — action" is what stays readable
//! in every one of the eight catalogs.

use super::*;
use bongocat_i18n::{compact_number, format_text, text};
use bongocat_pomodoro::{
    ArtKind, ArtOutcome, ArtView, Attribute, CultivationView, Derived, DerivedBreakdown, DropView,
    EquipSlot, ExplorationOutcome, Fighter, ItemView, MAX_LEVEL, MasteredArtView, PomodoroCommand,
    Realm, ShopOffer, ShopOfferView, WalkRecord, blueprint,
};

use gpui_kit::component::Sizable as _;
use gpui_kit::component::hover_card::HoverCard;
// `test_support()` marks the elements the headless window tests observe; without
// `gpui-kit`'s `test-support` feature it is the identity function.
use gpui_kit::base::TestSupportExt as _;
use std::time::Duration;

/// How wide the growth bar is drawn, and therefore how wide its filled part is.
const GROWTH_BAR_WIDTH: f32 = 180.0;

/// Build the search keywords so the page is reachable from the settings search
/// box even though its title lives under the `navigation` namespace. The keys
/// are leaf catalog strings — they render as the searchable nouns themselves.
pub(super) fn keywords(language: SettingsLanguage) -> Vec<SharedString> {
    SettingsNavigationPage::Cultivation.search_keywords(
        language,
        [
            "cultivation.realm",
            "cultivation.attributes",
            "cultivation.equipment",
            "cultivation.power",
        ],
    )
}

/// The whole cultivation body.
pub(super) fn content(
    view: Entity<SettingsView>,
    _options: &RenderOptions,
    _window: &mut Window,
    cx: &mut App,
) -> gpui_kit::AnyElement {
    let language = view.read(cx).display_language();
    let locale = language.catalog_locale();
    let tokens = Tokens::from_theme(cx);
    let snapshot = view.read(cx).pomodoro.snapshot();
    let cultivation = snapshot.cultivation.clone();
    let open_item = view.read(cx).cultivation_open_item;

    div()
        .id("cultivation-settings")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(realm_header(&cultivation, locale, tokens))
        .children(notices(&cultivation, locale, tokens))
        .child(attributes_panel(&view, &cultivation, locale, tokens))
        .child(derived_panel(&cultivation, locale, tokens))
        .child(equipment_panel(
            &view,
            &cultivation,
            locale,
            tokens,
            open_item,
        ))
        .child(inventory_panel(
            &view,
            &cultivation,
            locale,
            tokens,
            open_item,
        ))
        .child(arts_panel(&view, &cultivation, locale, tokens))
        .child(mastered_panel(&cultivation, locale, tokens))
        .child(seed_panel(&view, &cultivation, locale, tokens))
        .child(garden_panel(&view, &cultivation, locale, tokens))
        .child(breakthrough_panel(&view, &cultivation, locale, tokens))
        .child(exploration_panel(&view, &cultivation, locale, tokens))
        .child(record_section(&cultivation, locale, tokens))
        .child(shop_panel(&view, &cultivation, locale, tokens))
        .into_any_element()
}

/// The realm line, the power, and how far through the realm the growth is.
fn realm_header(cultivation: &CultivationView, locale: &str, tokens: Tokens) -> impl IntoElement {
    let realm = format_text(
        locale,
        "cultivation.realm",
        &[
            (
                "stage",
                text(locale, cultivation.realm.name_key()).to_string(),
            ),
            ("layer", cultivation.layer.to_string()),
            ("rank", cultivation.rank.to_string()),
        ],
    );
    let power = format_text(
        locale,
        "cultivation.power",
        &[("power", cultivation.power.to_string())],
    );
    let (growth_text, fraction) = growth_line(cultivation, locale);
    let hint_tokens = tokens;
    // Read out here because the hover body outlives this call and cannot borrow
    // `locale`.
    let power_hint = text(locale, "cultivation.hint.power_hint");
    let realm_hint = text(locale, "cultivation.hint.realm_title");

    div()
        .id("cultivation-realm")
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                // The realm name is a ceiling, not a title: it is the one thing on
                // the page that says where the user cannot go yet, so it says so.
                .child(
                    HoverCard::new("cultivation-hint-realm")
                        .trigger(
                            div()
                                .id("cultivation-realm-name")
                                .font_semibold()
                                .child(realm)
                                .test_support(),
                        )
                        .open_delay(Duration::from_millis(120))
                        .content(move |_, _, _| {
                            hint_body("cultivation-hint-realm-body", realm_hint, hint_tokens)
                        })
                        .into_any_element(),
                )
                // The one number on the page that decides something: the
                // breakthrough compares it against the guardian's. A bare number
                // with no such label reads as a score, so it says what it decides.
                .child(
                    HoverCard::new("cultivation-hint-power")
                        .trigger(
                            div()
                                .id("cultivation-power")
                                .text_color(tokens.muted)
                                .child(power)
                                .test_support(),
                        )
                        .open_delay(Duration::from_millis(120))
                        .content(move |_, _, _| {
                            hint_body("cultivation-hint-power-body", power_hint, hint_tokens)
                        })
                        .into_any_element(),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_3()
                .child(growth_bar(tokens, fraction))
                .child(div().text_xs().text_color(tokens.muted).child(growth_text)),
        )
        // What the next pomodoro is worth, in growth. A number rather than a
        // percentage: the four dimensions that decide it are not shown, so a
        // multiplier would be a rule the user can neither see nor move. This
        // is the growth the next session will actually pay.
        .child(
            div()
                .id("cultivation-next-session-growth")
                .text_xs()
                .text_color(tokens.accent)
                .child(format_text(
                    locale,
                    "cultivation.growth.next",
                    &[(
                        "value",
                        compact_number(u64::from(cultivation.next_session_growth)),
                    )],
                ))
                .test_support(),
        )
}

/// The growth line and the fraction of the current layer it represents.
///
/// The line is the pair the ladder is actually about — what has been grown, and
/// what is still owed before the next layer opens — printed short enough to sit
/// beside the bar. A capped realm reads full: the layers past the cap are bought
/// but not shown until the guardian is beaten, and a bar that kept filling would
/// promise a level the user cannot reach.
pub(super) fn growth_line(cultivation: &CultivationView, locale: &str) -> (String, f32) {
    match cultivation.next {
        Some(next) => {
            let span = next.saturating_sub(cultivation.floor).max(1);
            let into = cultivation
                .growth
                .saturating_sub(cultivation.floor)
                .min(span);
            (
                format_text(
                    locale,
                    "cultivation.progress",
                    &[
                        ("value", compact_number(u64::from(cultivation.growth))),
                        (
                            "remaining",
                            compact_number(u64::from(next.saturating_sub(cultivation.growth))),
                        ),
                    ],
                ),
                into as f32 / span as f32,
            )
        }
        None => (format_text(locale, "cultivation.capped", &[]), 1.0),
    }
}

/// The filled part of the growth bar. A fixed pixel width rather than a
/// percentage: the bar is decorative next to the numbers beside it, and a fixed
/// width needs no layout pass to agree with what was drawn.
fn growth_bar(tokens: Tokens, fraction: f32) -> impl IntoElement {
    let filled = GROWTH_BAR_WIDTH * fraction.clamp(0.0, 1.0);
    div()
        .id("cultivation-growth-bar")
        .w(px(GROWTH_BAR_WIDTH))
        .h(px(6.0))
        .rounded_full()
        .bg(tokens.border)
        .child(
            div()
                .w(px(filled))
                .h(px(6.0))
                .rounded_full()
                .bg(tokens.accent),
        )
}

/// The last penalty, drop and breakthrough, when this session produced one.
///
/// They are sticky rather than a toast: the snapshot carries the most recent
/// event, so a notice left on the page is the record of what happened last,
/// which is what a user coming back to the page wants to see.
fn notices(
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
) -> Vec<gpui_kit::AnyElement> {
    let mut notices = Vec::new();
    if let Some(absence) = cultivation.last_absence {
        notices.push(
            notice(
                "cultivation-notice-absence",
                format_text(
                    locale,
                    "cultivation.notice.absence",
                    &[
                        ("days", absence.days.to_string()),
                        ("charged", absence.charged.to_string()),
                    ],
                ),
                tokens.danger,
            )
            .into_any_element(),
        );
    }
    if let Some(drop) = &cultivation.last_drop {
        let drop = drop.clone();
        let (key, item) = match drop {
            DropView::Equipment(item) => (
                "cultivation.notice.drop_equipment",
                item_label(locale, &item),
            ),
            DropView::Seed { attribute, tier } => (
                "cultivation.notice.drop_seed",
                seed_label(locale, attribute, tier, None),
            ),
        };
        notices.push(
            notice(
                "cultivation-notice-drop",
                format_text(locale, key, &[("item", item)]),
                tokens.accent,
            )
            .into_any_element(),
        );
    }
    if let Some(seed) = &cultivation.last_seed_return {
        notices.push(
            notice(
                "cultivation-notice-seed-return",
                format_text(
                    locale,
                    "cultivation.notice.seed_return",
                    &[(
                        "seed",
                        seed_label(locale, seed.attribute, seed.tier, seed.name.as_deref()),
                    )],
                ),
                tokens.accent,
            )
            .into_any_element(),
        );
    }
    if let Some(training) = &cultivation.last_training {
        let (key, color) = match training.outcome {
            ArtOutcome::Failed => ("cultivation.notice.art_failed", tokens.muted),
            ArtOutcome::Mastered => ("cultivation.notice.art_mastered", tokens.accent),
            _ => ("cultivation.notice.art_ranked", tokens.accent),
        };
        let line = format_text(
            locale,
            key,
            &[
                ("name", training.name.clone()),
                ("rank", training.rank.to_string()),
            ],
        );
        notices.push(notice("cultivation-notice-art", line, color).into_any_element());
    }
    if let Some(outcome) = &cultivation.last_breakthrough {
        let line = if outcome.report.won {
            format_text(
                locale,
                "cultivation.breakthrough.won",
                &[("stage", text(locale, outcome.target.name_key()).to_string())],
            )
        } else {
            format_text(
                locale,
                "cultivation.breakthrough.lost",
                &[
                    ("power", outcome.report.player_power.to_string()),
                    ("guardian", outcome.report.guardian_power.to_string()),
                ],
            )
        };
        let color = if outcome.report.won {
            tokens.accent
        } else {
            tokens.danger
        };
        notices.push(notice("cultivation-notice-breakthrough", line, color).into_any_element());
    }
    // The last walk, and whatever it turned up. A walk that found something
    // names it; a walk that met someone says who won.
    if let Some(outcome) = cultivation.last_exploration {
        let kind = match outcome {
            ExplorationOutcome::Nothing => Some(("cultivation.explore.nothing", tokens.muted)),
            ExplorationOutcome::Victory => Some(("cultivation.explore.victory", tokens.accent)),
            ExplorationOutcome::Defeat => Some(("cultivation.explore.defeat", tokens.danger)),
            // A quiet find is named by the line under it.
            ExplorationOutcome::Seed
            | ExplorationOutcome::Gear
            | ExplorationOutcome::Art
            | ExplorationOutcome::Stones => None,
        };
        if let Some((key, color)) = kind {
            notices.push(
                notice(
                    "cultivation-notice-exploration",
                    text(locale, key).to_string(),
                    color,
                )
                .into_any_element(),
            );
        }
        // Each thing the walk turned up gets its own line, because a walk can
        // turn up more than one: the stones are rolled beside the finding
        // rather than in place of it, so a herb and a handful of money can
        // come back together.
        if let Some(found) = &cultivation.last_exploration_found {
            if let Some(seed) = &found.seed {
                notices.push(
                    notice(
                        "cultivation-notice-exploration-seed",
                        format_text(
                            locale,
                            "cultivation.explore.found_seed",
                            &[(
                                "name",
                                seed_label(locale, seed.attribute, seed.tier, seed.name.as_deref()),
                            )],
                        ),
                        tokens.accent,
                    )
                    .into_any_element(),
                );
            }
            if let Some(item) = &found.gear {
                notices.push(
                    notice(
                        "cultivation-notice-exploration-gear",
                        format_text(
                            locale,
                            "cultivation.explore.found_gear",
                            &[("name", item_label(locale, item))],
                        ),
                        tokens.accent,
                    )
                    .into_any_element(),
                );
            }
            if let Some(name) = &found.art {
                notices.push(
                    notice(
                        "cultivation-notice-exploration-art",
                        format_text(
                            locale,
                            "cultivation.explore.found_art",
                            &[("name", name.clone())],
                        ),
                        tokens.accent,
                    )
                    .into_any_element(),
                );
            }
            if let Some(amount) = found.stones {
                notices.push(
                    notice(
                        "cultivation-notice-exploration-stones",
                        format_text(
                            locale,
                            "cultivation.explore.found_stones",
                            &[("value", amount.to_string())],
                        ),
                        tokens.accent,
                    )
                    .into_any_element(),
                );
            }
        }
    }
    notices
}

fn notice(id: &'static str, line: String, color: Hsla) -> impl IntoElement {
    div().id(id).text_xs().text_color(color).child(line)
}

/// The four attributes, what each source contributes, and the free points the
/// levels have paid.
///
/// A level pays one point the user places by hand, so the panel is also the one
/// place that spends them: the point is offered on the attribute itself rather
/// than in a separate editor, because "where does this point go" is answered by
/// looking at the four numbers it could raise.
fn attributes_panel(
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
    let breakdown = format!(
        "{} · {} · {}",
        format_text(
            locale,
            "cultivation.power_level",
            &[("value", cultivation.level_attributes.power().to_string())],
        ),
        format_text(
            locale,
            "cultivation.power_equipment",
            &[(
                "value",
                cultivation.equipment_attributes.power().to_string()
            )],
        ),
        format_text(
            locale,
            "cultivation.power_crop",
            &[("value", cultivation.crop_attributes.power().to_string())],
        ),
    );
    // The two schools the four attributes feed. Shown beside the totals because
    // a lopsided build is invisible in a single power number: two cultivators at
    // the same power can be one spellcaster and one brawler.
    let schools = format_text(
        locale,
        "cultivation.power_schools",
        &[
            ("spell", cultivation.attributes.spell_power().to_string()),
            (
                "martial",
                cultivation.attributes.martial_power().to_string(),
            ),
        ],
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
    // The two schools are the panel most likely to read as two more attributes,
    // so their line carries the sentence that says they are a cut of the four
    // above rather than two things of their own. The strings are read out here
    // because the hover body outlives this call and cannot borrow `locale`.
    let schools_hint_title = text(locale, "cultivation.hint.schools_title");
    let schools_hint_body = text(locale, "cultivation.hint.schools_hint");
    panel
        .child(
            HoverCard::new("cultivation-hint-schools")
                .trigger(
                    div()
                        .id("cultivation-power-schools")
                        .text_xs()
                        .text_color(tokens.muted)
                        .child(schools)
                        .test_support(),
                )
                .open_delay(Duration::from_millis(120))
                .content(move |_, _, _| {
                    hint_body_with_title(
                        "cultivation-hint-schools-body",
                        schools_hint_title,
                        schools_hint_body,
                        tokens,
                    )
                })
                .into_any_element(),
        )
        .child(div().text_xs().text_color(tokens.muted).child(breakdown))
}

/// The seven numbers the four attributes turn into, and the two schools they
/// fight in.
///
/// Shown as read-only chips rather than rows with a "+" beside them: these are
/// what the four already bought, and a point can only be spent on the four
/// themselves. The hint line says so, because a panel full of numbers with no
/// button on them otherwise reads as unfinished.
fn derived_panel(cultivation: &CultivationView, locale: &str, tokens: Tokens) -> impl IntoElement {
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
            &[("value", cultivation.derived.aptitude.to_string())],
        ),
    ));
    // The arithmetic behind each number, matched to the number by its catalog
    // key: the chip that prints the value also prints the sum that produced
    // it, so the two cannot be read as two different numbers.
    let breakdowns: std::collections::HashMap<&'static str, &DerivedBreakdown> = cultivation
        .derived_breakdowns
        .iter()
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
pub(super) struct CombatStats {
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
    pub(super) fn of_fighter(fighter: &Fighter) -> Self {
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
pub(super) fn derived_rows(stats: &CombatStats, locale: &str) -> Vec<(&'static str, String)> {
    [
        ("cultivation.attack", stats.attack),
        ("cultivation.health", stats.health),
        ("cultivation.inner", stats.inner),
        ("cultivation.spell", stats.spell),
        ("cultivation.resist", stats.resist),
        ("cultivation.speed", stats.speed),
        ("cultivation.hit", stats.hit / 10),
        ("cultivation.evade", stats.evade / 10),
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
    let body = text(locale, "cultivation.hint.derived_hint_each");
    // The arithmetic, spelled out with the very numbers the domain computed.
    let formula = breakdown.map(|breakdown| derived_formula(breakdown, locale));
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
            hover
                .content(move |_, _, _| {
                    derived_hint_body(
                        format!("cultivation-hint-{key}-body"),
                        title,
                        body,
                        formula.clone(),
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
pub(super) fn derived_formula(breakdown: &DerivedBreakdown, locale: &str) -> String {
    let figure = |value: u32| {
        if !breakdown.per_mille {
            return value.to_string();
        }
        let tenths = value / 10;
        let tenth = value % 10;
        if tenth == 0 {
            tenths.to_string()
        } else {
            format!("{tenths}.{tenth}")
        }
    };
    let unit = if breakdown.per_mille { "%" } else { "" };
    let formula = format_text(
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
    );
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

/// A hover body carrying the arithmetic behind a number.
///
/// The formula is its own line rather than part of the paragraph: it is a sum
/// to be checked, and a sum buried in a sentence is a sum nobody checks.
fn derived_hint_body(
    id: impl Into<gpui_kit::ElementId>,
    title: &'static str,
    body: &'static str,
    formula: String,
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
        .child(
            div()
                .text_xs()
                .text_color(tokens.muted)
                .whitespace_normal()
                .child(body),
        )
        .test_support()
}

/// The 功法书 the cultivator is still learning from, and which one the next
/// pomodoro trains.
///
/// Selecting is the whole interaction: the training itself happens when a
/// session finishes, so the row says how far along it is and what the next 阶
/// asks for rather than offering a button that does the learning.
fn arts_panel(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let mut panel = div()
        .id("cultivation-arts")
        .flex()
        .flex_col()
        .gap_1()
        .child(section_title(locale, "cultivation.arts.title", tokens));
    if cultivation.arts.is_empty() {
        return panel.child(
            div()
                .text_xs()
                .text_color(tokens.muted)
                .child(text(locale, "cultivation.arts.empty")),
        );
    }
    panel = panel.children(
        cultivation
            .arts
            .iter()
            .map(|art| art_row(view, art, locale, tokens)),
    );
    panel
}

/// The 功法 the cultivator has learned outright.
///
/// A separate panel because it is a separate thing: a book is still being
/// trained and can be sold, while a learned 功法 is kept for good and pays its
/// mastery for as long as the save lives. Putting them in one list would make
/// the second look like the first.
fn mastered_panel(cultivation: &CultivationView, locale: &str, tokens: Tokens) -> impl IntoElement {
    let mut panel = div()
        .id("cultivation-mastered-arts")
        .flex()
        .flex_col()
        .gap_1()
        .child(section_title(locale, "cultivation.mastered.title", tokens))
        // Marked so a frame can be asked whether the panel is on screen.
        .test_support();
    if cultivation.mastered_arts.is_empty() {
        return panel.child(
            div()
                .text_xs()
                .text_color(tokens.muted)
                .child(text(locale, "cultivation.mastered.empty")),
        );
    }
    panel = panel.children(
        cultivation
            .mastered_arts
            .iter()
            .map(|art| mastered_art_row(art, locale, tokens)),
    );
    panel
}

/// One learned 功法: what it is and what it pays, for good.
fn mastered_art_row(art: &MasteredArtView, locale: &str, tokens: Tokens) -> impl IntoElement {
    let grade = text(locale, art.grade.name_key());
    let kind = text(locale, art.kind.name_key());
    let head = format!("{} · {} · {}", art.name, kind, grade);
    let detail = format_text(
        locale,
        "cultivation.art.mastery_paid",
        &[("value", (art.mastery_permille / 10).to_string())],
    );
    div()
        .id(format!("cultivation-mastered-art-{}", art.id))
        .flex()
        .flex_col()
        .gap_1()
        .py_1()
        .child(div().flex_1().min_w_0().truncate().text_xs().child(head))
        .child(
            div()
                .id(format!("cultivation-mastered-art-detail-{}", art.id))
                .text_xs()
                .text_color(tokens.muted)
                .child(detail)
                .test_support(),
        )
        .test_support()
}

/// One 功法: what it is, how far it has been trained, and the select button.
fn art_row(
    view: &Entity<SettingsView>,
    art: &ArtView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let mastered = art.rank >= art.max_rank;
    let grade = text(locale, art.grade.name_key());
    let kind = text(locale, art.kind.name_key());
    let head = format!("{} · {} · {}", art.name, kind, grade);
    let rank = format_text(
        locale,
        "cultivation.art.rank",
        &[
            ("rank", art.rank.to_string()),
            ("max", art.max_rank.to_string()),
        ],
    );
    let detail = if mastered {
        text(locale, "cultivation.art.mastered").to_string()
    } else {
        format!(
            "{} · {}",
            format_text(
                locale,
                "cultivation.art.progress",
                &[
                    ("have", art.progress.to_string()),
                    ("need", art.required.to_string()),
                ],
            ),
            format_text(
                locale,
                "cultivation.art.failure",
                &[("value", (art.failure_permille / 10).to_string())],
            ),
        )
    };
    let payoff = art_payoff(art, locale);
    let art_id = art.id;
    div()
        .id(format!("cultivation-art-{}", art.id))
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
                .child(div().flex_1().min_w_0().truncate().text_xs().child(head))
                .child(
                    Button::new(format!("cultivation-art-select-{}", art.id))
                        .ghost()
                        .small()
                        .label(text(
                            locale,
                            if art.selected {
                                "cultivation.art.selected"
                            } else {
                                "cultivation.art.select"
                            },
                        ))
                        .disabled(art.selected)
                        .on_click({
                            let view = view.clone();
                            move |_, _, cx| {
                                view.update(cx, |view, cx| {
                                    let _ = view.pomodoro.try_publish(PomodoroCommand::SelectArt {
                                        art_id: Some(art_id),
                                    });
                                    view.poll_pomodoro(cx);
                                });
                            }
                        }),
                )
                .child(
                    // The trained 功法 is not for sale: 修炼 reads it every session,
                    // and the button is disabled rather than hidden so that the
                    // reason it cannot be sold is a thing the user can see.
                    Button::new(format!("cultivation-sell-art-{}", art.id))
                        .ghost()
                        .small()
                        .label(sell_label(locale, art_sell_price(art.id)))
                        .disabled(art.selected)
                        .on_click({
                            let view = view.clone();
                            move |_, _, cx| {
                                view.update(cx, |view, cx| {
                                    let _ = view
                                        .pomodoro
                                        .try_publish(PomodoroCommand::SellArt { art_id });
                                    view.poll_pomodoro(cx);
                                });
                            }
                        }),
                ),
        )
        .child(
            div()
                .id(format!("cultivation-art-detail-{}", art.id))
                .text_xs()
                .text_color(tokens.muted)
                .child(format!("{rank} · {detail}")),
        )
        .child(div().text_xs().text_color(tokens.muted).child(payoff))
}

/// What training this 功法 actually pays: attributes for an 内功, damage and a
/// cost for an 外功 — and, once every 阶 is in, the mastery bonus on top.
fn art_payoff(art: &ArtView, locale: &str) -> String {
    let mut parts = match art.kind {
        ArtKind::Internal => {
            let mut parts = Vec::new();
            for attribute in Attribute::ALL {
                let value = attribute.of(art.gain);
                if value > 0 {
                    parts.push(format_text(
                        locale,
                        "cultivation.art.gain",
                        &[
                            ("attribute", text(locale, attribute.name_key()).to_string()),
                            ("value", value.to_string()),
                        ],
                    ));
                }
            }
            parts.push(text(locale, "cultivation.derived_hint").to_string());
            parts
        }
        ArtKind::External => vec![
            format_text(
                locale,
                "cultivation.art.multiplier",
                &[("value", (art.multiplier_permille / 10).to_string())],
            ),
            format_text(
                locale,
                "cultivation.art.cost",
                &[("value", art.cost.to_string())],
            ),
        ],
    };
    // A completed 功法 pays its mastery bonus on top of what its 阶 already
    // bought, and the row says so: "圆满" beside the same numbers as an
    // unfinished art reads as a label rather than as a reward.
    if art.mastery_permille > 0 {
        let key = match art.kind {
            ArtKind::Internal => "cultivation.art.mastery_attribute",
            ArtKind::External => "cultivation.art.mastery_damage",
        };
        parts.push(format_text(
            locale,
            key,
            &[("value", (art.mastery_permille / 10).to_string())],
        ));
    }
    parts.join(" · ")
}

/// The four slots and whatever is worn in each.
fn equipment_panel(
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

/// The 集市: what the purse holds, what is on today's shelf, and what a sale pays.
///
/// Two halves on one panel because they are one loop: the seed rows and the gear
/// rows carry a sell button, the shelf carries a buy button, and the number at the
/// top is what both move. A shop that lived apart from the inventory would ask
/// the user to go and remember what they were holding.
fn shop_panel(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let mut panel = div()
        .id("cultivation-shop")
        .flex()
        .flex_col()
        .gap_2()
        // Marked so a frame can be asked whether the market is on screen at all.
        .test_support()
        .child(section_title(locale, "cultivation.shop.title", tokens))
        .child(
            div()
                .id("cultivation-shop-stones")
                .text_xs()
                .text_color(tokens.accent)
                .child(format_text(
                    locale,
                    "cultivation.shop.stones",
                    &[("stones", cultivation.stones.to_string())],
                ))
                .test_support(),
        )
        .child(
            div()
                .text_xs()
                .text_color(tokens.muted)
                .child(text(locale, "cultivation.shop.shelf")),
        );

    if cultivation.shop_offers.is_empty() {
        panel = panel.child(
            div()
                .id("cultivation-shop-empty")
                .text_xs()
                .text_color(tokens.muted)
                .child(text(locale, "cultivation.shop.empty"))
                .test_support(),
        );
        return panel;
    }
    panel = panel.child(
        div().flex().flex_col().gap_1().children(
            cultivation
                .shop_offers
                .iter()
                .map(|entry| shop_row(view, entry, cultivation, locale, tokens)),
        ),
    );
    panel
}

/// One line of the shelf: what it is, what it costs, and the button that buys it.
///
/// A bought row keeps its place and says so rather than vanishing: a shelf that
/// reshuffles under the cursor is a shelf where the wrong thing gets bought.
fn shop_row(
    view: &Entity<SettingsView>,
    entry: &ShopOfferView,
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let label = shop_offer_label(&entry.offer, locale);
    let price = format_text(
        locale,
        "cultivation.shop.price",
        &[("stones", entry.price.to_string())],
    );
    // The two endings are written as two whole rows rather than as two values
    // slotted into one: the buy ending owns a `Button` and the bought ending
    // owns a `Div`, and an element tree cannot mix the two in one branch. Two
    // rows also let the bought one drop the purse comparison it no longer needs.
    if entry.bought {
        return shop_row_bought(entry, &label, &price, locale, tokens).into_any_element();
    }
    let afford = cultivation.stones >= entry.price;
    shop_row_buy(view, entry, &label, &price, afford, locale, tokens).into_any_element()
}

/// A shelf line that is still for sale.
fn shop_row_buy(
    view: &Entity<SettingsView>,
    entry: &ShopOfferView,
    label: &str,
    price: &str,
    afford: bool,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let button = Button::new(format!("cultivation-shop-buy-{}", entry.slot))
        .ghost()
        .small()
        // A button the user cannot afford is shown and says why on hover rather
        // than being hidden: "the shelf is empty" and "you cannot pay for that"
        // are different answers, and only one of them is true.
        .label(if afford {
            text(locale, "cultivation.shop.buy").to_owned()
        } else {
            price.to_owned()
        })
        .on_click({
            let view = view.clone();
            let slot = entry.slot;
            move |_, _, cx| {
                view.update(cx, |view, cx| {
                    let _ = view
                        .pomodoro
                        .try_publish(PomodoroCommand::BuyFromShop { slot });
                    view.poll_pomodoro(cx);
                });
            }
        });
    div()
        .id(format!("cultivation-shop-row-{}", entry.slot))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_xs()
                .text_color(tokens.text)
                .child(format!("{label} · {price}")),
        )
        .child(button)
        .test_support()
}

/// A shelf line that has been bought: it keeps its place and says so, because a
/// shelf that reshuffles under the cursor is a shelf where the wrong thing gets
/// bought.
fn shop_row_bought(
    entry: &ShopOfferView,
    label: &str,
    price: &str,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    div()
        .id(format!("cultivation-shop-row-{}", entry.slot))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_xs()
                .text_color(tokens.muted)
                .child(format!("{label} · {price}")),
        )
        .child(
            div()
                .id(format!("cultivation-shop-bought-{}", entry.slot))
                .text_xs()
                .text_color(tokens.accent)
                .child(text(locale, "cultivation.shop.bought"))
                .test_support(),
        )
        .test_support()
}

/// What the 集市 pays for a 功法 of that id.
///
/// Grade-priced like every other 功法 rather than read off the row, so the number
/// on the button is the number the sale pays even if the catalog is retuned.
fn art_sell_price(art_id: u64) -> u32 {
    ShopOffer::Art { art_id }.sell_price()
}

/// A sell button's label: the price it pays, spelled out.
///
/// The number is on the button rather than in a tooltip because a sell is a
/// decision the user is about to make, and a price hidden behind a hover is a
/// price they cannot compare across rows.
fn sell_label(locale: &str, stones: u32) -> String {
    format_text(
        locale,
        "cultivation.shop.sell",
        &[("stones", stones.to_string())],
    )
}

/// What one offer is called on the shelf.
///
/// The same names the inventory uses, so a thing bought from the shelf and the
/// same thing found on a walk read as the same object on both sides of the page.
fn shop_offer_label(offer: &ShopOffer, locale: &str) -> String {
    match offer {
        ShopOffer::Equipment(item) => item_label(
            locale,
            &ItemView {
                id: item.id,
                slot: item.slot,
                tier: item.tier,
                rarity: item.rarity,
                attributes: item.attributes(),
                power: item.power(),
                equipped: false,
                name: item.name.clone(),
                sell_price: offer.sell_price(),
            },
        ),
        ShopOffer::Seed {
            attribute,
            tier,
            name,
        } => seed_label(locale, *attribute, *tier, name.as_deref()),
        ShopOffer::Art { art_id } => blueprint(*art_id).name.clone(),
    }
}

/// A hover body carrying one paragraph.
fn hint_body(
    id: impl Into<gpui_kit::ElementId>,
    body: &'static str,
    tokens: Tokens,
) -> impl IntoElement {
    div()
        .id(id.into())
        .flex()
        .flex_col()
        .gap_1()
        .w(px(240.0))
        .text_xs()
        .text_color(tokens.muted)
        .whitespace_normal()
        .child(body)
        .test_support()
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

/// An item's name as a click-to-expand control, or plain text when there is no
/// item in the slot. Clicking toggles the attribute breakdown under the row.
fn item_label_element(
    view: &Entity<SettingsView>,
    item: Option<&ItemView>,
    label: String,
    tokens: Tokens,
) -> gpui_kit::AnyElement {
    match item {
        Some(item) => {
            let id = item.id;
            div()
                .id(format!("cultivation-item-toggle-{id}"))
                .flex_1()
                .min_w_0()
                .truncate()
                .text_xs()
                .text_color(tokens.text)
                .child(label)
                .on_click({
                    let view = view.clone();
                    move |_, _, cx| {
                        view.update(cx, |view, cx| {
                            view.cultivation_open_item = if view.cultivation_open_item == Some(id) {
                                None
                            } else {
                                Some(id)
                            };
                            cx.notify();
                        });
                    }
                })
                .into_any_element()
        }
        None => div()
            .flex_1()
            .min_w_0()
            .truncate()
            .text_xs()
            .text_color(tokens.muted)
            .child(label)
            .into_any_element(),
    }
}

/// One item's attribute breakdown, shown under its row when expanded.
///
/// Two lines: the four raw numbers, then what they are worth in each school. The
/// second line is the one that answers "is this a spellcaster's piece or a
/// brawler's", which the tier and the rarity alone do not say.
fn item_detail(item: &ItemView, locale: &str, tokens: Tokens) -> impl IntoElement {
    let attributes = item.attributes;
    div()
        .id(format!("cultivation-item-detail-{}", item.id))
        .flex()
        .flex_col()
        .gap_1()
        .text_xs()
        .text_color(tokens.muted)
        .child(format_text(
            locale,
            "cultivation.item.details",
            &[
                ("root", attributes.root.to_string()),
                ("strength", attributes.strength.to_string()),
                ("mana", attributes.mana.to_string()),
                ("agility", attributes.agility.to_string()),
            ],
        ))
        .child(format_text(
            locale,
            "cultivation.item.schools",
            &[
                ("spell", attributes.spell_power().to_string()),
                ("martial", attributes.martial_power().to_string()),
                ("power", item.power.to_string()),
            ],
        ))
}

/// The satchel: everything owned and not worn.
fn inventory_panel(
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

/// The seeds waiting to be planted.
fn seed_panel(
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
fn garden_panel(
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

/// The guardian at the end of the realm, and the one button that fights it.
///
/// The button answers to the cooldown rather than to the power line: the fight
/// is what decides a breakthrough, and a cultivator who is behind on paper has
/// won one before. The power comparison stays on the page because it is what
/// the report reads back, and because knowing where you stand is half of
/// deciding to spend the attempt.
fn breakthrough_panel(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let panel = div()
        .id("cultivation-breakthrough")
        .flex()
        .flex_col()
        .gap_1()
        .child(section_title(
            locale,
            "cultivation.breakthrough.title",
            tokens,
        ));
    let Some(guardian) = cultivation.guardian else {
        let line = if cultivation.realm == Realm::Immortal && cultivation.level == MAX_LEVEL {
            text(locale, "cultivation.breakthrough.max")
        } else {
            text(locale, "cultivation.capped")
        };
        return panel.child(div().text_xs().text_color(tokens.muted).child(line));
    };
    let enough = cultivation.power >= guardian.power;
    let hint_key = if enough {
        "cultivation.breakthrough.ready"
    } else {
        "cultivation.breakthrough.weak"
    };
    let mut button = Button::new("cultivation-breakthrough-attempt")
        .small()
        .label(text(locale, "cultivation.breakthrough.attempt"));
    if cultivation.breakthrough_ready {
        button = button.on_click({
            let view = view.clone();
            move |_, _, cx| {
                view.update(cx, |view, cx| {
                    view.ask_cultivation(
                        CultivationDuelSource::Breakthrough,
                        PomodoroCommand::Breakthrough,
                        cx,
                    );
                });
            }
        });
    }
    let mut panel = panel
        .child(div().text_xs().text_color(tokens.muted).child(format_text(
            locale,
            "cultivation.breakthrough.guardian",
            &[
                (
                    "stage",
                    text(locale, guardian.target.name_key()).to_string(),
                ),
                ("power", guardian.power.to_string()),
            ],
        )))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .text_color(tokens.muted)
                        .child(text(locale, hint_key)),
                )
                .child(button),
        );
    // A loss holds the attempt for twelve hours. The countdown is the honest
    // thing to show: a disabled button with no reason beside it reads as broken.
    if !cultivation.breakthrough_ready {
        panel = panel.child(
            div()
                .id("cultivation-breakthrough-held")
                .text_xs()
                .text_color(tokens.danger)
                .child(cooldown_line(
                    cultivation.breakthrough_ready_at,
                    "cultivation.breakthrough.held",
                    locale,
                ))
                .test_support(),
        );
    }
    panel
}

/// The walk out of the door: one every twelve hours, whatever it comes to.
///
/// Gated by the cooldown alone — the thing being rationed is the player's
/// attention, not a cost — and the countdown says when the next one is owed.
/// The button is offered whether or not one is owed: a walk that is not owed
/// says so, and a button that quietly disappeared would leave the user
/// wondering whether the page was broken or the walk was.
fn exploration_panel(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let button = Button::new("cultivation-explore-attempt")
        .small()
        .label(text(locale, "cultivation.explore.attempt"))
        .on_click({
            let view = view.clone();
            move |_, _, cx| {
                view.update(cx, |view, cx| {
                    view.ask_cultivation(
                        CultivationDuelSource::Exploration,
                        PomodoroCommand::Explore,
                        cx,
                    );
                });
            }
        });
    let status = if cultivation.exploration_ready {
        text(locale, "cultivation.explore.ready").to_string()
    } else {
        cooldown_line(
            cultivation.exploration_ready_at,
            "cultivation.explore.held",
            locale,
        )
    };
    div()
        .id("cultivation-exploration")
        .flex()
        .flex_col()
        .gap_1()
        .child(section_title(locale, "cultivation.explore.title", tokens))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .id("cultivation-exploration-status")
                        .text_xs()
                        .text_color(tokens.muted)
                        .child(status)
                        .test_support(),
                )
                .child(button),
        )
}

/// The hours and minutes left on a cooldown, as one line.
///
/// A cooldown is read from the instant it ends, which the view already carries;
/// the page owns the clock, so the subtraction happens here rather than in the
/// domain. An ended cooldown reads as zero rather than as a negative, and a
/// cooldown that was never started falls back to the bare key.
fn cooldown_line(owed_at: Option<i64>, key: &'static str, locale: &str) -> String {
    let Some(owed_at) = owed_at else {
        return text(locale, key).to_string();
    };
    let left = owed_at.saturating_sub(now_unix()).max(0);
    let hours = left / 3_600;
    let minutes = (left % 3_600) / 60;
    format_text(
        locale,
        key,
        &[
            ("hours", hours.to_string()),
            ("minutes", minutes.to_string()),
        ],
    )
}

/// The fights fought and what the last walk settled.
///
/// Both are lines of history rather than state: the popups that show them are
/// forgotten at quit, so this is the only place a save can be asked what it has
/// been through. The record is eight lines — long enough that a run of losses
/// still shows the wins, short enough to be read at a glance.
fn record_section(cultivation: &CultivationView, locale: &str, tokens: Tokens) -> impl IntoElement {
    let mut column = div()
        .id("cultivation-record")
        .flex()
        .flex_col()
        .gap_1()
        .child(section_title(locale, "cultivation.record.title", tokens))
        // Marked so a frame can be asked whether the record is on screen.
        .test_support();
    for (index, line) in cultivation.duel_record.iter().enumerate() {
        column = column.child(
            div()
                .id(format!("cultivation-record-duel-{index}"))
                .flex()
                .flex_row()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .text_color(tokens.text)
                        .child(text(locale, line.kind.key())),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(if line.won {
                            tokens.accent
                        } else {
                            tokens.danger
                        })
                        .child(text(
                            locale,
                            if line.won {
                                "cultivation.record.win"
                            } else {
                                "cultivation.record.loss"
                            },
                        )),
                )
                .test_support(),
        );
    }
    if let Some(walk) = &cultivation.last_walk {
        column = column.child(
            div()
                .id("cultivation-record-walk")
                .text_xs()
                .text_color(tokens.muted)
                .child(walk_line(walk, locale))
                .test_support(),
        );
    }
    column
}

/// One walk's settlement, as a single line: how it ended and what it turned up.
///
/// A walk can pay three things at once — and the 灵石 beside them, which is
/// rolled beside the finding rather than in place of it — so the line names each
/// in turn rather than saying "something was found". A record that does not say
/// what was found is not worth keeping.
fn walk_line(walk: &WalkRecord, locale: &str) -> String {
    let mut line = format_text(
        locale,
        "cultivation.record.walk",
        &[(
            "outcome",
            text(locale, walk.outcome.outcome_key()).to_string(),
        )],
    );
    for found in [
        walk.art
            .as_deref()
            .map(|name| ("cultivation.record.found_art", name)),
        walk.gear
            .as_deref()
            .map(|name| ("cultivation.record.found_gear", name)),
        walk.seed
            .as_deref()
            .map(|name| ("cultivation.record.found_seed", name)),
    ]
    .into_iter()
    .flatten()
    {
        line.push_str(&format!(
            " · {}",
            format_text(locale, found.0, &[("name", found.1.to_owned())],)
        ));
    }
    if let Some(amount) = walk.stones {
        line.push_str(&format!(
            " · {}",
            format_text(
                locale,
                "cultivation.record.found_stones",
                &[("value", amount.to_string())]
            )
        ));
    }
    line
}

fn section_title(locale: &str, key: &'static str, tokens: Tokens) -> impl IntoElement {
    div()
        .text_xs()
        .font_semibold()
        .text_color(tokens.text)
        .child(text(locale, key))
}

/// An item's display line: its xianxia name when it has one, else the slot, and
/// in both cases the tier and the grade as stars. The name is what tells two
/// same-slot, same-tier drops apart, so it leads the line when present.
pub(super) fn item_label(locale: &str, item: &ItemView) -> String {
    let stars = "★".repeat(item.rarity.stars());
    let slot = text(locale, item.slot.name_key()).to_string();
    match item.name.as_deref() {
        Some(name) => format_text(
            locale,
            "cultivation.item.label_named",
            &[
                ("name", name.to_owned()),
                ("slot", slot),
                ("tier", item.tier.to_string()),
                ("stars", stars),
            ],
        ),
        None => format_text(
            locale,
            "cultivation.item.label",
            &[
                ("slot", slot),
                ("tier", item.tier.to_string()),
                ("stars", stars),
            ],
        ),
    }
}

/// A seed's or a crop's display line: its xianxia name when the stack carries
/// one, else the attribute it raises, and in both cases its tier.
pub(super) fn seed_label(
    locale: &str,
    attribute: Attribute,
    tier: u8,
    name: Option<&str>,
) -> String {
    match name {
        Some(name) => format_text(
            locale,
            "cultivation.seed.label_named",
            &[("name", name.to_owned()), ("tier", tier.to_string())],
        ),
        None => format_text(
            locale,
            "cultivation.seed.label",
            &[
                ("attribute", text(locale, attribute.name_key()).to_string()),
                ("tier", tier.to_string()),
            ],
        ),
    }
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
