//! 打工: odd jobs, the safe counterpart to the 外出 a cultivator could have
//! taken instead.
//!
//! A 外出 is a walk that spends the same day and comes home with 灵石 — both are
//! the slow, can't-lose income, and the two compete for the cultivator's hours.
//! A job is the tamer of the pair: its pay is one figure read before it is
//! taken, where a walk's is a roll that can come up thin.
//!
//! The wage's *number* is still drawn from the 试炼 purse band — a fight's
//! middle, doubled and widened by the hours — so the pay tracks the realm's own
//! fighting rather than floating free. The trial is only where the number comes
//! from; the choice the job stands in for, and the thing it is measured against,
//! is the walk.
//!
//! The purse is a **figure**, not a range. A 试炼 opens a purse somewhere between
//! two numbers because it is a roll; a job is a wage, and a wage the user cannot
//! read before taking it is not a wage. So the job pays the middle of the 试炼's
//! own range, twice over — the same expectation the range carried, printed as
//! one number the board can be judged on.
//!
//! What the wage cannot carry, the job's own trade does. Every job trains an
//! attribute (or two), and how well the cultivator holds that attribute decides
//! how the work comes off: the wage is the wage come what may, but the chance of
//! a lasting gain rides on the cultivator's competence at the work, not on their
//! 潜质. A gifted hand can be clumsy at a trade it never learnt, and a plain one
//! can be steady at a trade it has practised.
//!
//! Three offers hang on the board at a time, and each carries the dice the user
//! reads before taking it:
//!
//! * **品阶** — 黄 to 神, on the same odds a 功法 is graded on. The grade is the
//!   whole difference between two jobs of the same length, which is what makes
//!   reading the board a decision rather than a formality. It also raises the
//!   attribute the job asks of the cultivator, so a better-paid job is a harder
//!   one rather than the same work at a better rate.
//! * **时长** — four to twenty-four hours. The purse follows the hours the way a
//!   walk's does, so a long job is more money for more of the cultivator's day
//!   rather than a better rate.
//! * **种类** — one of the thirty-six trades, each of which trains one outer
//!   attribute or two, and is judged on the one it trains.
//!
//! The 阶级 is not a choice: a job is priced at the realm the cultivator is worth
//! plus or minus one, so the board always holds something within reach and
//! something just past it.

use serde::{Deserialize, Serialize};

use super::explore::PROVING_STONES;
use super::{ArtGrade, Derived, DerivedSlot, Rng};

/// How many jobs the board holds at a time.
pub const JOB_OFFER_COUNT: usize = 3;

/// The shortest job the board offers, in hours.
pub const JOB_MIN_HOURS: u8 = 4;

/// The longest job the board offers, in hours.
pub const JOB_MAX_HOURS: u8 = 24;

/// What a job pays over the 试炼 purse band it is drawn from, in parts per
/// thousand: twice.
///
/// A job is the safe counterpart to a 外出, so it has to pay as well as the walk
/// it stands in for — and the band it is drawn from is the 试炼's, doubled, which
/// keeps the wage anchored to the realm's own fighting. A 试炼 is a fight the
/// cultivator can lose and a job is a wage they cannot: the same four hours come
/// home as double the 试炼's middle, whatever the 试炼 rolled.
const JOB_OVER_TRIAL_PERMILLE: u64 = 2_000;

/// What the five grades multiply a job's purse by, in parts per thousand.
///
/// The same ladder the 外功 multipliers walk, read from one: a 黄阶 job is the
/// baseline and a 神阶 one is a windfall. Every step is half again the one below
/// it, so the grade is the first thing a user reads on the row rather than the
/// last.
const JOB_GRADE_MULTIPLIER_PERMILLE: [u64; 5] = [1_000, 1_600, 2_400, 3_600, 5_400];

/// What the five grades raise a job's attribute requirement by, in parts per
/// thousand.
///
/// A 黄阶 job asks exactly the realm's baseline for the attribute it trains — the
/// number a naked body at that level reads — and each higher grade asks more:
/// a 玄阶 job is a fifth over, a 地阶 half over, a 天阶 nearly double, and a
/// 神阶 asks two and a half times what the realm itself is worth. The grade is
/// thus the whole difference between two jobs: more money *and* more asked.
const JOB_REQUIREMENT_MULTIPLIER_PERMILLE: [u64; 5] = [1_000, 1_200, 1_500, 1_900, 2_400];

/// The chance a job comes off when the cultivator clears the requirement by a
/// wide margin, in parts per thousand: a certainty.
const JOB_CLEAR_PERMILLE: u64 = 1_000;

/// The chance a job comes off when the cultivator just meets the requirement, in
/// parts per thousand.
const JOB_MEET_PERMILLE: u64 = 900;

/// The margin over the requirement a cultivator has to clear before the chance
/// is a certainty, in parts per thousand: fifteen percent.
const JOB_CLEAR_RATIO_PERMILLE: u64 = 1_150;

/// The least chance any job comes off at, in parts per thousand: a tenth.
const JOB_FLOOR_PERMILLE: u64 = 100;

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

/// One of the thirty-six kinds of work the board offers.
///
/// A kind trains one outer attribute or two. The eight a fight reads — 攻击,
/// 生命, 内息, 法术, 法抗, 速度, 命中, 闪避 — can be worked alone, or any two
/// of them mixed: 8 alone and 28 in pairs is 36. The 潜质 line is left out: it
/// is a gift of birth, not a muscle a job can train, which is the whole reason a
/// job is judged on the attribute it asks for rather than on how gifted the hand
/// is.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct JobKind {
    /// The first (or only) attribute the job trains.
    first: DerivedSlot,
    /// The second attribute a mixed job trains; `None` for a single.
    second: Option<DerivedSlot>,
}

impl JobKind {
    /// The eight single-attribute trades, in the order the page draws them.
    const SINGLE_SLOTS: [DerivedSlot; 8] = [
        DerivedSlot::Attack,
        DerivedSlot::Health,
        DerivedSlot::Inner,
        DerivedSlot::Spell,
        DerivedSlot::Resist,
        DerivedSlot::Speed,
        DerivedSlot::Hit,
        DerivedSlot::Evade,
    ];

    /// Every kind the board offers: eight single and twenty-eight mixed.
    pub const ALL: [Self; 36] = Self::build_all();

    /// The attribute(s) this job trains: the first and the optional second.
    pub const fn slots(self) -> (DerivedSlot, Option<DerivedSlot>) {
        (self.first, self.second)
    }

    /// This kind's place in [`Self::ALL`], for a stable key on a row.
    pub const fn index(self) -> usize {
        let mut i = 0;
        while i < Self::ALL.len() {
            if Self::same(self, Self::ALL[i]) {
                return i;
            }
            i += 1;
        }
        0
    }

    /// The key the board reads for this kind's name.
    ///
    /// A trade is a trade, not a pair of attributes, so it has a name of its
    /// own: 炼体 is a name and 攻击·生命 is a recipe. The name is a catalog key
    /// read straight off [`Self::ALL`], which is why the table below has to be
    /// kept in the same order the kinds are built in.
    pub fn name_key(self) -> &'static str {
        Self::NAMES[self.index()]
    }

    /// The catalog key each kind reads by, in the order [`Self::ALL`] draws
    /// them: the eight single-attribute trades, then the twenty-eight pairs.
    const NAMES: [&str; 36] = [
        // Eight trades that train one attribute each.
        "cultivation.jobs.names.attack",
        "cultivation.jobs.names.health",
        "cultivation.jobs.names.inner",
        "cultivation.jobs.names.spell",
        "cultivation.jobs.names.resist",
        "cultivation.jobs.names.speed",
        "cultivation.jobs.names.hit",
        "cultivation.jobs.names.evade",
        // Twenty-eight mixed trades, two attributes at a time.
        "cultivation.jobs.names.attack_health",
        "cultivation.jobs.names.attack_inner",
        "cultivation.jobs.names.attack_spell",
        "cultivation.jobs.names.attack_resist",
        "cultivation.jobs.names.attack_speed",
        "cultivation.jobs.names.attack_hit",
        "cultivation.jobs.names.attack_evade",
        "cultivation.jobs.names.health_inner",
        "cultivation.jobs.names.health_spell",
        "cultivation.jobs.names.health_resist",
        "cultivation.jobs.names.health_speed",
        "cultivation.jobs.names.health_hit",
        "cultivation.jobs.names.health_evade",
        "cultivation.jobs.names.inner_spell",
        "cultivation.jobs.names.inner_resist",
        "cultivation.jobs.names.inner_speed",
        "cultivation.jobs.names.inner_hit",
        "cultivation.jobs.names.inner_evade",
        "cultivation.jobs.names.spell_resist",
        "cultivation.jobs.names.spell_speed",
        "cultivation.jobs.names.spell_hit",
        "cultivation.jobs.names.spell_evade",
        "cultivation.jobs.names.resist_speed",
        "cultivation.jobs.names.resist_hit",
        "cultivation.jobs.names.resist_evade",
        "cultivation.jobs.names.speed_hit",
        "cultivation.jobs.names.speed_evade",
        "cultivation.jobs.names.hit_evade",
    ];

    /// Whether two kinds are the same trade, read by their slots.
    ///
    /// Written out rather than derived from `==` so it can run in a `const`
    /// context, where a derived `PartialEq` is not yet available.
    const fn same(self, other: Self) -> bool {
        self.first.index() == other.first.index()
            && match (self.second, other.second) {
                (Some(a), Some(b)) => a.index() == b.index(),
                (None, None) => true,
                _ => false,
            }
    }

    const fn build_all() -> [Self; 36] {
        let mut table = [Self {
            first: DerivedSlot::Attack,
            second: None,
        }; 36];
        let mut i = 0;
        while i < Self::SINGLE_SLOTS.len() {
            table[i] = Self {
                first: Self::SINGLE_SLOTS[i],
                second: None,
            };
            i += 1;
        }
        let mut n = Self::SINGLE_SLOTS.len();
        let mut a = 0;
        while a < Self::SINGLE_SLOTS.len() {
            let mut b = a + 1;
            while b < Self::SINGLE_SLOTS.len() {
                table[n] = Self {
                    first: Self::SINGLE_SLOTS[a],
                    second: Some(Self::SINGLE_SLOTS[b]),
                };
                n += 1;
                b += 1;
            }
            a += 1;
        }
        table
    }
}

impl Default for JobKind {
    /// The first single-attribute trade — 攻击 — which is what an old save with
    /// no kind written down is treated as.
    fn default() -> Self {
        Self {
            first: DerivedSlot::Attack,
            second: None,
        }
    }
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
    /// What the job trains, which is what it asks of the cultivator.
    #[serde(default)]
    pub kind: JobKind,
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
    /// What the job trains, read again when it comes home.
    #[serde(default)]
    pub kind: JobKind,
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
            kind: offer.kind,
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
            kind: self.kind,
        }
    }
}

/// What one job pays, as the one figure it is.
///
/// Drawn from the 试炼 purse band and doubled: the courtyard's own purse, at this
/// job's tier and widened by this job's hours, then twice again and then the
/// grade. The 试炼 opens a range and a job pays its middle, so the expectation is
/// exactly twice the 试炼 the trade promised — and the board can be read before
/// anything is taken. The job is the safe counterpart to a 外出, the walk it
/// stands in for; the trial band is only the number's source. The hours widen it
/// *linearly* — a job twice as long pays twice as much — because a job is a rate,
/// not a roll, and a rate that paid more per hour the longer it ran would make
/// the longest job the only job.
pub fn job_stones(tier: u8, hours: u8, grade: ArtGrade) -> u32 {
    let hours = u64::from(hours.clamp(JOB_MIN_HOURS, JOB_MAX_HOURS));
    let purse = PROVING_STONES.scaled(hours * 250);
    let (low, high) = purse.at(tier);
    let middle = (u64::from(low) + u64::from(high)) / 2;
    let over = middle * JOB_OVER_TRIAL_PERMILLE / 1_000;
    let grade_permille = JOB_GRADE_MULTIPLIER_PERMILLE[grade.index()];
    let paid = over * grade_permille / 1_000;
    u32::try_from(paid.max(1)).unwrap_or(u32::MAX)
}

/// The attribute a job asks of a cultivator, for one of its slots.
///
/// The baseline is the slot's own value for a naked body at that level — the
/// number the cultivator would read on their own panel with nothing worn and
/// nothing trained — and the grade raises it: a 黄阶 job asks the baseline, and
/// a 神阶 one asks nearly two and a half times it. Priced off the cultivator's
/// own realm so the requirement reads as "what this realm is worth" rather than
/// as an arbitrary number the user has no way to judge.
pub fn requirement(slot: DerivedSlot, level: u16, grade: ArtGrade) -> u32 {
    let baseline = slot.of(super::base_attributes(level).derived());
    let mult = JOB_REQUIREMENT_MULTIPLIER_PERMILLE[grade.index()];
    u32::try_from(u64::from(baseline) * mult / 1_000).unwrap_or(u32::MAX)
}

/// The chance a job comes off, in parts per thousand.
///
/// The job is judged on the weakest of its slots: for a mixed job both
/// attributes have to hold, so the smaller of the two margins decides. A
/// cultivator at their realm's baseline meets the requirement, which is a
/// ninety-percent chance; fifteen percent over is a certainty; and a shortfall
/// costs the same ninety parts, pro-rated, down to a tenth.
pub fn success_rate_permille(kind: JobKind, derived: Derived, level: u16, grade: ArtGrade) -> u64 {
    let (first, second) = kind.slots();
    let worst = match second {
        Some(second) => ratio_permille(first, derived, level, grade)
            .min(ratio_permille(second, derived, level, grade)),
        None => ratio_permille(first, derived, level, grade),
    };
    ratio_to_success(worst)
}

/// The cultivator's margin over a job's requirement for one slot, in parts per
/// thousand: a thousand means they exactly meet it.
fn ratio_permille(slot: DerivedSlot, derived: Derived, level: u16, grade: ArtGrade) -> u64 {
    let requirement = requirement(slot, level, grade);
    let actual = u64::from(slot.of(derived));
    actual.saturating_mul(1_000) / u64::from(requirement.max(1))
}

/// Turn a margin into a chance: at the requirement it is ninety percent, a
/// fifteen-percent margin is a certainty, and a shortfall costs the same ninety
/// pro-rated down to a tenth.
fn ratio_to_success(ratio: u64) -> u64 {
    if ratio >= JOB_CLEAR_RATIO_PERMILLE {
        return JOB_CLEAR_PERMILLE;
    }
    if ratio >= 1_000 {
        return JOB_MEET_PERMILLE;
    }
    (JOB_MEET_PERMILLE * ratio / 1_000).max(JOB_FLOOR_PERMILLE)
}

/// The chance a finished job pays a lasting gain, in parts per thousand.
///
/// The success rate is read as a share of a fifth: a certain job is a
/// five-percent chance, a ninety-percent one is four-and-a-half, and a poor one
/// is less. It is rolled whether the job came off or not — the trade teaches
/// even when it does not pay well.
pub fn point_chance_permille(success_rate: u64) -> u64 {
    success_rate / 20
}

/// The points a finished job pays, once the chance has come up.
///
/// One to as many as the grade's step on the ladder: a 黄阶 job is a single
/// point, a 神阶 one up to five.
pub fn points(rng: &mut Rng, grade: ArtGrade) -> u32 {
    let max = grade.index() as u64 + 1;
    u32::try_from(rng.below(max) + 1).unwrap_or(u32::MAX)
}

/// The slot a mixed job pays, when the gain comes up: one of the two, at random.
///
/// A single-attribute job pays its own slot. A mixed one pays one of the two it
/// trains, since a gain cannot be split between two numbers the page reads
/// separately.
pub fn reward_slot(rng: &mut Rng, kind: JobKind) -> DerivedSlot {
    let (first, second) = kind.slots();
    match second {
        Some(second) if rng.below(2) == 1 => second,
        _ => first,
    }
}

/// Roll the board: [`JOB_OFFER_COUNT`] jobs around the cultivator's tier.
///
/// The tier is the cultivator's own plus or minus one, which is what keeps a board
/// worth reading: a job a rung down is money for nothing much, a job a rung up is
/// the same money for a longer day. Each job carries a kind from the full
/// thirty-six, so the board holds a spread of trades rather than one trade three
/// times.
pub fn roll_jobs(rng: &mut Rng, tier: u8) -> Vec<JobOffer> {
    let tier = tier.clamp(1, 10);
    (0..JOB_OFFER_COUNT)
        .map(|_| {
            let offset = rng.below(3) as i16 - 1;
            let tier = i16::from(tier).saturating_add(offset).clamp(1, 10) as u8;
            let span = u64::from(JOB_MAX_HOURS - JOB_MIN_HOURS + 1);
            let hours = JOB_MIN_HOURS + u8::try_from(rng.below(span)).unwrap_or(0);
            let kind = JobKind::ALL[rng.below(36) as usize];
            JobOffer {
                tier,
                hours,
                grade: super::arts::roll_art_grade(rng),
                kind,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::ArtGrade;

    #[test]
    fn a_job_pays_twice_the_middle_of_the_trial_it_stands_in_for() {
        // Twelve hours at the fourth tier: the 试炼's purse is 72–120, so its
        // middle is 96 and twice that is 192.
        let trial = PROVING_STONES.scaled(3_000).at(4);
        let middle = (u64::from(trial.0) + u64::from(trial.1)) / 2;
        let job = job_stones(4, 12, ArtGrade::Yellow);
        let expected = middle * 2_000 / 1_000;
        assert!(
            u64::from(job).abs_diff(expected) <= 2,
            "{job} against a 试炼 at {trial:?}"
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
            kind: JobKind::default(),
        };
        let run = JobRun::started(offer, 1_000);
        assert_eq!(run.ends_unix, 1_000 + 8 * 3_600);
        assert!(!run.is_done(1_000 + 8 * 3_600 - 1));
        assert!(run.is_done(1_000 + 8 * 3_600));
        assert_eq!(run.offer(), offer);
    }

    #[test]
    fn there_are_eight_single_and_twenty_eight_mixed_kinds() {
        // 8 alone and 28 in pairs is 36, and every kind on the board is one of
        // them: no trade off the table, and no trade twice.
        assert_eq!(JobKind::ALL.len(), 36);
        let singles = JobKind::ALL
            .iter()
            .filter(|kind| kind.slots().1.is_none())
            .count();
        let mixed = JobKind::ALL.len() - singles;
        assert_eq!(singles, 8, "{singles} single trades");
        assert_eq!(mixed, 28, "{mixed} mixed trades");
        for (i, kind) in JobKind::ALL.iter().enumerate() {
            assert_eq!(kind.index(), i, "kind at {i}");
        }
    }

    #[test]
    fn the_grade_raises_the_requirement_the_way_it_raises_the_wage() {
        let level = 40;
        let slot = DerivedSlot::Attack;
        let yellow = requirement(slot, level, ArtGrade::Yellow);
        let divine = requirement(slot, level, ArtGrade::Divine);
        assert!(divine > yellow, "{yellow} to {divine}");
        assert!(
            divine * 100 >= yellow * 230 && divine * 100 <= yellow * 250,
            "{yellow} to {divine}"
        );
        // Every step asks more, in order.
        let steps: Vec<u32> = ArtGrade::ALL
            .into_iter()
            .map(|grade| requirement(slot, level, grade))
            .collect();
        for pair in steps.windows(2) {
            assert!(pair[1] > pair[0], "{steps:?}");
        }
    }

    #[test]
    fn meeting_the_requirement_is_ninety_percent_and_clearing_it_is_certain() {
        let level = 40;
        // A 黄阶 job asks exactly the realm's baseline, so a naked body at the
        // level meets it to the point.
        let grade = ArtGrade::Yellow;
        let kind = JobKind::default(); // 攻击 alone
        let requirement = requirement(kind.slots().0, level, grade);
        // A naked body at the level is exactly at the requirement: ninety percent.
        let bare = crate::base_attributes(level).derived();
        assert_eq!(
            success_rate_permille(kind, bare, level, grade),
            JOB_MEET_PERMILLE
        );
        // Just past fifteen percent over the requirement is a certainty: the
        // extra point beats the floor that integer division would leave behind.
        let clear = Derived {
            attack: requirement * 15 / 100 + 1,
            ..Derived::default()
        };
        assert_eq!(
            success_rate_permille(kind, bare.plus(clear), level, grade),
            JOB_CLEAR_PERMILLE
        );
        // A body short of the requirement pays the shortfall pro-rated, down to
        // the floor.
        let weak = Derived {
            attack: requirement * 80 / 100,
            ..bare
        };
        let short = success_rate_permille(kind, weak, level, grade);
        assert!(short < JOB_MEET_PERMILLE, "{short}");
        assert!(short >= JOB_FLOOR_PERMILLE, "{short}");
    }

    #[test]
    fn a_mixed_job_is_judged_on_its_weakest_slot() {
        let level = 40;
        let grade = ArtGrade::Yellow;
        // 攻击 + 生命: strong in 攻击, weak in 生命, so the 生命 margin rules.
        let kind = JobKind::ALL
            .iter()
            .copied()
            .find(|kind| kind.slots() == (DerivedSlot::Attack, Some(DerivedSlot::Health)))
            .expect("the attack-health trade exists");
        let bare = crate::base_attributes(level).derived();
        let strong_attack = bare.plus(Derived {
            attack: requirement(DerivedSlot::Attack, level, grade) * 30 / 100,
            ..Derived::default()
        });
        // Strong attack, baseline health: judged on health, so ninety percent.
        assert_eq!(
            success_rate_permille(kind, strong_attack, level, grade),
            JOB_MEET_PERMILLE
        );
        // Both over by fifteen percent: certain. The extra point on each beats
        // the floor integer division would leave behind.
        let both = bare.plus(Derived {
            attack: requirement(DerivedSlot::Attack, level, grade) * 15 / 100 + 1,
            health: requirement(DerivedSlot::Health, level, grade) * 15 / 100 + 1,
            ..Derived::default()
        });
        assert_eq!(
            success_rate_permille(kind, both, level, grade),
            JOB_CLEAR_PERMILLE
        );
    }

    #[test]
    fn the_reward_chance_is_the_success_rate_over_twenty() {
        // A certain job is a five-percent chance, a ninety-percent one is
        // four-and-a-half, and a poor one is less.
        assert_eq!(point_chance_permille(1_000), 50);
        assert_eq!(point_chance_permille(900), 45);
        assert_eq!(point_chance_permille(500), 25);
        assert_eq!(point_chance_permille(100), 5);
    }

    #[test]
    fn points_ride_on_the_grade() {
        let mut rng = Rng::seeded(0x0B0B_5C17);
        // A 黄阶 job always pays one point; a 神阶 one pays up to five.
        assert_eq!(points(&mut rng, ArtGrade::Yellow), 1);
        let mut seen = [false; 5];
        for _ in 0..20_000 {
            let p = points(&mut rng, ArtGrade::Divine);
            assert!((1..=5).contains(&p), "{p}");
            seen[(p - 1) as usize] = true;
        }
        assert!(seen.iter().all(|s| *s), "{seen:?}");
    }

    #[test]
    fn a_mixed_job_pays_one_of_its_two_slots() {
        let mut rng = Rng::seeded(0xC0DE);
        let kind = JobKind::ALL
            .iter()
            .copied()
            .find(|kind| kind.slots().1.is_some())
            .expect("a mixed trade exists");
        let (_first, second) = kind.slots();
        let mut first_hits = 0;
        let mut second_hits = 0;
        for _ in 0..20_000 {
            if reward_slot(&mut rng, kind) == second.unwrap() {
                second_hits += 1;
            } else {
                first_hits += 1;
            }
        }
        assert!(
            first_hits > 0 && second_hits > 0,
            "{first_hits}/{second_hits}"
        );
        // Roughly even: either of the two, not one always.
        let share = second_hits * 1_000 / (first_hits + second_hits);
        assert!((450..=550).contains(&share), "{share}‰ to the second");
    }

    #[test]
    fn a_single_job_always_pays_its_own_slot() {
        let mut rng = Rng::seeded(0x5EED);
        let kind = JobKind::ALL[0]; // 攻击 alone
        for _ in 0..1_000 {
            assert_eq!(reward_slot(&mut rng, kind), DerivedSlot::Attack);
        }
    }

    #[test]
    fn every_kind_reads_a_name_of_its_own() {
        // A trade has a name, not a recipe: each of the thirty-six reads a
        // distinct catalog key, so the board never shows two trades that are
        // the same word.
        let mut names = std::collections::BTreeSet::new();
        for kind in JobKind::ALL {
            assert!(
                names.insert(kind.name_key()),
                "two kinds share the name {}",
                kind.name_key()
            );
        }
        assert_eq!(names.len(), JobKind::ALL.len());
        // The first kind — 攻击 alone — is the key an old save with no kind
        // written down is treated as.
        assert_eq!(
            JobKind::default().name_key(),
            "cultivation.jobs.names.attack"
        );
    }
}
