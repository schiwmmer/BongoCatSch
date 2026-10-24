//! 潜质: the one number that decides how fast — and how reliably — 功法 are
//! learned.
//!
//! Two consequences hang off one value on purpose, because they answer the same
//! question from two sides: *how good is this body at learning a technique?* A
//! high 潜质 learns an 阶 in fewer pomodoros **and** fails fewer attempts, so a
//! user who invests in it feels one decision pay twice instead of two numbers
//! moving independently and cancelling out.
//!
//! The value is fractional and can start negative: a body is born somewhere on
//! a −10..+10 band, and the only way up is two rare herbs. Every hundred points
//! is a gate that only 洗髓丹 opens, which is what makes the top of the band a
//! project rather than a shopping spree.
//!
//! Stored in tenths (`i32`) rather than as a float: every herb pays a tenth of a
//! point at the smallest, a tenth is exactly representable in an integer, and a
//! save that carries `47` reads as 4.7 for ever instead of as the nearest
//! binary approximation of it.

use serde::{Deserialize, Serialize};

use super::{ArtGrade, Rng};

/// The lowest 潜质 a body can hold: −10.0.
pub const APTITUDE_MIN_TENTHS: i32 = -100;

/// The highest 潜质 the band reaches: 1000.0.
pub const APTITUDE_MAX_TENTHS: i32 = 10_000;

/// How far from zero a body starts: −10.0..=+10.0 inclusive.
pub const APTITUDE_BIRTH_SPAN_TENTHS: i32 = 100;

/// Every hundred points is a gate: 100, 200, … 1000.
pub const APTITUDE_GATE_TENTHS: i32 = 1_000;

/// The base the success rate is multiplied by per point of 潜质.
///
/// One percent per point: the whole band therefore spans about ×21 000 from
/// −10 to +1000, which is why the rate is capped — see
/// [`SUCCESS_CAP_PERMILLE`].
const SUCCESS_STEP: f64 = 1.01;

/// The base the required pomodoros are multiplied by per point of 潜质.
///
/// One percent off per point, the mirror of the success step: the two halves of
/// the number climb together, so a point bought is worth the same wherever it
/// is spent.
const SPEED_STEP: f64 = 0.99;

/// What a zero-潜质 body takes compared with the printed cost: three times.
const ZERO_APTITUDE_STUDY_MULTIPLE: f64 = 3.0;

/// The most a single attempt can be worth believing in, in parts per thousand.
///
/// A rate past a hundred percent is not a rate, and a rate of exactly a
/// thousand would make the roll a formality that still prints a die roll. Five
/// parts in a thousand of failure keeps 神阶 a *hard* thing even at 1000 潜质.
pub const SUCCESS_CAP_PERMILLE: u32 = 995;

/// The least a rate can be, in parts per thousand: a body at −10 is bad at a
/// 神阶 art, not incapable of ever reading it.
pub const SUCCESS_FLOOR_PERMILLE: u32 = 1;

/// What a run of failures at the *same* 阶 is worth, in parts per thousand.
///
/// The base rate multiplied by 1.01 once per failure already spent on this
/// 阶: the body learns by being wrong, so a 神阶 art that has failed twenty
/// times is no longer the same coin toss it was at the first attempt. The
/// count belongs to one 阶 of one 功法 — the caller resets it when the 阶
/// turns over — because an art is not easier for having been read badly at a
/// shallower rung.
///
/// An ever-growing rate is still a rate: it is held to
/// [`SUCCESS_CAP_PERMILLE`] so the die roll never becomes a formality.
pub fn compensated_success_permille(base: u32, failures: u32) -> u32 {
    // A save with a corrupt count cannot be allowed to overflow `powi` into an
    // infinity the cast would then have to guess at.
    let exponent = i32::try_from(failures.min(10_000)).unwrap_or(10_000);
    let rate = f64::from(base) * SUCCESS_STEP.powi(exponent);
    (rate.round() as u32).clamp(SUCCESS_FLOOR_PERMILLE, SUCCESS_CAP_PERMILLE)
}

/// The 潜质 a newborn body carries, rolled once and then written down.
pub fn birth_aptitude(rng: &mut Rng) -> i32 {
    let span = u64::try_from(APTITUDE_BIRTH_SPAN_TENTHS * 2 + 1).unwrap_or(1);
    (rng.below(span) as i32) - APTITUDE_BIRTH_SPAN_TENTHS
}

/// The stored tenths held to the band.
pub fn clamp_tenths(tenths: i32) -> i32 {
    tenths.clamp(APTITUDE_MIN_TENTHS, APTITUDE_MAX_TENTHS)
}

/// 潜质 as a point value: 47 tenths reads as 4.7.
pub fn points(tenths: i32) -> f32 {
    tenths as f32 / 10.0
}

/// 潜质 as a point value, from its stored tenths.
pub fn points_f64(tenths: i32) -> f64 {
    f64::from(tenths) / 10.0
}

/// The success rate one attempt at the given grade carries at this 潜质, in
/// parts per thousand.
///
/// The 潜质-zero ladder is authored: 黄 50%, 玄 42%, 地 35%, 天 20%, 神 5%. The
/// 玄 step is the one interpolation in the row — the brief names 黄/地/天/神 —
/// and it sits between its neighbours rather than beside one of them.
pub fn success_permille(grade: ArtGrade, aptitude_tenths: i32) -> u32 {
    let base = f64::from(base_success_permille(grade)) / 1_000.0;
    let chance = base * SUCCESS_STEP.powf(points_f64(aptitude_tenths));
    let permille = (chance * 1_000.0).round();
    let clamped = permille.clamp(
        f64::from(SUCCESS_FLOOR_PERMILLE),
        f64::from(SUCCESS_CAP_PERMILLE),
    );
    clamped as u32
}

/// The success rate a body with no 潜质 at all carries, by grade.
pub const fn base_success_permille(grade: ArtGrade) -> u32 {
    match grade {
        ArtGrade::Yellow => 500,
        ArtGrade::Profound => 420,
        ArtGrade::Earth => 350,
        ArtGrade::Heaven => 200,
        ArtGrade::Divine => 50,
    }
}

/// How many pomodoros one 阶 costs at this 潜质.
///
/// `base` is the printed, 潜质-zero-relative cost. The printed cost is the
/// *unmodified* one, and a zero-潜质 body needs three times it; every point
/// above zero divides that by 1.01, every point below multiplies it.
///
/// The fraction rule is the brief's, not a rounding convenience: up to and
/// including 0.2 of a pomodoro is dropped (a body that has done 1.2 of the work
/// is not owed a second session for it), past 0.2 it counts as a whole one.
pub fn required_pomodoros(base: u32, aptitude_tenths: i32) -> u32 {
    let raw = f64::from(base)
        * ZERO_APTITUDE_STUDY_MULTIPLE
        * SPEED_STEP.powf(points_f64(aptitude_tenths));
    // Counted in thousandths so the 0.2 boundary is an integer comparison
    // rather than a float one: `x.fract() > 0.2` is a coin flip at the boundary.
    let milli = (raw * 1_000.0).round().max(0.0) as u64;
    let whole = milli / 1_000;
    let fraction = milli % 1_000;
    let value = if fraction > 200 { whole + 1 } else { whole };
    u32::try_from(value).unwrap_or(u32::MAX).max(1)
}

/// How many 洗髓丹 it takes to carry 潜质 from `current` up to `target`.
///
/// Every hundred crossed costs its own number of pills — the hundredth one pill,
/// the two-hundredth two — so 100 → 250 costs 1 + 2 = 3. A move that stays
/// inside the same hundred costs nothing, and a body already sitting *on* a
/// hundred is held there until the gate is paid: that is the "stuck at the even
/// hundred" the brief describes.
pub fn marrow_needed(current_tenths: i32, target_tenths: i32) -> u32 {
    let current = current_tenths.max(0);
    let target = target_tenths.min(APTITUDE_MAX_TENTHS);
    if target <= current {
        return 0;
    }
    let mut need = 0u32;
    // The *next* hundred, not the one the body already stands in: a body at
    // 100.0 has already paid for 100 and is not charged for it twice.
    let mut gate = current / APTITUDE_GATE_TENTHS + 1;
    while gate * APTITUDE_GATE_TENTHS <= target {
        need = need.saturating_add(u32::try_from(gate).unwrap_or(0));
        gate += 1;
    }
    need
}

/// The lowest chance a fight drops a 洗髓丹: one in a hundred thousand, in parts
/// per million.
pub const MARROW_COMBAT_FLOOR_PPM: u64 = 10;

/// The highest chance a fight drops one: one in a thousand.
pub const MARROW_COMBAT_CEIL_PPM: u64 = 1_000;

/// The chance a fight drops a 洗髓丹, in parts per million.
///
/// Tied to how far the opponent outclasses the cultivator, because the pill is
/// the reward for punching up: at parity the odds are the brief's floor of one
/// in a hundred thousand, and an opponent three times the cultivator's power
/// pays the ceiling of one in a thousand. The ramp is linear in between, so the
/// number is explainable in one sentence.
pub fn marrow_combat_ppm(rival_power: u32, player_power: u32) -> u64 {
    let rival = f64::from(rival_power);
    let player = f64::from(player_power.max(1));
    let ratio = rival / player;
    let climb = ((ratio - 1.0) / 2.0).clamp(0.0, 1.0);
    let span = (MARROW_COMBAT_CEIL_PPM - MARROW_COMBAT_FLOOR_PPM) as f64;
    MARROW_COMBAT_FLOOR_PPM + (span * climb) as u64
}

/// The chance a *quiet* source — a walk that met nobody, or the morning event —
/// turns up a 洗髓丹: one in ten thousand, the brief's number.
pub const MARROW_FORTUNE_ONE_IN: u64 = 10_000;

/// One 潜质 item as the page reads it: how many are held, what it costs, and
/// what it does.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HerbView {
    pub herb: AptitudeHerb,
    /// How many the cultivator holds.
    pub held: u32,
    /// What the shelf asks for one.
    pub price: u32,
}

/// The three things a 潜质 build consumes.
///
/// The two herbs are the only way up; the pill opens the gates the herbs cannot
/// pass. Kept as one enum so the shelf, the purse and the page name them the
/// same way.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AptitudeHerb {
    /// 悟心草: the common one, +0.1 ..= +1.2 潜质.
    Insight,
    /// 慧心草: the rare one, +0.5 ..= +2.0 潜质.
    Wisdom,
    /// 洗髓丹: no 潜质 of its own; opens one even-hundred gate.
    Marrow,
}

impl AptitudeHerb {
    /// Every herb, commonest first.
    pub const ALL: [Self; 3] = [Self::Insight, Self::Wisdom, Self::Marrow];

    /// The catalog key naming it.
    pub const fn name_key(self) -> &'static str {
        match self {
            Self::Insight => "cultivation.herb.insight",
            Self::Wisdom => "cultivation.herb.wisdom",
            Self::Marrow => "cultivation.herb.marrow",
        }
    }

    /// What the shelf asks for it, in 灵石.
    ///
    /// Priced against the ladder the herbs move: a single 悟心草 is worth a
    /// fortnight of sessions, 慧心草 two and a half times that, and a 洗髓丹 —
    /// the only thing that opens a gate past a hundred — five times the rarer
    /// herb.
    pub const fn price(self) -> u32 {
        match self {
            Self::Insight => 8_000,
            Self::Wisdom => 20_000,
            Self::Marrow => 100_000,
        }
    }

    /// The 潜质 the herb pays, in tenths, as an inclusive range; `None` for the
    /// pill, which pays none.
    pub const fn gain_tenths(self) -> Option<(i32, i32)> {
        match self {
            Self::Insight => Some((1, 12)),
            Self::Wisdom => Some((5, 20)),
            Self::Marrow => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_body() -> Rng {
        Rng::seeded(0x5EED_1234_5678_9ABC)
    }

    #[test]
    fn a_body_is_born_between_minus_ten_and_ten() {
        let mut rng = a_body();
        let mut seen_low = false;
        let mut seen_high = false;
        for _ in 0..4_000 {
            let tenths = birth_aptitude(&mut rng);
            assert!(
                (APTITUDE_MIN_TENTHS..=APTITUDE_BIRTH_SPAN_TENTHS).contains(&tenths),
                "born at {tenths} tenths"
            );
            seen_low |= tenths <= -90;
            seen_high |= tenths >= 90;
        }
        assert!(seen_low && seen_high, "the whole band is reachable");
    }

    #[test]
    fn the_zero_aptitude_ladder_is_the_one_the_brief_names() {
        assert_eq!(base_success_permille(ArtGrade::Yellow), 500);
        assert_eq!(base_success_permille(ArtGrade::Earth), 350);
        assert_eq!(base_success_permille(ArtGrade::Heaven), 200);
        assert_eq!(base_success_permille(ArtGrade::Divine), 50);
        // The 玄 step sits between its neighbours.
        assert!(base_success_permille(ArtGrade::Profound) < 500);
        assert!(base_success_permille(ArtGrade::Profound) > 350);
        for grade in ArtGrade::ALL {
            assert_eq!(success_permille(grade, 0), base_success_permille(grade));
        }
    }

    #[test]
    fn each_point_of_aptitude_multiplies_the_base_rate_by_one_percent() {
        // 天阶 at 潜质 4 is 20% × 1.01⁴ = 208‰, rounded.
        assert_eq!(success_permille(ArtGrade::Heaven, 40), 208);
        // 黄阶 at 潜质 100 doubles: 50% × 1.01¹⁰⁰ = 135% → held to the cap.
        assert_eq!(
            success_permille(ArtGrade::Yellow, 1_000),
            SUCCESS_CAP_PERMILLE
        );
        // Negative 潜质 lowers it: 50% × 1.01⁻¹⁰.
        assert_eq!(success_permille(ArtGrade::Yellow, -100), 453);
    }

    #[test]
    fn zero_aptitude_needs_three_times_the_printed_cost() {
        // 1 阶 prints 1; a zero-潜质 body needs 3.
        assert_eq!(required_pomodoros(1, 0), 3);
        // 2 阶 prints 2 → 6.
        assert_eq!(required_pomodoros(2, 0), 6);
        // 潜质 10 shaves a tenth off 3.0 → 2.71 → 3 (fraction .71 counts).
        assert_eq!(required_pomodoros(1, 100), 3);
        // 潜质 12 → 3 × 0.99¹² = 2.66 → 3.
        assert_eq!(required_pomodoros(1, 120), 3);
        // 潜质 13 → 2.64 → 3; 潜质 40 → 2.01 → 3; the step is slow on purpose.
        assert!(required_pomodoros(1, 1_000) >= 1, "never below one session");
    }

    #[test]
    fn a_fraction_past_a_fifth_counts_as_a_whole_session() {
        // 3 × 0.99^39 = 2.024 → 2.02: the .02 is dropped.
        let dropped = required_pomodoros(1, 390);
        // 3 × 0.99^64 = 1.579 → .579 counts, so 2.
        let counted = required_pomodoros(1, 640);
        assert_eq!(dropped, 2);
        assert_eq!(counted, 2);
        // The boundary itself is a keeper: a body one point further along must
        // not pay a whole extra session for a rounding coin flip.
        assert_eq!(required_pomodoros(10, 0), 30);
    }

    #[test]
    fn the_even_hundreds_cost_their_own_number_of_pills() {
        // Inside the first hundred: free.
        assert_eq!(marrow_needed(0, 90), 0);
        assert_eq!(marrow_needed(50, 99), 0);
        // Reaching an even hundred costs that hundred's own number of pills.
        assert_eq!(marrow_needed(990, 1_000), 1, "100 costs one");
        assert_eq!(marrow_needed(1_990, 2_000), 2, "200 costs two");
        // Standing on a hundred and moving inside it is free: the gate it paid
        // for is behind it, not ahead.
        assert_eq!(marrow_needed(1_000, 1_050), 0);
        assert_eq!(marrow_needed(2_000, 2_100), 0);
        // A body stuck just under one pays for that one: 199.9 → 250 crosses
        // 200 and nothing else.
        assert_eq!(marrow_needed(1_999, 2_500), 2);
        // From zero to 1000 pays 1+2+…+10 = 55: each hundred its own number.
        assert_eq!(marrow_needed(0, APTITUDE_MAX_TENTHS), 55);
        // Nothing needed for a move that is not upward.
        assert_eq!(marrow_needed(2_000, 2_000), 0);
    }

    /// Each failure on the current 阶 is worth one percent on the next attempt,
    /// and the run is capped rather than allowed to promise certainty.
    #[test]
    fn a_run_of_failures_is_worth_one_percent_each_on_this_rank() {
        let base = base_success_permille(ArtGrade::Heaven);
        assert_eq!(compensated_success_permille(base, 0), base);
        // 20 × 1.01 = 20.2 → 202‰.
        assert_eq!(compensated_success_permille(base, 1), 202);
        assert_eq!(compensated_success_permille(base, 4), 208);
        // Compounding, not adding: 20 × 1.01² = 20.4.
        assert_eq!(compensated_success_permille(base, 2), 204);
        // A very long run of bad luck lands on the cap, not past it.
        assert_eq!(
            compensated_success_permille(base, 100_000),
            SUCCESS_CAP_PERMILLE
        );
        // And a rate that was already at the cap cannot be raised past it.
        assert_eq!(
            compensated_success_permille(SUCCESS_CAP_PERMILLE, 10),
            SUCCESS_CAP_PERMILLE
        );
    }

    #[test]
    fn the_band_is_clamped_at_both_ends() {
        assert_eq!(clamp_tenths(-5_000), APTITUDE_MIN_TENTHS);
        assert_eq!(clamp_tenths(50_000), APTITUDE_MAX_TENTHS);
        assert_eq!(points(47), 4.7);
    }

    #[test]
    fn the_marrow_drop_climbs_with_the_opponent_and_is_capped_at_both_ends() {
        // Parity and anything easier: the floor, one in a hundred thousand.
        assert_eq!(marrow_combat_ppm(1_000, 1_000), MARROW_COMBAT_FLOOR_PPM);
        assert_eq!(marrow_combat_ppm(500, 1_000), MARROW_COMBAT_FLOOR_PPM);
        // Three times the cultivator: the ceiling, one in a thousand.
        assert_eq!(marrow_combat_ppm(3_000, 1_000), MARROW_COMBAT_CEIL_PPM);
        assert_eq!(marrow_combat_ppm(9_000, 1_000), MARROW_COMBAT_CEIL_PPM);
        // Halfway up the ramp is halfway between them.
        let middle = marrow_combat_ppm(2_000, 1_000);
        assert!(middle > MARROW_COMBAT_FLOOR_PPM && middle < MARROW_COMBAT_CEIL_PPM);
        assert_eq!(middle, 505);
        // A zero-power player cannot divide by zero.
        assert_eq!(marrow_combat_ppm(0, 0), MARROW_COMBAT_FLOOR_PPM);
    }

    #[test]
    fn the_herbs_are_priced_and_ranged_apart() {
        // 慧心草 is rarer and dearer, and pays more per draw.
        assert!(AptitudeHerb::Wisdom.price() > AptitudeHerb::Insight.price());
        assert!(AptitudeHerb::Marrow.price() > AptitudeHerb::Wisdom.price());
        let (insight_low, insight_high) = AptitudeHerb::Insight.gain_tenths().unwrap();
        let (wisdom_low, wisdom_high) = AptitudeHerb::Wisdom.gain_tenths().unwrap();
        assert_eq!((insight_low, insight_high), (1, 12), "0.1 ..= 1.2");
        assert_eq!((wisdom_low, wisdom_high), (5, 20), "0.5 ..= 2.0");
        assert!(AptitudeHerb::Marrow.gain_tenths().is_none());
        assert_eq!(AptitudeHerb::Marrow.price(), 100_000, "十万灵石");
    }
}
