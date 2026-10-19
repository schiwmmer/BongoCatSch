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
    assert_eq!(view.plots.len(), PLOT_COUNT);
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
    assert_eq!(swift.hit, 800);
    assert_eq!(swift.evade, 90);

    let rooted = Attributes::new(10, 0, 0, 0).derived();
    assert_eq!(rooted.inner, 130, "50 + 根骨×8");
    assert_eq!(rooted.aptitude, 20);
}

/// Both chances are capped: an uncapped 闪避 would make a build untouchable, and
/// an uncapped 命中 would make every fight a formality.
#[test]
fn hit_and_evade_stop_climbing() {
    let absurd = Attributes::new(0, 0, 0, 10_000).derived();
    assert_eq!(absurd.hit, 950);
    assert_eq!(absurd.evade, 400);
    assert_eq!(absurd.hit_percent(), 95);
    assert_eq!(absurd.evade_percent(), 40);
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
    let art = view
        .arts
        .iter()
        .find(|art| art.id == granted.id)
        .expect("the art is still owned");
    assert_eq!(art.rank, art.max_rank, "enough sessions master a 黄阶 art");
    assert_eq!(art.progress, 0);
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
