//! 掉落的两把尺子: how good a find is, and how deep it stands.
//!
//! Every source on the page — a walk, a fight, a floor, the shelf — hands over
//! the same two answers, and both of them are read off one number: **the
//! absolute strength of whoever the goods came from**. A fight reads its
//! opponent's power, everything quiet reads the cultivator's own. Two rules
//! follow from that, and they are the whole of this module:
//!
//! * **品质 is one ladder, scaled by strength.** The lowest quality is found at
//!   [`QUALITY_BASE_PER_BILLION`] — thirty-six percent — and every grade above
//!   it is a fifth of the one below. What the strength scales is the whole
//!   ladder: the same roll read through a stronger source is likelier to land at
//!   all, and likelier to land high, in exactly the ratio the ladder names.
//! * **阶级 is the rung that strength stands on.** A tier is a realm, so a tier
//!   is read off power: the goods of a body worth three realms' worth of power
//!   are three realms deep, whether that body is an opponent or the cultivator.
//!
//! Reading strength itself rather than strength *relative to the player* is
//! deliberate: an ambush by a stronger opponent has to pay better than a walk
//! taken by the same save, and a difference-relative rule would make both equal
//! the moment the save outgrew the road.

use super::{Rarity, Realm, Rng, base_attributes};

/// The chance the lowest quality is found at, in parts per billion: 36%.
///
/// The brief's own anchor. The three quarters of a percent of a billion left
/// over from the grade ladder's sum is what "nothing came of it" is made of, so
/// the numbers below are absolute chances and not weights: every one of them can
/// be read on the page without being divided by a total first.
pub const QUALITY_BASE_PER_BILLION: u64 = 360_000_000;

/// What each grade above the lowest is worth against the one below it: a fifth.
pub const QUALITY_STEP_DIVISOR: u64 = 5;

/// The domain a quality roll is measured against: one whole, in billionths.
pub const QUALITY_ROLL_DOMAIN: u64 = 1_000_000_000;

/// What absolute strength multiplies the whole ladder by, per band of power.
///
/// The bands are the realm caps said as power — 筑基圆满 is about three and a
/// half thousand, 化神圆满 about nine, 真仙圆满 about twenty — with the multiplier
/// climbing linearly inside a band so the number on the page moves where the
/// strength does. Half at the bottom, five at the top.
pub const POWER_FACTOR_BANDS: [(u32, u32); 4] = [
    (0, POWER_FACTOR_MIN_PERMILLE),
    (3_600, 1_200),
    (9_600, 2_200),
    (20_000, POWER_FACTOR_MAX_PERMILLE),
];

/// What the weakest body on the ladder multiplies the ladder by.
pub const POWER_FACTOR_MIN_PERMILLE: u32 = 500;

/// What the strongest body on the ladder multiplies it by.
pub const POWER_FACTOR_MAX_PERMILLE: u32 = 5_000;

/// How often a grade is found at all, in parts per billion.
///
/// The bottom of the ladder at rest, before any strength is folded in: 36%, a
/// fifth of that, a fifth of that again, and so on to the seven hundredths of one
/// per cent a 绝世 is, which rounds to two ten-thousandths of a part per billion
/// and still comes out of the roll as a number rather than as zero. Summed, the
/// eight come to 45% — so a strength of one leaves a little over half of every
/// roll as "nothing", which is what the multiplier is there to move.
pub fn quality_per_billion(rarity: Rarity) -> u64 {
    let mut value = QUALITY_BASE_PER_BILLION;
    for _ in 0..rarity.index() {
        value /= QUALITY_STEP_DIVISOR;
    }
    value
}

/// What one grade's chance is at a source of this strength, in parts per
/// billion — the figure a surface prints when it explains the roll.
///
/// The chance the roll *actually* pays, not the ladder's raw share: once the
/// strength has lifted the ladder past one whole, the roll is made over the
/// lifted sum and every grade's share of it shrinks with the others. A page that
/// printed the raw share would quote odds the roll does not run.
pub fn quality_chance_per_billion(rarity: Rarity, factor_permille: u32) -> u64 {
    let factor = u64::from(factor_permille.max(1));
    let scaled = |rarity: Rarity| quality_per_billion(rarity) * factor / 1_000;
    let total: u64 = Rarity::ALL.into_iter().map(scaled).sum();
    let ceiling = total.max(QUALITY_ROLL_DOMAIN);
    scaled(rarity) * QUALITY_ROLL_DOMAIN / ceiling.max(1)
}

/// What strength multiplies the quality ladder by, in parts per thousand.
///
/// Linear inside each band, held at both ends: half at nothing, five at the top
/// of the ladder. Every call is a pure function of the power handed in, so the
/// number a page prints beside a fight and the number the roll actually used
/// cannot be two different numbers.
pub fn power_factor_permille(power: u32) -> u32 {
    let first = POWER_FACTOR_BANDS[0];
    if power <= first.0 {
        return first.1;
    }
    for band in POWER_FACTOR_BANDS.windows(2) {
        let (low_power, low_factor) = band[0];
        let (high_power, high_factor) = band[1];
        if power <= high_power {
            let span = u64::from(high_power - low_power);
            let climbed = u64::from(power - low_power);
            let step = u64::from(high_factor - low_factor) * climbed / span;
            return low_factor + u32::try_from(step).unwrap_or(0);
        }
    }
    POWER_FACTOR_MAX_PERMILLE
}

/// The realm a body of this power stands in, as the one-based tier its goods are.
///
/// The first realm whose *cap* the strength has not passed: a body as strong as
/// 炼气's last layer carries 炼气's goods, and one a hair stronger carries 筑基's.
/// Nothing here looks at the cultivator's own realm, which is the point — a floor
/// or a duel that outclasses the save is how a save reaches the goods of a realm
/// it has not entered.
pub fn tier_for_power(power: u32) -> u8 {
    for realm in Realm::ALL {
        if realm_power(realm) >= power {
            return u8::try_from(realm.index() + 1).unwrap_or(1);
        }
    }
    u8::try_from(Realm::ALL.len()).unwrap_or(10)
}

/// What a naked body at a realm's last layer is worth: the strength a realm's
/// own goods are read against, and what the shelf rolls its stock at.
pub fn realm_power(realm: Realm) -> u32 {
    base_attributes(realm.cap_level()).power()
}

/// Roll one quality at a source of this strength, or nothing at all.
///
/// The roll is read in the ladder's own domain. When the strength lifts the
/// ladder's sum past one whole, the roll is made over the lifted sum — so a
/// strong enough source always pays *something*, and the shares of the grades
/// stay exactly the ladder's shares. When the sum is still short of one whole,
/// the remainder is the walk that found nothing.
pub(crate) fn roll_quality(rng: &mut Rng, factor_permille: u32) -> Option<Rarity> {
    let factor = u64::from(factor_permille.max(1));
    let scaled = |rarity: Rarity| quality_per_billion(rarity) * factor / 1_000;
    let total: u64 = Rarity::ALL.into_iter().map(scaled).sum();
    let ceiling = total.max(QUALITY_ROLL_DOMAIN);
    let roll = rng.below(ceiling.max(1));
    let mut edge = 0;
    for rarity in Rarity::ALL {
        edge += scaled(rarity);
        if roll < edge {
            return Some(rarity);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ladder is the brief's: a fifth per grade, in the order the eight
    /// qualities run.
    #[test]
    fn every_grade_above_the_lowest_is_a_fifth_of_the_one_below_it() {
        assert_eq!(quality_per_billion(Rarity::Common), 360_000_000, "36%");
        assert_eq!(
            quality_per_billion(Rarity::Fine),
            quality_per_billion(Rarity::Common) / 5
        );
        for pair in Rarity::ALL.windows(2) {
            let (lower, higher) = (quality_per_billion(pair[0]), quality_per_billion(pair[1]));
            assert!(
                lower / higher == 5,
                "{:?} at {lower} is not five times {:?} at {higher}",
                pair[0],
                pair[1]
            );
        }
        // The whole ladder at rest is under one: the rest of the billion is the
        // find that never happened.
        let total: u64 = Rarity::ALL.into_iter().map(quality_per_billion).sum();
        assert!(total < QUALITY_ROLL_DOMAIN, "{total} fills the whole roll");
        assert!(
            total > 420_000_000,
            "the ladder lost more than rounding: {total}"
        );
    }

    /// Strength only ever helps, and it spans the brief's half to five.
    #[test]
    fn strength_multiplies_the_ladder_from_a_half_to_five() {
        assert_eq!(power_factor_permille(0), 500);
        assert!(
            (500..=510).contains(&power_factor_permille(10)),
            "a body worth ten points is still at the bottom of the ladder"
        );
        assert_eq!(power_factor_permille(20_000), 5_000);
        assert_eq!(power_factor_permille(u32::MAX), 5_000, "held at the top");
        let mut previous = 0;
        for power in (0..30_000).step_by(50) {
            let factor = power_factor_permille(power);
            assert!(factor >= previous, "power {power} went backwards");
            assert!((500..=5_000).contains(&factor));
            previous = factor;
        }
        // Inside a band it climbs with the power rather than jumping at the edges.
        assert!(power_factor_permille(1_800) > power_factor_permille(900));
        assert!(power_factor_permille(6_000) > power_factor_permille(4_800));
    }

    /// A tier is a realm's rung: the cap of a realm carries that realm's goods,
    /// and a hair more carries the next one's.
    #[test]
    fn a_tier_is_the_rung_the_strength_stands_on() {
        for realm in Realm::ALL {
            let cap = realm_power(realm);
            assert_eq!(tier_for_power(cap), realm.index() as u8 + 1);
            assert_eq!(
                tier_for_power(cap.saturating_sub(1)),
                realm.index() as u8 + 1
            );
            if realm.next().is_some() {
                assert_eq!(tier_for_power(cap + 1), realm.index() as u8 + 2);
            }
        }
        assert_eq!(
            tier_for_power(0),
            1,
            "a body worth nothing is still rung one"
        );
        assert_eq!(
            tier_for_power(u32::MAX),
            10,
            "and nothing outruns the ladder"
        );
    }

    /// A stronger source lands more often *and* higher: both halves of the
    /// ladder move together, which is what one multiplier means.
    #[test]
    fn a_stronger_source_finds_more_and_finds_better() {
        let sample = |power: u32| {
            let factor = power_factor_permille(power);
            let mut rng = Rng::seeded(0x0000_5EED_1234_5678);
            let mut found = 0;
            let mut deep = 0;
            let draws = 20_000;
            for _ in 0..draws {
                if let Some(rarity) = roll_quality(&mut rng, factor) {
                    found += 1;
                    if rarity.index() >= Rarity::Immortal.index() {
                        deep += 1;
                    }
                }
            }
            (found, deep)
        };
        let (weak_found, weak_deep) = sample(0);
        let (strong_found, strong_deep) = sample(20_000);
        // 45% of the roll read through a half is 22.5%, and through a five the
        // ladder fills the whole roll, so every draw finds something.
        assert!(
            (4_000..=5_000).contains(&weak_found),
            "a half should find about 22.5% of twenty thousand: {weak_found}"
        );
        assert_eq!(
            strong_found, 20_000,
            "five fills the roll, so every draw finds something"
        );
        assert!(
            strong_deep > weak_deep,
            "a stronger source must find the deep grades more often: \
             {strong_deep} against {weak_deep}"
        );
    }

    /// The deep grades are found at the ladder's own ratio, not at a ratio the
    /// roll invented: a 5× source finds 史诗 about as often as 36% × 0.2³ × 5
    /// says, measured over a large sample.
    #[test]
    fn the_deep_grades_keep_the_ladders_ratio() {
        let factor = 5_000;
        let chance = quality_chance_per_billion(Rarity::Immortal, factor);
        let mut rng = Rng::seeded(0x1234_5678_9ABC_DEF0);
        let draws = 200_000;
        let hits = (0..draws)
            .filter(|_| roll_quality(&mut rng, factor) == Some(Rarity::Immortal))
            .count();
        let expected = draws as f64 * chance as f64 / QUALITY_ROLL_DOMAIN as f64;
        assert!(
            (hits as f64 - expected).abs() < expected * 0.3 + 4.0,
            "{hits} 史诗 against an expected {expected:.1}"
        );
        // And nothing above the ladder: no 绝世 every ten draws.
        let rarest = quality_chance_per_billion(Rarity::Transcendent, factor);
        assert!(
            rarest < 30_000,
            "a 绝世 at {rarest} per billion is not rare"
        );
    }
}
