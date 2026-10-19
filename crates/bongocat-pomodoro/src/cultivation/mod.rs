//! The cultivation ladder the focus sessions feed: ten realms of a hundred
//! layers each, the attributes those layers buy, the gear and the herbs that
//! add to them, and the guardian that stands between a full realm and the next.
//!
//! Everything here is a pure function of the persisted document plus the wall
//! clock the caller hands in. The one exception is the drop roll, which is
//! seeded from that clock: a session that finishes at the same second twice
//! rolls the same item, which is what makes the reward path testable without a
//! dependency on a random source.
//!
//! The rules, in one place:
//!
//! * Growth is the experience: a finished focus session adds
//!   [`session_growth`] — [`SESSION_BASE_GROWTH`] for the first of the day, then
//!   scaled by the session number and the realm — and every whole local day
//!   without one charges [`absence_penalty`], which grows with the run of missed
//!   days and the realm the cultivator has reached.
//! * Growth never falls below the floor of the realm already entered, so a
//!   penalty costs progress but never a breakthrough.
//! * A realm ends at layer 100 and the next one opens only by beating its
//!   guardian; the guardian is deliberately stronger than a naked cultivator at
//!   that cap, so gear and herbs are the way through.
//! * Gear drops are one roll per session at [`EQUIPMENT_DROP_PERMILLE`], seeds
//!   at [`SEED_DROP_PERMILLE`]; both are deliberately rare next to the growth
//!   a session already pays.
//! * Every level pays [`FREE_POINTS_PER_LEVEL`] point the user places by hand,
//!   on top of what the level itself raises. The points are paid against the
//!   highest level ever reached, so a penalty that lowers the ladder never
//!   takes back a point already earned.
//! * A harvest pays its attribute and then rolls once at
//!   [`SEED_RETURN_PERMILLE`] for a seed of the same attribute, the same tier
//!   or one higher, which is what lets a garden keep running.

mod battle;
mod curve;
mod document;
mod garden;
mod items;
mod state;

#[cfg(test)]
mod tests;

pub use battle::{BattleReport, Guardian};
pub use curve::{LAYERS_PER_STAGE, LevelProgress, MAX_LEVEL, level_for_growth, threshold_for};
pub use document::CultivationDocument;
pub use garden::{MAX_SEED_TIER, PLOT_COUNT, PlotRecord, SeedStack};
pub use items::{EquipSlot, EquipmentRecord, Rarity, SLOT_COUNT};
pub use state::{
    AbsenceCharge, BreakthroughOutcome, CropView, Cultivation, CultivationView, Drop, DropView,
    GuardianView, HarvestOutcome, ItemView, PlotView, SeedView, SessionReward,
};

use serde::{Deserialize, Serialize};

/// Growth the first finished focus session of a local day pays.
///
/// Every further session the same day is worth more: see [`session_growth`]. A
/// single flat base keeps the first sit-down of the day honest, and the growth it
/// pays is deliberately large so one finished pomodoro is felt on the ladder.
pub const SESSION_BASE_GROWTH: u32 = 500;

/// How much each extra session the same day multiplies the base growth by, per
/// step past the first.
///
/// The second session of the day is `BASE * STEP`, the third `BASE * STEP^2`, and
/// so on; the escalation rewards a day of repeated focus without letting a single
/// long session out-earn several short ones.
const SESSION_GROWTH_STEP: f64 = 1.1;

/// Growth the first missed day costs.
///
/// Every further consecutive missed day costs more: see [`absence_penalty`]. The
/// base is deliberately harsh so a day with no session is felt, but it is a fixed
/// amount rather than a multiple of the base growth, which keeps a lapse from
/// wiping out a single session's worth of progress in one stroke.
pub const ABSENCE_BASE_PENALTY: u32 = 800;

/// How much each extra consecutive missed day multiplies the base penalty by, per
/// step past the first.
///
/// The second missed day is `BASE * STEP`, the third `BASE * STEP^2`, and so on,
/// so a run of empty days stings proportionally more the longer it goes.
const ABSENCE_PENALTY_STEP: f64 = 1.2;

/// The chance one finished session drops a piece of gear, in parts per thousand.
pub const EQUIPMENT_DROP_PERMILLE: u64 = 60;

/// The chance one finished session drops a seed, in parts per thousand. The
/// rolls are exclusive: a session drops gear, or a seed, or nothing.
pub const SEED_DROP_PERMILLE: u64 = 50;

/// The chance one harvest gives a seed back, in parts per thousand.
///
/// Low enough that a harvest is still a payout rather than a trade, high enough
/// that a garden keeps running without a session having to feed it: one in
/// seven harvests returns a seed.
pub const SEED_RETURN_PERMILLE: u64 = 150;

/// The share of returned seeds that come back one tier higher, in percent.
///
/// The rest come back at the tier that was planted, so the garden drifts
/// upwards slowly instead of ratcheting.
const SEED_RETURN_UPGRADE_PERCENT: u64 = 30;

/// Free attribute points a level pays on the way up.
///
/// The level already raises every attribute on its own; this is the point the
/// user spends, so climbing the ladder is a decision rather than a fixed curve.
pub const FREE_POINTS_PER_LEVEL: u32 = 1;

/// How many realms the ladder holds. Each one carries [`LAYERS_PER_STAGE`]
/// layers, so the ladder is ten realms of a hundred levels.
pub const STAGE_COUNT: usize = 10;

/// The ten realms, in the order a cultivator climbs them.
///
/// The index is the realm's position on the ladder and the value stored in the
/// document, so a realm is never persisted by name.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Realm {
    QiRefining,
    Foundation,
    GoldenCore,
    NascentSoul,
    SpiritTransformation,
    VoidRefinement,
    Integration,
    Mahayana,
    Tribulation,
    Immortal,
}

impl Realm {
    /// Every realm, first to last.
    pub const ALL: [Self; STAGE_COUNT] = [
        Self::QiRefining,
        Self::Foundation,
        Self::GoldenCore,
        Self::NascentSoul,
        Self::SpiritTransformation,
        Self::VoidRefinement,
        Self::Integration,
        Self::Mahayana,
        Self::Tribulation,
        Self::Immortal,
    ];

    /// The realm's position on the ladder.
    pub const fn index(self) -> usize {
        match self {
            Self::QiRefining => 0,
            Self::Foundation => 1,
            Self::GoldenCore => 2,
            Self::NascentSoul => 3,
            Self::SpiritTransformation => 4,
            Self::VoidRefinement => 5,
            Self::Integration => 6,
            Self::Mahayana => 7,
            Self::Tribulation => 8,
            Self::Immortal => 9,
        }
    }

    /// The realm at a ladder position, clamped to the last one.
    pub fn from_index(index: usize) -> Self {
        Self::ALL[index.min(STAGE_COUNT - 1)]
    }

    /// The realm a level sits in. Levels are one-based.
    pub fn from_level(level: u16) -> Self {
        let index = usize::from(level.saturating_sub(1)) / usize::from(LAYERS_PER_STAGE);
        Self::from_index(index)
    }

    /// The catalog key that names this realm.
    pub const fn name_key(self) -> &'static str {
        match self {
            Self::QiRefining => "stats.stage.name.1",
            Self::Foundation => "stats.stage.name.2",
            Self::GoldenCore => "stats.stage.name.3",
            Self::NascentSoul => "stats.stage.name.4",
            Self::SpiritTransformation => "stats.stage.name.5",
            Self::VoidRefinement => "stats.stage.name.6",
            Self::Integration => "stats.stage.name.7",
            Self::Mahayana => "stats.stage.name.8",
            Self::Tribulation => "stats.stage.name.9",
            Self::Immortal => "stats.stage.name.10",
        }
    }

    /// The last layer of this realm: the level its guardian gates.
    pub const fn cap_level(self) -> u16 {
        (self.index() as u16 + 1) * LAYERS_PER_STAGE
    }

    /// The realm after this one, or `None` at the top of the ladder.
    pub fn next(self) -> Option<Self> {
        Self::ALL.get(self.index() + 1).copied()
    }
}

/// The four attributes a level, a piece of gear and a herb each move.
///
/// The order is the order every surface lists them in, and the order the
/// persisted bonuses are summed in; it is deliberately not alphabetical.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Attribute {
    Root,
    Strength,
    Spirit,
    Agility,
}

impl Attribute {
    /// Every attribute, in display order.
    pub const ALL: [Self; 4] = [Self::Root, Self::Strength, Self::Spirit, Self::Agility];

    /// The attribute's position in [`Self::ALL`].
    pub const fn index(self) -> usize {
        match self {
            Self::Root => 0,
            Self::Strength => 1,
            Self::Spirit => 2,
            Self::Agility => 3,
        }
    }

    /// The catalog key that names this attribute.
    pub const fn name_key(self) -> &'static str {
        match self {
            Self::Root => "cultivation.attribute.root",
            Self::Strength => "cultivation.attribute.strength",
            Self::Spirit => "cultivation.attribute.spirit",
            Self::Agility => "cultivation.attribute.agility",
        }
    }

    /// The catalog key that explains what this attribute does. Kept beside
    /// [`Self::name_key`] so a surface can put the explanation on a tooltip
    /// without a second attribute-to-key table drifting from this one.
    pub const fn desc_key(self) -> &'static str {
        match self {
            Self::Root => "cultivation.attribute.root_desc",
            Self::Strength => "cultivation.attribute.strength_desc",
            Self::Spirit => "cultivation.attribute.spirit_desc",
            Self::Agility => "cultivation.attribute.agility_desc",
        }
    }

    /// This attribute's value in a set.
    pub const fn of(self, attributes: Attributes) -> i32 {
        match self {
            Self::Root => attributes.root,
            Self::Strength => attributes.strength,
            Self::Spirit => attributes.spirit,
            Self::Agility => attributes.agility,
        }
    }

    /// Add to this attribute in a set.
    pub fn add_to(self, attributes: &mut Attributes, amount: i32) {
        let slot = match self {
            Self::Root => &mut attributes.root,
            Self::Strength => &mut attributes.strength,
            Self::Spirit => &mut attributes.spirit,
            Self::Agility => &mut attributes.agility,
        };
        *slot = slot.saturating_add(amount);
    }

    /// The attribute at a position in [`Self::ALL`], clamped to the last one.
    pub fn from_index(index: usize) -> Self {
        Self::ALL[index.min(Self::ALL.len() - 1)]
    }
}

/// A set of attribute values.
///
/// Bonuses are stored as the numbers they add rather than as a multiplier, so a
/// tuned catalog never re-scores gear a user already owns.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Attributes {
    #[serde(default)]
    pub root: i32,
    #[serde(default)]
    pub strength: i32,
    #[serde(default)]
    pub spirit: i32,
    #[serde(default)]
    pub agility: i32,
}

impl Attributes {
    /// An explicit set.
    pub const fn new(root: i32, strength: i32, spirit: i32, agility: i32) -> Self {
        Self {
            root,
            strength,
            spirit,
            agility,
        }
    }

    /// The sum of two sets.
    pub fn plus(self, other: Self) -> Self {
        Self {
            root: self.root.saturating_add(other.root),
            strength: self.strength.saturating_add(other.strength),
            spirit: self.spirit.saturating_add(other.spirit),
            agility: self.agility.saturating_add(other.agility),
        }
    }

    /// The combat power these attributes buy.
    ///
    /// One scale for every comparison in this module: two fighters are compared
    /// by this number alone, so the guardian's power and the cultivator's are
    /// computed the same way and can be shown side by side.
    pub fn power(self) -> u32 {
        let weighted = i64::from(self.strength) * 20
            + i64::from(self.spirit) * 20
            + i64::from(self.root) * 15
            + i64::from(self.agility) * 12;
        u32::try_from(weighted.max(0) / 10).unwrap_or(u32::MAX)
    }

    /// The points this set holds in total, used to count what has been spent.
    pub fn total(self) -> u32 {
        let sum = i64::from(self.root)
            + i64::from(self.strength)
            + i64::from(self.spirit)
            + i64::from(self.agility);
        u32::try_from(sum.max(0)).unwrap_or(u32::MAX)
    }
}

/// The attributes a level alone carries.
///
/// Linear in the level with a flat jump at every realm boundary, so entering a
/// realm is felt at once instead of being a rounding difference. The jump is
/// what makes a breakthrough worth the fight even before any gear is found.
pub fn base_attributes(level: u16) -> Attributes {
    let level = i32::from(level.clamp(1, MAX_LEVEL));
    let stage = Realm::from_level(level as u16).index() as i32;
    Attributes {
        root: 10 + level * 2 + stage * 60,
        strength: 8 + level * 2 + stage * 60,
        spirit: 8 + level * 2 + stage * 60,
        agility: 6 + level + stage * 40,
    }
}

/// The growth one finished session pays, given which number it is in the local
/// day (one-based) and the cultivator's realm (one-based stage).
///
/// The first session of the day is a flat [`SESSION_BASE_GROWTH`]. Every further
/// one is the base grown by [`SESSION_GROWTH_STEP`] for each step past the first
/// and by the realm's stage, so sitting down again the same day — or climbing the
/// ladder — is rewarded harder. The realm factor means a missed day hurts more
/// the higher a cultivator has climbed, the same way a longer fall lands harder.
pub fn session_growth(session_number: u32, stage: u32) -> u32 {
    if session_number <= 1 {
        return SESSION_BASE_GROWTH;
    }
    let exponent = f64::from(session_number - 1);
    let value = f64::from(SESSION_BASE_GROWTH)
        * SESSION_GROWTH_STEP.powf(exponent)
        * f64::from(stage.max(1));
    // Growth is an unsigned whole number; the float product saturates rather than
    // wrapping if a long run of same-day sessions somehow overflowed it.
    value as u32
}

/// Growth a missed day costs after `missed_streak` consecutive missed days, at a
/// cultivator in realm `stage` (one-based).
///
/// A missed day costs [`ABSENCE_BASE_PENALTY`], scaled by [`ABSENCE_PENALTY_STEP`]
/// for every step past the first (the `missed_streak` is zero-based, so the first
/// missed day is the base) and by the realm's stage. The stage factor ties the
/// cost to how far the cultivator has climbed, so a lapse at the top of the
/// ladder is far dearer than one at the bottom — and at the starting realm
/// (stage one) the first missed day is exactly [`ABSENCE_BASE_PENALTY`].
pub fn absence_penalty(missed_streak: u32, stage: u32) -> u32 {
    let exponent = f64::from(missed_streak);
    let value = f64::from(ABSENCE_BASE_PENALTY)
        * ABSENCE_PENALTY_STEP.powf(exponent)
        * f64::from(stage.max(1));
    value as u32
}

/// A small deterministic random source.
///
/// SplitMix64: three lines, no dependency, and reproducible from a seed the
/// caller controls — which is what lets a test assert an exact drop.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) const fn seeded(seed: u64) -> Self {
        Self(seed)
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    /// A value in `0..bound`. A zero bound answers zero rather than panicking,
    /// because every caller here derives the bound from a table length.
    pub(crate) fn below(&mut self, bound: u64) -> u64 {
        if bound == 0 {
            return 0;
        }
        self.next_u64() % bound
    }
}
