//! 探索: one walk out of the door every ten hours.
//!
//! Exploration is the ladder's quiet faucet. A session already pays growth and
//! may drop gear, and a breakthrough pays a 功法, so walking pays the third thing
//! a cultivator runs short of: seeds. It is gated by a cooldown rather than by a
//! cost, because the thing being rationed is the player's attention, not a
//! currency.
//!
//! A walk can also go wrong: a quarter of them meet someone. That fight is not a
//! guardian — it is a wanderer, weaker, and paying less — and losing it costs
//! nothing but the walk.

use serde::{Deserialize, Serialize};

use super::Rng;

/// How long a failed breakthrough locks the attempt for, in seconds.
///
/// Twelve hours: long enough that a loss is a real setback, short enough that a
/// player who goes and trains comes back the same day.
pub const BREAKTHROUGH_COOLDOWN_SECONDS: i64 = 12 * 60 * 60;

/// How long a walk takes, which is also what it is worth.
///
/// The same road, walked for longer, finds more: the length is the one lever
/// the user pulls, and it is a real choice rather than a bigger-is-better one
/// because a long walk is a walk you cannot take again for half a day. A day
/// holds six short walks or two long ones, and six rolls at one fifth come to
/// about what two rolls at three fifths do — the question is how finely the
/// user wants their rolls, not which button is better.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum ExplorationHours {
    /// Four hours, at the base chances.
    Four,
    /// Eight hours, at two and a tenth the base chances.
    #[default]
    Eight,
    /// Twelve hours, at three and a fifth the base chances.
    Twelve,
}

impl ExplorationHours {
    /// Every length, shortest first.
    pub const ALL: [Self; 3] = [Self::Four, Self::Eight, Self::Twelve];

    /// The length in whole hours, as a surface prints it.
    pub const fn hours(self) -> u8 {
        match self {
            Self::Four => 4,
            Self::Eight => 8,
            Self::Twelve => 12,
        }
    }

    /// The length in seconds: how long the next walk is held for.
    pub const fn seconds(self) -> i64 {
        self.hours() as i64 * 60 * 60
    }

    /// What this length multiplies a walk's chances by, in parts per thousand.
    ///
    /// One, two point one and three point two — the brief's own ladder. Every
    /// chance a walk rolls is read through it, so the multiplier is exactly
    /// what it says rather than what is left of it after a cap.
    pub const fn multiplier_permille(self) -> u64 {
        match self {
            Self::Four => 1_000,
            Self::Eight => 2_100,
            Self::Twelve => 3_200,
        }
    }

    /// The length a stored number of hours names.
    ///
    /// Anything else — a zero from a save written before walks had lengths, a
    /// hand-edited seven — is the middle length rather than a refusal: the
    /// field is a preference, and a preference that cannot be understood is
    /// better replaced than thrown away.
    pub const fn from_hours(hours: u8) -> Self {
        match hours {
            4 => Self::Four,
            12 => Self::Twelve,
            _ => Self::Eight,
        }
    }
}

/// The chance a walk meets someone, in parts per thousand, at four hours.
///
/// Every chance below is the *four-hour* one: a longer walk multiplies it by
/// [`ExplorationHours::multiplier_permille`]. They are a third of what they
/// were when ten hours was the only length on offer, because a day now holds
/// up to six walks instead of two and a half: six rolls of a third are the
/// same day's worth of findings the old cadence paid, which is what keeps the
/// road a trickle rather than a fountain.
pub const ENCOUNTER_PERMILLE: u64 = 90;
/// What a walk turns up when it is quiet: seeds most often, gear sometimes, a
/// 功法 almost never.
pub const SEED_FIND_PERMILLE: u64 = 150;
pub const GEAR_FIND_PERMILLE: u64 = 40;
pub const ART_FIND_PERMILLE: u64 = 10;
/// What winning the fight that met you pays. Better than a quiet walk, because
/// the walk cost a fight to keep — and led by gear rather than by herbs, which
/// is the one place the two findings differ in kind: a wanderer carries what
/// they were carrying, a roadside carries what grows there.
pub const VICTORY_ART_PERMILLE: u64 = 80;
pub const VICTORY_GEAR_PERMILLE: u64 = 300;
pub const VICTORY_SEED_PERMILLE: u64 = 220;

/// A purse one source pays, as it stands at the first realm.
///
/// 灵石 travels beside an item rather than in place of it — it is money, not a
/// finding — so every source has its own purse rather than one shared roll. Two
/// things make the purses differ, and both are the design:
///
/// * **Which source it is.** A fight pays more than a walk: the walk is the
///   cheapest thing a save can do, and a save that fights has earned the purse.
/// * **How deep the ladder is.** The numbers here are the *first realm's*, and
///   the purse is multiplied by the tier it was paid at — the realm a walk was
///   taken in, the realm a guardian guarded, the realm a trial floor stands in.
///   Gear prices climb with the tier too, so the shop keeps roughly its weight
///   on the page at every depth instead of becoming pocket change at the top.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StonePurse {
    /// The fewest coins it pays at the first realm.
    pub min: u32,
    /// The most coins it pays at the first realm.
    pub max: u32,
}

impl StonePurse {
    /// The purse as a walk of a given length opens it, in parts per thousand.
    ///
    /// The walk's purse is the one purse that has to be widened by the length:
    /// six short walks a day would otherwise collect two and a half times the
    /// money two long ones do, and the shortest length would be the money
    /// button rather than one of three equal choices. A fight's purse is not
    /// scaled — a wanderer carries what they carry whatever time it is.
    pub(crate) const fn scaled(self, permille: u64) -> Self {
        let scale = permille as u32;
        Self {
            min: self.min.saturating_mul(scale) / 1_000,
            max: self.max.saturating_mul(scale) / 1_000,
        }
    }

    /// The range this purse pays at `tier`, which is a one-based realm.
    ///
    /// A zero tier cannot happen on the ladder, but a document read from a hand
    /// edit can say anything, so it is read as the first realm rather than as a
    /// purse of nothing.
    pub(crate) const fn at(self, tier: u8) -> (u32, u32) {
        let tier = if tier == 0 { 1 } else { tier } as u32;
        (self.min.saturating_mul(tier), self.max.saturating_mul(tier))
    }
}

/// What a quiet walk turns up, as the *four-hour* walk opens it. The smallest
/// purse of the four: a walk costs nothing but time, so it is a trickle beside a
/// fight — but it always pays one, because a walk that came home with nothing
/// would read as a bug rather than as a quiet road.
///
/// Read per walk and then widened by the length: a day holds six of these or two
/// twelve-hour ones, and the two have to come to about the same purse or the
/// shortest length is simply the better button.
pub(crate) const WALK_STONES: StonePurse = StonePurse { min: 2, max: 4 };

/// What clearing a trial floor pays. Above a walk because the tower is a fight,
/// and below a wanderer because a floor is a level-matched opponent rather than
/// someone who has been out on the road longer.
pub(crate) const TRIAL_STONES: StonePurse = StonePurse { min: 12, max: 18 };

/// What beating a wanderer pays — the road's own purse, which is what an ambush
/// is for. Its finding roll leans the same way: gear over herbs.
pub(crate) const WANDERER_STONES: StonePurse = StonePurse { min: 20, max: 34 };

/// What winning a 试炼 in the courtyard pays.
///
/// The courtyard is the fourth place a fight happens and the one that used to
/// pay no purse at all: its reward was a single exclusive roll, and most rolls
/// come up empty, so a clean win could leave the log reading "试炼获胜。" and
/// nothing else. Sized above a walk because it is a fight and below a tower
/// floor because the opponent is the cultivator's own level rather than a
/// warden that climbs with the tower.
pub(crate) const PROVING_STONES: StonePurse = StonePurse { min: 6, max: 10 };

/// What beating a realm's guardian pays. The richest purse, because a guardian
/// guards a realm and its treasury both, and because it is the rarest fight on
/// the ladder.
pub(crate) const GUARDIAN_STONES: StonePurse = StonePurse { min: 38, max: 66 };

/// What one pomodoro's thousandth pays when the purse is the shape it took.
///
/// Above the guardian's on purpose: this is not a source that pays for work, it
/// is a jackpot that pays for luck, so it has to read as one — a purse the size
/// of a road's walk would be a strange thing to be a thousandth.
pub(crate) const BONUS_STONES: StonePurse = StonePurse { min: 50, max: 90 };

/// The purses are ordered on purpose — a walk under a courtyard bout, a bout
/// under a floor, a floor under a wanderer, a wanderer under a guardian —
/// because that ordering is both the difficulty and the emphasis, and a retune
/// that broke it would flatten the sources into one.
const _: () = assert!(WALK_STONES.max < PROVING_STONES.min);
const _: () = assert!(PROVING_STONES.max < TRIAL_STONES.min);
const _: () = assert!(TRIAL_STONES.max < WANDERER_STONES.min);
const _: () = assert!(WANDERER_STONES.max < GUARDIAN_STONES.min);

/// What one walk came to.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ExplorationOutcome {
    /// Nothing but the walk itself.
    Nothing,
    /// A seed came of it.
    Seed,
    /// Gear came of it.
    Gear,
    /// A 功法 came of it: the rarest thing a walk can pay.
    Art,
    /// 灵石 came of it, and nothing else did.
    Stones,
    /// Someone was met, and beaten.
    Victory,
    /// Someone was met, and not beaten. Nothing was lost but the walk.
    Defeat,
}

impl ExplorationOutcome {
    /// Whether the walk met someone.
    pub fn fought(self) -> bool {
        matches!(self, Self::Victory | Self::Defeat)
    }

    /// The catalog key naming how the walk ended.
    pub const fn outcome_key(self) -> &'static str {
        match self {
            Self::Nothing => "cultivation.explore.nothing",
            Self::Seed => "cultivation.explore.found_seed",
            Self::Gear => "cultivation.explore.found_gear",
            Self::Art => "cultivation.explore.found_art",
            Self::Stones => "cultivation.explore.found_stones",
            Self::Victory => "cultivation.explore.victory",
            Self::Defeat => "cultivation.explore.defeat",
        }
    }

    /// The catalog key naming how the walk ended, for a line that already names
    /// the finds beside it.
    ///
    /// [`Self::outcome_key`] hands back sentences that carry a `%{name}` or a
    /// `%{value}` — right in a popup, where the surface fills them in, and
    /// wrong in a history line where the finds are named *after* the outcome.
    /// Those four outcomes get placeholder-free sentences here; the other three
    /// never had one to lose.
    pub const fn record_outcome_key(self) -> &'static str {
        match self {
            Self::Nothing => "cultivation.explore.nothing",
            Self::Seed => "cultivation.record.outcome.seed",
            Self::Gear => "cultivation.record.outcome.gear",
            Self::Art => "cultivation.record.outcome.art",
            Self::Stones => "cultivation.record.outcome.stones",
            Self::Victory => "cultivation.explore.victory",
            Self::Defeat => "cultivation.explore.defeat",
        }
    }
}

/// How many walks the record keeps.
pub const WALK_RECORD_CAP: usize = 8;

/// What the last walk settled, kept after the app restarts.
///
/// The popup that shows a walk is deliberately forgotten on quit — it is a
/// moment, not a record — so without this the page could never say what the last
/// walk actually paid. Only names are stored: the record is a line of history,
/// and a line that went stale because an item was renamed would be a worse lie
/// than one that says what it found.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WalkRecord {
    /// When the walk was taken.
    pub at_unix: i64,
    /// The realm index it was taken in.
    pub realm: u8,
    /// How it ended.
    pub outcome: ExplorationOutcome,
    /// The 功法 it turned up, by name, when it turned one up.
    pub art: Option<String>,
    /// The gear it turned up, by name, when it turned some up.
    pub gear: Option<String>,
    /// The herb it turned up, by name, when it turned one up.
    pub seed: Option<String>,
    /// How many 灵石 it turned up, when it turned any up.
    ///
    /// Counted rather than named, because a purse is a number: what the walk
    /// found is worth exactly what it says, and a name would only be a number
    /// the page had to guess at again.
    #[serde(default)]
    pub stones: Option<u32>,
}

/// What one walk turns up, before any fight is settled.
///
/// One roll answers both questions — is anything there, and does anyone want it —
/// so the four quiet outcomes are exclusive and their chances add up to less than
/// one, leaving the rest of the thousand to "nothing".
pub fn roll_walk(rng: &mut Rng, hours: ExplorationHours) -> ExplorationOutcome {
    let scale = hours.multiplier_permille();
    let widened = |base: u64| base.saturating_mul(scale) / 1_000;
    let mut edge = widened(SEED_FIND_PERMILLE);
    let roll = rng.below(1_000);
    if roll < edge {
        return ExplorationOutcome::Seed;
    }
    edge = edge.saturating_add(widened(GEAR_FIND_PERMILLE));
    if roll < edge {
        return ExplorationOutcome::Gear;
    }
    edge = edge.saturating_add(widened(ART_FIND_PERMILLE));
    if roll < edge {
        return ExplorationOutcome::Art;
    }
    edge = edge.saturating_add(widened(ENCOUNTER_PERMILLE));
    if roll < edge {
        // Someone is here; who wins is the fight's to say, not the walk's.
        return ExplorationOutcome::Victory;
    }
    ExplorationOutcome::Nothing
}

/// What winning the fight pays, as a second roll of its own.
pub fn roll_victory(rng: &mut Rng) -> ExplorationOutcome {
    let roll = rng.below(1_000);
    if roll < VICTORY_ART_PERMILLE {
        return ExplorationOutcome::Art;
    }
    if roll < VICTORY_ART_PERMILLE + VICTORY_GEAR_PERMILLE {
        return ExplorationOutcome::Gear;
    }
    if roll < VICTORY_ART_PERMILLE + VICTORY_GEAR_PERMILLE + VICTORY_SEED_PERMILLE {
        return ExplorationOutcome::Seed;
    }
    ExplorationOutcome::Nothing
}

/// How many 灵石 an outing turns up.
///
/// A roll of its own, beside the finding roll rather than inside it: 灵石 is a
/// currency, not one of the three things an outing finds, so the same walk can
/// come back with a herb and a handful of stones both. The amount is a short
/// range rather than a fixed figure, so two walks in the same day do not pay
/// the same purse, and it is scaled by the tier the purse was opened at.
///
/// Crate-private like the other rolls: the seeded source it takes is nobody
/// else's business, and the state machine is the only thing that draws it.
pub(crate) fn roll_stones(rng: &mut Rng, purse: StonePurse, tier: u8) -> u32 {
    let (min, max) = purse.at(tier);
    let span = max.saturating_sub(min).saturating_add(1);
    min.saturating_add(u32::try_from(rng.below(u64::from(span))).unwrap_or(0))
}

/// A walk under way: when it left, when it comes home, and whether the user
/// called it back early.
///
/// A walk is a stretch of time rather than a click that pays immediately: the
/// cultivator is *out*, and what the road turned up is only known when they are
/// back. That is the whole reason the length is a choice — a twelve-hour walk is
/// twelve hours of not being able to walk again.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WalkInProgress {
    /// When the walk left.
    pub started_unix: i64,
    /// When it comes home — the instant its find is settled.
    pub ends_unix: i64,
    /// The length that was chosen, as hours.
    pub hours: u8,
    /// Whether the user called the walk home early.
    ///
    /// A called walk pays nothing: the road is not walked, so there is nothing
    /// on it. What it costs is the return trip, which is a third of the time
    /// already spent — a user cannot recall a walk for free and set off again.
    #[serde(default)]
    pub early: bool,
}

impl WalkInProgress {
    /// A walk that left just now, for the length chosen.
    pub fn started(started_unix: i64, hours: ExplorationHours) -> Self {
        Self {
            started_unix,
            ends_unix: started_unix.saturating_add(hours.seconds()),
            hours: hours.hours(),
            early: false,
        }
    }

    /// The same walk, called home at `now_unix`.
    ///
    /// The return trip is a third of the time already spent: the road home is
    /// shorter than the road out, but it is not free, and a walk recalled at the
    /// eleventh hour cannot be a walk that never happened. A walk recalled the
    /// moment it left still owes [`MIN_RETURN_SECONDS`], so the button is never
    /// a way to undo a walk for nothing.
    pub fn called_home(self, now_unix: i64) -> Self {
        let spent = now_unix.saturating_sub(self.started_unix).max(0);
        Self {
            ends_unix: now_unix.saturating_add(return_trip_seconds(spent)),
            early: true,
            ..self
        }
    }

    /// Whether the walk is home at this instant.
    pub fn is_home(&self, now_unix: i64) -> bool {
        now_unix >= self.ends_unix
    }

    /// How long is left before the walk is home; zero once it is.
    pub fn remaining_seconds(&self, now_unix: i64) -> i64 {
        self.ends_unix.saturating_sub(now_unix).max(0)
    }

    /// The length as its own enum.
    pub fn length(&self) -> ExplorationHours {
        ExplorationHours::from_hours(self.hours)
    }
}

/// The least a return trip can take, in seconds.
///
/// Six: a walk recalled the moment it left still has to be recalled, and a trip
/// that ended in the same second the button was pressed would read as a walk
/// that never started rather than as one that came home.
pub const MIN_RETURN_SECONDS: i64 = 6;

/// How long the trip home takes once an outing has been called back.
///
/// A third of the time already spent: the road home is shorter than the road
/// out, but it is not free, so calling an outing back is a way to stop paying
/// attention to it rather than a way to undo it. Shared by both kinds of outing
/// — a walk and a shift — so the two cannot drift into different prices for the
/// same decision.
pub fn return_trip_seconds(spent_seconds: i64) -> i64 {
    (spent_seconds.max(0) / 3).max(MIN_RETURN_SECONDS)
}

/// Whether the guardian can be fought again: the cooldown after a loss has run
/// out.
pub fn breakthrough_ready(blocked_until: Option<i64>, now_unix: i64) -> bool {
    match blocked_until {
        None => true,
        Some(until) => now_unix >= until,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The quiet outcomes and the encounter share one roll, so together they
    /// cannot fill it: the rest of the thousand is a walk that finds nothing.
    /// The *longest* walk is the one that has to fit, because its chances are
    /// the base ones multiplied by the largest of the three.
    const _: () = assert!(
        (SEED_FIND_PERMILLE + GEAR_FIND_PERMILLE + ART_FIND_PERMILLE + ENCOUNTER_PERMILLE)
            * ExplorationHours::Twelve.multiplier_permille()
            / 1_000
            <= 1_000
    );

    #[test]
    fn a_walk_lands_on_every_outcome_sooner_or_later() {
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..1_000_u64 {
            seen.insert(roll_walk(&mut Rng::seeded(seed), ExplorationHours::Four));
        }
        for outcome in [
            ExplorationOutcome::Seed,
            ExplorationOutcome::Gear,
            ExplorationOutcome::Art,
            ExplorationOutcome::Victory,
            ExplorationOutcome::Nothing,
        ] {
            assert!(seen.contains(&outcome), "{outcome:?} never came up");
        }
        // Seeds are the common finding, a 功法 the rarest.
        let counts = |outcome| {
            (0..5_000_u64)
                .filter(|seed| {
                    roll_walk(&mut Rng::seeded(*seed), ExplorationHours::Four) == outcome
                })
                .count()
        };
        assert!(counts(ExplorationOutcome::Seed) > counts(ExplorationOutcome::Gear));
        assert!(counts(ExplorationOutcome::Gear) > counts(ExplorationOutcome::Art));
    }

    #[test]
    fn a_victory_pays_out_and_a_defeat_does_not() {
        assert!(ExplorationOutcome::Victory.fought());
        assert!(ExplorationOutcome::Defeat.fought());
        assert!(!ExplorationOutcome::Seed.fought());
    }

    /// The purse is the tier's: the same walk pays six times as much in the
    /// sixth realm, which is what keeps the shop's weight at every depth.
    #[test]
    fn a_purse_scales_with_the_tier_it_was_opened_at() {
        assert_eq!(WALK_STONES.at(1), (WALK_STONES.min, WALK_STONES.max));
        assert_eq!(WALK_STONES.at(6).0, WALK_STONES.min * 6);
        assert_eq!(WALK_STONES.at(10).1, WALK_STONES.max * 10);
        assert_eq!(
            WALK_STONES.at(0),
            WALK_STONES.at(1),
            "a hand-edited zero tier reads as the first realm"
        );
        // Every purse is opened at the tier of the thing that paid it, so a
        // deeper fight is a bigger purse by arithmetic and not by a second table.
        let mut rng = Rng::seeded(11);
        let shallow = roll_stones(&mut rng, GUARDIAN_STONES, 1);
        let mut deep = 0;
        for _ in 0..16 {
            deep = deep.max(roll_stones(&mut rng, GUARDIAN_STONES, 5));
        }
        assert!(deep > shallow * 2, "{deep} against {shallow}");
    }

    /// A longer walk finds more, exactly as much more as the multiplier says —
    /// measured on the roll itself rather than asserted from the constant, so a
    /// tuning that quietly broke the ladder would be caught here.
    #[test]
    fn a_longer_walk_finds_more_by_exactly_its_multiplier() {
        let hits = |hours: ExplorationHours| {
            (0..20_000_u64)
                .filter(|seed| {
                    roll_walk(&mut Rng::seeded(*seed), hours) != ExplorationOutcome::Nothing
                })
                .count()
        };
        let four = hits(ExplorationHours::Four) as f64;
        let eight = hits(ExplorationHours::Eight) as f64;
        let twelve = hits(ExplorationHours::Twelve) as f64;
        assert!(
            (eight / four - 2.1).abs() < 0.1,
            "eight hours is 2.1×: {eight} against {four}"
        );
        assert!(
            (twelve / four - 3.2).abs() < 0.15,
            "twelve hours is 3.2×: {twelve} against {four}"
        );
        assert!(twelve > eight && eight > four);
    }

    /// A day's quiet walks pay about the same whatever their length: six short
    /// ones, three middling, two long. The purse follows the length for exactly
    /// this reason — otherwise the shortest chip would be the money button.
    #[test]
    fn a_days_walks_pay_about_the_same_whatever_their_length() {
        let per_day = |hours: ExplorationHours| {
            let walks = 24 / i64::from(hours.hours());
            let purse = WALK_STONES.scaled(hours.multiplier_permille());
            (walks * i64::from(purse.min), walks * i64::from(purse.max))
        };
        let four = per_day(ExplorationHours::Four);
        assert_eq!(four, (12, 24), "six short walks");
        assert_eq!(per_day(ExplorationHours::Eight), four, "three middling");
        assert_eq!(per_day(ExplorationHours::Twelve), four, "two long");
    }

    #[test]
    fn a_walk_is_out_for_its_own_length_and_comes_home_with_nothing_until_it_does() {
        let start = 1_000;
        let walk = WalkInProgress::started(start, ExplorationHours::Four);
        assert_eq!(walk.ends_unix, start + 4 * 3_600);
        assert!(!walk.early, "a walk that was not called is a walk");
        assert!(!walk.is_home(start));
        assert!(!walk.is_home(start + 4 * 3_600 - 1));
        assert!(walk.is_home(start + 4 * 3_600));
        assert_eq!(walk.remaining_seconds(start), 4 * 3_600);
        assert_eq!(walk.remaining_seconds(start + 4 * 3_600 + 9_999), 0);
        assert_eq!(walk.length(), ExplorationHours::Four);
    }

    /// 提前回返：the walk pays nothing, and the trip home is a third of the time
    /// already spent — never less than a moment's worth.
    #[test]
    fn a_called_walk_owes_a_third_of_the_time_it_spent() {
        let start = 1_000;
        let walk = WalkInProgress::started(start, ExplorationHours::Twelve);
        let called = walk.called_home(start + 6 * 3_600);
        assert!(called.early);
        assert_eq!(
            called.ends_unix,
            start + 6 * 3_600 + 2 * 3_600,
            "six hours out is two hours home"
        );
        assert!(!called.is_home(start + 7 * 3_600));
        assert!(called.is_home(start + 8 * 3_600));
        // Called the instant it left: the floor, not zero.
        let at_once = walk.called_home(start);
        assert_eq!(at_once.ends_unix, start + MIN_RETURN_SECONDS);
        assert!(!at_once.is_home(start));
        // A clock that went backwards cannot hand out a free walk.
        let backwards = walk.called_home(start - 600);
        assert_eq!(backwards.ends_unix, start - 600 + MIN_RETURN_SECONDS);
    }

    #[test]
    fn the_guardian_cooldown_runs_out() {
        assert!(breakthrough_ready(None, 0));
        assert!(!breakthrough_ready(Some(500), 499));
        assert!(breakthrough_ready(Some(500), 500));
    }
}
