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
    ArtKind, ArtOutcome, ArtView, Attribute, Attributes, CultivationView, DailyKind, Derived,
    DropView, EquipSlot, EventDetail, EventRecord, ExplorationOutcome, GearEffect, ItemView,
    MAX_LEVEL, MAX_STAR, MasteredArtView, PlotView, PomodoroCommand, Rarity, Realm, SaleKind,
    ShopOffer, ShopOfferView, TRIAL_ATTEMPTS_PER_DAY, TrialBlocked, art_bonus_at, art_payout_at,
    blueprint, grow_seconds, harvest_points, mastered_art_bonus_payout, mastered_art_payout,
    star_multiplier_permille,
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
mod trial;

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
// The history popup is mounted by the window frame beside the fight, so it
// has to be visible past this module.
pub(super) use records::events_overlay;
use records::record_section;
use shop::shop_panel;
use trial::trial_panel;

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
    let open_seed = view.read(cx).cultivation_open_seed;
    let open_plot = view.read(cx).cultivation_open_plot;
    let mastered_open = view.read(cx).cultivation_mastered_open;

    div()
        .id("cultivation-settings")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(realm_header(&cultivation, locale, tokens))
        .children(notices(&cultivation, locale, tokens))
        .child(record_section(&view, &cultivation, locale, tokens))
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
        .child(mastered_panel(
            &view,
            &cultivation,
            mastered_open,
            locale,
            tokens,
        ))
        .child(seed_panel(&view, &cultivation, open_seed, locale, tokens))
        .child(garden_panel(&view, &cultivation, open_plot, locale, tokens))
        .child(breakthrough_panel(&view, &cultivation, locale, tokens))
        .child(exploration_panel(&view, &cultivation, locale, tokens))
        .child(trial_panel(&view, &cultivation, locale, tokens))
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
    let realm_hint = text(locale, "cultivation.hint.realm_title").to_string();
    // The whole ladder, read out here for the same reason: the card lists where
    // the cultivator stands among the ten, and the one they are in is marked.
    let realms: Vec<(String, bool)> = Realm::ALL
        .iter()
        .map(|row| {
            (
                text(locale, row.name_key()).to_string(),
                *row == cultivation.realm,
            )
        })
        .collect();

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
                            realm_ladder_body(
                                "cultivation-hint-realm-body",
                                realm_hint.clone(),
                                realms.clone(),
                                hint_tokens,
                            )
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

/// The last penalty and the last thing a fight dropped, when this session
/// produced one.
///
/// These three are the notices that survived: a charge, a piece of gear lost and
/// a seed returned are facts the event log below does not carry, so a sticky
/// line is the only place the user can read them. What the log *does* carry —
/// a 功法 turning over, a guardian falling — is no longer repeated here: the
/// same sentence twice, one above the other, read as two events rather than one,
/// and the log is the one that keeps them past a restart.
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
    // What the last star attempt came to. One line, and it names the piece: a
    // wager whose result the user has to infer from the stars on the row is a
    // wager they cannot check.
    if let Some(star) = &cultivation.last_star {
        let key = if star.success {
            "cultivation.notice.star_success"
        } else {
            "cultivation.notice.star_failed"
        };
        let name = star
            .name
            .clone()
            .unwrap_or_else(|| text(locale, "cultivation.slot.weapon").to_string());
        notices.push(
            notice(
                "cultivation-notice-star",
                format_text(
                    locale,
                    key,
                    &[("name", name), ("star", star.to.to_string())],
                ),
                if star.success {
                    tokens.accent
                } else {
                    tokens.danger
                },
            )
            .into_any_element(),
        );
    }
    // What a walk turned up, and what a trial floor paid, are not repeated
    // here: the event log below the notices records every walk and every floor
    // in order, and a find printed twice — once as a sticky line and once as
    // history — read as two finds.
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

/// The 升星 button: what the wager costs, spelled out, and nothing at all once
/// the piece holds every star it can.
///
/// The price is the domain's own — half of what the piece sells for — so the
/// number on the button is the number the purse pays. A capped piece offers no
/// button rather than a disabled one: a button that cannot be pressed is a
/// button the user has to wonder about.
fn star_button(
    view: &Entity<SettingsView>,
    item: &ItemView,
    locale: &str,
) -> Option<gpui_kit::AnyElement> {
    let cost = item.star_cost?;
    let item_id = item.id;
    Some(
        Button::new(format!("cultivation-star-{item_id}"))
            .ghost()
            .small()
            .label(format_text(
                locale,
                "cultivation.item.star_button",
                &[("stones", cost.to_string())],
            ))
            .on_click({
                let view = view.clone();
                move |_, _, cx| {
                    view.update(cx, |view, cx| {
                        let _ = view
                            .pomodoro
                            .try_publish(PomodoroCommand::StarEquipment { item_id });
                        view.poll_pomodoro(cx);
                    });
                }
            })
            .into_any_element(),
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

/// The realm card's body: what the realm name means, then the ten rungs it is
/// one of, with the one the cultivator stands in marked.
///
/// "元婴 · 3 层" alone does not say whether that is near the top of the ladder
/// or near the bottom. The card lists all ten in climbing order, and the current
/// one carries the danger colour and a caret — the colour is the page's word for
/// "this is the one that matters", which on every other row means a penalty and
/// here means the rung the save is standing on.
fn realm_ladder_body(
    id: impl Into<gpui_kit::ElementId>,
    title: String,
    realms: Vec<(String, bool)>,
    tokens: Tokens,
) -> impl IntoElement {
    div()
        .id(id.into())
        .flex()
        .flex_col()
        .gap_1()
        .w(px(180.0))
        .text_xs()
        .text_color(tokens.muted)
        .whitespace_normal()
        .child(title)
        .children(
            realms
                .into_iter()
                .enumerate()
                .map(move |(index, (name, current))| {
                    let row = div()
                        .id(format!("cultivation-realm-rung-{index}"))
                        .child(if current { format!("▸ {name}") } else { name })
                        .test_support();
                    if current {
                        row.text_color(tokens.danger).font_semibold()
                    } else {
                        row.text_color(tokens.muted)
                    }
                }),
        )
        .test_support()
}

/// An item's name as a click-to-expand control, or plain text when there is no
/// item in the slot. Clicking toggles the attribute breakdown under the row.
///
/// A worn or owned piece carries the quality's colour chip and is painted in
/// the quality's own colour: eight grades are only readable if the row says
/// which one it is before the detail is opened.
fn item_label_element(
    view: &Entity<SettingsView>,
    item: Option<&ItemView>,
    label: String,
    tokens: Tokens,
) -> gpui_kit::AnyElement {
    match item {
        Some(item) => {
            let id = item.id;
            let color = quality_color(item.rarity);
            div()
                .id(format!("cultivation-item-toggle-{id}"))
                .flex_1()
                .min_w_0()
                .flex()
                .flex_row()
                .items_center()
                .gap_1()
                .truncate()
                .child(
                    div()
                        .w(px(8.0))
                        .h(px(8.0))
                        .flex_shrink_0()
                        .rounded_sm()
                        .bg(color),
                )
                .child(div().text_xs().text_color(color).truncate().child(label))
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

/// One item's breakdown, shown under its row when expanded.
///
/// The lines each answer a different question the row raises: the four raw
/// numbers, what they are worth in each school, the combat numbers the piece
/// adds outright, what its stars have multiplied, and — for a 神装 — the effect
/// it carries. The schools line is the one that answers "is this a
/// spellcaster's piece or a brawler's", which the tier and the quality alone do
/// not say.
fn item_detail(item: &ItemView, locale: &str, tokens: Tokens) -> impl IntoElement {
    let attributes = item.attributes;
    let mut panel = div()
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
        ));
    // The outright combat numbers, when the piece pays any: a robe that soaks
    // says so here rather than asking the user to work it out from 根骨.
    if let Some(lines) = bonus_segment(locale, &item.bonus) {
        panel = panel.child(lines);
    }
    // The stars, when the piece holds any: what they are worth together is the
    // number the fight reads, so the row says it rather than implying it.
    if item.star > 0 {
        panel = panel.child(format_text(
            locale,
            "cultivation.item.star_line",
            &[
                ("stars", "★".repeat(item.star as usize)),
                (
                    "percent",
                    star_percent_label(star_multiplier_permille(item.id, item.star)),
                ),
            ],
        ));
    }
    // The effect, when the piece carries one: what it does to the other side of
    // the fight, named and numbered. The number is the one the duel reads, so
    // the line and the fight cannot disagree about how deep it bites.
    if let Some(effect) = item.effect {
        let depth = effect.depth_permille(item.rarity) / 10;
        panel = panel.child(format_text(
            locale,
            "cultivation.item.effect_line",
            &[
                ("effect", text(locale, effect.key()).to_string()),
                (
                    "detail",
                    format_text(
                        locale,
                        effect_desc_key(effect),
                        &[("value", depth.to_string())],
                    ),
                ),
            ],
        ));
    }
    panel
}

fn section_title(locale: &str, key: &'static str, tokens: Tokens) -> impl IntoElement {
    div()
        .text_xs()
        .font_semibold()
        .text_color(tokens.text)
        .child(text(locale, key))
}

/// An item's display line: its xianxia name when it has one, else the slot, and
/// in both cases the tier, the quality and the stars. The name is what tells two
/// same-slot, same-tier drops apart, so it leads the line when present.
///
/// The quality is the eight-grade colour the row is painted in — white through
/// crimson — and the stars are the piece's own, bought one wager at a time. A
/// 神装's effect rides inside the name itself, so the log, the notice and the
/// shop row all say what the piece does without a second line.
pub(super) fn item_label(locale: &str, item: &ItemView) -> String {
    let quality = quality_segment(locale, item.rarity, item.star);
    let slot = text(locale, item.slot.name_key()).to_string();
    match item.name.as_deref() {
        Some(name) => format_text(
            locale,
            "cultivation.item.label_named",
            &[
                ("name", name.to_owned()),
                ("slot", slot),
                ("tier", item.tier.to_string()),
                ("quality", quality),
            ],
        ),
        None => format_text(
            locale,
            "cultivation.item.label",
            &[
                ("slot", slot),
                ("tier", item.tier.to_string()),
                ("quality", quality),
            ],
        ),
    }
}

/// The quality half of an item's line: its name, and its stars when it has any.
///
/// Stars are symbols rather than catalog text — they are the one thing on the
/// page that is the same in every language — so they are spelled here and the
/// quality's name is the only translated word in the segment.
pub(super) fn quality_segment(locale: &str, rarity: Rarity, star: u8) -> String {
    let stars = "★".repeat(star.min(MAX_STAR) as usize);
    if stars.is_empty() {
        text(locale, rarity.name_key()).to_string()
    } else {
        format_text(
            locale,
            "cultivation.item.quality",
            &[
                ("quality", text(locale, rarity.name_key()).to_string()),
                ("stars", stars),
            ],
        )
    }
}

/// The quality's colour, as the domain names it.
///
/// One conversion, here, so the eight colours live in the domain beside the
/// eight names and a retune of either moves the row and the chip together.
pub(super) fn quality_color(rarity: Rarity) -> Hsla {
    let value = rarity.color();
    let red = ((value >> 16) & 0xFF) as f32 / 255.0;
    let green = ((value >> 8) & 0xFF) as f32 / 255.0;
    let blue = (value & 0xFF) as f32 / 255.0;
    hsla_from_rgb(red, green, blue)
}

/// The standard RGB-to-HSL turn, so a colour stored as `0xRRGGBB` can be painted
/// without a second table of the same eight colours written out by hand.
fn hsla_from_rgb(red: f32, green: f32, blue: f32) -> Hsla {
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let lightness = (max + min) / 2.0;
    if (max - min).abs() < f32::EPSILON {
        return gpui_kit::hsla(0.0, 0.0, lightness, 1.0);
    }
    let span = max - min;
    let saturation = if lightness <= 0.5 {
        span / (max + min)
    } else {
        span / (2.0 - max - min)
    };
    let hue = if max == red {
        ((green - blue) / span + if green < blue { 6.0 } else { 0.0 }) / 6.0
    } else if max == green {
        ((blue - red) / span + 2.0) / 6.0
    } else {
        ((red - green) / span + 4.0) / 6.0
    };
    gpui_kit::hsla(hue, saturation, lightness, 1.0)
}

/// The outright combat numbers a set of them pays, as the page says them.
///
/// Each line is the catalog's own name for the number — 攻击力, 生命值, 命中 —
/// so a bonus and an attribute read the same way wherever they appear.
pub(super) fn bonus_lines(locale: &str, bonus: &Derived) -> Vec<String> {
    [
        ("cultivation.attack", bonus.attack),
        ("cultivation.health", bonus.health),
        ("cultivation.inner", bonus.inner),
        ("cultivation.spell", bonus.spell),
        ("cultivation.resist", bonus.resist),
        ("cultivation.speed", bonus.speed),
        ("cultivation.hit", bonus.hit),
        ("cultivation.evade", bonus.evade),
        ("cultivation.aptitude", bonus.aptitude),
    ]
    .into_iter()
    .filter(|(_, value)| *value > 0)
    .map(|(key, value)| format_text(locale, key, &[("value", value.to_string())]))
    .collect()
}

/// The outright combat numbers as one line, or `None` when there are none.
///
/// Shared by gear and 功法 rows: a piece that pays 攻击力 outright and an 内功
/// that pays it outright are the same fact, said the same way.
pub(super) fn bonus_segment(locale: &str, bonus: &Derived) -> Option<String> {
    let lines = bonus_lines(locale, bonus);
    (!lines.is_empty())
        .then(|| format_text(locale, "cultivation.bonus", &[("lines", lines.join(" · "))]))
}

/// The catalog key naming what an effect does to the other side of a fight.
pub(super) fn effect_desc_key(effect: GearEffect) -> &'static str {
    match effect {
        GearEffect::Frost => "cultivation.effect.frost_desc",
        GearEffect::Thunder => "cultivation.effect.thunder_desc",
        GearEffect::Sunder => "cultivation.effect.sunder_desc",
        GearEffect::Weaken => "cultivation.effect.weaken_desc",
    }
}

/// A star multiplier as the percent it is worth, one decimal at most: 106.7
/// reads `106.7`, 110 reads `110`.
fn star_percent_label(permille: u32) -> String {
    let whole = permille / 10;
    let tenth = permille % 10;
    if tenth == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{tenth}")
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
