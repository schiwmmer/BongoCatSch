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
use bongocat_i18n::{format_text, text};
use bongocat_pomodoro::{
    ArtKind, ArtOutcome, ArtView, Attribute, CultivationView, DropView, EquipSlot,
    ExplorationFindView, ExplorationOutcome, ItemView, MAX_LEVEL, PomodoroCommand, Realm,
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
        .child(seed_panel(&view, &cultivation, locale, tokens))
        .child(garden_panel(&view, &cultivation, locale, tokens))
        .child(breakthrough_panel(&view, &cultivation, locale, tokens))
        .child(exploration_panel(&view, &cultivation, locale, tokens))
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
                .child(div().font_semibold().child(realm))
                .child(div().text_color(tokens.muted).child(power)),
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
}

/// The growth line and the fraction of the current layer it represents.
///
/// A capped realm reads full: the layers past the cap are bought but not shown
/// until the guardian is beaten, and a bar that kept filling would promise a
/// level the user cannot reach.
fn growth_line(cultivation: &CultivationView, locale: &str) -> (String, f32) {
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
                        ("value", cultivation.growth.to_string()),
                        ("next", next.to_string()),
                    ],
                ),
                into as f32 / span as f32,
            )
        }
        None => (
            format_text(
                locale,
                "cultivation.capped",
                &[("value", cultivation.growth.to_string())],
            ),
            1.0,
        ),
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
            ExplorationOutcome::Seed | ExplorationOutcome::Gear | ExplorationOutcome::Art => None,
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
        let found = cultivation.last_exploration_found.as_ref().map(|found| {
            let (id, line) = match found {
                ExplorationFindView::Seed(seed) => (
                    "cultivation-notice-exploration-seed",
                    format_text(
                        locale,
                        "cultivation.explore.found_seed",
                        &[(
                            "name",
                            seed_label(locale, seed.attribute, seed.tier, seed.name.as_deref()),
                        )],
                    ),
                ),
                ExplorationFindView::Gear(item) => (
                    "cultivation-notice-exploration-gear",
                    format_text(
                        locale,
                        "cultivation.explore.found_gear",
                        &[("name", item_label(locale, item))],
                    ),
                ),
                ExplorationFindView::Art(name) => (
                    "cultivation-notice-exploration-art",
                    format_text(
                        locale,
                        "cultivation.explore.found_art",
                        &[("name", name.clone())],
                    ),
                ),
            };
            notice(id, line, tokens.accent).into_any_element()
        });
        notices.extend(found);
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
    panel
        .child(
            div()
                .id("cultivation-power-schools")
                .text_xs()
                .text_color(tokens.muted)
                .child(schools)
                .test_support(),
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
    let derived = cultivation.derived;
    // The keys are spelled out rather than built from a name: the catalog test
    // reads the source for the keys it checks, and a key only a `format!` could
    // produce would look unused to it.
    let chips = [
        ("cultivation.attack", derived.attack),
        ("cultivation.health", derived.health),
        ("cultivation.inner", derived.inner),
        ("cultivation.spell", derived.spell),
        ("cultivation.resist", derived.resist),
        ("cultivation.speed", derived.speed),
        ("cultivation.hit", derived.hit_percent()),
        ("cultivation.evade", derived.evade_percent()),
        ("cultivation.aptitude", derived.aptitude),
    ]
    .into_iter()
    .map(|(key, value)| derived_chip(locale, key, value, tokens));
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

/// One derived number: its name and its value, with the explanation the four
/// attributes were read through.
fn derived_chip(locale: &str, key: &str, value: u32, tokens: Tokens) -> impl IntoElement {
    div()
        .id(format!("cultivation-derived-{key}"))
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .px_2()
        .py_1()
        .rounded_md()
        .bg(tokens.overlay)
        .child(div().text_xs().text_color(tokens.muted).child(format_text(
            locale,
            &format!("cultivation.{key}"),
            &[("value", value.to_string())],
        )))
}

/// The 功法 the cultivator owns, and which one the next pomodoro trains.
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
/// cost for an 外功.
fn art_payoff(art: &ArtView, locale: &str) -> String {
    match art.kind {
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
            parts.join(" · ")
        }
        ArtKind::External => format!(
            "{} · {}",
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
        ),
    }
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
fn exploration_panel(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
    let mut button = Button::new("cultivation-explore-attempt")
        .small()
        .label(text(locale, "cultivation.explore.attempt"));
    if cultivation.exploration_ready {
        button = button.on_click({
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
    }
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
