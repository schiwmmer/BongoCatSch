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
    BREAKTHROUGH_COOLDOWN_SECONDS, BattleReport, CultivationDocument, DailyKind, Derived,
    DerivedBreakdown, DuelReport, EQUIPMENT_DROP_PERMILLE, EXPLORATION_COOLDOWN_SECONDS, EquipSlot,
    EquipmentRecord, ExplorationOutcome, FREE_POINTS_PER_LEVEL, Fighter, Guardian, MAX_LEVEL,
    MasteredArtView, OwnedArt, PLAYER_NAME, RECENT_ARTS_CAP, Rarity, Realm, Refusal, Rng,
    SEED_DROP_PERMILLE, SLOT_COUNT, SeedStack, VICTORY_SEED_PERMILLE, absence_penalty,
    art_attributes, art_view,
    arts::{blueprint, mastered_art_attributes, mastered_art_view, train},
    base_attributes,
    battle::{guardian_for, resolve},
    curve::{layer_rank, level_for_growth, stage_floor, threshold_for},
    daily::{
        DAILY_AMBUSH_STONES, DAILY_GIFT_STONES, DAILY_TRINKET_STONES, GiftKind, roll_gift,
        roll_kind,
    },
    duel::{DUEL_RECORD_CAP, DuelRecord, FightKind, duel},
    events::{EVENT_LOG_CAP, EventDetail, EventRecord, SaleKind},
    explore::{
        GUARDIAN_STONES, TRIAL_STONES, WALK_STONES, WANDERER_STONES, WalkRecord,
        breakthrough_ready, exploration_ready, roll_stones, roll_victory, roll_walk,
    },
    garden::{CropRecord, PlotRecord, harvest_points, plot_price, seed_name, seed_return},
    growth_permille,
    items::roll_equipment,
    mastery_permille,
    opponent::{self, Opponent},
    scaled_growth,
    shop::{self, ShopOffer, ShopOfferView},
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Drop {
    Equipment(EquipmentRecord),
    Seed { attribute: Attribute, tier: u8 },
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
            Self::Drained => Refusal::TrialDrained,
            Self::Top => Refusal::TrialTop,
        }
    }

    /// The catalog key saying why the tower is closed.
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
    /// same tier or one higher.
    pub seed: Option<(Attribute, u8)>,
}

/// One owned item, as a surface reads it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemView {
    pub id: u64,
    pub slot: EquipSlot,
    pub tier: u8,
    pub rarity: Rarity,
    pub attributes: Attributes,
    pub power: u32,
    pub equipped: bool,
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
    Seed { attribute: Attribute, tier: u8 },
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
    pub equipped: [Option<ItemView>; SLOT_COUNT],
    pub inventory: Vec<ItemView>,
    pub seeds: Vec<SeedView>,
    pub plots: Vec<PlotView>,
    /// What one more garden plot would cost, and `None` once the garden holds
    /// every plot it can.
    pub next_plot_price: Option<u32>,
    /// The guardian offered right now: present only at a realm's last layer.
    pub guardian: Option<GuardianView>,
    pub last_breakthrough: Option<BreakthroughOutcome>,
    pub last_absence: Option<AbsenceCharge>,
    pub last_drop: Option<DropView>,
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
    /// The 功法 the next finished pomodoro trains.
    pub selected_art: Option<u64>,
    /// What the last finished session did to the 功法 being trained.
    pub last_training: Option<TrainingView>,
    /// The fight the last walk met, if it met one.
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
    /// What the day opened with, for the page and for the popup an ambush earns.
    pub last_daily_event: Option<DailyOutcome>,
    /// How many morning events have fired this session, so a surface can tell
    /// one it has already shown from a new one.
    pub daily_events: u32,
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

/// What the 集市 pays for one seed of that attribute and tier.
fn seed_sell_price(attribute: Attribute, tier: u8) -> u32 {
    shop::ShopOffer::Seed {
        attribute,
        tier,
        name: None,
    }
    .sell_price()
}

/// How often a lost breakthrough costs a piece of gear, in parts per thousand.
const LOST_GEAR_PERMILLE: u64 = 250;

/// The grade of 功法 a realm pays: the deeper the realm, the rarer the art.
fn art_grade_for_realm(realm: Realm) -> ArtGrade {
    match realm.index() {
        0..=1 => ArtGrade::Yellow,
        2..=3 => ArtGrade::Profound,
        4..=5 => ArtGrade::Earth,
        6..=7 => ArtGrade::Heaven,
        _ => ArtGrade::Divine,
    }
}

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
    /// The floor the last trial settled, if one was climbed this session.
    last_trial: Option<TrialOutcome>,
    /// What the day opened with, once it has opened.
    last_daily_event: Option<DailyOutcome>,
    /// How many morning events have fired this session. A surface watches this
    /// rather than the revision, so an ambush opens its fight exactly once.
    daily_events: u32,
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

impl Cultivation {
    /// The state a stored document produces.
    pub fn new(document: CultivationDocument) -> Self {
        Self {
            document: document.normalized(),
            roll_sequence: 0,
            last_absence: None,
            last_drop: None,
            last_breakthrough: None,
            last_seed_return: None,
            last_training: None,
            last_exploration_duel: None,
            last_exploration: None,
            last_exploration_found: None,
            last_trial: None,
            trial_attempts: 0,
            last_daily_event: None,
            daily_events: 0,
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
            total = total.plus(item.attributes());
        }
        total
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

    /// The combat power everything together buys.
    pub fn power(&self) -> u32 {
        self.attributes().power()
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
    pub fn apply_absence(&mut self, today: CalendarDay) -> Option<AbsenceCharge> {
        let last_active = self.document.last_active_day?;
        let charged_through = self.document.charged_through_day.unwrap_or(last_active);
        let first = charged_through.saturating_add(1);
        let last = today.day_number().saturating_sub(1);
        if last < first {
            return None;
        }
        let mut days = 0u32;
        let mut asked = 0u32;
        let stage = self.realm().index() as u32 + 1;
        for _ in first..=last {
            asked = asked.saturating_add(scaled_growth(
                absence_penalty(self.document.missed_streak, stage),
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
        Some(charge)
    }

    /// Fold one finished focus session into the ladder.
    pub fn on_session_completed(
        &mut self,
        now_unix: i64,
        today: CalendarDay,
        _focus_minutes: u32,
    ) -> SessionReward {
        let absence = self.apply_absence(today);
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
        let aptitude = self.attributes().derived().aptitude;
        let mut rng = self.rng(now_unix);
        let Some(art) = self
            .document
            .arts
            .iter_mut()
            .find(|art| art.art_id == art_id)
        else {
            return;
        };
        let outcome = train(art, aptitude, &mut rng);
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
            });
            if self.document.selected_art.is_none() {
                self.document.selected_art = Some(art_id);
            }
            return Some(art_view(
                self.document.arts.last()?,
                self.attributes().derived().aptitude,
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
        });
        if self.document.selected_art.is_none() {
            self.document.selected_art = Some(art_id);
        }
        Some(art_view(
            self.document.arts.last()?,
            self.attributes().derived().aptitude,
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

    /// The one roll a finished session makes: gear, or a seed, or nothing.
    fn roll_drop(&mut self, now_unix: i64) -> Option<Drop> {
        let mut rng = self.rng(now_unix);
        let roll = rng.below(1_000);
        let tier = self.realm().index() as u8 + 1;
        if roll < EQUIPMENT_DROP_PERMILLE {
            let item = self.push_equipment(tier, &mut rng);
            return Some(Drop::Equipment(item));
        }
        if roll < EQUIPMENT_DROP_PERMILLE + SEED_DROP_PERMILLE {
            let attribute = Attribute::from_index(rng.below(Attribute::ALL.len() as u64) as usize);
            self.push_seed(attribute, tier, &mut rng);
            return Some(Drop::Seed { attribute, tier });
        }
        None
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

    /// Own one rolled item.
    fn push_equipment(&mut self, tier: u8, rng: &mut Rng) -> EquipmentRecord {
        let id = self.next_id();
        let item = roll_equipment(id, tier, rng);
        self.document.equipment.push(item.clone());
        item
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

    /// Take one piece off a beaten opponent, or roll one if it wore none.
    fn take_gear(&mut self, rival: &Opponent, tier: u8, rng: &mut Rng) -> EquipmentRecord {
        let dropped = (!rival.gear.is_empty())
            .then(|| rival.gear[rng.below(rival.gear.len() as u64) as usize].clone());
        match dropped {
            Some(item) => self.adopt_equipment(item),
            // A bare-handed opponent still pays: a fight that dropped nothing
            // would read as a bug rather than as a poor opponent.
            None => self.push_equipment(tier, rng),
        }
    }

    /// Take the book a beaten opponent was swinging, or grant one of the grade
    /// its realm keeps.
    ///
    /// The fallback is not a consolation prize: a book the cultivator already
    /// owns cannot be given twice, and a fight that promised a 功法 has to pay
    /// one somehow.
    fn take_art(&mut self, rival: &Opponent, realm: Realm, now_unix: i64) -> Option<String> {
        rival
            .art
            .and_then(|art_id| self.grant_art_book(art_id))
            .map(|art| art.name)
            .or_else(|| {
                self.grant_art(art_grade_for_realm(realm), now_unix)
                    .map(|art| art.name)
            })
    }

    /// Own one seed that already has its name.
    ///
    /// The shop names the herbs it sells so a shelf can be read before buying;
    /// folding a bought seed in through [`Self::push_seed`] would re-roll that name
    /// and the shelf would have promised one herb and delivered another.
    fn push_seed_named(
        &mut self,
        attribute: Attribute,
        tier: u8,
        name: Option<String>,
        rng: &mut Rng,
    ) -> SeedView {
        if let Some(stack) = self
            .document
            .seeds
            .iter_mut()
            .find(|stack| stack.attribute == attribute && stack.tier == tier)
        {
            stack.count = stack.count.saturating_add(1);
            return SeedView {
                attribute: stack.attribute,
                tier: stack.tier,
                count: stack.count,
                name: stack.name.clone(),
                sell_price: seed_sell_price(stack.attribute, stack.tier),
            };
        }
        let mut stack = SeedStack::single(attribute, tier);
        stack.name = name.or_else(|| Some(seed_name(rng)));
        let view = SeedView {
            attribute: stack.attribute,
            tier: stack.tier,
            count: stack.count,
            name: stack.name.clone(),
            sell_price: seed_sell_price(stack.attribute, stack.tier),
        };
        self.document.seeds.push(stack);
        view
    }

    /// Own one seed, folded into an existing stack when there is one.
    ///
    /// Returns the stack as a surface reads it, so a caller that just found the
    /// seed can say which herb it was.
    fn push_seed(&mut self, attribute: Attribute, tier: u8, rng: &mut Rng) -> SeedView {
        if let Some(stack) = self
            .document
            .seeds
            .iter_mut()
            .find(|stack| stack.attribute == attribute && stack.tier == tier)
        {
            stack.count = stack.count.saturating_add(1);
            return SeedView {
                attribute: stack.attribute,
                tier: stack.tier,
                count: stack.count,
                name: stack.name.clone(),
                sell_price: seed_sell_price(stack.attribute, stack.tier),
            };
        }
        let mut stack = SeedStack::single(attribute, tier);
        stack.name = Some(seed_name(rng));
        let view = SeedView {
            attribute: stack.attribute,
            tier: stack.tier,
            count: stack.count,
            name: stack.name.clone(),
            sell_price: seed_sell_price(stack.attribute, stack.tier),
        };
        self.document.seeds.push(stack);
        view
    }

    /// Plant one seed in the first empty plot.
    ///
    /// Returns whether anything was planted: no seed of that kind, or no empty
    /// plot, leaves the garden exactly as it was.
    pub fn plant(&mut self, attribute: Attribute, tier: u8, now_unix: i64) -> bool {
        let Some(stack) =
            self.document.seeds.iter_mut().find(|stack| {
                stack.attribute == attribute && stack.tier == tier && stack.count > 0
            })
        else {
            return false;
        };
        // The plot inherits the herb's name: the seed stack is spent one
        // planting at a time and the name is the only thing that survives it.
        let name = stack.name.clone();
        let Some(plot) = self.document.plots.iter_mut().find(|plot| plot.is_empty()) else {
            return false;
        };
        plot.crop = Some(CropRecord::planted(attribute, tier, name, now_unix));
        stack.count = stack.count.saturating_sub(1);
        self.document.seeds.retain(|stack| stack.count > 0);
        true
    }

    /// Pick a ripe crop, if the plot holds one.
    ///
    /// The harvest pays its attribute points, and then rolls once for a seed:
    /// the same attribute, the same tier or one higher. That roll is what keeps
    /// a garden from running dry, and it is deliberately rare next to the
    /// points the crop already paid.
    pub fn harvest(&mut self, plot: usize, now_unix: i64) -> Option<HarvestOutcome> {
        let crop = {
            let plot = self.document.plots.get_mut(plot)?;
            let ready = plot.crop.as_ref()?.is_ready(now_unix);
            if !ready {
                return None;
            }
            plot.crop.take()?
        };
        let points = harvest_points(crop.tier);
        crop.attribute.add_to(&mut self.document.crop_bonus, points);
        // Counted here rather than on the seed roll: a crop that returned nothing
        // was still pulled, and the 造化 dimension is fed by pulling.
        self.document.harvests = self.document.harvests.saturating_add(1);
        let mut rng = self.rng(now_unix);
        let seed = seed_return(crop.attribute, crop.tier, &mut rng);
        if let Some((attribute, tier)) = seed {
            self.push_seed(attribute, tier, &mut rng);
        }
        self.last_seed_return = seed.map(|(attribute, tier)| SeedView {
            attribute,
            tier,
            count: 1,
            name: None,
            sell_price: seed_sell_price(attribute, tier),
        });
        // The line names the herb and what it paid, because "a crop came up" is
        // a fact the user cannot check against the garden that is now empty.
        self.push_event(
            now_unix,
            EventDetail::Harvested {
                attribute: crop.attribute,
                tier: crop.tier,
                name: crop.name.clone(),
                points,
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
        let duel = duel(fighter, rival.fighter.clone(), &mut rng);
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
                },
            );
            return Some(outcome);
        }
        self.enter_realm(guardian, &rival, duel, now_unix);
        let outcome = self.last_breakthrough.clone();
        // The log is written after the settlement so the line carries the
        // realm that was opened *and* the purse that came with it: the event is
        // one line about one event, not two half-lines about a fight and a buy.
        self.push_event(
            now_unix,
            EventDetail::Breakthrough {
                won: true,
                stage: Some(guardian.target.index() as u8),
                stones: outcome.as_ref().and_then(|outcome| outcome.stones),
            },
        );
        outcome
    }

    /// Write one fought fight into the record, newest first.
    ///
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
        let derived = self.attributes().derived();
        let art = self.strike(rng);
        Fighter::from_derived(PLAYER_NAME.to_owned(), derived, art)
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
        let tier = target.index() as u8 + 1;
        // The trophy is the piece the guardian was wearing, not a fresh roll:
        // what the fight was against and what it leaves behind are one thing.
        let reward = match rival.gear.first() {
            Some(item) => self.adopt_equipment(item.clone()),
            // A guardian met bare-handed (it can happen at the shallow end,
            // where a piece of gear would outweigh the whole body) still pays
            // for the realm it opened.
            None => self.push_equipment(tier, &mut rng),
        };
        // A win always pays a 功法 — that is the promise the fight is made for —
        // and it is the book the guardian was swinging.
        let granted_art = self.take_art(rival, target, now_unix);
        if rng.below(1_000) < VICTORY_SEED_PERMILLE {
            let attribute = Attribute::from_index(rng.below(Attribute::ALL.len() as u64) as usize);
            self.push_seed(attribute, tier, &mut rng);
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
            reward: Some(reward),
            duel,
            lost: None,
            granted_art,
            stones,
        };
        self.last_breakthrough = Some(outcome);
    }

    /// What beating someone on the road pays, handed to the satchel and
    /// reported.
    ///
    /// Shared by the walk and the morning ambush because it is the same
    /// encounter either way: the same weighting, the same tier, the same three
    /// kinds. Two copies of this roll would be two chances for one of them to
    /// drift out of tune with the panel that explains it.
    fn award_wanderer_find(
        &mut self,
        rival: &Opponent,
        rng: &mut Rng,
        now_unix: i64,
    ) -> ExplorationFindView {
        let mut found = ExplorationFindView::default();
        let tier = self.realm().index() as u8 + 1;
        match roll_victory(rng) {
            // What the roll decides is *which* of the beaten fighter's things
            // comes home with you — the things themselves were never a fresh
            // roll, they were on the body that lost.
            ExplorationOutcome::Art => {
                found.art = self.take_art(rival, self.realm(), now_unix);
            }
            ExplorationOutcome::Gear => {
                let item = self.take_gear(rival, tier, rng);
                found.gear = Some(self.item_view(&item));
            }
            ExplorationOutcome::Seed => {
                let attribute =
                    Attribute::from_index(rng.below(Attribute::ALL.len() as u64) as usize);
                found.seed = Some(self.push_seed(attribute, tier, rng));
            }
            _ => {}
        }
        found
    }

    /// Walk out of the door once every ten hours.
    ///
    /// Returns `None` while the walk is still on cooldown, and says why in
    /// [`Self::last_refusal`] so the page can show it: a button that does
    /// nothing has to say that it is cooling down rather than look broken. A
    /// walk that meets someone settles that fight the same way a breakthrough
    /// does, and losing it costs nothing but the walk.
    pub fn explore(&mut self, now_unix: i64) -> Option<ExplorationOutcome> {
        if !exploration_ready(self.document.last_exploration_unix, now_unix) {
            self.note_refusal(Refusal::ExploreCooldown);
            return None;
        }
        self.document.last_exploration_unix = Some(now_unix);
        // Counted here rather than on the outcome: a walk that found nothing is
        // still a walk a surface has not shown yet.
        self.document.explorations = self.document.explorations.saturating_add(1);
        let mut rng = self.rng(now_unix);
        let mut outcome = roll_walk(&mut rng);
        let tier = self.realm().index() as u8 + 1;
        let mut found = ExplorationFindView::default();
        match outcome {
            ExplorationOutcome::Seed => {
                let attribute =
                    Attribute::from_index(rng.below(Attribute::ALL.len() as u64) as usize);
                found.seed = Some(self.push_seed(attribute, tier, &mut rng));
            }
            ExplorationOutcome::Gear => {
                let item = self.push_equipment(tier, &mut rng);
                found.gear = Some(self.item_view(&item));
            }
            ExplorationOutcome::Art => {
                found.art = self
                    .grant_art(art_grade_for_realm(self.realm()), now_unix)
                    .map(|art| art.name);
            }
            ExplorationOutcome::Victory => {
                let fighter = self.fighter(&mut rng);
                // A wanderer is rolled per meeting: nobody promised who the road
                // would send, and its numbers are only ever shown after the fact.
                let rival = opponent::wanderer(self.realm(), rng.below(u64::MAX));
                let duel = duel(fighter, rival.fighter.clone(), &mut rng);
                if duel.won {
                    found = self.award_wanderer_find(&rival, &mut rng, now_unix);
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
            WALK_STONES
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
        Some(outcome)
    }

    /// The floor the next attempt climbs.
    ///
    /// A trial only ever offers the floor above the highest one cleared: the
    /// climb is a frontier, not a menu, so re-fighting ground already held
    /// cannot be used to farm a tier the save has outgrown.
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

    /// Why the tower is closed right now, or `None` when a floor can be climbed.
    ///
    /// Three different walls, and the page prints which one it hit: out on a
    /// walk, out of attempts, or past the last floor. A button that went quiet
    /// without saying which would read as broken.
    pub fn trial_blocked(&self, now_unix: i64, day: i64) -> Option<TrialBlocked> {
        if self.trial_next_floor() > TRIAL_MAX_FLOOR {
            return Some(TrialBlocked::Top);
        }
        if !exploration_ready(self.document.last_exploration_unix, now_unix) {
            return Some(TrialBlocked::Out);
        }
        if self.trial_attempts_left(day) == 0 {
            return Some(TrialBlocked::Drained);
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
        let duel = duel(fighter, rival.fighter.clone(), &mut rng);
        self.record_duel(duel.won, FightKind::Trial, now_unix);
        let mut found = ExplorationFindView::default();
        if duel.won {
            self.document.trial_floor = floor.min(TRIAL_MAX_FLOOR);
            // The floor is a level, so its goods are that level's realm's goods:
            // climbing past your own realm is how the tower pays a tier the
            // garden and the road cannot reach yet.
            let tier = floor_realm(floor).index() as u8 + 1;
            match roll_reward(&mut rng) {
                TrialReward::Seed => {
                    let attribute =
                        Attribute::from_index(rng.below(Attribute::ALL.len() as u64) as usize);
                    found.seed = Some(self.push_seed(attribute, tier, &mut rng));
                }
                TrialReward::Gear => {
                    let item = self.take_gear(&rival, tier, &mut rng);
                    found.gear = Some(self.item_view(&item));
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
                let duel = duel(fighter, rival.fighter.clone(), &mut rng);
                self.record_duel(duel.won, FightKind::Wanderer, now_unix);
                outcome.won = duel.won;
                if duel.won {
                    outcome.found = self.award_wanderer_find(&rival, &mut rng, now_unix);
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
                    outcome.found.seed = Some(self.push_seed(attribute, tier, &mut rng));
                }
                GiftKind::Gear => {
                    let item = self.push_equipment(tier, &mut rng);
                    outcome.found.gear = Some(self.item_view(&item));
                }
            },
            DailyKind::Trinket => {
                let amount = roll_stones(&mut rng, DAILY_TRINKET_STONES, tier);
                self.document.stones = self.document.stones.saturating_add(amount);
                outcome.found.stones = Some(amount);
            }
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
        let aptitude = attributes.derived().aptitude;
        let derived = attributes.derived();
        let arts: Vec<ArtView> = self
            .document
            .arts
            .iter()
            .map(|art| {
                art_view(
                    art,
                    aptitude,
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
        let mut seeds: Vec<SeedView> = self
            .document
            .seeds
            .iter()
            .map(|stack| SeedView {
                attribute: stack.attribute,
                tier: stack.tier,
                count: stack.count,
                name: stack.name.clone(),
                sell_price: seed_sell_price(stack.attribute, stack.tier),
            })
            .collect();
        seeds.sort_by(|left, right| {
            right
                .tier
                .cmp(&left.tier)
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
            power: attributes.power(),
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
                        name: crop.name.clone(),
                        ready_at_unix: crop.ready_at_unix,
                    }),
                })
                .collect(),
            // What the next plot would cost, and nothing once the garden is
            // full: the buy button reads this rather than recomputing the
            // table, so the number on it is the number the domain charges.
            next_plot_price: plot_price(self.document.plots.len()),
            guardian: self.guardian().map(|guardian| GuardianView {
                target: guardian.target,
                power: guardian.power,
            }),
            last_breakthrough: self.last_breakthrough.clone(),
            last_absence: self.last_absence,
            last_drop: self.last_drop.clone().map(|drop| match drop {
                Drop::Equipment(item) => DropView::Equipment(ItemView {
                    id: item.id,
                    slot: item.slot,
                    tier: item.tier,
                    rarity: item.rarity,
                    attributes: item.attributes(),
                    power: item.power(),
                    equipped: false,
                    name: item.name.clone(),
                    sell_price: shop::ShopOffer::Equipment(item.clone()).sell_price(),
                }),
                Drop::Seed { attribute, tier } => DropView::Seed { attribute, tier },
            }),
            last_seed_return: self.last_seed_return.clone(),
            arts,
            mastered_arts,
            selected_art: self.document.selected_art,
            last_training: self.last_training.clone(),
            last_exploration_duel: self.last_exploration_duel.clone(),
            last_exploration: self.last_exploration,
            last_exploration_found: self.last_exploration_found.clone(),
            exploration_ready: exploration_ready(self.document.last_exploration_unix, now_unix),
            breakthrough_ready: breakthrough_ready(
                self.document.breakthrough_blocked_until,
                now_unix,
            ),
            exploration_ready_at: self
                .document
                .last_exploration_unix
                .map(|last| last.saturating_add(EXPLORATION_COOLDOWN_SECONDS)),
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
            last_daily_event: self.last_daily_event.clone(),
            daily_events: self.daily_events,
            next_session_growth: self.next_session_growth(day),
            derived_breakdowns: derived.breakdowns(attributes),
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
            attributes: item.attributes(),
            power: item.power(),
            equipped: self
                .document
                .equipped
                .get(item.slot.index())
                .copied()
                .flatten()
                == Some(item.id),
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
            },
            ShopOffer::Seed { name, tier, .. } => SaleKind::Seed {
                name: name.clone(),
                tier: *tier,
            },
            ShopOffer::Art { art_id } => SaleKind::Art {
                name: Some(blueprint(*art_id).name),
            },
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
                name,
            } => {
                let mut rng = self.rng(now_unix);
                self.push_seed_named(attribute, tier, name, &mut rng);
            }
            ShopOffer::Art { art_id } => {
                // The book that was on the shelf, not another one of its grade:
                // the shelf names the 功法, and buying a different one would be
                // a market that sells what it does not show.
                self.grant_art_book(art_id);
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
                what: SaleKind::Equipment { name: item.name },
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
                what: SaleKind::Art { name: Some(name) },
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
        now_unix: i64,
    ) -> Result<u32, TradeRefusal> {
        let Some(index) = self
            .document
            .seeds
            .iter()
            .position(|stack| stack.attribute == attribute && stack.tier == tier)
        else {
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
                what: SaleKind::Seed { name, tier },
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
        DISCIPLINE_STEP_CAP, DISCIPLINE_STEP_PERMILLE, Discipline, GROWTH_MULTIPLIER_CAP_PERMILLE,
        LAYERS_PER_STAGE, STAGE_COUNT, discipline_bonus, discipline_cap, growth_permille,
        scaled_growth, session_growth, threshold_for,
    };
    use super::*;
    use crate::CalendarDay;

    fn day(number: i64) -> CalendarDay {
        CalendarDay::from_day_number(number)
    }

    /// A cultivator at the first realm's last layer, so a guardian is offered.
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
            cultivation.explore(1_000 + step * EXPLORATION_COOLDOWN_SECONDS);
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
        let charged = trained.apply_absence(day(21)).expect("a day was charged");
        let flat = bare.apply_absence(day(21)).expect("a day was charged");
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
            let _ = cultivation.explore(100_000 + step * EXPLORATION_COOLDOWN_SECONDS);
        }
        // Fought last, so the guardian's line is the newest one: a window of eight
        // would otherwise push it out, which is the window working, not failing.
        let _ = cultivation.breakthrough(100_000 + 60 * EXPLORATION_COOLDOWN_SECONDS);
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
        let _ = cultivation.breakthrough(1_000);
        assert_eq!(
            discipline_bonus(Discipline::Fight, 0, 0, cultivation.realm()),
            0,
            "a loss trains nothing"
        );

        for step in 0..40 {
            let _ = cultivation.explore(100_000 + step * EXPLORATION_COOLDOWN_SECONDS);
        }
        let view = cultivation.view();
        let won = view
            .duel_record
            .iter()
            .filter(|line| line.kind == FightKind::Wanderer && line.won)
            .count() as u32;
        assert!(won > 0, "forty walks should have met someone: {view:?}");
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
        cultivation.document.last_exploration_unix = Some(1_000);
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
        cultivation.document.last_exploration_unix = None;
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
            let item =
                crate::cultivation::items::roll_equipment(id, realm.index() as u8 + 1, &mut rng);
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
            },
            OwnedArt {
                art_id: 7,
                rank: 1,
                progress: 0,
                outcome: None,
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
                let _ = cultivation.explore(1_000 + step * 100_000);
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
