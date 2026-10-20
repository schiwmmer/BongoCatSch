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

mod arts;
mod battle;
mod curve;
mod document;
mod duel;
mod explore;
mod garden;
mod items;
mod shop;
mod state;
mod trade;

#[cfg(test)]
mod tests;

pub use arts::{
    ART_COUNT, ArtBlueprint, ArtGrade, ArtKind, ArtOutcome, ArtView, OwnedArt, art_attributes,
    art_view, blueprint, effective_failure_permille, external_damage, mastery_permille,
    required_pomodoros,
};
pub use battle::{BattleReport, Guardian, guardian_fighter, wanderer_fighter};
// `duel` itself stays crate-private: it takes the seeded `Rng`, which is nobody
// else's business. A surface asks the state machine to fight, not the engine.
pub use curve::{
    LAYERS_PER_REALM, LAYERS_PER_STAGE, LayerRank, LevelProgress, MAX_LEVEL, RANKS_PER_LAYER,
    layer_rank, level_for_growth, threshold_for,
};
pub use document::CultivationDocument;
pub use duel::{
    ArtStrike, DUEL_RECORD_CAP, DUEL_ROUND_CAP, DuelRecord, DuelReport, DuelRound, FightKind,
    Fighter, GUARDIAN_NAME, PLAYER_NAME, WANDERER_NAME,
};
pub use explore::{
    BREAKTHROUGH_COOLDOWN_SECONDS, ENCOUNTER_PERMILLE, EXPLORATION_COOLDOWN_SECONDS,
    ExplorationOutcome, VICTORY_SEED_PERMILLE, WALK_RECORD_CAP, WalkRecord,
};
pub use garden::{MAX_SEED_TIER, PLOT_COUNT, PlotRecord, SeedStack};
pub use items::{EquipSlot, EquipmentRecord, Rarity, SLOT_COUNT};
pub use shop::SESSION_EARNINGS;
pub use shop::{
    SHOP_MAX_OFFERS, SHOP_MIN_OFFERS, ShopOffer, ShopOfferView, art_price, equipment_price,
    offers_for_day, roll_offers, seed_price,
};
pub use state::{
    AbsenceCharge, BreakthroughOutcome, CropView, Cultivation, CultivationView, Drop, DropView,
    ExplorationFindView, GuardianView, HarvestOutcome, ItemView, PlotView, SeedView, SessionReward,
    TrainingView,
};
pub use trade::{TradeRefusal, session_earnings};

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
    /// 命中: chance to land a blow, in parts per thousand.
    pub hit: u32,
    /// 闪避: chance to avoid one, in parts per thousand.
    pub evade: u32,
    /// 潜质: how fast 功法 are learned and how rarely the attempt fails.
    pub aptitude: u32,
}

impl Derived {
    /// 命中 as a percentage the surfaces can print.
    pub fn hit_percent(self) -> u32 {
        self.hit / 10
    }

    /// 闪避 as a percentage the surfaces can print.
    pub fn evade_percent(self) -> u32 {
        self.evade / 10
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
/// 命中 and 闪避 start here and climb with 身法, in parts per thousand. Both are
/// capped: an uncapped 闪避 makes a build untouchable and an uncapped 命中 makes
/// the cap the only number that matters.
const BASE_HIT_PERMILLE: i64 = 750;
const HIT_PERMILLE_PER_AGILITY: i64 = 5;
const HIT_PERMILLE_CAP: i64 = 950;
const BASE_EVADE_PERMILLE: i64 = 50;
// Two per point rather than four, and a lower ceiling: at four, a cultivator and
// the guardian both carry enough 闪避 to push every fight to a forty percent hit
// rate, which turns a real fight into a coin toss — the opposite of what the log
// is for. This curve keeps 闪避 worth having without making 命中 meaningless.
const EVADE_PERMILLE_PER_AGILITY: i64 = 2;
const EVADE_PERMILLE_CAP: i64 = 250;
/// What one point of 根骨 is worth to 潜质.
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
        let strength = i64::from(self.strength);
        let mana = i64::from(self.mana);
        let agility = i64::from(self.agility);
        let root = i64::from(self.root);
        Derived {
            attack: scaled(BASE_ATTACK + strength * ATTACK_PER_STRENGTH),
            health: scaled(BASE_HEALTH + strength * HEALTH_PER_STRENGTH),
            inner: scaled(BASE_INNER + root * INNER_PER_ROOT),
            spell: scaled(mana * SPELL_PER_MANA),
            resist: scaled(mana * RESIST_PER_MANA),
            speed: scaled(BASE_SPEED + agility * SPEED_PER_AGILITY),
            hit: scaled(
                (BASE_HIT_PERMILLE + agility * HIT_PERMILLE_PER_AGILITY).min(HIT_PERMILLE_CAP),
            ),
            evade: scaled(
                (BASE_EVADE_PERMILLE + agility * EVADE_PERMILLE_PER_AGILITY)
                    .min(EVADE_PERMILLE_CAP),
            ),
            aptitude: scaled(root * APTITUDE_PER_ROOT),
        }
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

    /// The catalog key naming this dimension.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Practice => "cultivation.discipline.practice",
            Self::Wander => "cultivation.discipline.wander",
            Self::Fight => "cultivation.discipline.fight",
            Self::Fortune => "cultivation.discipline.fortune",
        }
    }
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

/// One dimension as a surface reads it.
///
/// `progress_permille` is how far the dimension has come toward its own cap,
/// which is the bar the page draws: it is already clamped, so a surface never has
/// to divide by a cap that could be zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DisciplineView {
    pub discipline: Discipline,
    /// What this dimension adds on top of a flat session, in parts per thousand.
    pub bonus_permille: u32,
    /// The most it could add in this realm.
    pub cap_permille: u32,
    pub progress_permille: u32,
}

impl DisciplineView {
    /// The four dimensions as the cultivator stands now.
    pub fn of(realm: Realm, rank: u8, walks: u32, wins: u32, harvests: u32) -> [Self; 4] {
        let feeds = [
            (Discipline::Practice, rank, 0),
            (Discipline::Wander, 0, walks),
            (Discipline::Fight, 0, wins),
            (Discipline::Fortune, 0, harvests),
        ];
        feeds.map(|(discipline, rank, actions)| {
            let bonus_permille = discipline_bonus(discipline, rank, actions, realm);
            let cap_permille = discipline_cap(discipline, realm);
            Self {
                discipline,
                bonus_permille,
                cap_permille,
                // A dimension with no cap left reads as no progress rather than
                // as a division by zero: the bar is the whole point of the view.
                progress_permille: bonus_permille
                    .saturating_mul(1_000)
                    .checked_div(cap_permille)
                    .unwrap_or(0),
            }
        })
    }
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
