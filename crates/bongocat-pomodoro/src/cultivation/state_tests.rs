use super::super::{
    AptitudeHerb, DISCIPLINE_STEP_CAP, DISCIPLINE_STEP_PERMILLE, Discipline, ExplorationHours,
    GROWTH_MULTIPLIER_CAP_PERMILLE, LAYERS_PER_STAGE, STAGE_COUNT, discipline_bonus,
    discipline_cap, growth_permille, scaled_growth, session_growth, threshold_for,
};
use super::*;
use crate::CalendarDay;

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

/// An even hundred holds a body until its 洗髓丹 are paid: the herb is not
/// burned against a gate it did not open, and the pill count is the number
/// the brief names — 200 costs two, 300 three.
#[test]
fn an_even_hundred_holds_a_body_until_its_pills_are_paid() {
    let make = |pills: u32| {
        Cultivation::new(CultivationDocument {
            aptitude_tenths: Some(1_999),
            insight_herbs: 1,
            marrow_pills: pills,
            ..CultivationDocument::default()
        })
    };
    // 199.9 plus any 悟心草 crosses 200, which the brief prices at two pills.
    let mut poor = make(1);
    assert_eq!(
        poor.use_aptitude_herb(AptitudeHerb::Insight, 10),
        Err(TradeRefusal::NotEnoughMarrow)
    );
    assert_eq!(poor.aptitude_tenths(), 1_999, "still stuck at 199.9");
    assert_eq!(poor.aptitude_herbs()[0], 1, "the herb is not burned");

    let mut rich = make(2);
    let used = rich
        .use_aptitude_herb(AptitudeHerb::Insight, 10)
        .expect("two pills open the two-hundredth");
    assert_eq!(used.marrow_spent, 2);
    assert!(used.aptitude_tenths > 2_000, "the body crossed 200");
    assert_eq!(used.gained_tenths, used.aptitude_tenths - 1_999);
    assert_eq!(rich.aptitude_herbs()[0], 0);
    assert_eq!(rich.aptitude_herbs()[2], 0, "the pills were spent");
}

/// One herb of one grade pays one fixed number, not a roll: the same 慧心草
/// fed twice at the same grade pays the same 潜质 both times, and that
/// number is the row the satchel shows.
#[test]
fn the_same_herb_at_the_same_grade_always_pays_the_same() {
    let feed = |herb: AptitudeHerb| {
        let mut c = Cultivation::new(CultivationDocument {
            aptitude_tenths: Some(0),
            insight_herbs: u32::from(herb == AptitudeHerb::Insight),
            wisdom_herbs: u32::from(herb == AptitudeHerb::Wisdom),
            ..CultivationDocument::default()
        });
        c.use_aptitude_herb(herb, 10).expect("held at 地阶")
    };
    for herb in [AptitudeHerb::Insight, AptitudeHerb::Wisdom] {
        let first = feed(herb);
        let second = feed(herb);
        assert_eq!(
            first.gained_tenths, second.gained_tenths,
            "{herb:?} must not roll"
        );
        assert_eq!(
            first.gained_tenths,
            herb.gain_tenths_for(super::SeedGrade::Earth).unwrap(),
            "the gain is the row's one number"
        );
    }
}

/// A body with no herb in the bag is refused, and the refusal costs nothing.
#[test]
fn a_herb_nobody_holds_is_refused_rather_than_faked() {
    let mut cultivation = Cultivation::new(CultivationDocument {
        aptitude_tenths: Some(0),
        ..CultivationDocument::default()
    });
    assert_eq!(
        cultivation.use_aptitude_herb(AptitudeHerb::Insight, 10),
        Err(TradeRefusal::HerbNotHeld)
    );
    // The pill is not a herb: feeding one to the body is refused, because a
    // pill has nothing to feed — it opens a gate, and that is
    // [`Cultivation::use_marrow_pill`].
    assert_eq!(
        cultivation.use_aptitude_herb(AptitudeHerb::Marrow, 10),
        Err(TradeRefusal::HerbNotHeld)
    );
    assert_eq!(cultivation.aptitude_tenths(), 0);
}

/// The pill on its own buys the gate ahead, at the price the crossing asks:
/// one price for one gate, whether a herb or a hand opens it.
#[test]
fn a_pill_on_its_own_opens_the_gate_ahead_at_that_gates_price() {
    let mut cultivation = Cultivation::new(CultivationDocument {
        aptitude_tenths: Some(50),
        marrow_pills: 3,
        ..CultivationDocument::default()
    });
    assert_eq!(cultivation.view().marrow_gate_cost, Some(1));
    let first = cultivation
        .use_marrow_pill(10)
        .expect("the first hundred is one pill");
    assert_eq!(first.marrow_spent, 1);
    assert_eq!(
        first.aptitude_tenths, 1_000,
        "carried to the hundred itself"
    );
    assert_eq!(cultivation.aptitude_herbs()[2], 2, "one pill was spent");
    // The line is written, so the satchel and the log agree about the jump.
    assert!(
        matches!(
            cultivation.view().events.first().map(|event| &event.detail),
            Some(EventDetail::Aptitude { marrow: 1, .. })
        ),
        "the pill is logged as a 潜质 move"
    );
    // The gate ahead is now the two-hundredth, and it costs two.
    assert_eq!(cultivation.view().marrow_gate_cost, Some(2));
    let second = cultivation
        .use_marrow_pill(10)
        .expect("the two-hundredth costs two");
    assert_eq!(second.marrow_spent, 2);
    assert_eq!(second.aptitude_tenths, 2_000);
    assert_eq!(cultivation.aptitude_herbs()[2], 0);
    // Nothing left to pay with: the refusal costs nothing.
    assert_eq!(
        cultivation.use_marrow_pill(10),
        Err(TradeRefusal::NotEnoughMarrow)
    );
    assert_eq!(cultivation.aptitude_tenths(), 2_000);
}

/// The top of the band has no gate past it, so there is nothing to buy and
/// no pill is burned finding that out.
#[test]
fn a_body_at_the_top_of_the_band_has_no_gate_left_to_buy() {
    let mut cultivation = Cultivation::new(CultivationDocument {
        aptitude_tenths: Some(crate::cultivation::APTITUDE_MAX_TENTHS),
        marrow_pills: 9,
        ..CultivationDocument::default()
    });
    assert_eq!(cultivation.view().marrow_gate_cost, None);
    assert_eq!(
        cultivation.use_marrow_pill(10),
        Err(TradeRefusal::AptitudeCapped)
    );
    assert_eq!(cultivation.aptitude_herbs()[2], 9, "no pill is burned");
}

/// The 潜质 an art asks of the body is the one the page shows, and it is
/// three times the printed cost at zero.
#[test]
fn aptitude_sets_what_a_rank_asks_of_the_body() {
    let cultivation = Cultivation::new(CultivationDocument {
        aptitude_tenths: Some(0),
        arts: vec![OwnedArt {
            art_id: 1,
            rank: 1,
            progress: 0,
            outcome: None,
            failures: 0,
            copies: 1,
        }],
        ..CultivationDocument::default()
    });
    let view = cultivation.view();
    let art = view.arts.first().expect("the book is on the shelf");
    assert_eq!(art.required, 3, "the printed 1 costs a zero-潜质 body 3");
    assert_eq!(
        art.success_permille,
        super::super::aptitude::base_success_permille(art.grade),
        "the rate is the grade's own at zero 潜质 and no failures"
    );
}

/// A cultivator at the first realm's last layer, so a guardian is offered.
/// A finished pomodoro buys one 试炼 chance, ten at most are banked, and a
/// banked chance buys a fight — with no clock anywhere in the bargain.
#[test]
fn a_session_buys_a_chance_and_a_chance_buys_a_fight() {
    let mut cultivation = Cultivation::default();
    assert!(
        cultivation.proving(1_000).is_none(),
        "an empty bank buys no fight"
    );
    assert_eq!(cultivation.last_refusal, Some(Refusal::ProvingSpent));

    // Twelve finished sessions, one local day: the bank fills to its cap
    // and stays there rather than growing past it.
    for step in 0..12 {
        cultivation.on_session_completed(1_000 + step * 10, day(1), 25);
    }
    let filled = cultivation.view();
    assert_eq!(filled.proving_tokens, TRIAL_CHANCES_CAP, "ten at most");
    assert_eq!(filled.proving_token_cap, TRIAL_CHANCES_CAP);

    let outcome = cultivation
        .proving(2_000)
        .expect("a banked chance buys a fight");
    assert!(!outcome.duel.rounds.is_empty(), "a fight has blows");
    assert_eq!(
        cultivation.view().proving_tokens,
        TRIAL_CHANCES_CAP - 1,
        "the chance is spent"
    );
    assert_eq!(cultivation.view().provings, 1);
    assert!(
        cultivation.view().last_proving.is_some(),
        "the popup has a report to replay"
    );
}

/// A won 试炼 pays a purse of its own, and the log says so.
///
/// The courtyard used to pay its exclusive roll and nothing else, so a clean
/// win whose roll came up empty read as "试炼获胜。" and a chance spent for
/// free. Every other fight pays a purse; this one does now too.
#[test]
fn a_won_proving_pays_a_purse_the_log_names() {
    let mut paid = None;
    for step in 0..40i64 {
        let mut cultivation = Cultivation::new(CultivationDocument {
            proving_tokens: 1,
            ..CultivationDocument::default()
        });
        let before = cultivation.view().stones;
        let Some(outcome) = cultivation.proving(1_000 + step * 7) else {
            continue;
        };
        if !outcome.duel.won {
            continue;
        }
        let purse = outcome.found.stones.expect("a win pays a purse");
        assert!(purse > 0, "a purse is never empty");
        assert_eq!(
            cultivation.view().stones,
            before + purse,
            "the purse is credited to the cultivator"
        );
        match cultivation
            .view()
            .events
            .first()
            .map(|event| event.detail.clone())
        {
            Some(EventDetail::Proving {
                won: true,
                stones: Some(logged),
                ..
            }) => assert_eq!(logged, purse, "the line names the purse it paid"),
            other => panic!("the newest line is the bout: {other:?}"),
        }
        paid = Some(purse);
        break;
    }
    assert!(paid.is_some(), "forty bouts should win at least one");
}

/// 试炼 is fought at home: a walk that has not come back closes the courtyard
/// even when a chance is banked.
#[test]
fn a_proving_waits_for_the_walk_to_come_home() {
    let mut cultivation = Cultivation::default();
    for step in 0..3 {
        cultivation.on_session_completed(1_000 + step * 10, day(1), 25);
    }
    let tokens = cultivation.view().proving_tokens;
    assert!(tokens > 0, "three sessions banked three chances");
    cultivation.explore(2_000, ExplorationHours::Four);
    assert!(
        cultivation.proving(2_000 + 3_600).is_none(),
        "an unreturned walk closes the courtyard"
    );
    assert_eq!(cultivation.last_refusal, Some(Refusal::ProvingOut));
    assert_eq!(
        cultivation.view().proving_tokens,
        tokens,
        "a refused 试炼 spends no chance"
    );
    let home = 2_000 + 4 * 3_600;
    assert!(
        cultivation.proving(home).is_some(),
        "back home, the chance buys its fight"
    );
}

/// 试炼 closes with a job in hand, the way the tower does: both are fights
/// fought at home, and a job is hours of the same day as a walk. The wall
/// falls when the hours are up — before the tick has come round to pay the
/// wage — so a fight refused on a job that is already done would be reading
/// the clock wrong.
#[test]
fn a_proving_waits_for_the_job_to_come_home() {
    let mut cultivation = Cultivation::new(CultivationDocument {
        proving_tokens: 3,
        ..CultivationDocument::default()
    });
    let hours = ExplorationHours::from_hours(4);
    let out = 2_000 + hours.seconds();
    cultivation.explore(2_000, hours);
    cultivation.settle_walk(out);
    let run = cultivation
        .accept_job(0, out + 1, 1)
        .expect("the job is taken once the walk is home");
    assert_eq!(
        cultivation.proving_blocked(out + 60),
        Some(ProvingBlocked::Working),
        "the job in hand is the wall the page should print"
    );
    assert!(
        cultivation.proving(out + 60).is_none(),
        "an unreturned job closes the courtyard too"
    );
    assert_eq!(cultivation.last_refusal, Some(Refusal::Working));
    assert_eq!(
        cultivation.view().proving_tokens,
        3,
        "a refused 试炼 spends no chance"
    );
    // The hours are up: no wall left, even before the wage has been paid.
    assert_eq!(
        cultivation.proving_blocked(run.ends_unix),
        None,
        "the clock is home"
    );
    assert!(
        cultivation.proving(run.ends_unix).is_some(),
        "back home, the chance buys its fight"
    );
    assert_eq!(
        cultivation.view().proving_tokens,
        2,
        "and this one spent its chance"
    );
}

/// The thousandth: one finished pomodoro in a thousand pays something beside
/// its growth and its ordinary find.
#[test]
fn a_pomodoro_in_a_thousand_pays_a_bonus() {
    assert_eq!(SESSION_BONUS_PERMILLE, 1, "千分之一");
    // The roll itself, drawn often enough to see every shape it can take.
    let mut cultivation = a_bare_save_at(40);
    let mut kinds = std::collections::BTreeSet::new();
    let mut rng = Rng::seeded(0x0000_5EED_B0A7_1234);
    for _ in 0..2_000 {
        let strength = Strength::of(cultivation.power());
        if let Some(drop) = cultivation.roll_bonus(strength, &mut rng, 1_000) {
            kinds.insert(match drop {
                Drop::Equipment(_) => 0,
                Drop::Seed { .. } => 1,
                Drop::Art { .. } => 2,
                Drop::Herb { .. } => 3,
                Drop::Stones(_) => 4,
            });
        }
    }
    assert_eq!(kinds.len(), 5, "every shape of the jackpot is reachable");
    // And the rate it is rolled at, measured rather than asserted: one in a
    // thousand, over a hundred thousand sessions' worth of rolls.
    let mut rng = Rng::seeded(0x0000_5EED_5EED_5EED);
    let hits = (0..100_000)
        .filter(|_| rng.below(1_000) < SESSION_BONUS_PERMILLE)
        .count();
    assert!(
        (60..=140).contains(&hits),
        "{hits} of a hundred thousand is not one in a thousand"
    );
}

/// A dropped seed carries a grade, and two grades of one patch are two
/// stacks: the grade is what the harvest pays at.
#[test]
fn a_dropped_seed_carries_its_grade() {
    let mut cultivation = Cultivation::default();
    let mut rng = Rng::seeded(7);
    let earth = cultivation.push_seed(Attribute::Root, 2, SeedGrade::Earth, &mut rng);
    let divine = cultivation.push_seed(Attribute::Root, 2, SeedGrade::Divine, &mut rng);
    assert_eq!(earth.count, 1);
    assert_eq!(divine.count, 1);
    assert_eq!(
        cultivation.document().seeds.len(),
        2,
        "two grades of one patch are two things to plant"
    );
    assert!(
        divine.sell_price > earth.sell_price,
        "{} against {}",
        divine.sell_price,
        earth.sell_price
    );
    assert!(
        cultivation
            .view()
            .seeds
            .iter()
            .any(|seed| seed.grade == SeedGrade::Divine)
    );
}

/// The ten 潜质 books are the only 功法 that move 潜质, and the number the
/// panel reads is the number the learning roll uses.
#[test]
fn only_the_aptitude_books_move_the_aptitude_reading() {
    for art_id in crate::cultivation::APTITUDE_ART_IDS {
        let cultivation = Cultivation::new(CultivationDocument {
            // A body born with nothing: the reading below is the books'.
            aptitude_tenths: Some(0),
            arts: vec![OwnedArt {
                art_id,
                rank: 1,
                progress: 0,
                outcome: None,
                failures: 0,
                copies: 1,
            }],
            ..CultivationDocument::default()
        });
        assert_eq!(
            cultivation.aptitude_tenths(),
            crate::cultivation::arts::aptitude_tenths_per_rank(art_id),
            "one 阶 of {art_id} is worth its grade's tenths"
        );
    }
    // Any other book pays nothing, however far it has climbed.
    let ordinary = Cultivation::new(CultivationDocument {
        aptitude_tenths: Some(0),
        arts: vec![OwnedArt {
            art_id: 1,
            rank: 2,
            progress: 0,
            outcome: None,
            failures: 0,
            copies: 1,
        }],
        ..CultivationDocument::default()
    });
    assert_eq!(ordinary.aptitude_tenths(), 0);
}

/// The length a walk was taken for is the length that holds the next one
/// off: four hours is owed again at four, not at ten.
#[test]
fn a_walk_is_held_for_its_own_length_and_no_longer() {
    let mut cultivation = Cultivation::default();
    cultivation.explore(1_000, ExplorationHours::Four);
    assert!(
        cultivation
            .explore(1_000 + 4 * 3_600 - 1, ExplorationHours::Four)
            .is_none(),
        "three hours fifty-nine is too early"
    );
    assert!(
        cultivation
            .explore(1_000 + 4 * 3_600, ExplorationHours::Four)
            .is_some(),
        "four hours is a walk"
    );
    // The length is written down, so the page's countdown is the length the
    // walk actually took rather than a fixed ten hours.
    assert_eq!(cultivation.document().last_exploration_hours, 4);
    assert_eq!(
        cultivation.view().exploration_ready_at,
        Some(1_000 + 4 * 3_600 + 4 * 3_600)
    );
}

fn at_the_first_cap() -> Cultivation {
    Cultivation::new(CultivationDocument {
        growth: threshold_for(LAYERS_PER_STAGE),
        ..CultivationDocument::default()
    })
}

/// Four dimensions that add rather than compound, capped at four times a bare
/// session.
///
/// The cap is the whole promise of the formula: a save that trained
/// everything is worth four sessions, never sixteen, or the ladder would be a
/// different game for anyone who played long enough.
#[test]
fn four_dimensions_add_up_to_at_most_four_times_a_bare_session() {
    let trained = Realm::from_level(LAYERS_PER_STAGE * STAGE_COUNT as u16 - 1);
    assert_eq!(
        growth_permille(trained, 9, 40, 40, 40),
        GROWTH_MULTIPLIER_CAP_PERMILLE,
        "a fully trained save is capped, not multiplied twice"
    );
    assert_eq!(
        growth_permille(trained, 0, 0, 0, 0),
        1_000,
        "an untrained one is flat"
    );
}

/// 修炼 is capped by the realm, not by a count: a 阶 of 功法 is worth more the
/// deeper the realm is, which is what keeps the late game honest.
#[test]
fn a_trained_art_is_worth_more_the_deeper_the_realm_is() {
    let first = Realm::from_level(1);
    let deepest = Realm::from_level(LAYERS_PER_STAGE * STAGE_COUNT as u16 - 1);
    assert_eq!(discipline_cap(Discipline::Practice, first), 200);
    assert_eq!(discipline_cap(Discipline::Practice, deepest), 3_000);
    assert_eq!(
        discipline_bonus(Discipline::Practice, 9, 0, first),
        200,
        "炼气 stops at one fifth more than a bare session"
    );
    assert_eq!(
        discipline_bonus(Discipline::Practice, 9, 0, deepest),
        450,
        "and no further than four 阶 can carry it"
    );
}

/// The three action dimensions share one cap, so no single path is worth
/// farming forever.
#[test]
fn an_action_dimension_stops_at_twenty_acts() {
    let realm = Realm::from_level(1);
    assert_eq!(discipline_bonus(Discipline::Wander, 0, 0, realm), 0);
    assert_eq!(discipline_bonus(Discipline::Wander, 0, 1, realm), 50);
    assert_eq!(
        discipline_bonus(Discipline::Wander, 0, 20, realm),
        1_000,
        "twenty walks is the whole of it"
    );
    assert_eq!(
        discipline_bonus(Discipline::Wander, 0, 400, realm),
        1_000,
        "and four hundred is no more"
    );
}

/// What the page shows and what the session pays are the same number.
///
/// The page prints the growth the next session pays rather than the
/// multiplier behind it, so this reads that number and checks the session
/// against it: the two are the same formula read twice.
#[test]
fn a_finished_session_pays_what_the_page_says_it_pays() {
    let mut cultivation = Cultivation::default();
    // The page is asked first, before the session happens: it says what the
    // next pomodoro is worth, and the session then pays that.
    let promised = cultivation.view().next_session_growth;
    assert_eq!(
        promised,
        session_growth(1, 1),
        "an untrained save pays the base"
    );
    let flat = cultivation.on_session_completed(1_000, day(20), 25).growth;
    assert_eq!(
        flat, promised,
        "and the session pays exactly what the page printed"
    );

    // Twenty walks train 游历 to its cap; each walk is one cooldown apart, and
    // a walk that meets someone may train 战斗 as well — so the number the
    // page shows is read back rather than assumed.
    for step in 0..20 {
        let at = 1_000 + step * WALK_SECONDS;
        cultivation.explore(at, ExplorationHours::Twelve);
        // A walk trains the 游历 dimension when it comes home, so the test
        // brings it home on the same clock the next walk starts on.
        cultivation.settle_walk(at + WALK_SECONDS);
    }
    let view = cultivation.view();
    assert!(
        view.next_session_growth > session_growth(1, 1),
        "walking has to be worth something: {}",
        view.next_session_growth
    );

    let reward = cultivation.on_session_completed(1_000_000, day(30), 25);
    assert_eq!(
        reward.growth, view.next_session_growth,
        "the session pays exactly what the page printed"
    );
}

/// A missed day is scaled by the same number, so a save that trained deep
/// also loses more for letting a day go.
#[test]
fn a_missed_day_costs_what_a_trained_save_earns() {
    let active = |explorations| CultivationDocument {
        growth: 100_000,
        explorations,
        last_active_day: Some(19),
        charged_through_day: Some(19),
        ..CultivationDocument::default()
    };
    let mut trained = Cultivation::new(active(20));
    let multiplier = growth_permille(trained.realm(), 0, 20, 0, 0);
    assert_eq!(multiplier, 2_000);
    let mut bare = Cultivation::new(active(0));
    assert_eq!(growth_permille(bare.realm(), 0, 0, 0, 0), 1_000);

    // Charged through yesterday: asking on the twentieth charges nothing,
    // because the twentieth is the day still being lived in.
    let charged = trained
        .apply_absence(day(21), day_unix(day(21)))
        .expect("a day was charged");
    let flat = bare
        .apply_absence(day(21), day_unix(day(21)))
        .expect("a day was charged");
    assert_eq!(charged.days, 1);
    assert_eq!(charged.charged, flat.charged * 2, "twice the bare penalty");
    assert_eq!(
        charged.charged,
        scaled_growth(flat.charged, multiplier),
        "and exactly what the multiplier says"
    );
}

/// The record is a window of eight, newest first, however many were fought.
///
/// Guardian fights cannot fill a window on their own: a win enters the next
/// realm and there is nothing left to fight until the cap is climbed again,
/// and a loss writes a twelve hour cooldown. So the window is filled the way a
/// real save fills it — one guardian fight, then the fights met on walks.
#[test]
fn the_record_keeps_the_last_eight_fights_newest_first() {
    let mut cultivation = at_the_first_cap();
    for step in 0..60 {
        let at = 100_000 + step * WALK_SECONDS;
        let _ = cultivation.explore(at, ExplorationHours::Twelve);
        cultivation.settle_walk(at + WALK_SECONDS);
    }
    // Fought last, so the guardian's line is the newest one: a window of eight
    // would otherwise push it out, which is the window working, not failing.
    let _ = cultivation.breakthrough(100_000 + 60 * WALK_SECONDS + 1);
    let view = cultivation.view();
    assert_eq!(
        view.duel_record.len(),
        DUEL_RECORD_CAP,
        "the window holds at eight"
    );
    assert!(
        view.duel_record
            .windows(2)
            .all(|pair| pair[0].at_unix > pair[1].at_unix),
        "newest first: {:?}",
        view.duel_record
            .iter()
            .map(|line| line.at_unix)
            .collect::<Vec<_>>()
    );
    assert!(
        view.duel_record
            .iter()
            .any(|line| line.kind == FightKind::Guardian),
        "the guardian fight is in the record"
    );
    assert!(
        view.duel_record
            .iter()
            .any(|line| line.kind == FightKind::Wanderer),
        "and so are the walks' fights"
    );
}

/// A win is what trains the 战斗 dimension, and a loss is not.
///
/// A guardian fight cannot carry this on its own — a fresh save loses it — so
/// the count is read from the fights actually fought and checked against the
/// dimension the growth formula reads.
#[test]
fn only_wins_train_the_battle_dimension() {
    let mut cultivation = at_the_first_cap();
    // One 外功 taken to its last 阶: a body that can win a road fight. At the
    // cap with every book still at rank zero the player swings bare-handed
    // against a wanderer carrying an art, and loses every meeting — which
    // would leave this test asserting nothing about wins at all.
    cultivation.document.arts.push(OwnedArt {
        art_id: 404,
        rank: 6,
        progress: 0,
        failures: 0,
        copies: 1,
        outcome: None,
    });
    cultivation.document.selected_art = Some(404);
    let _ = cultivation.breakthrough(1_000);
    assert_eq!(
        discipline_bonus(Discipline::Fight, 0, 0, cultivation.realm()),
        0,
        "a loss trains nothing"
    );

    for step in 0..40 {
        let at = 100_000 + step * WALK_SECONDS;
        let _ = cultivation.explore(at, ExplorationHours::Twelve);
        cultivation.settle_walk(at + WALK_SECONDS);
    }
    let view = cultivation.view();
    // The record is a window of eight, so the count read here is the
    // document's own — the same number the growth formula reads.
    let won = cultivation.document().battle_wins;
    assert!(won > 0, "forty walks should have won one: {view:?}");
    assert!(
        view.duel_record
            .iter()
            .any(|line| line.kind == FightKind::Wanderer),
        "and the record says who was met"
    );
    assert_eq!(
        discipline_bonus(Discipline::Fight, 0, won, view.realm),
        DISCIPLINE_STEP_PERMILLE * won.min(DISCIPLINE_STEP_CAP),
        "one step per win, and no more than the cap"
    );
}

/// The counters the four dimensions read are in the document, so a relaunch
/// does not quietly reset them.
#[test]
fn the_dimensions_survive_a_relaunch() {
    let cultivation = Cultivation::new(CultivationDocument {
        growth: 100_000,
        explorations: 7,
        battle_wins: 5,
        harvests: 3,
        ..CultivationDocument::default()
    });
    let realm = cultivation.realm();
    assert_eq!(discipline_bonus(Discipline::Wander, 0, 7, realm), 350);
    assert_eq!(discipline_bonus(Discipline::Fight, 0, 5, realm), 250);
    assert_eq!(discipline_bonus(Discipline::Fortune, 0, 3, realm), 150);
    assert_eq!(growth_permille(realm, 0, 7, 5, 3), 1_750);
}

/// A save standing on a level with nothing worn and nothing trained.
fn a_bare_save_at(level: u16) -> Cultivation {
    Cultivation::new(CultivationDocument {
        growth: threshold_for(level),
        realm: Realm::from_level(level).index() as u8,
        ..CultivationDocument::default()
    })
}

/// A 破境 re-rolls the same day's shelf in place, so a slot index bought
/// under the old realm is a position in a shelf nobody can see any more.
/// The bought slots are invalidated, and the good that now sits in that
/// slot is not struck through as already paid for.
#[test]
fn a_breakthrough_clears_the_shelves_bought_slots() {
    let mut cultivation = a_bare_save_at(20);
    cultivation.document.stones = 100_000;
    let today = 20_000;
    let slot = 0usize;
    cultivation
        .buy_from_shop(slot, today, 1_000)
        .expect("the purchase went through");
    assert!(
        cultivation.shop(today)[slot].bought,
        "the slot is struck through after the buy"
    );
    // The guardian falls: the realm lifts inside the same day, and the
    // shelf is rolled again from the new realm.
    let old_realm = cultivation.document.realm;
    cultivation.document.realm = old_realm.saturating_add(1);
    assert!(
        !cultivation.shop(today)[slot].bought,
        "a slot bought under the old realm is not marked on the new shelf"
    );
}

/// A save from before herbs carried a grade holds bare counts. They fold
/// into stacks at 地阶 — the anchor the old numbers were — and the counts
/// are spent, so the two stores never disagree about what is held.
#[test]
fn legacy_herb_counts_become_earth_stacks() {
    let cultivation = Cultivation::new(CultivationDocument {
        insight_herbs: 2,
        wisdom_herbs: 3,
        ..CultivationDocument::default()
    });
    let stacks = &cultivation.document.herb_stacks;
    assert_eq!(stacks.len(), 2, "two herbs, two stacks");
    let insight = stacks
        .iter()
        .find(|stack| stack.herb == super::super::AptitudeHerb::Insight)
        .expect("the 悟心草 stack");
    assert_eq!(insight.count, 2);
    assert_eq!(
        insight.grade,
        super::super::SeedGrade::Earth,
        "an old herb is 地阶"
    );
    let wisdom = stacks
        .iter()
        .find(|stack| stack.herb == super::super::AptitudeHerb::Wisdom)
        .expect("the 慧心草 stack");
    assert_eq!(wisdom.count, 3);
    assert_eq!(
        cultivation.document.insight_herbs, 0,
        "the legacy count is spent"
    );
    assert_eq!(cultivation.document.wisdom_herbs, 0);
    assert_eq!(
        cultivation.aptitude_herbs(),
        [2, 3, 0],
        "the total the satchel shows is unchanged"
    );
}

/// The tower is closed for two different reasons, and the page is told
/// which: out on a walk, and out of the day's floors.
#[test]
fn the_tower_opens_only_at_home_and_only_while_the_day_has_floors() {
    let mut cultivation = a_bare_save_at(20);
    cultivation.document.walk = Some(super::super::WalkInProgress::started(
        1_000,
        ExplorationHours::Four,
        0,
        1,
    ));
    assert_eq!(cultivation.trial_blocked(1_001, 9), Some(TrialBlocked::Out));
    assert!(
        cultivation.trial(1_001, 9).is_none(),
        "a walk that has not come back closes the tower"
    );
    assert_eq!(
        cultivation.view_on(1_001, 9).last_refusal.map(Refusal::key),
        Some("cultivation.trial.refuse_out")
    );

    // Home: five floors belong to the day, and the sixth does not.
    cultivation.document.walk = None;
    let mut climbed = 0;
    for step in 0..TRIAL_ATTEMPTS_PER_DAY {
        let now = 10_000 + i64::from(step) * 60;
        assert!(cultivation.trial(now, 9).is_some(), "floor {step} was owed");
        climbed += 1;
    }
    assert_eq!(climbed, TRIAL_ATTEMPTS_PER_DAY);
    assert_eq!(
        cultivation.trial_blocked(20_000, 9),
        Some(TrialBlocked::Drained)
    );
    assert!(cultivation.trial(20_000, 9).is_none());
    assert_eq!(
        cultivation
            .view_on(20_000, 9)
            .last_refusal
            .map(Refusal::key),
        Some("cultivation.trial.refuse_drained")
    );

    // The next local day hands out the same five.
    let view = cultivation.view_on(20_000, 10);
    assert_eq!(view.trial_attempts_left, TRIAL_ATTEMPTS_PER_DAY);
    assert!(view.trial_blocked.is_none());
    assert!(cultivation.trial(20_001, 10).is_some());
    assert_eq!(
        cultivation.trial_attempts_left(10),
        TRIAL_ATTEMPTS_PER_DAY - 1
    );
}

/// The door the log reads through is the same door the tower and the 试炼
/// ask: a job whose hours have passed is home whether or not the tick has
/// settled the wage yet, so the status line never reads "away" about a
/// cultivator the page already lets fight.
#[test]
fn the_door_is_home_the_moment_the_job_in_hand_comes_due() {
    let mut cultivation = a_bare_save_at(20);
    let offer = cultivation.jobs(9)[0];
    let now = 1_000;
    cultivation.document.work = Some(super::super::JobRun::started(offer, now));
    // While the job runs the cultivator is out.
    assert!(!cultivation.view_on(now, 9).at_home);
    assert!(!cultivation.view_on(now + offer.seconds() - 1, 9).at_home);
    // The instant the hours pass the door opens, before the tick pays.
    assert!(cultivation.view_on(now + offer.seconds(), 9).at_home);
}

/// A win moves the frontier up one floor; a loss costs the attempt and
/// leaves the frontier where it stood, so the tower is a wall rather than a
/// ladder that drops.
#[test]
fn a_win_moves_the_frontier_and_a_loss_holds_it() {
    let mut climbing = a_dressed_save_at(30);
    let won = climbing.trial(1_000, 9).expect("the first floor is owed");
    assert_eq!(won.floor, 1, "the climb starts at the first floor");
    assert!(
        won.duel.won,
        "a dressed level thirty save clears the first floor"
    );
    assert_eq!(climbing.document.trial_floor, 1);
    assert_eq!(climbing.trial_next_floor(), 2);

    let mut walled = a_bare_save_at(30);
    walled.document.trial_floor = TRIAL_MAX_FLOOR - 1;
    let lost = walled.trial(1_000, 9).expect("the top floor is still owed");
    assert_eq!(lost.floor, TRIAL_MAX_FLOOR);
    assert!(
        !lost.duel.won,
        "the top of the tower is not cleared by a bare level 30 save"
    );
    assert_eq!(
        walled.document.trial_floor,
        TRIAL_MAX_FLOOR - 1,
        "a loss costs the attempt, not the ground already held"
    );
    assert!(lost.found.is_empty(), "a loss finds nothing");

    // Every floor is behind this save now, and the tower says so.
    walled.document.trial_floor = TRIAL_MAX_FLOOR;
    assert_eq!(walled.trial_blocked(20_000, 9), Some(TrialBlocked::Top));
    assert_eq!(
        walled.view_on(20_000, 9).last_refusal,
        None,
        "reading the view is not a refusal"
    );
}

/// An overwhelmingly strong body sweeps the tower: ten floors clear in one
/// action, each paying its own reward, one attempt spent, and the log names
/// the stretch rather than one find.
#[test]
fn a_sweep_clears_ten_floors_and_pays_each() {
    let mut strong = a_bare_save_at(500);
    assert!(
        strong.trial_sweepable(),
        "a level five hundred save is no contest for the first floor"
    );
    let before = strong.document.trial_attempts_today;
    let sweep = strong
        .trial_sweep(1_000, 9)
        .expect("an overwhelmingly strong save may sweep");
    assert_eq!(sweep.from_floor, 1, "the sweep starts at the frontier");
    assert_eq!(sweep.floors, 10, "a sweep clears a full stretch");
    assert_eq!(
        strong.document.trial_floor, 10,
        "the frontier is ten floors on"
    );
    assert_eq!(
        strong.document.trial_attempts_today,
        before + 1,
        "the sweep spends one of the day's attempts"
    );
    assert_eq!(
        strong.last_trial_sweep.as_ref(),
        Some(&sweep),
        "the panel reads the same haul"
    );
    assert!(
        matches!(
            strong.document.events.first(),
            Some(EventRecord {
                detail: EventDetail::TrialSweep { floors: 10, .. },
                ..
            })
        ),
        "the log names the sweep, not one find"
    );

    // The other half: a body the warden is no contest *for* has nothing to
    // sweep, and is refused rather than silently doing nothing.
    let mut weak = a_bare_save_at(1);
    assert!(
        !weak.trial_sweepable(),
        "a level one save is not above the warden"
    );
    assert_eq!(
        weak.trial_sweep(1_000, 9),
        None,
        "a body that is no contest is refused"
    );
    assert_eq!(
        weak.view_on(1_000, 9).last_refusal,
        Some(Refusal::TrialNotSweepable),
        "the refusal names the real reason"
    );
}

/// A save at `level` wearing the four pieces of one quality, or nothing at all.
fn a_save_at(level: u16, gear: Option<Rarity>) -> Cultivation {
    a_save_at_seeded(level, gear, 0)
}

/// The same save with a different wardrobe drawn off the same table. A fresh
/// drop swings ten to thirty percent, so one draw is one roll of the dice:
/// the tower's three steps are a statement about the expectation, not about
/// whichever four pieces happened to land.
fn a_save_at_seeded(level: u16, gear: Option<Rarity>, wardrobe: u64) -> Cultivation {
    let realm = Realm::from_level(level);
    let mut document = CultivationDocument {
        growth: threshold_for(level),
        realm: realm.index() as u8,
        equipped: vec![None; SLOT_COUNT],
        ..CultivationDocument::default()
    };
    let Some(rarity) = gear else {
        return Cultivation::new(document);
    };
    let mut rng = Rng::seeded(u64::from(level) * 31 + rarity.index() as u64 + wardrobe);
    let mut slots = std::collections::BTreeSet::new();
    let mut id = 1;
    while slots.len() < SLOT_COUNT {
        let item = crate::cultivation::items::roll_equipment(
            id,
            realm.index() as u8 + 1,
            rarity,
            &mut rng,
        );
        if slots.insert(item.slot.index()) {
            document.equipped[item.slot.index()] = Some(item.id);
            document.equipment.push(item);
            id += 1;
        }
    }
    Cultivation::new(document)
}

/// A save at `level` in a set of common cloth: the four pieces a player of that
/// realm would be wearing.
fn a_dressed_save_at(level: u16) -> Cultivation {
    a_save_at(level, Some(Rarity::Common))
}

/// The tower is the ladder read sideways: a floor stands for a level, and its
/// warden is drawn from the band around that level's rung. Past floor 101 the
/// band is wide, and that width is what makes gear a slope rather than a cliff:
/// at its own level a naked body clears about three floors in ten, a body in
/// common cloth about five, a body in earth-grade cloth or better about eight.
///
/// These three numbers are the product requirement, so they are locked here: a
/// retune of the attributes, the band or the duel that drifts one of them out
/// of its window fails this. Each rate averages eight independent wardrobes,
/// so a single lean 成色 roll cannot speak for the whole table.
#[test]
fn the_deep_tower_reads_gear_as_three_steps() {
    let rate = |level: u16, floor: u16, gear: Option<Rarity>| {
        let runs = 200;
        const WARDROBES: u64 = 8;
        let mut wins = 0;
        for wardrobe in 0..WARDROBES {
            wins += (0..runs)
                .filter(|attempt: &usize| {
                    let mut cultivation = a_save_at_seeded(level, gear, wardrobe);
                    cultivation.document.trial_floor = floor - 1;
                    cultivation
                        .trial(1_000 + (*attempt as i64) * 977, 9)
                        .is_some_and(|outcome| outcome.duel.won)
                })
                .count();
        }
        wins * 100 / (runs * WARDROBES as usize)
    };
    for level in [101_u16, 200, 500, 700] {
        let bare = rate(level, level, None);
        let common = rate(level, level, Some(Rarity::Common));
        let elite = rate(level, level, Some(Rarity::Immortal));
        assert!(
            (22..=42).contains(&bare),
            "a bare level {level} save clears its own floor {bare}% of the time"
        );
        assert!(
            (40..=70).contains(&common),
            "a white-clad level {level} save clears its own floor {common}% of the time"
        );
        assert!(
            (62..=92).contains(&elite),
            "an earth-clad level {level} save clears its own floor {elite}% of the time"
        );
        assert!(
            bare < common && common < elite,
            "level {level}: gear stopped being a slope ({bare} / {common} / {elite})"
        );
    }
    // The first deep floor is the one the three numbers are quoted on, so it is
    // held tighter than the ladder as a whole.
    let bare = rate(101, 101, None);
    let common = rate(101, 101, Some(Rarity::Common));
    let elite = rate(101, 101, Some(Rarity::Immortal));
    assert!((18..=42).contains(&bare), "floor 101 bare {bare}%");
    assert!((38..=62).contains(&common), "floor 101 common {common}%");
    assert!((68..=92).contains(&elite), "floor 101 earth {elite}%");

    // A tenth past the save is already a bad bet: the tower's frontier is the
    // save's own level, and the floor above it is where the climb stops.
    for level in [300_u16, 700] {
        let past = (u32::from(level) * 110 / 100).min(u32::from(TRIAL_MAX_FLOOR)) as u16;
        let bare = rate(level, past, None);
        let common = rate(level, past, Some(Rarity::Common));
        assert!(
            bare <= 32,
            "a bare level {level} save clears floor {past} a tenth past it {bare}% of the time"
        );
        assert!(
            common <= 55,
            "a white-clad level {level} save clears floor {past} a tenth past it {common}% of the time"
        );
    }
}

/// The first hundred floors are the on-ramp and keep the odds they were tuned
/// with: a naked body there loses most of its own floors, and a body in cloth
/// clears its own floor almost every time. That is the tutorial, and the deep
/// band must not leak into it.
#[test]
fn the_shallow_tower_keeps_its_on_ramp() {
    let rate = |level: u16, gear: Option<Rarity>| {
        let runs = 200;
        let wins = (0..runs)
            .filter(|attempt: &usize| {
                let mut cultivation = a_save_at(level, gear);
                cultivation.document.trial_floor = level - 1;
                cultivation
                    .trial(1_000 + (*attempt as i64) * 977, 9)
                    .is_some_and(|outcome| outcome.duel.won)
            })
            .count();
        wins * 100 / runs
    };
    for level in [10_u16, 50, 100] {
        let bare = rate(level, None);
        let common = rate(level, Some(Rarity::Common));
        assert!(
            bare <= 60,
            "a bare level {level} save clears its own floor {bare}% of the time"
        );
        assert!(
            common >= 95,
            "a white-clad level {level} save clears its own floor only {common}% of the time"
        );
    }
}

/// Standing on a floor's own level is never enough to sweep it: the skip is for
/// a body that has outgrown the stretch, and the margin is three tenths over
/// what the floor prints.
#[test]
fn a_same_level_save_is_never_offered_the_sweep() {
    // The on-ramp is where a first set of cloth still doubles a body, so only
    // the naked save is held to the rule there.
    for level in [10_u16, 30, 100] {
        let mut save = a_bare_save_at(level);
        save.document.trial_floor = level - 1;
        assert!(
            !save.trial_sweepable(),
            "a bare level {level} save is offered the sweep at its own floor"
        );
    }
    for level in [101_u16, 300, 700] {
        for gear in [None, Some(Rarity::Common), Some(Rarity::Immortal)] {
            let mut save = a_save_at(level, gear);
            save.document.trial_floor = level - 1;
            assert!(
                !save.trial_sweepable(),
                "a level {level} save in {gear:?} gear is offered the sweep at its own floor"
            );
        }
    }
    // A body a realm ahead of the floor is exactly what the skip is for.
    let mut ahead = a_bare_save_at(300);
    ahead.document.trial_floor = 100;
    assert!(
        ahead.trial_sweepable(),
        "a level 300 save cannot sweep the hundredth floor"
    );
}

/// A save written when a region covered two realms stores five entries per
/// array; the ten-region read spreads each old region across the two realms it
/// used to cover, so a walker keeps the ground it had already walked.
#[test]
fn a_five_region_save_spreads_across_ten() {
    let stored = r#"{"explored_levels":[5,2,0,0,0],"region_progress":[400,0,0,0,0]}"#;
    let document: CultivationDocument =
        serde_json::from_str(stored).expect("the five-region shape still loads");
    assert_eq!(
        document.explored_levels[..4],
        [10, 0, 4, 0],
        "five old levels are ten new ones, and two old levels are four"
    );
    assert_eq!(
        document.region_progress[0], 400,
        "the old slot's progress rides the first realm"
    );
    assert_eq!(document.explored_levels.len(), 10);
}

/// The road is walked down the ladder, never up it: a region ahead of the
/// cultivator's own realm is refused, and one at or below it is offered.
#[test]
fn a_region_ahead_of_the_realm_is_closed() {
    let mut save = a_bare_save_at(10);
    let ahead = super::super::explore::Region::from_index(3);
    assert!(
        save.explore_region(1_000, ExplorationHours::Four, ahead)
            .is_none(),
        "a 炼气 save walked into 元婴's region"
    );
    assert_eq!(
        save.view_on(1_000, 9).last_refusal,
        Some(Refusal::RegionLocked),
        "the refusal names the reason"
    );
    let own = super::super::explore::Region::from_index(0);
    assert!(
        save.explore_region(2_000, ExplorationHours::Four, own)
            .is_some(),
        "the save's own region must stay open"
    );
}

/// Walking a region end to end pays its one-off haul once: a purse worth
/// twenty long walks and one piece of the realm's finest gear. Walking it again
/// pays the ordinary purses and no second haul, which is what makes a cleared
/// region worth returning to rather than a chest that refills.
#[test]
fn a_region_pays_its_haul_once() {
    let mut save = a_bare_save_at(100);
    let region = super::super::explore::Region::AzurePeaks;
    let mut at = 1_000_i64;
    for _ in 0..400 {
        let _ = save.explore_region(at, ExplorationHours::Twelve, region);
        save.settle_walk(at + ExplorationHours::Twelve.seconds());
        at += ExplorationHours::Twelve.seconds() + 10;
        if save.document.cleared_regions[0] {
            break;
        }
    }
    assert!(
        save.document.cleared_regions[0],
        "the region never reached its end"
    );
    assert_eq!(save.cleared_level(0), super::super::explore::MAX_LEVEL);
    let hauls = |save: &Cultivation| {
        save.document
            .events
            .iter()
            .filter(|event| matches!(event.detail, EventDetail::RegionCleared { .. }))
            .count()
    };
    assert_eq!(hauls(&save), 1, "the haul is paid once");
    let divine = save
        .document
        .equipment
        .iter()
        .filter(|item| item.rarity.is_divine())
        .count();
    assert_eq!(divine, 1, "the haul carried exactly one 神装");

    // Walking it again still works, and pays no second haul.
    let before = save.document.stones;
    for _ in 0..5 {
        let _ = save.explore_region(at, ExplorationHours::Twelve, region);
        save.settle_walk(at + ExplorationHours::Twelve.seconds());
        at += ExplorationHours::Twelve.seconds() + 10;
    }
    assert_eq!(hauls(&save), 1, "a cleared region pays its haul only once");
    assert!(
        save.document.explorations > 0,
        "the cleared region stays walkable"
    );
    let _ = before;
}

/// The day opens once, and once only: a relaunch the same afternoon is still
/// the same morning.
#[test]
fn the_day_opens_once_and_only_once() {
    let mut cultivation = a_bare_save_at(20);
    let first = cultivation.daily_event(1_000, 9).expect("the day opens");
    assert!(
        matches!(
            first.kind,
            DailyKind::Ambush | DailyKind::Gift | DailyKind::Trinket
        ),
        "the morning has to be one of the three kinds"
    );
    assert!(
        cultivation.daily_event(2_000, 9).is_none(),
        "a second tick on the same day opens nothing"
    );
    assert_eq!(cultivation.document.daily_event_day, Some(9));
    assert!(
        matches!(
            cultivation.view().events.first().map(|event| &event.detail),
            Some(EventDetail::Daily { .. })
        ),
        "the morning is on the page's log"
    );
    assert!(
        cultivation.daily_event(3_000, 10).is_some(),
        "the next day opens on its own"
    );
    assert_eq!(cultivation.view().daily_events, 2);
}

/// The morning can only add. Every kind pays something — a fight only if it
/// is won, which is the one branch that can come back empty-handed.
#[test]
fn a_morning_only_ever_adds() {
    for day_number in 0..60_i64 {
        let mut cultivation = a_bare_save_at(20);
        let before = cultivation.document.stones;
        let outcome = cultivation
            .daily_event(1_000 + day_number * 86_400, day_number)
            .expect("each day opens");
        assert!(
            cultivation.document.stones >= before,
            "the morning took money away"
        );
        match outcome.kind {
            DailyKind::Ambush => {
                assert!(outcome.duel.is_some(), "an ambush is a fight to replay");
                if outcome.won {
                    assert!(!outcome.found.is_empty(), "a won ambush pays something");
                } else {
                    assert!(outcome.found.is_empty(), "a lost ambush takes nothing");
                }
            }
            DailyKind::Gift | DailyKind::Trinket => {
                assert!(outcome.duel.is_none(), "only an ambush is a fight");
                assert!(
                    !outcome.found.is_empty(),
                    "a gift always hands something over"
                );
            }
        }
    }
}

/// The learned panel is ordered by the ring, not by the shelf: the three it
/// shows have to be the three being worked on.
#[test]
fn the_learned_panel_puts_the_recently_trained_first() {
    let mut cultivation = a_bare_save_at(20);
    cultivation.document.arts = vec![
        OwnedArt {
            art_id: 3,
            rank: 1,
            progress: 0,
            outcome: None,
            failures: 0,
            copies: 1,
        },
        OwnedArt {
            art_id: 7,
            rank: 1,
            progress: 0,
            outcome: None,
            failures: 0,
            copies: 1,
        },
    ];
    cultivation.select_art(Some(3));
    cultivation.train_selected(1_000);
    cultivation.select_art(Some(7));
    cultivation.train_selected(2_000);
    assert_eq!(
        cultivation.document.recent_arts.first(),
        Some(&7),
        "the last thing trained is at the front"
    );
    assert_eq!(
        cultivation.view().mastered_arts.first().map(|art| art.id),
        Some(7),
        "the ring orders the panel"
    );
    // Going back to the older book moves it back to the front.
    cultivation.select_art(Some(3));
    cultivation.train_selected(3_000);
    assert_eq!(
        cultivation.view().mastered_arts.first().map(|art| art.id),
        Some(3)
    );
}

/// The ring is a ring: it keeps the few, not every 功法 the save has met.
#[test]
fn the_recent_ring_forgets_the_oldest() {
    let mut cultivation = a_bare_save_at(20);
    for art_id in 1..=(RECENT_ARTS_CAP as u64 + 3) {
        cultivation.note_recent_art(art_id);
    }
    assert_eq!(cultivation.document.recent_arts.len(), RECENT_ARTS_CAP);
    assert_eq!(
        cultivation.document.recent_arts.first().copied(),
        Some(RECENT_ARTS_CAP as u64 + 3)
    );
    // Training the same book twice is one entry, not two.
    cultivation.note_recent_art(2);
    assert_eq!(
        cultivation
            .document
            .recent_arts
            .iter()
            .filter(|id| **id == 2)
            .count(),
        1
    );
    assert_eq!(cultivation.document.recent_arts.first().copied(), Some(2));
}

/// Every won fight pays a purse, and each source pays its own size: the
/// guardian the richest, a trial floor the leanest, a wanderer in between.
#[test]
fn every_fight_pays_a_purse_of_its_own_size() {
    // A save at the first realm's cap, far stronger than the guardian needs:
    // the point is the purse a win pays, not whether it can be won.
    let mut wins = 0;
    for attempt in 0..40_i64 {
        let mut cultivation = Cultivation::new(CultivationDocument {
            growth: threshold_for(LAYERS_PER_STAGE),
            allocated: Attributes::new(2_000, 2_000, 2_000, 2_000),
            ..CultivationDocument::default()
        });
        let before = cultivation.document.stones;
        let Some(outcome) = cultivation.breakthrough(1_000 + attempt * 977) else {
            break;
        };
        if outcome.duel.won {
            wins += 1;
            // The purse is opened at the realm the guardian guarded, which
            // is the one the win entered.
            let (min, max) = GUARDIAN_STONES.at(outcome.target.index() as u8 + 1);
            assert!(
                outcome
                    .stones
                    .is_some_and(|amount| (min..=max).contains(&amount)),
                "a won breakthrough pays the guardian's purse: {:?}",
                outcome.stones
            );
            assert!(cultivation.document.stones > before);
        }
    }
    assert!(wins > 0, "the reference save never won a breakthrough");

    // The tower's purse is the leanest of the four, and only a cleared floor
    // pays it.
    let mut floors = 0;
    for attempt in 0..40_i64 {
        let mut cultivation = a_bare_save_at(30);
        let before = cultivation.document.stones;
        let Some(outcome) = cultivation.trial(1_000 + attempt * 977, 9) else {
            break;
        };
        if outcome.duel.won {
            floors += 1;
            let (min, max) = TRIAL_STONES.at(floor_realm(outcome.floor).index() as u8 + 1);
            assert!(
                outcome
                    .found
                    .stones
                    .is_some_and(|amount| (min..=max).contains(&amount)),
                "a cleared floor pays the tower's purse"
            );
            assert!(cultivation.document.stones > before);
        } else {
            assert_eq!(
                cultivation.document.stones, before,
                "a failed floor pays nothing at all"
            );
        }
    }
    assert!(
        floors > 0,
        "the reference save never cleared the first floor"
    );
}

/// The same road pays more once the road is a deeper one: a 化神 region's purse
/// is worth several 炼气 ones, because the shelf it is spent on costs several
/// times as much there.
///
/// Read off the purse rather than off a walk's takings: what a walk actually
/// pockets is the purse times how far the body's advantage carried it, and a
/// fresh save in a shallow region is always at the top of that multiplier. The
/// purse itself is what "the deep road pays more" means.
#[test]
fn a_deeper_realm_pays_a_bigger_purse() {
    let purse = |region: super::super::explore::Region| {
        let (min, max) = super::super::explore::walk_purse(region, ExplorationHours::Twelve, 1)
            .at(region.tier());
        (min, max)
    };
    let shallow = purse(super::super::explore::Region::AzurePeaks);
    let deep = purse(super::super::explore::Region::VoidSanctum);
    assert!(
        deep.0 >= shallow.0 * 3,
        "a 化神 region pays {deep:?} against a 炼气 {shallow:?}"
    );
}

/// The trophy is the piece the guardian was wearing, and the 功法 is the one
/// it was swinging: a won fight hands over what lost to it, rather than
/// rolling two fresh things that were never on the field.
#[test]
fn a_won_breakthrough_hands_over_what_the_guardian_carried() {
    let mut cultivation = at_the_first_cap();
    let guardian = cultivation.guardian().expect("a guardian at the cap");
    let current = cultivation.realm();
    let rival = opponent::guardian(current, guardian.target);
    let mut rng = Rng::seeded(7);
    let fighter = cultivation.fighter(&mut rng);
    let report = duel(fighter, rival.fighter.clone(), &mut rng);
    cultivation.enter_realm(guardian, &rival, report, 1_000);
    let outcome = cultivation
        .last_breakthrough
        .clone()
        .expect("the realm was opened");
    let reward = outcome.reward.expect("a win pays a trophy");
    assert_eq!(
        reward.attributes, rival.gear[0].attributes,
        "the trophy is the piece it wore, not a fresh roll"
    );
    assert_eq!(reward.name, rival.gear[0].name);
    let art_id = rival.art.expect("a guardian always carries an art");
    assert_eq!(
        outcome.granted_art.as_deref(),
        Some(blueprint(art_id).name.as_str()),
        "the book is the one it swung"
    );
    assert!(
        cultivation
            .document
            .arts
            .iter()
            .any(|owned| owned.art_id == art_id),
        "and it lands in the satchel"
    );
}

/// A cultivator who held the realm back keeps the growth they earned, but the
/// part that was banked past the old realm's cap is only half carried into the
/// new one. The cap is the last level of the realm they just left; anything
/// above it is surplus, and surplus is taxed at 50%.
#[test]
fn a_breakthrough_taxs_surplus_growth_at_fifty_percent() {
    // Start at the first realm's cap with a surplus of growth above the cap.
    let first_cap_growth = threshold_for(LAYERS_PER_STAGE);
    let surplus = 2_000_u32;
    let mut cultivation = Cultivation::new(CultivationDocument {
        growth: first_cap_growth.saturating_add(surplus),
        ..CultivationDocument::default()
    });

    let old_cap = threshold_for(LAYERS_PER_STAGE);
    let new_floor = stage_floor(1);
    let guardian = cultivation.guardian().expect("a guardian at the cap");
    let current = cultivation.realm();
    let rival = opponent::guardian(current, guardian.target);
    let mut rng = Rng::seeded(11);
    let fighter = cultivation.fighter(&mut rng);
    let report = duel(fighter, rival.fighter.clone(), &mut rng);
    cultivation.enter_realm(guardian, &rival, report, 1_000);

    // The surplus is taxed at 50%: half of 2_000 is 1_000, added to the old
    // cap. The new floor is the realm they just opened, so the final growth
    // is the greater of the old cap plus half the surplus and the new floor.
    let expected = old_cap.saturating_add(surplus / 2).max(new_floor);
    assert_eq!(
        cultivation.document.growth, expected,
        "half the surplus is inherited, not all of it"
    );
}

/// A beaten wanderer's gear is its own: whatever a walk hands over is one of
/// the pieces the loser was wearing.
#[test]
fn a_beaten_wanderer_hands_over_its_own_gear() {
    let mut cultivation = a_bare_save_at(30);
    let rival = opponent::wanderer(cultivation.realm(), 0x5741_4E44_4552_0000);
    assert!(!rival.gear.is_empty(), "the probe needs it to wear gear");
    let worn: Vec<Attributes> = rival.gear.iter().map(|item| item.attributes).collect();
    let mut dropped = 0;
    for seed in 0..60_u64 {
        let mut rng = Rng::seeded(seed);
        let found = cultivation.award_wanderer_find(&rival, &mut rng, 1_000);
        if let Some(gear) = found.gear {
            assert!(
                worn.contains(&gear.attributes),
                "a walk paid a piece the wanderer was not wearing"
            );
            dropped += 1;
        }
    }
    assert!(dropped > 0, "no wanderer in sixty fights left its gear");
}

/// Two rows a day are cut, both inside the band, and the cut is the one the
/// purchase charges: the button's number is the price, or the badge is a
/// label on a shelf that never moved.
#[test]
fn two_rows_of_the_day_are_on_special() {
    let cultivation = a_bare_save_at(1);
    let shelf = cultivation.shop(9);
    let specials: Vec<&ShopOfferView> = shelf
        .iter()
        .filter(|row| row.discount_permille > 0)
        .collect();
    assert_eq!(specials.len(), 2, "the day marks two rows and no more");
    for row in &specials {
        assert!(
            (600..=800).contains(&row.discount_permille),
            "a special outside 6-8折: {}",
            row.discount_permille
        );
        assert!(
            row.price < row.list_price,
            "a special that costs the list price"
        );
        assert_eq!(
            row.price,
            row.list_price.saturating_mul(row.discount_permille) / 1_000,
            "the row's price is its own cut"
        );
    }
    // The same day shows the same shelf, cut and all: a special a reload
    // could change is not a special.
    assert_eq!(cultivation.shop(9), shelf);
    // And the next day rolls its own.
    let tomorrow = cultivation.shop(10);
    assert_ne!(
        tomorrow
            .iter()
            .map(|row| (row.slot, row.discount_permille))
            .collect::<Vec<_>>(),
        shelf
            .iter()
            .map(|row| (row.slot, row.discount_permille))
            .collect::<Vec<_>>(),
        "every day would carry the same two cuts"
    );
}

/// What the purchase charges is what the row asked.
#[test]
fn a_special_is_charged_at_its_cut() {
    let mut cultivation = Cultivation::new(CultivationDocument {
        stones: 100_000,
        ..CultivationDocument::default()
    });
    let today = 9;
    let shelf = cultivation.shop(today);
    let row = shelf
        .iter()
        .find(|row| row.discount_permille > 0)
        .expect("a shelf with a special on it");
    let before = cultivation.document.stones;
    cultivation
        .buy_from_shop(row.slot, today, 1_000)
        .expect("the purse covers it");
    let paid = before - cultivation.document.stones;
    assert_eq!(paid, row.price, "the row's number is the price charged");
    assert!(paid < row.list_price, "and the cut was real");
    assert_eq!(
        cultivation
            .view_on(1_000, today)
            .last_refusal
            .map(Refusal::key),
        None,
        "nothing was refused"
    );
}

/// Finishing a book does not burn the spares: they were never opened, so
/// they go back on the shelf as one unread book of the same 功法.
#[test]
fn finishing_a_book_leaves_its_spare_unread() {
    let mut cultivation = a_bare_save_at(20);
    let max_rank = blueprint(1).grade.ranks();
    cultivation.document.arts = vec![OwnedArt {
        art_id: 1,
        rank: max_rank,
        progress: 0,
        outcome: None,
        failures: 0,
        copies: 2,
    }];
    cultivation.document.selected_art = Some(1);
    cultivation.train_selected(1_000);
    let view = cultivation.view();
    assert!(
        view.mastered_arts.iter().any(|art| art.id == 1),
        "the 功法 is learned"
    );
    let left = view
        .arts
        .iter()
        .find(|art| art.id == 1)
        .cloned()
        .expect("the spare is still a book on the shelf");
    assert_eq!(left.copies, 1, "one unread copy left");
    assert_eq!(left.rank, 0, "an unopened book starts at the first page");
    assert_eq!(left.progress, 0, "with nothing banked");
    assert_eq!(left.failures, 0, "and nothing spent");
    assert!(
        !left.selected,
        "a finished session does not restart the reading"
    );
}
