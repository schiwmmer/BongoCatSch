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
    Attribute, Attributes, BattleReport, CultivationDocument, EQUIPMENT_DROP_PERMILLE, EquipSlot,
    EquipmentRecord, FREE_POINTS_PER_LEVEL, Guardian, LAYERS_PER_STAGE, MAX_LEVEL, PLOT_COUNT,
    Rarity, Realm, Rng, SEED_DROP_PERMILLE, SLOT_COUNT, SeedStack, absence_penalty,
    base_attributes,
    battle::{guardian_for, resolve},
    curve::{level_for_growth, stage_floor, threshold_for},
    garden::{CropRecord, harvest_points, seed_name, seed_return},
    items::roll_equipment,
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

/// Everything a surface reads in one value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CultivationView {
    pub level: u16,
    pub realm: Realm,
    /// The layer inside the realm, one-based.
    pub layer: u16,
    pub growth: u32,
    /// The growth the current level opened at.
    pub floor: u32,
    /// The growth the next level needs, or `None` at the realm's cap or the top
    /// of the ladder.
    pub next: Option<u32>,
    /// Level, gear, herbs and the points the user placed together.
    pub attributes: Attributes,
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

    /// Every attribute together: level, gear, herbs and placed points.
    pub fn attributes(&self) -> Attributes {
        self.level_attributes()
            .plus(self.equipment_attributes())
            .plus(self.crop_attributes())
            .plus(self.allocated_attributes())
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
        let drop = self.roll_drop(now_unix);
        self.last_drop = drop.clone();
        SessionReward {
            growth,
            absence,
            drop,
        }
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
    fn push_seed(&mut self, attribute: Attribute, tier: u8, rng: &mut Rng) {
        if let Some(stack) = self
            .document
            .seeds
            .iter_mut()
            .find(|stack| stack.attribute == attribute && stack.tier == tier)
        {
            stack.count = stack.count.saturating_add(1);
            return;
        }
        let mut stack = SeedStack::single(attribute, tier);
        stack.name = Some(seed_name(rng));
        self.document.seeds.push(stack);
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
    pub fn breakthrough(&mut self) -> Option<BreakthroughOutcome> {
        let current = self.realm();
        if self.level() < current.cap_level() {
            return None;
        }
        let guardian = guardian_for(current)?;
        let report = resolve(self.attributes(), guardian);
        if !report.won {
            self.document.breakthrough_failures =
                self.document.breakthrough_failures.saturating_add(1);
            let outcome = BreakthroughOutcome {
                report,
                target: guardian.target,
                reward: None,
            };
            self.last_breakthrough = Some(outcome.clone());
            return Some(outcome);
        }
        self.enter_realm(guardian);
        self.last_breakthrough.clone()
    }

    /// Open the realm the guardian defended and pay its trophy.
    fn enter_realm(&mut self, guardian: Guardian) {
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
        let outcome = BreakthroughOutcome {
            report: BattleReport {
                player_power,
                guardian_power: guardian.power,
                won: true,
            },
            target,
            reward: Some(reward),
        };
        self.last_breakthrough = Some(outcome);
    }

    /// The guardian offered right now, or `None` while the realm still has
    /// layers to climb.
    pub fn guardian(&self) -> Option<Guardian> {
        let realm = self.realm();
        (self.level() >= realm.cap_level())
            .then(|| guardian_for(realm))
            .flatten()
    }

    /// Read the snapshot a surface renders.
    pub fn view(&self) -> CultivationView {
        let realm = self.realm();
        let level = self.level();
        let layer = level - realm.index() as u16 * LAYERS_PER_STAGE;
        let floor = threshold_for(level);
        let next = (level < realm.cap_level()).then(|| threshold_for(level + 1));
        let level_attributes = self.level_attributes();
        let equipment_attributes = self.equipment_attributes();
        let crop_attributes = self.crop_attributes();
        let allocated_attributes = self.allocated_attributes();
        let attributes = level_attributes
            .plus(equipment_attributes)
            .plus(crop_attributes)
            .plus(allocated_attributes);
        let mut equipped: [Option<ItemView>; SLOT_COUNT] = array::from_fn(|_| None);
        let mut inventory = Vec::new();
        for item in &self.document.equipment {
            let view = ItemView {
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
            };
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
            layer,
            growth: self.document.growth,
            floor,
            next,
            attributes,
            level_attributes,
            equipment_attributes,
            crop_attributes,
            allocated_attributes,
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
        }
    }
}
