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
    BREAKTHROUGH_COOLDOWN_SECONDS, BattleReport, CultivationDocument, Derived, DuelReport,
    EQUIPMENT_DROP_PERMILLE, EXPLORATION_COOLDOWN_SECONDS, EquipSlot, EquipmentRecord,
    ExplorationOutcome, FREE_POINTS_PER_LEVEL, Fighter, Guardian, MAX_LEVEL, OwnedArt, PLAYER_NAME,
    PLOT_COUNT, Rarity, Realm, Rng, SEED_DROP_PERMILLE, SLOT_COUNT, SeedStack,
    VICTORY_SEED_PERMILLE, absence_penalty, art_attributes, art_view,
    arts::{blueprint, train},
    base_attributes,
    battle::{guardian_fighter, guardian_for, resolve, wanderer_fighter},
    curve::{layer_rank, level_for_growth, stage_floor, threshold_for},
    duel::duel,
    explore::{breakthrough_ready, exploration_ready, roll_victory, roll_walk},
    garden::{CropRecord, harvest_points, seed_name, seed_return},
    items::roll_equipment,
    mastery_permille,
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
}

/// What a harvest took from a plot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
}

/// One seed stack, as a surface reads it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeedView {
    pub attribute: Attribute,
    pub tier: u8,
    pub count: u32,
    /// The herb's xianxia-given name, if the stack carries one.
    pub name: Option<String>,
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CropView {
    pub attribute: Attribute,
    pub tier: u8,
    /// The instant it can be picked. A surface subtracts its own clock, so the
    /// countdown keeps moving without the domain being polled for it.
    pub ready_at_unix: i64,
}

/// One plot, as a surface reads it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

/// One thing a walk turned up, as a surface reads it.
///
/// A walk is the only source that pays all three kinds at once, so its find is
/// carried whole rather than reduced to the outcome's name: the panel can say
/// which herb, which piece and which 功法 came of it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExplorationFindView {
    Seed(SeedView),
    Gear(ItemView),
    /// A 功法, by the name it was given.
    Art(String),
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
    /// The guardian offered right now: present only at a realm's last layer.
    pub guardian: Option<GuardianView>,
    pub last_breakthrough: Option<BreakthroughOutcome>,
    pub last_absence: Option<AbsenceCharge>,
    pub last_drop: Option<DropView>,
    /// The seed the most recent harvest gave back, if it gave one.
    pub last_seed_return: Option<SeedView>,
    /// Every 功法 owned, with the blueprint folded in.
    pub arts: Vec<ArtView>,
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
}

/// How often a lost breakthrough costs a piece of gear, in parts per thousand.
const LOST_GEAR_PERMILLE: u64 = 250;
/// How often a won breakthrough pays gear or a seed on top of its 功法.
const VICTORY_EXTRA_PERMILLE: u64 = 400;

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
    /// How many breakthroughs have been fought. A surface watches this rather
    /// than the report: a pomodoro tick bumps the snapshot revision every
    /// second, so "the revision moved" cannot tell a new fight from a running
    /// timer.
    breakthrough_attempts: u32,
    /// How many walks have been taken, for the same reason.
    explorations: u32,
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
            breakthrough_attempts: 0,
            explorations: 0,
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
    pub fn art_attributes(&self) -> Attributes {
        art_attributes(&self.document.arts)
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
            asked = asked.saturating_add(absence_penalty(self.document.missed_streak, stage));
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
        let growth = super::session_growth(session_number, stage);
        self.document.growth = self.document.growth.saturating_add(growth);
        self.document.last_active_day = Some(today_number);
        self.document.missed_streak = 0;
        self.document.charged_through_day = Some(today_number);
        self.note_peak_level();
        // The session also trains whatever 功法 is selected: the pomodoro is the
        // only honest measure of time spent training, so learning rides the same
        // event that pays the growth.
        self.train_selected(now_unix);
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
        self.last_training = Some(TrainingView {
            art_id,
            name: blueprint(art_id).name,
            outcome,
            rank,
        });
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
    pub fn grant_art(&mut self, grade: ArtGrade, now_unix: i64) -> Option<ArtView> {
        let mut rng = self.rng(now_unix);
        for grade in ArtGrade::ALL.iter().rev().skip_while(|g| **g != grade) {
            let unowned: Vec<u64> = (1..=ART_COUNT)
                .filter(|id| {
                    blueprint(*id).grade == *grade
                        && !self.document.arts.iter().any(|art| art.art_id == *id)
                })
                .collect();
            if unowned.is_empty() {
                continue;
            }
            let art_id = unowned[rng.below(unowned.len() as u64) as usize];
            self.document.arts.push(OwnedArt {
                art_id,
                rank: 1,
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

    /// Remember a new high-water mark for the level.
    ///
    /// The free points are paid against this rather than the live level, so a
    /// missed week lowers the ladder but never the points already earned.
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
            };
        }
        let mut stack = SeedStack::single(attribute, tier);
        stack.name = Some(seed_name(rng));
        let view = SeedView {
            attribute: stack.attribute,
            tier: stack.tier,
            count: stack.count,
            name: stack.name.clone(),
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
        let Some(plot) = self.document.plots.iter_mut().find(|plot| plot.is_empty()) else {
            return false;
        };
        plot.crop = Some(CropRecord::planted(attribute, tier, now_unix));
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
            let crop = plot.crop?;
            if !crop.is_ready(now_unix) {
                return None;
            }
            plot.crop = None;
            crop
        };
        crop.attribute
            .add_to(&mut self.document.crop_bonus, harvest_points(crop.tier));
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
        });
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
        let duel = duel(self.fighter(), guardian_fighter(current), &mut rng);
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
            };
            self.last_breakthrough = Some(outcome.clone());
            return Some(outcome);
        }
        self.enter_realm(guardian, duel, now_unix);
        self.last_breakthrough.clone()
    }

    /// The cultivator as a fighter, with the 外功 they would use.
    pub fn fighter(&self) -> Fighter {
        let derived = self.attributes().derived();
        let art = self.strike();
        Fighter::from_derived(PLAYER_NAME.to_owned(), derived, art)
    }

    /// The 外功 the cultivator brings to a fight: the one being trained if it is
    /// an 外功, otherwise the deepest one owned.
    fn strike(&self) -> Option<ArtStrike> {
        let arts: Vec<(u64, u8)> = self
            .document
            .arts
            .iter()
            .filter(|owned| blueprint(owned.art_id).kind == ArtKind::External)
            .map(|owned| (owned.art_id, owned.rank))
            .collect();
        let pick = self
            .document
            .selected_art
            .filter(|id| arts.iter().any(|(art_id, _)| art_id == id))
            .or_else(|| {
                arts.iter()
                    .max_by_key(|(_, rank)| *rank)
                    .map(|(art_id, _)| *art_id)
            })?;
        let art = blueprint(pick);
        let rank = arts
            .iter()
            .find(|(art_id, _)| *art_id == pick)
            .map_or(1, |(_, rank)| *rank);
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
    fn enter_realm(&mut self, guardian: Guardian, duel: DuelReport, now_unix: i64) {
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
        let reward = self.push_equipment(target.index() as u8 + 1, &mut rng);
        // A win always pays a 功法 — that is the promise the fight is made for —
        // and may pay gear or a seed besides.
        let grade = art_grade_for_realm(target);
        let granted_art = self.grant_art(grade, now_unix).map(|art| art.name);
        if rng.below(1_000) < VICTORY_EXTRA_PERMILLE {
            let tier = target.index() as u8 + 1;
            let _ = self.push_equipment(tier, &mut rng);
        }
        if rng.below(1_000) < VICTORY_SEED_PERMILLE {
            let attribute = Attribute::from_index(rng.below(Attribute::ALL.len() as u64) as usize);
            self.push_seed(attribute, target.index() as u8 + 1, &mut rng);
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
        };
        self.last_breakthrough = Some(outcome);
    }

    /// Walk out of the door once every twelve hours.
    ///
    /// Returns `None` while the walk is still on cooldown. A walk that meets
    /// someone settles that fight the same way a breakthrough does, and losing it
    /// costs nothing but the walk.
    pub fn explore(&mut self, now_unix: i64) -> Option<ExplorationOutcome> {
        if !exploration_ready(self.document.last_exploration_unix, now_unix) {
            return None;
        }
        self.document.last_exploration_unix = Some(now_unix);
        // Counted here rather than on the outcome: a walk that found nothing is
        // still a walk a surface has not shown yet.
        self.explorations = self.explorations.saturating_add(1);
        let mut rng = self.rng(now_unix);
        let mut outcome = roll_walk(&mut rng);
        let tier = self.realm().index() as u8 + 1;
        let mut found = None;
        match outcome {
            ExplorationOutcome::Seed => {
                let attribute =
                    Attribute::from_index(rng.below(Attribute::ALL.len() as u64) as usize);
                found = Some(ExplorationFindView::Seed(
                    self.push_seed(attribute, tier, &mut rng),
                ));
            }
            ExplorationOutcome::Gear => {
                let item = self.push_equipment(tier, &mut rng);
                found = Some(ExplorationFindView::Gear(self.item_view(&item)));
            }
            ExplorationOutcome::Art => {
                let art = self.grant_art(art_grade_for_realm(self.realm()), now_unix);
                found = art.map(|art| ExplorationFindView::Art(art.name));
            }
            ExplorationOutcome::Victory => {
                let duel = duel(self.fighter(), wanderer_fighter(self.realm()), &mut rng);
                outcome = if duel.won {
                    match roll_victory(&mut rng) {
                        ExplorationOutcome::Art => {
                            let art = self.grant_art(art_grade_for_realm(self.realm()), now_unix);
                            found = art.map(|art| ExplorationFindView::Art(art.name));
                            ExplorationOutcome::Victory
                        }
                        ExplorationOutcome::Gear => {
                            let item = self.push_equipment(tier, &mut rng);
                            found = Some(ExplorationFindView::Gear(self.item_view(&item)));
                            ExplorationOutcome::Victory
                        }
                        ExplorationOutcome::Seed => {
                            let attribute = Attribute::from_index(
                                rng.below(Attribute::ALL.len() as u64) as usize,
                            );
                            found = Some(ExplorationFindView::Seed(
                                self.push_seed(attribute, tier, &mut rng),
                            ));
                            ExplorationOutcome::Victory
                        }
                        _ => ExplorationOutcome::Victory,
                    }
                } else {
                    ExplorationOutcome::Defeat
                };
                self.last_exploration_duel = Some(duel);
            }
            ExplorationOutcome::Defeat | ExplorationOutcome::Nothing => {}
        }
        self.last_exploration = Some(outcome);
        self.last_exploration_found = found;
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
    /// [`Self::view_at`], so the cooldowns it reads are the real ones.
    pub fn view(&self) -> CultivationView {
        self.view_at(0)
    }

    /// Read the snapshot a surface renders.
    pub fn view_at(&self, now_unix: i64) -> CultivationView {
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
            derived: attributes.derived(),
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
                .take(PLOT_COUNT)
                .map(|plot| PlotView {
                    crop: plot.crop.map(|crop| CropView {
                        attribute: crop.attribute,
                        tier: crop.tier,
                        ready_at_unix: crop.ready_at_unix,
                    }),
                })
                .collect(),
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
                }),
                Drop::Seed { attribute, tier } => DropView::Seed { attribute, tier },
            }),
            last_seed_return: self.last_seed_return.clone(),
            arts,
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
            explorations: self.explorations,
        }
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
        }
    }
}
