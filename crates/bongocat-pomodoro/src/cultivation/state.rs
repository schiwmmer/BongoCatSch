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
    ART_COUNT, ArtGrade, ArtKind, ArtOutcome, ArtStrike, ArtView, Attribute, Attributes,
    BONUS_GEAR_PERMILLE, BONUS_INSIGHT_PERMILLE, BONUS_SEED_PERMILLE, BONUS_STONES_PERMILLE,
    BONUS_WISDOM_PERMILLE, BREAKTHROUGH_COOLDOWN_SECONDS, BattleReport, CultivationDocument,
    DailyKind, Derived, DerivedBreakdown, DuelReport, EQUIPMENT_DROP_PERMILLE, EquipSlot,
    EquipmentRecord, ExplorationOutcome, FREE_POINTS_PER_LEVEL, Fighter, GearEffect, Guardian,
    MAX_LEVEL, MAX_STAR, MasteredArtView, OwnedArt, PLAYER_NAME, RECENT_ARTS_CAP, Rarity, Realm,
    Refusal, Rng, SEED_DROP_PERMILLE, SESSION_BONUS_PERMILLE, SLOT_COUNT, SeedStack,
    VICTORY_SEED_PERMILLE, absence_penalty, art_attributes, art_view,
    arts::{
        art_bonus, blueprint, mastered_art_attributes, mastered_art_bonus, mastered_art_view, train,
    },
    base_attributes,
    battle::{guardian_for, resolve},
    curve::{layer_growth, layer_rank, level_for_growth, stage_floor, threshold_for},
    daily::{
        DAILY_AMBUSH_STONES, DAILY_GIFT_STONES, DAILY_TRINKET_STONES, GiftKind, roll_gift,
        roll_kind,
    },
    duel::{DUEL_RECORD_CAP, DuelRecord, FightKind, duel},
    events::{BonusFind, EVENT_LOG_CAP, EventDetail, EventRecord, MarrowSource, SaleKind},
    explore::{
        BONUS_STONES, GUARDIAN_STONES, PROVING_STONES, TRIAL_STONES, WALK_STONES, WANDERER_STONES,
        WalkRecord, breakthrough_ready, roll_stones, roll_victory, roll_walk,
    },
    garden::{
        CropRecord, MAX_GARDEN_RANK, PITY_MIN_PERMILLE, PITY_STREAK, PlotRecord, SeedGrade,
        harvest_points, plant_permille, plot_price, rank_cost, rank_xp_total, roll_seed_grade,
        seed_name, seed_return, wither_points,
    },
    growth_permille,
    items::roll_equipment,
    mastery_permille,
    opponent::{self, Opponent},
    proving::{ProvingOutcome, TRIAL_CHANCES_CAP},
    scaled_growth,
    shop::{self, ShopOffer, ShopOfferView, star_up_cost},
    star_success_permille,
    trade::{TradeRefusal, session_earnings},
    trial::{
        TRIAL_ATTEMPTS_PER_DAY, TRIAL_MAX_FLOOR, TrialReward, floor_realm, roll_reward,
        warden_power,
    },
};
use crate::CalendarDay;

/// What the absence evaluation took, if anything.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
#[derive(Clone, Debug, Eq, PartialEq)]
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
    Herb {
        herb: super::AptitudeHerb,
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionReward {
    /// The growth the session itself paid.
    pub growth: u32,
    /// The penalty charged for the days before it, when there were any.
    pub absence: Option<AbsenceCharge>,
    pub drop: Option<Drop>,
}

/// What a breakthrough attempt came to.
#[derive(Clone, Debug, Eq, PartialEq)]
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrialOutcome {
    /// The floor that was climbed, one-based.
    pub floor: u16,
    /// Every blow of the fight that settled it.
    pub duel: DuelReport,
    /// What clearing the floor turned up. A loss finds nothing but the floor.
    pub found: ExplorationFindView,
}

/// Why a 试炼 floor cannot be climbed right now.
///
/// The page needs a reason to print beside a dead button, and it is the same
/// reason the domain refuses with — so the two cannot disagree about whether
/// the tower is closed or the day is spent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
#[derive(Clone, Debug, Eq, PartialEq)]
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
#[derive(Clone, Debug, Eq, PartialEq)]
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
#[derive(Clone, Debug, Eq, PartialEq)]
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
    /// What the attempt cost, in 灵石.
    pub cost: u32,
}

/// What one 潜质 herb did.
///
/// Carried rather than left to the event log because the popup that says "潜质
/// 4.7 → 5.3" has to know the move, and a surface reading the log would have to
/// match a line by timestamp to find it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
/// Kept beside the walk's record for the same reason: the popup is a moment and
/// the quit forgets it, and a page that could not say what the last job paid is a
/// page whose money has no history.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkPaid {
    /// How many 灵石 the job paid, the wage and any 潜质's windfall together.
    pub stones: u32,
    /// How long the job took, in hours.
    pub hours: u8,
    /// The job's grade, which is what its wage was multiplied by.
    pub grade: ArtGrade,
    /// What 潜质 did to the wage: one, two or three times.
    pub multiplier: u32,
}

/// One job as a surface reads it: what it is, how long it takes and what it pays.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JobView {
    /// Its position on the board, which is what taking it names.
    pub slot: usize,
    pub tier: u8,
    pub hours: u8,
    pub grade: ArtGrade,
    /// What it pays, as the one figure the board is read against.
    pub stones: u32,
}

/// One owned item, as a surface reads it.
#[derive(Clone, Debug, Eq, PartialEq)]
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
    /// What one more star costs, in 灵石: half of what the piece sells for.
    /// `None` once the piece holds every star it can.
    pub star_cost: Option<u32>,
    /// The xianxia-given name the drop carried, if any.
    pub name: Option<String>,
    /// What the 集市 pays for it, in 灵石.
    ///
    /// Carried on the view rather than derived at the surface because the price
    /// is the shop's margin over its own asking price: a surface that worked it
    /// out from the tier alone would show a number the sale does not pay.
    pub sell_price: u32,
}

/// One seed stack, as a surface reads it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeedView {
    pub attribute: Attribute,
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrainingView {
    pub art_id: u64,
    pub name: String,
    pub outcome: ArtOutcome,
    /// The 阶 the art stands at now.
    pub rank: u8,
}

/// One crop in a plot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CropView {
    pub attribute: Attribute,
    pub tier: u8,
    /// The grade it was planted at, which is what the harvest will pay at.
    pub grade: SeedGrade,
    /// The herb's name, carried from the seed it grew from. A crop planted by an
    /// old save has none and reads as its attribute label.
    pub name: Option<String>,
    /// The instant it can be picked. A surface subtracts its own clock, so the
    /// countdown keeps moving without the domain being polled for it.
    pub ready_at_unix: i64,
}

/// One plot, as a surface reads it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlotView {
    pub crop: Option<CropView>,
}

/// The guardian a surface offers to fight.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GuardianView {
    pub target: Realm,
    pub power: u32,
}

/// The drop a surface announces.
#[derive(Clone, Debug, Eq, PartialEq)]
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
#[derive(Clone, Debug, Default, Eq, PartialEq)]
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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
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
#[derive(Clone, Debug, Eq, PartialEq)]
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
    /// Whether the next planting is owed one: two counted failures in a row have
    /// earned it whatever the seed asks for, and the surface says so rather than
    /// letting the streak do its work invisibly.
    pub garden_pity_ready: bool,
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

/// One seed stack as a surface reads it. The price is read off the stack rather
/// than passed in, so a row cannot quote a figure the sale would not pay.
fn seed_view(stack: &SeedStack) -> SeedView {
    SeedView {
        attribute: stack.attribute,
        tier: stack.tier,
        grade: stack.grade,
        count: stack.count,
        name: stack.name.clone(),
        sell_price: seed_sell_price(stack.attribute, stack.tier, stack.grade),
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
        Drop::Herb { herb } => BonusFind::Herb { herb: *herb },
        Drop::Stones(stones) => BonusFind::Stones { stones: *stones },
    };
    EventDetail::Bonus { find }
}

/// How often a lost breakthrough costs a piece of gear, in parts per thousand.
const LOST_GEAR_PERMILLE: u64 = 250;

/// The cultivation ladder of one user.
#[derive(Clone, Debug)]
pub struct Cultivation {
    document: CultivationDocument,
    /// The roll counter, so two sessions finishing in the same second still
    /// roll different items.
    roll_sequence: u64,
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
        Self::new(CultivationDocument::default())
    }
}

/// A seed for the one roll a new body gets: its 潜质.
///
/// Read from the clock once, at birth. Every other roll in this module is seeded
/// from the caller's `now_unix`; this is the single exception, because a body
/// rolled from a determinable seed would hand every fresh save the same 潜质,
/// which is the one thing "初始值为 −10～10 的随机值" rules out. The value is
/// written down by the next save, so it is only ever read once.
fn birth_seed() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0x5EED_1E55_1234_5678, |elapsed| {
            (elapsed.as_nanos() as u64) ^ 0x9E37_79B9_7F4A_7C15
        })
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
        let mut document = document.normalized();
        if document.aptitude_tenths.is_none() {
            let mut rng = Rng::seeded(birth_seed());
            document.aptitude_tenths = Some(super::aptitude::birth_aptitude(&mut rng));
        }
        Self {
            document,
            roll_sequence: 0,
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
            last_proving: None,
            provings: 0,
            trial_attempts: 0,
            last_daily_event: None,
            daily_events: 0,
            star_attempts: 0,
            last_star: None,
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
        let mut total = Derived::default();
        for item in self.equipped_items() {
            total = total.plus(item.effective_bonus());
        }
        total
    }

    /// The combat numbers the trained 内功 pay outright.
    pub fn art_bonus(&self) -> Derived {
        art_bonus(&self.document.arts).plus(mastered_art_bonus(&self.document.mastered_arts))
    }

    /// The nine numbers this cultivator fights with: the four attributes read
    /// through the one table, plus everything gear and 功法 add outright.
    ///
    /// The single reader of the fighter's numbers, so the page, the fight and
    /// the breakdown behind them cannot disagree about what a 神装 is worth.
    pub fn derived(&self) -> Derived {
        self.attributes()
            .derived()
            .plus(self.equipment_bonus())
            .plus(self.art_bonus())
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
    /// A hand-edited document can hold more allocated points than the peak paid.
    /// The subtraction saturates rather than paying a negative balance, and no
    /// new point is granted until the peak catches up.
    pub fn free_points(&self) -> u32 {
        self.granted_points()
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
            self.document.insight_herbs,
            self.document.wisdom_herbs,
            self.document.marrow_pills,
        ]
    }

    /// Feed one 潜质 herb to the body.
    ///
    /// The herb pays a roll inside its own band, the gate it crosses is paid in
    /// 洗髓丹, and nothing is spent unless both halves can be paid: a herb that
    /// purchased half a move would be a consumable the user cannot account for.
    ///
    /// The gain is measured against the total 潜质 the page shows — born band
    /// plus the whole points gear and 内功 pay — so "stuck at 100" means the
    /// number on the page, not a hidden base behind it.
    pub fn use_aptitude_herb(
        &mut self,
        herb: super::AptitudeHerb,
        now_unix: i64,
    ) -> Result<AptitudeUse, TradeRefusal> {
        let Some((low, high)) = herb.gain_tenths() else {
            // The pill opens a gate; it does not feed the body, so there is
            // nothing for it to do on its own.
            return Err(TradeRefusal::HerbNotHeld);
        };
        let held = match herb {
            super::AptitudeHerb::Insight => self.document.insight_herbs,
            super::AptitudeHerb::Wisdom => self.document.wisdom_herbs,
            super::AptitudeHerb::Marrow => 0,
        };
        if held == 0 {
            return Err(TradeRefusal::HerbNotHeld);
        }
        let total = self.aptitude_tenths();
        if total >= super::APTITUDE_MAX_TENTHS {
            return Err(TradeRefusal::AptitudeCapped);
        }
        let outright = total.saturating_sub(self.document.aptitude_tenths.unwrap_or(0));
        let mut rng = self.rng(now_unix);
        let span = u64::try_from(high - low + 1).unwrap_or(1);
        let gained = low.saturating_add(i32::try_from(rng.below(span)).unwrap_or(0));
        let target = total.saturating_add(gained).min(super::APTITUDE_MAX_TENTHS);
        let marble = super::aptitude::marrow_needed(total, target);
        if self.document.marrow_pills < marble {
            return Err(TradeRefusal::NotEnoughMarrow);
        }
        match herb {
            super::AptitudeHerb::Insight => {
                self.document.insight_herbs = self.document.insight_herbs.saturating_sub(1);
            }
            super::AptitudeHerb::Wisdom => {
                self.document.wisdom_herbs = self.document.wisdom_herbs.saturating_sub(1);
            }
            super::AptitudeHerb::Marrow => {}
        }
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

    /// Feed a herb that has just been found, the moment it is found.
    ///
    /// The page used to hold the three 潜质 items in a row of their own with a
    /// 服用 button beside each; a herb that the body takes on arrival needs
    /// neither the row nor the button. What is held is written first, so a body
    /// that cannot take it — the band is full, or the gate ahead has no pill for
    /// it — simply keeps it in the bag, where the satchel offers the button the
    /// page no longer carries.
    fn feed_found_herb(&mut self, herb: super::AptitudeHerb, now_unix: i64) {
        match herb {
            super::AptitudeHerb::Insight => {
                self.document.insight_herbs = self.document.insight_herbs.saturating_add(1);
            }
            super::AptitudeHerb::Wisdom => {
                self.document.wisdom_herbs = self.document.wisdom_herbs.saturating_add(1);
            }
            // A pill is never fed to the body: it opens a gate, and gates are
            // opened from the satchel by the hand that wants them opened.
            super::AptitudeHerb::Marrow => {
                self.document.marrow_pills = self.document.marrow_pills.saturating_add(1);
                return;
            }
        }
        // A refusal is not a failure to report: the herb is in the bag and the
        // satchel offers it, which is what a hoarded herb always was.
        let _ = self.use_aptitude_herb(herb, now_unix);
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

    /// Put the finished session into the selected 功法, if there is one.
    ///
    /// 潜质 is what decides how often the attempt lands, so a 根骨 build learns
    /// faster — the same knob the panel promises when it shows 潜质.
    ///
    /// A 功法 that reaches its last 阶 leaves the books and joins the learned
    /// ones: it is no longer something being trained, and the attributes it
    /// pays are paid from then on whether or not its book is still on the
    /// shelf.
    fn train_selected(&mut self, now_unix: i64) {
        let Some(art_id) = self.document.selected_art else {
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
        let outcome = train(art, aptitude_tenths, &mut rng);
        let rank = art.rank;
        // Every attempt counts as training, a failure included: the panel's
        // question is "what have I been working on", not "what have I won".
        self.note_recent_art(art_id);
        let max_rank = blueprint(art_id).grade.ranks();
        if outcome == ArtOutcome::Mastered && rank >= max_rank {
            // The book has been read to the end: it stops being a book. The
            // learned 功法 is written down in its own list so that selling the
            // book cannot take what it taught with it.
            self.document.arts.retain(|art| art.art_id != art_id);
            if !self.document.mastered_arts.contains(&art_id) {
                self.document.mastered_arts.push(art_id);
            }
            // Nothing is being trained any more, so the selection goes with it.
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

    /// Learn a 功法 the cultivator does not own yet.
    ///
    /// Returns whether anything was gained: every 功法 of the asked-for grade is
    /// already owned, the next grade down is tried, and only a cultivator who
    /// owns all five hundred walks away with nothing.
    ///
    /// A 功法 that has already been learned outright is not given again: it is
    /// known, and a second book of it would be a thing the page could not say
    /// anything true about.
    pub fn grant_art(&mut self, grade: ArtGrade, now_unix: i64) -> Option<ArtView> {
        let mut rng = self.rng(now_unix);
        for grade in ArtGrade::ALL.iter().rev().skip_while(|g| **g != grade) {
            let unowned: Vec<u64> = (1..=ART_COUNT)
                .filter(|id| {
                    blueprint(*id).grade == *grade
                        && !self.document.arts.iter().any(|art| art.art_id == *id)
                        && !self.document.mastered_arts.contains(id)
                })
                .collect();
            if unowned.is_empty() {
                continue;
            }
            let art_id = unowned[rng.below(unowned.len() as u64) as usize];
            self.document.arts.push(OwnedArt {
                art_id,
                // A book on the shelf is not yet any of it learned: the first
                // 阶 is what the sessions buy.
                rank: 0,
                progress: 0,
                outcome: None,
                failures: 0,
            });
            if self.document.selected_art.is_none() {
                self.document.selected_art = Some(art_id);
            }
            return Some(art_view(
                self.document.arts.last()?,
                self.aptitude_tenths(),
                true,
            ));
        }
        None
    }

    /// Give the cultivator the book for one named 功法.
    ///
    /// The counterpart to [`Self::grant_art`]: a walk or a fight pays a 功法
    /// nobody chose, while the market stocks a specific one and sells that one.
    /// A book the cultivator already has, or a 功法 they have already learned,
    /// is not given again — the shelf names a 功法, and handing over a second
    /// copy of one they already hold would be a market that sells what it does
    /// not show.
    pub(crate) fn grant_art_book(&mut self, art_id: u64) -> Option<ArtView> {
        if !(1..=ART_COUNT).contains(&art_id) {
            return None;
        }
        if self.document.arts.iter().any(|art| art.art_id == art_id)
            || self.document.mastered_arts.contains(&art_id)
        {
            return None;
        }
        self.document.arts.push(OwnedArt {
            art_id,
            // The same zero the walks and the shelf start a book at: owning it
            // is not having learned any of it.
            rank: 0,
            progress: 0,
            outcome: None,
            failures: 0,
        });
        if self.document.selected_art.is_none() {
            self.document.selected_art = Some(art_id);
        }
        Some(art_view(
            self.document.arts.last()?,
            self.aptitude_tenths(),
            true,
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
            // Straight into the body: a herb found is a herb taken, and the
            // page no longer keeps a shelf of them waiting for a button.
            self.feed_found_herb(super::AptitudeHerb::Insight, now_unix);
            return Some(Drop::Herb {
                herb: super::AptitudeHerb::Insight,
            });
        }
        edge += BONUS_WISDOM_PERMILLE;
        if roll < edge {
            self.feed_found_herb(super::AptitudeHerb::Wisdom, now_unix);
            return Some(Drop::Herb {
                herb: super::AptitudeHerb::Wisdom,
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
    /// never rolls what the previous one did, and the counter keeps two events
    /// in the same second apart.
    fn rng(&mut self, now_unix: i64) -> Rng {
        self.roll_sequence = self.roll_sequence.wrapping_add(1);
        Rng::seeded(
            (now_unix as u64)
                .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                .wrapping_add(self.roll_sequence.wrapping_mul(0xBF58_476D_1CE4_E5B9)),
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
    /// The fallback is not a consolation prize: a book the cultivator already
    /// owns cannot be given twice, and a fight that promised a 功法 has to pay
    /// one somehow.
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
        if let Some(stack) = self.document.seeds.iter_mut().find(|stack| {
            stack.attribute == attribute && stack.tier == tier && stack.grade == grade
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
}

/// What came of planting one seed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlantOutcome {
    /// The seed took, and the crop is in the ground: from here it can only be
    /// picked, never lost.
    Planted,
    /// It did not take. The seed is spent, the plot frees itself at once, and
    /// the gardener keeps what killed it — see [`wither_points`].
    Withered {
        /// The odds it went in at, so the line can say how close it was: losing
        /// at ninety-five percent is not the same news as losing at thirty.
        permille: u32,
        /// The 造化 the failure still paid.
        xp: i32,
    },
}

impl Cultivation {
    /// Plant one seed in the first empty plot.
    ///
    /// The roll happens **now**, while the seed is still in hand, and never at
    /// harvest: a tenth-rank seed asks for ten days of the user's bed, and an
    /// answer ten days late is a punishment they will simply stop asking for.
    /// A crop that survived its planting cannot be lost afterwards, which is why
    /// the plot below is only ever handed to a planting that has already come up.
    ///
    /// Returns what became of it, or `None` when there was no seed of that kind
    /// or nowhere to put one — nothing is spent in either of those cases.
    pub fn plant(
        &mut self,
        attribute: Attribute,
        tier: u8,
        grade: SeedGrade,
        now_unix: i64,
    ) -> Option<PlantOutcome> {
        // Decided before anything is taken, because the two reads below borrow
        // the document's own fields and cannot see past themselves.
        let permille = plant_permille(self.document.garden_rank, tier, grade);
        let certain = self.planting_is_certain(tier, grade);
        let took = {
            let mut rng = self.rng(now_unix);
            certain || rng.below(1_000) < u64::from(permille)
        };
        let stack = self.document.seeds.iter_mut().find(|stack| {
            stack.attribute == attribute && stack.tier == tier && stack.grade == grade
        })?;
        // The plot inherits the herb's name: the seed stack is spent one
        // planting at a time and the name is the only thing that survives it.
        let name = stack.name.clone();
        let plot = self
            .document
            .plots
            .iter_mut()
            .find(|plot| plot.is_empty())?;
        stack.count = stack.count.saturating_sub(1);
        self.document.seeds.retain(|stack| stack.count > 0);
        if took {
            self.document.plant_fail_streak = 0;
            plot.crop = Some(CropRecord::planted(attribute, tier, grade, name, now_unix));
            return Some(PlantOutcome::Planted);
        }
        // Gone, but not grown: the seed is spent, which is what makes the odds
        // worth printing, and the bed it wanted is free again before the line is
        // written. Nothing waits and nothing is owed.
        let xp = wither_points(tier, grade);
        self.document.garden_xp = self.document.garden_xp.saturating_add(i64::from(xp));
        if permille >= PITY_MIN_PERMILLE {
            self.document.plant_fail_streak = self.document.plant_fail_streak.saturating_add(1);
        }
        self.push_event(
            now_unix,
            EventDetail::Withered {
                attribute,
                tier,
                grade,
                name,
                permille,
                xp,
            },
        );
        Some(PlantOutcome::Withered { permille, xp })
    }

    /// Whether the safety net has run out of patience: two counted failures in a
    /// row at even money or better, and the next planting comes up whatever its
    /// odds said.
    ///
    /// Below even money nothing is counted and nothing is owed — planting a
    /// tenth-rank seed in a first-rank bed is a gamble the user chose, and part
    /// of what a gamble pays is its own uncertainty.
    pub fn planting_is_certain(&self, tier: u8, grade: SeedGrade) -> bool {
        plant_permille(self.document.garden_rank, tier, grade) >= PITY_MIN_PERMILLE
            && self.document.plant_fail_streak >= PITY_STREAK
    }

    /// Raise the garden one rank, paid in 灵石 and earned in harvests.
    ///
    /// Two gates rather than one, and deliberately unlike each other: the 造化
    /// comes only from planting and cannot be bought, while the stones come from
    /// *selling what was planted* and cannot be farmed. One is the gardener's
    /// patience, the other is how much of the harvest they were willing to give
    /// up for this instead. See [`rank_cost`] for why the price is what it is.
    pub fn upgrade_garden(&mut self, now_unix: i64) -> Result<u8, TradeRefusal> {
        let rank = self.document.garden_rank;
        if rank >= MAX_GARDEN_RANK {
            return Err(TradeRefusal::GardenCapped);
        }
        if self.document.garden_xp < rank_xp_total(rank + 1) {
            return Err(TradeRefusal::NotEnoughRankXp);
        }
        let price = rank_cost(rank);
        if self.document.stones < price {
            return Err(TradeRefusal::NotEnoughStones);
        }
        self.document.stones -= price;
        self.document.garden_rank = rank.saturating_add(1);
        self.push_event(
            now_unix,
            EventDetail::GardenRank {
                rank: self.document.garden_rank,
                stones: price,
            },
        );
        Ok(self.document.garden_rank)
    }

    /// Pick a ripe crop, if the plot holds one.
    ///
    /// The harvest pays its attribute points — the tier's base times the grade's
    /// multiplier — and then rolls once for a seed: the same attribute, the same
    /// tier or one higher, at a grade of its own. That roll is what keeps a
    /// garden from running dry, and it is deliberately rare next to the points
    /// the crop already paid.
    pub fn harvest(&mut self, plot: usize, now_unix: i64) -> Option<HarvestOutcome> {
        let crop = {
            let plot = self.document.plots.get_mut(plot)?;
            let ready = plot.crop.as_ref()?.is_ready(now_unix);
            if !ready {
                return None;
            }
            plot.crop.take()?
        };
        let points = harvest_points(crop.tier, crop.grade);
        crop.attribute.add_to(&mut self.document.crop_bonus, points);
        // Counted here rather than on the seed roll: a crop that returned nothing
        // was still pulled, and the 造化 dimension is fed by pulling.
        self.document.harvests = self.document.harvests.saturating_add(1);
        // The garden learns from being gardened: the crop's own points are the
        // 造化 towards its next rank, so a herb kept is also a herb counted, and
        // there is no second currency to plant for.
        self.document.garden_xp = self.document.garden_xp.saturating_add(i64::from(points));
        let mut rng = self.rng(now_unix);
        let seed = seed_return(crop.attribute, crop.tier, &mut rng);
        // The name travels with the stack it landed in, so the notice and the
        // log can both say which herb came back rather than "a seed".
        let mut seed_name = None;
        if let Some((attribute, tier, grade)) = seed {
            seed_name = self.push_seed(attribute, tier, grade, &mut rng).name;
        }
        self.last_seed_return = seed.map(|(attribute, tier, grade)| SeedView {
            attribute,
            tier,
            grade,
            count: 1,
            name: seed_name.clone(),
            sell_price: seed_sell_price(attribute, tier, grade),
        });
        // The line names the herb and what it paid, because "a crop came up" is
        // a fact the user cannot check against the garden that is now empty.
        self.push_event(
            now_unix,
            EventDetail::Harvested {
                attribute: crop.attribute,
                tier: crop.tier,
                grade: crop.grade,
                name: crop.name.clone(),
                points,
                seed: seed_name,
            },
        );
        Some(HarvestOutcome { crop, seed })
    }

    /// Wear one owned item in its slot, replacing what was there.
    pub fn equip(&mut self, item_id: u64) -> bool {
        let Some(item) = self.item(item_id) else {
            return false;
        };
        let slot = item.slot.index();
        if self.document.equipped.get(slot).copied().flatten() == Some(item_id) {
            return true;
        }
        if let Some(slot) = self.document.equipped.get_mut(slot) {
            *slot = Some(item_id);
        }
        true
    }

    /// Take whatever is worn in a slot off. The item stays owned.
    pub fn unequip(&mut self, slot: EquipSlot) -> bool {
        let Some(entry) = self.document.equipped.get_mut(slot.index()) else {
            return false;
        };
        entry.take().is_some()
    }

    /// Spend 灵石 on one attempt at a piece's next star.
    ///
    /// The wager is the same shape whatever the piece: the stones are spent
    /// first, the star turns over at [`star_success_permille`] of the star the
    /// piece stands at, and a failure drops one star back — never past the
    /// first, so a run of bad luck costs progress and never the piece's
    /// footing. Refused with the reason when the purse cannot cover it, the
    /// piece is not owned, or the piece already holds every star.
    pub fn star_item(&mut self, item_id: u64, now_unix: i64) -> Result<StarOutcome, TradeRefusal> {
        let Some(item) = self.item(item_id).map(|mut item| {
            item.star = item.star.min(MAX_STAR);
            item
        }) else {
            return Err(TradeRefusal::NotOwned);
        };
        if item.star >= MAX_STAR {
            return Err(TradeRefusal::StarCapped);
        }
        let cost = star_up_cost(&item);
        if self.document.stones < cost {
            return Err(TradeRefusal::NotEnoughStones);
        }
        self.document.stones = self.document.stones.saturating_sub(cost);
        let mut rng = self.rng(now_unix);
        let success = rng.below(1_000) < u64::from(star_success_permille(item.star));
        let from = item.star;
        // One up on a success; one down on a failure, with the first star as
        // the floor — a piece is never walked back to nothing.
        let to = if success {
            from.saturating_add(1).min(MAX_STAR)
        } else {
            // The first star is the floor: a failure below it costs the wager
            // and nothing else, so a piece is never walked back to nothing.
            from.saturating_sub(1).max(from.min(1))
        };
        if let Some(owned) = self
            .document
            .equipment
            .iter_mut()
            .find(|owned| owned.id == item_id)
        {
            owned.star = to;
        }
        let outcome = StarOutcome {
            item_id,
            name: item.name.clone(),
            from,
            to,
            success,
            cost,
        };
        self.star_attempts = self.star_attempts.saturating_add(1);
        self.last_star = Some(outcome.clone());
        self.push_event(
            now_unix,
            EventDetail::Star {
                name: item.name.unwrap_or_default(),
                star: to,
                success,
                cost,
            },
        );
        Ok(outcome)
    }

    /// Mark the day's morning event as seen, so the window does not replay it.
    ///
    /// A moment the surface owns: the popup is dismissed once and the day is
    /// written down, which is what lets a window opened again the same
    /// afternoon stay quiet about a fight that was already shown.
    pub fn acknowledge_daily_event(&mut self, day: i64) {
        self.document.daily_event_seen_day = Some(day);
    }

    /// Fight the guardian standing at the end of the current realm.
    ///
    /// Returns `None` when there is nothing to fight: the realm is not at its
    /// last layer, or the ladder has no next realm. A loss costs nothing but
    /// the attempt; a win opens the next realm, lifts the growth to that
    /// realm's floor and pays one piece of the new tier's gear.
    pub fn breakthrough(&mut self, now_unix: i64) -> Option<BreakthroughOutcome> {
        let current = self.realm();
        if self.level() < current.cap_level() {
            return None;
        }
        if !breakthrough_ready(self.document.breakthrough_blocked_until, now_unix) {
            return None;
        }
        let guardian = guardian_for(current)?;
        // The count is taken here, where a fight is actually joined: it is what
        // tells a surface there is a report it has not shown yet, and "the
        // snapshot revision moved" cannot say that — a running pomodoro bumps
        // it every second.
        self.breakthrough_attempts = self.breakthrough_attempts.saturating_add(1);
        let mut rng = self.rng(now_unix);
        let report = resolve(self.attributes(), guardian);
        // The fight is what decides, not the number above: the panel prints the
        // comparison so the user knows where they stand, and then the two of
        // them actually trade blows.
        let fighter = self.fighter(&mut rng);
        // The guardian is the same opponent every time it is met: its power is
        // printed before the fight, so a roll per attempt would make that number
        // a lie. What it wears is what makes it harder than the naked rung, and
        // that gear is what a win takes.
        let rival = opponent::guardian(current, guardian.target);
        let duel = self.duel_with_effects(fighter, rival.fighter.clone(), &mut rng);
        // Won fights are what train the 战斗 dimension; a loss is not counted
        // there, and only ever shows up in the record below.
        self.record_duel(duel.won, FightKind::Guardian, now_unix);
        if !duel.won {
            self.document.breakthrough_failures =
                self.document.breakthrough_failures.saturating_add(1);
            self.document.breakthrough_blocked_until =
                Some(now_unix.saturating_add(BREAKTHROUGH_COOLDOWN_SECONDS));
            // A loss can cost a piece of gear. One in four, and only one: the
            // point is that a loss stings, not that it sets the cultivator back.
            let lost = (rng.below(1_000) < LOST_GEAR_PERMILLE)
                .then(|| self.drop_one_item(&mut rng))
                .flatten();
            let outcome = BreakthroughOutcome {
                report,
                target: guardian.target,
                reward: None,
                duel,
                lost,
                granted_art: None,
                stones: None,
                seed: None,
            };
            self.last_breakthrough = Some(outcome.clone());
            // The realm it failed to open is still named: a loss is history
            // too, and one line with the realm on it beats two kinds of line.
            self.push_event(
                now_unix,
                EventDetail::Breakthrough {
                    won: false,
                    stage: Some(guardian.target.index() as u8),
                    stones: None,
                    gear: None,
                    art: None,
                    seed: None,
                },
            );
            return Some(outcome);
        }
        // The guardian is the strongest thing on the page, so a win here is
        // where the pill's odds are best — and it is rolled before the realm
        // opens, so the newest line in the log is the find rather than the
        // breakthrough it came with.
        self.roll_marrow_from_combat(guardian.power, &mut rng, now_unix);
        self.enter_realm(guardian, &rival, duel, now_unix);
        let outcome = self.last_breakthrough.clone();
        // The log is written after the settlement so the line carries the
        // realm that was opened *and* everything that came with it — purse,
        // 功法, gear and herb: the event is one line about one event, not two
        // half-lines about a fight and a buy. A win that left its finds out of
        // the log left a bag the user could not check against it.
        self.push_event(
            now_unix,
            EventDetail::Breakthrough {
                won: true,
                stage: Some(guardian.target.index() as u8),
                stones: outcome.as_ref().and_then(|outcome| outcome.stones),
                gear: outcome
                    .as_ref()
                    .and_then(|outcome| outcome.reward.as_ref())
                    .and_then(|item| item.name.clone()),
                art: outcome
                    .as_ref()
                    .and_then(|outcome| outcome.granted_art.clone()),
                seed: outcome.as_ref().and_then(|outcome| outcome.seed.clone()),
            },
        );
        outcome
    }

    /// Roll one 洗髓丹 after a won fight.
    ///
    /// The odds climb with the opponent's power relative to the cultivator's —
    /// one in a hundred thousand at parity, one in a thousand against something
    /// three times as strong — so the pill is the reward for punching up rather
    /// than for farming the easiest thing on the page.
    fn roll_marrow_from_combat(&mut self, rival_power: u32, rng: &mut Rng, now_unix: i64) -> bool {
        let player = self.power();
        let ppm = super::aptitude::marrow_combat_ppm(rival_power, player);
        if rng.below(1_000_000) >= ppm {
            return false;
        }
        self.document.marrow_pills = self.document.marrow_pills.saturating_add(1);
        self.push_event(
            now_unix,
            EventDetail::MarrowFound {
                source: MarrowSource::Combat,
            },
        );
        true
    }

    /// Roll one 洗髓丹 out of a quiet source: a walk that met nobody, or the
    /// day's own event. One in ten thousand, the same number the shelf uses.
    fn roll_marrow_from_fortune(&mut self, rng: &mut Rng, now_unix: i64) -> bool {
        if rng.below(super::aptitude::MARROW_FORTUNE_ONE_IN) != 0 {
            return false;
        }
        self.document.marrow_pills = self.document.marrow_pills.saturating_add(1);
        self.push_event(
            now_unix,
            EventDetail::MarrowFound {
                source: MarrowSource::Fortune,
            },
        );
        true
    }

    /// Write one fought fight into the record, newest first.
    /// A win is also what trains the 战斗 dimension, so the count and the line of
    /// history are written together: a record that disagreed with the dimension
    /// would be two truths about the same fight.
    fn record_duel(&mut self, won: bool, kind: FightKind, now_unix: i64) {
        if won {
            self.document.battle_wins = self.document.battle_wins.saturating_add(1);
        }
        self.document.duel_record.insert(
            0,
            DuelRecord {
                won,
                kind,
                realm: self.realm().index() as u8,
                at_unix: now_unix,
            },
        );
        self.document.duel_record.truncate(DUEL_RECORD_CAP);
    }

    /// Write one event into the page's log, newest first.
    ///
    /// The log is a window like the duel record: sixteen lines is what a page
    /// reads at a glance, and a save that kept every event since the first
    /// pomodoro would be a diary nobody opens.
    fn push_event(&mut self, now_unix: i64, detail: EventDetail) {
        self.document.events.insert(
            0,
            EventRecord {
                at_unix: now_unix,
                detail,
            },
        );
        self.document.events.truncate(EVENT_LOG_CAP);
    }

    /// The multiplier every finished session is currently worth, in parts per
    /// thousand.
    ///
    /// Read from the four disciplines rather than carried, so the page and the
    /// formula cannot disagree about what a session pays.
    fn growth_multiplier_permille(&self) -> u32 {
        growth_permille(
            self.realm(),
            self.trained_art_rank(),
            self.document.explorations,
            self.document.battle_wins,
            self.document.harvests,
        )
    }

    /// The cultivator as a fighter, with the 外功 they would use.
    pub(crate) fn fighter(&self, rng: &mut Rng) -> Fighter {
        let derived = self.derived();
        let art = self.strike(rng);
        Fighter::from_derived(PLAYER_NAME.to_owned(), derived, art)
    }

    /// The duel the cultivator fights, with the effects their 神装 carry.
    ///
    /// An effect is a debuff the other side walks in with, read when the duel
    /// is built: the number the panel shows the user before they press the
    /// button has to be the number that decides, and a hidden post-fight
    /// adjustment would make the shown numbers a lie about the fight that
    /// happened.
    pub(crate) fn duel_with_effects(
        &self,
        fighter: Fighter,
        mut rival: Fighter,
        rng: &mut Rng,
    ) -> DuelReport {
        if let Some((effect, rarity)) = self.strongest_effect() {
            let depth = effect.depth_permille(rarity);
            let bite = |value: u32| -> u32 {
                (u64::from(value) * u64::from(1_000 - depth) / 1_000) as u32
            };
            match effect {
                GearEffect::Frost => {
                    rival.attack = bite(rival.attack);
                    rival.hit = bite(rival.hit);
                }
                GearEffect::Thunder => rival.speed = bite(rival.speed),
                GearEffect::Sunder => rival.resist = bite(rival.resist),
                GearEffect::Weaken => rival.health = bite(rival.health),
            }
        }
        duel(fighter, rival, rng)
    }

    /// The effect the worn gear carries, if any: the deepest one, and the
    /// quality it came from.
    fn strongest_effect(&self) -> Option<(GearEffect, Rarity)> {
        self.equipped_items()
            .filter_map(|item| item.effect.map(|effect| (effect, item.rarity)))
            .max_by_key(|(_, rarity)| *rarity)
    }

    /// The 外功 the cultivator brings to a fight: randomly one of the three
    /// strongest they can swing.
    ///
    /// "Strongest" is what one swing is worth: the multiplier the 外功 lands,
    /// with the mastery share folded in for one learned to the end. A book
    /// still part-trained competes on its multiplier alone, and one that has
    /// not been trained to its first 阶 does not compete at all — owning a
    /// book is not swinging it. The training's selection does not pre-empt the
    /// roll either: the strongest three fight it out, not the book the session
    /// happens to be reading. With fewer than three the roll runs over what
    /// there is, and with none the fists do the work.
    fn strike(&self, rng: &mut Rng) -> Option<ArtStrike> {
        let mut candidates: Vec<(u64, u8, u32)> = self
            .document
            .arts
            .iter()
            .filter(|owned| blueprint(owned.art_id).kind == ArtKind::External && owned.rank >= 1)
            .map(|owned| {
                let art = blueprint(owned.art_id);
                (owned.art_id, owned.rank, art.multiplier_permille)
            })
            .collect();
        for id in &self.document.mastered_arts {
            let art = blueprint(*id);
            if art.kind == ArtKind::External {
                candidates.push((
                    *id,
                    art.grade.ranks(),
                    art.multiplier_permille + mastery_permille(art.grade),
                ));
            }
        }
        if candidates.is_empty() {
            return None;
        }
        candidates.sort_by_key(|&(_, _, strength)| std::cmp::Reverse(strength));
        candidates.truncate(3);
        let (art_id, rank, _) = candidates[rng.below(candidates.len() as u64) as usize];
        let art = blueprint(art_id);
        Some(ArtStrike {
            name: art.name,
            multiplier_permille: art.multiplier_permille,
            cost: art.cost,
            // A completed 外功 spends its mastery share on every blow it lands;
            // an art one 阶 short of its last pays only its multiplier.
            mastery_permille: if rank >= art.grade.ranks() {
                mastery_permille(art.grade)
            } else {
                0
            },
        })
    }

    /// Give up one owned item and say which. Equipped or not, one item only —
    /// and the cultivator is never left with nothing they can wear.
    fn drop_one_item(&mut self, rng: &mut Rng) -> Option<String> {
        if self.document.equipment.is_empty() {
            return None;
        }
        let index = rng.below(self.document.equipment.len() as u64) as usize;
        let item = self.document.equipment.remove(index);
        let name = item
            .name
            .clone()
            .unwrap_or_else(|| item.slot.name_key().to_owned());
        self.document
            .equipped
            .iter_mut()
            .filter(|slot| **slot == Some(item.id))
            .for_each(|slot| *slot = None);
        Some(name)
    }

    /// Open the realm the guardian defended and pay its trophy.
    fn enter_realm(
        &mut self,
        guardian: Guardian,
        rival: &Opponent,
        duel: DuelReport,
        now_unix: i64,
    ) {
        let target = guardian.target;
        // The power that fought, read before anything moves: entering the realm
        // lifts the level ceiling and pays a trophy, so a report read afterwards
        // would credit the win to the reward rather than to the gear.
        let player_power = self.power();
        self.document.realm = target.index() as u8;
        self.document.growth = self.document.growth.max(stage_floor(target.index()));
        self.document.breakthrough_failures = 0;
        // The realm lifts the level ceiling, so the layers the growth already
        // bought become real levels — and real free points — now.
        self.note_peak_level();
        let mut rng = self.rng(0);
        // The guardian's own strength, not the realm it guarded: what a win pays
        // is read off the body that was beaten, which is why a guardian met at
        // its realm's cap pays at that realm's depth.
        let strength = Strength::of(rival.attributes.power());
        // The trophy is the piece the guardian was wearing, not a fresh roll:
        // what the fight was against and what it leaves behind are one thing.
        let reward = match rival.gear.first() {
            Some(item) => Some(self.adopt_equipment(item.clone())),
            // A guardian met bare-handed (it can happen at the shallow end,
            // where a piece of gear would outweigh the whole body) still pays
            // for the realm it opened.
            None => self.push_equipment(strength, &mut rng),
        };
        // A win always pays a 功法 — that is the promise the fight is made for —
        // and it is the book the guardian was swinging.
        let granted_art = self.take_art(rival, target, now_unix);
        // The herb is named here rather than only folded into the bag: it is the
        // rarest thing a realm hands over, and a log that stayed silent about it
        // left the user finding a seed they could not account for.
        let mut seed = None;
        if rng.below(1_000) < VICTORY_SEED_PERMILLE {
            let attribute = Attribute::from_index(rng.below(Attribute::ALL.len() as u64) as usize);
            let grade = roll_seed_grade(&mut rng);
            seed = self
                .push_seed(attribute, strength.tier(), grade, &mut rng)
                .name;
        }
        // The guardian's purse, drawn after the 功法 and the gear because those
        // are what a breakthrough is *for*: the money is what a realm's treasury
        // adds on top, not the reason to fight.
        let stones = Some(roll_stones(
            &mut rng,
            GUARDIAN_STONES,
            target.index() as u8 + 1,
        ));
        if let Some(amount) = stones {
            self.document.stones = self.document.stones.saturating_add(amount);
        }
        let outcome = BreakthroughOutcome {
            report: BattleReport {
                player_power,
                guardian_power: guardian.power,
                won: true,
            },
            target,
            reward,
            duel,
            lost: None,
            granted_art,
            stones,
            seed,
        };
        self.last_breakthrough = Some(outcome);
    }

    /// What beating someone on the road pays, handed to the satchel and
    /// reported.
    ///
    /// Shared by the walk and the morning ambush because it is the same
    /// encounter either way: the same weighting, the same strength, the same
    /// three kinds. Two copies of this roll would be two chances for one of them
    /// to drift out of tune with the panel that explains it.
    fn award_wanderer_find(
        &mut self,
        rival: &Opponent,
        rng: &mut Rng,
        now_unix: i64,
    ) -> ExplorationFindView {
        let mut found = ExplorationFindView::default();
        // A beaten opponent is the source, so what it hands over is read off its
        // own strength rather than off the realm the fight happened in: beating
        // something bigger than the save is how a save gets gear from deeper in.
        let strength = Strength::of(rival.attributes.power());
        match roll_victory(rng) {
            // What the roll decides is *which* of the beaten fighter's things
            // comes home with you — the things themselves were never a fresh
            // roll, they were on the body that lost.
            ExplorationOutcome::Art => {
                found.art = self.take_art(rival, self.realm(), now_unix);
            }
            ExplorationOutcome::Gear => {
                if let Some(item) = self.take_gear(rival, strength, rng) {
                    found.gear = Some(self.item_view(&item));
                }
            }
            ExplorationOutcome::Seed => {
                let attribute =
                    Attribute::from_index(rng.below(Attribute::ALL.len() as u64) as usize);
                let grade = roll_seed_grade(rng);
                found.seed = Some(self.push_seed(attribute, strength.tier(), grade, rng));
            }
            _ => {}
        }
        found
    }

    /// Set out on one walk, for the length chosen.
    ///
    /// Nothing is paid here: the road pays when the walk comes home, which is
    /// [`Self::settle_walk`] and the worker's tick. Returns the walk that
    /// started, and `None` while another is still out — with the refusal noted
    /// so the page can say why. A walk whose time has already passed is settled
    /// first, so a save that was never ticked (a launch after a night away) can
    /// still set out the moment it is opened.
    pub fn explore(
        &mut self,
        now_unix: i64,
        hours: super::ExplorationHours,
    ) -> Option<super::WalkInProgress> {
        self.settle_walk(now_unix);
        if self.document.walk.is_some() {
            self.note_refusal(Refusal::ExploreCooldown);
            return None;
        }
        // A job holds the cultivator the way a walk does — it is hours of the
        // same day — so the road is closed while one is being worked.
        if self.document.work.is_some() {
            self.note_refusal(Refusal::Working);
            return None;
        }
        let walk = super::WalkInProgress::started(now_unix, hours);
        self.document.last_exploration_unix = Some(now_unix);
        // The length is written down beside the instant, because the page draws
        // the chip the user picked last: a four-hour walk read back as a
        // twelve-hour one would draw the wrong chip under the right countdown.
        self.document.last_exploration_hours = hours.hours();
        self.document.walk = Some(walk);
        Some(walk)
    }

    /// Call the walk home early.
    ///
    /// The find is forfeit — the road is not walked, so there is nothing on it —
    /// and the return trip is a third of the time already spent, so calling a
    /// walk back is a way to stop paying attention to it rather than a way to
    /// undo it.
    pub fn return_early(&mut self, now_unix: i64) -> bool {
        let Some(walk) = self.document.walk else {
            self.note_refusal(Refusal::ExploreCooldown);
            return false;
        };
        if walk.early {
            return false;
        }
        self.document.walk = Some(walk.called_home(now_unix));
        true
    }

    /// Settle the walk that is home: roll what it turned up, pay it, and let the
    /// cultivator be at home again.
    ///
    /// Returns whether anything was settled. A walk still out answers `false`
    /// and nothing moves; a walk called home comes back empty, because a called
    /// walk is a walk that was not taken.
    pub fn settle_walk(&mut self, now_unix: i64) -> bool {
        let Some(walk) = self.document.walk else {
            return false;
        };
        if !walk.is_home(now_unix) {
            return false;
        }
        self.document.walk = None;
        if walk.early {
            // Called home: no find, no purse, no record. The walk did not
            // happen, which is exactly what it pays for having been called.
            self.document.last_exploration_unix = Some(now_unix);
            return true;
        }
        let hours = walk.length();
        self.document.last_exploration_unix = Some(now_unix);
        // Counted here rather than on the outcome: a walk that found nothing is
        // still a walk a surface has not shown yet.
        self.document.explorations = self.document.explorations.saturating_add(1);
        // The fight the last walk met is forgotten before this one is settled:
        // a walk that meets nobody must carry no fight into its popup, or the
        // popup replays an older walk's battle as this walk's verdict — which
        // is how a quiet afternoon that turned up a handful of stones came to
        // be announced as 败北 under a won fight's log.
        self.last_exploration_duel = None;
        let mut rng = self.rng(now_unix);
        // The length this walk was taken at is the multiplier its chances are
        // read through — the one lever the user pulled.
        let mut outcome = roll_walk(&mut rng, hours);
        // A walk is the quiet half of the ladder, so its source is the walker:
        // what the road turns up is what the cultivator is worth, and a save that
        // has climbed finds deeper goods on the same road.
        let strength = Strength::of(self.power());
        let mut found = ExplorationFindView::default();
        match outcome {
            ExplorationOutcome::Seed => {
                let attribute =
                    Attribute::from_index(rng.below(Attribute::ALL.len() as u64) as usize);
                let grade = roll_seed_grade(&mut rng);
                found.seed = Some(self.push_seed(attribute, strength.tier(), grade, &mut rng));
            }
            ExplorationOutcome::Gear => {
                if let Some(item) = self.push_equipment(strength, &mut rng) {
                    found.gear = Some(self.item_view(&item));
                }
            }
            ExplorationOutcome::Art => {
                // Which grade turns up is the discovery roll, not the realm's
                // band: a walk can hand a beginner a 神阶 book, at the same
                // ten-thousandth the shelf and the guardian use.
                let grade = {
                    let mut grade_rng = self.rng(now_unix);
                    super::arts::roll_art_grade(&mut grade_rng)
                };
                found.art = self.grant_art(grade, now_unix).map(|art| art.name);
            }
            ExplorationOutcome::Victory => {
                let fighter = self.fighter(&mut rng);
                // A wanderer is rolled per meeting: nobody promised who the road
                // would send, and its numbers are only ever shown after the fact.
                let rival = opponent::wanderer(self.realm(), rng.below(u64::MAX));
                let duel = self.duel_with_effects(fighter, rival.fighter.clone(), &mut rng);
                if duel.won {
                    found = self.award_wanderer_find(&rival, &mut rng, now_unix);
                    // A beaten wanderer can leave a 洗髓丹 behind: the odds are
                    // read off how far it outclassed the cultivator.
                    self.roll_marrow_from_combat(rival.attributes.power(), &mut rng, now_unix);
                } else {
                    outcome = ExplorationOutcome::Defeat;
                }
                self.record_duel(duel.won, FightKind::Wanderer, now_unix);
                self.last_exploration_duel = Some(duel);
            }
            ExplorationOutcome::Defeat
            | ExplorationOutcome::Nothing
            | ExplorationOutcome::Stones => {}
        }
        // A kind that was rolled but paid nothing is a walk that found nothing:
        // the quality ladder's remainder is exactly the find that did not
        // happen, and a line saying "found gear" over an empty satchel would be
        // a promise the roll broke.
        if outcome == ExplorationOutcome::Gear && found.gear.is_none() {
            outcome = ExplorationOutcome::Nothing;
        }
        // A quiet walk is a random event like the morning's: one in ten thousand
        // turns up a 洗髓丹. A fight's own roll is above, and a loss finds
        // nothing — the pill is not a consolation prize.
        if !matches!(
            outcome,
            ExplorationOutcome::Victory | ExplorationOutcome::Defeat
        ) {
            self.roll_marrow_from_fortune(&mut rng, now_unix);
        }
        // 灵石 are rolled beside the finding rather than in place of it: an
        // outing can come back with a herb and a handful of stones both, and
        // the purse is fed by going out rather than only by working.
        //
        // Which purse is what tells the two outings apart: a quiet walk pays a
        // handful for the trouble of walking, and beating a wanderer pays what
        // a wanderer was carrying. A walk that was beaten pays nothing at all —
        // a purse that filled up on a defeat would make being beaten a way of
        // earning.
        let purse = if outcome == ExplorationOutcome::Victory {
            WANDERER_STONES
        } else {
            // The quiet road's purse is the one that follows the length: a
            // day's six short walks and its two long ones have to come to the
            // same money, or the shortest chip is simply the money button.
            WALK_STONES.scaled(hours.multiplier_permille())
        };
        let stones = if outcome == ExplorationOutcome::Defeat {
            None
        } else {
            // The purse is opened at the realm the walk was taken in: the same
            // road pays more once the road is a deeper one.
            Some(roll_stones(&mut rng, purse, self.realm().index() as u8 + 1))
        };
        if let Some(amount) = stones {
            self.document.stones = self.document.stones.saturating_add(amount);
            found.stones = Some(amount);
            // A walk that turned up stones and nothing else reads as having
            // turned up stones, not as having turned up nothing.
            if outcome == ExplorationOutcome::Nothing {
                outcome = ExplorationOutcome::Stones;
            }
        }
        // The settlement is written before the popup is shown and outlives it: the
        // popup is forgotten at quit, the page's record is not.
        self.document.last_walk = Some(WalkRecord {
            at_unix: now_unix,
            realm: self.realm().index() as u8,
            outcome,
            art: found.art.clone(),
            gear: found.gear.as_ref().and_then(|item| item.name.clone()),
            seed: found.seed.as_ref().and_then(|seed| seed.name.clone()),
            stones,
        });
        self.last_exploration = Some(outcome);
        self.last_exploration_found = Some(found);
        // The walk joins the page's log beside the popups: finds and all, so a
        // week of walks reads as a week rather than as one line of "last walk".
        self.push_event(
            now_unix,
            EventDetail::Walk {
                outcome,
                art: self.document.last_walk.as_ref().and_then(|w| w.art.clone()),
                gear: self
                    .document
                    .last_walk
                    .as_ref()
                    .and_then(|w| w.gear.clone()),
                seed: self
                    .document
                    .last_walk
                    .as_ref()
                    .and_then(|w| w.seed.clone()),
                stones,
            },
        );
        true
    }

    /// The board of jobs on offer, for a local day.
    ///
    /// The board is a function of the day and of the realm the cultivator is
    /// worth rather than a stored list: a board written three realms ago would be
    /// pocket change today, and a board a user never opened should not be the one
    /// they are handed next week. The counter is what a paid re-roll moves.
    pub fn jobs(&self, day: i64) -> Vec<super::JobOffer> {
        let tier = super::drop::tier_for_power(self.power());
        let mut rng = Rng::seeded(
            (day as u64)
                .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                .wrapping_add(
                    u64::from(self.document.jobs_roll).wrapping_mul(0xBF58_476D_1CE4_E5B9),
                ),
        );
        super::jobs::roll_jobs(&mut rng, tier)
    }

    /// Take the job in that slot of the board.
    ///
    /// Refused while the cultivator is out on a walk or already at a job: both are
    /// hours of the same day, and a cultivator who can be in two places at once
    /// has no reason to choose between them.
    pub fn accept_job(
        &mut self,
        slot: usize,
        now_unix: i64,
        day: i64,
    ) -> Result<super::JobRun, Refusal> {
        if self.document.walk.is_some() {
            return Err(Refusal::TrialOut);
        }
        if self.document.work.is_some() {
            return Err(Refusal::Working);
        }
        let Some(offer) = self.jobs(day).get(slot).copied() else {
            return Err(Refusal::JobGone);
        };
        let run = super::JobRun::started(offer, now_unix);
        self.document.work = Some(run);
        Ok(run)
    }

    /// Pay for a fresh board.
    ///
    /// The price is what stops the board being a slot machine: three draws at a
    /// ten-thousandth, re-rolled until a 神阶 job turns up, would cost nothing at
    /// all if this were free.
    pub fn refresh_jobs(&mut self) -> Result<u32, TradeRefusal> {
        let tier = super::drop::tier_for_power(self.power());
        let cost = super::jobs::refresh_cost(tier);
        if self.document.stones < cost {
            return Err(TradeRefusal::NotEnoughStones);
        }
        self.document.stones = self.document.stones.saturating_sub(cost);
        self.document.jobs_roll = self.document.jobs_roll.saturating_add(1);
        Ok(cost)
    }

    /// Settle a job that has come due. Returns what it paid.
    ///
    /// Called by the worker's tick the way a walk's return is: a job is hours of
    /// the cultivator's day, so it comes home on the clock rather than on a click,
    /// and an app that was shut for the whole of it has to pay on the next launch
    /// rather than lose the hours.
    pub fn settle_work(&mut self, now_unix: i64) -> Option<WorkPaid> {
        let run = self.document.work?;
        if !run.is_done(now_unix) {
            return None;
        }
        self.document.work = None;
        // The wage is the figure the board printed; 潜质 is the only thing that
        // moves it, and it is rolled once, here, off the cultivator's own.
        let wage = run.offer().stones();
        let mut rng = self.rng(now_unix);
        let multiplier = super::jobs::crit_multiplier(&mut rng, self.document.aptitude_tenths);
        let stones = wage.saturating_mul(multiplier);
        self.document.stones = self.document.stones.saturating_add(stones);
        // A finished job hands over a new board: the outing is over, and the next
        // three are what the cultivator walks back into.
        self.document.jobs_roll = self.document.jobs_roll.saturating_add(1);
        let paid = WorkPaid {
            stones,
            hours: run.hours,
            grade: run.grade,
            multiplier,
        };
        self.last_work = Some(paid);
        self.push_event(
            now_unix,
            EventDetail::Work {
                stones,
                hours: run.hours,
                grade: run.grade,
                multiplier,
            },
        );
        Some(paid)
    }

    /// Whether the cultivator is at home right now: no walk is under way.
    ///
    /// The walk is its own cooldown — the road pays when it comes home — so
    /// being home is the whole gate. A walk whose time has passed is settled by
    /// the worker's tick, and by [`Self::explore`] on the way in, so this never
    /// says "out" about a walk that is already back.
    pub fn exploration_ready(&self, now_unix: i64) -> bool {
        match self.document.walk {
            None => true,
            // A walk whose time is up is home whether or not the tick has come
            // round to settle it yet: a page that said "still out" about a walk
            // that is back would be reading the clock wrong, not the walk.
            Some(walk) => walk.is_home(now_unix),
        }
    }

    /// The walk under way, if the cultivator is out.
    pub fn walk(&self) -> Option<super::WalkInProgress> {
        self.document.walk
    }

    /// The instant the walk under way comes home, while one is out.
    pub fn walk_home_at(&self) -> Option<i64> {
        self.document.walk.map(|walk| walk.ends_unix)
    }

    /// The floor the next attempt climbs.
    pub fn trial_next_floor(&self) -> u16 {
        self.document.trial_floor.saturating_add(1)
    }

    /// How many floors the local day still offers.
    ///
    /// Reset by the day rather than by elapsed time: five floors belong to a
    /// date the way the shelf does, so a save that has not played since
    /// yesterday is owed five today whatever the hour.
    pub fn trial_attempts_left(&self, day: i64) -> u32 {
        if self.document.trial_day != Some(day) {
            return TRIAL_ATTEMPTS_PER_DAY;
        }
        TRIAL_ATTEMPTS_PER_DAY.saturating_sub(self.document.trial_attempts_today)
    }

    /// Whether a job in hand is still holding the cultivator.
    ///
    /// The same reading the road applies to a walk: an outing whose hours have
    /// passed is over whether or not the tick has come round to settle it, so a
    /// fight refused because of a job that is already done would be reading the
    /// clock wrong. The tick pays the wage; this only says where the
    /// cultivator is, and the tower and the 试炼 ask it the same question.
    fn working(&self, now_unix: i64) -> bool {
        self.document
            .work
            .is_some_and(|work| !work.is_done(now_unix))
    }

    /// Why the tower is closed right now, or `None` when a floor can be climbed.
    ///
    /// Four different walls, and the page prints which one it hit: out on a
    /// walk, at a job, out of the day's attempts, or past the last floor. A
    /// button that went quiet without saying which would read as broken.
    pub fn trial_blocked(&self, now_unix: i64, day: i64) -> Option<TrialBlocked> {
        if self.trial_next_floor() > TRIAL_MAX_FLOOR {
            return Some(TrialBlocked::Top);
        }
        if !self.exploration_ready(now_unix) {
            return Some(TrialBlocked::Out);
        }
        if self.working(now_unix) {
            return Some(TrialBlocked::Working);
        }
        if self.trial_attempts_left(day) == 0 {
            return Some(TrialBlocked::Drained);
        }
        None
    }

    /// Why the 试炼 cannot be fought right now, or `None` when it can.
    ///
    /// The two home fights ask the same question in the same order, so the page
    /// and the domain cannot disagree about the reason: still on the road, then
    /// still at work, then out of chances.
    pub fn proving_blocked(&self, now_unix: i64) -> Option<ProvingBlocked> {
        if !self.exploration_ready(now_unix) {
            return Some(ProvingBlocked::Out);
        }
        if self.working(now_unix) {
            return Some(ProvingBlocked::Working);
        }
        if self.document.proving_tokens == 0 {
            return Some(ProvingBlocked::Spent);
        }
        None
    }

    /// Climb one floor of the 试炼 tower.
    ///
    /// The gate is the day's attempts and the cultivator's whereabouts, not a
    /// cost: a floor is refused while a walk is still out — the tower is climbed
    /// at home — and while the day's five are spent. A win clears the floor,
    /// moves the frontier up one and rolls what it turned up; a loss costs
    /// nothing but the attempt, and leaves the frontier where it stood, so the
    /// tower is a wall to be worn down rather than a ladder that drops.
    pub fn trial(&mut self, now_unix: i64, day: i64) -> Option<TrialOutcome> {
        // A walk that is home is settled first: the courtyard is closed to a
        // walk that is *out*, not to one whose find has not been read yet. The
        // same for the job in hand — the fight waits for the road, not for the
        // tick that pays the wage.
        self.settle_walk(now_unix);
        self.settle_work(now_unix);
        if let Some(blocked) = self.trial_blocked(now_unix, day) {
            self.note_refusal(blocked.refusal());
            return None;
        }
        // The day is stamped on the first floor of it, so the count the page
        // reads and the count the next attempt spends cannot disagree.
        if self.document.trial_day != Some(day) {
            self.document.trial_day = Some(day);
            self.document.trial_attempts_today = 0;
        }
        let floor = self.trial_next_floor();
        self.document.trial_attempts_today = self.document.trial_attempts_today.saturating_add(1);
        self.document.trials = self.document.trials.saturating_add(1);
        self.trial_attempts = self.trial_attempts.saturating_add(1);
        let mut rng = self.rng(now_unix);
        let fighter = self.fighter(&mut rng);
        // A floor's warden is the same opponent for that floor every day: the
        // page prints its power next to the floor, so it cannot be a roll.
        let rival = opponent::warden(floor);
        let duel = self.duel_with_effects(fighter, rival.fighter.clone(), &mut rng);
        self.record_duel(duel.won, FightKind::Trial, now_unix);
        let mut found = ExplorationFindView::default();
        if duel.won {
            self.document.trial_floor = floor.min(TRIAL_MAX_FLOOR);
            // The warden is the source, so what the floor pays is read off the
            // body that defended it: a floor reached past the save's own realm
            // hands over that realm's goods, which is how the tower is a way
            // down the ladder rather than a second one.
            let strength = Strength::of(rival.attributes.power());
            match roll_reward(&mut rng) {
                TrialReward::Seed => {
                    let attribute =
                        Attribute::from_index(rng.below(Attribute::ALL.len() as u64) as usize);
                    let grade = roll_seed_grade(&mut rng);
                    found.seed = Some(self.push_seed(attribute, strength.tier(), grade, &mut rng));
                }
                TrialReward::Gear => {
                    if let Some(item) = self.take_gear(&rival, strength, &mut rng) {
                        found.gear = Some(self.item_view(&item));
                    }
                }
                TrialReward::Art => {
                    found.art = self.take_art(&rival, floor_realm(floor), now_unix);
                }
                TrialReward::Nothing => {}
            }
            // The floor's purse, drawn beside the finding: a cleared floor
            // always pays a little, so a run of empty rolls is still a run that
            // bought something.
            // Opened at the floor's own realm, so a deeper floor pays more even
            // within one realm's climb.
            if let Some(amount) = Some(roll_stones(
                &mut rng,
                TRIAL_STONES,
                floor_realm(floor).index() as u8 + 1,
            )) {
                self.document.stones = self.document.stones.saturating_add(amount);
                found.stones = Some(amount);
            }
        }
        self.push_event(
            now_unix,
            EventDetail::Trial {
                floor,
                won: duel.won,
                art: found.art.clone(),
                gear: found.gear.as_ref().and_then(|item| item.name.clone()),
                seed: found.seed.as_ref().and_then(|seed| seed.name.clone()),
                stones: found.stones,
            },
        );
        let outcome = TrialOutcome { floor, duel, found };
        self.last_trial = Some(outcome.clone());
        Some(outcome)
    }

    /// Spend one 试炼 chance on a fight at the cultivator's own level.
    ///
    /// The courtyard is fought at home, so the same two walls the tower has
    /// close it too: a walk still on the road and a job still in hand are hours
    /// of the same day, and the cultivator is only in one place at a time. What
    /// this fight adds is its own ration — a banked chance, one per finished
    /// pomodoro, ten at most — and the opponent is rolled per meeting at the
    /// cultivator's own level: a 试炼 is a fight, not a wall. A win pays the
    /// 锁妖塔's base table, so the two dungeons differ in when they pay, not
    /// what.
    pub fn proving(&mut self, now_unix: i64) -> Option<ProvingOutcome> {
        // The walk that is home is settled before the courtyard is asked, the
        // way the tower does it: the gate is where the cultivator is, not
        // whether the tick has come round.
        self.settle_walk(now_unix);
        self.settle_work(now_unix);
        if let Some(blocked) = self.proving_blocked(now_unix) {
            self.note_refusal(blocked.refusal());
            return None;
        }
        self.document.proving_tokens = self.document.proving_tokens.saturating_sub(1);
        self.provings = self.provings.saturating_add(1);
        let mut rng = self.rng(now_unix);
        let fighter = self.fighter(&mut rng);
        let tier = self.realm().index() as u8 + 1;
        let rival = opponent::challenger(self.level(), tier, rng.below(u64::MAX));
        let duel = self.duel_with_effects(fighter, rival.fighter.clone(), &mut rng);
        self.record_duel(duel.won, FightKind::Wanderer, now_unix);
        let mut found = ExplorationFindView::default();
        if duel.won {
            let strength = Strength::of(rival.attributes.power());
            match roll_reward(&mut rng) {
                TrialReward::Seed => {
                    let attribute =
                        Attribute::from_index(rng.below(Attribute::ALL.len() as u64) as usize);
                    let grade = roll_seed_grade(&mut rng);
                    found.seed = Some(self.push_seed(attribute, strength.tier(), grade, &mut rng));
                }
                TrialReward::Gear => {
                    if let Some(item) = self.take_gear(&rival, strength, &mut rng) {
                        found.gear = Some(self.item_view(&item));
                    }
                }
                TrialReward::Art => {
                    found.art = self.take_art(&rival, self.realm(), now_unix);
                }
                TrialReward::Nothing => {}
            }
            // Every fight pays a purse of its own size (ADR-0096), and the
            // courtyard was the one that did not: its only reward was the
            // exclusive roll above, which comes up empty most of the time, so a
            // clean win could pay the cultivator nothing at all. The purse is
            // drawn at the realm the bout was fought in, like every other one.
            let stones = roll_stones(&mut rng, PROVING_STONES, tier);
            self.document.stones = self.document.stones.saturating_add(stones);
            found.stones = Some(stones);
            // A beaten challenger can leave a 洗髓丹 behind, on the same odds
            // every other fight rolls: the pill is not a tower's privilege.
            self.roll_marrow_from_combat(rival.attributes.power(), &mut rng, now_unix);
        }
        self.push_event(
            now_unix,
            EventDetail::Proving {
                won: duel.won,
                art: found.art.clone(),
                gear: found.gear.as_ref().and_then(|item| item.name.clone()),
                seed: found.seed.as_ref().and_then(|seed| seed.name.clone()),
                stones: found.stones,
            },
        );
        let outcome = ProvingOutcome { duel, found };
        self.last_proving = Some(outcome.clone());
        Some(outcome)
    }

    /// What the day opened with, once per local day.
    ///
    /// Called by the worker's tick, not by a click: the day has to arrive on its
    /// own, whether the machine was launched this morning or left running over
    /// midnight. `None` on every tick after the first of a day, so the trick is
    /// idempotent without the caller having to remember anything.
    ///
    /// An ambush is a real fight — the wanderer off the road, the same engine and
    /// the same record — and a gift or a coin is paid straight into the satchel.
    /// Nothing is ever taken away: the morning can only add.
    pub fn daily_event(&mut self, now_unix: i64, day: i64) -> Option<DailyOutcome> {
        if self.document.daily_event_day == Some(day) {
            return None;
        }
        self.document.daily_event_day = Some(day);
        self.daily_events = self.daily_events.saturating_add(1);
        let mut rng = self.rng(now_unix);
        let kind = roll_kind(&mut rng);
        let mut outcome = DailyOutcome {
            kind,
            won: true,
            duel: None,
            found: ExplorationFindView::default(),
        };
        let tier = self.realm().index() as u8 + 1;
        match kind {
            DailyKind::Ambush => {
                let fighter = self.fighter(&mut rng);
                let rival = opponent::wanderer(self.realm(), rng.below(u64::MAX));
                let duel = self.duel_with_effects(fighter, rival.fighter.clone(), &mut rng);
                self.record_duel(duel.won, FightKind::Wanderer, now_unix);
                outcome.won = duel.won;
                if duel.won {
                    outcome.found = self.award_wanderer_find(&rival, &mut rng, now_unix);
                    // The ambush is a fight like any other, so it rolls the
                    // pill on the same odds: the stronger the ambusher, the
                    // better the chance it was carrying one.
                    self.roll_marrow_from_combat(rival.attributes.power(), &mut rng, now_unix);
                    let amount = roll_stones(&mut rng, DAILY_AMBUSH_STONES, tier);
                    self.document.stones = self.document.stones.saturating_add(amount);
                    outcome.found.stones = Some(amount);
                }
                outcome.duel = Some(duel);
            }
            DailyKind::Gift => match roll_gift(&mut rng) {
                GiftKind::Stones => {
                    let amount = roll_stones(&mut rng, DAILY_GIFT_STONES, tier);
                    self.document.stones = self.document.stones.saturating_add(amount);
                    outcome.found.stones = Some(amount);
                }
                GiftKind::Seed => {
                    let attribute =
                        Attribute::from_index(rng.below(Attribute::ALL.len() as u64) as usize);
                    // A gift is the quiet kind of source, so it is the
                    // cultivator's own strength that decides what the day hands
                    // over: nothing was fought for it.
                    let strength = Strength::of(self.power());
                    let grade = roll_seed_grade(&mut rng);
                    outcome.found.seed =
                        Some(self.push_seed(attribute, strength.tier(), grade, &mut rng));
                }
                GiftKind::Gear => {
                    let strength = Strength::of(self.power());
                    match self.push_equipment(strength, &mut rng) {
                        Some(item) => outcome.found.gear = Some(self.item_view(&item)),
                        // A gift always hands something over — that is what makes
                        // it a gift — so a wardrobe that rolled up empty becomes
                        // the purse it would have been worth.
                        None => {
                            let amount = roll_stones(&mut rng, DAILY_GIFT_STONES, tier);
                            self.document.stones = self.document.stones.saturating_add(amount);
                            outcome.found.stones = Some(amount);
                        }
                    }
                }
            },
            DailyKind::Trinket => {
                let amount = roll_stones(&mut rng, DAILY_TRINKET_STONES, tier);
                self.document.stones = self.document.stones.saturating_add(amount);
                outcome.found.stones = Some(amount);
            }
        }
        // A gift or a trinket is a random event, so it carries the same one in
        // ten thousand the shelf does. An ambush rolls its own odds above, and a
        // lost ambush finds nothing.
        if !matches!(kind, DailyKind::Ambush) {
            self.roll_marrow_from_fortune(&mut rng, now_unix);
        }
        let found = outcome.found.clone();
        self.push_event(
            now_unix,
            EventDetail::Daily {
                kind,
                won: outcome.won,
                art: found.art,
                gear: found.gear.as_ref().and_then(|item| item.name.clone()),
                seed: found.seed.as_ref().and_then(|seed| seed.name.clone()),
                stones: found.stones,
            },
        );
        self.last_daily_event = Some(outcome.clone());
        Some(outcome)
    }

    /// The guardian offered right now, or `None` while the realm still has
    /// layers to climb.
    pub fn guardian(&self) -> Option<Guardian> {
        let realm = self.realm();
        (self.level() >= realm.cap_level())
            .then(|| guardian_for(realm))
            .flatten()
    }

    /// Read the snapshot a surface renders, without a clock.
    ///
    /// Only the tests reach for this: a surface has a clock and asks for
    /// [`Self::view_on`], so the cooldowns it reads are the real ones.
    pub fn view(&self) -> CultivationView {
        self.view_on(0, 0)
    }

    /// Read the snapshot a surface renders.
    ///
    /// The local day is passed in rather than derived from the timestamp,
    /// because the day is a local one: the session counter resets on it, and
    /// the shelf is rolled from it. A surface that worked the day out for
    /// itself could be a timezone's worth of hours out — which would show
    /// yesterday's shelf and price the next pomodoro as the second of the day
    /// when it is really the first.
    pub fn view_on(&self, now_unix: i64, day: i64) -> CultivationView {
        let realm = self.realm();
        let level = self.level();
        let said = layer_rank(level);
        let floor = threshold_for(level);
        let next = (level < realm.cap_level()).then(|| threshold_for(level + 1));
        let level_attributes = self.level_attributes();
        let equipment_attributes = self.equipment_attributes();
        let crop_attributes = self.crop_attributes();
        let allocated_attributes = self.allocated_attributes();
        let art_attributes = self.art_attributes();
        let attributes = level_attributes
            .plus(equipment_attributes)
            .plus(crop_attributes)
            .plus(allocated_attributes)
            .plus(art_attributes);
        let bonus = self.equipment_bonus().plus(self.art_bonus());
        let attribute_derived = attributes.derived();
        // 潜质 is its own fractional number now, not a slice of 根骨: the
        // outright lines are folded into it by the one reader, so the rate a
        // row prints and the roll a session makes are the same value.
        let aptitude_tenths = self.aptitude_tenths();
        let derived = attribute_derived.plus(bonus);
        let arts: Vec<ArtView> = self
            .document
            .arts
            .iter()
            .map(|art| {
                art_view(
                    art,
                    aptitude_tenths,
                    self.document.selected_art == Some(art.art_id),
                )
            })
            .collect();
        let mut mastered_arts: Vec<MasteredArtView> = self
            .document
            .mastered_arts
            .iter()
            .map(|id| MasteredArtView {
                sold: self.document.sold_mastered_arts.contains(id),
                ..mastered_art_view(*id)
            })
            .collect();
        // A book trained to its first 阶 is 已习得 as far as it has gone: it
        // reports here with the 阶 it stands at and no mastery yet, while the
        // shelf keeps the training and the selling.
        for owned in &self.document.arts {
            if owned.rank == 0 {
                continue;
            }
            mastered_arts.push(MasteredArtView {
                rank: owned.rank,
                max_rank: blueprint(owned.art_id).grade.ranks(),
                complete: false,
                mastery_permille: 0,
                sold: false,
                ..mastered_art_view(owned.art_id)
            });
        }
        // Most recently trained first, the rest in the order the save met them:
        // the three on the page are the three the cultivator is working on, and
        // a stable sort leaves the older ones readable in the order they were
        // learned rather than in whatever order the two lists were merged.
        let recent = &self.document.recent_arts;
        mastered_arts.sort_by_key(|art| {
            recent
                .iter()
                .position(|id| *id == art.id)
                .unwrap_or(usize::MAX)
        });
        let mut equipped: [Option<ItemView>; SLOT_COUNT] = array::from_fn(|_| None);
        let mut inventory = Vec::new();
        for item in &self.document.equipment {
            let view = self.item_view(item);
            if view.equipped {
                equipped[item.slot.index()] = Some(view);
            } else {
                inventory.push(view);
            }
        }
        inventory.sort_by_key(|item| std::cmp::Reverse(item.id));
        let mut seeds: Vec<SeedView> = self.document.seeds.iter().map(seed_view).collect();
        seeds.sort_by(|left, right| {
            right
                .tier
                .cmp(&left.tier)
                .then_with(|| right.grade.cmp(&left.grade))
                .then_with(|| left.attribute.index().cmp(&right.attribute.index()))
        });
        CultivationView {
            level,
            realm,
            layer: said.layer,
            rank: said.rank,
            growth: self.document.growth,
            floor,
            next,
            attributes,
            derived,
            level_attributes,
            equipment_attributes,
            crop_attributes,
            allocated_attributes,
            art_attributes,
            free_points: self.free_points(),
            power: self.power(),
            power_breakdown: self.power_breakdown(),
            equipped,
            inventory,
            seeds,
            plots: self
                .document
                .plots
                .iter()
                .map(|plot| PlotView {
                    crop: plot.crop.as_ref().map(|crop| CropView {
                        attribute: crop.attribute,
                        tier: crop.tier,
                        grade: crop.grade,
                        name: crop.name.clone(),
                        ready_at_unix: crop.ready_at_unix,
                    }),
                })
                .collect(),
            // What the next plot would cost, and nothing once the garden is
            // full: the buy button reads this rather than recomputing the
            // table, so the number on it is the number the domain charges.
            next_plot_price: plot_price(self.document.plots.len()),
            garden_rank: self.document.garden_rank,
            garden_xp: self.document.garden_xp,
            garden_xp_needed: if self.document.garden_rank < MAX_GARDEN_RANK {
                rank_xp_total(self.document.garden_rank.saturating_add(1))
            } else {
                0
            },
            garden_upgrade_cost: (self.document.garden_rank < MAX_GARDEN_RANK)
                .then(|| rank_cost(self.document.garden_rank)),
            garden_pity_ready: self.document.plant_fail_streak >= PITY_STREAK,
            guardian: self.guardian().map(|guardian| GuardianView {
                target: guardian.target,
                power: guardian.power,
            }),
            last_breakthrough: self.last_breakthrough.clone(),
            last_absence: self.last_absence,
            last_work: self.last_work,
            work: self.document.work,
            job_refresh_cost: super::jobs::refresh_cost(super::drop::tier_for_power(self.power())),
            jobs: self
                .jobs(day)
                .into_iter()
                .enumerate()
                .map(|(slot, offer)| JobView {
                    slot,
                    tier: offer.tier,
                    hours: offer.hours,
                    grade: offer.grade,
                    stones: offer.stones(),
                })
                .collect(),
            last_drop: self.last_drop.clone().map(|drop| match drop {
                Drop::Equipment(item) => DropView::Equipment(ItemView {
                    id: item.id,
                    slot: item.slot,
                    tier: item.tier,
                    rarity: item.rarity,
                    attributes: item.effective_attributes(),
                    bonus: item.effective_bonus(),
                    equipped: false,
                    star: item.star(),
                    effect: item.effect,
                    star_cost: (!item.star_capped()).then(|| star_up_cost(&item)),
                    name: item.name.clone(),
                    sell_price: shop::ShopOffer::Equipment(item.clone()).sell_price(),
                }),
                Drop::Seed {
                    attribute,
                    tier,
                    grade,
                } => DropView::Seed {
                    attribute,
                    tier,
                    grade,
                },
                Drop::Art { name } => DropView::Art { name },
                Drop::Herb { herb } => DropView::Herb { herb },
                Drop::Stones(stones) => DropView::Stones(stones),
            }),
            last_seed_return: self.last_seed_return.clone(),
            arts,
            mastered_arts,
            art_power: self.art_power(),
            external_power: self.external_power(),
            aptitude_tenths,
            aptitude_herbs: self.aptitude_herbs(),
            marrow_gate_cost: super::aptitude::next_gate(aptitude_tenths).map(|(_, cost)| cost),
            total_levels: MAX_LEVEL,
            realm_full: self.level() >= self.realm().cap_level(),
            selected_art: self.document.selected_art,
            last_training: self.last_training.clone(),
            last_exploration_duel: self.last_exploration_duel.clone(),
            last_exploration: self.last_exploration,
            last_exploration_found: self.last_exploration_found.clone(),
            exploration_ready: self.exploration_ready(now_unix),
            breakthrough_ready: breakthrough_ready(
                self.document.breakthrough_blocked_until,
                now_unix,
            ),
            exploration_ready_at: self.walk_home_at(),
            exploration_hours: super::ExplorationHours::from_hours(
                self.document.last_exploration_hours,
            ),
            // Whether the walk under way has already been called home: the page
            // draws the button as "on its way back" rather than offering to
            // call a called walk.
            walk_early: self.document.walk.is_some_and(|walk| walk.early),
            breakthrough_ready_at: self.document.breakthrough_blocked_until,
            breakthrough_attempts: self.breakthrough_attempts,
            explorations: self.document.explorations,
            trial_floor: self.document.trial_floor,
            trial_next_floor: self.trial_next_floor(),
            trial_warden_power: warden_power(self.trial_next_floor()),
            trial_attempts_left: self.trial_attempts_left(day),
            trial_blocked: self.trial_blocked(now_unix, day),
            last_trial: self.last_trial.clone(),
            trial_attempts: self.trial_attempts,
            proving_tokens: self.document.proving_tokens,
            proving_token_cap: TRIAL_CHANCES_CAP,
            proving_blocked: self.proving_blocked(now_unix),
            provings: self.provings,
            last_proving: self.last_proving.clone(),
            last_daily_event: self.last_daily_event.clone(),
            daily_event_day: self.document.daily_event_day,
            daily_events: self.daily_events,
            daily_event_seen_day: self.document.daily_event_seen_day,
            star_attempts: self.star_attempts,
            last_star: self.last_star.clone(),
            next_session_growth: self.next_session_growth(day),
            derived_breakdowns: attribute_derived.breakdowns(attributes, bonus, level),
            duel_record: self.document.duel_record.clone(),
            last_walk: self.document.last_walk.clone(),
            events: self.document.events.clone(),
            stones: self.document.stones,
            last_refusal: self.last_refusal,
            refusal_count: self.refusal_count,
            day,
            shop_offers: self.shop(day),
        }
    }

    /// What the next finished session pays, in growth.
    ///
    /// The number rather than the multiplier the four disciplines add up to: the
    /// disciplines are not shown, so a percentage would be a rule the user can
    /// neither see nor move. This is the growth the next pomodoro is worth, and
    /// it is read from the same formula the session itself is paid by, so the
    /// two cannot disagree.
    ///
    /// The session number is the one the next session will actually be: the
    /// counter resets when the day changes, so a session that lands on a new day
    /// is the first of that day rather than the next of this one.
    fn next_session_growth(&self, day: i64) -> u32 {
        let stage = self.realm().index() as u32 + 1;
        let session_number = if self.document.last_active_day == Some(day) {
            self.document.session_count_today.saturating_add(1)
        } else {
            1
        };
        scaled_growth(
            super::session_growth(session_number, stage),
            self.growth_multiplier_permille(),
        )
    }

    /// The 阶 of the 功法 a finished session trains, or zero when none is chosen.
    ///
    /// The 修炼 dimension is trained by this and nothing else, so both the bar and
    /// the session multiplier read it from here.
    fn trained_art_rank(&self) -> u8 {
        self.document
            .selected_art
            .and_then(|id| self.document.arts.iter().find(|art| art.art_id == id))
            .map_or(0, |art| art.rank)
    }

    /// One owned item as a surface reads it, worn or not.
    fn item_view(&self, item: &EquipmentRecord) -> ItemView {
        ItemView {
            id: item.id,
            slot: item.slot,
            tier: item.tier,
            rarity: item.rarity,
            // The numbers the piece actually pays, stars folded in: what the
            // fight reads and what the row prints are the same numbers.
            attributes: item.effective_attributes(),
            bonus: item.effective_bonus(),
            equipped: self
                .document
                .equipped
                .get(item.slot.index())
                .copied()
                .flatten()
                == Some(item.id),
            star: item.star(),
            effect: item.effect,
            // What one more star costs, and nothing once the piece holds every
            // star it can: a button that asked for a star that cannot be had
            // would be a button that lies.
            star_cost: (!item.star_capped()).then(|| star_up_cost(item)),
            name: item.name.clone(),
            sell_price: shop::ShopOffer::Equipment(item.clone()).sell_price(),
        }
    }
}

// The shop's transitions live here rather than beside the shop's types: each one
// reads and writes the document's private fields, and a separate module could
// only do that by reaching through a `&mut Cultivation` it was handed.
impl Cultivation {
    /// The day's shelf, as a surface reads it.
    ///
    /// Rolled from the day and the realm, so the same day always shows the same
    /// goods and a document cannot carry a shelf it never would have rolled. The
    /// bought slots are cleared here rather than in the domain's own transition:
    /// a user who opens the page on a new day has, for that page, a new shelf.
    pub fn shop(&self, today: i64) -> Vec<ShopOfferView> {
        let realm = self.realm();
        let bought: &[u8] = if self.document.shop_day == Some(today) {
            &self.document.shop_bought
        } else {
            &[]
        };
        let offers = shop::roll_offers(today, realm, self.shop_seed(today));
        // The day's specials are read off the same day and realm as the shelf,
        // so the row's cut and the price a purchase charges are one number seen
        // twice rather than two guesses at the same discount.
        let specials = shop::special_slots(today, realm, offers.len());
        offers
            .into_iter()
            .enumerate()
            .map(|(slot, offer)| {
                let list_price = offer.price();
                let discount_permille = specials
                    .iter()
                    .find(|(special, _)| *special == slot)
                    .map_or(0, |(_, permille)| *permille);
                let price = if discount_permille == 0 {
                    list_price
                } else {
                    shop::discounted_price(list_price, discount_permille)
                };
                ShopOfferView {
                    offer: offer.clone(),
                    price,
                    list_price,
                    discount_permille,
                    sell_price: offer.sell_price(),
                    bought: bought.contains(&(slot as u8)),
                    slot,
                }
            })
            .collect()
    }

    /// Buy one of the day's offers.
    ///
    /// The slot is what a command names, not the item, because the shelf is
    /// regenerated from the day: naming the item would mean a click on an item
    /// from yesterday's shelf could buy today's item in the same slot.
    pub fn buy_from_shop(
        &mut self,
        slot: usize,
        today: i64,
        now_unix: i64,
    ) -> Result<ShopOffer, TradeRefusal> {
        self.roll_shop_day(today);
        let realm = self.realm();
        let shelf = shop::roll_offers(today, realm, self.shop_seed(today));
        let Some(offer) = shelf.get(slot).cloned() else {
            return Err(TradeRefusal::NoSuchOffer);
        };
        if self.document.shop_bought.contains(&(slot as u8)) {
            return Err(TradeRefusal::AlreadyBought);
        }
        // What the shelf asks, special and all: the button's number is the one
        // the purchase charges, or the cut would be a label on nothing.
        let price = match shop::special_slots(today, realm, shelf.len())
            .into_iter()
            .find(|(special, _)| *special == slot)
        {
            Some((_, permille)) => shop::discounted_price(offer.price(), permille),
            None => offer.price(),
        };
        if self.document.stones < price {
            return Err(TradeRefusal::NotEnoughStones);
        }
        // A 功法 the cultivator already has cannot be bought: it is either a
        // book they are still reading or a 功法 they have learned, and a second
        // copy of either is not a thing the page could say anything true about.
        if let ShopOffer::Art { art_id } = &offer
            && (self.document.arts.iter().any(|art| art.art_id == *art_id)
                || self.document.mastered_arts.contains(art_id))
        {
            return Err(TradeRefusal::AlreadyOwned);
        }
        self.document.stones -= price;
        self.document.shop_bought.push(slot as u8);
        // The goods land the same way a drop's do, so a bought 功法 counts toward
        // 修炼 and a bought herb plants, rather than sitting in a drawer.
        let bought = offer.clone();
        // The log says what was bought, and what it cost: named when the thing
        // has a name, the kind when it does not, and the price beside both —
        // a purchase that does not say what it paid is one the buyer cannot
        // check against the shelf.
        let what = match &bought {
            ShopOffer::Equipment(item) => SaleKind::Equipment {
                name: item.name.clone(),
                attributes: Some(item.effective_attributes()),
            },
            ShopOffer::Seed {
                attribute,
                name,
                tier,
                grade,
            } => SaleKind::Seed {
                name: name.clone(),
                tier: *tier,
                grade: *grade,
                // The herb's own attribute, kept beside its name: the log line
                // prints what a harvest of it pays, and that needs the attribute
                // rather than the words in the name.
                attribute: Some(*attribute),
            },
            ShopOffer::Art { art_id } => SaleKind::Art {
                name: Some(blueprint(*art_id).name),
                art_id: Some(*art_id),
            },
            ShopOffer::Herb { herb } => SaleKind::Herb { herb: *herb },
        };
        match bought {
            ShopOffer::Equipment(item) => {
                let mut item = item;
                item.id = self.next_id();
                self.document.equipment.push(item);
            }
            ShopOffer::Seed {
                attribute,
                tier,
                grade,
                name,
            } => {
                let mut rng = self.rng(now_unix);
                self.push_seed_named(attribute, tier, grade, name, &mut rng);
            }
            ShopOffer::Art { art_id } => {
                // The book that was on the shelf, not another one of its grade:
                // the shelf names the 功法, and buying a different one would be
                // a market that sells what it does not show.
                self.grant_art_book(art_id);
            }
            ShopOffer::Herb { herb } => {
                // The two 潜质 herbs are fed on arrival — the row that used to
                // hold them on the page is gone — while a 洗髓丹 lands in the bag,
                // because a pill is spent by opening a gate from the satchel.
                match herb {
                    super::AptitudeHerb::Marrow => {
                        self.document.marrow_pills = self.document.marrow_pills.saturating_add(1);
                        self.push_event(
                            now_unix,
                            EventDetail::MarrowFound {
                                source: MarrowSource::Shop,
                            },
                        );
                    }
                    _ => self.feed_found_herb(herb, now_unix),
                }
            }
        }
        self.push_event(
            now_unix,
            EventDetail::Bought {
                what,
                stones: price,
            },
        );
        Ok(offer)
    }

    /// Sell one piece of gear for 灵石.
    pub fn sell_equipment(&mut self, item_id: u64, now_unix: i64) -> Result<u32, TradeRefusal> {
        let Some(index) = self
            .document
            .equipment
            .iter()
            .position(|item| item.id == item_id)
        else {
            return Err(TradeRefusal::NotOwned);
        };
        if self
            .document
            .equipped
            .iter()
            .flatten()
            .any(|worn| *worn == item_id)
        {
            return Err(TradeRefusal::Equipped);
        }
        let item: EquipmentRecord = self.document.equipment.remove(index);
        let stones = shop::ShopOffer::Equipment(item.clone()).sell_price();
        self.document.stones = self.document.stones.saturating_add(stones);
        self.push_event(
            now_unix,
            EventDetail::Sold {
                what: SaleKind::Equipment {
                    name: item.name.clone(),
                    attributes: Some(item.effective_attributes()),
                },
                stones,
            },
        );
        Ok(stones)
    }

    /// Sell one 功法 for 灵石 — a book, or the tome of a learned one.
    ///
    /// The one being trained is refused: 修炼 reads it every session, and the
    /// record of a session that trained a 功法 no longer owned would be a lie.
    ///
    /// A **已习得** 功法 is sellable too, and selling one sells the tome, not
    /// the knowledge: the 功法 stays in [`CultivationDocument::mastered_arts`],
    /// its attributes and its swing stay exactly what they were, and the stones
    /// are the market's price for the paper. The sale is collected once — a
    /// tome the market buys again every time it is asked about would be a
    /// money printer — so the id is written into
    /// [`CultivationDocument::sold_mastered_arts`] and a second attempt is
    /// refused.
    pub fn sell_art(&mut self, art_id: u64, now_unix: i64) -> Result<u32, TradeRefusal> {
        if self.document.selected_art == Some(art_id) {
            return Err(TradeRefusal::Training);
        }
        let name = blueprint(art_id).name;
        let learned = self.document.arts.iter().all(|art| art.art_id != art_id);
        if learned {
            // A learned 功法 is not in the books; if it is not learned either,
            // there is nothing here to sell.
            if !self.document.mastered_arts.contains(&art_id) {
                return Err(TradeRefusal::NotOwned);
            }
            if self.document.sold_mastered_arts.contains(&art_id) {
                return Err(TradeRefusal::AlreadySold);
            }
            self.document.sold_mastered_arts.push(art_id);
        } else {
            self.document.arts.retain(|art| art.art_id != art_id);
        }
        let stones = shop::ShopOffer::Art { art_id }.sell_price();
        self.document.stones = self.document.stones.saturating_add(stones);
        self.push_event(
            now_unix,
            EventDetail::Sold {
                what: SaleKind::Art {
                    name: Some(name),
                    art_id: Some(art_id),
                },
                stones,
            },
        );
        Ok(stones)
    }

    /// Sell one seed from a stack for 灵石.
    ///
    /// One at a time rather than the whole stack: a stack is nine seeds and one
    /// of them, and which of them the user wants to keep is not a question a
    /// single "sell all" button can ask.
    pub fn sell_seed(
        &mut self,
        attribute: super::Attribute,
        tier: u8,
        grade: SeedGrade,
        now_unix: i64,
    ) -> Result<u32, TradeRefusal> {
        let Some(index) = self.document.seeds.iter().position(|stack| {
            stack.attribute == attribute && stack.tier == tier && stack.grade == grade
        }) else {
            return Err(TradeRefusal::NotOwned);
        };
        if self.document.seeds[index].count == 0 {
            return Err(TradeRefusal::NotOwned);
        }
        let stack: &mut SeedStack = &mut self.document.seeds[index];
        stack.count -= 1;
        let stones = shop::ShopOffer::Seed {
            attribute,
            tier,
            grade,
            name: stack.name.clone(),
        }
        .sell_price();
        let name = stack.name.clone();
        if stack.count == 0 {
            self.document.seeds.remove(index);
        }
        self.document.stones = self.document.stones.saturating_add(stones);
        self.push_event(
            now_unix,
            EventDetail::Sold {
                what: SaleKind::Seed {
                    name,
                    tier,
                    grade,
                    attribute: Some(attribute),
                },
                stones,
            },
        );
        Ok(stones)
    }

    /// Buy one more garden plot with 灵石, past the four the garden starts
    /// with.
    ///
    /// The price doubles with every plot bought, and the garden holds nine at
    /// most: a plot is a permanent capacity, so the last one costs what a
    /// season of sessions earns rather than what one afternoon does. A refusal
    /// is remembered the way every trade's is, so the page can say why.
    pub fn buy_plot(&mut self, now_unix: i64) -> Result<u32, TradeRefusal> {
        let Some(price) = plot_price(self.document.plots.len()) else {
            return Err(TradeRefusal::PlotLimit);
        };
        if self.document.stones < price {
            return Err(TradeRefusal::NotEnoughStones);
        }
        self.document.stones -= price;
        self.document.plots.push(PlotRecord::default());
        self.push_event(
            now_unix,
            EventDetail::Bought {
                what: SaleKind::Plot,
                stones: price,
            },
        );
        Ok(price)
    }

    /// Roll the shop's day forward if it has not been done yet.
    fn roll_shop_day(&mut self, today: i64) {
        if self.document.shop_day != Some(today) {
            self.document.shop_day = Some(today);
            self.document.shop_bought.clear();
        }
    }

    /// The seed the day's shelf is rolled from.
    ///
    /// The day alone, so the shelf is a pure function of `(day, realm)` — the same
    /// property the tests pin. Folding in the user's own counter would have spread
    /// two saves apart, but it would also have moved the shelf every time the user
    /// bought something, which is a shop that rearranges itself mid-transaction.
    fn shop_seed(&self, today: i64) -> u64 {
        today as u64
    }

    /// Credit one finished session's 灵石.
    ///
    /// The only 灵石 that arrives without selling something. Small enough that a
    /// user who never sells is poor on purpose: the currency is a way to convert a
    /// walk into something useful, and a faucet that outran the walks would make
    /// the shop a second ladder.
    pub(crate) fn credit_session_earnings(&mut self) {
        let earnings = session_earnings(self.realm());
        self.document.stones = self.document.stones.saturating_add(earnings);
    }

    /// The purse, for a surface.
    pub fn stones(&self) -> u32 {
        self.document.stones
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        AptitudeHerb, DISCIPLINE_STEP_CAP, DISCIPLINE_STEP_PERMILLE, Discipline, ExplorationHours,
        GROWTH_MULTIPLIER_CAP_PERMILLE, LAYERS_PER_STAGE, STAGE_COUNT, discipline_bonus,
        discipline_cap, growth_permille, scaled_growth, session_growth, threshold_for,
    };
    use super::*;
    use crate::CalendarDay;

    /// How far apart the tests take their walks: the longest length, so a walk
    /// taken at any of the three is owed again by the time the next one starts.
    const WALK_SECONDS: i64 = 12 * 60 * 60;

    fn day(number: i64) -> CalendarDay {
        CalendarDay::from_day_number(number)
    }

    /// The instant that local day opens, for a charge that wants a timestamp.
    fn day_unix(day: CalendarDay) -> i64 {
        day.day_number() * 86_400
    }

    /// An even hundred holds a body until its 洗髓丹 are paid: the herb is not
    /// burned against a gate it did not open, and the pill count is the number
    /// the brief names — 200 costs two, 300 three.
    #[test]
    fn an_even_hundred_holds_a_body_until_its_pills_are_paid() {
        let make = |pills: u32| {
            Cultivation::new(CultivationDocument {
                aptitude_tenths: Some(1_999),
                insight_herbs: 1,
                marrow_pills: pills,
                ..CultivationDocument::default()
            })
        };
        // 199.9 plus any 悟心草 crosses 200, which the brief prices at two pills.
        let mut poor = make(1);
        assert_eq!(
            poor.use_aptitude_herb(AptitudeHerb::Insight, 10),
            Err(TradeRefusal::NotEnoughMarrow)
        );
        assert_eq!(poor.aptitude_tenths(), 1_999, "still stuck at 199.9");
        assert_eq!(poor.aptitude_herbs()[0], 1, "the herb is not burned");

        let mut rich = make(2);
        let used = rich
            .use_aptitude_herb(AptitudeHerb::Insight, 10)
            .expect("two pills open the two-hundredth");
        assert_eq!(used.marrow_spent, 2);
        assert!(used.aptitude_tenths > 2_000, "the body crossed 200");
        assert_eq!(used.gained_tenths, used.aptitude_tenths - 1_999);
        assert_eq!(rich.aptitude_herbs()[0], 0);
        assert_eq!(rich.aptitude_herbs()[2], 0, "the pills were spent");
    }

    /// A body with no herb in the bag is refused, and the refusal costs nothing.
    #[test]
    fn a_herb_nobody_holds_is_refused_rather_than_faked() {
        let mut cultivation = Cultivation::new(CultivationDocument {
            aptitude_tenths: Some(0),
            ..CultivationDocument::default()
        });
        assert_eq!(
            cultivation.use_aptitude_herb(AptitudeHerb::Insight, 10),
            Err(TradeRefusal::HerbNotHeld)
        );
        // The pill is not a herb: feeding one to the body is refused, because a
        // pill has nothing to feed — it opens a gate, and that is
        // [`Cultivation::use_marrow_pill`].
        assert_eq!(
            cultivation.use_aptitude_herb(AptitudeHerb::Marrow, 10),
            Err(TradeRefusal::HerbNotHeld)
        );
        assert_eq!(cultivation.aptitude_tenths(), 0);
    }

    /// The pill on its own buys the gate ahead, at the price the crossing asks:
    /// one price for one gate, whether a herb or a hand opens it.
    #[test]
    fn a_pill_on_its_own_opens_the_gate_ahead_at_that_gates_price() {
        let mut cultivation = Cultivation::new(CultivationDocument {
            aptitude_tenths: Some(50),
            marrow_pills: 3,
            ..CultivationDocument::default()
        });
        assert_eq!(cultivation.view().marrow_gate_cost, Some(1));
        let first = cultivation
            .use_marrow_pill(10)
            .expect("the first hundred is one pill");
        assert_eq!(first.marrow_spent, 1);
        assert_eq!(
            first.aptitude_tenths, 1_000,
            "carried to the hundred itself"
        );
        assert_eq!(cultivation.aptitude_herbs()[2], 2, "one pill was spent");
        // The line is written, so the satchel and the log agree about the jump.
        assert!(
            matches!(
                cultivation.view().events.first().map(|event| &event.detail),
                Some(EventDetail::Aptitude { marrow: 1, .. })
            ),
            "the pill is logged as a 潜质 move"
        );
        // The gate ahead is now the two-hundredth, and it costs two.
        assert_eq!(cultivation.view().marrow_gate_cost, Some(2));
        let second = cultivation
            .use_marrow_pill(10)
            .expect("the two-hundredth costs two");
        assert_eq!(second.marrow_spent, 2);
        assert_eq!(second.aptitude_tenths, 2_000);
        assert_eq!(cultivation.aptitude_herbs()[2], 0);
        // Nothing left to pay with: the refusal costs nothing.
        assert_eq!(
            cultivation.use_marrow_pill(10),
            Err(TradeRefusal::NotEnoughMarrow)
        );
        assert_eq!(cultivation.aptitude_tenths(), 2_000);
    }

    /// The top of the band has no gate past it, so there is nothing to buy and
    /// no pill is burned finding that out.
    #[test]
    fn a_body_at_the_top_of_the_band_has_no_gate_left_to_buy() {
        let mut cultivation = Cultivation::new(CultivationDocument {
            aptitude_tenths: Some(crate::cultivation::APTITUDE_MAX_TENTHS),
            marrow_pills: 9,
            ..CultivationDocument::default()
        });
        assert_eq!(cultivation.view().marrow_gate_cost, None);
        assert_eq!(
            cultivation.use_marrow_pill(10),
            Err(TradeRefusal::AptitudeCapped)
        );
        assert_eq!(cultivation.aptitude_herbs()[2], 9, "no pill is burned");
    }

    /// The 潜质 an art asks of the body is the one the page shows, and it is
    /// three times the printed cost at zero.
    #[test]
    fn aptitude_sets_what_a_rank_asks_of_the_body() {
        let cultivation = Cultivation::new(CultivationDocument {
            aptitude_tenths: Some(0),
            arts: vec![OwnedArt {
                art_id: 1,
                rank: 1,
                progress: 0,
                outcome: None,
                failures: 0,
            }],
            ..CultivationDocument::default()
        });
        let view = cultivation.view();
        let art = view.arts.first().expect("the book is on the shelf");
        assert_eq!(art.required, 3, "the printed 1 costs a zero-潜质 body 3");
        assert_eq!(
            art.success_permille,
            super::super::aptitude::base_success_permille(art.grade),
            "the rate is the grade's own at zero 潜质 and no failures"
        );
    }

    /// A cultivator at the first realm's last layer, so a guardian is offered.
    /// A finished pomodoro buys one 试炼 chance, ten at most are banked, and a
    /// banked chance buys a fight — with no clock anywhere in the bargain.
    #[test]
    fn a_session_buys_a_chance_and_a_chance_buys_a_fight() {
        let mut cultivation = Cultivation::default();
        assert!(
            cultivation.proving(1_000).is_none(),
            "an empty bank buys no fight"
        );
        assert_eq!(cultivation.last_refusal, Some(Refusal::ProvingSpent));

        // Twelve finished sessions, one local day: the bank fills to its cap
        // and stays there rather than growing past it.
        for step in 0..12 {
            cultivation.on_session_completed(1_000 + step * 10, day(1), 25);
        }
        let filled = cultivation.view();
        assert_eq!(filled.proving_tokens, TRIAL_CHANCES_CAP, "ten at most");
        assert_eq!(filled.proving_token_cap, TRIAL_CHANCES_CAP);

        let outcome = cultivation
            .proving(2_000)
            .expect("a banked chance buys a fight");
        assert!(!outcome.duel.rounds.is_empty(), "a fight has blows");
        assert_eq!(
            cultivation.view().proving_tokens,
            TRIAL_CHANCES_CAP - 1,
            "the chance is spent"
        );
        assert_eq!(cultivation.view().provings, 1);
        assert!(
            cultivation.view().last_proving.is_some(),
            "the popup has a report to replay"
        );
    }

    /// A won 试炼 pays a purse of its own, and the log says so.
    ///
    /// The courtyard used to pay its exclusive roll and nothing else, so a clean
    /// win whose roll came up empty read as "试炼获胜。" and a chance spent for
    /// free. Every other fight pays a purse; this one does now too.
    #[test]
    fn a_won_proving_pays_a_purse_the_log_names() {
        let mut paid = None;
        for step in 0..40i64 {
            let mut cultivation = Cultivation::new(CultivationDocument {
                proving_tokens: 1,
                ..CultivationDocument::default()
            });
            let before = cultivation.view().stones;
            let Some(outcome) = cultivation.proving(1_000 + step * 7) else {
                continue;
            };
            if !outcome.duel.won {
                continue;
            }
            let purse = outcome.found.stones.expect("a win pays a purse");
            assert!(purse > 0, "a purse is never empty");
            assert_eq!(
                cultivation.view().stones,
                before + purse,
                "the purse is credited to the cultivator"
            );
            match cultivation
                .view()
                .events
                .first()
                .map(|event| event.detail.clone())
            {
                Some(EventDetail::Proving {
                    won: true,
                    stones: Some(logged),
                    ..
                }) => assert_eq!(logged, purse, "the line names the purse it paid"),
                other => panic!("the newest line is the bout: {other:?}"),
            }
            paid = Some(purse);
            break;
        }
        assert!(paid.is_some(), "forty bouts should win at least one");
    }

    /// 试炼 is fought at home: a walk that has not come back closes the courtyard
    /// even when a chance is banked.
    #[test]
    fn a_proving_waits_for_the_walk_to_come_home() {
        let mut cultivation = Cultivation::default();
        for step in 0..3 {
            cultivation.on_session_completed(1_000 + step * 10, day(1), 25);
        }
        let tokens = cultivation.view().proving_tokens;
        assert!(tokens > 0, "three sessions banked three chances");
        cultivation.explore(2_000, ExplorationHours::Four);
        assert!(
            cultivation.proving(2_000 + 3_600).is_none(),
            "an unreturned walk closes the courtyard"
        );
        assert_eq!(cultivation.last_refusal, Some(Refusal::ProvingOut));
        assert_eq!(
            cultivation.view().proving_tokens,
            tokens,
            "a refused 试炼 spends no chance"
        );
        let home = 2_000 + 4 * 3_600;
        assert!(
            cultivation.proving(home).is_some(),
            "back home, the chance buys its fight"
        );
    }

    /// 试炼 closes with a job in hand, the way the tower does: both are fights
    /// fought at home, and a job is hours of the same day as a walk. The wall
    /// falls when the hours are up — before the tick has come round to pay the
    /// wage — so a fight refused on a job that is already done would be reading
    /// the clock wrong.
    #[test]
    fn a_proving_waits_for_the_job_to_come_home() {
        let mut cultivation = Cultivation::new(CultivationDocument {
            proving_tokens: 3,
            ..CultivationDocument::default()
        });
        let hours = ExplorationHours::from_hours(4);
        let out = 2_000 + hours.seconds();
        cultivation.explore(2_000, hours);
        cultivation.settle_walk(out);
        let run = cultivation
            .accept_job(0, out + 1, 1)
            .expect("the job is taken once the walk is home");
        assert_eq!(
            cultivation.proving_blocked(out + 60),
            Some(ProvingBlocked::Working),
            "the job in hand is the wall the page should print"
        );
        assert!(
            cultivation.proving(out + 60).is_none(),
            "an unreturned job closes the courtyard too"
        );
        assert_eq!(cultivation.last_refusal, Some(Refusal::Working));
        assert_eq!(
            cultivation.view().proving_tokens,
            3,
            "a refused 试炼 spends no chance"
        );
        // The hours are up: no wall left, even before the wage has been paid.
        assert_eq!(
            cultivation.proving_blocked(run.ends_unix),
            None,
            "the clock is home"
        );
        assert!(
            cultivation.proving(run.ends_unix).is_some(),
            "back home, the chance buys its fight"
        );
        assert_eq!(
            cultivation.view().proving_tokens,
            2,
            "and this one spent its chance"
        );
    }

    /// The thousandth: one finished pomodoro in a thousand pays something beside
    /// its growth and its ordinary find.
    #[test]
    fn a_pomodoro_in_a_thousand_pays_a_bonus() {
        assert_eq!(SESSION_BONUS_PERMILLE, 1, "千分之一");
        // The roll itself, drawn often enough to see every shape it can take.
        let mut cultivation = a_bare_save_at(40);
        let mut kinds = std::collections::BTreeSet::new();
        let mut rng = Rng::seeded(0x0000_5EED_B0A7_1234);
        for _ in 0..2_000 {
            let strength = Strength::of(cultivation.power());
            if let Some(drop) = cultivation.roll_bonus(strength, &mut rng, 1_000) {
                kinds.insert(match drop {
                    Drop::Equipment(_) => 0,
                    Drop::Seed { .. } => 1,
                    Drop::Art { .. } => 2,
                    Drop::Herb { .. } => 3,
                    Drop::Stones(_) => 4,
                });
            }
        }
        assert_eq!(kinds.len(), 5, "every shape of the jackpot is reachable");
        // And the rate it is rolled at, measured rather than asserted: one in a
        // thousand, over a hundred thousand sessions' worth of rolls.
        let mut rng = Rng::seeded(0x0000_5EED_5EED_5EED);
        let hits = (0..100_000)
            .filter(|_| rng.below(1_000) < SESSION_BONUS_PERMILLE)
            .count();
        assert!(
            (60..=140).contains(&hits),
            "{hits} of a hundred thousand is not one in a thousand"
        );
    }

    /// A dropped seed carries a grade, and two grades of one patch are two
    /// stacks: the grade is what the harvest pays at.
    #[test]
    fn a_dropped_seed_carries_its_grade() {
        let mut cultivation = Cultivation::default();
        let mut rng = Rng::seeded(7);
        let earth = cultivation.push_seed(Attribute::Root, 2, SeedGrade::Earth, &mut rng);
        let divine = cultivation.push_seed(Attribute::Root, 2, SeedGrade::Divine, &mut rng);
        assert_eq!(earth.count, 1);
        assert_eq!(divine.count, 1);
        assert_eq!(
            cultivation.document().seeds.len(),
            2,
            "two grades of one patch are two things to plant"
        );
        assert!(
            divine.sell_price > earth.sell_price,
            "{} against {}",
            divine.sell_price,
            earth.sell_price
        );
        assert!(
            cultivation
                .view()
                .seeds
                .iter()
                .any(|seed| seed.grade == SeedGrade::Divine)
        );
    }

    /// The ten 潜质 books are the only 功法 that move 潜质, and the number the
    /// panel reads is the number the learning roll uses.
    #[test]
    fn only_the_aptitude_books_move_the_aptitude_reading() {
        for art_id in crate::cultivation::APTITUDE_ART_IDS {
            let cultivation = Cultivation::new(CultivationDocument {
                // A body born with nothing: the reading below is the books'.
                aptitude_tenths: Some(0),
                arts: vec![OwnedArt {
                    art_id,
                    rank: 1,
                    progress: 0,
                    outcome: None,
                    failures: 0,
                }],
                ..CultivationDocument::default()
            });
            assert_eq!(
                cultivation.aptitude_tenths(),
                crate::cultivation::arts::aptitude_tenths_per_rank(art_id),
                "one 阶 of {art_id} is worth its grade's tenths"
            );
        }
        // Any other book pays nothing, however far it has climbed.
        let ordinary = Cultivation::new(CultivationDocument {
            aptitude_tenths: Some(0),
            arts: vec![OwnedArt {
                art_id: 1,
                rank: 2,
                progress: 0,
                outcome: None,
                failures: 0,
            }],
            ..CultivationDocument::default()
        });
        assert_eq!(ordinary.aptitude_tenths(), 0);
    }

    /// The length a walk was taken for is the length that holds the next one
    /// off: four hours is owed again at four, not at ten.
    #[test]
    fn a_walk_is_held_for_its_own_length_and_no_longer() {
        let mut cultivation = Cultivation::default();
        cultivation.explore(1_000, ExplorationHours::Four);
        assert!(
            cultivation
                .explore(1_000 + 4 * 3_600 - 1, ExplorationHours::Four)
                .is_none(),
            "three hours fifty-nine is too early"
        );
        assert!(
            cultivation
                .explore(1_000 + 4 * 3_600, ExplorationHours::Four)
                .is_some(),
            "four hours is a walk"
        );
        // The length is written down, so the page's countdown is the length the
        // walk actually took rather than a fixed ten hours.
        assert_eq!(cultivation.document().last_exploration_hours, 4);
        assert_eq!(
            cultivation.view().exploration_ready_at,
            Some(1_000 + 4 * 3_600 + 4 * 3_600)
        );
    }

    fn at_the_first_cap() -> Cultivation {
        Cultivation::new(CultivationDocument {
            growth: threshold_for(LAYERS_PER_STAGE),
            ..CultivationDocument::default()
        })
    }

    /// Four dimensions that add rather than compound, capped at four times a bare
    /// session.
    ///
    /// The cap is the whole promise of the formula: a save that trained
    /// everything is worth four sessions, never sixteen, or the ladder would be a
    /// different game for anyone who played long enough.
    #[test]
    fn four_dimensions_add_up_to_at_most_four_times_a_bare_session() {
        let trained = Realm::from_level(LAYERS_PER_STAGE * STAGE_COUNT as u16 - 1);
        assert_eq!(
            growth_permille(trained, 9, 40, 40, 40),
            GROWTH_MULTIPLIER_CAP_PERMILLE,
            "a fully trained save is capped, not multiplied twice"
        );
        assert_eq!(
            growth_permille(trained, 0, 0, 0, 0),
            1_000,
            "an untrained one is flat"
        );
    }

    /// 修炼 is capped by the realm, not by a count: a 阶 of 功法 is worth more the
    /// deeper the realm is, which is what keeps the late game honest.
    #[test]
    fn a_trained_art_is_worth_more_the_deeper_the_realm_is() {
        let first = Realm::from_level(1);
        let deepest = Realm::from_level(LAYERS_PER_STAGE * STAGE_COUNT as u16 - 1);
        assert_eq!(discipline_cap(Discipline::Practice, first), 200);
        assert_eq!(discipline_cap(Discipline::Practice, deepest), 3_000);
        assert_eq!(
            discipline_bonus(Discipline::Practice, 9, 0, first),
            200,
            "炼气 stops at one fifth more than a bare session"
        );
        assert_eq!(
            discipline_bonus(Discipline::Practice, 9, 0, deepest),
            450,
            "and no further than four 阶 can carry it"
        );
    }

    /// The three action dimensions share one cap, so no single path is worth
    /// farming forever.
    #[test]
    fn an_action_dimension_stops_at_twenty_acts() {
        let realm = Realm::from_level(1);
        assert_eq!(discipline_bonus(Discipline::Wander, 0, 0, realm), 0);
        assert_eq!(discipline_bonus(Discipline::Wander, 0, 1, realm), 50);
        assert_eq!(
            discipline_bonus(Discipline::Wander, 0, 20, realm),
            1_000,
            "twenty walks is the whole of it"
        );
        assert_eq!(
            discipline_bonus(Discipline::Wander, 0, 400, realm),
            1_000,
            "and four hundred is no more"
        );
    }

    /// What the page shows and what the session pays are the same number.
    ///
    /// The page prints the growth the next session pays rather than the
    /// multiplier behind it, so this reads that number and checks the session
    /// against it: the two are the same formula read twice.
    #[test]
    fn a_finished_session_pays_what_the_page_says_it_pays() {
        let mut cultivation = Cultivation::default();
        // The page is asked first, before the session happens: it says what the
        // next pomodoro is worth, and the session then pays that.
        let promised = cultivation.view().next_session_growth;
        assert_eq!(
            promised,
            session_growth(1, 1),
            "an untrained save pays the base"
        );
        let flat = cultivation.on_session_completed(1_000, day(20), 25).growth;
        assert_eq!(
            flat, promised,
            "and the session pays exactly what the page printed"
        );

        // Twenty walks train 游历 to its cap; each walk is one cooldown apart, and
        // a walk that meets someone may train 战斗 as well — so the number the
        // page shows is read back rather than assumed.
        for step in 0..20 {
            let at = 1_000 + step * WALK_SECONDS;
            cultivation.explore(at, ExplorationHours::Twelve);
            // A walk trains the 游历 dimension when it comes home, so the test
            // brings it home on the same clock the next walk starts on.
            cultivation.settle_walk(at + WALK_SECONDS);
        }
        let view = cultivation.view();
        assert!(
            view.next_session_growth > session_growth(1, 1),
            "walking has to be worth something: {}",
            view.next_session_growth
        );

        let reward = cultivation.on_session_completed(1_000_000, day(30), 25);
        assert_eq!(
            reward.growth, view.next_session_growth,
            "the session pays exactly what the page printed"
        );
    }

    /// A missed day is scaled by the same number, so a save that trained deep
    /// also loses more for letting a day go.
    #[test]
    fn a_missed_day_costs_what_a_trained_save_earns() {
        let active = |explorations| CultivationDocument {
            growth: 100_000,
            explorations,
            last_active_day: Some(19),
            charged_through_day: Some(19),
            ..CultivationDocument::default()
        };
        let mut trained = Cultivation::new(active(20));
        let multiplier = growth_permille(trained.realm(), 0, 20, 0, 0);
        assert_eq!(multiplier, 2_000);
        let mut bare = Cultivation::new(active(0));
        assert_eq!(growth_permille(bare.realm(), 0, 0, 0, 0), 1_000);

        // Charged through yesterday: asking on the twentieth charges nothing,
        // because the twentieth is the day still being lived in.
        let charged = trained
            .apply_absence(day(21), day_unix(day(21)))
            .expect("a day was charged");
        let flat = bare
            .apply_absence(day(21), day_unix(day(21)))
            .expect("a day was charged");
        assert_eq!(charged.days, 1);
        assert_eq!(charged.charged, flat.charged * 2, "twice the bare penalty");
        assert_eq!(
            charged.charged,
            scaled_growth(flat.charged, multiplier),
            "and exactly what the multiplier says"
        );
    }

    /// The record is a window of eight, newest first, however many were fought.
    ///
    /// Guardian fights cannot fill a window on their own: a win enters the next
    /// realm and there is nothing left to fight until the cap is climbed again,
    /// and a loss writes a twelve hour cooldown. So the window is filled the way a
    /// real save fills it — one guardian fight, then the fights met on walks.
    #[test]
    fn the_record_keeps_the_last_eight_fights_newest_first() {
        let mut cultivation = at_the_first_cap();
        for step in 0..60 {
            let at = 100_000 + step * WALK_SECONDS;
            let _ = cultivation.explore(at, ExplorationHours::Twelve);
            cultivation.settle_walk(at + WALK_SECONDS);
        }
        // Fought last, so the guardian's line is the newest one: a window of eight
        // would otherwise push it out, which is the window working, not failing.
        let _ = cultivation.breakthrough(100_000 + 60 * WALK_SECONDS + 1);
        let view = cultivation.view();
        assert_eq!(
            view.duel_record.len(),
            DUEL_RECORD_CAP,
            "the window holds at eight"
        );
        assert!(
            view.duel_record
                .windows(2)
                .all(|pair| pair[0].at_unix > pair[1].at_unix),
            "newest first: {:?}",
            view.duel_record
                .iter()
                .map(|line| line.at_unix)
                .collect::<Vec<_>>()
        );
        assert!(
            view.duel_record
                .iter()
                .any(|line| line.kind == FightKind::Guardian),
            "the guardian fight is in the record"
        );
        assert!(
            view.duel_record
                .iter()
                .any(|line| line.kind == FightKind::Wanderer),
            "and so are the walks' fights"
        );
    }

    /// A win is what trains the 战斗 dimension, and a loss is not.
    ///
    /// A guardian fight cannot carry this on its own — a fresh save loses it — so
    /// the count is read from the fights actually fought and checked against the
    /// dimension the growth formula reads.
    #[test]
    fn only_wins_train_the_battle_dimension() {
        let mut cultivation = at_the_first_cap();
        // One 外功 taken to its last 阶: a body that can win a road fight. At the
        // cap with every book still at rank zero the player swings bare-handed
        // against a wanderer carrying an art, and loses every meeting — which
        // would leave this test asserting nothing about wins at all.
        cultivation.document.arts.push(OwnedArt {
            art_id: 404,
            rank: 6,
            progress: 0,
            failures: 0,
            outcome: None,
        });
        cultivation.document.selected_art = Some(404);
        let _ = cultivation.breakthrough(1_000);
        assert_eq!(
            discipline_bonus(Discipline::Fight, 0, 0, cultivation.realm()),
            0,
            "a loss trains nothing"
        );

        for step in 0..40 {
            let at = 100_000 + step * WALK_SECONDS;
            let _ = cultivation.explore(at, ExplorationHours::Twelve);
            cultivation.settle_walk(at + WALK_SECONDS);
        }
        let view = cultivation.view();
        // The record is a window of eight, so the count read here is the
        // document's own — the same number the growth formula reads.
        let won = cultivation.document().battle_wins;
        assert!(won > 0, "forty walks should have won one: {view:?}");
        assert!(
            view.duel_record
                .iter()
                .any(|line| line.kind == FightKind::Wanderer),
            "and the record says who was met"
        );
        assert_eq!(
            discipline_bonus(Discipline::Fight, 0, won, view.realm),
            DISCIPLINE_STEP_PERMILLE * won.min(DISCIPLINE_STEP_CAP),
            "one step per win, and no more than the cap"
        );
    }

    /// The counters the four dimensions read are in the document, so a relaunch
    /// does not quietly reset them.
    #[test]
    fn the_dimensions_survive_a_relaunch() {
        let cultivation = Cultivation::new(CultivationDocument {
            growth: 100_000,
            explorations: 7,
            battle_wins: 5,
            harvests: 3,
            ..CultivationDocument::default()
        });
        let realm = cultivation.realm();
        assert_eq!(discipline_bonus(Discipline::Wander, 0, 7, realm), 350);
        assert_eq!(discipline_bonus(Discipline::Fight, 0, 5, realm), 250);
        assert_eq!(discipline_bonus(Discipline::Fortune, 0, 3, realm), 150);
        assert_eq!(growth_permille(realm, 0, 7, 5, 3), 1_750);
    }

    /// A save standing on a level with nothing worn and nothing trained.
    fn a_bare_save_at(level: u16) -> Cultivation {
        Cultivation::new(CultivationDocument {
            growth: threshold_for(level),
            realm: Realm::from_level(level).index() as u8,
            ..CultivationDocument::default()
        })
    }

    /// The tower is closed for two different reasons, and the page is told
    /// which: out on a walk, and out of the day's floors.
    #[test]
    fn the_tower_opens_only_at_home_and_only_while_the_day_has_floors() {
        let mut cultivation = a_bare_save_at(20);
        cultivation.document.walk = Some(super::super::WalkInProgress::started(
            1_000,
            ExplorationHours::Four,
        ));
        assert_eq!(cultivation.trial_blocked(1_001, 9), Some(TrialBlocked::Out));
        assert!(
            cultivation.trial(1_001, 9).is_none(),
            "a walk that has not come back closes the tower"
        );
        assert_eq!(
            cultivation.view_on(1_001, 9).last_refusal.map(Refusal::key),
            Some("cultivation.trial.refuse_out")
        );

        // Home: five floors belong to the day, and the sixth does not.
        cultivation.document.walk = None;
        let mut climbed = 0;
        for step in 0..TRIAL_ATTEMPTS_PER_DAY {
            let now = 10_000 + i64::from(step) * 60;
            assert!(cultivation.trial(now, 9).is_some(), "floor {step} was owed");
            climbed += 1;
        }
        assert_eq!(climbed, TRIAL_ATTEMPTS_PER_DAY);
        assert_eq!(
            cultivation.trial_blocked(20_000, 9),
            Some(TrialBlocked::Drained)
        );
        assert!(cultivation.trial(20_000, 9).is_none());
        assert_eq!(
            cultivation
                .view_on(20_000, 9)
                .last_refusal
                .map(Refusal::key),
            Some("cultivation.trial.refuse_drained")
        );

        // The next local day hands out the same five.
        let view = cultivation.view_on(20_000, 10);
        assert_eq!(view.trial_attempts_left, TRIAL_ATTEMPTS_PER_DAY);
        assert!(view.trial_blocked.is_none());
        assert!(cultivation.trial(20_001, 10).is_some());
        assert_eq!(
            cultivation.trial_attempts_left(10),
            TRIAL_ATTEMPTS_PER_DAY - 1
        );
    }

    /// A win moves the frontier up one floor; a loss costs the attempt and
    /// leaves the frontier where it stood, so the tower is a wall rather than a
    /// ladder that drops.
    #[test]
    fn a_win_moves_the_frontier_and_a_loss_holds_it() {
        let mut climbing = a_bare_save_at(30);
        let won = climbing.trial(1_000, 9).expect("the first floor is owed");
        assert_eq!(won.floor, 1, "the climb starts at the first floor");
        assert!(won.duel.won, "a level thirty save clears the first floor");
        assert_eq!(climbing.document.trial_floor, 1);
        assert_eq!(climbing.trial_next_floor(), 2);

        let mut walled = a_bare_save_at(30);
        walled.document.trial_floor = TRIAL_MAX_FLOOR - 1;
        let lost = walled.trial(1_000, 9).expect("the top floor is still owed");
        assert_eq!(lost.floor, TRIAL_MAX_FLOOR);
        assert!(
            !lost.duel.won,
            "the top of the tower is not cleared by level 30"
        );
        assert_eq!(
            walled.document.trial_floor,
            TRIAL_MAX_FLOOR - 1,
            "a loss costs the attempt, not the ground already held"
        );
        assert!(lost.found.is_empty(), "a loss finds nothing");

        // Every floor is behind this save now, and the tower says so.
        walled.document.trial_floor = TRIAL_MAX_FLOOR;
        assert_eq!(walled.trial_blocked(20_000, 9), Some(TrialBlocked::Top));
        assert_eq!(
            walled.view_on(20_000, 9).last_refusal,
            None,
            "reading the view is not a refusal"
        );
    }

    /// A save at `level` wearing the four pieces a player of that realm would be
    /// wearing.
    fn a_dressed_save_at(level: u16) -> Cultivation {
        let realm = Realm::from_level(level);
        let mut document = CultivationDocument {
            growth: threshold_for(level),
            realm: realm.index() as u8,
            equipped: vec![None; SLOT_COUNT],
            ..CultivationDocument::default()
        };
        let mut rng = Rng::seeded(u64::from(level));
        let mut slots = std::collections::BTreeSet::new();
        let mut id = 1;
        while slots.len() < SLOT_COUNT {
            let item = crate::cultivation::items::roll_equipment(
                id,
                realm.index() as u8 + 1,
                crate::cultivation::Rarity::Common,
                &mut rng,
            );
            if slots.insert(item.slot.index()) {
                document.equipped[item.slot.index()] = Some(item.id);
                document.equipment.push(item);
                id += 1;
            }
        }
        Cultivation::new(document)
    }

    /// The tower is the ladder read sideways: a floor stands for a level, and
    /// the warden is that level's cultivator softened, dressed, and swinging what
    /// it carries. This is the number that makes "how far can I climb" a question
    /// gear answers, so it is locked here — a retune of the attributes or the
    /// duel that moves the frontier a whole realm away from the save's own level
    /// fails this.
    #[test]
    fn a_dressed_save_holds_its_own_level_and_not_much_past_it() {
        let win_rate = |level: u16, floor: u16| {
            let runs = 200;
            let wins = (0..runs)
                .filter(|attempt: &usize| {
                    let mut cultivation = a_dressed_save_at(level);
                    cultivation.document.trial_floor = floor - 1;
                    cultivation
                        .trial(1_000 + (*attempt as i64) * 977, 9)
                        .is_some_and(|outcome| outcome.duel.won)
                })
                .count();
            wins * 100 / runs
        };
        for level in [10_u16, 100, 500, 1_000] {
            let own = win_rate(level, level);
            assert!(
                own >= 60,
                "level {level} holds its own floor only {own}% of the time"
            );
        }
        // The flat base numbers (攻击 ten, 生命 a hundred) dominate the first
        // realm, so its floors are a notch softer than the ladder's arithmetic
        // says — that is the on-ramp, and it is only the first realm's.
        for level in [100_u16, 500, 800] {
            let past = (u32::from(level) * 125 / 100).min(u32::from(TRIAL_MAX_FLOOR)) as u16;
            let past_rate = win_rate(level, past);
            assert!(
                past_rate <= 15,
                "level {level} clears floor {past} a quarter past it {past_rate}% of the time"
            );
        }
    }

    /// The day opens once, and once only: a relaunch the same afternoon is still
    /// the same morning.
    #[test]
    fn the_day_opens_once_and_only_once() {
        let mut cultivation = a_bare_save_at(20);
        let first = cultivation.daily_event(1_000, 9).expect("the day opens");
        assert!(
            matches!(
                first.kind,
                DailyKind::Ambush | DailyKind::Gift | DailyKind::Trinket
            ),
            "the morning has to be one of the three kinds"
        );
        assert!(
            cultivation.daily_event(2_000, 9).is_none(),
            "a second tick on the same day opens nothing"
        );
        assert_eq!(cultivation.document.daily_event_day, Some(9));
        assert!(
            matches!(
                cultivation.view().events.first().map(|event| &event.detail),
                Some(EventDetail::Daily { .. })
            ),
            "the morning is on the page's log"
        );
        assert!(
            cultivation.daily_event(3_000, 10).is_some(),
            "the next day opens on its own"
        );
        assert_eq!(cultivation.view().daily_events, 2);
    }

    /// The morning can only add. Every kind pays something — a fight only if it
    /// is won, which is the one branch that can come back empty-handed.
    #[test]
    fn a_morning_only_ever_adds() {
        for day_number in 0..60_i64 {
            let mut cultivation = a_bare_save_at(20);
            let before = cultivation.document.stones;
            let outcome = cultivation
                .daily_event(1_000 + day_number * 86_400, day_number)
                .expect("each day opens");
            assert!(
                cultivation.document.stones >= before,
                "the morning took money away"
            );
            match outcome.kind {
                DailyKind::Ambush => {
                    assert!(outcome.duel.is_some(), "an ambush is a fight to replay");
                    if outcome.won {
                        assert!(!outcome.found.is_empty(), "a won ambush pays something");
                    } else {
                        assert!(outcome.found.is_empty(), "a lost ambush takes nothing");
                    }
                }
                DailyKind::Gift | DailyKind::Trinket => {
                    assert!(outcome.duel.is_none(), "only an ambush is a fight");
                    assert!(
                        !outcome.found.is_empty(),
                        "a gift always hands something over"
                    );
                }
            }
        }
    }

    /// The learned panel is ordered by the ring, not by the shelf: the three it
    /// shows have to be the three being worked on.
    #[test]
    fn the_learned_panel_puts_the_recently_trained_first() {
        let mut cultivation = a_bare_save_at(20);
        cultivation.document.arts = vec![
            OwnedArt {
                art_id: 3,
                rank: 1,
                progress: 0,
                outcome: None,
                failures: 0,
            },
            OwnedArt {
                art_id: 7,
                rank: 1,
                progress: 0,
                outcome: None,
                failures: 0,
            },
        ];
        cultivation.select_art(Some(3));
        cultivation.train_selected(1_000);
        cultivation.select_art(Some(7));
        cultivation.train_selected(2_000);
        assert_eq!(
            cultivation.document.recent_arts.first(),
            Some(&7),
            "the last thing trained is at the front"
        );
        assert_eq!(
            cultivation.view().mastered_arts.first().map(|art| art.id),
            Some(7),
            "the ring orders the panel"
        );
        // Going back to the older book moves it back to the front.
        cultivation.select_art(Some(3));
        cultivation.train_selected(3_000);
        assert_eq!(
            cultivation.view().mastered_arts.first().map(|art| art.id),
            Some(3)
        );
    }

    /// The ring is a ring: it keeps the few, not every 功法 the save has met.
    #[test]
    fn the_recent_ring_forgets_the_oldest() {
        let mut cultivation = a_bare_save_at(20);
        for art_id in 1..=(RECENT_ARTS_CAP as u64 + 3) {
            cultivation.note_recent_art(art_id);
        }
        assert_eq!(cultivation.document.recent_arts.len(), RECENT_ARTS_CAP);
        assert_eq!(
            cultivation.document.recent_arts.first().copied(),
            Some(RECENT_ARTS_CAP as u64 + 3)
        );
        // Training the same book twice is one entry, not two.
        cultivation.note_recent_art(2);
        assert_eq!(
            cultivation
                .document
                .recent_arts
                .iter()
                .filter(|id| **id == 2)
                .count(),
            1
        );
        assert_eq!(cultivation.document.recent_arts.first().copied(), Some(2));
    }

    /// Every won fight pays a purse, and each source pays its own size: the
    /// guardian the richest, a trial floor the leanest, a wanderer in between.
    #[test]
    fn every_fight_pays_a_purse_of_its_own_size() {
        // A save at the first realm's cap, far stronger than the guardian needs:
        // the point is the purse a win pays, not whether it can be won.
        let mut wins = 0;
        for attempt in 0..40_i64 {
            let mut cultivation = Cultivation::new(CultivationDocument {
                growth: threshold_for(LAYERS_PER_STAGE),
                allocated: Attributes::new(2_000, 2_000, 2_000, 2_000),
                ..CultivationDocument::default()
            });
            let before = cultivation.document.stones;
            let Some(outcome) = cultivation.breakthrough(1_000 + attempt * 977) else {
                break;
            };
            if outcome.duel.won {
                wins += 1;
                // The purse is opened at the realm the guardian guarded, which
                // is the one the win entered.
                let (min, max) = GUARDIAN_STONES.at(outcome.target.index() as u8 + 1);
                assert!(
                    outcome
                        .stones
                        .is_some_and(|amount| (min..=max).contains(&amount)),
                    "a won breakthrough pays the guardian's purse: {:?}",
                    outcome.stones
                );
                assert!(cultivation.document.stones > before);
            }
        }
        assert!(wins > 0, "the reference save never won a breakthrough");

        // The tower's purse is the leanest of the four, and only a cleared floor
        // pays it.
        let mut floors = 0;
        for attempt in 0..40_i64 {
            let mut cultivation = a_bare_save_at(30);
            let before = cultivation.document.stones;
            let Some(outcome) = cultivation.trial(1_000 + attempt * 977, 9) else {
                break;
            };
            if outcome.duel.won {
                floors += 1;
                let (min, max) = TRIAL_STONES.at(floor_realm(outcome.floor).index() as u8 + 1);
                assert!(
                    outcome
                        .found
                        .stones
                        .is_some_and(|amount| (min..=max).contains(&amount)),
                    "a cleared floor pays the tower's purse"
                );
                assert!(cultivation.document.stones > before);
            } else {
                assert_eq!(
                    cultivation.document.stones, before,
                    "a failed floor pays nothing at all"
                );
            }
        }
        assert!(
            floors > 0,
            "the reference save never cleared the first floor"
        );
    }

    /// The same road pays more once the road is a deeper one: a 化神 walk is
    /// worth several 炼气 walks, because the shelf it is spent on costs several
    /// times as much there.
    #[test]
    fn a_deeper_realm_pays_a_bigger_purse() {
        let walked = |realm: Realm| {
            let mut paid = Vec::new();
            for step in 0..40_i64 {
                let mut cultivation = Cultivation::new(CultivationDocument {
                    growth: threshold_for(realm.cap_level()),
                    realm: realm.index() as u8,
                    ..CultivationDocument::default()
                });
                let at = 1_000 + step * 100_000;
                let _ = cultivation.explore(at, ExplorationHours::Twelve);
                cultivation.settle_walk(at + ExplorationHours::Twelve.seconds());
                if let Some(stones) = cultivation
                    .view()
                    .last_exploration_found
                    .and_then(|found| found.stones)
                {
                    paid.push(stones);
                }
            }
            assert!(!paid.is_empty(), "no walk in {realm:?} paid a purse");
            paid.iter().sum::<u32>() / paid.len() as u32
        };
        let shallow = walked(Realm::QiRefining);
        let deep = walked(Realm::VoidRefinement);
        assert!(
            deep >= shallow * 3,
            "a 化神 walk paid {deep} against a 炼气 {shallow}"
        );
    }

    /// The trophy is the piece the guardian was wearing, and the 功法 is the one
    /// it was swinging: a won fight hands over what lost to it, rather than
    /// rolling two fresh things that were never on the field.
    #[test]
    fn a_won_breakthrough_hands_over_what_the_guardian_carried() {
        let mut cultivation = at_the_first_cap();
        let guardian = cultivation.guardian().expect("a guardian at the cap");
        let current = cultivation.realm();
        let rival = opponent::guardian(current, guardian.target);
        let mut rng = Rng::seeded(7);
        let fighter = cultivation.fighter(&mut rng);
        let report = duel(fighter, rival.fighter.clone(), &mut rng);
        cultivation.enter_realm(guardian, &rival, report, 1_000);
        let outcome = cultivation
            .last_breakthrough
            .clone()
            .expect("the realm was opened");
        let reward = outcome.reward.expect("a win pays a trophy");
        assert_eq!(
            reward.attributes, rival.gear[0].attributes,
            "the trophy is the piece it wore, not a fresh roll"
        );
        assert_eq!(reward.name, rival.gear[0].name);
        let art_id = rival.art.expect("a guardian always carries an art");
        assert_eq!(
            outcome.granted_art.as_deref(),
            Some(blueprint(art_id).name.as_str()),
            "the book is the one it swung"
        );
        assert!(
            cultivation
                .document
                .arts
                .iter()
                .any(|owned| owned.art_id == art_id),
            "and it lands in the satchel"
        );
    }

    /// A beaten wanderer's gear is its own: whatever a walk hands over is one of
    /// the pieces the loser was wearing.
    #[test]
    fn a_beaten_wanderer_hands_over_its_own_gear() {
        let mut cultivation = a_bare_save_at(30);
        let rival = opponent::wanderer(cultivation.realm(), 0x5741_4E44_4552_0000);
        assert!(!rival.gear.is_empty(), "the probe needs it to wear gear");
        let worn: Vec<Attributes> = rival.gear.iter().map(|item| item.attributes).collect();
        let mut dropped = 0;
        for seed in 0..60_u64 {
            let mut rng = Rng::seeded(seed);
            let found = cultivation.award_wanderer_find(&rival, &mut rng, 1_000);
            if let Some(gear) = found.gear {
                assert!(
                    worn.contains(&gear.attributes),
                    "a walk paid a piece the wanderer was not wearing"
                );
                dropped += 1;
            }
        }
        assert!(dropped > 0, "no wanderer in sixty fights left its gear");
    }

    /// Two rows a day are cut, both inside the band, and the cut is the one the
    /// purchase charges: the button's number is the price, or the badge is a
    /// label on a shelf that never moved.
    #[test]
    fn two_rows_of_the_day_are_on_special() {
        let cultivation = a_bare_save_at(1);
        let shelf = cultivation.shop(9);
        let specials: Vec<&ShopOfferView> = shelf
            .iter()
            .filter(|row| row.discount_permille > 0)
            .collect();
        assert_eq!(specials.len(), 2, "the day marks two rows and no more");
        for row in &specials {
            assert!(
                (600..=800).contains(&row.discount_permille),
                "a special outside 6-8折: {}",
                row.discount_permille
            );
            assert!(
                row.price < row.list_price,
                "a special that costs the list price"
            );
            assert_eq!(
                row.price,
                row.list_price.saturating_mul(row.discount_permille) / 1_000,
                "the row's price is its own cut"
            );
        }
        // The same day shows the same shelf, cut and all: a special a reload
        // could change is not a special.
        assert_eq!(cultivation.shop(9), shelf);
        // And the next day rolls its own.
        let tomorrow = cultivation.shop(10);
        assert_ne!(
            tomorrow
                .iter()
                .map(|row| (row.slot, row.discount_permille))
                .collect::<Vec<_>>(),
            shelf
                .iter()
                .map(|row| (row.slot, row.discount_permille))
                .collect::<Vec<_>>(),
            "every day would carry the same two cuts"
        );
    }

    /// What the purchase charges is what the row asked.
    #[test]
    fn a_special_is_charged_at_its_cut() {
        let mut cultivation = Cultivation::new(CultivationDocument {
            stones: 100_000,
            ..CultivationDocument::default()
        });
        let today = 9;
        let shelf = cultivation.shop(today);
        let row = shelf
            .iter()
            .find(|row| row.discount_permille > 0)
            .expect("a shelf with a special on it");
        let before = cultivation.document.stones;
        cultivation
            .buy_from_shop(row.slot, today, 1_000)
            .expect("the purse covers it");
        let paid = before - cultivation.document.stones;
        assert_eq!(paid, row.price, "the row's number is the price charged");
        assert!(paid < row.list_price, "and the cut was real");
        assert_eq!(
            cultivation
                .view_on(1_000, today)
                .last_refusal
                .map(Refusal::key),
            None,
            "nothing was refused"
        );
    }
}
