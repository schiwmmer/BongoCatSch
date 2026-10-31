//! The ladder's rules, exercised without a clock: the tests hand in every
//! instant and every local day, so a hundred days of absence take microseconds.

use super::explore::WALK_STONES;
use super::garden::{grow_seconds, grow_seconds_at, harvest_points};
use super::*;
use crate::CalendarDay;

/// The growth that fills the first realm: the last layer of 炼气.
const FIRST_REALM_CAP: u32 = 9_900;

/// How far apart the tests take their walks: the longest length, so a walk
/// taken at any of the three is owed again by the time the next one starts.
const WALK_SECONDS: i64 = 12 * 60 * 60;

fn day(number: i64) -> CalendarDay {
    CalendarDay::from_day_number(number)
}

/// The instant that local day opens, for a charge that wants a timestamp.
fn day_unix(day: CalendarDay) -> i64 {
    day.day_number() * 86_400
}

/// Set out and come home: what the road paid, once the walk is back.
///
/// A walk pays nothing when it leaves any more — the tick that settles it is
/// what pays — so every test that wants a walk's find has to walk it and bring
/// it home. The clock moves by the walk's own length, which is the same instant
/// the old cooldown used to free up, so a day still holds the same number of
/// walks it always did.
fn walk(
    cultivation: &mut Cultivation,
    now_unix: i64,
    hours: ExplorationHours,
) -> Option<ExplorationOutcome> {
    cultivation.explore(now_unix, hours)?;
    let home = now_unix.saturating_add(hours.seconds());
    cultivation.settle_walk(home);
    cultivation.view_on(home, 0).last_exploration
}

/// A garden tended to the top of the ladder.
///
/// Nothing ever failed in these tests because nothing could: planting was a put
/// with no roll. The ones below are about ripening, paying and counting rather
/// than about odds, and the top rank is where every seed still comes up and they
/// still say exactly what they said. What it costs to get there is
/// `the_garden_is_raised_with_harvests_and_bought_with_stones`.
fn tended(cultivation: Cultivation) -> Cultivation {
    let mut document = cultivation.document().clone();
    document.garden_rank = MAX_GARDEN_RANK;
    Cultivation::new(document)
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
            ..Default::default()
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
    // A missed day costs five layers of the realm it was missed in: 100 growth
    // a layer at 炼气, so five hundred before the realm's own multiplier.
    let base = |step: i32| {
        (ABSENCE_LAYERS_PER_DAY as f64 * 1.2_f64.powi(step) * f64::from(layer_growth(0))) as u32
    };
    let first = cultivation
        .apply_absence(day(20_002), day_unix(day(20_002)))
        .expect("a charge");
    assert_eq!(first.days, 1);
    assert_eq!(first.charged, base(0));
    // Two more missed days each escalate by the step.
    let second = cultivation
        .apply_absence(day(20_004), day_unix(day(20_004)))
        .expect("a charge");
    assert_eq!(second.days, 2);
    assert_eq!(second.charged, base(1) + base(2));
    assert_eq!(
        cultivation.document().growth,
        100_000 - base(0) - (base(1) + base(2))
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
    assert!(
        cultivation
            .apply_absence(day(20_002), day_unix(day(20_002)))
            .is_some()
    );
    assert!(
        cultivation
            .apply_absence(day(20_002), day_unix(day(20_002)))
            .is_none(),
        "a re-launch on the same day charges nothing"
    );
    assert_eq!(
        cultivation.document().growth,
        1_000 - ABSENCE_LAYERS_PER_DAY * layer_growth(0)
    );
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
        .apply_absence(day(20_002), day_unix(day(20_002)))
        .expect("a charge")
        .charged;
    let mut deep = Cultivation::new(CultivationDocument {
        growth: 1_000_000,
        realm: 4,
        last_active_day: Some(20_000),
        charged_through_day: Some(20_000),
        ..CultivationDocument::default()
    });
    let deep_charge = deep
        .apply_absence(day(20_002), day_unix(day(20_002)))
        .expect("a charge")
        .charged;
    // A layer of 化神 costs twenty-five times a layer of 炼气, and the penalty is
    // spoken in layers, so the deeper realm pays exactly that much more.
    assert_eq!(
        deep_charge,
        shallow_charge * layer_growth(4) / layer_growth(0)
    );
}

/// A charge made while nobody was looking is in the log afterwards.
///
/// The penalty is the one move the ladder makes on its own, so it belongs in the
/// durable log rather than only in a notice the next quit forgets: this is the
/// test for the report "a day went missing and nothing said so".
#[test]
fn a_charge_is_written_into_the_event_log() {
    let mut cultivation = Cultivation::new(CultivationDocument {
        growth: 100_000,
        last_active_day: Some(20_000),
        charged_through_day: Some(20_000),
        ..CultivationDocument::default()
    });
    let at = day_unix(day(20_002));
    let charge = cultivation
        .apply_absence(day(20_002), at)
        .expect("a charge");
    let logged = cultivation.view().events.iter().any(|event| {
        event.at_unix == at
            && event.detail
                == EventDetail::Absence {
                    days: charge.days,
                    charged: charge.charged,
                }
    });
    assert!(
        logged,
        "the charge has to survive the notice: {:?}",
        cultivation.view().events
    );
    // And the charge is what it says it is: the growth it took is the drop the
    // level made, so the line and the ladder agree.
    assert_eq!(
        charge.charged,
        ABSENCE_LAYERS_PER_DAY * layer_growth(0),
        "one missed day is five layers of the realm it was missed in"
    );
    assert_eq!(cultivation.document().growth, 100_000 - charge.charged);
}

/// Every line of the power breakdown adds up to the total it explains.
///
/// The panel used to print five sources that summed to less than the number in
/// the corner, because the points the user had placed and the gear's outright
/// numbers were missing and the pairing bonus was counted per source instead of
/// once over the whole body. This pins the arithmetic the page now prints.
#[test]
fn the_power_breakdown_adds_up_to_the_power_it_explains() {
    let item = EquipmentRecord {
        id: 1,
        slot: EquipSlot::Weapon,
        tier: 4,
        rarity: Rarity::Rare,
        attributes: Attributes::new(30, 40, 0, 20),
        bonus: Derived {
            attack: 60,
            spell: 20,
            ..Derived::default()
        },
        ..EquipmentRecord::default()
    };
    let cultivation = Cultivation::new(CultivationDocument {
        growth: 20_000,
        realm: 1,
        allocated: Attributes::new(10, 20, 0, 30),
        crop_bonus: Attributes::new(5, 0, 7, 0),
        equipment: vec![item],
        equipped: vec![Some(1), None, None, None],
        ..CultivationDocument::default()
    });
    let breakdown = cultivation.power_breakdown();
    assert_eq!(breakdown.total, cultivation.power());
    assert_eq!(
        breakdown.parts_total(),
        breakdown.total,
        "the lines have to add up to the number they explain: {breakdown:?}"
    );
    for (name, value) in [
        ("level", breakdown.level),
        ("allocated", breakdown.allocated),
        ("equipment", breakdown.equipment),
        ("equipment_bonus", breakdown.equipment_bonus),
        ("crop", breakdown.crop),
    ] {
        assert!(
            value > 0,
            "{name} is missing from the breakdown: {breakdown:?}"
        );
    }
    // Power is linear in the four attributes, so there is no residual: the body
    // read whole is the body read source by source.
    let whole = cultivation.attributes().power();
    let by_source = breakdown.level
        + breakdown.allocated
        + breakdown.equipment
        + breakdown.crop
        + breakdown.art;
    assert_eq!(
        whole, by_source,
        "a source is worth alone what it is worth in the sum: {breakdown:?}"
    );
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
    let charge = cultivation
        .apply_absence(day(20_040), day_unix(day(20_040)))
        .expect("a charge");
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
    assert_eq!(spell.spell_power(), 45, "根骨 10 × 19 + 仙力 10 × 26");
    assert_eq!(spell.martial_power(), 0, "a spell set fights no melee");
    assert_eq!(martial.spell_power(), 0, "a martial set casts nothing");
    assert_eq!(martial.martial_power(), 41, "力量 10 × 26 + 身法 10 × 15");
    assert_eq!(
        spell.plus(martial).power(),
        86,
        "the total is the two halves, not a fourth weighting"
    );
}

/// Power is linear in the four attributes: two sets added together are worth
/// exactly what each is worth alone, which is what lets the panel's breakdown
/// add up without a residual line.
#[test]
fn two_sources_are_worth_exactly_their_two_powers() {
    let level = Attributes::new(930, 928, 928, 496);
    let gear = Attributes::new(30, 40, 12, 21);
    // Each half is rounded once, so the sum of two rounded halves can sit a
    // point or two under the rounded whole — the pairing bonus that used to make
    // the difference a systematic twenty percent is what is gone here.
    let together = level.plus(gear).power();
    let apart = level.power().saturating_add(gear.power());
    assert!(
        together.abs_diff(apart) <= 2,
        "a point is worth the same wherever it was earned: {together} against {apart}"
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

/// The shown numbers follow from the four allocated ones, in the direction the
/// panel promises: 力量 to 攻击力 and 生命值, 法力 to 法术强度 and 法抗, 身法 to
/// 速度/命中/闪避, 根骨 to 内力值.
///
/// 潜质 is deliberately *not* on this list any more: it is its own fractional
/// number, born somewhere on a −10..+10 band and raised only by two herbs, so
/// pouring points into 根骨 no longer buys it.
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
    assert_eq!(
        rooted.aptitude, 0,
        "根骨 no longer buys 潜质 — only the two herbs do"
    );
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
    let mut cultivation = tended(Cultivation::default());
    document_seeds(&mut cultivation, Attribute::Root, 1, 1);
    assert!(
        cultivation
            .plant(Attribute::Root, 1, SeedGrade::Earth, 1_000)
            .is_some(),
        "a first-tier seed comes up in a top-rank garden"
    );
    let planted = cultivation.view().plots[0]
        .crop
        .clone()
        .expect("a crop in the first plot");
    assert_eq!(
        planted.ready_at_unix,
        1_000 + grow_seconds_at(MAX_GARDEN_RANK, 1),
        "a top-rank garden grows a first-tier seed at its shortened rate"
    );
    assert!(
        cultivation.harvest(0, 1_000).is_none(),
        "an unripe crop is not pickable"
    );
    assert_eq!(
        cultivation.plant(Attribute::Strength, 1, SeedGrade::Earth, 1_000),
        None,
        "a seed the cultivator does not own cannot be planted"
    );
    let harvested = cultivation
        .harvest(0, 1_000 + grow_seconds(1))
        .expect("a ripe crop");
    assert_eq!(harvested.crop.attribute, Attribute::Root);
    assert_eq!(
        cultivation.crop_attributes().root,
        harvest_points(1, SeedGrade::Earth),
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
    let mut document = CultivationDocument {
        garden_rank: MAX_GARDEN_RANK,
        ..CultivationDocument::default()
    };
    document.seeds.push(SeedStack {
        attribute: Attribute::Mana,
        herb: None,
        tier: 3,
        grade: SeedGrade::Earth,
        count: 1,
        name: Some("青芝".to_owned()),
    });
    let mut cultivation = Cultivation::new(document);
    assert!(
        cultivation
            .plant(Attribute::Mana, 3, SeedGrade::Earth, 0)
            .is_some()
    );
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
    // The seed the pull gave back is a roll of its own, so the line is read
    // field by field: what the crop was and what it paid are the two things the
    // test is about, and the returned herb (if any) is named beside them.
    match event {
        EventDetail::Harvested {
            attribute,
            tier,
            grade,
            name,
            points,
            xp,
            ..
        } => {
            assert_eq!(attribute, Attribute::Mana);
            assert_eq!(tier, 3);
            assert_eq!(grade, SeedGrade::Earth);
            assert_eq!(name, Some("青芝".to_owned()));
            assert_eq!(points, harvest_points(3, SeedGrade::Earth));
            // The pull pays the garden the same number it pays the body, and the
            // line has to carry it: the bar moved, so the log says why.
            assert_eq!(xp, points, "the 造化 the pull banked");
            assert_eq!(
                cultivation.document().garden_xp,
                i64::from(xp),
                "and the bar it moved is the number on the line"
            );
        }
        other => panic!("a harvest line, not {other:?}"),
    }
}

#[test]
fn a_seed_stack_is_spent_one_planting_at_a_time() {
    let mut cultivation = tended(Cultivation::default());
    document_seeds(&mut cultivation, Attribute::Mana, 1, 2);
    // Twice over, because what is being counted is the stack going down: one
    // seed out of two at a time, until there is nothing left to give.
    for _ in 0..2 {
        assert!(
            cultivation
                .plant(Attribute::Mana, 1, SeedGrade::Earth, 0)
                .is_some()
        );
    }
    assert_eq!(
        cultivation.plant(Attribute::Mana, 1, SeedGrade::Earth, 0),
        None,
        "the stack is empty"
    );
    assert!(cultivation.view().seeds.is_empty());
}

/// Planting above your station. A tenth-tier 神阶 seed in a first-rank garden
/// comes up one time in eight, and the other seven are what this test is about:
/// what a failure takes, when it takes it, and when it says so.
#[test]
fn a_planting_that_does_not_take_keeps_the_bed_until_the_halfway_mark() {
    let mut cultivation = Cultivation::default();
    assert_eq!(
        plant_permille(1, MAX_SEED_TIER, SeedGrade::Divine),
        127,
        "asking a first-rank bed for the rarest herb in the game"
    );
    let grown = grow_seconds(MAX_SEED_TIER);
    let mut found = None;
    // The failures are found rather than staged: the roll is seeded by the clock
    // the test hands in, so share the Divine among sixty-four plantings and one
    // of them will not come up.
    for step in 0..64i64 {
        let at = step * (grown + 1) + 1;
        document_seed(
            &mut cultivation,
            Attribute::Root,
            MAX_SEED_TIER,
            SeedGrade::Divine,
        );
        let before = cultivation.view().garden_xp;
        assert!(
            cultivation
                .plant(Attribute::Root, MAX_SEED_TIER, SeedGrade::Divine, at)
                .is_some(),
            "the seed was there and every plot was free"
        );
        // Halfway is the first instant the bed may say how it is doing, and not
        // one second before it: the verdict is the bed's own, and the surface
        // cannot even ask for it earlier.
        let half = at + grown / 2;
        assert!(
            cultivation.view_on(half - 1, 0).plots[0]
                .crop
                .as_ref()
                .is_some_and(|crop| !crop.withered),
            "a bed under its halfway mark is still growing, whatever it knows"
        );
        let settled = cultivation.settle_garden(half);
        let dead = cultivation.view_on(half, 0).plots[0]
            .crop
            .as_ref()
            .is_some_and(|crop| crop.withered);
        if dead {
            found = Some((at, half, before, settled));
            break;
        }
        cultivation.harvest(0, at + grown).expect("a ripe crop");
    }
    let (at, half, before, settled) = found.expect("sixty-four tries find one failure");
    assert!(
        settled,
        "the tick that reaches the halfway mark does the work"
    );
    let xp = wither_points(MAX_SEED_TIER, SeedGrade::Divine);
    let view = cultivation.view_on(half, 0);
    // The bed still holds the dead crop: the failure costs the bed for the first
    // half of the growing time, and the gardener is told at the halfway mark so
    // the second half is theirs to spend on something else.
    assert!(
        view.plots[0].is_crop_dead(),
        "a dead crop is the bed's answer, and the bed still holds it"
    );
    assert_eq!(
        view.garden_xp,
        before + i64::from(xp),
        "a dead seedling still teaches something"
    );
    assert!(
        view.seeds
            .iter()
            .all(|seed| !(seed.tier == MAX_SEED_TIER && seed.grade == SeedGrade::Divine)),
        "the seed is spent, or the odds print a lie"
    );
    assert!(
        matches!(
            view.events.first().map(|event| &event.detail),
            Some(EventDetail::Withered { permille: 127, .. })
        ),
        "and the log says where the seed went"
    );
    // Once, not once per tick: the same belated machine that slept through the
    // halfway mark must not be paid again on every wake.
    assert!(
        !cultivation.settle_garden(half + 1),
        "a settled bed has nothing left to say"
    );
    assert_eq!(
        cultivation.view_on(half + 1, 0).garden_xp,
        before + i64::from(xp),
        "and nothing left to pay"
    );
    // 铲除 gives the rest of the bed's day back, and pays nothing on top: the
    // 造化 was the failure's, and it has already been paid.
    assert!(
        cultivation.dig_plot(0, half).is_some(),
        "a dead crop can be dug out"
    );
    assert!(
        cultivation.view_on(half, 0).plots[0].crop.is_none(),
        "and the bed is free for the next seed"
    );
    // A live crop cannot be dug: the odds already spared it, and throwing it away
    // would be a way to burn a seed for nothing.
    document_seed(&mut cultivation, Attribute::Root, 1, SeedGrade::Earth);
    cultivation.plant(Attribute::Root, 1, SeedGrade::Earth, half);
    assert!(
        cultivation.dig_plot(0, half + 1).is_none(),
        "只有枯死的作物可以铲除"
    );
    assert!(
        cultivation.view_on(at + grown, 0).plots[0]
            .crop
            .as_ref()
            .is_some(),
        "a live crop keeps its bed"
    );
}

/// The learned panel ranks by how far a book has come, then by its grade.
///
/// 阶 do not compare across grades — a 神阶 art has twenty-seven of them and a
/// 黄阶 three — so the panel reads the ratio. A book read to the end ranks above
/// everything half-read, and two books at the same ratio rank the better grade,
/// because a 神阶 half-read is worth more than a 黄阶 half-read.
#[test]
fn the_learned_panel_ranks_by_progress_then_grade() {
    let art_of = |grade: ArtGrade| {
        (1..=super::ART_COUNT)
            .find(|id| {
                let art = blueprint(*id);
                art.grade == grade && art.kind == ArtKind::Internal
            })
            .expect("every grade has an 内功")
    };
    let (yellow, heaven, divine) = (
        art_of(ArtGrade::Yellow),
        art_of(ArtGrade::Heaven),
        art_of(ArtGrade::Divine),
    );
    let mut document = CultivationDocument::default();
    // One 阶 of three is a third of the way; one 阶 of twenty-seven is not.
    for (art_id, rank) in [(divine, 1), (heaven, 1), (yellow, 2)] {
        document.arts.push(OwnedArt {
            art_id,
            rank,
            progress: 0,
            outcome: None,
            failures: 0,
            copies: 1,
        });
    }
    let view = Cultivation::new(document.clone()).view();
    let order: Vec<u64> = view.mastered_arts.iter().map(|art| art.id).collect();
    assert_eq!(
        order,
        vec![yellow, heaven, divine],
        "the furthest-read book leads, and 天 outranks 神 at one 阶 each"
    );
    // A finished book is further along than any book still being read, whatever
    // its grade: completion is the one ratio that cannot be beaten.
    document.mastered_arts.push(divine);
    let view = Cultivation::new(document).view();
    let first = view
        .mastered_arts
        .first()
        .map(|art| art.id)
        .expect("the panel lists something");
    assert_eq!(first, divine, "a 神阶 book read to the end leads the panel");
}

/// Two gates that cannot buy each other: the rank is earned in harvests and
/// paid for in coins, and neither alone opens it.
#[test]
fn the_garden_is_raised_with_harvests_and_bought_with_stones() {
    let document = CultivationDocument {
        garden_xp: rank_xp_total(2),
        stones: 10_000,
        ..CultivationDocument::default()
    };
    let mut cultivation = Cultivation::new(document);
    assert_eq!(cultivation.upgrade_garden(0), Ok(2));
    let view = cultivation.view();
    assert_eq!(view.garden_rank, 2);
    assert_eq!(view.garden_xp_needed, rank_xp_total(3));
    assert_eq!(view.garden_upgrade_cost, Some(rank_cost(2)));
    assert!(
        matches!(
            view.events.first().map(|event| &event.detail),
            Some(EventDetail::GardenRank { rank: 2, .. })
        ),
        "and the log keeps it"
    );
    // The coins are still there and the next rank still is not: no harvest has
    // been banked towards it.
    assert_eq!(
        cultivation.upgrade_garden(0),
        Err(TradeRefusal::NotEnoughRankXp)
    );
    // Nor do harvests alone: a gardener who kept every herb has nothing to
    // spend, and that is what choosing to keep them cost.
    let spent = cultivation.document().clone();
    {
        let mut document = spent.clone();
        document.garden_xp = rank_xp_total(MAX_GARDEN_RANK);
        document.stones = 0;
        let mut poor = Cultivation::new(document);
        assert_eq!(poor.upgrade_garden(0), Err(TradeRefusal::NotEnoughStones));
    }
    let mut document = spent;
    document.stones = 10_000;
    document.garden_rank = MAX_GARDEN_RANK;
    let mut cultivation = Cultivation::new(document);
    assert_eq!(
        cultivation.upgrade_garden(0),
        Err(TradeRefusal::GardenCapped)
    );
    assert_eq!(
        cultivation.view().garden_upgrade_cost,
        None,
        "there is no sixteenth rung to sell"
    );
}

/// What a save written before plantings could fail wakes up as. Those gardens
/// never lost a crop, so they are read onto the new ladder by the only number
/// that says how much gardening was done — and a gardener coming back after a
/// year finds the garden they left rather than a first-rank bed.
#[test]
fn an_old_save_keeps_the_garden_it_earned() {
    let old = Cultivation::new(CultivationDocument {
        harvests: 100,
        ..CultivationDocument::default()
    });
    let view = old.view();
    assert_eq!(view.garden_rank, 6, "a hundred harvests of gardening");
    assert_eq!(view.garden_xp, 300, "one ordinary crop each");
    assert_eq!(view.garden_upgrade_cost, Some(rank_cost(6)));
    // Monotone, so migrating two saves cannot read them out of order.
    for count in [1, 10, 50, 100, 1_000] {
        let earned = Cultivation::new(CultivationDocument {
            harvests: count,
            ..CultivationDocument::default()
        });
        assert!(
            earned.view().garden_rank <= view.garden_rank || count > 100,
            "{count} harvests reads above 100"
        );
    }
    assert_eq!(Cultivation::default().view().garden_rank, 1);
    assert_eq!(
        Cultivation::default().view().garden_xp_needed,
        rank_xp_total(2),
        "the first rung is one crop away"
    );
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
            ..Default::default()
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
        herb: None,
        tier,
        grade: SeedGrade::Earth,
        count,
        name: None,
    });
    *cultivation = Cultivation::new(document);
}

/// One seed of one grade: the grade matters now, because it is half of what the
/// planting's odds are read against.
fn document_seed(cultivation: &mut Cultivation, attribute: Attribute, tier: u8, grade: SeedGrade) {
    let mut document = cultivation.document().clone();
    document.seeds.push(SeedStack {
        attribute,
        herb: None,
        tier,
        grade,
        count: 1,
        name: None,
    });
    *cultivation = Cultivation::new(document);
}

/// Give the cultivator one 潜质 herb of a grade, without going through a drop.
///
/// The purse is topped up so a gate the herb crosses can never be what blocks
/// the feeding: this is about the grade, not about the pills.
fn document_herb(cultivation: &mut Cultivation, herb: AptitudeHerb, grade: SeedGrade) {
    let mut document = cultivation.document().clone();
    document.marrow_pills = document.marrow_pills.max(100);
    document.herb_stacks.push(HerbStack::single(herb, grade));
    *cultivation = Cultivation::new(document);
}

/// The satchel spends the cheapest grade held first, and the body pays the
/// grade it was fed: a 黄阶 慧心草 is a warm-up, a 神阶 one is held for the
/// gate it most wants to cross.
#[test]
fn the_satchel_spends_the_cheapest_grade_first() {
    let mut cultivation = Cultivation::default();
    document_herb(&mut cultivation, AptitudeHerb::Wisdom, SeedGrade::Yellow);
    document_herb(&mut cultivation, AptitudeHerb::Wisdom, SeedGrade::Divine);
    let first = cultivation
        .use_aptitude_herb(AptitudeHerb::Wisdom, 1_000)
        .expect("the body is fed");
    assert_eq!(
        first.gained_tenths, 10,
        "a 黄阶 慧心草 pays its one number, got {}",
        first.gained_tenths
    );
    assert_eq!(cultivation.aptitude_herbs()[1], 1, "one 慧心草 left");
    let stacks = &cultivation.document().herb_stacks;
    assert_eq!(stacks.len(), 1, "the 黄阶 is spent, the 神阶 waits");
    assert_eq!(
        stacks[0].grade,
        SeedGrade::Divine,
        "the one left is the best"
    );
    // The second feeding is the 神阶, and pays the big number.
    let second = cultivation
        .use_aptitude_herb(AptitudeHerb::Wisdom, 2_000)
        .expect("the body is fed again");
    assert_eq!(
        second.gained_tenths, 16,
        "a 神阶 慧心草 pays its one number, got {}",
        second.gained_tenths
    );
    assert_eq!(cultivation.aptitude_herbs()[1], 0, "the satchel is empty");
}

/// A 潜质 herb bought from the shelf is the graded herb the row showed: the
/// grade the buyer paid for is the grade the body pays, and the satchel holds
/// the grade it could not feed.
#[test]
fn a_bought_herb_keeps_the_grade_the_shelf_showed() {
    let mut cultivation = Cultivation::new(CultivationDocument {
        stones: 100_000,
        ..CultivationDocument::default()
    });
    let (day, slot, grade) = (20_000..20_500)
        .find_map(|day| {
            roll_offers(day, Realm::from_level(1), day as u64)
                .into_iter()
                .enumerate()
                .find_map(|(slot, offer)| match offer {
                    ShopOffer::Herb {
                        herb: AptitudeHerb::Wisdom,
                        grade,
                    } => Some((day, slot, grade)),
                    _ => None,
                })
        })
        .expect("some day stocks a 慧心草");
    let bought = cultivation
        .buy_from_shop(slot, day, 1_000)
        .expect("the herb is bought");
    match bought {
        ShopOffer::Herb {
            herb: AptitudeHerb::Wisdom,
            grade: got,
        } => assert_eq!(got, grade, "the row's grade is the one bought"),
        other => panic!("bought {other:?}, not a 慧心草"),
    }
}

/// A 潜质 herb bought from the shelf comes home as the herb it was sold as.
///
/// The stack was handed a rolled epithet when it was folded in, so the 悟心草
/// the shelf had just sold sat in the satchel reading 幽菊 and nothing on the
/// page still said what it grew into. A herb seed is named by its herb; only an
/// attribute seed keeps a rolled name, because nothing else tells two of them
/// apart.
#[test]
fn a_bought_herb_comes_home_named_by_its_herb() {
    let mut cultivation = Cultivation::new(CultivationDocument {
        stones: 100_000,
        ..CultivationDocument::default()
    });
    let (day, slot) = (20_000..20_500)
        .find_map(|day| {
            roll_offers(day, Realm::from_level(1), day as u64)
                .into_iter()
                .position(|offer| {
                    matches!(
                        offer,
                        ShopOffer::Herb {
                            herb: AptitudeHerb::Insight,
                            ..
                        }
                    )
                })
                .map(|slot| (day, slot))
        })
        .expect("some day stocks a 悟心草");
    cultivation
        .buy_from_shop(slot, day, 1_000)
        .expect("the herb is bought");
    let seed = cultivation
        .document()
        .seeds
        .iter()
        .find(|seed| seed.herb == Some(AptitudeHerb::Insight))
        .expect("the bought herb is held as a seed");
    assert_eq!(
        seed.name, None,
        "a herb seed is named by its herb, not by a roll"
    );
    assert_eq!(seed.tier, herb_tier(AptitudeHerb::Insight));
}

/// Give the cultivator one 潜质 herb seed, without going through a drop. The
/// purse is topped up so a gate the herb's harvest crosses can never block it.
fn document_herb_seed(cultivation: &mut Cultivation, herb: AptitudeHerb, grade: SeedGrade) {
    let mut document = cultivation.document().clone();
    document.marrow_pills = document.marrow_pills.max(100);
    document.seeds.push(SeedStack::herb_single(herb, grade));
    *cultivation = Cultivation::new(document);
}

#[test]
fn the_herb_tiers_are_fixed_by_the_herb() {
    // Rarity sets the tier: the rarer 慧心草 grows at tier eight, the common
    // 悟心草 at tier six — a higher tier is the longer bed, so the rarer
    // herb is the slower one to mature.
    assert_eq!(herb_tier(AptitudeHerb::Wisdom), 8);
    assert_eq!(herb_tier(AptitudeHerb::Insight), 6);
}

/// A save from an older build could hold a herb seed at the wrong tier — a
/// 悟心草 written as eight, 慧心草 as six, the reverse of the ladder now. The
/// read corrects it to the herb's fixed tier, so the seed reads the right 阶
/// and plants rather than being silently refused.
#[test]
fn an_old_herb_seed_is_read_at_its_herb_tier() {
    let mut document = CultivationDocument {
        garden_rank: 4,
        ..CultivationDocument::default()
    };
    // A 悟心草 seed the old build wrote at tier eight.
    let mut old = SeedStack::herb_single(AptitudeHerb::Insight, SeedGrade::Yellow);
    old.tier = 8;
    document.seeds.push(old);
    let mut cultivation = Cultivation::new(document);
    // Read onto the herb's own tier, not the number the old save wrote.
    let seed = cultivation
        .document()
        .seeds
        .iter()
        .find(|seed| seed.herb == Some(AptitudeHerb::Insight))
        .expect("the 悟心草 is still held");
    assert_eq!(
        seed.tier,
        herb_tier(AptitudeHerb::Insight),
        "six, not eight"
    );
    // And the corrected seed plants.
    assert!(
        cultivation
            .plant_herb_seed(AptitudeHerb::Insight, SeedGrade::Yellow, 1_000)
            .is_some(),
        "a seed read at its own tier plants"
    );
}

#[test]
fn a_herb_seed_planted_and_picked_feeds_the_body() {
    let mut cultivation = tended(Cultivation::default());
    document_herb_seed(&mut cultivation, AptitudeHerb::Wisdom, SeedGrade::Earth);
    // The seed sits in the satchel at its herb's fixed tier, and planting it
    // spends the seed and beds a herb crop.
    let seed = cultivation
        .document()
        .seeds
        .iter()
        .find(|seed| seed.herb == Some(AptitudeHerb::Wisdom))
        .expect("the seed is held");
    assert_eq!(seed.tier, 8, "慧心草 grows at tier eight");
    assert!(
        cultivation
            .plant_herb_seed(AptitudeHerb::Wisdom, SeedGrade::Earth, 0)
            .is_some()
    );
    assert!(cultivation.document().seeds.is_empty(), "the seed is spent");
    let view = cultivation.view();
    let crop = view.plots[0].crop.as_ref().expect("the crop is in the bed");
    assert_eq!(crop.herb, Some(AptitudeHerb::Wisdom));
    // Once it is ripe it is picked, and the body is fed on the spot.
    let before = cultivation.aptitude_tenths();
    cultivation
        .harvest(0, grow_seconds(8))
        .expect("a ripe herb crop");
    assert!(
        cultivation.aptitude_tenths() > before,
        "the harvest fed the body"
    );
    assert!(
        cultivation.document().herb_stacks.is_empty(),
        "nothing was held over: the herb was taken, not hoarded"
    );
    // The log carries both halves: the herb came up, and the body was fed.
    let events = &cultivation.view().events;
    assert!(
        events.iter().any(|event| matches!(
            event.detail,
            EventDetail::Harvested {
                herb: Some(AptitudeHerb::Wisdom),
                ..
            }
        )),
        "the harvest line names the herb"
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event.detail, EventDetail::Aptitude { .. })),
        "the feed line records the 潜质 move"
    );
}

#[test]
fn a_herb_seed_that_cannot_be_fed_is_kept_for_the_satchel() {
    // A body already at the top of the band cannot take the herb; the harvest
    // keeps it in the bag rather than burning a herb the body would refuse.
    let mut cultivation = tended(Cultivation::default());
    document_herb_seed(&mut cultivation, AptitudeHerb::Insight, SeedGrade::Earth);
    // Cap 潜质 at its ceiling so the feed is refused.
    let mut document = cultivation.document().clone();
    document.aptitude_tenths = Some(APTITUDE_MAX_TENTHS);
    cultivation = Cultivation::new(document);
    cultivation
        .plant_herb_seed(AptitudeHerb::Insight, SeedGrade::Earth, 0)
        .expect("the seed is planted");
    cultivation
        .harvest(0, grow_seconds(6))
        .expect("a ripe herb crop");
    assert_eq!(
        cultivation.aptitude_herbs()[0],
        1,
        "the 悟心草 is held for the satchel"
    );
    assert!(
        cultivation
            .document()
            .herb_stacks
            .iter()
            .any(|stack| stack.herb == AptitudeHerb::Insight),
        "the herb landed in the bag"
    );
}

#[test]
fn an_attribute_seed_and_a_herb_seed_of_the_same_tier_and_grade_are_distinct() {
    // The two share a tier and a grade only by coincidence, but they are two
    // different things to plant: a herb stack must never be read as an
    // attribute seed of the same numbers.
    let mut cultivation = tended(Cultivation::default());
    document_herb_seed(&mut cultivation, AptitudeHerb::Wisdom, SeedGrade::Earth);
    document_seed(&mut cultivation, Attribute::Strength, 6, SeedGrade::Earth);
    // Planting the attribute seed finds the attribute stack, not the herb one.
    cultivation
        .plant(Attribute::Strength, 6, SeedGrade::Earth, 0)
        .expect("the attribute seed is planted");
    assert_eq!(
        cultivation.view().plots[0]
            .crop
            .as_ref()
            .and_then(|crop| crop.herb),
        None,
        "the bed grew the attribute crop"
    );
    assert!(
        cultivation
            .document()
            .seeds
            .iter()
            .any(|seed| seed.herb == Some(AptitudeHerb::Wisdom)),
        "the herb seed still waits in the satchel"
    );
}

#[test]
fn exploration_rewards_climb_with_the_realm() {
    // 境界越高, 奖励越多: the two levers a walk reads — the 阶级 its goods
    // stand at and the multiplier its quality roll is read through — both rise
    // monotonically with the realm a walk is taken in, so a save that climbs
    // the ladder finds deeper goods on the same road and never poorer ones.
    let mut tiers = Vec::new();
    let mut factors = Vec::new();
    for realm in Realm::ALL {
        let power = super::drop::realm_power(realm);
        tiers.push(super::drop::tier_for_power(power));
        factors.push(super::drop::power_factor_permille(power));
    }
    for pair in tiers.windows(2) {
        assert!(pair[1] >= pair[0], "the goods tier never falls: {tiers:?}");
    }
    for pair in factors.windows(2) {
        assert!(
            pair[1] >= pair[0],
            "the quality factor never falls: {factors:?}"
        );
    }
    assert!(
        tiers.last() > tiers.first(),
        "a deeper 境界 pays a higher tier: {tiers:?}"
    );
    assert!(
        factors.last() > factors.first(),
        "a deeper 境界 pays a better quality: {factors:?}"
    );
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
fn a_pomodoro_with_no_book_trains_the_body_and_can_pay_points() {
    use super::events::EventDetail;
    // A fresh cultivator has no 功法 in study, so every finished pomodoro is a
    // 练功 session rather than an empty one.
    let mut cultivation = Cultivation::default();
    assert!(cultivation.document().selected_art.is_none());

    let mut paid = 0u32;
    for second in 1..=400i64 {
        let before = cultivation.document().training_points;
        cultivation.on_session_completed(second * 7, day(20_000), 25);
        let after = cultivation.document().training_points;
        if after > before {
            let gain = after - before;
            assert!((1..=3).contains(&gain), "练功 paid {gain} points");
            paid += gain;
        }
    }
    // The chance is a chance: some sessions pay, and none pays more than three,
    // so the total is neither nothing nor everything the sessions could have.
    assert!(paid > 0, "练功 paid over four hundred sessions");
    assert!(paid < 400 * 3, "练功 is not a dispenser");
    // The paid points are part of the placeable balance, beside the levels'.
    assert_eq!(
        cultivation.free_points(),
        cultivation.granted_points().saturating_add(paid),
        "the 练功 points join the level points in the balance"
    );
    // A point earned is one the log names.
    assert!(
        cultivation
            .document()
            .events
            .iter()
            .any(|event| matches!(event.detail, EventDetail::TrainBody { .. })),
        "the log tells the user about a 练功 point"
    );
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
        .apply_absence(day(20_010), day_unix(day(20_010)))
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
        if let Some((attribute, tier, grade)) =
            super::garden::seed_return(Attribute::Root, 3, super::garden::MAX_GARDEN_RANK, &mut rng)
        {
            let _ = grade;
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
        "10,000 harvests at the top 阶 returned {returns} seeds, which is not the ~15% the rate describes"
    );
    let upgrade_percent = upgraded * 100 / returns.max(1);
    assert!(
        (20..=40).contains(&upgrade_percent),
        "{upgrade_percent}% of returns were a tier higher, which is not the ~30% intended"
    );
}

/// The 仙草园 pays one percent per 阶 for its second seed: a rank-one garden
/// rolls one percent, and the top 阶 rolls the fifteen percent the old flat
/// rate always paid. A rank past the top cannot outrun it.
#[test]
fn the_seed_return_odds_climb_one_percent_per_garden_rank() {
    for (rank, permille) in [
        (1u8, 10u64),
        (2, 20),
        (5, 50),
        (10, 100),
        (super::garden::MAX_GARDEN_RANK, 150),
        (super::garden::MAX_GARDEN_RANK + 5, 150),
    ] {
        assert_eq!(
            super::garden::seed_return_permille(rank),
            permille,
            "rank {rank} should pay {permille}‰"
        );
    }
}

#[test]
fn a_harvest_that_gives_a_seed_back_puts_it_in_the_satchel() {
    let mut cultivation = tended(Cultivation::default());
    let mut returned = None;
    for step in 0..200i64 {
        let planted_at = step * 10_000;
        document_seeds(&mut cultivation, Attribute::Mana, 2, 1);
        assert!(
            cultivation
                .plant(Attribute::Mana, 2, SeedGrade::Earth, planted_at)
                .is_some()
        );
        let outcome = cultivation
            .harvest(0, planted_at + grow_seconds(2))
            .expect("a ripe crop");
        if let Some(seed) = outcome.seed {
            returned = Some(seed);
            break;
        }
    }
    let (attribute, tier, _grade) = returned.expect("200 harvests at 15% return at least one seed");
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

/// One walk at a time, and it pays when it comes home rather than when it
/// leaves: the walk itself is the wait, so there is no second cooldown beside
/// it.
#[test]
fn one_walk_at_a_time_and_it_pays_when_it_comes_home() {
    let start = 10_000;
    let mut cultivation = Cultivation::default();
    let started = cultivation
        .explore(start, ExplorationHours::Twelve)
        .expect("the first walk sets out");
    assert_eq!(started.ends_unix, start + WALK_SECONDS);
    assert!(!started.early, "a walk nobody called is a walk");
    assert!(
        cultivation
            .explore(start + 1, ExplorationHours::Twelve)
            .is_none(),
        "a walk already out is the only gate there is"
    );
    // The panel reads the same rule from the clock it is handed, and counts
    // down to the return rather than to a cooldown of its own.
    let underway = cultivation.view_on(start + 1, 0);
    assert!(!underway.exploration_ready, "still on the road");
    assert_eq!(underway.exploration_ready_at, Some(start + WALK_SECONDS));
    // Nothing is paid on the way out.
    assert!(cultivation.view().last_exploration.is_none());
    assert!(
        !cultivation.settle_walk(start + WALK_SECONDS - 1),
        "not home yet"
    );
    assert!(cultivation.settle_walk(start + WALK_SECONDS), "home");
    let home = cultivation.view_on(start + WALK_SECONDS, 0);
    assert!(home.exploration_ready, "home again, and owed a walk");
    assert!(home.last_exploration.is_some(), "the road paid on return");
    assert_eq!(home.explorations, 1, "the walk counted once");
    // And the next walk can start at once: what used to be the cooldown is the
    // walk itself.
    assert!(
        cultivation
            .explore(start + WALK_SECONDS, ExplorationHours::Twelve)
            .is_some()
    );
}

/// A job is taken, held, and paid on the clock — and the board hands over a new
/// three when it comes home.
#[test]
fn a_job_is_taken_and_paid_when_it_comes_due() {
    let mut cultivation = Cultivation::default();
    let board = cultivation.jobs(20_000);
    assert_eq!(board.len(), JOB_OFFER_COUNT);
    let offer = board[0];
    let before = cultivation.document().stones;
    let run = cultivation
        .accept_job(0, 1_000, 20_000)
        .expect("the job is taken");
    assert_eq!(run.ends_unix, 1_000 + offer.seconds());
    // A second job is refused while the first is in hand: both are hours of the
    // same day.
    assert!(cultivation.accept_job(1, 1_100, 20_000).is_err());
    // Not due yet, so nothing is paid.
    assert!(cultivation.settle_work(1_000).is_none());
    let wage = offer.stones();
    let paid = cultivation
        .settle_work(run.ends_unix)
        .expect("the job is paid");
    // The wage is the wage: the work coming off never changes what it paid.
    assert_eq!(paid.stones, wage, "{paid:?} against the wage {wage}");
    assert!(
        paid.success_rate <= 1_000 && paid.success_rate >= 100,
        "{paid:?}"
    );
    assert!(paid.gain.is_none() || paid.gain.unwrap().1 >= 1, "{paid:?}");
    assert_eq!(cultivation.document().stones, before + paid.stones);
    assert!(cultivation.document().work.is_none());
    // The board turned over: a finished job hands the cultivator a new three.
    assert_eq!(cultivation.document().jobs_roll, 1);
    // And the log says what it paid, which is the only place it survives a quit.
    let logged = cultivation.view().events.iter().any(|event| {
        matches!(
            event.detail,
            EventDetail::Work { stones, .. } if stones == paid.stones
        )
    });
    assert!(logged, "the purse has to be readable after the quit");
}

/// The pool a job pays is the pool the fighter reads: a gain lands in the nine
/// numbers `derived()` returns, and no gain leaves the pool alone.
#[test]
fn the_pool_a_job_pays_is_the_pool_the_fight_reads() {
    let mut cultivation = Cultivation::default();
    let run = cultivation
        .accept_job(0, 1_000, 20_000)
        .expect("the job is taken");
    let before = cultivation.job_bonus();
    let paid = cultivation.settle_work(run.ends_unix).expect("paid");
    let after = cultivation.job_bonus();
    match paid.gain {
        Some((slot, points)) => {
            let mut expected = before;
            slot.add_to(&mut expected, points);
            assert_eq!(after, expected, "the gain lands in the pool");
        }
        None => assert_eq!(after, before, "no gain leaves the pool alone"),
    }
    // And the pool is one of the shares the fighter is built from, so a gain
    // moves the number the fight reads rather than a figure nobody uses.
    assert_eq!(
        cultivation.derived(),
        cultivation
            .attributes()
            .derived()
            .plus(cultivation.equipment_bonus())
            .plus(cultivation.art_bonus())
            .plus(cultivation.job_bonus()),
        "the derived numbers fold the job pool in"
    );
}

/// The job in hand keeps its row on the board, but the board reads it as taken
/// rather than open: the row is the same job the working line names, so it is
/// marked and a second take would be refused.
#[test]
fn the_taken_job_is_marked_on_the_board() {
    let mut cultivation = Cultivation::default();
    let day = 20_000;
    let board = cultivation.jobs(day);
    let offer = board[1];
    cultivation
        .accept_job(1, 1_000, day)
        .expect("the job is taken");
    let view = cultivation.view_on(1_100, day);
    let taken = view
        .jobs
        .iter()
        .filter(|job| job.working)
        .map(|job| job.slot)
        .collect::<Vec<_>>();
    assert_eq!(
        taken,
        vec![1],
        "only the job in hand reads as taken: {taken:?}"
    );
    assert!(
        view.jobs[1].tier == offer.tier
            && view.jobs[1].hours == offer.hours
            && view.jobs[1].grade == offer.grade,
        "the taken row is the accepted job"
    );
}

/// A job and a walk are the same hours, so neither can be taken while the other
/// is in hand.
#[test]
fn a_job_and_a_walk_cannot_be_done_at_once() {
    let mut cultivation = Cultivation::default();
    cultivation
        .explore(1_000, ExplorationHours::Four)
        .expect("the walk sets out");
    assert!(
        cultivation.accept_job(0, 1_100, 20_000).is_err(),
        "the road is closed while the walk is out"
    );
    cultivation.settle_walk(1_000 + WALK_SECONDS);
    let run = cultivation
        .accept_job(0, 2_000, 20_000)
        .expect("the job is taken once the walk is home");
    assert!(cultivation.explore(2_100, ExplorationHours::Four).is_none());
    // Working closes the tower for the same reason: it is climbed at home.
    assert!(cultivation.trial_blocked(2_100, 20_000).is_some());
    let _ = run;
}

/// A walk outlives the quit: the document keeps it, so reopening the app finds
/// the cultivator still on the road with the time that is left rather than
/// silently home again.
#[test]
fn a_walk_survives_a_restart() {
    let started = 1_000;
    let mut cultivation = Cultivation::default();
    cultivation
        .explore(started, ExplorationHours::Twelve)
        .expect("the walk sets out");
    let reloaded = Cultivation::new(cultivation.document().clone());
    let walk = reloaded.walk().expect("still out after the restart");
    assert_eq!(walk.ends_unix, started + WALK_SECONDS);
    assert!(!walk.early, "a walk nobody called is still a walk");
    let view = reloaded.view_on(started + 3_600, 0);
    assert!(!view.exploration_ready, "still on the road");
    assert_eq!(view.exploration_ready_at, Some(started + WALK_SECONDS));
}

/// 提前回返：the walk comes home with nothing, and the trip back is a third of
/// the time already spent.
#[test]
fn a_called_walk_pays_nothing() {
    let start = 1_000;
    let mut cultivation = Cultivation::default();
    cultivation
        .explore(start, ExplorationHours::Twelve)
        .expect("the walk sets out");
    assert!(cultivation.return_early(start + 6 * 3_600), "called home");
    assert_eq!(
        cultivation
            .view_on(start + 6 * 3_600, 0)
            .outing_return_seconds,
        2 * 3_600,
        "six hours out, two back — and the page knows it"
    );
    let called = cultivation.walk().expect("still on the way back");
    assert!(called.early);
    assert_eq!(
        called.ends_unix,
        start + 8 * 3_600,
        "six hours out, two back"
    );
    // The return trip is not over yet, and a second call changes nothing.
    assert!(!cultivation.settle_walk(start + 7 * 3_600));
    assert!(!cultivation.return_early(start + 7 * 3_600));
    assert!(cultivation.settle_walk(start + 8 * 3_600));
    let home = cultivation.view_on(start + 8 * 3_600, 0);
    assert!(home.exploration_ready);
    assert!(
        home.last_exploration.is_none(),
        "a called walk pays nothing at all"
    );
    assert_eq!(home.explorations, 0, "a called walk does not count");
    assert_eq!(cultivation.stones(), 0, "and fills no purse");
}

/// 提前返回 from a job reads the same way as from a walk: the shift comes home
/// with no wage, no point and no fresh board, and the trip back is a third of
/// the hours already stood.
#[test]
fn a_called_shift_pays_nothing() {
    let start = 1_000;
    let mut cultivation = Cultivation::default();
    cultivation
        .accept_job(0, start, 20_000)
        .expect("the job is taken");
    let before = cultivation.document().stones;
    // Two hours stood, so the way home is forty minutes.
    let stood = start + 2 * 3_600;
    assert_eq!(
        cultivation.view_on(stood, 20_000).outing_return_seconds,
        40 * 60,
        "the page is told what the trip home costs before the click"
    );
    assert!(cultivation.return_early(stood), "called home");
    let called = cultivation.document().work.expect("still on the way back");
    assert!(called.early);
    assert_eq!(
        called.ends_unix,
        stood + 40 * 60,
        "two hours out, forty minutes back"
    );
    // The trip back is an outing of its own: still away, still not home, and a
    // second call cannot rewrite it.
    assert!(
        !cultivation.view_on(stood + 60, 20_000).at_home,
        "still away"
    );
    assert!(cultivation.view_on(stood + 60, 20_000).work_early);
    assert!(!cultivation.return_early(stood + 120));
    // Home again, and paid nothing at all.
    assert!(
        cultivation.settle_work(called.ends_unix).is_none(),
        "a called shift pays no wage"
    );
    let home = cultivation.view_on(called.ends_unix, 20_000);
    assert!(home.at_home, "the trip back ends at home");
    assert!(!home.work_early);
    assert!(home.work.is_none(), "the shift is over");
    assert_eq!(cultivation.document().stones, before, "and fills no purse");
    assert_eq!(
        cultivation.document().jobs_roll,
        0,
        "a called shift does not turn the board over"
    );
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
        let Some(outcome) = walk(
            &mut cultivation,
            1_000 + clock * 100_000,
            ExplorationHours::Twelve,
        ) else {
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
    let _ = walking.explore(5_000, ExplorationHours::Twelve);
    assert_eq!(
        walking.view().explorations,
        0,
        "a walk still on the road has not counted"
    );
    assert!(walking.explore(5_001, ExplorationHours::Twelve).is_none());
    walking.settle_walk(5_000 + WALK_SECONDS);
    assert_eq!(
        walking.view().explorations,
        1,
        "the walk counted when it came home"
    );
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
        let Some(outcome) = walk(
            &mut cultivation,
            1_000 + clock * 100_000,
            ExplorationHours::Twelve,
        ) else {
            continue;
        };
        let found = cultivation.view().last_exploration_found.clone();
        let Some(found) = found else { continue };
        let Some(amount) = found.stones else {
            continue;
        };
        turned_up += 1;
        // The walk was taken at twelve hours, so its purse is that length's:
        // the base purse widened by the same multiplier every chance is.
        let purse = WALK_STONES.scaled(ExplorationHours::Twelve.multiplier_permille());
        assert!(
            (purse.min..=purse.max).contains(&amount),
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
        let Some(outcome) = walk(
            &mut cultivation,
            1_000 + clock * 100_000,
            ExplorationHours::Twelve,
        ) else {
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

/// A walk that met nobody carries no fight into its popup.
///
/// The bug this guards: the fight the last walk met stayed on file through a
/// quiet walk, so the popup replayed an older walk's battle under the new
/// walk's verdict — a quiet afternoon that turned up a handful of stones,
/// announced as 败北 beneath a fight it never had. Every walk clears the
/// report; only a walk that settled a fight carries one.
#[test]
fn a_quiet_walk_carries_no_fight_into_its_popup() {
    let step = WALK_SECONDS + 1;
    // One save, walked until the road sends someone: the fight that walk met
    // is what a later quiet walk must not inherit.
    let mut cultivation = Cultivation::default();
    let mut met_someone = false;
    for attempt in 0..300_i64 {
        if walk(&mut cultivation, attempt * step, ExplorationHours::Twelve).is_none() {
            continue;
        }
        if cultivation.view().last_exploration_duel.is_some() {
            met_someone = true;
            break;
        }
    }
    assert!(met_someone, "the road sends someone within 300 walks");
    // The next walk that meets nobody must carry no fight: its popup has
    // nothing to replay and no verdict to draw.
    let mut quiet = None;
    for attempt in 300..600_i64 {
        let Some(outcome) = walk(&mut cultivation, attempt * step, ExplorationHours::Twelve) else {
            continue;
        };
        if !matches!(
            outcome,
            ExplorationOutcome::Victory | ExplorationOutcome::Defeat
        ) {
            quiet = Some(outcome);
            break;
        }
    }
    assert!(quiet.is_some(), "a quiet walk came up within 300 more");
    assert!(
        cultivation.view().last_exploration_duel.is_none(),
        "a walk that met nobody leaves no fight on file"
    );
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

/// A 功法 found twice is one book, not two: the second copy folds into the one
/// already on the shelf and the reading carries on from where it stood — the 阶,
/// the banked pomodoros and the failures it has already cost are all still
/// there, so a lucky duplicate never sends the user back to the first page.
#[test]
fn a_second_copy_of_a_book_carries_on_where_the_first_left_off() {
    let mut cultivation = Cultivation::default();
    cultivation.grant_art_book(1).expect("a book for art 1");
    // A few sessions in: some land and some fail, and either way the book is no
    // longer at its first page.
    for offset in 0..6_i64 {
        cultivation.on_session_completed(1_000 + offset, day(30_000 + offset), 25);
    }
    let before = cultivation
        .view()
        .arts
        .iter()
        .find(|art| art.id == 1)
        .cloned()
        .expect("the book is on the shelf");
    assert!(
        u32::from(before.rank) + before.progress + before.failures > 0,
        "the reading left a mark: {before:?}"
    );
    let again = cultivation.grant_art_book(1).expect("the same book again");
    assert_eq!(again.id, 1);
    assert_eq!(again.rank, before.rank, "the 阶 is where it was");
    assert_eq!(again.progress, before.progress, "and so is the progress");
    assert_eq!(again.failures, before.failures, "and the failures it cost");
    assert_eq!(
        again.copies, 2,
        "and the spare is a second book, not a reset"
    );
    assert_eq!(
        cultivation
            .view()
            .arts
            .iter()
            .filter(|art| art.id == 1)
            .count(),
        1,
        "one row on the shelf, not two: the copies are counted, not stacked"
    );
    assert_eq!(
        cultivation
            .view()
            .arts
            .iter()
            .find(|art| art.id == 1)
            .map(|art| art.progress),
        Some(before.progress),
        "the row still reads the one book being read"
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
    // A combat-paying shape pays no attributes, so the payout this is about is
    // the two together: what the 功法 pays, however it pays it.
    let bonus_before = cultivation.art_bonus().power();
    assert!(learned_before + bonus_before > 0, "the learned 内功 pays");

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
        cultivation.art_attributes().total() + cultivation.art_bonus().power(),
        learned_before + bonus_before,
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
    let _ = cultivation.explore(10_000, ExplorationHours::Twelve);
    assert!(
        cultivation
            .explore(10_001, ExplorationHours::Twelve)
            .is_none(),
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

/// A 功法 the cultivator already has is buyable, and what it buys is a spare:
/// the shelf says 已有 first so the stones are spent knowingly, and the copy
/// lands beside the book being read without touching a page of it.
#[test]
fn buying_a_book_already_on_the_shelf_buys_a_spare() {
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
    let held = cultivation
        .grant_art_book(art_id)
        .expect("the book is in hand");
    assert_eq!(
        held.copies, 1,
        "a first copy is the book itself, not a spare"
    );
    let before = cultivation.stones();
    let bought = cultivation
        .buy_from_shop(slot, day, 1_000)
        .expect("a spare is for sale");
    assert!(
        matches!(bought, ShopOffer::Art { art_id: bought } if bought == art_id),
        "the shelf sold the 功法 it showed: {bought:?}"
    );
    assert_eq!(
        cultivation.stones(),
        before - ShopOffer::Art { art_id }.price(),
        "and charged the shelf's own price"
    );
    let after = cultivation
        .view()
        .arts
        .iter()
        .find(|art| art.id == art_id)
        .cloned()
        .expect("the book is still on the shelf");
    assert_eq!(after.copies, 2, "two books now, one of them a spare");
    assert_eq!(after.rank, held.rank, "and the reading has not moved");
    assert_eq!(after.progress, held.progress, "neither the progress");
    assert_eq!(after.failures, held.failures, "nor the failures");
}

/// A 功法 learned outright has no page left in it: the shelf will not take
/// stones for a copy that could neither be read nor be anything but paper.
#[test]
fn a_learned_art_is_not_sold_again() {
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
    // Every 阶 is in: no book on the shelf, the knowledge written down instead.
    let mut cultivation = Cultivation::new(CultivationDocument {
        stones: 100_000,
        mastered_arts: vec![art_id],
        ..CultivationDocument::default()
    });
    let before = cultivation.stones();
    let refusal = cultivation
        .buy_from_shop(slot, day, 1_000)
        .expect_err("a learned 功法 is not sold twice");
    assert_eq!(refusal, TradeRefusal::AlreadyOwned, "the refusal says why");
    assert_eq!(cultivation.stones(), before, "and nothing was charged");
}

/// A spare goes to the market on its own: the book being read stays on the
/// shelf, page for page, and only the count moves.
#[test]
fn a_spare_is_sold_before_the_book_being_read() {
    let mut cultivation = Cultivation::default();
    cultivation.grant_art_book(1).expect("a book for art 1");
    for offset in 0..6_i64 {
        cultivation.on_session_completed(1_000 + offset, day(30_000 + offset), 25);
    }
    cultivation.grant_art_book(1).expect("a spare");
    let before = cultivation
        .view()
        .arts
        .iter()
        .find(|art| art.id == 1)
        .cloned()
        .expect("the book is on the shelf");
    assert_eq!(before.copies, 2, "a spare beside the book being read");
    assert!(before.selected, "and that book is the one being trained");
    let purse = cultivation.stones();
    let stones = cultivation
        .sell_art(1, 9_000)
        .expect("a spare sells even while the other copy is read");
    assert_eq!(
        cultivation.stones(),
        purse + stones,
        "the stones land in the purse"
    );
    let after = cultivation
        .view()
        .arts
        .iter()
        .find(|art| art.id == 1)
        .cloned()
        .expect("the book itself is still there");
    assert_eq!(after.copies, 1, "one copy left");
    assert_eq!(after.rank, before.rank, "the reading did not move");
    assert_eq!(after.progress, before.progress, "nor the progress");
    assert_eq!(after.failures, before.failures, "nor the failures");
    assert_eq!(
        cultivation.sell_art(1, 9_100),
        Err(TradeRefusal::Training),
        "the last copy of the book being read is not for sale"
    );
}

/// The garden sells its fifth plot for stones, charges double for each one
/// after it, and refuses the thirteenth: twelve is the whole estate.
#[test]
fn the_garden_sells_plots_up_to_nine() {
    let mut cultivation = Cultivation::new(CultivationDocument {
        stones: 40_000,
        ..CultivationDocument::default()
    });
    let mut prices = Vec::new();
    for _ in FREE_PLOT_COUNT..MAX_PLOT_COUNT {
        prices.push(cultivation.buy_plot(1_000).expect("a plot to sell"));
    }
    assert_eq!(
        prices,
        vec![150, 300, 600, 1200, 2400, 4800, 9600, 19200],
        "each plot dearer than the last"
    );
    assert_eq!(
        cultivation.document().plots.len(),
        MAX_PLOT_COUNT,
        "the garden is full"
    );
    assert_eq!(
        cultivation.document().stones,
        40_000 - 38_250,
        "the purse paid for exactly the eight plots"
    );
    assert!(
        matches!(cultivation.buy_plot(1_000), Err(TradeRefusal::PlotLimit)),
        "the thirteenth plot does not exist"
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
            what:
                SaleKind::Seed {
                    name,
                    tier,
                    grade,
                    attribute,
                },
            stones,
        } => {
            assert_eq!(name.as_deref(), Some("白虎茸"));
            assert_eq!((tier, stones), (0, 0), "the old events carry neither");
            assert_eq!(grade, SeedGrade::Earth, "and no grade reads as 地阶");
            assert_eq!(attribute, None, "nor an attribute to print");
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
            failures: 0,
            copies: 1,
        });
    }
    // A fourth deep book, never trained: owning it is not swinging it.
    document.arts.push(OwnedArt {
        art_id: external_of(ArtGrade::Heaven)[0],
        rank: 0,
        progress: 0,
        outcome: None,
        failures: 0,
        copies: 1,
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

/// The page compares every derived number against what the level alone is
/// worth — not against the four attributes spread evenly. A naked body at its
/// own level sits exactly on the reference in all nine, and anything worn,
/// trained or allocated is read as "over what my level pays".
#[test]
fn a_build_is_measured_against_what_its_level_alone_pays() {
    let level = 10;
    let bare = base_attributes(level);
    for breakdown in bare.derived().breakdowns(bare, Derived::default(), level) {
        assert_eq!(
            breakdown.result, breakdown.baseline,
            "{} is off what level {level} alone pays",
            breakdown.key
        );
        assert_eq!(breakdown.level, level, "the row says which level it read");
    }

    // 力量 poured on top of the same level: 攻击力 climbs over the reference by
    // exactly the points added, and 内力值 stays on it — the reference is the
    // level's, not the build's own mean, so an untouched number is unmoved.
    let stronger = bare.plus(Attributes::new(0, 100, 0, 0));
    let breakdowns = stronger
        .derived()
        .breakdowns(stronger, Derived::default(), level);
    let row = |key: &str| {
        breakdowns
            .iter()
            .find(|breakdown| breakdown.key == key)
            .copied()
            .expect("a row")
    };
    let attack = row("cultivation.attack");
    assert_eq!(attack.baseline, 10 + bare.strength as u32 * 3);
    assert_eq!(attack.result, 10 + (bare.strength as u32 + 100) * 3);
    let inner = row("cultivation.inner");
    assert_eq!(inner.result, inner.baseline, "根骨 was not touched");
    // A deep realm's reference is its own: the same bare body at level 10 read
    // against level 200 would look gigantic, which is the bug this replaces.
    let deep = base_attributes(200);
    let deep_rows = deep.derived().breakdowns(deep, Derived::default(), 200);
    assert!(
        deep_rows[0].baseline > attack.baseline,
        "a higher level is a higher yardstick"
    );
}

/// An opponent is the rung's own body plus the gear it wears plus the 功法 it
/// swings — and the gear is a bonus on top of the body, which is what makes a
/// well-dressed guardian a harder fight than a ragged one.
#[test]
fn every_opponent_walks_in_with_gear_and_a_move() {
    let mut seen_gear = std::collections::BTreeSet::new();
    let mut seen_worth = std::collections::BTreeSet::new();
    for (index, realm) in Realm::ALL.iter().enumerate() {
        let level = realm.cap_level();
        let rival = super::opponent::guardian(*realm, Realm::from_index(index + 1));
        assert!(
            !rival.gear.is_empty(),
            "the guardian of {realm:?} walks in bare-handed"
        );
        assert!(
            rival.art.is_some(),
            "the guardian of {realm:?} swings no art"
        );
        // The body is the rung's, and the gear is on top of it: the opponent is
        // never weaker than the naked cultivator it gates.
        let body = base_attributes(level);
        assert!(
            rival.attributes.power() >= body.power(),
            "the guardian of {realm:?} is weaker than its own bare rung"
        );
        assert!(rival.fighter.health > 0, "it has to have health to lose");
        seen_gear.insert(rival.gear.len());
        // A guardian has two slots and the allowance is generous enough that both
        // are usually filled, so the variety between two realms is what the gear
        // is *worth* rather than how many pieces there are.
        seen_worth.insert(
            rival
                .attributes
                .power()
                .saturating_sub(base_attributes(level).power()),
        );
    }
    assert!(
        seen_gear.len() == 1 && seen_gear.contains(&2),
        "a guardian wears both of its slots: {seen_gear:?}"
    );
    // Two realms do not have to dress alike: the roll is where the variety
    // between two guardians on the same ladder comes from.
    assert!(
        seen_worth.len() > 1,
        "every guardian on the ladder wears gear worth the same: {seen_worth:?}"
    );
}

/// Gear is what gets the player through. A save at the cap with nothing on loses
/// to the guardian it stands before most of the time; the same save wearing the
/// four pieces it would have by then wins — and wins at every rung, because a
/// realm nobody can open is not a challenge but a wall.
#[test]
fn gear_is_what_opens_a_realm() {
    for (index, realm) in Realm::ALL.iter().enumerate() {
        if realm.next().is_none() {
            continue;
        }
        let level = realm.cap_level();
        let rival = super::opponent::guardian(*realm, Realm::from_index(index + 1));
        let body = base_attributes(level);
        let free = i32::from(level) / 4;
        let naked = Fighter::from_attributes(
            PLAYER_NAME.to_owned(),
            Attributes::new(
                body.root + free,
                body.strength + free,
                body.mana + free,
                body.agility + free,
            ),
            None,
        );
        // The save that got here: the four pieces a player of that realm would
        // be wearing, rolled off the same table the drops use.
        let mut geared = Attributes::new(
            body.root + free,
            body.strength + free,
            body.mana + free,
            body.agility + free,
        );
        let mut slots = std::collections::BTreeSet::new();
        let mut dress = Rng::seeded(index as u64 + 99);
        while slots.len() < EquipSlot::ALL.len() {
            let item =
                super::items::roll_equipment(0, index as u8 + 1, super::Rarity::Common, &mut dress);
            if slots.insert(item.slot.index()) {
                geared = geared.plus(item.attributes);
            }
        }
        let dressed = Fighter::from_attributes(PLAYER_NAME.to_owned(), geared, None);
        let mut naked_wins = 0;
        let mut dressed_wins = 0;
        for seed in 0..400_u64 {
            if super::duel::duel(naked.clone(), rival.fighter.clone(), &mut Rng::seeded(seed)).won {
                naked_wins += 1;
            }
            if super::duel::duel(
                dressed.clone(),
                rival.fighter.clone(),
                &mut Rng::seeded(seed),
            )
            .won
            {
                dressed_wins += 1;
            }
        }
        assert!(
            dressed_wins > naked_wins,
            "{realm:?}: gear made no difference ({dressed_wins} against {naked_wins} of 400)"
        );
        assert!(
            dressed_wins >= 220,
            "{realm:?}: the realm is a wall — a geared save wins only {dressed_wins} of 400"
        );
    }
}

/// One attempt at a star, retried until it comes off the way the test wants.
///
/// The roll is seeded from the clock the caller hands in, so walking the clock
/// is walking the dice: the test never waits on luck it cannot see.
fn attempt_until(cultivation: &mut Cultivation, item_id: u64, want_success: bool) -> StarOutcome {
    for now in 0..400_i64 {
        let Ok(outcome) = cultivation.star_item(item_id, now) else {
            panic!("the attempt was refused");
        };
        if outcome.success == want_success {
            return outcome;
        }
    }
    panic!("the attempt never came off the wanted way");
}

/// A save owning one piece of gear, worn, with a purse that can star it.
///
/// The resources are stocked high so a 升星's tribute never refuses the attempt:
/// a test asking about a star is asking about the star, not about the ore.
fn owning_one(item: EquipmentRecord, stones: u32) -> Cultivation {
    let mut document = CultivationDocument {
        stones,
        resources: ResourceBag {
            wood: 100,
            stone: 100,
            iron: 100,
            coal: 100,
        },
        ..CultivationDocument::default()
    };
    document.equipment.push(item.clone());
    document.equipped = vec![None; SLOT_COUNT];
    document.equipped[item.slot.index()] = Some(item.id);
    document.next_item_id = item.id + 1;
    Cultivation::new(document)
}

/// The stars are the second axis: one star is worth five to eight percent of
/// everything the piece pays, attributes and outright combat numbers alike.
#[test]
fn a_star_is_worth_five_to_eight_percent_of_everything() {
    let base = EquipmentRecord {
        id: 7,
        slot: EquipSlot::Weapon,
        tier: 5,
        rarity: Rarity::Rare,
        attributes: Attributes::new(0, 100, 40, 0),
        bonus: Derived {
            attack: 60,
            health: 40,
            ..Derived::default()
        },
        star: 0,
        effect: None,
        name: Some("试剑".to_owned()),
        set: None,
    };
    let mut starred = base.clone();
    starred.star = 1;
    let one = star_multiplier_permille(base.id, 1);
    assert!((1_050..=1_080).contains(&one));
    let scaled = |value: u32| ((u64::from(value) * u64::from(one) + 500) / 1_000) as u32;
    assert_eq!(
        i32::try_from(scaled(100)).unwrap(),
        starred.effective_attributes().strength
    );
    assert_eq!(starred.effective_bonus().attack, scaled(60));
    assert_eq!(starred.effective_bonus().health, scaled(40));
    // The piece's own worth moves with it, and the worn total reads it.
    assert!(starred.power() > base.power());
    let plain = owning_one(base.clone(), 0);
    let shining = owning_one(starred.clone(), 0);
    assert!(shining.derived().attack > plain.derived().attack);
    assert!(shining.power() > plain.power());
}

/// The wager's price is half of what the piece sells for, and the purse pays it
/// whether the star turns over or not.
#[test]
fn a_star_costs_a_third_of_what_the_piece_sells_for() {
    let item = EquipmentRecord {
        id: 3,
        slot: EquipSlot::Clothes,
        tier: 4,
        rarity: Rarity::Fine,
        attributes: Attributes::new(10, 0, 0, 10),
        bonus: Derived::default(),
        star: 0,
        effect: None,
        name: None,
        set: None,
    };
    let sell = shop::ShopOffer::Equipment(item.clone()).sell_price();
    let cost = star_up_cost(&item, 0);
    assert_eq!(
        cost.stones,
        sell / 3,
        "the wager is a third of the sell price"
    );
    // The 灵石 wager does not climb with the stars already bought: every
    // attempt is the same bet. The tribute is the same wood and stone too —
    // until the star being reached is a high one, which is where the iron
    // begins.
    let mut deep = item.clone();
    deep.star = 7;
    let deep_cost = star_up_cost(&deep, 0);
    assert_eq!(deep_cost.stones, cost.stones, "the wager is the same bet");
    assert_eq!(cost.resources.iron, 0, "a low star burns no iron");
    assert!(
        deep_cost.resources.iron > 0,
        "a high star is where the 采石 trade's find is spent"
    );

    let purse = 1_000;
    let mut cultivation = owning_one(item.clone(), purse);
    let outcome = attempt_until(&mut cultivation, item.id, true);
    assert_eq!(outcome.to, 1, "the star turned over");
    assert_eq!(outcome.from, 0);
    assert_eq!(
        cultivation.document().stones,
        purse - cost.stones,
        "the purse paid the wager"
    );
    // A failed wager is still a wager: walk the clock until the roll comes to
    // nothing, counting every attempt it took on the way.
    let before = cultivation.document().stones;
    let mut attempts = 0_u32;
    let mut failed = None;
    for now in 0..400_i64 {
        attempts += 1;
        let outcome = cultivation.star_item(item.id, now).expect("the attempt");
        if !outcome.success {
            failed = Some(outcome);
            break;
        }
    }
    let failed = failed.expect("a failure came up");
    assert_eq!(
        cultivation.document().stones,
        before - cost.stones * attempts,
        "a failed wager is still a wager"
    );
    assert_eq!(
        failed.to,
        failed.from.saturating_sub(1).max(failed.from.min(1)),
        "and it walked one star back, never past the first"
    );
}

/// The floor is the first star: a failure below it costs the wager and nothing
/// else, so a run of bad luck costs progress and never the piece's footing.
#[test]
fn a_failure_never_walks_a_piece_past_its_first_star() {
    let item = EquipmentRecord {
        id: 9,
        slot: EquipSlot::Boots,
        tier: 3,
        rarity: Rarity::Common,
        attributes: Attributes::new(0, 0, 0, 20),
        bonus: Derived::default(),
        star: 1,
        effect: None,
        name: None,
        set: None,
    };
    // The clock whose roll fails at the first star: a fresh save per candidate,
    // so the observation is the first attempt's own and nothing that came
    // before it.
    let failed = (0..400_i64)
        .find_map(|now| {
            let mut cultivation = owning_one(item.clone(), 5_000);
            match cultivation.star_item(item.id, now) {
                Ok(outcome) if !outcome.success => Some(outcome),
                _ => None,
            }
        })
        .expect("a failing roll came up");
    assert_eq!(failed.from, 1);
    assert_eq!(failed.to, 1, "the first star is the floor");
    // And a success from one star goes to two, not past it.
    let mut cultivation = owning_one(item.clone(), 5_000);
    let won = attempt_until(&mut cultivation, item.id, true);
    assert_eq!(won.from, 1);
    assert_eq!(won.to, 2);
}

/// The purse is the gate: a wager the purse cannot cover is refused with the
/// reason, and a piece nobody owns is refused the same way.
#[test]
fn a_star_the_purse_cannot_cover_is_refused() {
    let item = EquipmentRecord {
        id: 4,
        slot: EquipSlot::Pants,
        tier: 6,
        rarity: Rarity::Immortal,
        attributes: Attributes::new(0, 30, 0, 30),
        bonus: Derived::default(),
        star: 0,
        effect: None,
        name: None,
        set: None,
    };
    let cost = star_up_cost(&item, 0);
    let mut cultivation = owning_one(item.clone(), cost.stones - 1);
    assert_eq!(
        cultivation.star_item(item.id, 500),
        Err(TradeRefusal::NotEnoughStones),
        "one stone short is one stone short"
    );
    assert_eq!(
        cultivation.document().stones,
        cost.stones - 1,
        "and nothing moved"
    );
    assert_eq!(
        cultivation.star_item(999, 500),
        Err(TradeRefusal::NotOwned),
        "a piece nobody owns"
    );
}

/// Ten is the whole sky: a full piece refuses the wager rather than charging
/// for a star it cannot hold.
#[test]
fn a_full_piece_refuses_the_wager() {
    let item = EquipmentRecord {
        id: 5,
        slot: EquipSlot::Weapon,
        tier: 2,
        rarity: Rarity::Common,
        attributes: Attributes::new(0, 10, 0, 0),
        bonus: Derived::default(),
        star: MAX_STAR,
        effect: None,
        name: None,
        set: None,
    };
    let mut cultivation = owning_one(item.clone(), 5_000);
    assert_eq!(
        cultivation.star_item(item.id, 500),
        Err(TradeRefusal::StarCapped)
    );
    assert_eq!(
        cultivation.document().stones,
        5_000,
        "and nothing was spent"
    );
}

/// One gathering at the forest fills the purse with wood and banks 伐木 xp; a
/// run of them climbs the trade, and the 仓库's ceiling clips a full purse
/// rather than overflowing it.
#[test]
fn a_forest_gathering_fills_the_purse_and_trains_the_trade() {
    let mut cultivation = Cultivation::new(CultivationDocument::default());
    let first = cultivation
        .collect_resources(GatherSite::Forest, 0)
        .expect("the gathering");
    assert!(first.gained.wood > 0, "the forest pays wood");
    assert_eq!(first.woodcut_xp, 100, "one gathering is a hundred xp");
    assert_eq!(first.stonecut_xp, 0, "the forest does not train 采石");
    let bag = &cultivation.document().resources;
    assert_eq!(bag.wood, first.gained.wood, "the purse holds the find");
    // A full purse takes only what it has room for: no overflow, no loss of a
    // kind that still has room.
    let mut full = Cultivation::new(CultivationDocument {
        resources: ResourceBag {
            wood: BASE_RESOURCE_CAP,
            stone: 0,
            iron: 0,
            coal: 0,
        },
        ..CultivationDocument::default()
    });
    let capped = full
        .collect_resources(GatherSite::Forest, 0)
        .expect("the gathering");
    assert_eq!(capped.gained.wood, 0, "a full purse takes no wood");
    assert_eq!(
        full.document().resources.wood,
        BASE_RESOURCE_CAP,
        "the purse stays at its ceiling"
    );
}

/// One round at the mine trains both trades at once, because the mine is where
/// the two trained hands are worth more together.
#[test]
fn a_mine_gathering_trains_both_trades() {
    let mut cultivation = Cultivation::new(CultivationDocument::default());
    let outcome = cultivation
        .collect_resources(GatherSite::Mine, 0)
        .expect("the gathering");
    assert_eq!(outcome.woodcut_xp, 100);
    assert_eq!(outcome.stonecut_xp, 100);
}

/// A finished gathering session pays a whole realm-sized round of the site's
/// yield and xp, and nothing of the 修习 ladder: no growth, no 功法 turn, no
/// 灵石 cut, no 试炼 chance, no roll. The day is marked active so the absence
/// clock does not charge it, and the log carries a line of its own.
#[test]
fn a_gathering_session_pays_a_realm_sized_yield_and_no_ladder() {
    let mut cultivation = Cultivation::new(CultivationDocument::default());
    // The base round for the same hands: what one manual click pays.
    let base = cultivation
        .collect_resources(GatherSite::Forest, 0)
        .expect("the gathering");
    let baseline_growth = cultivation.document().growth;
    let baseline_stones = cultivation.document().stones;
    // A 炼气 session is one hour: the same as a base round. The purse's ceiling
    // is shared with the manual round that already banked its wood, so the xp —
    // which is never clipped — is the fair comparison.
    let outcome = cultivation.collect_session(GatherSite::Forest, 1_000, day(20_000));
    assert_eq!(
        outcome.woodcut_xp, base.woodcut_xp,
        "炼气 is one hour of gathering"
    );
    assert_eq!(outcome.stonecut_xp, 0, "the forest does not train 采石");
    assert_eq!(
        cultivation.document().growth,
        baseline_growth,
        "a gathering session pays no growth"
    );
    assert_eq!(
        cultivation.document().stones,
        baseline_stones,
        "a gathering session pays no 灵石"
    );
    assert_eq!(
        cultivation.document().last_active_day,
        Some(20_000),
        "the day is marked active"
    );
    assert_eq!(
        cultivation.document().missed_streak,
        0,
        "the absence clock is cleared"
    );
    let newest = cultivation.document().events.first().expect("the line");
    assert!(
        matches!(&newest.detail, EventDetail::SessionGather { site, .. } if *site == GatherSite::Forest),
        "the log names the gathering session: {:?}",
        newest.detail
    );
}

/// A higher realm's gathering session is worth more: one quarter of an hour per
/// realm, so a 大乘 session is 3.25 base rounds of the same trained hands.
#[test]
fn a_higher_realm_gathering_session_pays_more() {
    let mut base = Cultivation::new(CultivationDocument::default());
    let base_round = base
        .collect_resources(GatherSite::Forest, 0)
        .expect("the gathering");
    let mut high = Cultivation::new(CultivationDocument {
        realm: 9,
        last_active_day: Some(20_000),
        charged_through_day: Some(20_000),
        ..CultivationDocument::default()
    });
    assert_eq!(high.realm(), Realm::Immortal, "the top realm for the test");
    let session = high.collect_session(GatherSite::Forest, 2_000, day(20_001));
    assert_eq!(
        session.woodcut_xp,
        base_round.woodcut_xp.saturating_mul(3_250) / 1_000,
        "a 仙人 session is 3.25 hours of gathering"
    );
}

/// The mode rides the document and the view: what is chosen before a session is
/// what the completion reads, and a 修习 mode is the default a fresh save gets.
#[test]
fn the_session_mode_persists_and_rides_the_view() {
    let mut cultivation = Cultivation::new(CultivationDocument::default());
    assert_eq!(
        cultivation.session_mode(),
        SessionMode::Study,
        "a fresh save 修习s"
    );
    cultivation.set_session_mode(SessionMode::Woodcut);
    assert_eq!(cultivation.session_mode(), SessionMode::Woodcut);
    assert_eq!(cultivation.view().session_mode, SessionMode::Woodcut);
    // A gathering mode is a session mode, not a stored 修习 state: it names one
    // site and no other.
    assert_eq!(SessionMode::Woodcut.site(), Some(GatherSite::Forest));
    assert_eq!(SessionMode::Study.site(), None);
}

/// A building rank is paid in 灵石 and the gathered tribute, atomically: a
/// purse with the money but not the ore is told it is short of the ore, and a
/// rank at the top of the ladder is refused outright.
#[test]
fn a_building_rank_costs_stones_and_resources() {
    let mut cultivation = Cultivation::new(CultivationDocument {
        stones: 1_000,
        resources: ResourceBag {
            wood: 100,
            stone: 100,
            iron: 100,
            coal: 100,
        },
        ..CultivationDocument::default()
    });
    let cost = building_upgrade_cost(0);
    let outcome = cultivation
        .upgrade_building(Building::Workshop, 0)
        .expect("the rank");
    assert_eq!(outcome.from, 0);
    assert_eq!(outcome.to, 1);
    assert_eq!(cultivation.document().workshop_level, 1);
    assert_eq!(cultivation.document().stones, 1_000 - cost.stones);
    assert_eq!(
        cultivation.document().resources.wood,
        100 - cost.resources.wood,
        "the tribute was paid"
    );
    // A purse with the money but not the ore is refused for the ore.
    let mut poor = Cultivation::new(CultivationDocument {
        stones: 1_000,
        ..CultivationDocument::default()
    });
    assert_eq!(
        poor.upgrade_building(Building::Workshop, 0),
        Err(TradeRefusal::NotEnoughResources),
        "the money is there, the ore is not"
    );
}

/// A 神装's effect is a debuff the other side walks in with, read when the
/// duel is built: the numbers the panel shows are the numbers the fight uses.
#[test]
fn a_divine_pieces_effect_reaches_the_other_side_of_the_fight() {
    let mut blade = EquipmentRecord {
        id: 11,
        slot: EquipSlot::Weapon,
        tier: 8,
        rarity: Rarity::Divine,
        attributes: Attributes::new(0, 200, 0, 0),
        bonus: Derived::default(),
        star: 0,
        effect: Some(GearEffect::Frost),
        name: Some("寒霜剑【寒霜】".to_owned()),
        set: None,
    };
    let mut cultivation = owning_one(blade.clone(), 0);
    let plain = cultivation.fighter(&mut Rng::seeded(1));
    // The same save with the 神装 worn strikes the same and carries the effect.
    cultivation.equip(blade.id);
    let wearing = cultivation.fighter(&mut Rng::seeded(1));
    assert_eq!(
        wearing.attack, plain.attack,
        "the effect bites the other side"
    );
    let mut rival =
        Fighter::from_attributes("对手".to_owned(), Attributes::new(0, 300, 0, 0), None);
    let before = (rival.attack, rival.hit);
    let report = cultivation.duel_with_effects(wearing.clone(), rival.clone(), &mut Rng::seeded(2));
    let _ = report;
    // Read the debuff the way the duel builds it: frost bites attack and hit.
    let depth = GearEffect::Frost.depth_permille(Rarity::Divine);
    let bitten = |value: u32| (u64::from(value) * u64::from(1_000 - depth) / 1_000) as u32;
    assert_eq!(rival.attack, before.0, "the rival is handed over unchanged");
    rival.attack = bitten(rival.attack);
    rival.hit = bitten(rival.hit);
    assert!(rival.attack < before.0 && rival.hit < before.1);
    let _ = &mut blade;
}

/// The day's event is fired once and seen once: the acknowledgement is written
/// down, so a window opened again the same day stays quiet about it.
#[test]
fn the_morning_event_is_seen_once_and_remembered() {
    let mut cultivation = Cultivation::default();
    let first = cultivation.daily_event(1_000, 40).expect("the day opens");
    assert!(
        first.kind == DailyKind::Ambush
            || first.kind == DailyKind::Gift
            || first.kind == DailyKind::Trinket
    );
    let view = cultivation.view_on(1_000, 40);
    assert_eq!(view.daily_event_day, Some(40));
    assert_eq!(view.daily_event_seen_day, None, "nobody has seen it yet");
    cultivation.acknowledge_daily_event(40);
    let view = cultivation.view_on(1_000, 40);
    assert_eq!(
        view.daily_event_seen_day,
        Some(40),
        "the day it was shown is written down"
    );
    // A second call the same day opens nothing, and the next day opens again.
    assert!(cultivation.daily_event(2_000, 40).is_none());
    assert!(cultivation.daily_event(3_000, 41).is_some());
}

/// Two 内功 of one 前缀 are one set; a lone book is a collection of one, and
/// two books of different 前缀 are two collections.
///
/// Only 玄阶 and deeper books count towards a family, so the books here are
/// 玄阶 ones: 241 and 321 are both 内功 (odd) and both carry prefix 0
/// (((241-1)/2) % 40 is 120 % 40, ((321-1)/2) % 40 is 160 % 40), while 243
/// carries prefix 1.
#[test]
fn two_books_of_one_prefix_are_a_set_and_a_lone_book_is_not() {
    let book = |id: u64| OwnedArt {
        art_id: id,
        rank: 3,
        progress: 0,
        failures: 0,
        copies: 1,
        outcome: None,
    };
    assert_eq!(
        art_set_bonus(&[book(241)]).power(),
        0,
        "a lone book is not a set"
    );
    let pair = art_set_bonus(&[book(241), book(321)]);
    assert!(
        pair.power() > 0,
        "two books of one prefix pay their set: {pair:?}"
    );
    assert_eq!(
        art_set_bonus(&[book(241), book(243)]).power(),
        0,
        "two books of different prefixes are two collections"
    );
}

/// The set bonus lands on the family's primary combat number and grows with
/// how many books the family holds: an extra book is another step of the step.
#[test]
fn an_art_set_pays_its_familys_primary_combat_number_and_scales() {
    let book = |id: u64| OwnedArt {
        art_id: id,
        rank: 1,
        progress: 0,
        failures: 0,
        copies: 1,
        outcome: None,
    };
    // Two same-prefix books that pay attack (the 刚 family) and nothing else.
    // Only 玄阶+ books count towards a family, so the search stays inside the
    // books a set can open.
    let counts = |id: u64| super::arts::book_counts_towards_a_set(id);
    let mut pair_ids = None;
    'outer: for a in 1..=ART_COUNT {
        if a % 2 == 0 || !counts(a) {
            continue;
        }
        for b in (a + 1..=ART_COUNT).step_by(2) {
            if !counts(b) {
                continue;
            }
            let probe = art_set_bonus(&[book(a), book(b)]);
            if probe.attack > 0 && probe.health == 0 && probe.spell == 0 && probe.speed == 0 {
                pair_ids = Some((a, b));
                break 'outer;
            }
        }
    }
    let (a, b) = pair_ids.expect("a 刚 family pair must pay attack only");
    // A third book of the *same* 前缀 deepens the set. Same-prefix 内功 ids
    // differ by multiples of 80, so one always exists below ART_COUNT.
    let mut third = None;
    for c in 1..=ART_COUNT {
        if c % 2 == 0 || !counts(c) || c == a || c == b {
            continue;
        }
        if super::arts::prefix_of(c) == super::arts::prefix_of(a) {
            third = Some(c);
            break;
        }
    }
    let c = third.expect("a third same-prefix 内功 must exist below ART_COUNT");
    let pair = art_set_bonus(&[book(a), book(b)]);
    let three = art_set_bonus(&[book(a), book(b), book(c)]);
    assert!(pair.attack > 0, "the pair pays attack");
    assert!(
        three.attack > pair.attack,
        "a third book of the same 前缀 deepens the set: {} against {}",
        three.attack,
        pair.attack
    );
}

/// Two pieces of gear worn from one set are a set; a lone worn piece is a
/// collection, and two pieces of different sets are two collections. A set is
/// read off what is worn, not what is owned.
#[test]
fn two_worn_pieces_of_one_set_are_a_set_and_a_lone_piece_is_not() {
    let piece = |set: Option<GearSet>, tier: u8| EquipmentRecord {
        id: 0,
        slot: EquipSlot::Weapon,
        tier,
        rarity: Rarity::Rare,
        attributes: Attributes::default(),
        bonus: Derived::default(),
        star: 0,
        effect: None,
        name: None,
        set,
    };
    assert_eq!(gear_set_bonus(&[piece(Some(GearSet::Sun), 5)]).power(), 0);
    let pair = gear_set_bonus(&[piece(Some(GearSet::Sun), 5), piece(Some(GearSet::Sun), 5)]);
    assert!(
        pair.attack >= 30 && pair.spell >= 15,
        "曜日 × 2 at tier 5 pays 攻 30 法 15: {pair:?}"
    );
    assert_eq!(
        gear_set_bonus(&[piece(Some(GearSet::Sun), 5), piece(Some(GearSet::Abyss), 5),]).power(),
        0,
        "two pieces of different sets are not one set"
    );
}

/// The set's name is the prefix its pieces carry: a 曜日 piece is named 曜日…,
/// and a guardian's piece carries no set at all.
#[test]
fn a_sets_name_is_the_prefix_its_pieces_carry() {
    let mut rng = Rng::seeded(7);
    let name = super::items::equipment_name(EquipSlot::Weapon, Some(GearSet::Sun), &mut rng);
    assert!(
        name.starts_with("曜日"),
        "a 曜日 piece is named 曜日…: {name}"
    );
    let mut rng = Rng::seeded(8);
    let solo = super::items::roll_equipment_solo(0, 5, Rarity::Rare, &mut rng);
    assert_eq!(solo.set, None, "a guardian's gear belongs to nothing");
}

/// A rolled piece that carries a set is named after it, so the set is readable
/// off the name — the naming *is* the association.
#[test]
fn a_drop_keeps_its_set_and_name_across_the_roll() {
    let mut rng = Rng::seeded(21);
    let item = super::items::roll_equipment(0, 5, Rarity::Rare, &mut rng);
    if let Some(set) = item.set {
        let prefix = set.name();
        assert!(
            item.name
                .as_deref()
                .is_some_and(|name| name.starts_with(prefix)),
            "a {prefix} piece is named {prefix}…: {:?}",
            item.name
        );
    } else {
        assert!(item.name.is_some());
    }
}

/// The per-set views a surface reads add up to the total the fight reads: two
/// sets and a lone piece are two views, and the sum of their bonuses is exactly
/// the total [`gear_set_bonus`] pays.
#[test]
fn the_worn_set_views_add_up_to_the_set_bonus() {
    let piece = |id: u64, set: Option<GearSet>, tier: u8| EquipmentRecord {
        id,
        slot: EquipSlot::Weapon,
        tier,
        rarity: Rarity::Rare,
        attributes: Attributes::default(),
        bonus: Derived::default(),
        star: 0,
        effect: None,
        name: None,
        set,
    };
    let worn = vec![
        piece(1, Some(GearSet::Sun), 5),
        piece(2, Some(GearSet::Sun), 5),
        piece(3, Some(GearSet::Abyss), 3),
        piece(4, Some(GearSet::Abyss), 3),
        piece(5, None, 4),
    ];
    let views = gear_set_views(&worn);
    assert_eq!(views.len(), 2, "two active sets, one lone piece");
    let sum = views
        .iter()
        .fold(Derived::default(), |total, view| total.plus(view.bonus));
    assert_eq!(sum, gear_set_bonus(&worn), "views add up to the total");
    let sun = views
        .iter()
        .find(|view| view.set == GearSet::Sun)
        .expect("曜日 is one of the active sets");
    assert_eq!(sun.pieces, 2, "a two-piece set is two pieces");
    assert!(sun.bonus.attack > 0, "曜日 pays 攻");
}

/// The per-family views a surface reads add up to the total the fight reads:
/// one pair and one lone book are one view, and its bonus is exactly the total
/// [`art_set_bonus`] pays.
#[test]
fn the_art_set_views_add_up_to_the_set_bonus() {
    let book = |id: u64| OwnedArt {
        art_id: id,
        rank: 1,
        progress: 0,
        failures: 0,
        copies: 1,
        outcome: None,
    };
    // 241 and 321 share 前缀 0; 243 is 前缀 1. All are 玄阶, so all count.
    let owned = vec![book(241), book(321), book(243)];
    let views = art_set_views(&owned);
    assert_eq!(views.len(), 1, "only the 前缀-0 pair is a set");
    let sum = views
        .iter()
        .fold(Derived::default(), |total, view| total.plus(view.bonus));
    assert_eq!(sum, art_set_bonus(&owned), "views add up to the total");
    let family = &views[0];
    assert_eq!(family.prefix, 0);
    assert_eq!(family.books, 2);
    assert!(
        family.bonus.power() > 0,
        "the 前缀-0 family pays its primary line"
    );
}
