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
use super::resources::GatherSite;

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

/// The find rates below are *four-hour* rates, and a longer walk multiplies them
/// by [`ExplorationHours::multiplier_permille`].
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
pub(crate) const WALK_STONES: StonePurse = StonePurse { min: 3, max: 5 };

/// What clearing a trial floor pays. Above a walk because the tower is a fight,
/// and below a wanderer because a floor is a level-matched opponent rather than
/// someone who has been out on the road longer.
pub(crate) const TRIAL_STONES: StonePurse = StonePurse { min: 12, max: 18 };

/// What beating a wanderer pays — the road's own purse, which is what an ambush
/// is for. Its finding roll leans the same way: gear over herbs.
///
/// A wanderer is a walk's richest find, so its purse carries the road's money:
/// the two are what make a twelve-hour walk worth three-fifths of a 黄阶 job of
/// the same hours — below a job, because the walk also drops gear and seeds a
/// job cannot, and the 灵石 is only the road's cash on top of them.
pub(crate) const WANDERER_STONES: StonePurse = StonePurse { min: 48, max: 68 };

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
///
/// A guardian stands above a wanderer, so its purse has to widen as the
/// wanderer's does: a breakthrough is the ladder's gate, and the guardian that
/// guards it is worth more than the road it closes off.
pub(crate) const GUARDIAN_STONES: StonePurse = StonePurse { min: 70, max: 120 };

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
    /// The region this walk was taken in.
    #[serde(default)]
    pub region: u8,
    /// The level inside that region.
    #[serde(default)]
    pub level: u8,
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

/// A region-seeded roll of one walk's finding: the road meets someone first,
/// with the region's and level's own rate, and the quiet remainder is the
/// usual seed-and-gear-and-art split. The combat is the larger share because a
/// region walk is more a corridor between fights than a quiet road.
pub fn roll_region_walk(
    rng: &mut Rng,
    hours: ExplorationHours,
    region: u8,
    level: u8,
) -> ExplorationOutcome {
    let region = Region::from_index(region as usize);
    let combat = encounter_permille(region, level, hours);
    let gear = region_gear_permille(level, hours);
    let seed = region_seed_permille(level);
    let art = ART_FIND_PERMILLE * hours.multiplier_permille() / 1_000;
    let mut edge = combat;
    let roll = rng.below(1_000);
    if roll < edge {
        return ExplorationOutcome::Victory;
    }
    edge = edge.saturating_add(gear);
    if roll < edge {
        return ExplorationOutcome::Gear;
    }
    edge = edge.saturating_add(seed);
    if roll < edge {
        return ExplorationOutcome::Seed;
    }
    edge = edge.saturating_add(art);
    if roll < edge {
        return ExplorationOutcome::Art;
    }
    ExplorationOutcome::Nothing
}

/// A wanderer scaled to a region walk's floor: the region's base realm, with
/// its tier raised by `level / 3` so the corridor gets deeper as it goes. The
/// wanderer's drops are of the region's own tier band, and the tier step is what
/// makes the difficulty feel like `base * 150` rung by rung.
pub fn region_wanderer(region: Region, level: u8, seed: u64) -> super::opponent::Opponent {
    // The level's own middle: level one is the first tenth of the region, so
    // its wanderer stands at five percent of it, not at its first rung.
    let progress = (u32::from(level.clamp(1, MAX_LEVEL)) * 100).saturating_sub(50);
    let rung = region_level_at(region, progress);
    super::opponent::challenger(rung, region.tier(), seed)
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
/// twelve hours of not being able to walk again. A walk now also carries its
/// region and the level it was set out at, because the road's difficulty and
/// purse are no longer a pure function of the realm — they are a function of
/// the region and the depth the walk was taken at.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WalkInProgress {
    /// When the walk left.
    pub started_unix: i64,
    /// When it comes home — the instant its find is settled.
    pub ends_unix: i64,
    /// The length that was chosen, as hours.
    pub hours: u8,
    /// The region this walk was taken in.
    #[serde(default)]
    pub region: u8,
    /// The level inside that region.
    #[serde(default)]
    pub level: u8,
    /// Whether the user called the walk home early.
    ///
    /// A called walk pays nothing: the road is not walked, so there is nothing
    /// on it. What it costs is the return trip, which is a third of the time
    /// already spent — a user cannot recall a walk for free and set off again.
    #[serde(default)]
    pub early: bool,
}

impl WalkInProgress {
    /// A walk that left just now, for the length and level chosen.
    pub fn started(started_unix: i64, hours: ExplorationHours, region: u8, level: u8) -> Self {
        Self {
            started_unix,
            ends_unix: started_unix.saturating_add(hours.seconds()),
            hours: hours.hours(),
            region,
            level,
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

/// An expedition under way: a stretch of gathering hours at one site.
///
/// The same shape as a walk — the cultivator is *out*, and the find is only
/// known when they are back — so the two share the gates: one outing at a time,
/// and a return trip that is a third of the time already spent. What an
/// expedition turns up is the site's yield, not a road's find, so the settle
/// pays [`super::resources::expedition_yield`] rather than a roll.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ExpeditionInProgress {
    /// When the expedition set off.
    pub started_unix: i64,
    /// When it comes home — the instant its yield is settled.
    pub ends_unix: i64,
    /// The site the expedition worked.
    pub site: GatherSite,
    /// The length that was chosen, as hours.
    pub hours: u8,
    /// Whether the user called the expedition home early.
    ///
    /// A called expedition pays nothing: the site is not worked, so there is
    /// nothing to bring home. What it costs is the return trip, the same third
    /// of the time already spent a walk owes.
    #[serde(default)]
    pub early: bool,
}

impl ExpeditionInProgress {
    /// An expedition that set off just now, at the site and length chosen.
    pub fn started(started_unix: i64, site: GatherSite, hours: ExplorationHours) -> Self {
        Self {
            started_unix,
            ends_unix: started_unix.saturating_add(hours.seconds()),
            site,
            hours: hours.hours(),
            early: false,
        }
    }

    /// The same expedition, called home at `now_unix`.
    ///
    /// The return trip is a third of the time already spent: the site is
    /// abandoned, and the way home is not free. The same rule a walk owes, so
    /// the two outings cannot drift into different prices for the same decision.
    pub fn called_home(self, now_unix: i64) -> Self {
        let spent = now_unix.saturating_sub(self.started_unix).max(0);
        Self {
            ends_unix: now_unix.saturating_add(return_trip_seconds(spent)),
            early: true,
            ..self
        }
    }

    /// Whether the expedition is home at this instant.
    pub fn is_home(&self, now_unix: i64) -> bool {
        now_unix >= self.ends_unix
    }

    /// How long is left before the expedition is home; zero once it is.
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

// ---------------------------------------------------------------------------
// 区域探索: five regions × two realms each × ten levels.
//
// A region is a tier band two realms wide, and a level is one step inside it.
// The walk's difficulty, its combat rate, its drop purse and its drop tier are
// all read off the region and the level, so a walk is no longer an outing in a
// vacuum — it is a rung on a five-rung ladder.
// ---------------------------------------------------------------------------

/// The ten exploration regions, shallowest first.
///
/// One region per realm: region one is 炼气, region ten is 真仙, and the region's
/// tier is the realm it stands in — a walk in the fifth region drops 化神 goods,
/// not tier-one. Ten levels split the realm's hundred rungs, so a region read at
/// fifty percent is that realm's fiftieth level.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Region {
    AzurePeaks,
    FrozenWastes,
    FlameCaverns,
    ShadowRealm,
    CelestialCourt,
    StarfallPlains,
    BloodSea,
    VoidSanctum,
    ThunderAbyss,
    ChaosPalace,
}

impl Region {
    /// Every region, shallowest first.
    pub const ALL: [Self; 10] = [
        Self::AzurePeaks,
        Self::FrozenWastes,
        Self::FlameCaverns,
        Self::ShadowRealm,
        Self::CelestialCourt,
        Self::StarfallPlains,
        Self::BloodSea,
        Self::VoidSanctum,
        Self::ThunderAbyss,
        Self::ChaosPalace,
    ];

    /// The region's position on the ladder, zero-based.
    pub const fn index(self) -> usize {
        match self {
            Self::AzurePeaks => 0,
            Self::FrozenWastes => 1,
            Self::FlameCaverns => 2,
            Self::ShadowRealm => 3,
            Self::CelestialCourt => 4,
            Self::StarfallPlains => 5,
            Self::BloodSea => 6,
            Self::VoidSanctum => 7,
            Self::ThunderAbyss => 8,
            Self::ChaosPalace => 9,
        }
    }

    /// The region at a ladder position, clamped to the last one.
    pub fn from_index(index: usize) -> Self {
        Self::ALL[index.min(Self::ALL.len() - 1)]
    }

    /// The region a realm walks: one region per realm, so the ladder of regions
    /// and the ladder of realms are the same ladder read twice.
    pub fn from_realm(realm: super::Realm) -> Self {
        Self::from_index(realm.index())
    }

    /// The realm this region stands in, as a ladder position.
    pub const fn base_realm_index(self) -> usize {
        self.index()
    }

    /// The region's tier, one-based: region one sits on tier one, region ten on
    /// tier ten.
    pub const fn tier_base(self) -> u8 {
        (self.index() as u8) + 1
    }

    /// The region's tier as a single roll tier — the realm it stands in.
    pub const fn tier(self) -> u8 {
        self.tier_base()
    }

    /// The raw power the region's wanderers are built around: the realm's own
    /// floor, which the region's levels climb from.
    #[cfg(test)]
    fn base_power_permille(self) -> u32 {
        let realm = super::Realm::from_index(self.base_realm_index());
        super::drop::realm_power(realm).saturating_mul(150)
    }
}

/// How many levels one region has: ten rungs, and completing the last of a
/// region is the gate to the next.
pub const LEVELS_PER_REGION: u8 = 10;

/// The deepest level clear a save can hold in one region, as a one-based index.
pub const MAX_LEVEL: u8 = LEVELS_PER_REGION;

/// The level a region's progress stands at.
///
/// A region covers one realm's hundred rungs and reads them as ten levels:
/// zero percent is the realm's first level, fifty percent its fiftieth, and a
/// hundred percent its last. The tower and the road then speak the same scale —
/// a floor's warden and a region's requirement are the same body, read twice.
pub fn region_level_at(region: Region, progress_permille: u32) -> u16 {
    let first = (region.base_realm_index() as u16) * super::LAYERS_PER_STAGE + 1;
    let span = u32::from(super::LAYERS_PER_STAGE - 1);
    let step = (progress_permille.min(1_000) * span / 1_000) as u16;
    first.saturating_add(step)
}

/// What a region's progress asks of a body, as power: nine tenths of the
/// tower's warden at the level that progress stands at.
///
/// The road is the tower's softer counterpart by design — the tower is the
/// benchmark and the road is where a save grinds — so the same level costs a
/// tenth less to walk than to climb.
pub const REGION_REQUIREMENT_PERMILLE: u32 = 900;

pub fn region_requirement(region: Region, progress_permille: u32) -> u32 {
    let level = region_level_at(region, progress_permille);
    super::trial::warden_power(level).saturating_mul(REGION_REQUIREMENT_PERMILLE) / 1_000
}

/// What one walk advances, in parts per thousand of the region: a base draw of
/// half a percent to two percent at four hours, multiplied by how far the body
/// stands past what the floor asks.
///
/// The factor is the body's own ratio between a fifth and five times, so the
/// strongest cannot buy more than five walks in one. A twelve-hour walk triples
/// the base before the factor is applied, which is what makes the longer
/// outings worth the wait.
pub const REGION_GAIN_BASE_MIN: u32 = 5;
pub const REGION_GAIN_BASE_MAX: u32 = 20;
/// What a body that has not reached the road earns: a fifth of the base draw.
pub const REGION_GAIN_FACTOR_MIN: u32 = 200;
pub const REGION_GAIN_FACTOR_MAX: u32 = 5_000;
/// Where the floor stops applying, in parts per thousand of the requirement.
///
/// A body at or below two fifths of what the road asks is not walking it — it
/// is being dragged along — so it earns the flat floor and nothing more, however
/// weak it actually is. Above that line the body's own ratio is what pays, so
/// four tenths of the requirement reads as four tenths of a walk. The step at
/// the line is deliberate: the floor exists to keep a hopeless body crawling,
/// not to pay it a share of a road it cannot walk.
pub const REGION_GAIN_FLOOR_CEILING: u32 = 400;

/// How far this body's advantage carries a walk, in parts per thousand: the
/// ratio itself once it is past [`REGION_GAIN_FLOOR_CEILING`], the floor below
/// it, and five times at the top.
pub fn walk_gain_factor(power: u32, requirement: u32) -> u32 {
    let ratio = u64::from(power).saturating_mul(1_000) / u64::from(requirement.max(1));
    let ratio = u32::try_from(ratio).unwrap_or(u32::MAX);
    if ratio <= REGION_GAIN_FLOOR_CEILING {
        REGION_GAIN_FACTOR_MIN
    } else {
        ratio.min(REGION_GAIN_FACTOR_MAX)
    }
}

pub fn walk_gain_permille(
    rng: &mut Rng,
    hours: ExplorationHours,
    power: u32,
    requirement: u32,
) -> u32 {
    let span = u64::from(REGION_GAIN_BASE_MAX - REGION_GAIN_BASE_MIN + 1);
    let base = REGION_GAIN_BASE_MIN + rng.below(span) as u32;
    let factor = walk_gain_factor(power, requirement);
    let base = base.saturating_mul(hours.multiplier_permille() as u32) / 1_000;
    base.saturating_mul(factor) / 1_000
}

/// The chance a walk meets someone, in parts per thousand.
///
/// The old walk carried one hundred and twelve at four hours: every region
/// carries a base of a hundred and fifty, the level adds twenty a rung, and
/// the length multiplies the result. A region-five, level-ten walk at twelve
/// hours meets someone one time in two.
pub const REGION_ENCOUNTER_PERMILLE_BASE: u64 = 150;
pub const REGION_ENCOUNTER_PER_LEVEL: u64 = 20;

pub fn encounter_permille(_region: Region, level: u8, hours: ExplorationHours) -> u64 {
    let level = level.clamp(1, MAX_LEVEL);
    let per_level = REGION_ENCOUNTER_PER_LEVEL.saturating_mul(u64::from(level));
    let base = REGION_ENCOUNTER_PERMILLE_BASE.saturating_add(per_level);
    base.saturating_mul(hours.multiplier_permille()) / 1_000
}

/// The chance a quiet walk turns up gear (rather than a seed). Per level: a
/// level-one walk is one-in-fifty, a level-ten walk is two-in-five.
pub const REGION_GEAR_PERMILLE_BASE: u64 = 40;
pub const REGION_GEAR_PER_LEVEL: u64 = 30;

pub fn region_gear_permille(level: u8, hours: ExplorationHours) -> u64 {
    let level = level.clamp(1, MAX_LEVEL);
    let per_level = REGION_GEAR_PER_LEVEL.saturating_mul(level as u64);
    let base = REGION_GEAR_PERMILLE_BASE.saturating_add(per_level);
    base.saturating_mul(hours.multiplier_permille()) / 1_000
}

/// The chance a quiet walk turns up a seed. Inverse to the gear chance: a level
/// one walk is half seeds, a level-ten walk is about even.
pub const REGION_SEED_PERMILLE_BASE: u64 = 300;
pub const REGION_SEED_PER_LEVEL_DROP: i64 = 30;

pub fn region_seed_permille(level: u8) -> u64 {
    let level = level.clamp(1, MAX_LEVEL);
    let drop = REGION_SEED_PER_LEVEL_DROP.saturating_mul(level as i64);
    REGION_SEED_PERMILLE_BASE.saturating_add_signed(-drop)
}

/// What a defeat costs beyond the walk: the chance the road takes a piece of
/// the cultivator's own gear.
///
/// One in five — well below a victory's find rate, because a defeat is the
/// price of losing rather than a worse way to win. The piece comes off the
/// cultivator's own satchel (and body), not off the wanderer's back.
pub const DEFEAT_DROP_PERMILLE: u64 = 200;

/// The purse a clear walk opens: the first tier's `黄阶 × 0.6` purse.
///
/// The walk is the safe counterpart to a job, and its purse is the job's at
/// sixty percent — the brief's 0.6:1 ratio. The purse is built from the first
/// tier's 黄阶 job so that [`StonePurse::at`] can scale it by the region's tier
/// exactly once; a purse built at the region's own tier would be scaled again
/// when it is opened, and the walk would pay double.
///
/// The level re-purses the band: a level-ten walk pays a third again more than
/// a level-one, so a deep walk reads as a real step up inside the band.
pub fn walk_purse(_region: Region, hours: ExplorationHours, level: u8) -> StonePurse {
    let job = u64::from(super::jobs::job_stones(1, 4, super::arts::ArtGrade::Yellow));
    // The brief's 0.6:1 — the walk pays the job's own purse at sixty percent.
    let scaled = job * 600 / 1_000;
    // The level re-purses the band: a level-ten walk pays a third again more
    // than a level-one, so a deep walk reads as a real step up inside the band.
    let level_factor = 1_000 + 30 * level as u64;
    let purse = (scaled * level_factor / 1_000) as u32;
    StonePurse {
        min: purse,
        max: purse,
    }
    .scaled(hours.multiplier_permille())
}

/// The purse a victorious fight opens: the purse of the clear walk, plus a half
/// for the trouble of beating someone on the road.
pub fn combat_purse(region: Region, hours: ExplorationHours, level: u8) -> StonePurse {
    let quiet = walk_purse(region, hours, level);
    StonePurse {
        min: quiet.min.saturating_mul(3) / 2,
        max: quiet.max.saturating_mul(3) / 2,
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Realm, arts, jobs};
    use super::*;

    // --- Old flat-walk tests (removed with roll_walk) ---------------------
    // Replaced by region-level tests below.

    #[test]
    fn a_walk_lands_on_every_outcome_sooner_or_later() {
        let mut seen = std::collections::BTreeSet::new();
        let region = 0;
        let level = 1;
        for seed in 0..2_000_u64 {
            seen.insert(roll_region_walk(
                &mut Rng::seeded(seed),
                ExplorationHours::Four,
                region,
                level,
            ));
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
        // In a level-one quiet walk, seeds are more common than gear, gear than art.
        let counts = |outcome| {
            (0..10_000_u64)
                .filter(|seed| {
                    roll_region_walk(
                        &mut Rng::seeded(*seed),
                        ExplorationHours::Four,
                        region,
                        level,
                    ) == outcome
                })
                .count()
        };
        let seed = counts(ExplorationOutcome::Seed);
        let gear = counts(ExplorationOutcome::Gear);
        let art = counts(ExplorationOutcome::Art);
        assert!(seed > gear, "Seed {seed} > Gear {gear}");
        assert!(gear > art, "Gear {gear} > Art {art}");
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

    /// A longer walk finds more — measured on the roll itself rather than
    /// asserted from the constant, so a tuning that quietly broke the ladder
    /// would be caught here. Measured on the new region roll at level one,
    /// where the quiet band has room for the length multiplier to be felt.
    #[test]
    fn a_longer_walk_finds_more_by_exactly_its_multiplier() {
        let hits = |hours: ExplorationHours| {
            (0..20_000_u64)
                .filter(|seed| {
                    roll_region_walk(&mut Rng::seeded(*seed), hours, 0, 1)
                        != ExplorationOutcome::Nothing
                })
                .count()
        };
        let four = hits(ExplorationHours::Four) as f64;
        let eight = hits(ExplorationHours::Eight) as f64;
        let twelve = hits(ExplorationHours::Twelve) as f64;
        // The region ladder is not the exact 2.1× / 3.2× of a single band, but
        // it keeps the same direction: eight hours finds more than four, twelve
        // more than eight.
        assert!(eight > four, "eight > four: {eight} > {four}");
        assert!(twelve > eight, "twelve > eight: {twelve} > {eight}");
    }

    /// A day's quiet walks pay about the same whatever their length: six short
    /// ones, three middling, two long. The purse follows the length for exactly
    /// this reason — otherwise the shortest chip would be the money button. The
    /// equality is an approximation because the widening truncates: a 3.2×
    /// purse of five is sixteen, not quite the 3.2 a four-hour walk's own five
    /// would be, so a day's two long walks can end a stone either side of its
    /// six short ones.
    #[test]
    fn a_days_walks_pay_about_the_same_whatever_their_length() {
        let per_day = |hours: ExplorationHours| {
            let walks = 24 / i64::from(hours.hours());
            let purse = WALK_STONES.scaled(hours.multiplier_permille());
            (walks * i64::from(purse.min), walks * i64::from(purse.max))
        };
        let four = per_day(ExplorationHours::Four);
        let eight = per_day(ExplorationHours::Eight);
        let twelve = per_day(ExplorationHours::Twelve);
        let close = |a: (i64, i64), b: (i64, i64)| {
            (a.0 as f64 / b.0 as f64 - 1.0).abs() < 0.1
                && (a.1 as f64 / b.1 as f64 - 1.0).abs() < 0.1
        };
        assert!(
            close(eight, four) && close(twelve, four) && close(twelve, eight),
            "a day's walks pay about the same whatever their length: \
             {four:?} {eight:?} {twelve:?}"
        );
    }

    #[test]
    fn a_walk_is_out_for_its_own_length_and_comes_home_with_nothing_until_it_does() {
        let start = 1_000;
        let walk = WalkInProgress::started(start, ExplorationHours::Four, 0, 1);
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
        let walk = WalkInProgress::started(start, ExplorationHours::Twelve, 0, 1);
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

    // --- Region tests --------------------------------------------------------

    /// Ten regions map one realm each, in order.
    #[test]
    fn ten_regions_cover_one_realm_each() {
        assert_eq!(Region::ALL.len(), Realm::ALL.len());
        for (i, region) in Region::ALL.iter().enumerate() {
            assert_eq!(region.index(), i);
            assert_eq!(region.base_realm_index(), i);
            assert_eq!(region.tier(), (i + 1) as u8);
            let from_realm = Region::from_realm(Realm::ALL[i]);
            assert_eq!(from_realm.index(), i, "region of realm {i:?}");
        }
    }

    /// Fifty percent of a region is that realm's fiftieth level, and the ends
    /// are the realm's first and last rungs.
    #[test]
    fn a_regions_percent_reads_the_realms_own_levels() {
        for (i, region) in Region::ALL.iter().enumerate() {
            let first = (i as u16) * super::super::LAYERS_PER_STAGE + 1;
            assert_eq!(region_level_at(*region, 0), first);
            assert_eq!(region_level_at(*region, 500), first + 49);
            assert_eq!(region_level_at(*region, 1_000), first + 99);
        }
    }

    /// The requirement is nine tenths of what the tower asks at the same level,
    /// so the road is the tower's softer counterpart.
    #[test]
    fn a_regions_requirement_is_nine_tenths_of_the_tower() {
        let region = Region::FlameCaverns;
        for progress in [0_u32, 250, 500, 750, 1_000] {
            let level = region_level_at(region, progress);
            let tower = super::super::trial::warden_power(level);
            let requirement = region_requirement(region, progress);
            assert_eq!(requirement, tower * REGION_REQUIREMENT_PERMILLE / 1_000);
        }
    }

    /// The gain is the base draw times the body's advantage, clamped at both
    /// ends: a body five times the floor cannot buy more than five walks, and a
    /// body a tenth of the floor still moves a fifth of the base.
    #[test]
    fn the_gain_follows_the_body_and_stops_at_the_clamps() {
        let requirement = 10_000_u32;
        let (mut low, mut high) = (u32::MAX, 0);
        for seed in 0..500u64 {
            let gain = walk_gain_permille(
                &mut Rng::seeded(seed),
                ExplorationHours::Four,
                10_000,
                requirement,
            );
            low = low.min(gain);
            high = high.max(gain);
        }
        // A body at the floor draws the base band: half a percent to two.
        assert!((REGION_GAIN_BASE_MIN..=REGION_GAIN_BASE_MAX).contains(&low));
        assert!((REGION_GAIN_BASE_MIN..=REGION_GAIN_BASE_MAX).contains(&high));

        let weak = walk_gain_permille(
            &mut Rng::seeded(1),
            ExplorationHours::Four,
            requirement / 10,
            requirement,
        );
        let strong = walk_gain_permille(
            &mut Rng::seeded(1),
            ExplorationHours::Twelve,
            requirement * 50,
            requirement,
        );
        assert!(weak >= 1, "a hopeless body still crawls forward: {weak}");
        assert!(strong <= 320, "the strongest walk is capped: {strong}");
        assert!(
            strong > weak * 10,
            "the strong walk pays far more: {strong} vs {weak}"
        );
    }

    /// The floor is for the bodies that cannot walk the road at all: at or below
    /// four tenths of what it asks, the walk pays the flat fifth. One part per
    /// thousand past that line the body's own ratio pays, and the cap holds it
    /// at five.
    #[test]
    fn the_floor_stops_at_four_tenths_of_the_requirement() {
        let requirement = 1_000_u32;
        for ratio in [0_u32, 100, 300, 400] {
            assert_eq!(
                walk_gain_factor(requirement * ratio / 1_000, requirement),
                REGION_GAIN_FACTOR_MIN,
                "a body at {ratio} per mille of the requirement must earn the floor"
            );
        }
        assert_eq!(
            walk_gain_factor(requirement * 401 / 1_000, requirement),
            401,
            "one part past the line and the body's own ratio pays"
        );
        assert_eq!(walk_gain_factor(requirement * 3, requirement), 3_000);
        assert_eq!(
            walk_gain_factor(requirement * 50, requirement),
            REGION_GAIN_FACTOR_MAX,
            "the cap holds at five"
        );
    }

    /// A region's floor scales with its tier band: the deeper the region, the
    /// higher the floor.
    #[test]
    fn region_floor_scales_with_band() {
        let r1 = Region::AzurePeaks.base_power_permille();
        let r5 = Region::CelestialCourt.base_power_permille();
        assert!(r5 > r1, "region 5 floor > region 1");
    }

    /// The encounter rate is the base scaled by level, and longer walks add
    /// their multiplier: a twelve-hour walk at level-ten is about a third.
    #[test]
    fn encounter_permille_scales_with_level_and_length() {
        let l1 = encounter_permille(Region::AzurePeaks, 1, ExplorationHours::Four);
        let l10 = encounter_permille(Region::AzurePeaks, 10, ExplorationHours::Four);
        assert!(l10 > l1, "level ten meets more often than level one");
        let l4 = encounter_permille(Region::AzurePeaks, 1, ExplorationHours::Four);
        let l12 = encounter_permille(Region::AzurePeaks, 1, ExplorationHours::Twelve);
        assert!(l12 > l4, "a longer walk meets more often");
        assert!(
            l12 <= 1_000,
            "the encounter rate stays below the whole roll"
        );
    }

    /// A gear find is more likely than at the old flat rate when the level is
    /// deep: the level re-rates the quiet finds.
    #[test]
    fn deeper_levels_find_gear_more_often() {
        let shallow = region_gear_permille(1, ExplorationHours::Four);
        let deep = region_gear_permille(10, ExplorationHours::Four);
        assert!(deep > shallow, "level ten finds more gear than level one");
    }

    /// The purse carries the brief's 0.6:1 ratio against the same tier's 黄阶 job.
    #[test]
    fn walk_purse_runs_sixty_percent_of_the_yellow_job() {
        // A first-region, level-one, four-hour walk is priced against the first
        // tier's own 黄阶 job, and the purse is scaled by the region's tier when
        // it is opened — so the pocket is the job's at sixty percent. The purse
        // is built from the first tier's job and scaled by the region's tier,
        // so the ratio is exact up to the integer truncation at each step.
        let region = Region::AzurePeaks;
        let purse = walk_purse(region, ExplorationHours::Four, 1);
        let (min, max) = purse.at(region.tier());
        let job = jobs::job_stones(region.tier(), 4, arts::ArtGrade::Yellow);
        // The level factor at level one is 1.03, so the pocket is a little over
        // the sixty percent the brief names — but never under it by more than
        // the truncation the intermediate steps leave behind.
        let expected = job * 600 / 1_000;
        assert!(
            min + 2 >= expected,
            "purse {min} is under 黄阶×0.6 ({job} × 0.6 = {expected})"
        );
        assert!(
            max <= expected + 2,
            "purse {max} is far over 黄阶×0.6 ({job} × 0.6 = {expected})"
        );
    }

    /// A victory opens the combat purse, and the combat purse outpays the quiet
    /// purse: the road's richer purse is the one that costs a fight.
    #[test]
    fn combat_purse_outpays_quiet_purse() {
        let quiet = walk_purse(Region::AzurePeaks, ExplorationHours::Four, 5);
        let combat = combat_purse(Region::AzurePeaks, ExplorationHours::Four, 5);
        assert!(combat.min > quiet.min, "combat wins more than a quiet walk");
    }
}
