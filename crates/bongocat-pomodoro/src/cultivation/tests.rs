//! The ladder's rules, exercised without a clock: the tests hand in every
//! instant and every local day, so a hundred days of absence take microseconds.

use super::garden::{grow_seconds, harvest_points};
use super::*;
use crate::CalendarDay;

/// The growth that fills the first realm: the last layer of 炼气.
const FIRST_REALM_CAP: u32 = 1_188;

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
        };
        document.equipment.push(item);
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
    assert_eq!(view.layer, 1);
    assert_eq!(view.growth, 0);
    assert_eq!(view.next, Some(threshold_for(2)));
    assert!(view.inventory.is_empty());
    assert_eq!(view.plots.len(), PLOT_COUNT);
    assert!(view.guardian.is_none());
}

#[test]
fn a_session_pays_growth_in_proportion_to_its_length() {
    assert_eq!(growth_for_session(25), GROWTH_PER_SESSION);
    assert_eq!(growth_for_session(50), GROWTH_PER_SESSION * 2);
    assert_eq!(growth_for_session(1), 1, "a short session still pays");
    let mut cultivation = Cultivation::default();
    let reward = cultivation.on_session_completed(1_000, day(20_000), 25);
    assert_eq!(reward.growth, GROWTH_PER_SESSION);
    assert_eq!(cultivation.document().growth, GROWTH_PER_SESSION);
    assert_eq!(cultivation.document().last_active_day, Some(20_000));
}

#[test]
fn a_missed_day_costs_growth_and_every_following_one_doubles() {
    let mut cultivation = Cultivation::new(CultivationDocument {
        growth: 1_000,
        last_active_day: Some(20_000),
        charged_through_day: Some(20_000),
        ..CultivationDocument::default()
    });
    // The first day without a session: 20_001 has ended.
    let first = cultivation.apply_absence(day(20_002)).expect("a charge");
    assert_eq!(first.days, 1);
    assert_eq!(first.charged, ABSENCE_PENALTY);
    // Two days later, both days missed: the second one doubles.
    let second = cultivation.apply_absence(day(20_004)).expect("a charge");
    assert_eq!(second.days, 2);
    assert_eq!(second.charged, ABSENCE_PENALTY * 2 + ABSENCE_PENALTY * 4);
    assert_eq!(
        cultivation.document().growth,
        1_000 - ABSENCE_PENALTY - (ABSENCE_PENALTY * 2 + ABSENCE_PENALTY * 4)
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
    assert_eq!(cultivation.document().growth, 990);
}

#[test]
fn a_penalty_never_takes_back_the_realm_already_entered() {
    let mut cultivation = Cultivation::new(CultivationDocument {
        growth: 1_200,
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
        1_200,
        "the floor of the entered realm holds"
    );
    assert_eq!(charge.charged, 0, "nothing was taken");
    assert_eq!(cultivation.realm(), Realm::Foundation);
}

#[test]
fn the_guardian_is_beyond_a_naked_cultivator_at_the_cap() {
    let naked = at_the_first_cap();
    let guardian = naked.guardian().expect("a guardian at the cap");
    assert!(guardian.power > naked.power(), "gear must be required");
    let outcome = naked.clone().breakthrough().expect("an attempt");
    assert!(!outcome.report.won);
    assert_eq!(outcome.target, Realm::Foundation);
    assert!(outcome.reward.is_none());
    assert_eq!(naked.document().realm, 0, "a loss opens nothing");
}

#[test]
fn gear_is_what_gets_a_cultivator_through() {
    let mut cultivation = geared(at_the_first_cap());
    assert!(cultivation.power() > cultivation.guardian().expect("a guardian").power);
    let outcome = cultivation.breakthrough().expect("an attempt");
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
    let outcome = cultivation.breakthrough().expect("an attempt");
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
    assert!(cultivation.breakthrough().is_none());
}

#[test]
fn growth_beyond_the_cap_is_kept_but_buys_no_layers() {
    let cultivation = Cultivation::new(CultivationDocument {
        growth: 5_000,
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

#[test]
fn a_seed_stack_is_spent_one_planting_at_a_time() {
    let mut cultivation = Cultivation::default();
    document_seeds(&mut cultivation, Attribute::Spirit, 1, 2);
    assert!(cultivation.plant(Attribute::Spirit, 1, 0));
    assert!(cultivation.plant(Attribute::Spirit, 1, 0));
    assert!(
        !cultivation.plant(Attribute::Spirit, 1, 0),
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
        };
        document.equipment.push(item);
    }
    document.next_item_id = 3;
    cultivation = Cultivation::new(document);
    assert!(cultivation.equip(1));
    assert_eq!(cultivation.view().equipped[0].map(|item| item.id), Some(1));
    assert!(cultivation.equip(2));
    assert_eq!(
        cultivation.view().equipped[0].map(|item| item.id),
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
    let outcome = cultivation.breakthrough().expect("an attempt");
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

    // Two default sessions reach the second layer; the first one does not.
    cultivation.on_session_completed(1_000, day(20_000), 25);
    assert_eq!(cultivation.level(), 1);
    assert_eq!(cultivation.free_points(), 0);
    cultivation.on_session_completed(2_000, day(20_000), 25);
    assert_eq!(cultivation.level(), 2);
    assert_eq!(cultivation.free_points(), FREE_POINTS_PER_LEVEL);

    let before = cultivation.attributes().strength;
    assert!(cultivation.allocate(Attribute::Strength));
    assert_eq!(
        cultivation.attributes().strength,
        before + 1,
        "the placed point is part of the attribute total"
    );
    assert_eq!(cultivation.free_points(), 0);
    assert_eq!(cultivation.view().allocated_attributes.strength, 1);
    assert!(!cultivation.allocate(Attribute::Root), "the point is spent");
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
    assert_eq!(cultivation.level(), 100, "the cap holds the level down");
    assert_eq!(
        cultivation.granted_points(),
        99 * FREE_POINTS_PER_LEVEL,
        "the peak is the highest level, not the sum of every session"
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
        document_seeds(&mut cultivation, Attribute::Spirit, 2, 1);
        assert!(cultivation.plant(Attribute::Spirit, 2, planted_at));
        let outcome = cultivation
            .harvest(0, planted_at + grow_seconds(2))
            .expect("a ripe crop");
        if let Some(seed) = outcome.seed {
            returned = Some(seed);
            break;
        }
    }
    let (attribute, tier) = returned.expect("200 harvests at 15% return at least one seed");
    assert_eq!(attribute, Attribute::Spirit);
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
