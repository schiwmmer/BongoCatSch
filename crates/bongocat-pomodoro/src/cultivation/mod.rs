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
//! * The four attributes feed two schools: 根骨 and 仙力 make the spell half,
//!   力量 and 身法 the martial half, and each pair pays a synergy bonus on its
//!   shorter side. See [`Attributes::power`].
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

mod aptitude;
mod arts;
mod battle;
mod curve;
mod daily;
mod document;
mod drop;
mod duel;
mod events;
mod explore;
mod garden;
mod items;
mod opponent;
mod proving;
mod shop;
mod state;
mod trade;
mod trial;

#[cfg(test)]
mod tests;

pub use aptitude::{
    APTITUDE_MAX_TENTHS, AptitudeHerb, HerbView, base_success_permille, success_permille,
};
pub use arts::{
    APTITUDE_ART_IDS, APTITUDE_ART_TENTHS_PER_RANK, ART_COUNT, ArtBlueprint, ArtGrade, ArtKind,
    ArtOutcome, ArtView, MasteredArtView, OwnedArt, RECENT_ARTS_CAP, art_aptitude_tenths,
    art_aptitude_tenths_at, art_attributes, art_bonus, art_bonus_at, art_payout_at, art_view,
    blueprint, external_damage, external_power_gain, mastered_art_aptitude_payout,
    mastered_art_aptitude_tenths, mastered_art_attributes, mastered_art_bonus,
    mastered_art_bonus_payout, mastered_art_payout, mastered_art_view, mastery_permille,
    printed_pomodoros, required_pomodoros,
};
pub use battle::{BattleReport, Guardian};
// `duel` itself stays crate-private: it takes the seeded `Rng`, which is nobody
// else's business. A surface asks the state machine to fight, not the engine.
pub use curve::{
    LAYERS_PER_REALM, LAYERS_PER_STAGE, LayerRank, LevelProgress, MAX_LEVEL, RANKS_PER_LAYER,
    layer_rank, level_for_growth, threshold_for,
};
pub use daily::DailyKind;
pub use document::CultivationDocument;
pub use drop::{
    POWER_FACTOR_MAX_PERMILLE, POWER_FACTOR_MIN_PERMILLE, QUALITY_BASE_PER_BILLION,
    QUALITY_STEP_DIVISOR, power_factor_permille, quality_chance_per_billion, quality_per_billion,
    realm_power, tier_for_power,
};
pub use duel::{
    ArtStrike, DUEL_RECORD_CAP, DUEL_ROUND_CAP, DuelRecord, DuelReport, DuelRound, FightKind,
    Fighter, GUARDIAN_NAME, PLAYER_NAME, WANDERER_NAME,
};
pub use events::{BonusFind, EVENT_LOG_CAP, EventDetail, EventRecord, MarrowSource, SaleKind};
pub use explore::{
    BREAKTHROUGH_COOLDOWN_SECONDS, ENCOUNTER_PERMILLE, ExplorationHours, ExplorationOutcome,
    VICTORY_SEED_PERMILLE, WALK_RECORD_CAP, WalkInProgress, WalkRecord,
};
pub use garden::{
    CropRecord, FREE_PLOT_COUNT, MAX_PLOT_COUNT, MAX_SEED_TIER, PlotRecord, SeedGrade, SeedStack,
    grow_seconds, harvest_points,
};
pub use items::{
    EquipSlot, EquipmentRecord, GearEffect, MAX_STAR, Rarity, SLOT_COUNT, STAR_SUCCESS_PERMILLE,
    star_multiplier_permille, star_success_permille,
};
pub use proving::{ProvingOutcome, TRIAL_CHANCES_CAP};
pub use shop::SESSION_EARNINGS;
pub use shop::{
    SEED_STONES_PER_POINT, SHOP_MAX_OFFERS, SHOP_MIN_OFFERS, ShopOffer, ShopOfferView, art_price,
    equipment_price, offers_for_day, roll_offers, seed_price, star_up_cost,
};
pub use state::{
    AbsenceCharge, AptitudeUse, BreakthroughOutcome, CropView, Cultivation, CultivationView,
    DailyOutcome, Drop, DropView, ExplorationFindView, GuardianView, HarvestOutcome, ItemView,
    PlotView, PowerBreakdown, SeedView, SessionReward, StarOutcome, TrainingView, TrialBlocked,
    TrialOutcome,
};
pub use trade::{TradeRefusal, session_earnings};
pub use trial::{
    TRIAL_ATTEMPTS_PER_DAY, TRIAL_MAX_FLOOR, TRIAL_OPPONENT_NAME, floor_level, warden_power,
};

use serde::{Deserialize, Serialize};

/// Why an action the page offered could not be carried out.
///
/// The page has to say something when a click does nothing: "灵石不够" and
/// "还没到时候" are different sentences, and a silent no-op teaches the user
/// that the button is broken. The reasons live here rather than in the surfaces
/// so that the page and the domain cannot disagree about what happened — the
/// surface reads the reason the domain recorded, it never invents one.
///
/// A reason is carried as a catalog key rather than as text, because the text
/// has to be translated and the key is what the catalog is written in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    /// A purchase or a sale was refused; the trade layer says which.
    Trade(TradeRefusal),
    /// A walk was asked for while the last one is still on cooldown.
    ExploreCooldown,
    /// A trial was asked for while the cultivator is still out on a walk: the
    /// tower is climbed at home, so an unreturned walk closes it.
    TrialOut,
    /// A trial was asked for after the day's floors were used up.
    TrialDrained,
    /// A trial was asked for on a save that has already cleared the top floor.
    TrialTop,
    /// A 试炼 was asked for with no chance banked: a finished pomodoro pays one.
    ProvingSpent,
    /// A 试炼 was asked for while the cultivator is still out on a walk: the
    /// courtyard is fought in, so an unreturned walk closes it too.
    ProvingOut,
}

impl Refusal {
    /// The catalog key saying why.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Trade(trade) => trade.key(),
            Self::ExploreCooldown => "cultivation.explore.refuse_cooldown",
            Self::TrialOut => "cultivation.trial.refuse_out",
            Self::TrialDrained => "cultivation.trial.refuse_drained",
            Self::TrialTop => "cultivation.trial.refuse_top",
            Self::ProvingSpent => "cultivation.proving.refuse_empty",
            Self::ProvingOut => "cultivation.proving.refuse_out",
        }
    }
}

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

/// How often one finished pomodoro pays a bonus of some other kind, in parts
/// per thousand: one in a thousand.
///
/// A pomodoro is the ladder's main fuel, and this is the ladder occasionally
/// noticing: a purse, a herb, a book, a piece of gear — one thing, at the
/// session's own strength. Rare enough that the notice is worth reading, and
/// often enough that a week of focus has a story in it.
pub const SESSION_BONUS_PERMILLE: u64 = 1;

/// What the thousandth's luck pays, per kind, in parts per thousand of its own
/// roll. Gear and seeds are the ordinary finds; 灵石 is the consolation; the
/// two 潜质 herbs and the 功法 are what a jackpot is *worth*.
const BONUS_GEAR_PERMILLE: u64 = 350;
const BONUS_SEED_PERMILLE: u64 = 350;
const BONUS_STONES_PERMILLE: u64 = 200;
const BONUS_INSIGHT_PERMILLE: u64 = 60;
const BONUS_WISDOM_PERMILLE: u64 = 30;

/// The six shares are one roll: the rest of the thousand — ten — is a 功法.
const _: () = assert!(
    BONUS_GEAR_PERMILLE
        + BONUS_SEED_PERMILLE
        + BONUS_STONES_PERMILLE
        + BONUS_INSIGHT_PERMILLE
        + BONUS_WISDOM_PERMILLE
        < 1_000
);

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

/// What one point of 根骨 adds to the spell half of the power.
///
/// The channel matters, but the force matters more: 仙力 carries twice the
/// weight of 根骨, so a spellcaster leans on mana while root is what lets the
/// mana be spent at all.
const ROOT_WEIGHT: i64 = 15;

/// What one point of 仙力 adds to the spell half of the power.
const MANA_WEIGHT: i64 = 20;

/// What one point of 力量 adds to the martial half of the power.
const STRENGTH_WEIGHT: i64 = 20;

/// What one point of 身法 adds to the martial half of the power.
const AGILITY_WEIGHT: i64 = 12;

/// What a balanced pair adds on top of its two weights, per point of the shorter
/// half.
///
/// This is the link between the two attributes of a school: a spell needs both
/// 根骨 and 仙力, a technique needs both 力量 and 身法, so the pair is scored by
/// its weaker half as well as by its parts. Pouring every point into the heavier
/// attribute of a pair therefore wastes the bonus, which is what makes the four
/// numbers a decision instead of one number and three decorations.
const PAIR_SYNERGY_WEIGHT: i64 = 10;

/// The power scale: the weighted sum is divided by this to reach the number the
/// surfaces show and the fights compare.
const POWER_SCALE: i64 = 10;

/// Turn a weighted sum into the power number, saturation instead of wrapping.
fn scaled_power(weighted: i64) -> u32 {
    u32::try_from(weighted.max(0) / POWER_SCALE).unwrap_or(u32::MAX)
}

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
    ManaTransformation,
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
        Self::ManaTransformation,
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
            Self::ManaTransformation => 4,
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
            Self::ManaTransformation => "stats.stage.name.5",
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
    Mana,
    Agility,
}

impl Attribute {
    /// Every attribute, in display order.
    pub const ALL: [Self; 4] = [Self::Root, Self::Strength, Self::Mana, Self::Agility];

    /// The attribute's position in [`Self::ALL`].
    pub const fn index(self) -> usize {
        match self {
            Self::Root => 0,
            Self::Strength => 1,
            Self::Mana => 2,
            Self::Agility => 3,
        }
    }

    /// The catalog key that names this attribute.
    pub const fn name_key(self) -> &'static str {
        match self {
            Self::Root => "cultivation.attribute.root",
            Self::Strength => "cultivation.attribute.strength",
            Self::Mana => "cultivation.attribute.mana",
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
            Self::Mana => "cultivation.attribute.mana_desc",
            Self::Agility => "cultivation.attribute.agility_desc",
        }
    }

    /// This attribute's value in a set.
    pub const fn of(self, attributes: Attributes) -> i32 {
        match self {
            Self::Root => attributes.root,
            Self::Strength => attributes.strength,
            Self::Mana => attributes.mana,
            Self::Agility => attributes.agility,
        }
    }

    /// Add to this attribute in a set.
    pub fn add_to(self, attributes: &mut Attributes, amount: i32) {
        let slot = match self {
            Self::Root => &mut attributes.root,
            Self::Strength => &mut attributes.strength,
            Self::Mana => &mut attributes.mana,
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
    /// Spell force. Renamed from `spirit`; the alias keeps an older document —
    /// one written before 法力 was the word for it — reading its own number.
    #[serde(default, alias = "spirit")]
    pub mana: i32,
    #[serde(default)]
    pub agility: i32,
}

impl Attributes {
    /// An explicit set.
    pub const fn new(root: i32, strength: i32, mana: i32, agility: i32) -> Self {
        Self {
            root,
            strength,
            mana,
            agility,
        }
    }

    /// The sum of two sets.
    pub fn plus(self, other: Self) -> Self {
        Self {
            root: self.root.saturating_add(other.root),
            strength: self.strength.saturating_add(other.strength),
            mana: self.mana.saturating_add(other.mana),
            agility: self.agility.saturating_add(other.agility),
        }
    }

    /// The combat power these attributes buy.
    ///
    /// One scale for every comparison in this module: two fighters are compared
    /// by this number alone, so the guardian's power and the cultivator's are
    /// computed the same way and can be shown side by side.
    ///
    /// It is the sum of the two schools rather than one flat weighting, because
    /// the four attributes are not four independent numbers: a spell needs a
    /// body to channel it and a technique needs a body to land it. Splitting the
    /// power into [`Self::spell_power`] and [`Self::martial_power`] lets a
    /// surface show which half a piece of gear actually feeds.
    pub fn power(self) -> u32 {
        self.spell_power().saturating_add(self.martial_power())
    }

    /// The spell half (法术): 根骨 carries the channel, 仙力 is the force.
    ///
    /// The two are paired, so the weaker of them pays as well: each point of the
    /// shorter half adds [`PAIR_SYNERGY_WEIGHT`] on top of its own weight. That
    /// is what makes a spellcaster raise both instead of pouring everything into
    /// the heavier attribute — a lopsided pair wastes the synergy.
    pub fn spell_power(self) -> u32 {
        let root = i64::from(self.root).max(0);
        let mana = i64::from(self.mana).max(0);
        let weighted =
            root * ROOT_WEIGHT + mana * MANA_WEIGHT + root.min(mana) * PAIR_SYNERGY_WEIGHT;
        scaled_power(weighted)
    }

    /// The martial half (武术): 力量 is the force, 身法 is how well it lands.
    ///
    /// Paired on the same rule as [`Self::spell_power`], so a fighter who only
    /// stacks strength hits softer than one who trains both.
    pub fn martial_power(self) -> u32 {
        let strength = i64::from(self.strength).max(0);
        let agility = i64::from(self.agility).max(0);
        let weighted = strength * STRENGTH_WEIGHT
            + agility * AGILITY_WEIGHT
            + strength.min(agility) * PAIR_SYNERGY_WEIGHT;
        scaled_power(weighted)
    }

    /// The points this set holds in total, used to count what has been spent.
    pub fn total(self) -> u32 {
        let sum = i64::from(self.root)
            + i64::from(self.strength)
            + i64::from(self.mana)
            + i64::from(self.agility);
        u32::try_from(sum.max(0)).unwrap_or(u32::MAX)
    }
}

/// What the four attributes are worth once they are read as a fighter.
///
/// These are shown and fought with, never allocated: a point can only go into
/// one of the four above, and everything here follows from those four. That is
/// what keeps the panel honest — the numbers the user compares are the numbers
/// the fight uses, and there is no second pool of points that can make a build
/// look stronger than it fights.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Derived {
    /// 攻击力: what a bare hand and every 外功 scale from.
    pub attack: u32,
    /// 生命值: how much damage can be taken before the fight is lost.
    pub health: u32,
    /// 内力值: the pool a 外功 spends. Empty, the cultivator falls back on 攻击力.
    pub inner: u32,
    /// 法术强度: what a spell scales from.
    pub spell: u32,
    /// 法抗: subtracted from incoming 法术强度.
    pub resist: u32,
    /// 速度: how often the cultivator acts.
    pub speed: u32,
    /// 命中: how hard the fighter's blows are to slip, as a plain number the
    /// fight reads against the other side's 闪避.
    pub hit: u32,
    /// 闪避: how hard the fighter is to hit, as a plain number the fight reads
    /// against the attacker's 命中.
    pub evade: u32,
    /// 潜质: how fast 功法 are learned and how rarely the attempt fails.
    pub aptitude: u32,
}

/// One of the nine numbers the four attributes turn into, and the arithmetic
/// that produces it.
///
/// The base, the per-point gain and the ceiling are the very constants
/// [`Attributes::derived`] reads, so a surface that shows the recipe shows the
/// arithmetic that actually ran: there is no second table to drift from the
/// first, and a number the page prints can be checked by hand against the
/// numbers beside it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DerivedBreakdown {
    /// The catalog key naming the number.
    pub key: &'static str,
    /// The attribute that feeds it.
    pub attribute: Attribute,
    /// That attribute's current value.
    pub attribute_value: i32,
    /// What every cultivator starts from.
    pub base: u32,
    /// What one point of the attribute adds.
    pub per_point: u32,
    /// The ceiling, where the number has one.
    pub cap: Option<u32>,
    /// What the arithmetic yields before the ceiling.
    pub computed: u32,
    /// What the page shows: the arithmetic, held to the ceiling.
    pub result: u32,
    /// What gear and 功法 added to this number outright, over and above what
    /// the four attributes bought. Zero on a cultivator wearing nothing: the
    /// breakdown then reads exactly as the attribute arithmetic alone.
    pub bonus: u32,
    /// The level the reference below was read at.
    ///
    /// The page prints it beside the comparison, because "同级基础" is only
    /// readable if it says which level: a 元婴 cultivator wearing nothing is not
    /// the yardstick a 炼气 one is measured against.
    pub level: u16,
    /// What this number comes to at that level with **only** the level's own
    /// attributes — no gear, no crops, no allocated points, no 功法.
    ///
    /// This is the comparison the page shows: a build is read as "＋X over what
    /// my level alone is worth / −X under it", which is the question a number
    /// on a sheet actually raises. Comparing against the mean of the four
    /// attributes answered a different one — how lopsided the build is — and
    /// said nothing about whether the number was any *good* for the level.
    pub baseline: u32,
    /// Whether the numbers are counted in parts per thousand, as 命中 and 闪避
    /// are. A surface prints those as percentages and the rest as they are.
    pub per_mille: bool,
}

/// The nine numbers the four attributes turn into, in the order the page draws
/// them.
///
/// Every line of [`Attributes::derived`] is one of these, and the list is the
/// only place a derived number's arithmetic is written down: the values, the
/// recipes and the breakdowns all come from it, so the page cannot show one
/// number and another set of numbers behind it.
const DERIVED_SPECS: [DerivedSpec; 9] = [
    DerivedSpec {
        key: "cultivation.attack",
        slot: DerivedSlot::Attack,
        attribute: Attribute::Strength,
        base: BASE_ATTACK,
        per_point: ATTACK_PER_STRENGTH,
        cap: None,
        per_mille: false,
    },
    DerivedSpec {
        key: "cultivation.health",
        slot: DerivedSlot::Health,
        attribute: Attribute::Strength,
        base: BASE_HEALTH,
        per_point: HEALTH_PER_STRENGTH,
        cap: None,
        per_mille: false,
    },
    DerivedSpec {
        key: "cultivation.inner",
        slot: DerivedSlot::Inner,
        attribute: Attribute::Root,
        base: BASE_INNER,
        per_point: INNER_PER_ROOT,
        cap: None,
        per_mille: false,
    },
    DerivedSpec {
        key: "cultivation.spell",
        slot: DerivedSlot::Spell,
        attribute: Attribute::Mana,
        base: 0,
        per_point: SPELL_PER_MANA,
        cap: None,
        per_mille: false,
    },
    DerivedSpec {
        key: "cultivation.resist",
        slot: DerivedSlot::Resist,
        attribute: Attribute::Mana,
        base: 0,
        per_point: RESIST_PER_MANA,
        cap: None,
        per_mille: false,
    },
    DerivedSpec {
        key: "cultivation.speed",
        slot: DerivedSlot::Speed,
        attribute: Attribute::Agility,
        base: BASE_SPEED,
        per_point: SPEED_PER_AGILITY,
        cap: None,
        per_mille: false,
    },
    DerivedSpec {
        key: "cultivation.hit",
        slot: DerivedSlot::Hit,
        attribute: Attribute::Agility,
        base: BASE_HIT,
        per_point: HIT_PER_AGILITY,
        cap: None,
        per_mille: false,
    },
    DerivedSpec {
        key: "cultivation.evade",
        slot: DerivedSlot::Evade,
        attribute: Attribute::Agility,
        base: BASE_EVADE,
        per_point: EVADE_PER_AGILITY,
        cap: None,
        per_mille: false,
    },
    DerivedSpec {
        key: "cultivation.aptitude",
        slot: DerivedSlot::Aptitude,
        attribute: Attribute::Root,
        // 潜质 no longer follows 根骨: it is its own fractional number, born
        // somewhere between −10 and +10 and raised only by the two herbs. What
        // this line carries is the *outright* share gear and 内功 pay — a fixed
        // number of whole points on top of the herb-fed band.
        base: 0,
        per_point: 0,
        cap: None,
        per_mille: false,
    },
];

/// Which of the nine numbers one line of the table fills.
///
/// Named rather than positional so that reordering the table cannot silently
/// swap two numbers: the value a slot receives is written where the slot is
/// named, and the compiler is the one that notices a mismatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DerivedSlot {
    Attack,
    Health,
    Inner,
    Spell,
    Resist,
    Speed,
    Hit,
    Evade,
    Aptitude,
}

impl DerivedSlot {
    /// The slot's place in the values the table fills.
    const fn index(self) -> usize {
        match self {
            Self::Attack => 0,
            Self::Health => 1,
            Self::Inner => 2,
            Self::Spell => 3,
            Self::Resist => 4,
            Self::Speed => 5,
            Self::Hit => 6,
            Self::Evade => 7,
            Self::Aptitude => 8,
        }
    }

    /// This line's value in a set of the nine numbers.
    const fn of(self, derived: Derived) -> u32 {
        match self {
            Self::Attack => derived.attack,
            Self::Health => derived.health,
            Self::Inner => derived.inner,
            Self::Spell => derived.spell,
            Self::Resist => derived.resist,
            Self::Speed => derived.speed,
            Self::Hit => derived.hit,
            Self::Evade => derived.evade,
            Self::Aptitude => derived.aptitude,
        }
    }
}

/// One line of the table above: which number it is and how it is computed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DerivedSpec {
    key: &'static str,
    slot: DerivedSlot,
    attribute: Attribute,
    base: i64,
    per_point: i64,
    cap: Option<i64>,
    per_mille: bool,
}

impl DerivedSpec {
    /// What the arithmetic yields for a set of attributes, before the ceiling.
    fn computed(self, attributes: Attributes) -> i64 {
        self.base + i64::from(self.attribute.of(attributes)) * self.per_point
    }

    /// The number the page shows: the arithmetic, held to the ceiling.
    fn value(self, attributes: Attributes) -> u32 {
        let computed = self.computed(attributes);
        let held = match self.cap {
            Some(cap) => computed.min(cap),
            None => computed,
        };
        scaled(held)
    }
}

impl Derived {
    /// The sum of two sets of the nine numbers.
    ///
    /// Saturation rather than wrapping: a number that wrapped would read as a
    /// penalty on a page that promised a bonus.
    pub fn plus(self, other: Self) -> Self {
        Self {
            attack: self.attack.saturating_add(other.attack),
            health: self.health.saturating_add(other.health),
            inner: self.inner.saturating_add(other.inner),
            spell: self.spell.saturating_add(other.spell),
            resist: self.resist.saturating_add(other.resist),
            speed: self.speed.saturating_add(other.speed),
            hit: self.hit.saturating_add(other.hit),
            evade: self.evade.saturating_add(other.evade),
            aptitude: self.aptitude.saturating_add(other.aptitude),
        }
    }

    /// What these nine numbers are worth as power, at the rates the attributes
    /// are worth them.
    ///
    /// A flat 攻击力 is read at what the 力量 behind it would have paid, and
    /// every other line at its own attribute's rate — the same constants the
    /// table above is built from, so a bonus cannot be worth more (or less)
    /// than the attribute it stands in for. The synergy the pairs pay is the
    /// attributes' own and is not counted twice here.
    pub fn power(self) -> u32 {
        let attack = i64::from(self.attack) * STRENGTH_WEIGHT * 1_000 / ATTACK_PER_STRENGTH;
        let health = i64::from(self.health) * STRENGTH_WEIGHT * 1_000 / HEALTH_PER_STRENGTH;
        let speed = i64::from(self.speed) * AGILITY_WEIGHT * 1_000 / SPEED_PER_AGILITY;
        let footwork = i64::from(self.hit.saturating_add(self.evade)) * AGILITY_WEIGHT * 1_000
            / HIT_PER_AGILITY;
        let spell = i64::from(self.spell) * MANA_WEIGHT * 1_000 / SPELL_PER_MANA;
        let resist = i64::from(self.resist) * MANA_WEIGHT * 1_000 / RESIST_PER_MANA;
        let inner = i64::from(self.inner) * ROOT_WEIGHT * 1_000 / INNER_PER_ROOT;
        let aptitude = i64::from(self.aptitude) * ROOT_WEIGHT * 1_000 / APTITUDE_PER_ROOT;
        let martial = attack + health + speed + footwork;
        let spell_half = spell + resist + inner + aptitude;
        scaled_power((martial + spell_half) / 1_000)
    }

    /// The nine numbers, filled from the one table that says how they are
    /// computed.
    ///
    /// Every value comes from a line of [`DERIVED_SPECS`], and every line of
    /// that table is read here — so the number a fight uses and the arithmetic
    /// the page prints are the same arithmetic, read in the same place. There is
    /// no second set of constants to drift.
    fn from_table(attributes: Attributes) -> Self {
        let mut values = [0u32; 9];
        for spec in DERIVED_SPECS.iter() {
            values[spec.slot.index()] = spec.value(attributes);
        }
        Self {
            attack: values[DerivedSlot::Attack.index()],
            health: values[DerivedSlot::Health.index()],
            inner: values[DerivedSlot::Inner.index()],
            spell: values[DerivedSlot::Spell.index()],
            resist: values[DerivedSlot::Resist.index()],
            speed: values[DerivedSlot::Speed.index()],
            hit: values[DerivedSlot::Hit.index()],
            evade: values[DerivedSlot::Evade.index()],
            aptitude: values[DerivedSlot::Aptitude.index()],
        }
    }

    /// The nine numbers with the arithmetic that produced each.
    ///
    /// The same order as [`Attributes::derived`] reads, so a surface that walks
    /// the numbers once gets the value and the recipe that made it together.
    ///
    /// `bonus` is what gear and 功法 added outright: the arithmetic the card
    /// prints is the attribute recipe *plus* that share, so the sum on the card
    /// is the sum that ran.
    ///
    /// `level` is the cultivator's own level, and it is what builds the
    /// reference each row is compared against: [`base_attributes`] of that
    /// level, read through the same table. A naked body at level `level` lands
    /// exactly on every one of those nine numbers, which is what makes the
    /// comparison mean "over and above what the level alone is worth".
    pub fn breakdowns(
        self,
        attributes: Attributes,
        bonus: Derived,
        level: u16,
    ) -> Vec<DerivedBreakdown> {
        // The reference: the attributes the level itself carries, nothing worn
        // and nothing trained.
        let bare = base_attributes(level);
        DERIVED_SPECS
            .iter()
            .map(|spec| {
                let computed = spec.computed(attributes);
                let held = match spec.cap {
                    Some(cap) => computed.min(cap),
                    None => computed,
                };
                // The flat share gear and 功法 pay outright, read off the set
                // the caller passed in: the number the page shows is the
                // attribute arithmetic *and* that share, and the card says so
                // rather than printing a sum that does not add up.
                let bonus = spec.slot.of(bonus);
                DerivedBreakdown {
                    key: spec.key,
                    attribute: spec.attribute,
                    attribute_value: spec.attribute.of(attributes),
                    base: scaled(spec.base),
                    per_point: u32::try_from(spec.per_point.max(0)).unwrap_or(u32::MAX),
                    cap: spec.cap.map(scaled),
                    computed: scaled(computed),
                    result: scaled(held).saturating_add(bonus),
                    bonus,
                    level: level.clamp(1, MAX_LEVEL),
                    // Read through the same `value` the fight reads, so the
                    // comparison cannot be computed by a second set of rules.
                    baseline: spec.value(bare),
                    per_mille: spec.per_mille,
                }
            })
            .collect()
    }
}
/// The floor every 攻击力 starts from, and what one point of 力量 adds.
const BASE_ATTACK: i64 = 10;
const ATTACK_PER_STRENGTH: i64 = 3;
/// The floor every 生命值 starts from, and what one point of 力量 adds.
const BASE_HEALTH: i64 = 100;
const HEALTH_PER_STRENGTH: i64 = 20;
/// The floor every 内力值 starts from, and what one point of 根骨 adds.
const BASE_INNER: i64 = 50;
const INNER_PER_ROOT: i64 = 8;
/// What one point of 法力 is worth to 法术强度 and to 法抗.
const SPELL_PER_MANA: i64 = 3;
const RESIST_PER_MANA: i64 = 2;
/// The floor every 速度 starts from, and what one point of 身法 adds.
const BASE_SPEED: i64 = 10;
const SPEED_PER_AGILITY: i64 = 2;
/// 命中 and 闪避 start here and climb with 身法, as plain numbers a fight
/// compares rather than percentages.
///
/// They share one curve on purpose: the hit chance is read off the *ratio* of
/// the attacker's 命中 to the defender's 闪避, and a fighter only ever stands
/// across from someone whose stats came off the same ladder — so two equal
/// numbers is what "evenly matched" means, and the curve turns that into the
/// chance the fight promises. What separates builds is how far each side's
/// numbers have climbed relative to the other's, not a built-in gap between the
/// two stats.
const BASE_HIT: i64 = 100;
const HIT_PER_AGILITY: i64 = 10;
const BASE_EVADE: i64 = 100;
const EVADE_PER_AGILITY: i64 = 10;
/// What one point of 根骨 *would* be worth to 潜质, used only to price the whole
/// points gear and 功法 pay outright: the herb-fed band has no attribute behind
/// it, so the conversion exists only for what is added on top.
const APTITUDE_PER_ROOT: i64 = 2;

/// Turn a weighted sum into a whole non-negative number.
fn scaled(value: i64) -> u32 {
    u32::try_from(value.max(0)).unwrap_or(u32::MAX)
}

impl Attributes {
    /// The fighter these four attributes make.
    ///
    /// Every line is one attribute read through one constant, so the panel can
    /// say exactly where a number came from and a change to a constant moves the
    /// panel and the fight together.
    pub fn derived(self) -> Derived {
        Derived::from_table(self)
    }
}

/// The four ways a cultivator is said to be trained, and the one number the
/// growth formula and the page both read for each.
///
/// Four is not decoration: each dimension is fed by a different action the user
/// already takes — finishing a pomodoro, walking out, winning a fight, pulling a
/// crop — so the growth a session pays says which of those they have actually
/// been doing. A user who only ever finishes pomodoros watches the 修炼 bar rise
/// and the other three sit still, and the page can say so.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Discipline {
    /// 修炼 — the 功法 being trained, which only finished sessions feed.
    Practice,
    /// 游历 — the walks taken.
    Wander,
    /// 战斗 — the fights won.
    Fight,
    /// 造化 — the crops pulled.
    Fortune,
}

impl Discipline {
    /// Every dimension, in the order the page draws them.
    pub const ALL: [Self; 4] = [Self::Practice, Self::Wander, Self::Fight, Self::Fortune];
}

/// What one step of a dimension is worth, in parts per thousand of the growth it
/// multiplies.
///
/// Fifty parts — five percent — is the step that makes a dimension read as a
/// discipline rather than as a rounding difference: a 阶 of 功法 or a single walk
/// has to be worth seeing on the page, and twenty of them is most of a dimension
/// before the realm's own cap takes over.
const DISCIPLINE_STEP_PERMILLE: u32 = 50;

/// How many steps each action dimension is worth before it stops: two hundred
/// walks, wins or harvests is far past any save, and the cap is what stops the
/// formula from being worth farming one path forever.
const DISCIPLINE_STEP_CAP: u32 = 20;

/// How much the three action dimensions can add between them, in parts per
/// thousand.
///
/// Each is capped at [`DISCIPLINE_STEP_CAP`] steps, so three full ones reach a
/// thousand; the shared cap says a save cannot hold all three *and* a trained
/// 功法 at full value, which keeps 修炼 worth choosing at every realm.
const ACTION_BONUS_CAP_PERMILLE: u32 = 1_000;

/// The most the four dimensions add to one session between them.
///
/// Four times the base is the ceiling on the whole formula — see
/// [`GROWTH_MULTIPLIER_CAP_PERMILLE`] — so this is what says "as trained as this
/// save can get".
const TOTAL_BONUS_CAP_PERMILLE: u32 = GROWTH_MULTIPLIER_CAP_PERMILLE - 1_000;

/// What the dimensions are worth on top of a flat session, in parts per
/// thousand.
///
/// Authored per realm rather than derived: 修炼 is the dimension that has to
/// carry the late game (a 功法 is the only thing that keeps up with a 真仙's
/// growth), while the three action dimensions are worth most exactly where a new
/// save can still fill them — the first realms. Read as a multiplier: 炼气 caps
/// at ×1.2, the deepest realm at ×4.
const PRACTICE_CAP_PERMILLE: [u32; STAGE_COUNT] = [
    200, 500, 800, 1_200, 1_600, 2_000, 2_400, 2_800, 3_000, 3_000,
];

/// The most one finished session can be worth, in parts per thousand of the flat
/// [`session_growth`].
///
/// A session that has trained everything is four times a bare one. The cap is
/// what keeps the formula honest: the ladder is meant to take years, and a
/// multiplier that could reach ten would be a different game.
pub const GROWTH_MULTIPLIER_CAP_PERMILLE: u32 = 4_000;

/// What one dimension is worth right now, in parts per thousand **on top of** a
/// flat session — so zero means "this dimension has not been trained yet".
///
/// `rank` is the 阶 of the 功法 being trained (only [`Discipline::Practice`] reads
/// it) and `actions` the lifetime count the dimension is fed by. The three action
/// dimensions share [`ACTION_BONUS_CAP_PERMILLE`]; 修炼 is capped by the realm
/// instead, because a 阶 of 功法 is worth more the deeper the realm is.
pub fn discipline_bonus(discipline: Discipline, rank: u8, actions: u32, realm: Realm) -> u32 {
    match discipline {
        Discipline::Practice => u32::from(rank)
            .saturating_mul(DISCIPLINE_STEP_PERMILLE)
            .min(PRACTICE_CAP_PERMILLE[realm.index()]),
        _ => actions
            .min(DISCIPLINE_STEP_CAP)
            .saturating_mul(DISCIPLINE_STEP_PERMILLE)
            .min(ACTION_BONUS_CAP_PERMILLE),
    }
}

/// The most one dimension can be worth in the given realm — what its bar is drawn
/// against, and what the page says it is capped at.
pub fn discipline_cap(discipline: Discipline, realm: Realm) -> u32 {
    match discipline {
        Discipline::Practice => PRACTICE_CAP_PERMILLE[realm.index()],
        _ => ACTION_BONUS_CAP_PERMILLE,
    }
}

/// The multiplier one finished session pays, in parts per thousand.
///
/// The four bonuses add rather than compound: four trained dimensions are worth
/// four times a bare session, not sixteen, which is what
/// [`GROWTH_MULTIPLIER_CAP_PERMILLE`] promises. A penalty for a missed day is
/// scaled by the same number, so a save that has trained deep also loses more for
/// letting a day go — the four dimensions are the same lever whichever way it
/// moves.
pub fn growth_permille(realm: Realm, rank: u8, walks: u32, wins: u32, harvests: u32) -> u32 {
    let counts = [
        (Discipline::Practice, rank, 0),
        (Discipline::Wander, 0, walks),
        (Discipline::Fight, 0, wins),
        (Discipline::Fortune, 0, harvests),
    ];
    let bonus = counts
        .iter()
        .map(|(discipline, rank, actions)| discipline_bonus(*discipline, *rank, *actions, realm))
        .sum::<u32>()
        .min(TOTAL_BONUS_CAP_PERMILLE);
    1_000 + bonus
}

/// Scale a growth figure by a [`growth_permille`] multiplier.
///
/// Integer arithmetic in u64 so a deep realm's flat growth times four cannot
/// overflow before it is divided back down.
pub fn scaled_growth(growth: u32, multiplier_permille: u32) -> u32 {
    let multiplier_permille = multiplier_permille.min(GROWTH_MULTIPLIER_CAP_PERMILLE);
    (u64::from(growth) * u64::from(multiplier_permille) / 1_000) as u32
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
        mana: 8 + level * 2 + stage * 60,
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
