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
    ArtKind, ArtOutcome, ArtView, Attribute, Attributes, CultivationView, DropView, EquipSlot,
    EventDetail, ExplorationOutcome, ItemView, MAX_LEVEL, MasteredArtView, PomodoroCommand, Realm,
    SaleKind, ShopOffer, ShopOfferView, blueprint, mastered_art_payout,
};

use gpui_kit::component::Sizable as _;
use gpui_kit::component::hover_card::HoverCard;
// `test_support()` marks the elements the headless window tests observe; without
// `gpui-kit`'s `test-support` feature it is the identity function.
use gpui_kit::base::TestSupportExt as _;
use std::time::Duration;

mod arts;
mod attributes;
mod explore;
mod garden;
mod gear;
mod records;
mod shop;

pub(super) use attributes::{CombatStats, derived_rows};
// Only the headless page tests read the formula directly; the lib itself
// reaches it through the page render.
use arts::{arts_panel, mastered_panel};
#[cfg(test)]
pub(super) use attributes::derived_formula;
use attributes::{attributes_panel, derived_panel};
use explore::{breakthrough_panel, exploration_panel};
use garden::{garden_panel, seed_panel};
use gear::{equipment_panel, inventory_panel};
use records::record_section;
use shop::shop_panel;

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
    let open_offer = view.read(cx).cultivation_open_offer;

    div()
        .id("cultivation-settings")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(realm_header(&cultivation, locale, tokens))
        .children(notices(&cultivation, locale, tokens))
        .child(record_section(&cultivation, locale, tokens))
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
        .child(mastered_panel(&view, &cultivation, locale, tokens))
        .child(seed_panel(&view, &cultivation, locale, tokens))
        .child(garden_panel(&view, &cultivation, locale, tokens))
        .child(breakthrough_panel(&view, &cultivation, locale, tokens))
        .child(exploration_panel(&view, &cultivation, locale, tokens))
        .child(shop_panel(&view, &cultivation, open_offer, locale, tokens))
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
