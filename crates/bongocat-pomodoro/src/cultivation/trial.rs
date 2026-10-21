//! 试炼: the tower climbed at home, a few floors a day.
//!
//! Exploration is the road and this is the courtyard: the walk is rationed by a
//! cooldown because the cultivator is away for it, and the trial is rationed by
//! an attempt count because the cultivator stays. The two are mutually
//! exclusive for that reason — one takes the day's absence, the other fills it —
//! so a save cannot farm both at once.
//!
//! A floor is a *level*, not a separate difficulty scale: the warden of floor
//! nine stands where a naked cultivator of level nine stands, softened to
//! [`TRIAL_OPPONENT_PERMILLE`] so the first floors are an on-ramp rather than a
//! coin toss. That is what makes the tower read as a benchmark — the floor a
//! save can hold is its power said as a rung of the ladder — and it is why the
//! reward tier comes from the floor's own realm: climbing is how a save reaches
//! the goods of a realm it has not entered yet.

use super::{Attributes, MAX_LEVEL, Realm, Rng, base_attributes, duel::Fighter};

/// How many floors one local day offers.
///
/// Five is a short sitting rather than a second job: enough that a frontier
/// floor can be read as "nearly there" and pushed at, few enough that the tower
/// never replaces the pomodoro as the thing the day is made of.
pub const TRIAL_ATTEMPTS_PER_DAY: u32 = 5;

/// The top floor. The tower is the ladder's own length, so the last floor is
/// the last level: a save that clears it has outgrown the tower rather than
/// found its edge.
pub const TRIAL_MAX_FLOOR: u16 = MAX_LEVEL;

/// What a floor's warden is worth against a naked cultivator of that level, in
/// parts per thousand.
///
/// Nine tenths: a cultivator standing on their own level wins most floors and
/// loses the ones at the frontier, which is what turns "how far can I climb"
/// into a question the next piece of gear answers. Scaling it with the floor
/// instead would make the tower a second growth curve to balance; holding it
/// flat keeps one number to tune.
pub const TRIAL_OPPONENT_PERMILLE: u32 = 900;

/// What a cleared floor turns up, in parts per thousand, rolled once: a seed
/// most often, gear sometimes, a 功法 almost never. The four outcomes are
/// exclusive, and the rest of the thousand is a floor that paid nothing but the
/// climb.
///
/// Deliberately leaner per attempt than a walk: five attempts a day against one
/// walk every twelve hours means a per-attempt price near a walk's would make
/// the tower the only faucet worth turning.
pub const TRIAL_SEED_PERMILLE: u64 = 300;
pub const TRIAL_GEAR_PERMILLE: u64 = 60;
pub const TRIAL_ART_PERMILLE: u64 = 10;

/// The catalog key naming the thing standing on a floor. A surface translates
/// it; the domain never holds a translated word.
pub const TRIAL_OPPONENT_NAME: &str = "cultivation.trial.opponent";

/// The level a floor stands for: one-based, clamped to the ladder.
///
/// Floor one is level one — 炼气·1 层 1 阶 — which is the rung the tower starts
/// at, so the climb and the ladder are read on the same scale instead of on two.
pub fn floor_level(floor: u16) -> u16 {
    floor.clamp(1, TRIAL_MAX_FLOOR)
}

/// The realm a floor stands in, which is what its reward tier is read from.
pub fn floor_realm(floor: u16) -> Realm {
    Realm::from_level(floor_level(floor))
}

/// What a floor's warden is worth, as the page prints it beside the floor.
///
/// Read the same way the guardian's is: the power of a naked cultivator at that
/// level, scaled by [`TRIAL_OPPONENT_PERMILLE`], so the number the panel shows
/// and the fighter the duel settles are one ladder's arithmetic.
pub fn warden_power(floor: u16) -> u32 {
    let naked = base_attributes(floor_level(floor)).power();
    naked.saturating_mul(TRIAL_OPPONENT_PERMILLE) / 1_000
}

/// The warden as a set of attributes: a naked cultivator of the floor's level,
/// softened by [`TRIAL_OPPONENT_PERMILLE`].
///
/// Scaling the attributes rather than the derived numbers keeps the warden on
/// the ladder's own shape — the same hit and evade curve, the same health per
/// 力量 — so a floor is "the same cultivator, a little less of them" instead of
/// a hand-patched stat block.
fn warden_attributes(floor: u16) -> Attributes {
    let naked = base_attributes(floor_level(floor));
    let scale = |value: i32| value.saturating_mul(TRIAL_OPPONENT_PERMILLE as i32) / 1_000;
    Attributes::new(
        scale(naked.root),
        scale(naked.strength),
        scale(naked.mana),
        scale(naked.agility),
    )
}

/// The warden of a floor, for the fight that settles it.
pub fn warden_fighter(floor: u16) -> Fighter {
    Fighter::from_attributes(
        TRIAL_OPPONENT_NAME.to_owned(),
        warden_attributes(floor),
        None,
    )
}

/// What clearing a floor turned up.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TrialReward {
    Seed,
    Gear,
    Art,
    /// The floor paid the climb and nothing else.
    Nothing,
}

/// Roll what a cleared floor turns up.
///
/// One exclusive roll, the way a walk's quiet finding is rolled: a floor pays a
/// seed, or gear, or a 功法, or nothing, so the panel never has to explain why
/// two finds came out of one chest.
pub(crate) fn roll_reward(rng: &mut Rng) -> TrialReward {
    let roll = rng.below(1_000);
    if roll < TRIAL_SEED_PERMILLE {
        return TrialReward::Seed;
    }
    if roll < TRIAL_SEED_PERMILLE + TRIAL_GEAR_PERMILLE {
        return TrialReward::Gear;
    }
    if roll < TRIAL_SEED_PERMILLE + TRIAL_GEAR_PERMILLE + TRIAL_ART_PERMILLE {
        return TrialReward::Art;
    }
    TrialReward::Nothing
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three findings share one roll, so together they cannot fill it: the
    /// rest of the thousand is a floor that paid nothing.
    const _: () = assert!(TRIAL_SEED_PERMILLE + TRIAL_GEAR_PERMILLE + TRIAL_ART_PERMILLE <= 1_000);

    #[test]
    fn floor_one_is_the_first_rung_of_the_ladder() {
        assert_eq!(floor_level(1), 1);
        assert_eq!(floor_realm(1), Realm::QiRefining);
        // The first floor's warden is the first level's cultivator, softened.
        assert_eq!(
            warden_power(1),
            base_attributes(1).power() * TRIAL_OPPONENT_PERMILLE / 1_000
        );
        assert!(warden_power(1) < base_attributes(1).power());
    }

    #[test]
    fn a_deeper_floor_is_a_deeper_level_and_a_harder_warden() {
        let mut previous = 0;
        for floor in 1..=TRIAL_MAX_FLOOR {
            let power = warden_power(floor);
            assert!(
                power >= previous,
                "floor {floor} is softer than the one below it"
            );
            previous = power;
        }
        // Crossing a realm boundary is felt: the last layer of 炼气 is worth
        // less than the first of 筑基, because the realm jump is in the ladder.
        assert!(warden_power(101) > warden_power(100));
    }

    #[test]
    fn the_warden_stays_on_the_ladder_below_a_naked_save_of_the_same_level() {
        for floor in [1_u16, 50, 100, 500, TRIAL_MAX_FLOOR] {
            let naked = base_attributes(floor_level(floor));
            let warden = warden_attributes(floor);
            assert!(warden.power() < naked.power());
            assert!(warden.root < naked.root);
        }
    }

    #[test]
    fn a_cleared_floor_lands_on_every_reward_sooner_or_later() {
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..1_000_u64 {
            seen.insert(roll_reward(&mut Rng::seeded(seed)));
        }
        for reward in [
            TrialReward::Seed,
            TrialReward::Gear,
            TrialReward::Art,
            TrialReward::Nothing,
        ] {
            assert!(seen.contains(&reward), "{reward:?} never came up");
        }
        // A seed is the common find and a 功法 the rarest, the way a walk reads.
        let counts = |reward| {
            (0..5_000_u64)
                .filter(|seed| roll_reward(&mut Rng::seeded(*seed)) == reward)
                .count()
        };
        assert!(counts(TrialReward::Seed) > counts(TrialReward::Gear));
        assert!(counts(TrialReward::Gear) > counts(TrialReward::Art));
    }
}
