//! The cultivation state and the transitions over it.
//!
//! Like [`crate::PomodoroState`], this is a plain value: every transition takes
//! the wall time and the local day as arguments, so the tests can walk a whole
//! ladder — including six missed days and a breakthrough — without a clock.
//!
//! Two rules carry most of the design. The first is that the level is derived:
//! it is a function of the growth total, capped at the current realm's last
//! layer, so there is no second number that can drift out of step with the
//! first. The second is that growth never drops below the floor of the realm
//! already entered, which is what makes an absence penalty cost progress
//! without ever taking back a breakthrough the user fought for.

use std::array;

use super::{
    ART_COUNT, ArtGrade, ArtKind, ArtOutcome, ArtSetView, ArtStrike, ArtView, Attribute,
    Attributes, BONUS_GEAR_PERMILLE, BONUS_INSIGHT_PERMILLE, BONUS_SEED_PERMILLE,
    BONUS_STONES_PERMILLE, BONUS_WISDOM_PERMILLE, BREAKTHROUGH_COOLDOWN_SECONDS, BattleReport,
    Building, CollectionYield, Cost, CultivationDocument, DailyKind, Derived, DerivedBreakdown,
    DuelReport, EQUIPMENT_DROP_PERMILLE, EquipSlot, EquipmentRecord, ExpeditionInProgress,
    ExplorationOutcome, FREE_POINTS_PER_LEVEL, Fighter, GatherSite, GearEffect, GearSetView,
    Guardian, MAX_LEVEL, MAX_STAR, MAX_TRADE_LEVEL, MasteredArtView, OwnedArt, PLAYER_NAME,
    RECENT_ARTS_CAP, Rarity, Realm, Refusal, Resource, ResourceBag, Rng, SEED_DROP_PERMILLE,
    SESSION_BONUS_PERMILLE, SLOT_COUNT, SeedStack, SessionMode, VICTORY_SEED_PERMILLE,
    absence_penalty, art_attributes, art_view,
    arts::{
        art_bonus, art_set_bonus, art_set_views, blueprint, mastered_art_attributes,
        mastered_art_bonus, mastered_art_view, train,
    },
    base_attributes,
    battle::{guardian_for, resolve},
    building_upgrade_cost, collect_scaled,
    curve::{layer_growth, layer_rank, level_for_growth, stage_floor, threshold_for},
    daily::{
        DAILY_AMBUSH_STONES, DAILY_GIFT_STONES, DAILY_TRINKET_STONES, GiftKind, roll_gift,
        roll_kind,
    },
    duel::{DUEL_RECORD_CAP, DuelRecord, FightKind, duel},
    events::{BonusFind, EVENT_LOG_CAP, EventDetail, EventRecord, MarrowSource, SaleKind},
    expedition_yield,
    explore::{
        BONUS_STONES, GUARDIAN_STONES, TRIAL_STONES, WalkRecord, breakthrough_ready, roll_stones,
        roll_victory,
    },
    garden::{
        CropRecord, MAX_GARDEN_RANK, PlotRecord, SeedGrade, harvest_points, plant_permille,
        plot_price, rank_cost, rank_xp_total, roll_seed_grade, seed_name, seed_return,
        wither_points,
    },
    gather_duration_permille, growth_permille,
    items::{gear_set_bonus, gear_set_views, roll_equipment},
    mastery_permille,
    opponent::{self, Opponent},
    proving::{ProvingOutcome, TRIAL_CHANCES_CAP},
    resource_cap, rounds_per_hour_permille, scaled_growth,
    shop::{self, ShopOffer, ShopOfferView, star_up_cost},
    skill_ready, skill_xp_total, star_success_permille,
    trade::{TradeRefusal, session_earnings},
    trial::{
        TRIAL_ATTEMPTS_PER_DAY, TRIAL_MAX_FLOOR, TRIAL_SWEEP_MARGIN_PERMILLE, TrialReward,
        floor_realm, roll_reward, warden_power,
    },
};
use crate::CalendarDay;
use serde::Serialize;

/// What the absence evaluation took, if anything.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AbsenceCharge {
    /// How many whole days without a session were charged.
    pub days: u32,
    /// The growth actually lost. A penalty that reaches the realm's floor
    /// charges less than it asked for, and the difference is reported here
    /// rather than pretended.
    pub charged: u32,
}

/// What one finished session produced beyond its growth.
///
/// The first two are the ordinary drop — a piece of gear or a herb, at the
/// session's own odds — and the rest are what the thousandth's luck pays: the
/// ladder's one jackpot, in whatever shape it came.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum Drop {
    Equipment(EquipmentRecord),
    Seed {
        attribute: Attribute,
        tier: u8,
        grade: SeedGrade,
    },
    /// A 功法, by the name it was given.
    Art {
        name: String,
    },
    /// One 潜质 herb.
    ///
    /// The grade it was found at is what the body pays, so the notice and the
    /// log name the grade beside the herb.
    Herb {
        herb: super::AptitudeHerb,
        grade: SeedGrade,
    },
    /// A purse of 灵石.
    Stones(u32),
}

/// What one source's *absolute* strength hands the two ladders.
///
/// One value, read twice: [`Self::tier`] is the 阶级 the goods stand at and
/// [`Self::factor`] is what the quality roll is multiplied by. Both are pure
/// functions of the power, so a page that prints "对手战力 N" and the roll that
/// paid out cannot disagree about what N bought.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Strength {
    power: u32,
}

impl Strength {
    /// The strength a body of this power is worth.
    pub(crate) const fn of(power: u32) -> Self {
        Self { power }
    }

    /// The rung its goods stand on: a realm, one-based.
    pub(crate) fn tier(self) -> u8 {
        super::drop::tier_for_power(self.power)
    }

    /// What it multiplies the quality ladder by, in parts per thousand.
    pub(crate) fn factor(self) -> u32 {
        super::drop::power_factor_permille(self.power)
    }
}

/// Everything one finished session changed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SessionReward {
    /// The growth the session itself paid.
    pub growth: u32,
    /// The penalty charged for the days before it, when there were any.
    pub absence: Option<AbsenceCharge>,
    pub drop: Option<Drop>,
}

/// What a breakthrough attempt came to.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BreakthroughOutcome {
    pub report: BattleReport,
    /// The realm a win opens, or the one that was defended on a loss.
    pub target: Realm,
    /// The gear the win paid. A loss pays nothing and takes nothing.
    pub reward: Option<EquipmentRecord>,
    /// Every blow of the fight that settled it, for the log a surface shows.
    pub duel: DuelReport,
    /// The gear a loss cost, if it cost any.
    pub lost: Option<String>,
    /// The 功法 a win paid. A win always pays one; a loss never does.
    pub granted_art: Option<String>,
    /// The purse a win turned up, beside the 功法 and the gear.
    pub stones: Option<u32>,
    /// The herb a win turned up, by name: the one find a breakthrough makes
    /// that used to reach the bag without the log ever saying so.
    pub seed: Option<String>,
}

/// What one 试炼 floor came to.
///
/// The duel travels whole, the way a breakthrough's does, because the tower is
/// watched blow by blow like every other fight; what the win turned up travels
/// as the same find a walk reports, since the three kinds are the same three.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TrialOutcome {
    /// The floor that was climbed, one-based.
    pub floor: u16,
    /// What a warden of this floor is worth, as the page reads it: the band's
    /// middle around the cultivator, floored at the floor's own rung.
    pub warden_power: u32,
    /// Every blow of the fight that settled it.
    pub duel: DuelReport,
    /// What clearing the floor turned up. A loss finds nothing but the floor.
    pub found: ExplorationFindView,
}

/// What one swept run of the tower turned up.
///
/// A sweep clears several floors in one action and settles them all at once:
/// the fights are skipped because the cultivator is overwhelmingly stronger, so
/// the outcome is the haul rather than a blow-by-blow. The counts are the haul,
/// because ten floors can turn up several of a kind and a single named find
/// would have to drop the others on the floor.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct TrialSweepOutcome {
    /// The first floor the sweep covered, one-based.
    pub from_floor: u16,
    /// How many floors were swept in one go.
    pub floors: u16,
    pub seeds: u32,
    pub gear: u32,
    pub arts: u32,
    pub stones: u32,
}

/// Why a 试炼 floor cannot be climbed right now.
///
/// The page needs a reason to print beside a dead button, and it is the same
/// reason the domain refuses with — so the two cannot disagree about whether
/// the tower is closed or the day is spent.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum TrialBlocked {
    /// The cultivator is still out on a walk: the tower is climbed at home.
    Out,
    /// The cultivator is at a job: the tower is climbed at home, and a job is
    /// hours of the same day a walk would have been.
    Working,
    /// The day's floors are used up.
    Drained,
    /// Every floor of the tower is behind the save already.
    Top,
}

impl TrialBlocked {
    /// The domain refusal this state would produce.
    pub const fn refusal(self) -> Refusal {
        match self {
            Self::Out => Refusal::TrialOut,
            Self::Working => Refusal::Working,
            Self::Drained => Refusal::TrialDrained,
            Self::Top => Refusal::TrialTop,
        }
    }

    /// The catalog key saying why the tower is closed.
    pub const fn key(self) -> &'static str {
        self.refusal().key()
    }
}

/// Why the 试炼 cannot be fought right now, or the three reasons it can.
///
/// The two fights fought at home — the tower and the 试炼 — share the first two
/// walls: a walk on the road and a job in hand are both out of the courtyard.
/// What each adds is its own ration, the day's floors here and a banked chance
/// there, which is why the two keep separate enums and one shared pair of
/// refusals.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum ProvingBlocked {
    /// The cultivator is still out on a walk: the courtyard is fought in.
    Out,
    /// The cultivator is at a job: the courtyard is fought at home, and a job
    /// is hours of the same day a walk would have been.
    Working,
    /// No chance is banked: a finished pomodoro pays one.
    Spent,
}

impl ProvingBlocked {
    /// The domain refusal this state would produce.
    pub const fn refusal(self) -> Refusal {
        match self {
            Self::Out => Refusal::ProvingOut,
            Self::Working => Refusal::Working,
            Self::Spent => Refusal::ProvingSpent,
        }
    }

    /// The catalog key saying why the courtyard is closed.
    pub const fn key(self) -> &'static str {
        self.refusal().key()
    }
}

/// What the day opened with, as a surface reads it.
///
/// The three kinds travel as one value because they are one event: whether it
/// was a fight, what the fight came to, and what any of the three left behind.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DailyOutcome {
    pub kind: DailyKind,
    /// Whether the ambush was beaten. True for the kinds that are not a fight.
    pub won: bool,
    /// Only an ambush settles a fight, and only a fight is replayed.
    pub duel: Option<DuelReport>,
    /// What the day left behind.
    pub found: ExplorationFindView,
}

/// What a harvest took from a plot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HarvestOutcome {
    /// The crop that was picked.
    pub crop: CropRecord,
    /// The seed the harvest gave back, if it gave one: the same attribute, the
    /// same tier or one higher, at a grade rolled fresh.
    pub seed: Option<(Attribute, u8, SeedGrade)>,
}

/// What one attempt at a piece's next star came to.
///
/// The stones are spent whether the star turns over or not — a wager is a wager
/// — so the outcome carries the cost beside the result: a failure that cost
/// nothing would read as a free reroll, and a run of them would be the whole
/// ladder's economy in one button.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StarOutcome {
    /// The item the attempt was made on.
    pub item_id: u64,
    /// The item's name, for the line that says what happened.
    pub name: Option<String>,
    /// The star the piece stood at before the attempt.
    pub from: u8,
    /// The star it stands at now: one up on a success, one down on a failure,
    /// and never below the first star.
    pub to: u8,
    /// Whether the attempt came off.
    pub success: bool,
    /// What the attempt cost: the 灵石 wager and the resources the forge burned.
    pub cost: super::Cost,
}

/// What one gathering at a site turned up.
///
/// Carried rather than left to the event log because the popup that says "伐木
/// +12 木材，+100 伐木经验" has to know the move, and a gathering is too routine
/// to be a line in the history.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CollectOutcome {
    /// Where the gathering was done.
    pub site: GatherSite,
    /// What the purse actually gained, after the 仓库's ceiling.
    pub gained: ResourceBag,
    /// The 伐木 xp banked by this gathering.
    pub woodcut_xp: u32,
    /// The 采石 xp banked by this gathering.
    pub stonecut_xp: u32,
    /// The 伐木 level after the gathering, if it trained 伐木.
    pub woodcut_level: u8,
    /// The 采石 level after the gathering, if it trained 采石.
    pub stonecut_level: u8,
}

/// What one building rank came to.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BuildingOutcome {
    /// The building raised.
    pub building: Building,
    /// The rank it stood at before.
    pub from: u8,
    /// The rank it stands at now.
    pub to: u8,
    /// What the rank cost.
    pub cost: Cost,
}

/// One gathering trade, as the page reads it: the level, the xp banked toward
/// the next, and what that next costs.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct TradeView {
    /// The trade's level.
    pub level: u8,
    /// The xp banked toward the next level.
    pub xp: u32,
    /// What the next level costs, in xp.
    pub xp_to_next: u32,
}

/// One building, as the page reads it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BuildingView {
    /// The rank the building stands at.
    pub level: u8,
    /// Whether it is as tall as it can be.
    pub capped: bool,
    /// What the next rank costs, and nothing at the top of the ladder.
    pub upgrade_cost: Option<Cost>,
}

/// One gathering expedition under way, as the page reads it.
///
/// Carried beside the sites so the page can say "you are out gathering" and
/// offer the same early return the walk and the job offer — and so a surface
/// that knows the expedition is out can close the sites rather than let a
/// click be refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ExpeditionView {
    /// The site the expedition works.
    pub site: GatherSite,
    /// The length it was out for, in hours.
    pub hours: u8,
    /// When it comes home.
    pub ends_unix: i64,
    /// Whether it has been called home already.
    pub early: bool,
}

/// The resource economy as the settings page reads it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResourcesView {
    /// The purse.
    pub bag: ResourceBag,
    /// What the purse holds at most, of one resource.
    pub cap: u32,
    /// 伐木, the forest trade.
    pub woodcut: TradeView,
    /// 采石, the mountain trade.
    pub stonecut: TradeView,
    /// 工坊: the yield's multiplier.
    pub workshop: BuildingView,
    /// 仓库: the purse's width.
    pub warehouse: BuildingView,
    /// 铁匠铺: the wager's discount.
    pub smithy: BuildingView,
    /// 道场: the xp's multiplier.
    pub dojo: BuildingView,
    /// The gathering expedition under way, if the cultivator is out gathering.
    pub expedition: Option<ExpeditionView>,
}

/// What one 潜质 herb did.
///
/// Carried rather than left to the event log because the popup that says "潜质
/// 4.7 → 5.3" has to know the move, and a surface reading the log would have to
/// match a line by timestamp to find it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AptitudeUse {
    /// Which herb was fed.
    pub herb: super::AptitudeHerb,
    /// How much it actually moved the total, in tenths: less than the roll when
    /// the band's top clipped it.
    pub gained_tenths: i32,
    /// Where 潜质 stands now, in tenths.
    pub aptitude_tenths: i32,
    /// How many 洗髓丹 the move cost.
    pub marrow_spent: u32,
}

/// What one finished job paid.
///
/// One region of the road, as the page lists it.
///
/// The page draws every region the cultivator's realm has reached — the one they
/// stand in and every one below it — with how far it has been walked and what it
/// asks of a body at the point it stands on. A region is picked from this list,
/// so the walk's target is a choice rather than a consequence of the realm.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RegionView {
    /// Which region this row is.
    pub region: super::explore::Region,
    /// Levels cleared in it, zero to [`super::explore::MAX_LEVEL`].
    pub cleared_levels: u8,
    /// Progress through the level being walked, in tenths of a percent.
    pub level_progress: u32,
    /// The whole region's completion, in tenths of a percent: a cleared region
    /// reads `1000` and the page prints `100.0%`.
    pub percent: u32,
    /// The level the region's progress stands at.
    pub level: u16,
    /// What the next walk there asks of a body, in power.
    pub requirement: u32,
    /// Whether the region has been walked end to end once.
    pub cleared: bool,
    /// Whether the realm has reached it.
    pub unlocked: bool,
    /// Whether this is the region the next walk will be taken in.
    pub current: bool,
}

impl RegionView {
    /// The region's completion as a fraction, for a progress bar.
    ///
    /// The completion is carried in tenths of a percent, so a region walked
    /// half of one level of ten reads `0.5%` and a bar that is one percent
    /// full — the number the page prints and the width it draws are the same
    /// reading of the same slot.
    pub fn fraction(&self) -> f32 {
        (self.percent as f32 / 1_000.0).clamp(0.0, 1.0)
    }
}

/// Kept beside the walk's record for the same reason: the popup is a moment and
/// the quit forgets it, and a page that could not say what the last job paid is a
/// page whose money has no history.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct WorkPaid {
    /// How many 灵石 the job paid. The wage is the wage: nothing about the work
    /// coming off changes what the wage was.
    pub stones: u32,
    /// How long the job took, in hours.
    pub hours: u8,
    /// The job's grade, which sized both its wage and its requirement.
    pub grade: ArtGrade,
    /// What the job trained, which is what it asked of the cultivator.
    pub kind: super::JobKind,
    /// The chance the job came off at, in parts per thousand.
    pub success_rate: u64,
    /// The attribute the job paid, and how many points, if the gain came up.
    pub gain: Option<(super::DerivedSlot, u32)>,
}

/// One job as a surface reads it: what it is, what it asks, and what it pays.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct JobView {
    /// Its position on the board, which is what taking it names.
    pub slot: usize,
    pub tier: u8,
    pub hours: u8,
    pub grade: ArtGrade,
    /// What it pays, as the one figure the board is read against.
    pub stones: u32,
    /// Whether this is the job the cultivator is already working: the board
    /// keeps the row so the page can say 接取中, but taking it again would be
    /// refused, so the row is shown as taken rather than as open.
    pub working: bool,
    /// What the job trains, for the name and the reward.
    pub kind: super::JobKind,
    /// What the job asks of the first (or only) attribute it trains.
    pub requirement: u32,
    /// What the job asks of the second attribute, for a mixed job.
    pub requirement_second: Option<u32>,
    /// What the cultivator actually holds in the first (or only) attribute, for
    /// the row to say how far over or under the requirement they are.
    pub current_first: u32,
    /// What the cultivator holds in the second attribute, for a mixed job.
    pub current_second: Option<u32>,
    /// The chance the job comes off, in parts per thousand, given how the
    /// cultivator holds the attribute it asks for.
    pub success_rate: u64,
}

/// One owned item, as a surface reads it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ItemView {
    pub id: u64,
    pub slot: EquipSlot,
    pub tier: u8,
    pub rarity: Rarity,
    pub attributes: Attributes,
    /// The combat numbers this piece adds outright — 攻击力, 生命值, 命中 and the
    /// rest — with its stars folded in.
    pub bonus: Derived,
    pub equipped: bool,
    /// The stars the piece holds. Zero is a piece nobody has starred.
    pub star: u8,
    /// The effect a 神装 carries, for the name, the icon and the detail.
    pub effect: Option<GearEffect>,
    /// What one more star costs: the 灵石 wager and the resources the forge
    /// burns. `None` once the piece holds every star it can.
    pub star_cost: Option<super::Cost>,
    /// The xianxia-given name the drop carried, if any.
    pub name: Option<String>,
    /// The set this piece belongs to, when it is a set piece. A set's pieces all
    /// carry the same name prefix, and two or more worn pay the set's bonus.
    pub set: Option<super::items::GearSet>,
    /// What the 集市 pays for it, in 灵石.
    ///
    /// Carried on the view rather than derived at the surface because the price
    /// is the shop's margin over its own asking price: a surface that worked it
    /// out from the tier alone would show a number the sale does not pay.
    pub sell_price: u32,
}

/// One seed stack, as a surface reads it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SeedView {
    pub attribute: Attribute,
    /// The 潜质 herb this stack grows, when it is a herb seed rather than an
    /// attribute seed. A surface shows a herb seed by its herb, never by its
    /// `attribute`, which is a don't-care it is not allowed to read.
    pub herb: Option<super::AptitudeHerb>,
    pub tier: u8,
    /// The herb's grade: what the harvest will be multiplied by, and the colour
    /// the row is painted.
    pub grade: SeedGrade,
    pub count: u32,
    /// The herb's xianxia-given name, if the stack carries one.
    pub name: Option<String>,
    /// What the 集市 pays for one seed of this stack, in 灵石.
    ///
    /// Per seed rather than per stack: the row sells one at a time, and a number
    /// on the button that was the price of all ten would not be the price of the
    /// click.
    pub sell_price: u32,
}

/// What the last finished session did to the 功法 being trained.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TrainingView {
    pub art_id: u64,
    pub name: String,
    pub outcome: ArtOutcome,
    /// The 阶 the art stands at now.
    pub rank: u8,
}

/// One crop in a plot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CropView {
    pub attribute: Attribute,
    /// The 潜质 herb this crop grows, when it was a herb seed; a surface shows a
    /// herb crop by its herb, never by its `attribute`, which is a don't-care.
    pub herb: Option<super::AptitudeHerb>,
    pub tier: u8,
    /// The grade it was planted at, which is what the harvest will pay at.
    pub grade: SeedGrade,
    /// The herb's name, carried from the seed it grew from. A crop planted by an
    /// old save has none and reads as its attribute label.
    pub name: Option<String>,
    /// The instant it can be picked. A surface subtracts its own clock, so the
    /// countdown keeps moving without the domain being polled for it.
    pub ready_at_unix: i64,
    /// Whether this bed has been told — and is telling — that the seed did not
    /// take. False for every bed before its halfway mark, which is the whole
    /// point of the reveal: a surface cannot leak a verdict the domain has not
    /// given it.
    pub withered: bool,
}

/// One plot, as a surface reads it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PlotView {
    pub crop: Option<CropView>,
}

impl PlotView {
    /// Whether this bed is showing a crop that did not take. A surface reads
    /// this rather than the crop's fields so the rule for "dead" lives in one
    /// place, and the button it draws and the command it sends agree.
    pub fn is_crop_dead(&self) -> bool {
        self.crop.as_ref().is_some_and(|crop| crop.withered)
    }
}

/// The guardian a surface offers to fight.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct GuardianView {
    pub target: Realm,
    pub power: u32,
}

/// The drop a surface announces.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum DropView {
    Equipment(ItemView),
    Seed {
        attribute: Attribute,
        tier: u8,
        grade: SeedGrade,
    },
    Art {
        name: String,
    },
    Herb {
        herb: super::AptitudeHerb,
    },
    Stones(u32),
}

/// What one walk turned up, as a surface reads it.
///
/// A walk is the only source that pays every kind at once, so its find is
/// carried whole rather than reduced to the outcome's name: the panel can say
/// which herb, which piece, which 功法 and how much money came of it.
///
/// A struct rather than one-of, because the rolls are not exclusive: 灵石 is
/// drawn beside the finding rather than in place of it, so a walk can come
/// back with a herb and a handful of stones both, and a find that could only
/// hold one would have to drop the other on the floor.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ExplorationFindView {
    /// The herb it turned up, when it turned one up.
    pub seed: Option<SeedView>,
    /// The gear it turned up, when it turned some up.
    pub gear: Option<ItemView>,
    /// The 功法 it turned up, by the name it was given.
    pub art: Option<String>,
    /// The 灵石 it turned up, when it turned any up.
    pub stones: Option<u32>,
}

impl ExplorationFindView {
    /// Whether the walk came back with nothing at all.
    pub fn is_empty(&self) -> bool {
        self.seed.is_none() && self.gear.is_none() && self.art.is_none() && self.stones.is_none()
    }
}

/// The total power, cut into the sources that made it.
///
/// Every field except `external` adds up to [`Self::total`] exactly: power is
/// linear in the four attributes, so a source is worth on its own what it is
/// worth in the sum. `external` is what the best 外功 adds to a blow and is
/// deliberately outside that sum — a 外功 is not part of what the body is
/// worth.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct PowerBreakdown {
    /// What the level alone carries.
    pub level: u32,
    /// What the points the user placed are worth.
    pub allocated: u32,
    /// The four attributes the worn gear carries.
    pub equipment: u32,
    /// What the worn gear adds outright — 攻击力, 生命值, 命中 and the rest.
    pub equipment_bonus: u32,
    /// The attributes the crops have paid.
    pub crop: u32,
    /// The attributes the 内功 have paid.
    pub art: u32,
    /// What the 内功 add outright.
    pub art_bonus: u32,
    /// What the best 外功 adds per blow, as power. Not part of [`Self::total`].
    pub external: u32,
    /// The number every surface prints as 战力.
    pub total: u32,
}

impl PowerBreakdown {
    /// The sources as they add up — the number that has to equal
    /// [`Self::total`].
    pub fn parts_total(self) -> u32 {
        self.level
            .saturating_add(self.allocated)
            .saturating_add(self.equipment)
            .saturating_add(self.equipment_bonus)
            .saturating_add(self.crop)
            .saturating_add(self.art)
            .saturating_add(self.art_bonus)
    }
}

/// Everything a surface reads in one value.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CultivationView {
    pub level: u16,
    pub realm: Realm,
    /// The layer inside the realm, as [`LayerRank`] says it: ten layers of ten
    /// ranks, so 89 is layer 8 rank 9.
    pub layer: u8,
    /// The rank inside that layer, one-based.
    pub rank: u8,
    pub growth: u32,
    /// The growth the current level opened at.
    pub floor: u32,
    /// The growth the next level needs, or `None` at the realm's cap or the top
    /// of the ladder.
    pub next: Option<u32>,
    /// Level, gear, herbs and the points the user placed together.
    pub attributes: Attributes,
    /// What those four are worth once read as a fighter.
    pub derived: Derived,
    pub level_attributes: Attributes,
    pub equipment_attributes: Attributes,
    pub crop_attributes: Attributes,
    /// The points the user placed by hand.
    pub allocated_attributes: Attributes,
    /// The free points still waiting to be placed.
    pub free_points: u32,
    pub power: u32,
    /// The total above, cut into the sources that made it. Every line of it adds
    /// up to [`Self::power`], which is what lets the panel print a sum the user
    /// can check against the number in the corner.
    pub power_breakdown: PowerBreakdown,
    pub equipped: [Option<ItemView>; SLOT_COUNT],
    pub inventory: Vec<ItemView>,
    /// Every active 装备 set: one entry per set with two or more pieces worn,
    /// with what the wearing pays. Empty when no set is active.
    pub gear_sets: Vec<GearSetView>,
    pub seeds: Vec<SeedView>,
    pub plots: Vec<PlotView>,
    /// What one more garden plot would cost, and `None` once the garden holds
    /// every plot it can.
    pub next_plot_price: Option<u32>,
    /// How well the garden is tended: the rank every planting's odds are read
    /// against.
    pub garden_rank: u8,
    /// The 造化 banked towards the next rank, and what that rank asks for.
    ///
    /// Carried as the pair rather than as a ratio because the surface has to
    /// draw a bar and a number, and a bar whose ceiling it had to recompute is
    /// a bar that can disagree with the price below it. Both read zero-filed at
    /// the top rank: there is no sixteenth apple to reach for.
    pub garden_xp: i64,
    pub garden_xp_needed: i64,
    /// What the next rank costs in 灵石, and `None` once the garden is at the
    /// top of the ladder.
    pub garden_upgrade_cost: Option<u32>,
    /// The guardian offered right now: present only at a realm's last layer.
    pub guardian: Option<GuardianView>,
    pub last_breakthrough: Option<BreakthroughOutcome>,
    pub last_absence: Option<AbsenceCharge>,
    pub last_drop: Option<DropView>,
    /// What the last finished job paid, kept past the quit that ends its popup.
    pub last_work: Option<WorkPaid>,
    /// The jobs on the board today, and the one being worked if there is one.
    pub jobs: Vec<JobView>,
    /// The job being worked, with the instant it comes due.
    pub work: Option<super::JobRun>,
    /// What a fresh board costs, at the realm the cultivator is worth.
    pub job_refresh_cost: u32,
    /// The seed the most recent harvest gave back, if it gave one.
    pub last_seed_return: Option<SeedView>,
    /// Every 功法 owned, with the blueprint folded in.
    ///
    /// These are the **功法书**: the ones still being trained. A 功法 that has
    /// been trained to 圆满 is not in this list — it is in
    /// [`Self::mastered_arts`] instead.
    pub arts: Vec<ArtView>,
    /// Every 功法 learned outright, with the blueprint folded in.
    ///
    /// Kept apart from [`Self::arts`] because the two are sold differently: a
    /// book can be sold, and what it taught cannot be taken back by selling it.
    pub mastered_arts: Vec<MasteredArtView>,
    /// Every active 功法 family: one entry per 前缀 with two or more books,
    /// with what the holding pays. Empty when no set is active.
    pub art_sets: Vec<ArtSetView>,
    /// What the trained 内功 have paid in total.
    pub art_attributes: Attributes,
    /// What those 内功 are worth as battle power, at the same scale the total
    /// power uses: a slice of it, not a second number.
    pub art_power: u32,
    /// What the best 外功 the cultivator knows would add per blow, as power.
    ///
    /// An 外功 pays no attribute, so it cannot show up in any of the totals
    /// above; this is the figure that says what having one is worth.
    pub external_power: u32,
    /// 潜质, in tenths: 47 reads as 4.7. Fractional on purpose — every herb
    /// pays a tenth of a point at the smallest.
    pub aptitude_tenths: i32,
    /// The 潜质 items held, in [`AptitudeHerb::ALL`] order.
    pub aptitude_herbs: [u32; 3],
    /// What opening the gate ahead costs in 洗髓丹, or `None` at the top of the
    /// band. Read off the same rule the crossing charges, so the satchel's
    /// button prices the gate exactly what a herb would have paid for it.
    pub marrow_gate_cost: Option<u32>,
    /// How many levels the ladder holds in all: [`MAX_LEVEL`], carried so a
    /// surface can print "347 / 1000" without importing the ladder's size.
    pub total_levels: u16,
    /// Whether the realm is full: the growth has bought every layer the realm
    /// has, and only the guardian stands between the save and the next one.
    ///
    /// The page's one prompt reads this: a hint about breaking through belongs
    /// on screen when there is a wall to break, and nowhere else.
    pub realm_full: bool,
    /// The 功法 the next finished pomodoro trains.
    pub selected_art: Option<u64>,
    /// What the last finished session did to the 功法 being trained.
    pub last_training: Option<TrainingView>,
    /// The fight the last walk met, if it met one.
    ///
    /// Cleared by every walk that met nobody: a quiet walk's popup must carry
    /// no fight, or it replays an older walk's battle under this walk's
    /// verdict.
    pub last_exploration_duel: Option<DuelReport>,
    /// What the last walk came to.
    pub last_exploration: Option<ExplorationOutcome>,
    /// What the last walk turned up, when it turned something up.
    pub last_exploration_found: Option<ExplorationFindView>,
    /// Whether a walk is owed right now.
    pub exploration_ready: bool,
    /// Whether the cultivator is home: no walk on the road and no shift at work.
    ///
    /// The one state the page prints above the log, and the one that gates every
    /// 外出 and 战斗 action — a cultivator away from the door cannot start a
    /// second walk, take a job, or fight a 破境, a 守卫 or a 试炼 until the road
    /// has brought them back.
    pub at_home: bool,
    /// Whether the guardian can be fought right now.
    pub breakthrough_ready: bool,
    /// The instant a walk is owed again, while one is not.
    pub exploration_ready_at: Option<i64>,
    /// How long the last walk was, which is what set that instant.
    pub exploration_hours: super::ExplorationHours,
    /// Whether the walk under way has already been called home.
    ///
    /// A called walk still owes its return trip, so the page keeps it on the
    /// road rather than offering to call it twice.
    pub walk_early: bool,
    /// Whether the shift under way has already been called home.
    ///
    /// The walk's rule, read off the other outing: the page draws the status
    /// line as "on the way back" instead of offering to call a called shift.
    pub work_early: bool,
    /// How long the trip home would cost if the outing were called back now, in
    /// seconds. Zero while at home.
    ///
    /// The page says the price before the click rather than after it: the return
    /// trip is a third of the time already spent, and a user deciding whether to
    /// give up the hours is owed that number in the confirmation itself.
    pub outing_return_seconds: i64,
    /// The instant the guardian can be fought again, while a loss still holds
    /// the attempt.
    pub breakthrough_ready_at: Option<i64>,
    /// How many breakthroughs have been fought so far.
    pub breakthrough_attempts: u32,
    /// How many walks have been taken so far.
    pub explorations: u32,
    /// The highest 试炼 floor cleared, zero before the tower is entered.
    pub trial_floor: u16,
    /// The floor the next attempt climbs.
    pub trial_next_floor: u16,
    /// What that floor's warden is worth, as the panel prints it.
    pub trial_warden_power: u32,
    /// How many floors the local day still offers.
    pub trial_attempts_left: u32,
    /// Why a floor cannot be climbed right now, or `None` when one can.
    pub trial_blocked: Option<TrialBlocked>,
    /// The fight the last floor settled, for the popup that replays it.
    pub last_trial: Option<TrialOutcome>,
    /// Whether the next floor is a sweep rather than a fight: the cultivator is
    /// more than half again as strong as the warden, so the tower offers the
    /// skip instead of the climb.
    pub trial_sweepable: bool,
    /// The haul the last sweep brought home, for the line that names it.
    pub last_trial_sweep: Option<TrialSweepOutcome>,
    /// How many 试炼 chances are banked, and the most that can be.
    pub proving_tokens: u32,
    /// The ceiling the count above is printed against.
    pub proving_token_cap: u32,
    /// Why the courtyard cannot be fought right now, or `None` when it can.
    pub proving_blocked: Option<ProvingBlocked>,
    /// How many 试炼 have been fought, so a surface can tell a new report from
    /// the one it has already shown.
    pub provings: u32,
    /// The fight the last spent chance settled, for the popup that replays it.
    pub last_proving: Option<ProvingOutcome>,
    /// What the day opened with, for the page and for the popup an ambush earns.
    pub last_daily_event: Option<DailyOutcome>,
    /// The local day the morning event belongs to, or `None` before the first.
    ///
    /// Carried so a surface can tell *today's* event from an older one: the
    /// popup opens for the day's own ambush and never replays last week's.
    pub daily_event_day: Option<i64>,
    /// How many mornings have opened since this state was loaded. A count
    /// rather than a flag because "the day opened" is a fact with a history:
    /// the tests walk a ladder across several days through it, and a surface
    /// that ever wants "how many mornings has this save seen" reads it here.
    pub daily_events: u32,
    /// The local day whose morning event the user has already been shown.
    ///
    /// Written down when the surface acknowledges the popup, so a window opened
    /// again the same day does not replay this morning's ambush as if it had
    /// just happened. `None` before the first acknowledgement.
    pub daily_event_seen_day: Option<i64>,
    /// How many star attempts have been made this session, so a surface can
    /// tell a new result from the one it has already shown.
    pub star_attempts: u32,
    /// What the last star attempt came to, kept for the line that says it.
    pub last_star: Option<StarOutcome>,
    /// What the last gathering turned up, kept for the line that says it.
    pub last_collect: Option<CollectOutcome>,
    /// What the last building rank came to, kept for the line that says it.
    pub last_building: Option<BuildingOutcome>,
    /// The region a new walk will be taken in, read off the realm.
    #[serde(rename = "explore_region_idx")]
    pub exploration_region: u8,
    /// The level a new walk will sit at — one above the cleared level, capped.
    #[serde(rename = "explore_level")]
    pub exploration_level: u8,
    /// The level of the walk under way, or zero between walks.
    pub walk_level: u8,
    /// The furthest level cleared in each region, indexed by ladder position.
    pub explored_levels: [u8; super::explore::Region::ALL.len()],
    /// Progress through each region's current level, in tenths of a percent.
    pub region_progress: [u32; super::explore::Region::ALL.len()],
    /// Every region the page lists, shallowest first: the walk's menu.
    pub regions: Vec<RegionView>,
    /// How far the last settled walk moved its region, in tenths of a percent.
    pub walk_gain: u32,
    /// The resource economy as the page reads it: the purse, the two trades and
    /// the four buildings, each with what the page says about it.
    pub resources: ResourcesView,
    /// What the next finished pomodoro pays: 修习, 伐木, or 采石.
    ///
    /// The mode is the fork the session completion reads, so the page has to be
    /// able to show which arm is armed — and to arm the other one.
    pub session_mode: SessionMode,
    /// How many floors have been attempted, so a surface can tell a new report
    /// from one it has already shown.
    pub trial_attempts: u32,
    /// What the next finished session pays, in growth.
    ///
    /// A number rather than a percentage: the four disciplines that decide it
    /// are not shown, so a multiplier would be a rule the user cannot see and
    /// cannot move. This is what the next pomodoro is actually worth, and it is
    /// the number the ladder moves by.
    pub next_session_growth: u32,
    /// The eight shown numbers and the arithmetic behind each of them.
    ///
    /// Carried whole so a hover can print the very sum that produced the
    /// number beside it: the base, the attribute it reads, what one point of it
    /// adds and what the ceiling held it to.
    pub derived_breakdowns: Vec<DerivedBreakdown>,
    /// The fights fought, newest first.
    pub duel_record: Vec<DuelRecord>,
    /// What the last walk settled, kept past the quit that ended its popup.
    pub last_walk: Option<WalkRecord>,
    /// What has happened lately, newest first: sessions, walks, 功法 turning
    /// over, goods bought and sold, crops pulled.
    pub events: Vec<EventRecord>,
    /// The 灵石 in the purse.
    pub stones: u32,
    /// Why the last refused action was refused, if one was.
    ///
    /// The page shows this as an error rather than swallowing it: a click that
    /// does nothing has to say why, or the user reads the button as broken.
    pub last_refusal: Option<Refusal>,
    /// How many actions have been refused so far.
    ///
    /// A count rather than a flag, because the surface watches it to know a
    /// *new* refusal happened: the same reason refused twice is two messages,
    /// and one reason refused twice is one message said twice.
    pub refusal_count: u32,
    /// The day this view was read for.
    ///
    /// Carried because the shelf is rolled from the day: a surface that worked the
    /// day out for itself could disagree with the one the shelf was rolled from by
    /// a timezone's worth of hours, and then every offer would read as bought.
    pub day: i64,
    /// Today's shelf, as the shop rows read it.
    pub shop_offers: Vec<ShopOfferView>,
}

/// What the 集市 pays for one seed of that attribute, tier and grade.
fn seed_sell_price(attribute: Attribute, tier: u8, grade: SeedGrade) -> u32 {
    shop::ShopOffer::Seed {
        attribute,
        tier,
        grade,
        name: None,
    }
    .sell_price()
}

/// What the 集市 pays for one 潜质 herb seed of that herb and grade.
///
/// Priced off the herb offer rather than the seed offer, because a herb seed is
/// not a tier of attribute points — it is a herb, priced the way the shelf
/// prices the herb it used to hand over ready-grown.
fn herb_seed_sell_price(herb: super::AptitudeHerb, grade: SeedGrade) -> u32 {
    shop::ShopOffer::Herb { herb, grade }.sell_price()
}

/// One seed stack as a surface reads it. The price is read off the stack rather
/// than passed in, so a row cannot quote a figure the sale would not pay.
fn seed_view(stack: &SeedStack) -> SeedView {
    let sell_price = match stack.herb {
        Some(herb) => herb_seed_sell_price(herb, stack.grade),
        None => seed_sell_price(stack.attribute, stack.tier, stack.grade),
    };
    SeedView {
        attribute: stack.attribute,
        herb: stack.herb,
        tier: stack.tier,
        grade: stack.grade,
        count: stack.count,
        name: stack.name.clone(),
        sell_price,
    }
}

/// What one bonus drop says in the event log.
///
/// One variant for every shape the jackpot takes, because they are one kind of
/// event — a pomodoro that paid something extra — and the fields that are filled
/// say which shape it was.
fn bonus_event(drop: &Drop) -> EventDetail {
    let find = match drop {
        Drop::Equipment(item) => BonusFind::Equipment {
            name: item.name.clone(),
        },
        Drop::Seed {
            attribute,
            tier,
            grade,
        } => BonusFind::Seed {
            attribute: *attribute,
            tier: *tier,
            grade: *grade,
        },
        Drop::Art { name } => BonusFind::Art { name: name.clone() },
        Drop::Herb { herb, .. } => BonusFind::Herb { herb: *herb },
        Drop::Stones(stones) => BonusFind::Stones { stones: *stones },
    };
    EventDetail::Bonus { find }
}

/// How often a lost breakthrough costs a piece of gear, in parts per thousand.
const LOST_GEAR_PERMILLE: u64 = 250;

/// The cultivation ladder of one user.
#[derive(Clone, Debug, Serialize)]
pub struct Cultivation {
    document: CultivationDocument,
    /// The roll counter, so two sessions finishing in the same second still
    /// roll different items.
    roll_sequence: u64,
    /// A per-launch salt mixed into every roll's seed.
    ///
    /// The counter alone is not enough: it starts at zero on every launch, so
    /// a relaunch that rolls in the same second as the previous one — same
    /// `now_unix`, same counter — would land on the exact same number. The salt
    /// is derived from the birth seed, which is different on every launch (the
    /// wall clock on desktop, a host-supplied value in wasm), so the same second
    /// on two launches never rolls the same thing. It is deliberately *not*
    /// persisted: a salt that came back from the save would be the same salt
    /// every launch, which is the collision this exists to prevent.
    roll_salt: u64,
    last_absence: Option<AbsenceCharge>,
    last_drop: Option<Drop>,
    last_breakthrough: Option<BreakthroughOutcome>,
    last_seed_return: Option<SeedView>,
    /// What the last finished session did to the 功法 being trained.
    last_training: Option<TrainingView>,
    /// The fight the last walk met, if it met one.
    last_exploration_duel: Option<DuelReport>,
    /// What the last walk came to.
    last_exploration: Option<ExplorationOutcome>,
    /// What the last walk turned up, when it turned something up.
    last_exploration_found: Option<ExplorationFindView>,
    /// What the last finished job paid.
    last_work: Option<WorkPaid>,
    /// The floor the last trial settled, if one was climbed this session.
    last_trial: Option<TrialOutcome>,
    /// The haul the last sweep brought home, if one was swept this session.
    last_trial_sweep: Option<TrialSweepOutcome>,
    /// The 试炼 the last spent chance settled, if one was fought this session.
    last_proving: Option<ProvingOutcome>,
    /// How many 试炼 chances have been spent this session. A surface watches
    /// this rather than the report, for the same reason it watches
    /// [`Self::trial_attempts`].
    provings: u32,
    /// What the day opened with, once it has opened.
    last_daily_event: Option<DailyOutcome>,
    /// How many morning events have fired this session. A surface no longer
    /// watches it to open the popup — the day is written down for that — but
    /// the count is the ladder's own history of the mornings it has opened.
    daily_events: u32,
    /// How many star attempts have been made this session. A surface watches
    /// this rather than the outcome, for the same reason it watches
    /// [`Self::daily_events`].
    star_attempts: u32,
    /// What the last star attempt came to.
    last_star: Option<StarOutcome>,
    /// What the last gathering turned up.
    last_collect: Option<CollectOutcome>,
    /// What the last building rank came to.
    last_building: Option<BuildingOutcome>,
    /// How many floors have been attempted this session. A surface watches this
    /// rather than the report, for the same reason it watches
    /// [`Self::breakthrough_attempts`]: a running pomodoro bumps the revision
    /// every second.
    trial_attempts: u32,
    /// How many breakthroughs have been fought. A surface watches this rather
    /// than the report: a pomodoro tick bumps the snapshot revision every
    /// second, so "the revision moved" cannot tell a new fight from a running
    /// timer.
    breakthrough_attempts: u32,
    /// Why the last refused action was refused, if one was.
    ///
    /// Kept beside the document rather than in it: a refusal is a moment, not
    /// a fact worth keeping, and it is written back only so a surface can say
    /// it out loud.
    last_refusal: Option<Refusal>,
    /// How many actions have been refused so far.
    ///
    /// Monotonic, so a surface can tell a *new* refusal from the same reason
    /// sitting in the snapshot since the last time it looked.
    refusal_count: u32,
}

impl Default for Cultivation {
    fn default() -> Self {
        // A fixed birth seed rather than the wall clock: a default is a
        // placeholder a surface draws before a save exists, and the tests use
        // it to build a save whose rolls they can predict. The persistent path
        // goes through [`Self::new`] or [`Self::new_with_seed`], which are the
        // two that need a fresh salt per launch.
        Self::new_with_seed(CultivationDocument::default(), DEFAULT_BIRTH_SEED)
    }
}

/// The birth seed [`Cultivation::default`] uses: a fixed value so a default
/// save's rolls are reproducible, which is what a test that builds a save and
/// asserts an exact outcome needs.
const DEFAULT_BIRTH_SEED: u64 = 0x5EED_1E55_1234_5678;

/// One round of SplitMix64's finalizer, used to derive the per-launch salt.
///
/// A fresh value out of the clock is already well distributed, but the salt is
/// stored and reused across every roll of a launch, so it goes through the
/// finalizer once to keep it from carrying the low bits of a nanosecond count
/// into the seed arithmetic. The same function [`Rng::next_u64`] uses, kept
/// here so the salt does not need a whole [`Rng`] to be made.
fn splitmix_mix(value: u64) -> u64 {
    let mut z = value;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A seed for the one roll a new body gets: its 潜质.
///
/// Read from the clock once, at birth. Every other roll in this module is seeded
/// from the caller's `now_unix`; this is the single exception, because a body
/// rolled from a determinable seed would hand every fresh save the same 潜质,
/// which is the one thing "初始值为 −10～10 的随机值" rules out. The value is
/// written down by the next save, so it is only ever read once.
fn birth_seed() -> u64 {
    // A test builds a save and asserts an exact outcome, so it needs the same
    // rolls every run: a clock-read seed would make every assertion here a
    // coin flip. The persistent paths a user actually runs are not under
    // `cfg(test)` and still read the clock.
    #[cfg(test)]
    {
        DEFAULT_BIRTH_SEED
    }
    #[cfg(not(test))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(DEFAULT_BIRTH_SEED, |elapsed| {
                (elapsed.as_nanos() as u64) ^ 0x9E37_79B9_7F4A_7C15
            })
    }
}

impl Cultivation {
    /// The state a stored document produces.
    ///
    /// A body that has never been born is born here, once: the only wall-clock
    /// read in this module, and it exists because 潜质 starts on a −10..+10
    /// band rather than at zero. The number is written into the document
    /// immediately by the caller's next write, so every later read is the
    /// stored value and nothing is rerolled by a relaunch.
    pub fn new(document: CultivationDocument) -> Self {
        Self::new_with_seed(document, birth_seed())
    }

    /// The same birth roll, with the seed supplied by the caller.
    ///
    /// [`Self::new`] reads the clock, which a browser cannot do: the wasm build
    /// has no `SystemTime`, and the panic it raises is the platform's rather
    /// than the module's. A host that can produce a random number itself hands
    /// it in here instead, so a fresh save still gets its own 潜质.
    pub fn new_with_seed(document: CultivationDocument, seed: u64) -> Self {
        let mut document = document.normalized();
        if document.aptitude_tenths.is_none() {
            let mut rng = Rng::seeded(seed);
            document.aptitude_tenths = Some(super::aptitude::birth_aptitude(&mut rng));
        }
        Self {
            document,
            roll_sequence: 0,
            // Mixed once from the birth seed rather than used raw: the seed is
            // the one value this module reads from the clock, and the salt is
            // what keeps a relaunch's first roll from repeating the previous
            // launch's. A zero seed (a host that handed in 0) still yields a
            // non-zero salt, so the counter keeps its meaning on its own.
            roll_salt: splitmix_mix(seed),
            last_absence: None,
            last_drop: None,
            last_breakthrough: None,
            last_seed_return: None,
            last_training: None,
            last_exploration_duel: None,
            last_exploration: None,
            last_exploration_found: None,
            last_work: None,
            last_trial: None,
            last_trial_sweep: None,
            last_proving: None,
            provings: 0,
            trial_attempts: 0,
            last_daily_event: None,
            daily_events: 0,
            star_attempts: 0,
            last_star: None,
            last_collect: None,
            last_building: None,
            breakthrough_attempts: 0,
            last_refusal: None,
            refusal_count: 0,
        }
    }

    /// The document as it would be written back.
    pub fn document(&self) -> &CultivationDocument {
        &self.document
    }

    /// The realm the cultivator has entered.
    pub fn realm(&self) -> Realm {
        Realm::from_index(usize::from(self.document.realm))
    }

    /// The current level: the growth curve capped at the realm's last layer.
    ///
    /// The cap is the whole point of the guardian: growth past the cap is kept,
    /// but it buys no layers until the fight is won, so camping at a boundary is
    /// never a way around it.
    pub fn level(&self) -> u16 {
        let curve = level_for_growth(self.document.growth).level;
        curve.min(self.realm().cap_level()).min(MAX_LEVEL)
    }

    /// The growth floor of the realm already entered: a penalty never goes
    /// below it, so a breakthrough is never lost to an idle week.
    fn growth_floor(&self) -> u32 {
        stage_floor(usize::from(self.document.realm))
    }

    /// The attributes the level alone carries.
    pub fn level_attributes(&self) -> Attributes {
        base_attributes(self.level())
    }

    /// The attributes the worn gear carries.
    pub fn equipment_attributes(&self) -> Attributes {
        let mut total = Attributes::default();
        for item in self.equipped_items() {
            total = total.plus(item.effective_attributes());
        }
        total
    }

    /// The combat numbers the worn gear adds outright — 攻击力, 生命值, 命中 and
    /// the rest — with each piece's stars folded in.
    ///
    /// These ride beside the attributes rather than through them: a 神装's
    /// outright 攻击力 is 攻击力 the fight reads directly, and a surface that
    /// had to convert it back into 力量 to show it would be showing a number
    /// nobody wears.
    pub fn equipment_bonus(&self) -> Derived {
        let worn: Vec<EquipmentRecord> = self.equipped_items().collect();
        let mut total = Derived::default();
        for item in &worn {
            total = total.plus(item.effective_bonus());
        }
        total.plus(gear_set_bonus(&worn))
    }

    /// The combat numbers the trained 内功 pay outright.
    pub fn art_bonus(&self) -> Derived {
        art_bonus(&self.document.arts)
            .plus(mastered_art_bonus(&self.document.mastered_arts))
            .plus(art_set_bonus(&self.document.arts))
    }

    /// The nine numbers this cultivator fights with: the four attributes read
    /// through the one table, plus everything gear, 功法 and the jobs add
    /// outright.
    ///
    /// The single reader of the fighter's numbers, so the page, the fight and
    /// the breakdown behind them cannot disagree about what a 神装 is worth.
    pub fn derived(&self) -> Derived {
        self.attributes()
            .derived()
            .plus(self.equipment_bonus())
            .plus(self.art_bonus())
            .plus(self.job_bonus())
    }

    /// The outer attribute values the jobs have paid, permanently.
    ///
    /// A job pays a number rather than an attribute: the reward rides in the
    /// derived line the fight reads, beside the gear that pays the same way.
    pub fn job_bonus(&self) -> Derived {
        self.document.job_bonus
    }

    /// The attributes the harvests have paid.
    pub fn crop_attributes(&self) -> Attributes {
        self.document.crop_bonus
    }

    /// The points the user placed by hand.
    pub fn allocated_attributes(&self) -> Attributes {
        self.document.allocated
    }

    /// The attributes every trained 内功 has paid.
    ///
    /// The books still being trained and the 功法 already learned are counted
    /// apart and then added: the books pay what their 阶 have paid, the learned
    /// ones pay their full mastery, and neither counts the other's.
    pub fn art_attributes(&self) -> Attributes {
        art_attributes(&self.document.arts)
            .plus(mastered_art_attributes(&self.document.mastered_arts))
    }

    /// Record why an action was refused, so a surface can say it out loud.
    ///
    /// The count moves with it, which is what lets a surface notice a *new*
    /// refusal rather than re-announcing the reason that has been sitting in
    /// the snapshot since the last time it looked.
    pub(crate) fn note_refusal(&mut self, refusal: Refusal) {
        self.last_refusal = Some(refusal);
        self.refusal_count = self.refusal_count.saturating_add(1);
    }

    /// The points the peak level has paid so far.
    ///
    /// A cultivator starts at level one, so the count is one less than the
    /// highest level ever reached: level one has bought nothing yet.
    pub fn granted_points(&self) -> u32 {
        u32::from(self.document.peak_level.saturating_sub(1)) * FREE_POINTS_PER_LEVEL
    }

    /// The points still waiting to be placed.
    ///
    /// The levels grant one a level and 练功 occasionally pays a few on top, so
    /// the waiting balance is both sources minus what is already placed. A
    /// hand-edited document can hold more allocated points than the peak paid;
    /// the subtraction saturates rather than paying a negative balance, and no
    /// new point is granted until the peak catches up.
    pub fn free_points(&self) -> u32 {
        self.granted_points()
            .saturating_add(self.document.training_points)
            .saturating_sub(self.document.allocated.total())
    }

    /// Place one free point on an attribute.
    ///
    /// Returns whether anything moved: with nothing left to place the call is a
    /// no-op, so a click on a full attribute cannot overwrite a different one.
    pub fn allocate(&mut self, attribute: Attribute) -> bool {
        if self.free_points() == 0 {
            return false;
        }
        attribute.add_to(&mut self.document.allocated, 1);
        true
    }

    /// Count one keyboard or mouse interaction as a point of growth.
    ///
    /// A finished session is the ladder's main fuel; a key or a click is a
    /// trickle that keeps the cultivator's total moving between sessions. The
    /// point is added straight to the lifetime growth, above the realm floor, so
    /// an idle day can never take back growth a keystroke already paid.
    pub fn on_input(&mut self) {
        self.document.growth = self.document.growth.saturating_add(1);
        self.note_peak_level();
    }

    /// Every attribute together: level, gear, herbs, trained 内功 and the points
    /// the user placed.
    pub fn attributes(&self) -> Attributes {
        self.level_attributes()
            .plus(self.equipment_attributes())
            .plus(self.crop_attributes())
            .plus(self.allocated_attributes())
            .plus(self.art_attributes())
    }

    /// 潜质, in tenths: the born band plus everything gear and 功法 pay.
    ///
    /// One reader for the number every surface and every learning roll uses, so
    /// the rate a row prints, the pomodoros a 阶 asks for and the roll that
    /// decides an attempt are all the same value. The 功法 share is the ten
    /// books written for 潜质 — a 阶 at a time, and the mastery bonus on a
    /// finished one — and it is counted in tenths rather than as a whole number,
    /// because 0.2 a 阶 is what the brief's ladder is made of.
    pub fn aptitude_tenths(&self) -> i32 {
        let born = self.document.aptitude_tenths.unwrap_or(0);
        let outright = i32::try_from(self.equipment_bonus().aptitude)
            .unwrap_or(i32::MAX)
            .saturating_add(i32::try_from(self.art_bonus().aptitude).unwrap_or(i32::MAX));
        let books = super::arts::art_aptitude_tenths(&self.document.arts).saturating_add(
            super::arts::mastered_art_aptitude_tenths(&self.document.mastered_arts),
        );
        super::aptitude::clamp_tenths(
            born.saturating_add(outright.saturating_mul(10))
                .saturating_add(books),
        )
    }

    /// 潜质 as a point value: 47 tenths reads as 4.7.
    pub fn aptitude(&self) -> f32 {
        super::aptitude::points(self.aptitude_tenths())
    }

    /// The 潜质 items the cultivator is holding, in [`super::AptitudeHerb::ALL`]
    /// order.
    pub fn aptitude_herbs(&self) -> [u32; 3] {
        [
            self.herb_held(super::AptitudeHerb::Insight),
            self.herb_held(super::AptitudeHerb::Wisdom),
            self.document.marrow_pills,
        ]
    }

    /// How many of one 潜质 herb the cultivator is holding, across every grade
    /// of its stacks. The 洗髓丹 is a count of its own.
    fn herb_held(&self, herb: super::AptitudeHerb) -> u32 {
        self.document
            .herb_stacks
            .iter()
            .filter(|stack| stack.herb == herb)
            .map(|stack| stack.count)
            .sum()
    }

    /// Feed one 潜质 herb to the body.
    ///
    /// The herb pays its grade's one fixed number — the row the satchel shows
    /// is what the body gets — the gate it crosses is paid in 洗髓丹, and
    /// nothing is spent unless both halves can be paid: a herb that purchased
    /// half a move would be a consumable the user cannot account for.
    ///
    /// The gain is measured against the total 潜质 the page shows — born band
    /// plus the whole points gear and 内功 pay — so "stuck at 100" means the
    /// number on the page, not a hidden base behind it.
    pub fn use_aptitude_herb(
        &mut self,
        herb: super::AptitudeHerb,
        now_unix: i64,
    ) -> Result<AptitudeUse, TradeRefusal> {
        // The pill opens a gate; it does not feed the body, so there is nothing
        // for it to do on its own.
        if herb.gain_tenths().is_none() {
            return Err(TradeRefusal::HerbNotHeld);
        }
        // The satchel spends the cheapest grade held first, holding the best
        // for the gate it most wants to cross: a 黄阶 慧心草 is a warm-up, a
        // 神阶 one is kept for a hundred that needs a bigger push.
        let Some(grade) = self.lowest_held_grade(herb) else {
            return Err(TradeRefusal::HerbNotHeld);
        };
        let gained = herb.gain_tenths_for(grade).unwrap_or(1);
        let total = self.aptitude_tenths();
        if total >= super::APTITUDE_MAX_TENTHS {
            return Err(TradeRefusal::AptitudeCapped);
        }
        let outright = total.saturating_sub(self.document.aptitude_tenths.unwrap_or(0));
        let target = total.saturating_add(gained).min(super::APTITUDE_MAX_TENTHS);
        let marble = super::aptitude::marrow_needed(total, target);
        if self.document.marrow_pills < marble {
            return Err(TradeRefusal::NotEnoughMarrow);
        }
        self.take_one_herb(herb, grade);
        self.document.marrow_pills = self.document.marrow_pills.saturating_sub(marble);
        self.document.aptitude_tenths = Some(super::aptitude::clamp_tenths(
            target.saturating_sub(outright),
        ));
        let aptitude_tenths = self.aptitude_tenths();
        let gained_tenths = aptitude_tenths.saturating_sub(total);
        self.push_event(
            now_unix,
            EventDetail::Aptitude {
                herb,
                gained_tenths,
                value_tenths: aptitude_tenths,
                marrow: marble,
            },
        );
        Ok(AptitudeUse {
            herb,
            gained_tenths,
            aptitude_tenths,
            marrow_spent: marble,
        })
    }

    /// The cheapest grade of a herb the satchel is holding, if it holds one.
    fn lowest_held_grade(&self, herb: super::AptitudeHerb) -> Option<super::SeedGrade> {
        self.document
            .herb_stacks
            .iter()
            .filter(|stack| stack.herb == herb && stack.count > 0)
            .min_by_key(|stack| stack.grade.index())
            .map(|stack| stack.grade)
    }

    /// Take one herb of a grade out of the satchel, dropping the stack when it
    /// is spent.
    fn take_one_herb(&mut self, herb: super::AptitudeHerb, grade: super::SeedGrade) {
        if let Some(index) = self
            .document
            .herb_stacks
            .iter()
            .position(|stack| stack.herb == herb && stack.grade == grade && stack.count > 0)
        {
            let stack = &mut self.document.herb_stacks[index];
            stack.count = stack.count.saturating_sub(1);
            if stack.count == 0 {
                self.document.herb_stacks.remove(index);
            }
        }
    }

    /// Feed a herb that has just been found, the moment it is found.
    ///
    /// The page used to hold the three 潜质 items in a row of their own with a
    /// 服用 button beside each; a herb that the body takes on arrival needs
    /// neither the row nor the button. What is held is written first, so a body
    /// that cannot take it — the band is full, or the gate ahead has no pill for
    /// it — simply keeps it in the bag, where the satchel offers the button the
    /// page no longer carries. The grade the herb was found at is what it pays,
    /// and what it keeps when it waits in the bag.
    fn feed_found_herb(
        &mut self,
        herb: super::AptitudeHerb,
        grade: super::SeedGrade,
        now_unix: i64,
    ) {
        // A pill is never fed to the body: it opens a gate, and gates are
        // opened from the satchel by the hand that wants them opened.
        if herb == super::AptitudeHerb::Marrow {
            self.document.marrow_pills = self.document.marrow_pills.saturating_add(1);
            return;
        }
        // The herb is put into the bag first, because the feed is a move *out*
        // of the bag: [`Self::use_aptitude_herb`] spends what the satchel
        // holds, so a herb that has not been stored yet has nothing to spend.
        // A refusal is not a failure to report — the herb stays in the bag,
        // where the satchel offers the button the page no longer carries — and
        // keeping it at the grade it arrived at keeps the satchel honest about
        // what the body will pay for it.
        self.store_herb(herb, grade);
        let _ = self.use_aptitude_herb(herb, now_unix);
    }

    /// Put one herb of a grade into the satchel, onto the stack that already
    /// holds it when there is one.
    fn store_herb(&mut self, herb: super::AptitudeHerb, grade: super::SeedGrade) {
        if let Some(stack) = self
            .document
            .herb_stacks
            .iter_mut()
            .find(|stack| stack.herb == herb && stack.grade == grade)
        {
            stack.count = stack.count.saturating_add(1);
        } else {
            self.document
                .herb_stacks
                .push(super::HerbStack::single(herb, grade));
        }
    }

    /// Spend 洗髓丹 to open the gate ahead: 潜质 is carried up to the next even
    /// hundred, at that gate's own price.
    ///
    /// A pill on its own has nothing to feed, so the only thing it can buy is a
    /// gate — the same gate a herb would have had to cross, at the same price
    /// [`super::aptitude::marrow_needed`] charges. What it skips is the herbs
    /// that would have carried the body up to that hundred; what it does not
    /// skip is the gate itself, which is why a body cannot be bought to the top
    /// of the band without spending the pills the ladder asks for.
    pub fn use_marrow_pill(&mut self, now_unix: i64) -> Result<AptitudeUse, TradeRefusal> {
        let total = self.aptitude_tenths();
        let Some((target, cost)) = super::aptitude::next_gate(total) else {
            return Err(TradeRefusal::AptitudeCapped);
        };
        if self.document.marrow_pills < cost {
            return Err(TradeRefusal::NotEnoughMarrow);
        }
        self.document.marrow_pills = self.document.marrow_pills.saturating_sub(cost);
        // The points gear and 内功 pay outright ride beside the born value, so
        // the move is written against the same total the page prints.
        let outright = total.saturating_sub(self.document.aptitude_tenths.unwrap_or(0));
        self.document.aptitude_tenths = Some(super::aptitude::clamp_tenths(
            target.saturating_sub(outright),
        ));
        let aptitude_tenths = self.aptitude_tenths();
        let gained_tenths = aptitude_tenths.saturating_sub(total);
        let herb = super::AptitudeHerb::Marrow;
        self.push_event(
            now_unix,
            EventDetail::Aptitude {
                herb,
                gained_tenths,
                value_tenths: aptitude_tenths,
                marrow: cost,
            },
        );
        Ok(AptitudeUse {
            herb,
            gained_tenths,
            aptitude_tenths,
            marrow_spent: cost,
        })
    }

    /// The combat power everything together buys.
    ///
    /// The attributes' own two schools, plus what the outright combat numbers
    /// are worth at the rates those same schools pay — a flat 攻击力 stands in
    /// for the 力量 that would have produced it, so a 神装 moves this number
    /// exactly as hard as it hits.
    pub fn power(&self) -> u32 {
        self.attributes()
            .power()
            .saturating_add(self.equipment_bonus().power())
            .saturating_add(self.art_bonus().power())
    }

    /// The total above, cut into the sources that made it — every line adding
    /// up to [`Self::power`], including the one line that is not a source.
    ///
    /// A surface that printed the sources without that line printed a sum that
    /// did not reach the total, which is what the panel used to do: four
    /// attributes read as two schools pay a *pairing* bonus on the shorter half
    /// of each pair, and a pairing counted per source is worth less than the
    /// same pairing counted once over the whole body. Splitting the body into
    /// five parts therefore loses `min(a, b) − Σ min(aᵢ, bᵢ)` on each half, and
    /// that difference is [`PowerBreakdown::synergy`].
    ///
    /// The parts are the level, the points the user placed, the gear's own
    /// attributes, what the gear adds outright, the crops, the 内功's
    /// attributes and what the 内功 adds outright. 外功 is priced separately by
    /// [`Self::external_power`] because it is not part of this total: it is what
    /// a blow lands for, not what the body is worth.
    pub fn power_breakdown(&self) -> PowerBreakdown {
        let allocated = self.allocated_attributes().power();
        let equipment = self.equipment_attributes().power();
        let equipment_bonus = self.equipment_bonus().power();
        let crop = self.crop_attributes().power();
        let art = self.art_attributes().power();
        let art_bonus = self.art_bonus().power();
        let total = self.power();
        // Every line above is a rounded number, so six of them can come to a
        // point or two short of the whole. The level is the line that carries the
        // difference: it is by far the largest, and a breakdown whose lines did
        // not reach its own total is the report this panel exists to answer.
        let rest = allocated
            .saturating_add(equipment)
            .saturating_add(equipment_bonus)
            .saturating_add(crop)
            .saturating_add(art)
            .saturating_add(art_bonus);
        let level = total.saturating_sub(rest);
        PowerBreakdown {
            level,
            allocated,
            equipment,
            equipment_bonus,
            crop,
            art,
            art_bonus,
            external: self.external_power(),
            total,
        }
    }

    /// What the 内功 alone are worth as power.
    ///
    /// Their attributes at the very scale the four attributes are read at, plus
    /// the outright combat lines they pay — so "内功 +N" is a slice of the total
    /// above and not a second, incomparable number.
    pub fn art_power(&self) -> u32 {
        self.art_attributes()
            .power()
            .saturating_add(self.art_bonus().power())
    }

    /// What the best 外功 the cultivator knows would add per blow, as power.
    ///
    /// The *best* one rather than the sum: a fight swings one 外功 at a time, so
    /// adding four of them up would print a number no fight could ever reach.
    /// Each candidate is priced off the 攻击力 the cultivator actually carries,
    /// which is the number the strike scales from.
    pub fn external_power(&self) -> u32 {
        let attack = self.derived().attack;
        let known = self
            .document
            .arts
            .iter()
            .map(|art| art.art_id)
            .chain(self.document.mastered_arts.iter().copied());
        let mut best = 0;
        for id in known {
            let art = blueprint(id);
            if art.kind != ArtKind::External {
                continue;
            }
            best = best.max(super::arts::external_power_gain(
                attack,
                art.multiplier_permille,
            ));
        }
        best
    }

    /// The items currently worn, in slot order.
    fn equipped_items(&self) -> impl Iterator<Item = EquipmentRecord> + '_ {
        self.document
            .equipped
            .iter()
            .take(SLOT_COUNT)
            .filter_map(|slot| slot.and_then(|id| self.item(id)))
    }

    /// The item with this id, wherever it is.
    fn item(&self, id: u64) -> Option<EquipmentRecord> {
        self.document
            .equipment
            .iter()
            .find(|item| item.id == id)
            .cloned()
    }

    /// Charge every whole local day without a session that has not been charged
    /// yet.
    ///
    /// Charged through yesterday, never through today: a day the user is still
    /// in cannot have been finished without a session, and charging it would
    /// punish someone for not having worked yet at nine in the morning.
    pub fn apply_absence(&mut self, today: CalendarDay, now_unix: i64) -> Option<AbsenceCharge> {
        let last_active = self.document.last_active_day?;
        let charged_through = self.document.charged_through_day.unwrap_or(last_active);
        let first = charged_through.saturating_add(1);
        let last = today.day_number().saturating_sub(1);
        if last < first {
            return None;
        }
        let mut days = 0u32;
        let mut asked = 0u32;
        for _ in first..=last {
            asked = asked.saturating_add(scaled_growth(
                absence_penalty(
                    self.document.missed_streak,
                    layer_growth(self.document.realm),
                ),
                self.growth_multiplier_permille(),
            ));
            self.document.missed_streak = self.document.missed_streak.saturating_add(1);
            days = days.saturating_add(1);
        }
        self.document.charged_through_day = Some(last);
        let before = self.document.growth;
        self.document.growth = before.saturating_sub(asked).max(self.growth_floor());
        let charge = AbsenceCharge {
            days,
            charged: before.saturating_sub(self.document.growth),
        };
        self.last_absence = Some(charge);
        // The log keeps the charge past the quit that ends the notice: it is the
        // one move the ladder makes while nobody is watching, and a level the
        // user cannot account for is a level they will report as a bug.
        self.push_event(
            now_unix,
            EventDetail::Absence {
                days: charge.days,
                charged: charge.charged,
            },
        );
        Some(charge)
    }

    /// Fold one finished focus session into the ladder.
    pub fn on_session_completed(
        &mut self,
        now_unix: i64,
        today: CalendarDay,
        _focus_minutes: u32,
    ) -> SessionReward {
        let absence = self.apply_absence(today, now_unix);
        let today_number = today.day_number();
        // The session number resets on a new local day, which `last_active_day`
        // already records: the first sit-down of the day is number one, every
        // further one the same day escalates. The count is the realm's stage
        // aware, so the growth formula rewards climbing as well as repeating.
        if self.document.last_active_day != Some(today_number) {
            self.document.session_count_today = 0;
        }
        let session_number = self.document.session_count_today.saturating_add(1);
        self.document.session_count_today = session_number;
        let stage = self.realm().index() as u32 + 1;
        // The four disciplines say what this save has been doing, and a session
        // is worth what that is worth: a trained cultivator's pomodoro is several
        // times a bare one, up to the ceiling [`scaled_growth`] enforces.
        let growth = scaled_growth(
            super::session_growth(session_number, stage),
            self.growth_multiplier_permille(),
        );
        self.document.growth = self.document.growth.saturating_add(growth);
        self.document.last_active_day = Some(today_number);
        self.document.missed_streak = 0;
        self.document.charged_through_day = Some(today_number);
        self.note_peak_level();
        // The log says a session happened and what it paid, newest behind the
        // training it caused: the 功法's turn is the more telling line of the two.
        self.push_event(now_unix, EventDetail::Session { growth });
        // The session also trains whatever 功法 is selected: the pomodoro is the
        // only honest measure of time spent training, so learning rides the same
        // event that pays the growth.
        self.train_selected(now_unix);
        // The session's cut of 灵石, credited here so it lands exactly when the
        // growth does: a purse that paid on a different day than the work would
        // be a second, quieter ladder sitting next to the first.
        self.credit_session_earnings();
        // The session also buys one 试炼 chance: the training is the faucet for
        // the courtyard's other half, and ten chances is the bank it feeds.
        self.document.proving_tokens = self
            .document
            .proving_tokens
            .saturating_add(1)
            .min(TRIAL_CHANCES_CAP);
        let drop = self.roll_drop(now_unix);
        self.last_drop = drop.clone();
        SessionReward {
            growth,
            absence,
            drop,
        }
    }

    /// The chance one 练功 session pays its points, in parts per thousand.
    ///
    /// A finished pomodoro with no 功法 in study is a 练功 session, and it is a
    /// *chance* rather than a certainty so the desk stays the place the
    /// numbers come from instead of becoming a point dispenser. A quarter of
    /// the sessions pay.
    const TRAIN_BODY_CHANCE_PERMILLE: u64 = 250;

    /// The most one 练功 session can pay: three points. The least is one.
    const TRAIN_BODY_MAX_POINTS: u32 = 3;

    /// Put the finished session into the selected 功法, if there is one.
    ///
    /// 潜质 is what decides how often the attempt lands, so a 根骨 build learns
    /// faster — the same knob the panel promises when it shows 潜质.
    ///
    /// A 功法 that reaches its last 阶 leaves the books and joins the learned
    /// ones: it is no longer something being trained, and the attributes it
    /// pays are paid from then on whether or not its book is still on the
    /// shelf.
    ///
    /// With no 功法 selected the session trains the body instead: 练功, the
    /// fallback that keeps a pomodoro a session even when no book is open.
    fn train_selected(&mut self, now_unix: i64) {
        let Some(art_id) = self.document.selected_art else {
            self.train_body(now_unix);
            return;
        };
        let aptitude_tenths = self.aptitude_tenths();
        let mut rng = self.rng(now_unix);
        let Some(art) = self
            .document
            .arts
            .iter_mut()
            .find(|art| art.art_id == art_id)
        else {
            return;
        };
        let outcome = train(art, aptitude_tenths, self.document.dojo_level, &mut rng);
        let rank = art.rank;
        // Read before the book leaves the shelf: the copies beyond the one just
        // finished are unread books, and finishing one must not burn them.
        let copies = art.copies.max(1);
        // Every attempt counts as training, a failure included: the panel's
        // question is "what have I been working on", not "what have I won".
        self.note_recent_art(art_id);
        let max_rank = blueprint(art_id).grade.ranks();
        if outcome == ArtOutcome::Mastered && rank >= max_rank {
            // The book has been read to the end: it stops being a book. The
            // learned 功法 is written down in its own list so that selling the
            // book cannot take what it taught with it.
            self.document.arts.retain(|art| art.art_id != art_id);
            if copies > 1 {
                // The spares were never opened, so they go back on the shelf as
                // one unread book: a 功法 finished from one copy does not take
                // the others with it, and what is left is sellable on its own.
                self.document.arts.push(OwnedArt {
                    art_id,
                    rank: 0,
                    progress: 0,
                    outcome: None,
                    failures: 0,
                    copies: copies - 1,
                });
            }
            if !self.document.mastered_arts.contains(&art_id) {
                self.document.mastered_arts.push(art_id);
            }
            // Nothing is being trained any more, so the selection goes with it —
            // including when a spare is left: a book being read again is the
            // user's choice, not something a finished session starts.
            if self.document.selected_art == Some(art_id) {
                self.document.selected_art = None;
            }
        }
        self.last_training = Some(TrainingView {
            art_id,
            name: blueprint(art_id).name,
            outcome,
            rank,
        });
        // A session that trained a 功法 says what came of it, whatever it was:
        // a run of failures is history too, and the record that only kept the
        // wins would be a flatterer.
        self.push_event(
            now_unix,
            EventDetail::Training {
                name: blueprint(art_id).name,
                outcome,
                rank,
            },
        );
    }

    /// A finished pomodoro with no 功法 in study trains the body instead.
    ///
    /// The desk is the only place the numbers come from, so a session nobody
    /// spent on a book is still a session — 练功 is what it becomes. It pays
    /// straight into the attribute points, one to three at a time and only on
    /// [`TRAIN_BODY_CHANCE_PERMILLE`], and the pay is written to the log so a
    /// point the body earned is one the user was told about.
    fn train_body(&mut self, now_unix: i64) {
        let mut rng = self.rng(now_unix);
        if rng.below(1_000) >= Self::TRAIN_BODY_CHANCE_PERMILLE {
            return;
        }
        let points = u32::try_from(rng.below(u64::from(Self::TRAIN_BODY_MAX_POINTS)) + 1)
            .unwrap_or(Self::TRAIN_BODY_MAX_POINTS);
        self.document.training_points = self.document.training_points.saturating_add(points);
        self.push_event(now_unix, EventDetail::TrainBody { points });
    }

    /// Which 功法 the next finished pomodoro trains, or none.
    pub fn select_art(&mut self, art_id: Option<u64>) -> bool {
        if let Some(art_id) = art_id
            && !self.document.arts.iter().any(|art| art.art_id == art_id)
        {
            return false;
        }
        self.document.selected_art = art_id;
        true
    }

    /// Find a 功法 of the asked-for grade — a book the cultivator may well
    /// already own.
    ///
    /// The roll is over the grade's whole catalogue rather than over the books
    /// missing from the shelf: a 功法 can be found twice, the way a piece of
    /// gear can, and the second copy is folded into the book already in hand
    /// (see [`Self::keep_art_book`]) rather than being swapped for one the
    /// cultivator has never met.
    ///
    /// A 功法 learned outright is the one thing left out of the pool: there is
    /// no book left in it, so a second copy would teach nothing. Only a grade
    /// whose every 功法 is learned sends the roll down a grade, and only a
    /// cultivator who knows all five hundred walks away with nothing.
    pub fn grant_art(&mut self, grade: ArtGrade, now_unix: i64) -> Option<ArtView> {
        let mut rng = self.rng(now_unix);
        for grade in ArtGrade::ALL.iter().rev().skip_while(|g| **g != grade) {
            let pool: Vec<u64> = (1..=ART_COUNT)
                .filter(|id| {
                    blueprint(*id).grade == *grade && !self.document.mastered_arts.contains(id)
                })
                .collect();
            if pool.is_empty() {
                continue;
            }
            let art_id = pool[rng.below(pool.len() as u64) as usize];
            return self.keep_art_book(art_id);
        }
        None
    }

    /// Give the cultivator the book for one named 功法.
    ///
    /// The counterpart to [`Self::grant_art`]: a walk or a fight pays a 功法
    /// nobody chose, while the market stocks a specific one and sells that one.
    /// The shelf names a 功法, so a book already on it is a spare added to the
    /// count rather than a different 功法 substituted for the one on show
    /// ([`Self::keep_art_book`]).
    pub(crate) fn grant_art_book(&mut self, art_id: u64) -> Option<ArtView> {
        self.keep_art_book(art_id)
    }

    /// Put one 功法 book on the shelf, or one more copy beside the book already
    /// sitting there.
    ///
    /// A book the cultivator is still reading is not started over: a second copy
    /// of a 功法 already in hand is a spare, so the 阶 it stands at, the
    /// pomodoros banked toward the next one and the failures it has already cost
    /// stay exactly where they were and the reading carries on. What a duplicate
    /// adds is the count — one more physical book, worth the stones a sale
    /// brings and worth nothing else.
    ///
    /// A 功法 learned outright has no book left in it, so nothing about a second
    /// copy would be new and nothing is handed over.
    fn keep_art_book(&mut self, art_id: u64) -> Option<ArtView> {
        if !(1..=ART_COUNT).contains(&art_id) {
            return None;
        }
        let aptitude_tenths = self.aptitude_tenths();
        if let Some(index) = self
            .document
            .arts
            .iter()
            .position(|art| art.art_id == art_id)
        {
            let art = &mut self.document.arts[index];
            art.copies = art.copies.max(1).saturating_add(1);
            let selected = self.document.selected_art == Some(art_id);
            return Some(art_view(
                &self.document.arts[index],
                aptitude_tenths,
                selected,
                self.document.dojo_level,
            ));
        }
        if self.document.mastered_arts.contains(&art_id) {
            return None;
        }
        self.document.arts.push(OwnedArt {
            art_id,
            // A book on the shelf is not yet any of it learned: the first 阶 is
            // what the sessions buy.
            rank: 0,
            progress: 0,
            outcome: None,
            failures: 0,
            copies: 1,
        });
        if self.document.selected_art.is_none() {
            self.document.selected_art = Some(art_id);
        }
        let selected = self.document.selected_art == Some(art_id);
        Some(art_view(
            self.document.arts.last()?,
            aptitude_tenths,
            selected,
            self.document.dojo_level,
        ))
    }

    /// Remember a new high-water mark for the level.
    ///
    /// The free points are paid against this rather than the live level, so a
    /// missed week lowers the ladder but never the points already earned.
    /// Put a 功法 at the front of the recent ring.
    ///
    /// The ring is capped and de-duplicated: one book read twice is one entry,
    /// and the ninth-oldest falls off rather than the list growing with the
    /// save's age.
    fn note_recent_art(&mut self, art_id: u64) {
        self.document.recent_arts.retain(|recent| *recent != art_id);
        self.document.recent_arts.insert(0, art_id);
        self.document.recent_arts.truncate(RECENT_ARTS_CAP);
    }

    fn note_peak_level(&mut self) {
        self.document.peak_level = self.document.peak_level.max(self.level());
    }

    /// The rolls a finished session makes: its thousandth's luck first, then the
    /// ordinary drop.
    ///
    /// The jackpot is rolled before the ordinary find so a session that hits both
    /// announces the rarer thing — a thousandth beating a hundred-and-tenth is
    /// the one line worth reading. Both are read through the session's own
    /// strength: nothing fought here, so the cultivator's power is the source.
    fn roll_drop(&mut self, now_unix: i64) -> Option<Drop> {
        let mut rng = self.rng(now_unix);
        let strength = Strength::of(self.power());
        if rng.below(1_000) < SESSION_BONUS_PERMILLE
            && let Some(bonus) = self.roll_bonus(strength, &mut rng, now_unix)
        {
            // Written to the log as it is paid: a thousandth is the one thing in
            // a session the user will want to find again later.
            let detail = bonus_event(&bonus);
            self.push_event(now_unix, detail);
            return Some(bonus);
        }
        let roll = rng.below(1_000);
        if roll < EQUIPMENT_DROP_PERMILLE {
            // A quality roll that comes up empty is a session that found nothing:
            // the kind said "gear" and the ladder said "not this time", and the
            // only honest thing to report is the second one.
            let item = self.push_equipment(strength, &mut rng)?;
            // Written to the log as it is paid, the way the jackpot is: an
            // ordinary find that lived only in the notice left the satchel with
            // a piece of gear the history never mentioned.
            self.push_event(
                now_unix,
                EventDetail::Found {
                    gear: item.name.clone(),
                    seed: None,
                },
            );
            return Some(Drop::Equipment(item));
        }
        if roll < EQUIPMENT_DROP_PERMILLE + SEED_DROP_PERMILLE {
            let attribute = Attribute::from_index(rng.below(Attribute::ALL.len() as u64) as usize);
            let grade = roll_seed_grade(&mut rng);
            let seed = self.push_seed(attribute, strength.tier(), grade, &mut rng);
            self.push_event(
                now_unix,
                EventDetail::Found {
                    gear: None,
                    seed: seed.name.clone(),
                },
            );
            return Some(Drop::Seed {
                attribute,
                tier: strength.tier(),
                grade,
            });
        }
        None
    }

    /// What one lucky pomodoro pays: one thing, out of the whole ladder.
    ///
    /// The roll is weighted by what the ladder is short of rather than by what
    /// it is full of: gear and herbs are the ordinary finds, 灵石 is the
    /// consolation, and a 潜质 herb or a 功法 is the shape a thousandth is
    /// *worth*. Every branch is at the session's own strength, so the jackpot is
    /// the same find a walk would have made, only without the walk.
    fn roll_bonus(&mut self, strength: Strength, rng: &mut Rng, now_unix: i64) -> Option<Drop> {
        let roll = rng.below(1_000);
        let mut edge = BONUS_GEAR_PERMILLE;
        if roll < edge {
            let item = self.push_equipment(strength, rng)?;
            return Some(Drop::Equipment(item));
        }
        edge += BONUS_SEED_PERMILLE;
        if roll < edge {
            let attribute = Attribute::from_index(rng.below(Attribute::ALL.len() as u64) as usize);
            let grade = roll_seed_grade(rng);
            self.push_seed(attribute, strength.tier(), grade, rng);
            return Some(Drop::Seed {
                attribute,
                tier: strength.tier(),
                grade,
            });
        }
        edge += BONUS_STONES_PERMILLE;
        if roll < edge {
            let amount = roll_stones(rng, BONUS_STONES, strength.tier());
            self.document.stones = self.document.stones.saturating_add(amount);
            return Some(Drop::Stones(amount));
        }
        edge += BONUS_INSIGHT_PERMILLE;
        if roll < edge {
            // A herb is found as a seed: it has to be grown before it can be
            // taken, so the jackpot lands a seed in the satchel the way a walk's
            // find does. The grade is rolled where the find is, so the seed
            // carries the herb it was found at.
            let grade = roll_seed_grade(rng);
            self.push_herb_seed(super::AptitudeHerb::Insight, grade);
            return Some(Drop::Herb {
                herb: super::AptitudeHerb::Insight,
                grade,
            });
        }
        edge += BONUS_WISDOM_PERMILLE;
        if roll < edge {
            let grade = roll_seed_grade(rng);
            self.push_herb_seed(super::AptitudeHerb::Wisdom, grade);
            return Some(Drop::Herb {
                herb: super::AptitudeHerb::Wisdom,
                grade,
            });
        }
        // The rest of the roll — a hundredth of a thousandth — is a 功法: the
        // rarest shape a pomodoro can pay, and the one that has to be told apart
        // from the same book found on a walk.
        let grade = {
            let mut grade_rng = self.rng(now_unix);
            super::arts::roll_art_grade(&mut grade_rng)
        };
        self.grant_art(grade, now_unix)
            .map(|art| Drop::Art { name: art.name })
    }

    /// A random source for one event. The wall clock is mixed in so a session
    /// never rolls what the previous one did, the counter keeps two events in
    /// the same second apart, and the per-launch salt keeps a relaunch's roll
    /// in the same second from repeating the previous launch's — see
    /// [`Self::roll_salt`].
    fn rng(&mut self, now_unix: i64) -> Rng {
        self.roll_sequence = self.roll_sequence.wrapping_add(1);
        Rng::seeded(
            (now_unix as u64)
                .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                .wrapping_add(self.roll_sequence.wrapping_mul(0xBF58_476D_1CE4_E5B9))
                .wrapping_add(self.roll_salt),
        )
    }

    /// The id the next drop takes.
    fn next_id(&mut self) -> u64 {
        let id = self.document.next_item_id.max(1);
        self.document.next_item_id = id.saturating_add(1);
        id
    }

    /// Own one rolled item, at a source's own strength.
    ///
    /// `None` when the quality roll came up empty: a piece of gear is only ever
    /// rolled because something promised one, and the promise is exactly what
    /// the ladder's remainder takes back — a weak source's find is likelier to
    /// be nothing at all, which is what "获得概率" means.
    fn push_equipment(&mut self, strength: Strength, rng: &mut Rng) -> Option<EquipmentRecord> {
        let rarity = super::drop::roll_quality(rng, strength.factor())?;
        let id = self.next_id();
        let item = roll_equipment(id, strength.tier(), rarity, rng);
        self.document.equipment.push(item.clone());
        Some(item)
    }

    /// Own one item that already exists — the one a beaten opponent was wearing.
    ///
    /// The attributes and the name travel as they were: the piece taken off a
    /// guardian is the piece the panel showed it wearing, and re-rolling it into
    /// "a tier's worth of gear" would be a trophy that is not what was won.
    fn adopt_equipment(&mut self, mut item: EquipmentRecord) -> EquipmentRecord {
        item.id = self.next_id();
        self.document.equipment.push(item.clone());
        item
    }

    /// Take one piece off a beaten opponent.
    ///
    /// What it wore is what it hands over — the piece the panel showed on it,
    /// attributes and all. A bare-handed opponent still pays, and that fallback
    /// is rolled at the opponent's own strength: a strong opponent with nothing
    /// on it is still a strong source.
    fn take_gear(
        &mut self,
        rival: &Opponent,
        strength: Strength,
        rng: &mut Rng,
    ) -> Option<EquipmentRecord> {
        let dropped = (!rival.gear.is_empty())
            .then(|| rival.gear[rng.below(rival.gear.len() as u64) as usize].clone());
        match dropped {
            Some(item) => Some(self.adopt_equipment(item)),
            None => self.push_equipment(strength, rng),
        }
    }

    /// Take the book a beaten opponent was swinging, or grant one of the grade
    /// its realm keeps.
    ///
    /// The opponent's own book is what is taken, and a book the cultivator is
    /// still reading is taken again without being reset — the second copy folds
    /// into the one in hand. The fallback is only for the 功法 that is already
    /// learned outright, and for an opponent carrying none: a fight that
    /// promised a 功法 has to pay one somehow.
    fn take_art(&mut self, rival: &Opponent, _realm: Realm, now_unix: i64) -> Option<String> {
        if let Some(art_id) = rival.art
            && let Some(art) = self.grant_art_book(art_id)
        {
            return Some(art.name);
        }
        // Which grade the fallback book carries is the discovery roll: the realm
        // no longer decides it, because a 神阶 art is meant to be an event a
        // beginner can meet rather than a reward for finishing the ladder.
        let grade = {
            let mut rng = self.rng(now_unix);
            super::arts::roll_art_grade(&mut rng)
        };
        self.grant_art(grade, now_unix).map(|art| art.name)
    }

    /// Own one seed that already has its name.
    ///
    /// The shop names the herbs it sells so a shelf can be read before buying;
    /// folding a bought seed in through [`Self::push_seed`] would re-roll that name
    /// and the shelf would have promised one herb and delivered another.
    ///
    /// A stack is one attribute, one tier *and* one grade: two 地阶 herbs and a
    /// 天阶 one of the same patch are three different things to plant, because
    /// the grade is what the harvest pays at.
    fn push_seed_named(
        &mut self,
        attribute: Attribute,
        tier: u8,
        grade: SeedGrade,
        name: Option<String>,
        rng: &mut Rng,
    ) -> SeedView {
        // Only an attribute stack: a herb stack of the same tier and grade is a
        // different thing, matched on its own field in [`Self::push_herb_seed`].
        if let Some(stack) = self.document.seeds.iter_mut().find(|stack| {
            stack.herb.is_none()
                && stack.attribute == attribute
                && stack.tier == tier
                && stack.grade == grade
        }) {
            stack.count = stack.count.saturating_add(1);
            return seed_view(stack);
        }
        let mut stack = SeedStack::single(attribute, tier, grade);
        stack.name = name.or_else(|| Some(seed_name(rng)));
        let view = seed_view(&stack);
        self.document.seeds.push(stack);
        view
    }

    /// Own one seed, folded into an existing stack when there is one.
    ///
    /// Returns the stack as a surface reads it, so a caller that just found the
    /// seed can say which herb it was.
    fn push_seed(
        &mut self,
        attribute: Attribute,
        tier: u8,
        grade: SeedGrade,
        rng: &mut Rng,
    ) -> SeedView {
        self.push_seed_named(attribute, tier, grade, None, rng)
    }

    /// Own one 潜质 herb seed, at its herb's fixed tier, folded into an existing
    /// stack of the same herb and grade when there is one.
    ///
    /// A herb seed carries no name of its own: the herb *is* its name. A roll
    /// here invented one, so the 悟心草 the shelf had just sold came home as
    /// 幽菊 and nothing on the page still said what the seed grew into. The
    /// attribute seeds below keep their rolled names, because two seeds of the
    /// same attribute, tier and grade are told apart by nothing else.
    fn push_herb_seed(&mut self, herb: super::AptitudeHerb, grade: SeedGrade) -> SeedView {
        let tier = super::herb_tier(herb);
        if let Some(stack) =
            self.document.seeds.iter_mut().find(|stack| {
                stack.herb == Some(herb) && stack.grade == grade && stack.tier == tier
            })
        {
            stack.count = stack.count.saturating_add(1);
            return seed_view(stack);
        }
        let stack = SeedStack::herb_single(herb, grade);
        let view = seed_view(&stack);
        self.document.seeds.push(stack);
        view
    }
}

/// What came of planting one seed.
///
/// There is one answer and it is not the fruitful one: the roll has already
/// happened — it has to happen while the seed is in hand, because the odds are
/// the reason planting is a decision — but the bed keeps the answer to itself
/// until half the growing time is up. So planting always succeeds at the only
/// thing a click can be told about: a crop is in the ground.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PlantOutcome {
    /// The odds it went in at. Worth printing because a plant at ninety-five
    /// percent and a plant at thirty are two different wagers.
    pub permille: u32,
}

// The two later `impl Cultivation` blocks live in child modules. They still
// compile into this module's type, so every public path stays unchanged.
#[path = "state_actions.rs"]
mod actions;
#[path = "state_queries.rs"]
mod queries;

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
