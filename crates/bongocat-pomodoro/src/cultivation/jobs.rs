//! 打工: odd jobs, priced off the road the cultivator could have walked instead.
//!
//! A walk is the ladder's quiet faucet and it pays in kind — a herb, a garment, a
//! book. A job pays in the one thing the walk barely touches: money. It is the
//! slower of the two and the safer, and the trade is written into the price: the
//! same hours spent on a job pay a fifth more than the same hours on the road,
//! and they pay it in 灵石 rather than in a roll that might come up empty.
//!
//! The purse is a **figure**, not a range. A walk opens a purse somewhere between
//! two numbers because it is a roll; a job is a wage, and a wage the user cannot
//! read before taking it is not a wage. So the job pays the middle of the walk's
//! own range, a fifth over — the same expectation the range carried, printed as
//! one number the board can be judged on.
//!
//! What the wage cannot carry, 潜质 does: a cultivator with 潜质 to spare has a
//! hand that turns up early. Each whole point of it is a 0.11% chance the job
//! comes home at double and a 0.01% chance at triple, which is the one place the
//! ladder lets an attribute widen a purse rather than a fighter.
//!
//! Three offers hang on the board at a time, and each carries two dice the user
//! reads before taking it:
//!
//! * **品阶** — 黄 to 神, on the same odds a 功法 is graded on. The grade is the
//!   whole difference between two jobs of the same length, which is what makes
//!   reading the board a decision rather than a formality.
//! * **时长** — four to twenty-four hours. The purse follows the hours the way a
//!   walk's does, so a long job is more money for more of the cultivator's day
//!   rather than a better rate.
//!
//! The 阶级 is not a choice: a job is priced at the realm the cultivator is worth
//! plus or minus one, so the board always holds something within reach and
//! something just past it.

use serde::{Deserialize, Serialize};

use super::explore::WALK_STONES;
use super::{ArtGrade, Rng};

/// How many jobs the board holds at a time.
pub const JOB_OFFER_COUNT: usize = 3;

/// The shortest job the board offers, in hours.
pub const JOB_MIN_HOURS: u8 = 4;

/// The longest job the board offers, in hours.
pub const JOB_MAX_HOURS: u8 = 24;

/// What a job pays over the walk it stands in for, in parts per thousand: a fifth
/// more.
///
/// A job is the safe half of the choice, so it has to pay better than the road on
/// average — but only a fifth, because the road pays in goods a job cannot: a
/// seed, a garment, a book, and the fight that met you on the way.
const JOB_OVER_WALK_PERMILLE: u64 = 1_200;

/// What the five grades multiply a job's purse by, in parts per thousand.
///
/// The same ladder the 外功 multipliers walk, read from one: a 黄阶 job is the
/// baseline and a 神阶 one is a windfall. Every step is half again the one below
/// it, so the grade is the first thing a user reads on the row rather than the
/// last.
const JOB_GRADE_MULTIPLIER_PERMILLE: [u64; 5] = [1_000, 1_600, 2_400, 3_600, 5_400];

/// What ten whole points of 潜质 add to the chance a finished job pays double, in
/// parts per thousand: 0.11% a point is 1.1 of them, so ten points is eleven.
const JOB_DOUBLE_PERMILLE_PER_TEN_POINTS: u64 = 11;

/// What ten whole points of 潜质 add to the chance a finished job pays triple, in
/// parts per thousand: 0.01% a point is a tenth of one, so ten points is one.
const JOB_TRIPLE_PERMILLE_PER_TEN_POINTS: u64 = 1;

/// What the board charges to be re-rolled, per tier of the cultivator's own
/// realm, in 灵石.
///
/// A free re-roll would make the board a slot machine: three draws at a
/// ten-thousandth, refreshed until a 神阶 job turns up. The toll is half of the
/// board's cheapest honest work — a four-hour 黄阶 job at the cultivator's own
/// tier — so re-rolling is a decision about money rather than about patience:
/// cheap enough to try, dear enough that trying has a cost.
pub fn refresh_cost(tier: u8) -> u32 {
    job_stones(tier, 4, ArtGrade::Yellow)
        .saturating_div(2)
        .max(1)
}

/// One job on the board.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct JobOffer {
    /// The realm's goods the job is priced at: one below the cultivator's own,
    /// their own, or one above.
    pub tier: u8,
    /// How long it takes, in hours.
    pub hours: u8,
    /// The job's grade, on the ladder a 功法 is graded on.
    pub grade: ArtGrade,
}

impl JobOffer {
    /// What this job pays, as the one figure the board can be read against.
    pub fn stones(self) -> u32 {
        job_stones(self.tier, self.hours, self.grade)
    }

    /// How long this job takes, in seconds.
    pub fn seconds(self) -> i64 {
        i64::from(self.hours.clamp(JOB_MIN_HOURS, JOB_MAX_HOURS)) * 3_600
    }
}

/// A job being worked: when it started and when it comes due.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct JobRun {
    pub started_unix: i64,
    pub ends_unix: i64,
    pub tier: u8,
    pub hours: u8,
    pub grade: ArtGrade,
}

impl JobRun {
    /// Start one now, at the length the offer named.
    pub fn started(offer: JobOffer, now_unix: i64) -> Self {
        Self {
            started_unix: now_unix,
            ends_unix: now_unix.saturating_add(offer.seconds()),
            tier: offer.tier,
            hours: offer.hours,
            grade: offer.grade,
        }
    }

    /// Whether the work is done at this instant.
    pub fn is_done(&self, now_unix: i64) -> bool {
        now_unix >= self.ends_unix
    }

    /// How long is left before the work is done; zero once it is.
    pub fn remaining_seconds(&self, now_unix: i64) -> u32 {
        u32::try_from(self.ends_unix.saturating_sub(now_unix).max(0)).unwrap_or(u32::MAX)
    }

    /// The offer this run was taken from, for a purse that is priced the same way.
    pub fn offer(self) -> JobOffer {
        JobOffer {
            tier: self.tier,
            hours: self.hours,
            grade: self.grade,
        }
    }
}

/// What one job pays, as the one figure it is.
///
/// Priced off the walk it stands in for: the road's own purse, at this job's tier
/// and widened by this job's hours, then a fifth again and then the grade. The
/// walk opens a range and a job pays its middle, so the expectation is exactly
/// the fifth over the road the trade promised — and the board can be read before
/// anything is taken. The hours widen it *linearly* — a job twice as long pays
/// twice as much — because a job is a rate, not a roll, and a rate that paid more
/// per hour the longer it ran would make the longest job the only job.
pub fn job_stones(tier: u8, hours: u8, grade: ArtGrade) -> u32 {
    let hours = u64::from(hours.clamp(JOB_MIN_HOURS, JOB_MAX_HOURS));
    let purse = WALK_STONES.scaled(hours * 250);
    let (low, high) = purse.at(tier);
    let middle = (u64::from(low) + u64::from(high)) / 2;
    let over = middle * JOB_OVER_WALK_PERMILLE / 1_000;
    let grade_permille = JOB_GRADE_MULTIPLIER_PERMILLE[grade.index()];
    let paid = over * grade_permille / 1_000;
    u32::try_from(paid.max(1)).unwrap_or(u32::MAX)
}

/// What a 潜质 is worth as a chance, in parts per thousand.
///
/// 潜质 is stored in tenths and is allowed to be negative — a cultivator born
/// clumsy has nothing to offer a hand that turns up early — so a negative reads
/// as no chance at all rather than as a negative one. The rounding is to the
/// nearest part per thousand rather than down: a low 潜质 should still be able
/// to crit, just rarely.
fn crit_chance_permille(tenths: i32, per_ten_points: u64) -> u64 {
    let tenths = u64::try_from(tenths.max(0)).unwrap_or(0);
    // A point is ten tenths, so ten points are a hundred of them.
    (tenths * per_ten_points + 50) / 100
}

/// What a finished job paid its wage at: one, two or three times.
///
/// Rolled once, when the job comes home, off the cultivator's 潜质. The two
/// chances are read together and the deeper one wins, so a triple is a triple
/// and not a double that happens to be a triple.
pub fn crit_multiplier(rng: &mut Rng, aptitude_tenths: Option<i32>) -> u32 {
    let tenths = aptitude_tenths.unwrap_or(0);
    let triple = crit_chance_permille(tenths, JOB_TRIPLE_PERMILLE_PER_TEN_POINTS);
    let double = crit_chance_permille(tenths, JOB_DOUBLE_PERMILLE_PER_TEN_POINTS);
    if rng.below(1_000) < triple {
        3
    } else if rng.below(1_000) < double {
        2
    } else {
        1
    }
}

/// Roll the board: [`JOB_OFFER_COUNT`] jobs around the cultivator's tier.
///
/// The tier is the cultivator's own plus or minus one, which is what keeps a board
/// worth reading: a job a rung down is money for nothing much, a job a rung up is
/// the same money for a longer day.
pub fn roll_jobs(rng: &mut Rng, tier: u8) -> Vec<JobOffer> {
    let tier = tier.clamp(1, 10);
    (0..JOB_OFFER_COUNT)
        .map(|_| {
            let offset = rng.below(3) as i16 - 1;
            let tier = i16::from(tier).saturating_add(offset).clamp(1, 10) as u8;
            let span = u64::from(JOB_MAX_HOURS - JOB_MIN_HOURS + 1);
            let hours = JOB_MIN_HOURS + u8::try_from(rng.below(span)).unwrap_or(0);
            JobOffer {
                tier,
                hours,
                grade: super::arts::roll_art_grade(rng),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::ArtGrade;

    #[test]
    fn a_job_pays_a_fifth_over_the_middle_of_the_walk_it_stands_in_for() {
        // Twelve hours at the fourth tier: the road's purse is 25–51, so its
        // middle is 38 and a fifth over that is 45.
        let walk = WALK_STONES.scaled(3_000).at(4);
        let middle = (u64::from(walk.0) + u64::from(walk.1)) / 2;
        let job = job_stones(4, 12, ArtGrade::Yellow);
        let expected = middle * 1_200 / 1_000;
        assert!(
            u64::from(job).abs_diff(expected) <= 2,
            "{job} against a walk at {walk:?}"
        );
    }

    #[test]
    fn the_refresh_toll_is_half_a_four_hour_yellow_job_at_the_tier() {
        // The board's cheapest honest work — a four-hour 黄阶 job — prices the
        // toll at half, so a re-roll is a decision about money, not a free draw.
        for tier in 1..=10 {
            let toll = refresh_cost(tier);
            let job = job_stones(tier, 4, ArtGrade::Yellow);
            assert_eq!(toll, (job / 2).max(1), "tier {tier}");
        }
        assert!(refresh_cost(1) >= 1, "a re-roll is never free");
        // A better realm pays more to re-roll, the way it pays more for work.
        assert!(refresh_cost(10) > refresh_cost(1));
    }

    #[test]
    fn a_job_is_one_figure_not_a_range() {
        // The board is read before anything is taken, so the same job prices the
        // same every time it is asked.
        let first = job_stones(5, 12, ArtGrade::Heaven);
        let second = job_stones(5, 12, ArtGrade::Heaven);
        assert_eq!(first, second);
        assert!(first > 0, "{first}");
    }

    #[test]
    fn the_grade_is_the_whole_difference_between_two_jobs() {
        let hours = 12;
        let tier = 4;
        let payouts: Vec<u32> = ArtGrade::ALL
            .into_iter()
            .map(|grade| job_stones(tier, hours, grade))
            .collect();
        // Strictly better at every step, and the 神阶 job pays several times the
        // 黄阶 one.
        for pair in payouts.windows(2) {
            assert!(pair[1] > pair[0], "{payouts:?}");
        }
        assert!(payouts[4] >= payouts[0] * 5, "{payouts:?}");
    }

    #[test]
    fn a_longer_job_pays_more_at_the_same_rate() {
        let short = job_stones(4, 4, ArtGrade::Earth);
        let long = job_stones(4, 24, ArtGrade::Earth);
        assert!(long >= short * 5, "{short} to {long}");
        // And it is the same rate: six times the hours is six times the purse.
        let per_hour_short = short / 4;
        let per_hour_long = long / 24;
        assert!(
            per_hour_long.abs_diff(per_hour_short) <= 1,
            "{per_hour_short} an hour against {per_hour_long}"
        );
    }

    #[test]
    fn the_board_holds_three_jobs_around_the_tier_the_cultivator_is_worth() {
        let mut rng = Rng::seeded(0xB0B5);
        let board = roll_jobs(&mut rng, 4);
        assert_eq!(board.len(), JOB_OFFER_COUNT);
        for job in &board {
            assert!((3..=5).contains(&job.tier), "{job:?}");
            assert!(
                (JOB_MIN_HOURS..=JOB_MAX_HOURS).contains(&job.hours),
                "{job:?}"
            );
        }
    }

    #[test]
    fn every_grade_turns_up_and_the_bottom_one_is_the_commonest() {
        let mut counts = [0u32; 5];
        let mut rng = Rng::seeded(0x5EED_B0B5);
        for _ in 0..20_000 {
            counts[roll_jobs(&mut rng, 4)[0].grade.index()] += 1;
        }
        assert!(counts.iter().all(|count| *count > 0), "{counts:?}");
        assert!(
            counts[0] > counts[1] && counts[1] > counts[2] && counts[2] > counts[3],
            "{counts:?}"
        );
    }

    #[test]
    fn a_run_starts_at_the_length_the_offer_named() {
        let offer = JobOffer {
            tier: 3,
            hours: 8,
            grade: ArtGrade::Earth,
        };
        let run = JobRun::started(offer, 1_000);
        assert_eq!(run.ends_unix, 1_000 + 8 * 3_600);
        assert!(!run.is_done(1_000 + 8 * 3_600 - 1));
        assert!(run.is_done(1_000 + 8 * 3_600));
        assert_eq!(run.offer(), offer);
    }

    #[test]
    fn a_point_of_aptitude_is_the_hundredth_of_a_percent_it_was_asked_for() {
        // Ten points of 潜质: 0.11% at double and 0.01% at triple.
        assert_eq!(
            crit_chance_permille(100, JOB_DOUBLE_PERMILLE_PER_TEN_POINTS),
            11
        );
        assert_eq!(
            crit_chance_permille(100, JOB_TRIPLE_PERMILLE_PER_TEN_POINTS),
            1
        );
        // A hundred points: eleven times that.
        assert_eq!(
            crit_chance_permille(1_000, JOB_DOUBLE_PERMILLE_PER_TEN_POINTS),
            110
        );
        // A clumsy cultivator offers nothing, and a negative offers less.
        assert_eq!(
            crit_chance_permille(0, JOB_DOUBLE_PERMILLE_PER_TEN_POINTS),
            0
        );
        assert_eq!(
            crit_chance_permille(-50, JOB_TRIPLE_PERMILLE_PER_TEN_POINTS),
            0
        );
    }

    #[test]
    fn the_deeper_crit_is_the_rarer_one() {
        // Over a great many hands, doubles happen, triples happen less, and the
        // ratio is the one the ladder asked for.
        let tenths = Some(500); // fifty points: 5.5% double, 0.5% triple
        let mut double = 0u32;
        let mut triple = 0u32;
        let mut rng = Rng::seeded(0xC_C17);
        for _ in 0..200_000 {
            match crit_multiplier(&mut rng, tenths) {
                3 => triple += 1,
                2 => double += 1,
                _ => {}
            }
        }
        assert!(
            double > 0 && triple > 0,
            "{double} doubles, {triple} triples"
        );
        assert!(
            triple * 10 < double,
            "{double} doubles against {triple} triples"
        );
        // And it is roughly the share asked for, not a share of a share.
        let share = double * 1_000 / 200_000;
        assert!((45..=70).contains(&share), "{share}‰ doubled");
    }

    #[test]
    fn a_cultivator_with_no_aptitude_never_crits() {
        let mut rng = Rng::seeded(0x0B0B_5C17);
        for _ in 0..1_000 {
            assert_eq!(crit_multiplier(&mut rng, None), 1);
        }
    }
}
