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

use super::{MAX_LEVEL, Realm, Rng};

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
/// walk every ten hours means a per-attempt price near a walk's would make
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
/// The fight draws a fresh strength from the band each attempt, so the page
/// prints the band's *middle* — an honest estimate, not a promise: the number
/// beside the floor and the fighter the duel settles against are the same
/// spread, and the page reads its middle.
pub fn warden_power(floor: u16) -> u32 {
    // The midpoint of the band the floor's own rung is drawn from.
    super::opponent::floor_rung_power(floor).saturating_mul(1025) / 1_000
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
    use super::super::opponent::{challenge_band_permille, challenge_power, floor_rung_power};
    use super::*;

    /// The three findings share one roll, so together they cannot fill it: the
    /// rest of the thousand is a floor that paid nothing.
    const _: () = assert!(TRIAL_SEED_PERMILLE + TRIAL_GEAR_PERMILLE + TRIAL_ART_PERMILLE <= 1_000);

    /// The band is drawn as two uniforms averaged, so the middle is common and
    /// the ends are the rare, sharp fights — and it never leaves the chosen
    /// window around the cultivator: a fifth below, a quarter above.
    #[test]
    fn the_challenge_band_keeps_its_edges_and_its_middle() {
        let mut seen_min = u32::MAX;
        let mut seen_max = 0;
        let mut sum = 0u64;
        let count = 20_000u64;
        for seed in 0..count {
            let band = challenge_band_permille(&mut Rng::seeded(seed));
            assert!(
                (800..=1250).contains(&band),
                "seed {seed}: the band {band} left the chosen window"
            );
            seen_min = seen_min.min(band);
            seen_max = seen_max.max(band);
            sum += u64::from(band);
        }
        // The ends are the rare draws: approached across a long run.
        assert!(
            seen_min <= 810,
            "the easy end was never reached: {seen_min}"
        );
        assert!(
            seen_max >= 1240,
            "the hard end was never reached: {seen_max}"
        );
        // Two uniforms averaged centre on 1025: the middle is the common draw.
        let mean = sum / count;
        assert!((1_000..=1_050).contains(&mean), "mean {mean} not centred");
    }

    /// A floor reads the same against any save: the warden is the floor's own
    /// softened rung, drawn through the band, so the tower stays a benchmark —
    /// the fight is a draw, not a formality, but the floor does not move with
    /// who is climbing it.
    #[test]
    fn the_warden_is_drawn_around_the_floors_own_rung() {
        for floor in [1_u16, 50, 100, 500, TRIAL_MAX_FLOOR] {
            let rung = floor_rung_power(floor);
            for seed in 0..300u64 {
                let draw = challenge_power(floor, &mut Rng::seeded(seed));
                // The draw never leaves the band around the floor's own rung.
                assert!(
                    (rung * 800 / 1_000..=rung * 1250 / 1_000).contains(&draw),
                    "floor {floor} seed {seed}: draw {draw} left [0.8, 1.25] of the rung {rung}"
                );
                // The warden the duel settles against is levelled to that draw,
                // within a rung's own width — the ladder is the coarsest step
                // the draw can be read on, and the first floor is its edge.
                let warden = super::super::opponent::warden(floor, &mut Rng::seeded(seed));
                let slack = rung / 5;
                let w = warden.attributes.power();
                assert!(
                    w >= rung.saturating_mul(800) / 1_000 - slack
                        && w <= rung.saturating_mul(1250) / 1_000 + slack,
                    "floor {floor} seed {seed}: warden {w} left the band around the rung {rung}"
                );
                assert!(warden.fighter.health > 0);
            }
        }
    }

    /// The tower is a ladder: a deeper floor is a harder rung, so the climb only
    /// ever reads one direction.
    #[test]
    fn a_deeper_floor_is_a_harder_rung() {
        let mut previous = 0;
        for floor in 1..=TRIAL_MAX_FLOOR {
            let rung = floor_rung_power(floor);
            assert!(
                rung >= previous,
                "floor {floor}: rung {rung} below {previous}"
            );
            previous = rung;
        }
        // A realm crossing is felt in the rung itself.
        assert!(floor_rung_power(101) > floor_rung_power(100));
        // The printed estimate is the rung through the band's middle, so it
        // climbs the same ladder and never goes down either.
        let mut prev = 0;
        for floor in 1..=TRIAL_MAX_FLOOR {
            let p = warden_power(floor);
            assert!(p >= prev, "floor {floor}: printed {p} below {prev}");
            prev = p;
        }
    }

    #[test]
    fn floor_one_is_the_first_rung_of_the_ladder() {
        assert_eq!(floor_level(1), 1);
        assert_eq!(floor_realm(1), Realm::QiRefining);
        // The first floor is a fresh save's first contest, not a wall: the
        // band's hard end over the rung is the ceiling.
        let rung = floor_rung_power(1);
        let warden = super::super::opponent::warden(1, &mut Rng::seeded(0));
        let ceiling = rung * 1250 / 1_000 + rung / 10;
        assert!(
            warden.attributes.power() <= ceiling,
            "floor one is a wall rather than an on-ramp: {}",
            warden.attributes.power()
        );
        assert!(warden.fighter.health > 0);
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
