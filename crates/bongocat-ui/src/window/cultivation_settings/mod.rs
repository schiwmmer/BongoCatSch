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
use bongocat_i18n::{compact_number, format_text, group_thousands, text};
use bongocat_pomodoro::{
    AptitudeHerb, ArtGrade, ArtKind, ArtOutcome, ArtOwnership, ArtView, Attribute, Attributes,
    BonusFind, Building, BuildingView, Cost, CultivationView, DailyKind, Derived, DropView,
    EquipSlot, EventDetail, EventRecord, ExplorationHours, ExplorationOutcome, GatherSite,
    GatherSkill, GearEffect, ItemView, LAYERS_PER_STAGE, MAX_STAR, MarrowSource, PlotView,
    PomodoroCommand, Rarity, Realm, Region, Resource, ResourceBag, ResourcesView, SaleKind,
    SeedGrade, SeedView, SessionMode, ShopOffer, ShopOfferView, StrikeEffect,
    TRIAL_ATTEMPTS_PER_DAY, TradeView, TrialBlocked, blueprint, grow_seconds_at, harvest_points,
    herb_tier, plant_permille, star_multiplier_permille,
};

use gpui_kit::component::Sizable as _;
use gpui_kit::component::hover_card::HoverCard;
// `test_support()` marks the elements the headless window tests observe; without
// `gpui-kit`'s `test-support` feature it is the identity function.
use gpui_kit::base::TestSupportExt as _;
use gpui_kit::{PlatformInput, ScrollDelta, ScrollWheelEvent};
use std::time::Duration;

mod arts;
mod attributes;
mod explore;
mod garden;
mod gear;
mod jobs;
mod proving;
mod records;
mod resources;
mod shop;
mod trial;

pub(super) use attributes::{CombatStats, derived_rows};
// The seed rows name which of the two kinds of seed they are — an attribute
// seed or a 潜质 herb seed — so the page's one open-detail slot can tell them
// apart. Only the window's own state field needs the type, so it stays here.
pub(super) use garden::SeedKey;
// Only the headless page tests read the formula directly; the lib itself
// reaches it through the page render.
use arts::arts_panel;
#[cfg(test)]
pub(super) use attributes::derived_formula;
use attributes::{attributes_panel, derived_panel};
use explore::{breakthrough_banner, breakthrough_panel, exploration_panel};
use garden::{garden_panel, seed_panel};
use gear::{equipment_panel, inventory_panel};
use jobs::jobs_panel;
// The history popup is mounted by the window frame beside the fight, so it
// has to be visible past this module.
use proving::proving_panel;
pub(super) use records::events_overlay;
use records::record_section;
use shop::shop_panel;
use trial::trial_panel;

/// How wide the growth bar is drawn, and therefore how wide its filled part is.
const GROWTH_BAR_WIDTH: f32 = 180.0;

/// 潜质 is stored in tenths and shown in points: 47 reads as "4.7".
///
/// One decimal, never two: every herb pays a tenth of a point at the smallest,
/// so a second decimal could only ever be a zero the user has to read past.
pub(super) fn tenths_text(tenths: i32) -> String {
    format!(
        "{:.1}",
        f32::from(i16::try_from(tenths).unwrap_or(0)) / 10.0
    )
}

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
    // Whether the page has a 破境 panel at all: the module bar draws a button
    // per panel that exists, and a button that jumps nowhere is worse than no
    // button.
    let breakthrough = breakthrough_panel(&view, &cultivation, locale, tokens);
    let has_breakthrough = breakthrough.is_some();
    let open_item = view.read(cx).cultivation_open_item;
    let open_offer = view.read(cx).cultivation_open_offer;
    let open_seed = view.read(cx).cultivation_open_seed;
    let open_plot = view.read(cx).cultivation_open_plot;
    let dig_confirm = view.read(cx).cultivation_dig_confirm;
    let dig_confirm_final = view.read(cx).cultivation_dig_confirm_final;
    let seeds_open = view.read(cx).cultivation_seeds_open;
    // One slot per panel, filled during prepaint with the panel's own bounds in
    // window coordinates. The navigation bar reads them to scroll the settings
    // list — whose `ListState` is private — by dispatching a scroll-wheel event
    // at the panel's position.
    let panel_bounds: Rc<RefCell<Vec<Option<Bounds<Pixels>>>>> =
        Rc::new(RefCell::new(vec![None; MODULE_COUNT]));

    div()
        .id("cultivation-settings")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(module_navigation_bar(
            &view,
            &panel_bounds,
            has_breakthrough,
            locale,
            tokens,
        ))
        .children(breakthrough_banner(&cultivation, locale, tokens))
        .child(realm_header(
            &view,
            &cultivation,
            view.read(cx).cultivation_ladder_open,
            locale,
            tokens,
        ))
        .children(notices(&cultivation, locale, tokens))
        .child(panel_anchor(
            0,
            &panel_bounds,
            record_section(&view, &cultivation, locale, tokens),
        ))
        .child(panel_anchor(
            1,
            &panel_bounds,
            attributes_panel(&view, &cultivation, locale, tokens),
        ))
        .child(panel_anchor(
            2,
            &panel_bounds,
            derived_panel(&cultivation, locale, tokens),
        ))
        .child(panel_anchor(
            3,
            &panel_bounds,
            equipment_panel(&view, &cultivation, locale, tokens, open_item),
        ))
        .child(panel_anchor(
            4,
            &panel_bounds,
            inventory_panel(&view, &cultivation, locale, tokens, open_item),
        ))
        .child(panel_anchor(
            5,
            &panel_bounds,
            arts_panel(&view, &cultivation, locale, tokens),
        ))
        .child(panel_anchor(
            6,
            &panel_bounds,
            seed_panel(&view, &cultivation, open_seed, seeds_open, locale, tokens),
        ))
        .child(panel_anchor(
            7,
            &panel_bounds,
            garden_panel(
                &view,
                &cultivation,
                open_plot,
                dig_confirm,
                dig_confirm_final,
                locale,
                tokens,
            ),
        ))
        .children(breakthrough.map(|panel| panel_anchor(8, &panel_bounds, panel)))
        .child(panel_anchor(
            9,
            &panel_bounds,
            exploration_panel(
                &view,
                &cultivation,
                ExplorationHours::from_hours(view.read(cx).cultivation_explore_hours),
                view.read(cx)
                    .cultivation_explore_region
                    .unwrap_or(usize::from(cultivation.exploration_region))
                    .min(cultivation.regions.len().saturating_sub(1)),
                locale,
                tokens,
            ),
        ))
        .child(panel_anchor(
            10,
            &panel_bounds,
            jobs_panel(&view, &cultivation, locale, tokens),
        ))
        .child(panel_anchor(
            11,
            &panel_bounds,
            proving_panel(&view, &cultivation, locale, tokens),
        ))
        .child(panel_anchor(
            12,
            &panel_bounds,
            trial_panel(&view, &cultivation, locale, tokens),
        ))
        .child(panel_anchor(
            13,
            &panel_bounds,
            resources::resources_panel(&view, &cultivation, locale, tokens),
        ))
        .child(panel_anchor(
            14,
            &panel_bounds,
            shop_panel(&view, &cultivation, open_offer, locale, tokens),
        ))
        .into_any_element()
}

/// The number of modules the navigation bar jumps between. Must match the
/// number of `panel_anchor` calls in [`content`].
const MODULE_COUNT: usize = 15;

/// Wrap a panel in an id'd div that records its bounds during prepaint.
///
/// The wrapper is a plain `div`: it adds no visual chrome. The id lets a test
/// find the panel, and the prepaint listener fills the shared bounds slot the
/// navigation bar reads when its buttons scroll.
fn panel_anchor(
    index: usize,
    panel_bounds: &Rc<RefCell<Vec<Option<Bounds<Pixels>>>>>,
    panel: impl gpui_kit::IntoElement,
) -> impl gpui_kit::IntoElement {
    let store = Rc::clone(panel_bounds);
    div()
        .on_children_prepainted(move |bounds, _, _| {
            if let (Some(slot), Some(first)) = (store.borrow_mut().get_mut(index), bounds.first()) {
                *slot = Some(*first);
            }
        })
        .id(SharedString::from(format!("cultivation-anchor-{}", index)))
        .test_support()
        .child(panel.into_any_element())
}

/// The navigation bar at the top of the cultivation page.
///
/// One button per module, in the order the modules appear on the page. A click
/// scrolls the page to that module.
///
/// The page is one tall item inside the settings list, so the list is the
/// scroll container — and the list keeps its `ListState` private. The button
/// therefore asks the list to scroll by dispatching a scroll-wheel event at the
/// target panel's own position: the event hit-tests to the panel and bubbles to
/// the list's handler, and the delta is the distance the panel sits from the
/// viewport's centre. The list clamps the scroll to its bounds, so a target
/// near either end lands exactly there.
fn module_navigation_bar(
    _view: &Entity<SettingsView>,
    panel_bounds: &Rc<RefCell<Vec<Option<Bounds<Pixels>>>>>,
    breakthrough: bool,
    _locale: &str,
    tokens: Tokens,
) -> impl gpui_kit::IntoElement {
    // 只保留需要导航的模块：境界、最近事件、属性、外显属性、装备已在页面上方，
    // 种子和仙草园合并为一个「种子」，试炼和锁妖塔合并为一个「试炼」。
    let modules: [(SharedString, usize); 9] = [
        (SharedString::from("储物袋"), 4),
        (SharedString::from("功法"), 5),
        (SharedString::from("种子"), 6),
        (SharedString::from("破境"), 8),
        (SharedString::from("探索"), 9),
        (SharedString::from("打工"), 10),
        (SharedString::from("试炼"), 11),
        (SharedString::from("资源"), 13),
        (SharedString::from("坊市"), 14),
    ];
    let buttons = modules
        .into_iter()
        // 破境面板只在能破境时出现；按钮跟着面板走，不指一个不存在的位置。
        .filter(move |(_, index)| breakthrough || *index != 8)
        .map(|(label, index)| {
            let store = Rc::clone(panel_bounds);
            Button::new(format!("cultivation-nav-{index}"))
                .ghost()
                .small()
                .label(label)
                .on_click(move |_, window, cx| {
                    scroll_to_panel(index, &store, window, cx);
                })
        })
        .collect::<Vec<_>>();

    div()
        .id("cultivation-module-navigation")
        .flex()
        .flex_row()
        .gap_2()
        .flex_wrap()
        .py_2()
        .px_3()
        .bg(tokens.canvas)
        .rounded_lg()
        .children(buttons)
}

/// Scroll the settings list so the panel at `index` is in view.
///
/// The panel's bounds were recorded during its prepaint, in window
/// coordinates. The panel's *top* is aimed at a quarter of the viewport down
/// rather than at the middle: a module lands near the top of what the eye takes
/// in, with the page above it as context and three quarters of the screen for
/// the module itself. The delta is that gap — positive when the panel sits
/// below the mark (scroll down, a negative wheel delta) and negative when it
/// sits above (scroll up). A panel already at the mark asks for no scroll, and
/// the list clamps a panel near either end to the end.
///
/// The dispatch is deferred rather than sent from the click handler: a click
/// handler runs inside the list's own mouse dispatch, whose listeners have been
/// taken off the frame for the duration, so a scroll event raised there reaches
/// nobody. A deferred callback runs after the frame has been laid out and its
/// listeners are back, which is what lets the event hit the list at all.
fn scroll_to_panel(
    index: usize,
    panel_bounds: &Rc<RefCell<Vec<Option<Bounds<Pixels>>>>>,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(bounds) = panel_bounds.borrow().get(index).copied().flatten() else {
        return;
    };
    let viewport = window.bounds();
    // A quarter of the way down: the target panel's first line sits there, so
    // the module reads from its heading rather than from its middle.
    let aim = viewport.origin.y + viewport.size.height / 4.0;
    let delta_y = bounds.origin.y - aim;
    if delta_y.abs() < px(1.) {
        return;
    }
    // The event's position only decides which element it hit-tests to; the
    // delta is what scrolls. It has to land inside the list — a position at the
    // target panel is often below the fold, where the hit-test finds nothing
    // and the event goes nowhere — so it is aimed at the middle of the page.
    let position = point(
        viewport.origin.x + viewport.size.width / 2.0,
        viewport.origin.y + viewport.size.height / 2.0,
    );
    let delta = ScrollDelta::Pixels(point(px(0.), -delta_y));
    window.defer(cx, move |window, cx| {
        window.dispatch_event(
            PlatformInput::ScrollWheel(ScrollWheelEvent {
                position,
                delta,
                ..Default::default()
            }),
            cx,
        );
    });
}

/// The realm line, the power, and how far through the realm the growth is.
fn realm_header(
    view: &Entity<SettingsView>,
    cultivation: &CultivationView,
    ladder_open: bool,
    locale: &str,
    tokens: Tokens,
) -> impl IntoElement {
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
        &[("power", group_thousands(cultivation.power))],
    );
    let (growth_text, fraction) = growth_line(cultivation, locale);
    let hint_tokens = tokens;
    // Read out here because the hover body outlives this call and cannot borrow
    // `locale`.
    let power_hint = text(locale, "cultivation.hint.power_hint");
    // The whole ladder, read out here for the same reason: the list says where
    // the cultivator stands among the ten, which level that is on the thousand,
    // and how the two are converted into one another.
    let level_line = format_text(
        locale,
        "cultivation.level.total",
        &[
            ("level", cultivation.level.to_string()),
            ("total", cultivation.total_levels.to_string()),
        ],
    );
    let rule = text(locale, "cultivation.level.rule").to_string();
    let realm_rows: Vec<(String, bool, String)> = Realm::ALL
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let from = index as u32 * u32::from(LAYERS_PER_STAGE) + 1;
            let to = (index as u32 + 1) * u32::from(LAYERS_PER_STAGE);
            (
                text(locale, row.name_key()).to_string(),
                *row == cultivation.realm,
                format_text(
                    locale,
                    "cultivation.level.span",
                    &[("from", from.to_string()), ("to", to.to_string())],
                ),
            )
        })
        .collect();

    let mut header = div()
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
                // The realm name is a control, not a title: clicking it unfolds
                // the whole thousand-level ladder, so "元婴 · 4 层 7 阶" can be
                // read against the ten rungs it is one of instead of standing
                // alone as a name the user has to place from memory.
                .child(
                    div()
                        .id("cultivation-realm-name")
                        .font_semibold()
                        .child(realm)
                        .test_support()
                        .on_click({
                            let view = view.clone();
                            move |_, _, cx| {
                                view.update(cx, |view, cx| {
                                    view.cultivation_ladder_open = !view.cultivation_ladder_open;
                                    cx.notify();
                                });
                            }
                        }),
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
        );
    if ladder_open {
        header = header.child(realm_ladder_body(
            "cultivation-realm-ladder",
            level_line,
            rule,
            realm_rows,
            tokens,
        ));
    }
    header
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
            DropView::Seed {
                attribute,
                tier,
                grade,
            } => (
                "cultivation.notice.drop_seed",
                seed_label(locale, attribute, tier, grade, None),
            ),
            DropView::Art { name } => ("cultivation.notice.drop_art", name),
            DropView::Herb { herb } => (
                "cultivation.notice.drop_herb",
                text(locale, herb.name_key()).to_string(),
            ),
            DropView::Stones(stones) => ("cultivation.notice.drop_stones", stones.to_string()),
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
                        seed_label(
                            locale,
                            seed.attribute,
                            seed.tier,
                            seed.grade,
                            seed.name.as_deref(),
                        ),
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
    // What the last gathering turned up: the site, the find and the trade's xp,
    // one line, so a click that yielded nothing the purse had room for says so.
    if let Some(collect) = &cultivation.last_collect {
        notices.push(
            notice(
                "cultivation-notice-collect",
                format_text(
                    locale,
                    "cultivation.notice.collect",
                    &[
                        ("site", text(locale, collect.site.key()).to_string()),
                        ("gained", resource_bag_text(locale, &collect.gained)),
                        ("xp", (collect.woodcut_xp + collect.stonecut_xp).to_string()),
                    ],
                ),
                tokens.accent,
            )
            .into_any_element(),
        );
    }
    // What the last building rank came to.
    if let Some(building) = &cultivation.last_building {
        notices.push(
            notice(
                "cultivation-notice-building",
                format_text(
                    locale,
                    "cultivation.notice.building_upgrade",
                    &[
                        (
                            "building",
                            text(locale, building.building.key()).to_string(),
                        ),
                        ("level", building.to.to_string()),
                    ],
                ),
                tokens.accent,
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

/// The tribute one 升星 burns, spelled out: "木×3 · 石×2" or, for a high star,
/// "木×3 · 石×2 · 铁×2". The names are the catalog's own, and a find the purse
/// is not asked for is not listed — an attempt at a low star never names iron.
fn resource_bag_text(locale: &str, bag: &ResourceBag) -> String {
    let mut parts = Vec::new();
    for resource in Resource::ALL {
        let count = bag.get(resource);
        if count > 0 {
            parts.push(format!("{}×{count}", text(locale, resource.key())));
        }
    }
    parts.join(" · ")
}

/// What one 升星 button says: the 灵石 wager and the forge's tribute, so the
/// number the user weighs is the whole price and not a half of it.
fn star_cost_text(locale: &str, cost: &Cost) -> String {
    format_text(
        locale,
        "cultivation.item.star_button",
        &[
            ("stones", cost.stones.to_string()),
            ("tribute", resource_bag_text(locale, &cost.resources)),
        ],
    )
}

/// The 升星 button: what the wager costs, spelled out, and nothing at all once
/// the piece holds every star it can.
///
/// The price is the domain's own — the 灵石 wager and the forge's tribute — so
/// the number on the button is the number the purse pays. A capped piece offers
/// no button rather than a disabled one: a button that cannot be pressed is a
/// button the user has to wonder about.
fn star_button(
    view: &Entity<SettingsView>,
    item: &ItemView,
    locale: &str,
) -> Option<gpui_kit::AnyElement> {
    let cost = item.star_cost.as_ref()?;
    let item_id = item.id;
    Some(
        Button::new(format!("cultivation-star-{item_id}"))
            .ghost()
            .small()
            .label(star_cost_text(locale, cost))
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

/// The realm ladder: the level on the thousand, the conversion rule that puts it
/// there, and the ten rungs with the one the cultivator stands in marked.
///
/// "元婴 · 4 层 7 阶" alone does not say whether that is level 347 of 1000 or
/// near the top of the ladder — so the level, the total, the rule that joins
/// them and all ten rungs are shown together. The current rung carries the
/// danger colour and a caret: the colour is the page's word for "this is the one
/// that matters", which on every other row means a penalty and here means the
/// rung the save is standing on.
fn realm_ladder_body(
    id: impl Into<gpui_kit::ElementId>,
    level_line: String,
    rule: String,
    realms: Vec<(String, bool, String)>,
    tokens: Tokens,
) -> impl IntoElement {
    div()
        .id(id.into())
        .flex()
        .flex_col()
        .gap_1()
        .p_2()
        .rounded_md()
        .bg(tokens.overlay)
        .text_xs()
        .text_color(tokens.muted)
        .whitespace_normal()
        .child(
            div()
                .id("cultivation-level-total")
                .text_color(tokens.text)
                .child(level_line)
                .test_support(),
        )
        .child(
            div()
                .id("cultivation-level-rule")
                .child(rule)
                .test_support(),
        )
        .children(
            realms
                .into_iter()
                .enumerate()
                .map(move |(index, (name, current, span))| {
                    let row = div()
                        .id(format!("cultivation-realm-rung-{index}"))
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(if current { format!("▸ {name}") } else { name })
                        .child(span)
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
/// attributes, the combat numbers the piece adds outright, what its stars have
/// multiplied, and — for a 神装 — the effect it carries. Nothing here names a
/// 法术 or 武术 total: the piece's attributes are what it is, and the numbers a
/// fight reads are the ones below them.
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
                    permille_percent_label(star_multiplier_permille(item.id, item.star)),
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
    // The set, when the piece is a set piece: the name is the prefix the piece
    // carries, and the page says it so a 曜日 blade and a 曜日 robe read as kin.
    if let Some(set) = item.set {
        panel = panel.child(format_text(
            locale,
            "cultivation.item.set_line",
            &[("set", set.name().to_owned())],
        ));
    }
    panel
}

fn section_title(locale: &str, key: &'static str, tokens: Tokens) -> impl IntoElement {
    div()
        .text_base()
        .font_semibold()
        .text_color(tokens.text)
        .child(text(locale, key))
}

/// A section's header: its title, and the rule it plays by beside it.
///
/// The hints used to hang under their titles on a line of their own, which spent
/// a whole row of the page on prose and left the eye to jump a gap before it
/// reached the numbers. Read beside the title they cost nothing, are read
/// anyway, and the panel starts one row higher.
///
/// The rule is one line and stays one line: each hint is written short enough to
/// sit beside its title, and the row truncates with an ellipsis rather than
/// wrapping to a second — a hint that runs to two lines is the thing this
/// replaced. It carries its key as an id so a frame can be asked whether the
/// rule is on screen at all, which is the difference between a hint that is read
/// and one that is filed behind a hover.
fn section_header(
    locale: &str,
    key: &'static str,
    hint_key: &'static str,
    tokens: Tokens,
) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .child(section_title(locale, key, tokens))
        .child(
            div()
                .id(hint_key.replace('.', "-"))
                .flex_1()
                .min_w_0()
                .text_xs()
                .text_color(tokens.muted)
                .truncate()
                .child(text(locale, hint_key))
                .test_support(),
        )
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
    color_from_rgb(rarity.color())
}

/// The 功法 grade's colour, as the domain names it: 黄、蓝、紫、橙、红.
///
/// Read through the same conversion as the eight gear qualities, so a row of
/// 功法 is legible before its name is: the grade is the first thing a user
/// weighs, and five grades that all look alike are five grades nobody checks.
pub(super) fn grade_color(grade: ArtGrade) -> Hsla {
    color_from_rgb(grade.color())
}

/// The same ramp for a herb's grade: the two ladders share their five colours,
/// so a 天阶 herb and a 天阶 功法 read as the same rank at a glance.
pub(super) fn seed_grade_color(grade: SeedGrade) -> Hsla {
    color_from_rgb(grade.color())
}

/// One `0xRRGGBB` value as a paintable colour.
fn color_from_rgb(value: u32) -> Hsla {
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
        ("cultivation.defense", bonus.defense),
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

/// One active set as a line the page prints under its panel: the name the set's
/// pieces share, how many are active, and what the active count pays.
///
/// The bonus reads through the same lines a piece's own bonus reads through, so
/// a set's 攻击力 and a piece's 攻击力 cannot be printed by two rules.
pub(super) fn set_bonus_line(locale: &str, name: &str, count: u8, bonus: &Derived) -> String {
    let lines = bonus_lines(locale, bonus);
    let lines = if lines.is_empty() {
        // A set can only pay what it is, so this is a set that pays nothing —
        // which a surface prints as the count alone rather than as a bare "：".
        "—".to_owned()
    } else {
        lines.join(" · ")
    };
    format_text(
        locale,
        "cultivation.set.line",
        &[
            ("set", name.to_owned()),
            ("count", count.to_string()),
            ("lines", lines),
        ],
    )
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

/// A parts-per-thousand figure as the percent it is worth, one decimal at most:
/// 106.7 reads `106.7`, 110 reads `110`.
///
/// The one conversion every percent on the page reads through, so a 405‰ rate
/// and a 1067‰ star cannot be printed by two rules that disagree.
pub(super) fn permille_percent_label(permille: u32) -> String {
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
    grade: SeedGrade,
    name: Option<&str>,
) -> String {
    let grade_name = text(locale, grade.name_key()).to_string();
    match name {
        Some(name) => format_text(
            locale,
            "cultivation.seed.label_named",
            &[
                ("name", name.to_owned()),
                ("tier", tier.to_string()),
                ("grade", grade_name),
            ],
        ),
        None => format_text(
            locale,
            "cultivation.seed.label",
            &[
                ("attribute", text(locale, attribute.name_key()).to_string()),
                ("tier", tier.to_string()),
                ("grade", grade_name),
            ],
        ),
    }
}

/// A 潜质 herb seed's or crop's display line: the herb's own catalog name and its
/// tier.
///
/// Reuses the seed label keys, so a herb seed and an attribute seed of the same
/// numbers read the same way on the shelf and in the satchel — only the name
/// they fall back to differs (a herb's catalog name, not an attribute's). The
/// hero name a seed can carry is deliberately not read here: a herb seed rolled
/// one when it was folded in, and the satchel then showed 幽菊 for the 悟心草 the
/// shelf had just sold. An old save keeps those names in its stacks; this line
/// no longer repeats them.
pub(super) fn herb_seed_label(
    locale: &str,
    herb: AptitudeHerb,
    tier: u8,
    grade: SeedGrade,
) -> String {
    let grade_name = text(locale, grade.name_key()).to_string();
    format_text(
        locale,
        "cultivation.seed.label",
        &[
            ("attribute", text(locale, herb.name_key()).to_string()),
            ("tier", tier.to_string()),
            ("grade", grade_name),
        ],
    )
}

/// The plain name of an attribute slot, without the `%{value}` the
/// `cultivation.<slot>` catalogs carry. Those catalogs are templates meant to
/// be printed beside a number ("生命值 %{value}"); the sites that want only the
/// name drop the placeholder rather than leaking it into the page.
pub(super) fn slot_name(locale: &str, key: &str) -> String {
    format_text(locale, key, &[("value", String::new())])
        .trim()
        .to_string()
}
