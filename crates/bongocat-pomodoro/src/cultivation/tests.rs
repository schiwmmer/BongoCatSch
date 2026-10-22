//! The ladder's rules, exercised without a clock: the tests hand in every
//! instant and every local day, so a hundred days of absence take microseconds.

use super::explore::WALK_STONES;
use super::garden::{grow_seconds, harvest_points};
use super::*;
use crate::CalendarDay;

/// The growth that fills the first realm: the last layer of 炼气.
const FIRST_REALM_CAP: u32 = 9_900;

fn day(number: i64) -> CalendarDay {
    CalendarDay::from_day_number(number)
}

/// A cultivator at the first realm's last layer, with nothing equipped.
fn at_the_first_cap() -> Cultivation {
    Cultivation::new(CultivationDocument {
        growth: FIRST_REALM_CAP,
        last_active_day: Some(20_000),
        charged_through_day: Some(20_000),
        ..CultivationDocument::default()
    })
}

/// Four pieces of gear, each worth enough to make up the guardian's edge.
fn geared(mut cultivation: Cultivation) -> Cultivation {
    let mut document = cultivation.document().clone();
    for (index, slot) in EquipSlot::ALL.into_iter().enumerate() {
        let item = EquipmentRecord {
            id: index as u64 + 1,
            slot,
            tier: 1,
            rarity: Rarity::Rare,
            attributes: Attributes::new(5, 40, 0, 5),
            name: None,
        };
        document.equipment.push(item.clone());
        document.equipped[slot.index()] = Some(item.id);
    }
    document.next_item_id = 5;
    cultivation = Cultivation::new(document);
    cultivation
}

#[test]
fn a_fresh_cultivator_starts_at_the_first_layer_of_the_first_realm() {
    let cultivation = Cultivation::default();
    let view = cultivation.view();
    assert_eq!(view.level, 1);
    assert_eq!(view.realm, Realm::QiRefining);
    assert_eq!(view.layer, 0, "the first layer of ten");
    assert_eq!(view.rank, 1);
    assert_eq!(view.growth, 0);
    assert_eq!(view.next, Some(threshold_for(2)));
    assert!(view.inventory.is_empty());
    assert_eq!(view.plots.len(), FREE_PLOT_COUNT);
    assert!(view.guardian.is_none());
}

#[test]
fn a_session_pays_growth_in_proportion_to_its_length() {
    // The first session of a day is the flat base, whatever its length.
    let mut cultivation = Cultivation::default();
    let reward = cultivation.on_session_completed(1_000, day(20_000), 25);
    assert_eq!(reward.growth, SESSION_BASE_GROWTH);
    assert_eq!(cultivation.document().growth, SESSION_BASE_GROWTH);
    assert_eq!(cultivation.document().last_active_day, Some(20_000));
    // A second session the same day is worth more, and counts as number two.
    let second = cultivation.on_session_completed(2_000, day(20_000), 25);
    assert_eq!(second.growth, session_growth(2, 1));
    assert!(second.growth > SESSION_BASE_GROWTH);
    // The session counter resets on a new local day.
    let third = cultivation.on_session_completed(3_000, day(20_001), 25);
    assert_eq!(third.growth, SESSION_BASE_GROWTH, "a new day starts at one");
}

#[test]
fn session_growth_scales_with_the_day_and_the_realm() {
    assert_eq!(session_growth(1, 1), 500);
    assert_eq!(session_growth(2, 1), 550, "500 * 1.1, truncated");
    assert_eq!(
        session_growth(1, 5),
        500,
        "the realm does not touch the first session"
    );
    // 500 * 1.1^2 * 2 = 1210.
    assert_eq!(session_growth(3, 2), 1210);
}

#[test]
fn each_interaction_pays_one_growth() {
    let mut cultivation = Cultivation::default();
    assert_eq!(
        cultivation.document().growth,
        0,
        "a fresh ladder owes nothing"
    );
    cultivation.on_input();
    cultivation.on_input();
    cultivation.on_input();
    assert_eq!(
        cultivation.document().growth,
        3,
        "every keyboard or mouse interaction pays exactly one growth"
    );
}

#[test]
fn a_missed_day_costs_growth_and_every_following_one_escalates() {
    let mut cultivation = Cultivation::new(CultivationDocument {
        growth: 100_000,
        last_active_day: Some(20_000),
        charged_through_day: Some(20_000),
        ..CultivationDocument::default()
    });
    // The first day without a session costs the base penalty (stage one).
    let first = cultivation.apply_absence(day(20_002)).expect("a charge");
    assert_eq!(first.days, 1);
    assert_eq!(first.charged, ABSENCE_BASE_PENALTY);
    // Two more missed days each escalate by the step, at stage one.
    let second = cultivation.apply_absence(day(20_004)).expect("a charge");
    let day_two = (ABSENCE_BASE_PENALTY as f64 * 1.2_f64.powi(1)) as u32;
    let day_three = (ABSENCE_BASE_PENALTY as f64 * 1.2_f64.powi(2)) as u32;
    assert_eq!(second.days, 2);
    assert_eq!(second.charged, day_two + day_three);
    assert_eq!(
        cultivation.document().growth,
        100_000 - ABSENCE_BASE_PENALTY - (day_two + day_three)
    );
}

#[test]
fn the_same_day_is_never_charged_twice() {
    let mut cultivation = Cultivation::new(CultivationDocument {
        growth: 1_000,
        last_active_day: Some(20_000),
        charged_through_day: Some(20_000),
        ..CultivationDocument::default()
    });
    assert!(cultivation.apply_absence(day(20_002)).is_some());
    assert!(
        cultivation.apply_absence(day(20_002)).is_none(),
        "a re-launch on the same day charges nothing"
    );
    assert_eq!(cultivation.document().growth, 1_000 - ABSENCE_BASE_PENALTY);
}

#[test]
fn a_deeper_realm_pays_a_deeper_penalty() {
    let mut shallow = Cultivation::new(CultivationDocument {
        growth: 1_000_000,
        realm: 0,
        last_active_day: Some(20_000),
        charged_through_day: Some(20_000),
        ..CultivationDocument::default()
    });
    let shallow_charge = shallow
        .apply_absence(day(20_002))
        .expect("a charge")
        .charged;
    let mut deep = Cultivation::new(CultivationDocument {
        growth: 1_000_000,
        realm: 4,
        last_active_day: Some(20_000),
        charged_through_day: Some(20_000),
        ..CultivationDocument::default()
    });
    let deep_charge = deep.apply_absence(day(20_002)).expect("a charge").charged;
    // Stage five (realm index four) multiplies the base by five.
    assert_eq!(deep_charge, shallow_charge * 5);
}

#[test]
fn a_penalty_never_takes_back_the_realm_already_entered() {
    let mut cultivation = Cultivation::new(CultivationDocument {
        growth: 10_000,
        realm: 1,
        last_active_day: Some(20_000),
        charged_through_day: Some(20_000),
        ..CultivationDocument::default()
    });
    // A month of missing sessions would cost far more than the realm floor.
    let charge = cultivation.apply_absence(day(20_040)).expect("a charge");
    assert_eq!(charge.days, 39);
    assert_eq!(
        cultivation.document().growth,
        10_000,
        "the floor of the entered realm holds"
    );
    assert_eq!(charge.charged, 0, "nothing was taken");
    assert_eq!(cultivation.realm(), Realm::Foundation);
}

/// 法术 is 根骨 plus 仙力, 武术 is 力量 plus 身法: a point in one school must never
/// move the other, and the whole is exactly the two halves added up.
#[test]
fn the_two_schools_read_only_their_own_attributes() {
    let spell = Attributes::new(10, 0, 10, 0);
    let martial = Attributes::new(0, 10, 0, 10);
    assert_eq!(spell.spell_power(), 45, "根骨 10 + 仙力 10 with synergy");
    assert_eq!(spell.martial_power(), 0, "a spell set fights no melee");
    assert_eq!(martial.spell_power(), 0, "a martial set casts nothing");
    assert_eq!(
        martial.martial_power(),
        42,
        "力量 10 + 身法 10 with synergy"
    );
    assert_eq!(
        spell.plus(martial).power(),
        87,
        "the total is the two schools, not a fourth weighting"
    );
}

/// Each pair pays for its shorter half, so twenty points spent evenly beat
/// twenty poured into the heavier attribute alone.
#[test]
fn a_balanced_pair_out_powers_a_lopsided_one() {
    let balanced = Attributes::new(10, 0, 10, 0);
    let lopsided = Attributes::new(0, 0, 20, 0);
    assert_eq!(lopsided.spell_power(), 40, "仙力 alone, no synergy");
    assert!(
        balanced.spell_power() > lopsided.spell_power(),
        "根骨 and 仙力 together must beat 仙力 alone"
    );
    let balanced = Attributes::new(0, 10, 0, 10);
    let lopsided = Attributes::new(0, 20, 0, 0);
    assert_eq!(lopsided.martial_power(), 40, "力量 alone, no synergy");
    assert!(
        balanced.martial_power() > lopsided.martial_power(),
        "力量 and 身法 together must beat 力量 alone"
    );
}

/// Every school has a heavier half: 仙力 carries a spell further than 根骨 does,
/// and 力量 lands harder than 身法.
#[test]
fn each_school_has_its_heavier_half() {
    assert!(
        Attributes::new(0, 0, 10, 0).spell_power() > Attributes::new(10, 0, 0, 0).spell_power()
    );
    assert!(
        Attributes::new(0, 10, 0, 0).martial_power() > Attributes::new(0, 0, 0, 10).martial_power()
    );
}

/// The seven shown numbers follow from the four allocated ones, in the direction
/// the panel promises: 力量 to 攻击力 and 生命值, 法力 to 法术强度 and 法抗, 身法 to
/// 速度/命中/闪避, 根骨 to 内力值 and 潜质.
#[test]
fn every_shown_number_follows_from_the_attribute_that_owns_it() {
    let empty = Attributes::default().derived();
    assert_eq!(empty.attack, 10);
    assert_eq!(empty.health, 100);
    assert_eq!(empty.inner, 50);
    assert_eq!(empty.speed, 10);

    let strong = Attributes::new(0, 10, 0, 0).derived();
    assert_eq!(strong.attack, 40, "10 + 力量×3");
    assert_eq!(strong.health, 300, "100 + 力量×20");
    assert_eq!(strong.spell, 0, "力量 casts nothing");

    let mana = Attributes::new(0, 0, 10, 0).derived();
    assert_eq!(mana.spell, 30);
    assert_eq!(mana.resist, 20);
    assert_eq!(mana.health, 100, "法力 buys no flesh");

    let swift = Attributes::new(0, 0, 0, 10).derived();
    assert_eq!(swift.speed, 30);
    assert_eq!(swift.hit, 200, "100 + 身法×10");
    assert_eq!(swift.evade, 200, "the same curve 命中 climbs");

    let rooted = Attributes::new(10, 0, 0, 0).derived();
    assert_eq!(rooted.inner, 130, "50 + 根骨×8");
    assert_eq!(rooted.aptitude, 20);
}

/// 命中 and 闪避 are plain numbers the fight reads as a ratio, so they climb
/// without a ceiling: the ratio is what holds the chance, not a cap.
#[test]
fn hit_and_evade_climb_as_plain_numbers() {
    let absurd = Attributes::new(0, 0, 0, 10_000).derived();
    assert_eq!(absurd.hit, 100_100);
    assert_eq!(absurd.evade, 100_100);
    // Equal is equal at every depth: the curve the fight reads turns that
    // into the same eighty percent whatever the numbers are.
    assert_eq!(absurd.hit, absurd.evade);
}

#[test]
fn the_guardian_is_beyond_a_naked_cultivator_at_the_cap() {
    let naked = at_the_first_cap();
    let guardian = naked.guardian().expect("a guardian at the cap");
    assert!(guardian.power > naked.power(), "gear must be required");
    let outcome = naked.clone().breakthrough(1_000).expect("an attempt");
    assert!(!outcome.report.won);
    assert_eq!(outcome.target, Realm::Foundation);
    assert!(outcome.reward.is_none());
    assert_eq!(naked.document().realm, 0, "a loss opens nothing");
}

#[test]
fn gear_is_what_gets_a_cultivator_through() {
    let mut cultivation = geared(at_the_first_cap());
    assert!(cultivation.power() > cultivation.guardian().expect("a guardian").power);
    let outcome = cultivation.breakthrough(1_000).expect("an attempt");
    assert!(outcome.report.won);
    assert_eq!(cultivation.realm(), Realm::Foundation);
    assert_eq!(
        cultivation.level(),
        101,
        "the win lifts the cap to the new realm's first layer"
    );
    assert!(
        outcome.reward.is_some(),
        "a won breakthrough pays a piece of gear"
    );
    assert_eq!(cultivation.document().breakthrough_failures, 0);
}

#[test]
fn the_report_credits_the_power_that_fought_not_the_trophy() {
    let mut cultivation = geared(at_the_first_cap());
    let before = cultivation.power();
    let outcome = cultivation.breakthrough(1_000).expect("an attempt");
    assert_eq!(
        outcome.report.player_power, before,
        "the report must show what walked into the fight"
    );
    assert!(
        cultivation.power() > before,
        "the win itself adds power, which is why the report cannot be read after it"
    );
}

#[test]
fn a_breakthrough_is_only_offered_at_the_realm_cap() {
    let mut cultivation = Cultivation::new(CultivationDocument {
        growth: 100,
        ..CultivationDocument::default()
    });
    assert!(cultivation.guardian().is_none());
    assert!(cultivation.breakthrough(1_000).is_none());
}

#[test]
fn growth_beyond_the_cap_is_kept_but_buys_no_layers() {
    let cultivation = Cultivation::new(CultivationDocument {
        growth: 12_000,
        ..CultivationDocument::default()
    });
    assert_eq!(cultivation.level(), 100, "the cap holds the level down");
    assert_eq!(cultivation.view().next, None, "the realm bar reads full");
}

#[test]
fn a_planted_seed_ripens_on_the_clock_and_pays_an_attribute() {
    let mut cultivation = Cultivation::default();
    document_seeds(&mut cultivation, Attribute::Root, 1, 1);
    assert!(cultivation.plant(Attribute::Root, 1, 1_000));
    let planted = cultivation.view().plots[0]
        .crop
        .clone()
        .expect("a crop in the first plot");
    assert_eq!(planted.ready_at_unix, 1_000 + grow_seconds(1));
    assert!(
        cultivation.harvest(0, 1_000).is_none(),
        "an unripe crop is not pickable"
    );
    assert!(
        !cultivation.plant(Attribute::Strength, 1, 1_000),
        "a seed the cultivator does not own cannot be planted"
    );
    let harvested = cultivation
        .harvest(0, 1_000 + grow_seconds(1))
        .expect("a ripe crop");
    assert_eq!(harvested.crop.attribute, Attribute::Root);
    assert_eq!(
        cultivation.crop_attributes().root,
        harvest_points(1),
        "the harvest is permanent"
    );
    assert_eq!(
        cultivation.view().plots[0].crop,
        None,
        "the plot is free again"
    );
}

/// A harvest line says which herb came up and what it paid: the plot is empty
/// afterwards, so a line that said only "a crop was pulled" left nothing to
/// check the attribute gain against.
#[test]
fn a_harvest_is_logged_with_its_herb_and_its_points() {
    let mut document = CultivationDocument::default();
    document.seeds.push(SeedStack {
        attribute: Attribute::Mana,
        tier: 3,
        count: 1,
        name: Some("青芝".to_owned()),
    });
    let mut cultivation = Cultivation::new(document);
    assert!(cultivation.plant(Attribute::Mana, 3, 0));
    assert_eq!(
        cultivation.view().plots[0]
            .crop
            .as_ref()
            .and_then(|crop| crop.name.clone()),
        Some("青芝".to_owned()),
        "the plot keeps the herb's name the seed stack gave it"
    );
    cultivation
        .harvest(0, grow_seconds(3))
        .expect("a ripe crop");
    let event = cultivation
        .view()
        .events
        .first()
        .expect("the harvest is logged")
        .detail
        .clone();
    assert_eq!(
        event,
        EventDetail::Harvested {
            attribute: Attribute::Mana,
            tier: 3,
            name: Some("青芝".to_owned()),
            points: harvest_points(3),
        }
    );
}

#[test]
fn a_seed_stack_is_spent_one_planting_at_a_time() {
    let mut cultivation = Cultivation::default();
    document_seeds(&mut cultivation, Attribute::Mana, 1, 2);
    assert!(cultivation.plant(Attribute::Mana, 1, 0));
    assert!(cultivation.plant(Attribute::Mana, 1, 0));
    assert!(
        !cultivation.plant(Attribute::Mana, 1, 0),
        "the stack is empty"
    );
    assert!(cultivation.view().seeds.is_empty());
}

#[test]
fn equipping_replaces_what_was_worn_and_unequipping_keeps_the_item() {
    let mut cultivation = Cultivation::default();
    let mut document = cultivation.document().clone();
    for id in 1..=2u64 {
        let item = EquipmentRecord {
            id,
            slot: EquipSlot::Weapon,
            tier: 1,
            rarity: Rarity::Common,
            attributes: Attributes::new(0, 10 * id as i32, 0, 0),
            name: None,
        };
        document.equipment.push(item);
    }
    document.next_item_id = 3;
    cultivation = Cultivation::new(document);
    assert!(cultivation.equip(1));
    assert_eq!(
        cultivation.view().equipped[0].clone().map(|item| item.id),
        Some(1)
    );
    assert!(cultivation.equip(2));
    assert_eq!(
        cultivation.view().equipped[0].clone().map(|item| item.id),
        Some(2),
        "the second weapon takes the slot"
    );
    assert_eq!(cultivation.view().inventory.len(), 1);
    assert!(cultivation.unequip(EquipSlot::Weapon));
    assert_eq!(
        cultivation.document().equipment.len(),
        2,
        "an unequipped item is still owned"
    );
    assert!(cultivation.view().equipped[0].is_none());
}

#[test]
fn drops_are_rare_next_to_the_growth_a_session_pays() {
    let mut cultivation = Cultivation::default();
    let mut drops = 0u32;
    let mut sessions = 0u32;
    for second in 1..=1_000i64 {
        let reward = cultivation.on_session_completed(second * 7, day(20_000 + second), 25);
        sessions += 1;
        if reward.drop.is_some() {
            drops += 1;
        }
    }
    assert_eq!(sessions, 1_000);
    assert!(
        (20..=250).contains(&drops),
        "1,000 sessions dropped {drops} items, which is not the ~11% the rates describe"
    );
}

#[test]
fn a_reload_reads_the_ladder_back() {
    let mut cultivation = geared(at_the_first_cap());
    let outcome = cultivation.breakthrough(1_000).expect("an attempt");
    assert!(outcome.report.won);
    let document = cultivation.document().clone();
    let parsed: CultivationDocument =
        serde_json::from_str(&serde_json::to_string(&document).expect("serialize"))
            .expect("deserialize");
    let reloaded = Cultivation::new(parsed);
    assert_eq!(reloaded.realm(), Realm::Foundation);
    assert_eq!(reloaded.level(), cultivation.level());
    assert_eq!(reloaded.document().equipment.len(), 5);
}

/// Give the cultivator a stack of seeds without going through a drop.
fn document_seeds(cultivation: &mut Cultivation, attribute: Attribute, tier: u8, count: u32) {
    let mut document = cultivation.document().clone();
    document.seeds.push(SeedStack {
        attribute,
        tier,
        count,
        name: None,
    });
    *cultivation = Cultivation::new(document);
}

#[test]
fn every_level_pays_one_point_the_user_places() {
    // Level one has bought nothing yet: the ladder starts where the first
    // point would be paid, not before it.
    let mut cultivation = Cultivation::default();
    assert_eq!(cultivation.level(), 1);
    assert_eq!(cultivation.free_points(), 0);
    assert!(
        !cultivation.allocate(Attribute::Strength),
        "nothing to place"
    );

    // One session already jumps several layers; the points scale with the level
    // reached rather than being one per session.
    cultivation.on_session_completed(1_000, day(20_000), 25);
    let level = cultivation.level();
    assert!(level > 1, "a session moved the ladder");
    assert_eq!(
        cultivation.free_points(),
        (level.saturating_sub(1)) as u32 * FREE_POINTS_PER_LEVEL
    );

    let before = cultivation.attributes().strength;
    assert!(cultivation.allocate(Attribute::Strength));
    assert_eq!(
        cultivation.attributes().strength,
        before + 1,
        "the placed point is part of the attribute total"
    );
    assert_eq!(
        cultivation.free_points(),
        (level.saturating_sub(1) as u32).saturating_sub(1) * FREE_POINTS_PER_LEVEL
    );
    assert_eq!(cultivation.view().allocated_attributes.strength, 1);
    // The per-level grant leaves `free_points` points to place. Spending one leaves
    // the rest, so a second attribute can still take a point when any remain, and
    // only runs dry once they are all placed.
    let rest = cultivation.free_points();
    let spent = cultivation.allocate(Attribute::Root);
    assert_eq!(spent, rest > 0, "a point is placeable iff one remains");
    if spent {
        assert_eq!(cultivation.view().allocated_attributes.root, 1);
        assert_eq!(cultivation.free_points(), rest.saturating_sub(1));
    }
}

#[test]
fn an_absence_penalty_lowers_the_level_but_never_the_points_earned() {
    let mut cultivation = Cultivation::new(CultivationDocument {
        growth: 1_000,
        last_active_day: Some(20_000),
        charged_through_day: Some(20_000),
        ..CultivationDocument::default()
    });
    let granted = cultivation.granted_points();
    let level = cultivation.level();
    assert!(granted > 0, "level {level} has paid points");

    let charge = cultivation
        .apply_absence(day(20_010))
        .expect("nine missed days");
    assert!(charge.charged > 0);
    assert!(cultivation.level() < level, "the ladder came down");
    assert_eq!(
        cultivation.granted_points(),
        granted,
        "a penalty costs growth, not the points already earned"
    );
}

#[test]
fn a_reached_level_is_not_paid_twice() {
    let mut cultivation = Cultivation::new(CultivationDocument {
        growth: 1_000,
        ..CultivationDocument::default()
    });
    for second in 1..=20i64 {
        cultivation.on_session_completed(second * 7, day(20_000), 25);
    }
    // Twenty sessions in one day compound the growth, but the free points are paid
    // once against the highest level reached, never once per session.
    let level = cultivation.level();
    assert!(level > 1, "the day of sessions moved the ladder");
    assert_eq!(
        cultivation.granted_points(),
        (level.saturating_sub(1)) as u32 * FREE_POINTS_PER_LEVEL,
        "the points track the peak level, not the session count"
    );
    assert_eq!(
        cultivation.granted_points(),
        (cultivation.document().peak_level.saturating_sub(1)) as u32 * FREE_POINTS_PER_LEVEL,
        "the peak is the level reached"
    );
}

#[test]
fn a_harvest_gives_a_seed_back_about_one_time_in_seven() {
    let mut rng = super::Rng::seeded(0x5EED_5EED);
    let mut returns = 0u32;
    let mut upgraded = 0u32;
    for _ in 0..10_000 {
        if let Some((attribute, tier)) = super::garden::seed_return(Attribute::Root, 3, &mut rng) {
            returns += 1;
            assert_eq!(attribute, Attribute::Root, "the seed keeps its attribute");
            assert!(
                (3..=4).contains(&tier),
                "a returned seed is the same tier or one higher, not {tier}"
            );
            if tier == 4 {
                upgraded += 1;
            }
        }
    }
    assert!(
        (1_200..=1_800).contains(&returns),
        "10,000 harvests returned {returns} seeds, which is not the ~15% the rate describes"
    );
    let upgrade_percent = upgraded * 100 / returns.max(1);
    assert!(
        (20..=40).contains(&upgrade_percent),
        "{upgrade_percent}% of returns were a tier higher, which is not the ~30% intended"
    );
}

#[test]
fn a_harvest_that_gives_a_seed_back_puts_it_in_the_satchel() {
    let mut cultivation = Cultivation::default();
    let mut returned = None;
    for step in 0..200i64 {
        let planted_at = step * 10_000;
        document_seeds(&mut cultivation, Attribute::Mana, 2, 1);
        assert!(cultivation.plant(Attribute::Mana, 2, planted_at));
        let outcome = cultivation
            .harvest(0, planted_at + grow_seconds(2))
            .expect("a ripe crop");
        if let Some(seed) = outcome.seed {
            returned = Some(seed);
            break;
        }
    }
    let (attribute, tier) = returned.expect("200 harvests at 15% return at least one seed");
    assert_eq!(attribute, Attribute::Mana);
    assert_eq!(
        cultivation.view().last_seed_return.map(|seed| seed.tier),
        Some(tier)
    );
    assert!(
        cultivation
            .view()
            .seeds
            .iter()
            .any(|stack| stack.attribute == attribute && stack.tier == tier),
        "the returned seed is plantable"
    );
}

/// A 功法 is learned by finishing pomodoros, not by pressing a button: the
/// selected art takes the session and nothing else does.
#[test]
fn a_finished_session_trains_the_selected_art() {
    let mut cultivation = Cultivation::default();
    let granted = cultivation
        .grant_art(ArtGrade::Yellow, 1_000)
        .expect("a 黄阶 art");
    assert_eq!(
        cultivation.view().selected_art,
        Some(granted.id),
        "the first 功法 learned is the one being trained"
    );
    // Ninety-odd sessions is more than any 黄阶 art needs, so the ladder ends
    // the same way whatever the rolls were: mastered, with progress cleared.
    for offset in 0..40_i64 {
        cultivation.on_session_completed(1_000 + offset, day(30_000 + offset), 25);
    }
    let view = cultivation.view();
    // The book has been read to the end, so it is no longer a book: it has
    // become one of the 功法 the cultivator has learned outright.
    assert!(
        !view.arts.iter().any(|art| art.id == granted.id),
        "the 功法书 is no longer being trained"
    );
    let learned = view
        .mastered_arts
        .iter()
        .find(|art| art.id == granted.id)
        .expect("the 功法 is learned");
    assert_eq!(learned.rank, learned.grade.ranks());
    assert!(
        view.last_training.is_some(),
        "the panel says what the session did"
    );
}

/// An 内功 that has been trained pays attributes, and it pays through the same
/// total every other source feeds.
#[test]
fn a_trained_internal_art_feeds_the_attribute_total() {
    let mut cultivation = Cultivation::default();
    let before = cultivation.attributes().total();
    let granted = cultivation
        .grant_art(ArtGrade::Yellow, 7)
        .expect("a 黄阶 art");
    if blueprint(granted.id).kind != ArtKind::Internal {
        return;
    }
    for offset in 0..20_i64 {
        cultivation.on_session_completed(2_000 + offset, day(40_000 + offset), 25);
    }
    let paid = cultivation.art_attributes().total();
    assert!(paid > 0, "a trained 内功 pays something");
    assert_eq!(
        cultivation.attributes().total(),
        before + paid,
        "the 功法 total is part of the whole"
    );
}

/// Training can be stopped, and an art that is not owned cannot be trained.
#[test]
fn only_an_owned_art_can_be_selected() {
    let mut cultivation = Cultivation::default();
    assert!(!cultivation.select_art(Some(1)), "nothing is owned yet");
    let granted = cultivation.grant_art(ArtGrade::Yellow, 3).expect("an art");
    assert!(cultivation.select_art(Some(granted.id)));
    assert_eq!(cultivation.view().selected_art, Some(granted.id));
    assert!(cultivation.select_art(None));
    assert_eq!(cultivation.view().selected_art, None);
}

/// A lost breakthrough locks the attempt for twelve hours, and the lock lifts.
#[test]
fn a_lost_breakthrough_cannot_be_retried_at_once() {
    let mut cultivation = at_the_first_cap();
    let lost = cultivation
        .breakthrough(1_000)
        .expect("an attempt at the cap");
    assert!(!lost.duel.won, "a naked cultivator loses the fight");
    assert!(
        cultivation.breakthrough(1_001).is_none(),
        "the next attempt is locked out"
    );
    assert!(
        cultivation
            .breakthrough(1_000 + BREAKTHROUGH_COOLDOWN_SECONDS)
            .is_some()
    );
    // The fight is logged blow by blow, which is what the panel replays.
    assert!(
        !lost.duel.rounds.is_empty(),
        "a loss is shown, not merely reported"
    );
}

/// Winning the fight pays a 功法, every time: that is the promise the fight is
/// made for.
///
/// The fight is rolled, so the test walks a handful of clocks rather than betting
/// on one — the assertion is about what a win pays, not about a lucky seed.
#[test]
fn winning_the_fight_always_pays_an_art() {
    let mut paid = 0;
    for attempt in 0..20_i64 {
        let mut cultivation = geared(at_the_first_cap());
        let Some(won) = cultivation.breakthrough(2_000 + attempt * 1_000) else {
            continue;
        };
        if !won.duel.won {
            continue;
        }
        paid += 1;
        assert!(
            won.granted_art.is_some(),
            "a win pays a 功法, not just the next realm"
        );
        assert!(!cultivation.view().arts.is_empty());
    }
    assert!(
        paid > 0,
        "gear carries a cultivator through: no clock in twenty produced a win"
    );
}

/// Gear in the bag but on nobody: the cultivator fights as bare as one with
/// nothing at all, so the first guardian wins — and the loss still has gear to
/// cost. A win is not what is under test here.
fn carrying(cultivation: Cultivation) -> Cultivation {
    let mut document = cultivation.document().clone();
    for slot in document.equipped.iter_mut() {
        *slot = None;
    }
    Cultivation::new(document)
}

/// A loss costs a piece of gear one time in four and nothing the rest of the
/// time: it is a sting, not a setback. Never everything, never nothing.
#[test]
fn a_loss_costs_gear_sometimes_and_never_everything() {
    let mut losses = 0;
    let mut fights = 0;
    for clock in 0..60_i64 {
        let mut cultivation = carrying(geared(at_the_first_cap()));
        let Some(outcome) = cultivation.breakthrough(1_000 + clock * 100_000) else {
            continue;
        };
        if outcome.duel.won {
            continue;
        }
        fights += 1;
        if outcome.lost.is_some() {
            losses += 1;
            assert_eq!(
                cultivation.document().equipment.len(),
                3,
                "exactly one piece is lost, out of the four carried"
            );
        }
    }
    assert!(
        fights >= 30,
        "a cultivator with nothing worn loses to the first guardian: {fights}/60"
    );
    assert!(losses > 0, "a loss costs gear sometimes: {losses}/{fights}");
    assert!(
        losses < fights,
        "a loss is a chance, not a rule: {losses}/{fights}"
    );
}

/// Exploration is rationed by a cooldown, and a walk is never free of
/// consequences: it pays something, or it meets someone.
#[test]
fn one_walk_every_twelve_hours() {
    let mut cultivation = Cultivation::default();
    let first = cultivation.explore(10_000).expect("the first walk is owed");
    assert!(
        cultivation.explore(10_001).is_none(),
        "the walk is not owed"
    );
    // The panel reads the same rule the walk does, from the clock it is handed.
    assert!(
        !cultivation.view_on(10_001, 10_001).exploration_ready,
        "the second walk is not owed yet"
    );
    assert!(
        cultivation
            .view_on(10_000 + EXPLORATION_COOLDOWN_SECONDS, 10_000)
            .exploration_ready
    );
    let second = cultivation
        .explore(10_000 + EXPLORATION_COOLDOWN_SECONDS)
        .expect("the cooldown has run out");
    // Whatever the walk was, the panel can say what came of it.
    assert_eq!(cultivation.view().last_exploration, Some(second));
    let _ = first;
}

/// A walk that turns something up says what it turned up: the herb, the piece,
/// the 功法 or the 灵石, not merely that something happened.
///
/// Every walk that comes home pays at least its purse, so "nothing at all" is no
/// longer an outcome a walk can reach: the finding roll still misses most of the
/// time, and the line then says what the purse held.
#[test]
fn a_walk_says_what_it_turned_up() {
    let mut items = 0;
    let mut purse_only = 0;
    for clock in 0..40_i64 {
        let mut cultivation = Cultivation::default();
        let Some(outcome) = cultivation.explore(1_000 + clock * 100_000) else {
            continue;
        };
        let turned_up = cultivation.view().last_exploration_found.clone();
        let turned_up = turned_up.expect("a walk always carries what it found");
        match outcome {
            // A walk that was beaten owes nothing: the fight decided it, and a
            // purse that filled up on a defeat would make being beaten a way of
            // earning.
            ExplorationOutcome::Defeat => {
                assert!(
                    turned_up.is_empty(),
                    "a walk that was beaten turned something up"
                )
            }
            _ => {
                assert!(
                    !turned_up.is_empty(),
                    "a walk that came home paid nothing at all"
                );
                assert!(
                    turned_up.stones.is_some(),
                    "every walk that comes home pays a purse"
                );
                if turned_up.seed.is_some() || turned_up.gear.is_some() || turned_up.art.is_some() {
                    items += 1;
                } else {
                    purse_only += 1;
                }
            }
        }
    }
    assert!(items > 0, "a walk turns something up sometimes: {items}/40");
    assert!(
        purse_only > 0,
        "and comes home with only a purse the rest of the time: {purse_only}/40"
    );
}

/// A fight counts once, and only when it is joined: a surface watches the count
/// to know there is a report it has not shown, and a running pomodoro bumping
/// the snapshot revision every second cannot say that.
#[test]
fn a_fight_and_a_walk_each_count_once() {
    let mut cultivation = at_the_first_cap();
    assert_eq!(cultivation.view().breakthrough_attempts, 0);
    let _ = cultivation.breakthrough(1_000);
    assert_eq!(
        cultivation.view().breakthrough_attempts,
        1,
        "the fight counted"
    );
    // Held by the cooldown the loss wrote: nothing further is counted.
    assert!(cultivation.breakthrough(1_001).is_none());
    assert_eq!(
        cultivation.view().breakthrough_attempts,
        1,
        "a refused attempt does not count"
    );

    let mut walking = Cultivation::default();
    assert_eq!(walking.view().explorations, 0);
    let _ = walking.explore(5_000);
    assert_eq!(walking.view().explorations, 1, "the walk counted");
    assert!(walking.explore(5_001).is_none());
    assert_eq!(
        walking.view().explorations,
        1,
        "a walk that is not owed does not count"
    );
}

/// While a loss holds the attempt, the panel counts down to the next one rather
/// than showing a button that does nothing.
#[test]
fn a_lost_fight_says_when_the_next_one_is_owed() {
    let mut cultivation = at_the_first_cap();
    let lost = cultivation
        .breakthrough(1_000)
        .expect("an attempt at the cap");
    assert!(!lost.duel.won, "a naked cultivator loses the fight");
    let owed = 1_000 + BREAKTHROUGH_COOLDOWN_SECONDS;
    let held = cultivation.view_on(owed - 1, 0);
    assert_eq!(held.breakthrough_ready_at, Some(owed));
    assert!(!held.breakthrough_ready, "the attempt is still held");
    assert!(
        cultivation.view_on(owed, 0).breakthrough_ready,
        "the lock lifts"
    );
}

/// A walk can turn up 灵石, and they land in the purse.
///
/// The point of the rule is that the purse is fed by walking and not only by
/// working: a user who never sells anything still has a way to buy.
#[test]
fn a_walk_can_turn_up_stones() {
    let mut turned_up = 0;
    for clock in 0..60_i64 {
        let mut cultivation = Cultivation::default();
        let Some(outcome) = cultivation.explore(1_000 + clock * 100_000) else {
            continue;
        };
        let found = cultivation.view().last_exploration_found.clone();
        let Some(found) = found else { continue };
        let Some(amount) = found.stones else {
            continue;
        };
        turned_up += 1;
        assert!(
            (WALK_STONES.min..=WALK_STONES.max).contains(&amount),
            "a walk pays a handful, not a fortune: {amount}"
        );
        assert_eq!(cultivation.stones(), amount, "the stones land in the purse");
        if outcome == ExplorationOutcome::Stones {
            assert!(
                found.seed.is_none() && found.gear.is_none() && found.art.is_none(),
                "a walk that turned up stones and nothing else says so"
            );
        }
    }
    assert!(
        turned_up > 0,
        "a walk turns up stones sometimes: {turned_up}/60"
    );
}

/// A walk that was beaten pays nothing at all: a purse that filled up on a
/// defeat would make being beaten a way of earning.
#[test]
fn a_walk_that_was_beaten_pays_nothing() {
    for clock in 0..60_i64 {
        let mut cultivation = Cultivation::default();
        let Some(outcome) = cultivation.explore(1_000 + clock * 100_000) else {
            continue;
        };
        if outcome != ExplorationOutcome::Defeat {
            continue;
        }
        assert_eq!(
            cultivation.stones(),
            0,
            "a defeat fills the purse with nothing"
        );
        let found = cultivation.view().last_exploration_found.clone();
        assert!(
            found.expect("a walk carries its find").is_empty(),
            "and turns nothing up"
        );
    }
}

/// A 功法 that reaches 圆满 leaves the books and becomes one the cultivator
/// has learned outright.
#[test]
fn a_mastered_art_leaves_the_books_and_joins_the_learned_ones() {
    let mut cultivation = Cultivation::default();
    let granted = cultivation
        .grant_art(ArtGrade::Yellow, 1_000)
        .expect("a 黄阶 art");
    for offset in 0..40_i64 {
        cultivation.on_session_completed(1_000 + offset, day(30_000 + offset), 25);
    }
    let view = cultivation.view();
    assert!(
        !view.arts.iter().any(|art| art.id == granted.id),
        "the 功法书 is gone: it has been read to the end"
    );
    assert!(
        view.mastered_arts.iter().any(|art| art.id == granted.id),
        "and the 功法 is learned outright"
    );
}

/// Selling a learned 功法 sells the tome, not the knowledge: the stones land
/// once, and everything the 功法 pays stays exactly what it was.
#[test]
fn selling_a_learned_art_takes_nothing_it_pays_with_it() {
    let mut cultivation = Cultivation::default();
    // Art 1 is an 内功, so there is a mastery to lose: the point of the test.
    cultivation.grant_art_book(1).expect("a book for art 1");
    let id = 1;
    // Learn it: enough sessions for any 黄阶 art to reach its last 阶.
    for offset in 0..40_i64 {
        cultivation.on_session_completed(1_000 + offset, day(30_000 + offset), 25);
    }
    let learned_before = cultivation.art_attributes().total();
    assert!(learned_before > 0, "the learned 内功 pays");

    // The tome sells at the market's price for a 功法.
    let purse = cultivation.document().stones;
    let stones = cultivation
        .sell_art(id, 99_000)
        .expect("learned is sellable");
    assert_eq!(
        stones,
        super::shop::ShopOffer::Art { art_id: id }.sell_price()
    );
    assert_eq!(
        cultivation.document().stones,
        purse + stones,
        "the sale paid exactly the market price"
    );
    assert_eq!(
        cultivation.art_attributes().total(),
        learned_before,
        "the mastery it pays is untouched by the sale"
    );
    // The 功法 is still learned, its tome marked sold, and the log says so.
    let view = cultivation.view();
    let learned = view
        .mastered_arts
        .iter()
        .find(|art| art.id == id)
        .expect("the 功法 is still learned");
    assert!(learned.sold, "the tome's sale has been collected");
    let sold = view.events.first().expect("the sale logged");
    assert_eq!(sold.at_unix, 99_000);

    // A tome the market bought once is not for sale again.
    assert_eq!(
        cultivation.sell_art(id, 99_001).expect_err("already sold"),
        TradeRefusal::AlreadySold
    );
    assert_eq!(
        cultivation.document().stones,
        purse + stones,
        "and the second attempt paid nothing"
    );
}

/// The one 功法 that cannot be sold is the one being trained.
#[test]
fn selling_the_book_does_not_take_the_learned_art_away() {
    let mut cultivation = Cultivation::default();
    let granted = cultivation
        .grant_art(ArtGrade::Yellow, 1_000)
        .expect("a 黄阶 art");
    let id = granted.id;
    // Still training it: the sale is refused while the selection reads it.
    assert_eq!(
        cultivation.sell_art(id, 99_000).expect_err("being trained"),
        TradeRefusal::Training
    );
    // The choice to sell a 功法 one has finished with is always open, but the
    // book being trained is not the user's to sell until they stop training it.
    cultivation.select_art(None);
    let stones = cultivation
        .sell_art(id, 99_001)
        .expect("an unselected book is sellable");
    assert!(stones > 0);
}

/// A learned 内功 pays for as long as the save lives, whether or not the book
/// it was learned from is still on the shelf.
#[test]
fn a_learned_internal_art_pays_its_mastery_for_good() {
    let mut cultivation = Cultivation::default();
    let granted = cultivation
        .grant_art(ArtGrade::Yellow, 1_000)
        .expect("a 黄阶 art");
    if blueprint(granted.id).kind != ArtKind::Internal {
        return;
    }
    for offset in 0..40_i64 {
        cultivation.on_session_completed(1_000 + offset, day(30_000 + offset), 25);
    }
    let learned = mastered_art_attributes(&[granted.id]);
    assert!(learned.total() > 0, "a learned 内功 pays its attributes");
    assert_eq!(
        cultivation.art_attributes().total(),
        learned.total(),
        "and the page counts it exactly once"
    );
}

/// A save written before the books and the learned 功法 were split reads as
/// having learned what it had in fact finished.
#[test]
fn an_old_save_moves_its_finished_art_to_the_learned_ones() {
    let parsed: CultivationDocument =
        serde_json::from_str(r#"{"arts":[{"art_id":1,"rank":3,"progress":0}]}"#)
            .expect("the older shape loads");
    let document = parsed.normalized();
    assert!(
        document.arts.is_empty(),
        "the finished 功法 is no longer a book"
    );
    assert_eq!(
        document.mastered_arts,
        vec![1],
        "it is learned outright instead"
    );
}

/// A refused purchase says why, and says it again on the next refusal: the
/// count moves with the reason, so a surface can tell a new refusal from the
/// one still sitting in the snapshot.
#[test]
fn a_refusal_is_recorded_with_a_reason_and_a_count() {
    let mut cultivation = Cultivation::new(CultivationDocument {
        stones: 0,
        ..CultivationDocument::default()
    });
    assert_eq!(cultivation.view().refusal_count, 0);
    let refusal = cultivation
        .buy_from_shop(0, 20_000, 1_000)
        .expect_err("no stones, no goods");
    cultivation.note_refusal(Refusal::Trade(refusal));
    let view = cultivation.view();
    assert_eq!(view.refusal_count, 1);
    assert_eq!(
        view.last_refusal.map(|reason| reason.key()),
        Some("cultivation.shop.refuse_stones"),
        "the page can say which reason it was"
    );
}

/// A walk that is not owed says so rather than doing nothing in silence.
#[test]
fn a_walk_that_is_not_owed_says_why() {
    let mut cultivation = Cultivation::default();
    let _ = cultivation.explore(10_000);
    assert!(
        cultivation.explore(10_001).is_none(),
        "the walk is not owed"
    );
    let view = cultivation.view();
    assert_eq!(view.refusal_count, 1);
    assert_eq!(
        view.last_refusal.map(|reason| reason.key()),
        Some("cultivation.explore.refuse_cooldown"),
        "the page says it is cooling down"
    );
}

/// The market sells the 功法 its shelf names, not another one of its grade.
///
/// A shelf that names a 功法 and hands over a different one is a market that
/// lies about what it stocks, and the price the user paid was for the one on
/// the shelf.
#[test]
fn the_market_sells_the_art_its_shelf_names() {
    let mut cultivation = Cultivation::new(CultivationDocument {
        stones: 100_000,
        ..CultivationDocument::default()
    });
    let shelf = cultivation.shop(20_000);
    let art = shelf
        .iter()
        .find_map(|entry| match entry.offer {
            ShopOffer::Art { art_id } => Some((entry.slot, art_id)),
            _ => None,
        })
        .expect("a shelf stocks a 功法");
    let (slot, art_id) = art;
    cultivation
        .buy_from_shop(slot, 20_000, 1_000)
        .expect("the purchase went through");
    assert!(
        cultivation
            .document()
            .arts
            .iter()
            .any(|art| art.art_id == art_id),
        "the 功法 on the shelf is the one that was handed over"
    );
}

/// A 功法 the cultivator already has is not sold again: a second copy of
/// something they already hold is not a thing the page could say anything true
/// about.
#[test]
fn an_art_the_cultivator_already_has_is_not_sold_again() {
    let mut cultivation = Cultivation::new(CultivationDocument {
        stones: 100_000,
        ..CultivationDocument::default()
    });
    // Find a day whose shelf stocks a particular 功法, and the slot it sits in.
    let (day, slot, art_id) = (20_000..20_500)
        .find_map(|day| {
            crate::cultivation::roll_offers(day, Realm::from_level(1), day as u64)
                .into_iter()
                .enumerate()
                .find_map(|(slot, offer)| match offer {
                    ShopOffer::Art { art_id } => Some((day, slot, art_id)),
                    _ => None,
                })
        })
        .expect("some day stocks a 功法");
    // The cultivator already has that one: a walk or a fight gave it to them.
    assert!(
        cultivation.grant_art_book(art_id).is_some(),
        "the book is in hand"
    );
    let before = cultivation.stones();
    let refusal = cultivation
        .buy_from_shop(slot, day, 1_000)
        .expect_err("the same 功法 is not sold twice");
    assert_eq!(refusal, TradeRefusal::AlreadyOwned, "the refusal says why");
    assert_eq!(cultivation.stones(), before, "and nothing was charged");
}

/// The garden sells its fifth plot for stones, charges double for each one
/// after it, and refuses the tenth: nine is the whole estate.
#[test]
fn the_garden_sells_plots_up_to_nine() {
    let mut cultivation = Cultivation::new(CultivationDocument {
        stones: 10_000,
        ..CultivationDocument::default()
    });
    let mut prices = Vec::new();
    for _ in FREE_PLOT_COUNT..MAX_PLOT_COUNT {
        prices.push(cultivation.buy_plot(1_000).expect("a plot to sell"));
    }
    assert_eq!(
        prices,
        vec![150, 300, 600, 1200, 2400],
        "each plot dearer than the last"
    );
    assert_eq!(
        cultivation.document().plots.len(),
        MAX_PLOT_COUNT,
        "the garden is full"
    );
    assert_eq!(
        cultivation.document().stones,
        10_000 - 4_650,
        "the purse paid for exactly the five plots"
    );
    assert!(
        matches!(cultivation.buy_plot(1_000), Err(TradeRefusal::PlotLimit)),
        "the tenth plot does not exist"
    );
}

/// A buy event carries its price and a seed event its tier — while an old
/// save's events, which carry neither, still read.
#[test]
fn an_old_bought_event_still_reads() {
    use super::events::{EventDetail, EventRecord, SaleKind};

    let old: EventRecord = serde_json::from_str(
        r#"{"at_unix": 10, "detail": {"Bought": {"what": {"Seed": {"name": "白虎茸"}}}}}"#,
    )
    .expect("the old shape parses");
    match old.detail {
        EventDetail::Bought {
            what: SaleKind::Seed { name, tier },
            stones,
        } => {
            assert_eq!(name.as_deref(), Some("白虎茸"));
            assert_eq!((tier, stones), (0, 0), "the old events carry neither");
        }
        other => panic!("the old event read as {other:?}"),
    }
    let new: EventRecord = serde_json::from_str(
        r#"{"at_unix": 10, "detail": {"Bought": {"what": {"Seed": {"name": "白虎茸", "tier": 3}}, "stones": 110}}}"#,
    )
    .expect("the new shape parses");
    match new.detail {
        EventDetail::Bought {
            what: SaleKind::Seed { tier, .. },
            stones,
        } => assert_eq!((tier, stones), (3, 110), "the new event carries both"),
        other => panic!("the new event read as {other:?}"),
    }
}

/// The fight reaches for one of the three strongest 外功, at random — and an
/// untrained book stays on the shelf no matter how deep its grade is.
#[test]
fn the_fight_picks_among_the_strongest_external_arts() {
    let external_of = |grade: ArtGrade| {
        (1..=ART_COUNT)
            .filter(|id| {
                let art = blueprint(*id);
                art.kind == ArtKind::External && art.grade == grade
            })
            .take(3)
            .collect::<Vec<_>>()
    };
    let deep = external_of(ArtGrade::Divine);
    let shallow = external_of(ArtGrade::Yellow);
    assert!(
        deep.len() == 3 && shallow.len() == 3,
        "the five hundred hold enough 外功 of both grades"
    );
    let mut document = CultivationDocument::default();
    for art_id in deep.iter().copied().chain(shallow.iter().copied()) {
        document.arts.push(OwnedArt {
            art_id,
            rank: 1,
            progress: 0,
            outcome: None,
        });
    }
    // A fourth deep book, never trained: owning it is not swinging it.
    document.arts.push(OwnedArt {
        art_id: external_of(ArtGrade::Heaven)[0],
        rank: 0,
        progress: 0,
        outcome: None,
    });
    let cultivation = Cultivation::new(document);
    let mut rng = super::Rng::seeded(0x5EED_5EED);
    for _ in 0..50 {
        let fighter = cultivation.fighter(&mut rng);
        let strike = fighter.art.expect("an 外功 owner swings one");
        assert!(
            deep.iter().any(|id| blueprint(*id).name == strike.name),
            "the strike is one of the three deep arts, not {}",
            strike.name
        );
        // 神阶 under the new curve: 16 + 4 × 12.
        assert_eq!(strike.cost, 64, "the 内力 price is the new curve's");
    }
}

/// The page compares every derived number against the same four attributes
/// spread evenly, so a lopsided build reads as "+X over an even spread" and an
/// even one reads as exactly on it.
#[test]
fn a_lopsided_build_is_measured_against_an_even_spread() {
    let lopsided = Attributes::new(400, 0, 0, 0);
    let breakdowns = lopsided.derived().breakdowns(lopsided);
    let row = |key: &str| {
        breakdowns
            .iter()
            .find(|breakdown| breakdown.key == key)
            .copied()
            .expect("a row")
    };
    let attack = row("cultivation.attack");
    assert_eq!(
        attack.mean_attribute, 100,
        "400 points over four attributes"
    );
    // 攻击力 reads 力量: 10 + 100 × 3 spread evenly, 10 + 0 × 3 poured into 根骨.
    assert_eq!(attack.balanced, 310);
    assert_eq!(attack.result, 10);
    // 内力值 reads 根骨, so the same build is far above its even spread there.
    let inner = row("cultivation.inner");
    assert_eq!(inner.balanced, 50 + 100 * 8);
    assert_eq!(inner.result, 50 + 400 * 8);

    // An even build is on its own reference in every one of the nine numbers.
    let even = Attributes::new(100, 100, 100, 100);
    for breakdown in even.derived().breakdowns(even) {
        assert_eq!(
            breakdown.result, breakdown.balanced,
            "{} is off its own even spread",
            breakdown.key
        );
    }
}

/// An opponent is a body plus the gear it wears plus the 功法 it swings, and the
/// gear is a slice of its own strength rather than a bonus on top of it — the
/// power the panel promises before a breakthrough has to stay the power the
/// fight is worth.
#[test]
fn every_opponent_walks_in_with_gear_and_a_move() {
    for (index, realm) in Realm::ALL.iter().enumerate() {
        let level = realm.cap_level();
        let tier = index as u8 + 1;
        let guardian = super::opponent::opponent(
            super::opponent::Stance::Guardian,
            level,
            tier,
            GUARDIAN_NAME.to_owned(),
            super::opponent::fixed_seed(super::opponent::Stance::Guardian, index as u32),
        );
        // A guardian at a realm's cap has a body big enough to carry a piece, so
        // it always carries one and always swings an art — which is what makes
        // "a win always pays a 功法" still true.
        assert_eq!(
            guardian.gear.len(),
            1,
            "the guardian of {:?} walks in bare-handed",
            realm
        );
        assert!(
            guardian.art.is_some(),
            "the guardian of {:?} swings no art",
            realm
        );
        // What the panel promises is what the body and the gear add up to.
        let budget = base_attributes(level);
        assert_eq!(
            guardian.attributes, budget,
            "the guardian of {:?} is not the rung it guards",
            realm
        );
        assert!(!guardian.fighter.down(), "it has to have health to lose");
    }
}

/// The art is paid for out of the opponent's own hand: a guardian that swings a
/// named move must not be harder than the bare-handed one it replaced, or every
/// breakthrough on the ladder quietly moved one notch out of reach.
#[test]
fn a_named_move_does_not_make_the_fight_harder() {
    for (index, realm) in Realm::ALL.iter().enumerate() {
        let level = realm.cap_level();
        let rival = super::opponent::opponent(
            super::opponent::Stance::Guardian,
            level,
            index as u8 + 1,
            GUARDIAN_NAME.to_owned(),
            super::opponent::fixed_seed(super::opponent::Stance::Guardian, index as u32),
        );
        // The opponent this one replaced: the same rung, the same multipliers,
        // no gear and no art.
        let naked = base_attributes(level).derived();
        let bare = Fighter::from_derived(
            GUARDIAN_NAME.to_owned(),
            Derived {
                health: naked.health * 3 / 2,
                attack: naked.attack * 6 / 5,
                ..naked
            },
            None,
        );
        // The save that got here: the rung plus the free points it was paid.
        let free = i32::from(level) / 4;
        let body = base_attributes(level);
        let player = Fighter::from_attributes(
            PLAYER_NAME.to_owned(),
            Attributes::new(
                body.root + free,
                body.strength + free,
                body.mana + free,
                body.agility + free,
            ),
            None,
        );
        let mut before = 0;
        let mut after = 0;
        for seed in 0..400_u64 {
            if super::duel::duel(player.clone(), bare.clone(), &mut Rng::seeded(seed)).won {
                before += 1;
            }
            if super::duel::duel(
                player.clone(),
                rival.fighter.clone(),
                &mut Rng::seeded(seed),
            )
            .won
            {
                after += 1;
            }
        }
        // A band rather than an equality: the art front-loads its damage, and a
        // burst is worth a little more than the same damage spread out. Ten
        // points of four hundred is the room that leaves.
        let delta = i64::from(after) - i64::from(before);
        assert!(
            delta.abs() <= 40,
            "{realm:?}: the guardian's art moved the win rate from {before} to {after} of 400"
        );
    }
}
