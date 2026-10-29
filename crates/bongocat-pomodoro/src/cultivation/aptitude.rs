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

use super::{ArtGrade, Rng, SeedGrade};

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

/// The success rate one attempt at the given grade carries, in parts per
/// thousand: the grade's base, multiplied by 1.01 once per point of 潜质 and
/// once per attempt this 功法 has already lost.
///
/// One factor, two counts. 潜质 and failures are the same kind of help — a body
/// that is better at learning, and a body that has been wrong about *this* book
/// before — so they go into one exponent rather than into two rates multiplied
/// together: `base × 1.01^(潜质 + 失败)`. Two roundings would let a 玄阶 art at
/// 潜质 4.3 and three failures read a thousandth or two under the number the
/// formula on the page prints.
///
/// The 潜质-zero ladder is authored: 黄 50%, 玄 42%, 地 35%, 天 20%, 神 5%. The
/// 玄 step is the one interpolation in the row — the brief names 黄/地/天/神 —
/// and it sits between its neighbours rather than beside one of them.
///
/// `failures` belongs to the whole 功法 rather than to the 阶 it was spent on:
/// the caller never resets it, so twenty bad attempts at a 神阶 book are twenty
/// percent banked toward the rest of that book — and toward nothing else, since
/// every art carries its own count.
pub fn success_permille(grade: ArtGrade, aptitude_tenths: i32, failures: u32) -> u32 {
    let base = f64::from(base_success_permille(grade)) / 1_000.0;
    // A save with a corrupt count cannot be allowed to overflow `powf` into an
    // infinity the cast would then have to guess at.
    let spent = f64::from(failures.min(10_000));
    let chance = base * SUCCESS_STEP.powf(points_f64(aptitude_tenths) + spent);
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

/// The next even-hundred gate the body stands under, and what opening it costs.
///
/// `None` once 潜质 has reached [`APTITUDE_MAX_TENTHS`]: the top of the band has
/// no gate past it, so there is nothing for a pill to buy there. The price is
/// the gate's own number — the hundredth one pill, the two-hundredth two — the
/// same rule [`marrow_needed`] charges when a herb crosses, so a pill spent on
/// its own and a pill spent by a herb cost the same for the same gate.
pub fn next_gate(total_tenths: i32) -> Option<(i32, u32)> {
    let current = total_tenths.max(0);
    if current >= APTITUDE_MAX_TENTHS {
        return None;
    }
    // The gate ahead, never the one the body already stands on: a body at 100.0
    // has paid for 100 and is not charged for it twice.
    let gate = current / APTITUDE_GATE_TENTHS + 1;
    let target = gate * APTITUDE_GATE_TENTHS;
    (target <= APTITUDE_MAX_TENTHS).then(|| (target, u32::try_from(gate).unwrap_or(u32::MAX)))
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

/// A stack of the same 潜质 herb at one grade, the satchel's way of holding
/// what the shelf and the drops hand over.
///
/// A herb is a seed without a plot: the grade still decides what it pays, and
/// the body is only ever fed one of a stack at a time, so the stack records the
/// grade rather than a count of something interchangeable. 悟心草 and 慧心草
/// stack; the 洗髓丹 does not, because a pill pays no 潜质 of its own.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HerbStack {
    pub herb: AptitudeHerb,
    #[serde(default)]
    pub grade: SeedGrade,
    pub count: u32,
}

impl HerbStack {
    /// A stack of one herb.
    pub const fn single(herb: AptitudeHerb, grade: SeedGrade) -> Self {
        Self {
            herb,
            grade,
            count: 1,
        }
    }
}

/// The three things a 潜质 build consumes.
///
/// The two herbs are the only way up; the pill opens the gates the herbs cannot
/// pass. Kept as one enum so the shelf, the purse and the page name them the
/// same way.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AptitudeHerb {
    /// 悟心草: the common one; +0.7 潜质 at the 地阶 anchor.
    Insight,
    /// 慧心草: the rare one; +1.3 潜质 at the 地阶 anchor.
    Wisdom,
    /// 洗髓丹: no 潜质 of its own; opens one even-hundred gate.
    Marrow,
}

/// What a 潜质 herb's grade multiplies its base 潜质 by, in parts per thousand.
///
/// One ladder shared by every herb and by the price the shelf asks, so a grade
/// is one number for the whole row. Unlike the garden's harvest ladder — which
/// is allowed to be steep, because a crop is worth a fraction of a session — a
/// herb's ladder is deliberately shallow: the whole span from 黄 to 神 is only
/// ×1.64, which is what keeps the rare herb above the common one at *every*
/// grade. A 慧心草 at 黄阶 (+1.0) pays more than a 悟心草 at 神阶 (+0.9), so the
/// row reads "rarer is always better" and never asks a player to compare a rare
/// herb's bottom rung against a common one's top.
///
/// The anchor is 地 at a thousand, unchanged from the old numbers; 黄/玄 step
/// just above three quarters, 天/神 step just above the anchor. The old ladder
/// (250/600/1000/1600/2500) is what a garden harvest still uses — see
/// [`SeedGrade::multiplier_permille`] — while this one is what a 潜质 herb is
/// worth in 潜质 and in 灵石.
pub const HERB_GRADE_MULTIPLIER_PERMILLE: [u32; 5] = [760, 850, 1_000, 1_100, 1_250];

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
    /// Priced against the ladder the herbs move: one 慧心草 is worth about a
    /// hundred and twenty-five sessions of earnings and 悟心草 about
    /// seventy-five. The 洗髓丹 — the only thing that opens a gate past a
    /// hundred — sits five times above the rarer, dearer herb, which is what
    /// keeps it the dearest row on the shelf even at one 灵石 in ten thousand.
    pub const fn price(self) -> u32 {
        match self {
            Self::Insight => 1_200,
            Self::Wisdom => 2_000,
            Self::Marrow => 10_000,
        }
    }

    /// The 潜质 the herb pays at the 地阶 anchor, in tenths; `None` for the
    /// pill, which pays none.
    ///
    /// The old band paid at random inside its range — 悟心草 0.5 ..= 2.0, 慧心草
    /// 0.1 ..= 1.2 — so the same herb drew a different number every time. The
    /// band has collapsed onto its midpoint, keeping the expected reward: one
    /// herb of one grade always pays the same, and what the row says is what
    /// the body gets. The rarer herb pays more, which is what makes it the
    /// dearer one on the shelf too.
    pub const fn gain_tenths(self) -> Option<i32> {
        match self {
            Self::Insight => Some(7),
            Self::Wisdom => Some(13),
            Self::Marrow => None,
        }
    }

    /// The 潜质 the herb pays at one grade, in tenths: one fixed number.
    ///
    /// The anchor scaled by the herb ladder, so a herb is a seed in everything
    /// but the plot: 地阶 — the anchor — pays exactly the base, and the rarer
    /// herb out-pays the common one at every grade (慧心草 黄阶 +1.0 above 悟心草
    /// 神阶 +0.9). Rounded to the nearest tenth, so no grade ever pays nothing
    /// and no grade pays a fraction it cannot spend.
    pub const fn gain_tenths_for(self, grade: SeedGrade) -> Option<i32> {
        let base = match self.gain_tenths() {
            Some(base) => base,
            None => return None,
        };
        let mult = Self::herb_grade_multiplier(grade) as i32;
        Some((base.saturating_mul(mult) + 500) / 1_000)
    }

    /// What the shelf asks for one herb of this grade, in 灵石.
    ///
    /// The base price scaled by the same ladder the gain uses: 地阶 is the
    /// anchor, and the rarer herb is the dearer row at every grade. The pill
    /// carries no grade and keeps its own price.
    pub const fn price_for(self, grade: SeedGrade) -> u32 {
        if matches!(self, Self::Marrow) {
            // The pill pays no 潜质, so it carries no grade to price at.
            return self.price();
        }
        let mult = Self::herb_grade_multiplier(grade);
        (self.price() * mult + 500) / 1_000
    }

    /// The herb ladder's multiplier at a grade.
    const fn herb_grade_multiplier(grade: SeedGrade) -> u32 {
        HERB_GRADE_MULTIPLIER_PERMILLE[grade.index()]
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
            assert_eq!(
                success_permille(grade, 0, 0),
                base_success_permille(grade),
                "{grade:?} at 潜质 zero with no failures is its base"
            );
        }
    }

    #[test]
    fn each_point_of_aptitude_multiplies_the_base_rate_by_one_percent() {
        // 天阶 at 潜质 4 is 20% × 1.01⁴ = 208‰, rounded.
        assert_eq!(success_permille(ArtGrade::Heaven, 40, 0), 208);
        // 黄阶 at 潜质 100 doubles: 50% × 1.01¹⁰⁰ = 135% → held to the cap.
        assert_eq!(
            success_permille(ArtGrade::Yellow, 1_000, 0),
            SUCCESS_CAP_PERMILLE
        );
        // Negative 潜质 lowers it: 50% × 1.01⁻¹⁰.
        assert_eq!(success_permille(ArtGrade::Yellow, -100, 0), 453);
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
    fn the_next_gate_is_the_hundred_ahead_and_costs_its_own_number() {
        // A body below zero stands under the first gate: one pill pays it.
        assert_eq!(next_gate(-36), Some((1_000, 1)));
        assert_eq!(next_gate(0), Some((1_000, 1)));
        // Inside the first hundred the gate ahead is still the first one.
        assert_eq!(next_gate(990), Some((1_000, 1)));
        // Standing *on* a hundred means that one is behind it, not ahead.
        assert_eq!(next_gate(1_000), Some((2_000, 2)));
        assert_eq!(next_gate(1_999), Some((2_000, 2)));
        // The last gate is the tenth; past it there is nothing left to buy.
        assert_eq!(next_gate(9_990), Some((10_000, 10)));
        assert_eq!(next_gate(10_000), None);
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

    /// A failure is worth one percent on the next attempt, in the same exponent
    /// as 潜质: the two are added, not multiplied one rate by another.
    #[test]
    fn a_failure_is_worth_one_percent_on_the_same_exponent_as_aptitude() {
        // 天阶 at 潜质 0: 20% × 1.01¹ = 20.2 → 202‰.
        assert_eq!(success_permille(ArtGrade::Heaven, 0, 1), 202);
        assert_eq!(success_permille(ArtGrade::Heaven, 0, 4), 208);
        // Compounding, not adding: 20 × 1.01² = 20.4.
        assert_eq!(success_permille(ArtGrade::Heaven, 0, 2), 204);
        // 潜质 and failures share one exponent: 4.0 points of 潜质 and two
        // failures land on exactly 潜质 6 with no failures.
        assert_eq!(
            success_permille(ArtGrade::Heaven, 40, 2),
            success_permille(ArtGrade::Heaven, 60, 0)
        );
        // A very long run of bad luck lands on the cap, not past it.
        assert_eq!(
            success_permille(ArtGrade::Heaven, 0, 100_000),
            SUCCESS_CAP_PERMILLE
        );
        // And a rate that was already at the cap cannot be raised past it.
        assert_eq!(
            success_permille(ArtGrade::Yellow, 1_000, 10),
            SUCCESS_CAP_PERMILLE
        );
        // The floor holds too: a 神阶 book in a hopeless body still reads above
        // zero once the failures start to mount.
        assert!(success_permille(ArtGrade::Divine, -100, 0) >= SUCCESS_FLOOR_PERMILLE);
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
        // 慧心草 is the rarer, dearer one and pays more per draw; the 洗髓丹 is
        // five times the dearer herb again, so it is the dearest row on the shelf.
        assert!(AptitudeHerb::Marrow.price() > AptitudeHerb::Wisdom.price());
        assert!(AptitudeHerb::Wisdom.price() > AptitudeHerb::Insight.price());
        // One herb of one grade pays one fixed number — the old band's midpoint,
        // so the expected reward is unchanged.
        assert_eq!(AptitudeHerb::Wisdom.gain_tenths(), Some(13), "1.3 at 地");
        assert_eq!(AptitudeHerb::Insight.gain_tenths(), Some(7), "0.7 at 地");
        assert!(AptitudeHerb::Marrow.gain_tenths().is_none());
        assert_eq!(AptitudeHerb::Marrow.price(), 10_000, "一万灵石");
        assert_eq!(AptitudeHerb::Wisdom.price(), 2_000, "两千灵石");
        assert_eq!(AptitudeHerb::Insight.price(), 1_200, "一千二百灵石");
    }

    /// A 潜质 herb carries a grade like any seed: 地阶 is the anchor the old
    /// numbers were, and the rarer herb out-pays the common one at every grade.
    /// No grade pays nothing, and each grade pays one fixed number.
    #[test]
    fn a_herb_pays_its_grade_in_one_fixed_number() {
        // The anchor is the base: 1.3 for a 慧心草, 0.7 for a 悟心草.
        assert_eq!(
            AptitudeHerb::Wisdom.gain_tenths_for(SeedGrade::Earth),
            Some(13)
        );
        assert_eq!(
            AptitudeHerb::Insight.gain_tenths_for(SeedGrade::Earth),
            Some(7)
        );
        // Every grade pays one fixed number, and a better grade pays more.
        for herb in [AptitudeHerb::Insight, AptitudeHerb::Wisdom] {
            let mut last = 0i32;
            for grade in SeedGrade::ALL {
                let pay = herb.gain_tenths_for(grade).unwrap();
                assert!(pay >= 1, "a {herb:?} at {grade:?} pays nothing");
                assert!(
                    pay >= last,
                    "a better grade pays at least as much at {herb:?}"
                );
                last = pay;
            }
        }
        assert_eq!(
            AptitudeHerb::Marrow.gain_tenths_for(SeedGrade::Divine),
            None
        );
        // The rarer herb's floor sits above the common one's ceiling, so a
        // player never weighs a 慧心草 黄阶 against a 悟心草 神阶.
        let wisdom_floor = AptitudeHerb::Wisdom.gain_tenths_for(SeedGrade::Yellow).unwrap();
        let insight_ceiling = AptitudeHerb::Insight.gain_tenths_for(SeedGrade::Divine).unwrap();
        assert!(
            wisdom_floor > insight_ceiling,
            "慧心草 黄阶 {wisdom_floor} must beat 悟心草 神阶 {insight_ceiling}"
        );
        // The compressed ladder: the common herb spans +0.5..=+0.9, the rare
        // +1.0..=+1.6, both to the nearest tenth.
        assert_eq!(
            AptitudeHerb::Wisdom.gain_tenths_for(SeedGrade::Yellow),
            Some(10)
        );
        assert_eq!(
            AptitudeHerb::Wisdom.gain_tenths_for(SeedGrade::Divine),
            Some(16)
        );
        assert_eq!(
            AptitudeHerb::Insight.gain_tenths_for(SeedGrade::Yellow),
            Some(5)
        );
        assert_eq!(
            AptitudeHerb::Insight.gain_tenths_for(SeedGrade::Divine),
            Some(9)
        );
    }

    /// A herb's grade prices it the same way it pays it: 地阶 is the anchor and
    /// the rarer herb is the dearer row at every grade. The pill carries no
    /// grade and is unchanged.
    #[test]
    fn a_herb_is_priced_at_its_grade() {
        assert_eq!(AptitudeHerb::Wisdom.price_for(SeedGrade::Earth), 2_000);
        assert_eq!(AptitudeHerb::Wisdom.price_for(SeedGrade::Yellow), 1_520);
        assert_eq!(AptitudeHerb::Wisdom.price_for(SeedGrade::Divine), 2_500);
        assert_eq!(AptitudeHerb::Insight.price_for(SeedGrade::Divine), 1_500);
        // The pill is not a graded thing.
        for grade in SeedGrade::ALL {
            assert_eq!(
                AptitudeHerb::Marrow.price_for(grade),
                AptitudeHerb::Marrow.price(),
                "a 洗髓丹 has no grade to price at"
            );
        }
    }
}
