//! Real-time battles: the 锁妖塔, the 试炼 and the 破境 fought one round per
//! second instead of settled on the click.
//!
//! S5 of `docs/0103-melvor-idle-task-redesign.md` §4, §5.4 and §9. A battle is a
//! [`BattleRun`] in the document — `{ kind, floor, region, start_unix,
//! rounds_done, rounds_total, tick_seconds, autoclimb, preview }` — and
//! [`Cultivation::advance_battle`] folds the elapsed seconds into whole rounds
//! exactly as [`Cultivation::advance_task`] folds them into ticks.
//!
//! What makes it a battle and not a task is the split. The click computes a
//! [`BattlePreview`]: the whole fight and its haul, read off the document but
//! written to nothing. The clock only counts rounds. The last round applies the
//! preview. The judgment is therefore taken once, at the start, and the
//! real-time layer can only decide *when* the already-decided result lands —
//! which is what keeps a battle's outcome identical to the old click-and-settle
//! path ([`Cultivation::trial`], [`Cultivation::proving`] and
//! [`Cultivation::breakthrough`] are now thin preview-then-apply wrappers over
//! the same two halves).
//!
//! Battles do not offline-settle (§5.7): a run left while the app is closed
//! keeps its `rounds_done` and resumes on the next tick, rather than being
//! settled for the whole absence. There is deliberately no offline cap here —
//! the cap a task carries exists because a task is paid per tick and an absence
//! would otherwise pay a week of wages, while a battle is paid once at the end
//! and a late tick only decides when that one payment lands.

use serde::{Deserialize, Serialize};

use super::*;
use crate::cultivation::explore::{
    MAX_LEVEL, PROGRESS_TENTHS_PER_LEVEL, Region, proving_gain_tenths, proving_purse,
    region_requirement, walk_purse,
};
use crate::cultivation::proving::{TRIAL_CHALLENGE_MAX_PERMILLE, TRIAL_CHALLENGE_MIN_PERMILLE};
use crate::{DUEL_ROUND_CAP, ExplorationHours, TRIAL_OPPONENT_NAME};

/// The length of one battle round, in seconds: one.
///
/// §5.4: a fight is a second a blow, and the engine caps a fight at
/// [`DUEL_ROUND_CAP`] blows, so the longest battle on the ladder is thirty
/// seconds. The tempo is fixed at the start and carried with the run, the way a
/// task's is, so a save written mid-fight keeps its own rhythm.
pub const BATTLE_TICK_SECONDS: u32 = 1;

/// Which of the three real-time battles is under way.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum BattleKind {
    /// 锁妖塔: one floor of the tower.
    Trial,
    /// 试炼: one banked chance on the road.
    Proving,
    /// 破境: the guardian at the end of the realm.
    Breakthrough,
}

impl BattleKind {
    /// The catalog key naming this battle.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Trial => "cultivation.battle.trial",
            Self::Proving => "cultivation.battle.proving",
            Self::Breakthrough => "cultivation.battle.breakthrough",
        }
    }

    /// The fight record's kind for a battle of this kind.
    ///
    /// The record is the same window a walk's fight writes into, so a battle is
    /// history rather than a second ledger.
    pub const fn fight_kind(self) -> FightKind {
        match self {
            Self::Trial => FightKind::Trial,
            Self::Proving => FightKind::Wanderer,
            Self::Breakthrough => FightKind::Guardian,
        }
    }
}

/// One seed a battle turned up, as the apply phase needs it: the stack's own
/// key and the name it carries.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SeedReward {
    pub attribute: Attribute,
    pub tier: u8,
    pub grade: SeedGrade,
    /// The name the stack carries once it is in the bag.
    ///
    /// `None` for a seed folded into a stack that has no name of its own — an
    /// old save's unnamed stack. A seed the preview creates is always named,
    /// because [`Cultivation::push_seed_named`] rolls the name at creation.
    #[serde(default)]
    pub name: Option<String>,
}

/// The one-off haul for walking a region end to end, first time only.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FirstClear {
    pub stones: u32,
    pub gear: EquipmentRecord,
}

/// What a battle turned up, reduced to what the apply phase can spend.
///
/// The outcome types a surface reads ([`ExplorationFindView`] and its `ItemView`
/// / `SeedView`) are `Serialize` only and carry derived numbers, so they cannot
/// be persisted. What is persisted instead is the concrete payload: the item
/// itself, the seed's stack key, the 功法's id and the purse. The apply phase
/// rebuilds the view from the bag, so the surface reads exactly what it read
/// before.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct BattleFound {
    #[serde(default)]
    pub seed: Option<SeedReward>,
    #[serde(default)]
    pub gear: Option<EquipmentRecord>,
    /// The 功法 granted, by id; the name is read off the catalog at apply.
    #[serde(default)]
    pub art: Option<u64>,
    #[serde(default)]
    pub stones: Option<u32>,
}

impl BattleFound {
    /// The 功法's name, as the event and the outcome carry it.
    fn art_name(&self) -> Option<String> {
        self.art.map(|id| blueprint(id).name)
    }

    /// The gear's name, as the event carries it.
    fn gear_name(&self) -> Option<String> {
        self.gear.as_ref().and_then(|item| item.name.clone())
    }

    /// The seed's name, as the event carries it.
    fn seed_name(&self) -> Option<String> {
        self.seed.as_ref().and_then(|seed| seed.name.clone())
    }
}

/// What one 锁妖塔 floor's apply phase writes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrialApply {
    /// The local day the attempt belongs to, as stamped on the first floor.
    pub day: i64,
    /// The day's attempt count *after* this floor.
    pub attempts_today: u32,
    /// The save's lifetime floor count *after* this floor.
    pub trials: u32,
    /// The session's floor count *after* this floor.
    pub trial_attempts: u32,
    /// What a warden of this floor prints, carried so the outcome matches the
    /// old path's without recomputing it.
    pub warden_power: u32,
}

/// What one 试炼's apply phase writes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProvingApply {
    /// The banked chances *after* this fight.
    pub tokens: u32,
    /// The session's 试炼 count *after* this fight.
    pub provings: u32,
    /// The opponent's own power, as the outcome reports it.
    pub opponent_power: u32,
    /// How far a win moved the region, in tenths of a percent.
    pub gain: u32,
    /// Whether a 洗髓丹 came of the win.
    pub marrow: bool,
    /// The region's one-off haul, when this win crossed its last level.
    #[serde(default)]
    pub first_clear: Option<FirstClear>,
}

/// A piece of the cultivator's own gear a lost 破境 took.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LostGear {
    /// The item's id, so the apply removes the piece the preview named rather
    /// than whichever piece now sits at its old index.
    pub id: u64,
    pub name: String,
}

/// What a won 破境's apply phase writes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BreakthroughWin {
    /// The full power the cultivator brought, as [`Cultivation::power`] reads
    /// it — the win's report is the panel's number, not the naked attributes'.
    pub player_power: u32,
    pub guardian_power: u32,
    /// The realm the win opens.
    pub target: Realm,
    /// The growth after the realm's inheritance rule.
    pub growth: u32,
    /// The realm after the win.
    pub realm: u8,
    /// Whether a 洗髓丹 came of the win.
    pub marrow: bool,
}

/// What one 破境 attempt's apply phase writes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BreakthroughApply {
    /// The session's attempt count *after* this fight.
    pub attempts: u32,
    /// The naked attribute power [`resolve`] read, which is what a *loss*
    /// reports — the win reports [`BreakthroughWin::player_power`] instead.
    pub player_power: u32,
    pub guardian_power: u32,
    /// The realm that was defended.
    pub target: Realm,
    /// The consecutive-failure count *after* this attempt.
    pub failures: u32,
    /// The cooldown a loss sets, `None` while a win leaves none.
    #[serde(default)]
    pub blocked_until: Option<i64>,
    /// The win's settlement; `None` on a loss.
    #[serde(default)]
    pub win: Option<BreakthroughWin>,
    /// The gear a loss cost, if it cost any.
    #[serde(default)]
    pub lost: Option<LostGear>,
}

/// Which battle's apply payload a preview carries.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum BattleOutcome {
    Trial(TrialApply),
    Proving(ProvingApply),
    Breakthrough(BreakthroughApply),
}

/// The whole of a battle decided at the click: the fight, the haul, the log
/// line and the document writes.
///
/// Persisted inside [`BattleRun`] so a relaunch mid-fight resumes the fight that
/// was already decided rather than deciding a second one.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BattlePreview {
    /// Every blow of the fight, carried whole for the replay.
    pub duel: DuelReport,
    /// Whether the cultivator won.
    pub won: bool,
    #[serde(default)]
    pub found: BattleFound,
    /// The log line the apply phase writes, built now so the log reads what the
    /// fight actually turned up.
    pub event: EventDetail,
    /// The per-kind document writes.
    pub outcome: BattleOutcome,
}

/// One battle under way.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BattleRun {
    /// Which battle it is.
    pub kind: BattleKind,
    /// The floor a 锁妖塔 battle climbs; zero for the other two.
    #[serde(default)]
    pub floor: u16,
    /// The region a 试炼 stands on, by ladder position; zero for the others.
    #[serde(default)]
    pub region: u8,
    /// The instant the clock was last aligned to a whole round.
    ///
    /// Not the instant the battle began: [`Cultivation::advance_battle`] moves it
    /// forward by whole rounds, so the gap to `now` is always under one round.
    pub start_unix: i64,
    /// How many whole rounds have been played.
    pub rounds_done: u16,
    /// How many rounds the fight runs: the fight's own blow count, capped at
    /// [`DUEL_ROUND_CAP`].
    pub rounds_total: u16,
    /// The length of one round, in seconds.
    pub tick_seconds: u32,
    /// Keep climbing the tower after a win until a loss or the day's attempts
    /// run out (§5.4). Only meaningful for [`BattleKind::Trial`].
    #[serde(default)]
    pub autoclimb: bool,
    /// The decided fight, applied on the last round.
    pub preview: BattlePreview,
}

/// A battle whose preview is computed but whose clock has not started.
#[derive(Clone, Debug)]
pub struct PlannedBattle {
    pub kind: BattleKind,
    pub floor: u16,
    pub region: u8,
    pub autoclimb: bool,
    pub preview: BattlePreview,
}

impl PlannedBattle {
    /// How many rounds the fight runs.
    ///
    /// The fight's own blow count, floored at one and capped at the engine's
    /// cap: a fight the engine called at zero blows would never settle, and one
    /// longer than the cap cannot happen.
    pub fn rounds_total(&self) -> u16 {
        let rounds = u16::try_from(self.preview.duel.rounds.len()).unwrap_or(u16::MAX);
        rounds.clamp(1, DUEL_ROUND_CAP as u16)
    }

    /// The run this plan becomes once the clock starts.
    pub fn into_run(self, now_unix: i64) -> BattleRun {
        BattleRun {
            kind: self.kind,
            floor: self.floor,
            region: self.region,
            start_unix: now_unix,
            rounds_done: 0,
            rounds_total: self.rounds_total(),
            tick_seconds: BATTLE_TICK_SECONDS,
            autoclimb: self.autoclimb,
            preview: self.preview,
        }
    }
}

/// The item ids a preview hands out, mirroring [`Cultivation::next_id`] without
/// touching the document.
///
/// A preview cannot push an item — that is the apply phase's work — but it has
/// to know which id the item *will* take, because a second roll in the same
/// fight (a region's first-clear haul after the fight's own gear) reads
/// `next_item_id` after the first. The cursor carries that counter forward so
/// the ids the preview names are the ids the apply phase writes.
struct IdCursor {
    next: u64,
}

impl IdCursor {
    fn new(document: &CultivationDocument) -> Self {
        Self {
            next: document.next_item_id.max(1),
        }
    }

    fn take(&mut self) -> u64 {
        let id = self.next;
        self.next = id.saturating_add(1);
        id
    }
}

/// The piece of gear a beaten opponent hands over, computed without granting it.
///
/// Mirrors [`Cultivation::take_gear`] roll for roll: the opponent's own piece
/// when it wore one, otherwise a fresh roll at its strength. The id comes from
/// `ids`, which is what [`Cultivation::adopt_equipment`] /
/// [`Cultivation::push_equipment`] would have taken.
fn plan_take_gear(
    rival: &Opponent,
    strength: Strength,
    rng: &mut Rng,
    ids: &mut IdCursor,
) -> Option<EquipmentRecord> {
    let dropped = (!rival.gear.is_empty())
        .then(|| rival.gear[rng.below(rival.gear.len() as u64) as usize].clone());
    match dropped {
        Some(mut item) => {
            item.id = ids.take();
            Some(item)
        }
        None => {
            let rarity = crate::cultivation::drop::roll_quality(rng, strength.factor())?;
            let id = ids.take();
            Some(roll_equipment(id, strength.tier(), rarity, rng))
        }
    }
}

impl Cultivation {
    /// The battle under way, if any.
    pub fn battle_run(&self) -> Option<BattleRun> {
        self.document.battle.clone()
    }

    /// Begin a real-time battle: decide the whole fight now, start the clock.
    ///
    /// The gates are the old immediate path's, in the old order — the two home
    /// fights settle a walk that is already home first, then read the day's
    /// attempts / the banked chance / the 12-hour cooldown — plus the walls a
    /// battle adds: the one battle slot, the task slot, and an expedition. The
    /// cultivator is only in one place at a time, so a battle cannot be started
    /// while a task is running or an expedition is out, whichever of the three
    /// was asked for.
    ///
    /// The fight is decided here and the attempt is spent when it settles: the
    /// old path took the count before it rolled, and a loss did not refund it.
    /// A real-time battle keeps that rule — the count is read by the surfaces as
    /// "a report has landed", and the report lands at the end, so taking it
    /// earlier would have them open the previous fight mid-battle.
    pub fn start_battle(
        &mut self,
        kind: BattleKind,
        now_unix: i64,
        day: i64,
        region: Option<u8>,
        autoclimb: bool,
    ) -> Result<BattleRun, Refusal> {
        if self.document.battle.is_some() {
            self.note_refusal(Refusal::BattleRunning);
            return Err(Refusal::BattleRunning);
        }
        if self.document.task.is_some() {
            self.note_refusal(Refusal::TaskRunning);
            return Err(Refusal::TaskRunning);
        }
        if self.document.expedition.is_some() {
            self.note_refusal(Refusal::ExpeditionOut);
            return Err(Refusal::ExpeditionOut);
        }
        let planned = match kind {
            BattleKind::Trial => {
                // A walk that is home is settled first: the courtyard is closed
                // to a walk that is *out*, not to one whose find has not been
                // read yet. The same for the job in hand.
                self.settle_walk(now_unix);
                self.settle_legacy_work(now_unix);
                if let Some(blocked) = self.trial_blocked(now_unix, day) {
                    self.note_refusal(blocked.refusal());
                    return Err(blocked.refusal());
                }
                self.plan_trial(now_unix, day)
                    .expect("the tower's gates were checked above")
            }
            BattleKind::Proving => {
                self.settle_walk(now_unix);
                self.settle_legacy_work(now_unix);
                if let Some(blocked) = self.proving_blocked(now_unix) {
                    self.note_refusal(blocked.refusal());
                    return Err(blocked.refusal());
                }
                let index = region.unwrap_or(self.document.exploration_region);
                if Region::from_index(usize::from(index)).index() > self.realm().index() {
                    self.note_refusal(Refusal::RegionLocked);
                    return Err(Refusal::RegionLocked);
                }
                self.plan_proving(now_unix, index)
            }
            BattleKind::Breakthrough => match self.plan_breakthrough(now_unix) {
                Some(planned) => planned,
                None => {
                    // `plan_breakthrough` is `None` for three walls. The
                    // cooldown is the one a surface can name, so it is read here
                    // rather than collapsing the three into one silence.
                    let refusal = if !breakthrough_ready(
                        self.document.breakthrough_blocked_until,
                        now_unix,
                    ) {
                        Refusal::BreakthroughCooling
                    } else {
                        Refusal::BattleUnavailable
                    };
                    self.note_refusal(refusal);
                    return Err(refusal);
                }
            },
        };
        let autoclimb = planned.kind == BattleKind::Trial && autoclimb;
        let run = BattleRun {
            autoclimb,
            ..planned.into_run(now_unix)
        };
        self.document.battle = Some(run.clone());
        Ok(run)
    }

    /// Settle every whole round the elapsed time owes to the battle under way.
    ///
    /// Returns whether anything moved. The elapsed seconds are read against
    /// [`BattleRun::start_unix`], which the call advances by the settled rounds —
    /// the same idempotency [`Cultivation::advance_task`] uses, so a tick at the
    /// same instant is a no-op.
    ///
    /// A battle is *not* offline-settled (§5.7): nothing here caps the gap, so a
    /// machine closed mid-fight and reopened an hour later finds the fight still
    /// running and finishes it on the first tick. The cap a task carries is
    /// about per-tick wages; a battle pays once, at the end, and a late tick
    /// only decides when that one payment lands.
    pub fn advance_battle(&mut self, now_unix: i64) -> bool {
        let Some(run) = self.document.battle.clone() else {
            return false;
        };
        let unsettled = now_unix.saturating_sub(run.start_unix);
        if unsettled <= 0 {
            return false;
        }
        let tick = i64::from(run.tick_seconds.max(1));
        let due = (unsettled / tick) as u32;
        if due == 0 {
            return false;
        }
        let settled = (u32::from(run.rounds_done) + due).min(u32::from(run.rounds_total));
        // Align to the far edge of the settled rounds: the sub-round remainder
        // stays pending, the way a task's does. Written before the completion
        // check so a run already at its cap (a hand-edited save) settles rather
        // than stranding.
        if settled != u32::from(run.rounds_done) {
            let remainder = unsettled % tick;
            if let Some(current) = self.document.battle.as_mut() {
                current.rounds_done = settled as u16;
                current.start_unix = now_unix - remainder;
            }
        }
        if settled < u32::from(run.rounds_total) {
            return true;
        }
        let run = self
            .document
            .battle
            .take()
            .expect("the battle was read above and is cleared only here");
        let won = run.preview.won;
        self.settle_battle(&run, now_unix);
        // §5.4: a tower click keeps climbing while it wins. The next floor is a
        // new fight with a new roll, exactly as a second click would have been.
        if run.kind == BattleKind::Trial && run.autoclimb && won {
            self.chain_trial_autoclimb(now_unix);
        }
        true
    }

    /// Settle the battle under way now, without waiting for its last round.
    ///
    /// The pre-S5 entries — [`Self::trial`], [`Self::proving`] and
    /// [`Self::breakthrough`] — are thin wrappers that start a battle and then
    /// call this, so the click-and-settle behaviour and the real-time one share
    /// one apply phase and cannot drift apart. The outcome was fixed at the
    /// start, so settling early cannot change it; it only decides that the
    /// decided result lands now. Returns the run that was settled.
    pub fn settle_battle_now(&mut self, now_unix: i64) -> Option<BattleRun> {
        let run = self.document.battle.take()?;
        self.settle_battle(&run, now_unix);
        Some(run)
    }

    /// Climb the next floor after a won tower battle, while the day allows it.
    fn chain_trial_autoclimb(&mut self, now_unix: i64) {
        let day = self.document.trial_day.unwrap_or(0);
        if self.trial_attempts_left(day) == 0 || self.trial_next_floor() > TRIAL_MAX_FLOOR {
            return;
        }
        // The win has just been settled, so the only walls left are the day
        // running out or the top of the tower; either way the climb stops
        // quietly, which is what "until a loss or the day is spent" means.
        if self.trial_blocked(now_unix, day).is_some() {
            return;
        }
        let Some(planned) = self.plan_trial(now_unix, day) else {
            return;
        };
        // `plan_trial` always carries the climb, and only a tower battle is
        // chained, so the next floor keeps climbing the way the first did.
        self.document.battle = Some(planned.into_run(now_unix));
    }

    /// Decide one 锁妖塔 floor without writing anything.
    ///
    /// Roll for roll the old [`Self::trial`] path: the fighter, the warden, the
    /// duel, then — on a win — the finding, the purse, the log line and the
    /// counters. The only difference is that the writes are carried in the
    /// returned payload instead of being applied.
    fn plan_trial(&mut self, now_unix: i64, day: i64) -> Option<PlannedBattle> {
        let floor = self.trial_next_floor();
        // The day is stamped on the first floor of it, so the count the page
        // reads and the count the next attempt spends cannot disagree.
        let (trial_day, attempts_today) = if self.document.trial_day != Some(day) {
            (Some(day), 1)
        } else {
            (
                self.document.trial_day,
                self.document.trial_attempts_today.saturating_add(1),
            )
        };
        let trials = self.document.trials.saturating_add(1);
        let trial_attempts = self.trial_attempts.saturating_add(1);
        let mut rng = self.rng(now_unix);
        let fighter = self.fighter(&mut rng);
        // A floor's warden is drawn from a band around the floor's own rung each
        // attempt: the page prints the band's middle, and the duel settles the
        // draw.
        let rival = opponent::warden(floor, &mut rng);
        let duel = self.duel_with_effects(fighter, rival.fighter.clone(), &mut rng);
        let won = duel.won;
        let mut found = BattleFound::default();
        if won {
            // The warden is the source, so what the floor pays is read off the
            // body that defended it.
            let strength = Strength::of(rival.attributes.power());
            let mut ids = IdCursor::new(&self.document);
            match roll_reward(&mut rng) {
                TrialReward::Seed => {
                    let attribute =
                        Attribute::from_index(rng.below(Attribute::ALL.len() as u64) as usize);
                    let grade = roll_seed_grade(&mut rng);
                    let name = self.plan_seed_name(attribute, strength.tier(), grade, &mut rng);
                    found.seed = Some(SeedReward {
                        attribute,
                        tier: strength.tier(),
                        grade,
                        name,
                    });
                }
                TrialReward::Gear => {
                    found.gear = plan_take_gear(&rival, strength, &mut rng, &mut ids);
                }
                TrialReward::Art => {
                    found.art = self.plan_take_art(&rival, now_unix);
                }
                TrialReward::Nothing => {}
            }
            // The floor's purse, drawn beside the finding and opened at the
            // floor's own realm.
            found.stones = Some(roll_stones(
                &mut rng,
                TRIAL_STONES,
                floor_realm(floor).index() as u8 + 1,
            ));
        }
        let event = EventDetail::Trial {
            floor,
            won,
            art: found.art_name(),
            gear: found.gear_name(),
            seed: found.seed_name(),
            stones: found.stones,
            duel: Some(duel.clone()),
        };
        Some(PlannedBattle {
            kind: BattleKind::Trial,
            floor,
            region: 0,
            autoclimb: true,
            preview: BattlePreview {
                duel,
                won,
                found,
                event,
                outcome: BattleOutcome::Trial(TrialApply {
                    day: trial_day.unwrap_or(day),
                    attempts_today,
                    trials,
                    trial_attempts,
                    warden_power: warden_power(floor),
                }),
            },
        })
    }

    /// Decide one 试炼 without writing anything.
    ///
    /// The caller has already checked the gates; this is the old
    /// [`Self::proving`] path's rolls with the writes carried out.
    fn plan_proving(&mut self, now_unix: i64, region: u8) -> PlannedBattle {
        let region = Region::from_index(usize::from(region));
        let tokens = self.document.proving_tokens.saturating_sub(1);
        let provings = self.provings.saturating_add(1);
        let mut rng = self.rng(now_unix);
        let fighter = self.fighter(&mut rng);
        let level = self
            .cleared_level(region.index())
            .saturating_add(1)
            .min(MAX_LEVEL);
        // The opponent stands where the region's progress stands, multiplied by
        // a draw between one and a half and three.
        let progress = self
            .document
            .region_progress
            .get(region.index())
            .copied()
            .unwrap_or(0);
        let requirement = region_requirement(region, progress * 10);
        let span = TRIAL_CHALLENGE_MAX_PERMILLE - TRIAL_CHALLENGE_MIN_PERMILLE + 1;
        let challenge = TRIAL_CHALLENGE_MIN_PERMILLE + rng.below(u64::from(span)) as u32;
        let target = requirement.saturating_mul(challenge) / 1_000;
        let tier = region.tier();
        let name = TRIAL_OPPONENT_NAME.to_owned();
        let seed = rng.below(u64::MAX);
        let rung = opponent::level_near(target, tier, name.clone(), seed);
        let rival = opponent::opponent(opponent::Stance::Wanderer, rung, tier, name, seed);
        let duel = self.duel_with_effects(fighter, rival.fighter.clone(), &mut rng);
        let won = duel.won;
        let mut found = BattleFound::default();
        let mut ids = IdCursor::new(&self.document);
        let mut moved = None;
        let mut marrow = false;
        let mut first_clear = None;
        if won {
            let strength = Strength::of(rival.attributes.power());
            match roll_reward(&mut rng) {
                TrialReward::Seed => {
                    let attribute =
                        Attribute::from_index(rng.below(Attribute::ALL.len() as u64) as usize);
                    let grade = roll_seed_grade(&mut rng);
                    let name = self.plan_seed_name(attribute, strength.tier(), grade, &mut rng);
                    found.seed = Some(SeedReward {
                        attribute,
                        tier: strength.tier(),
                        grade,
                        name,
                    });
                }
                TrialReward::Gear => {
                    found.gear = plan_take_gear(&rival, strength, &mut rng, &mut ids);
                }
                TrialReward::Art => {
                    found.art = self.plan_take_art(&rival, now_unix);
                }
                TrialReward::Nothing => {}
            }
            // The purse is the road's own for one hour at the region.
            found.stones = Some(roll_stones(&mut rng, proving_purse(region, level), tier));
            marrow = self.plan_marrow(rival.attributes.power(), &mut rng);
            let gain = proving_gain_tenths(&mut rng, self.power(), requirement);
            moved = Some(gain);
            // Crossing the last level pays the one-off haul, the way a walk
            // that reaches the end of the road does. The read is the one the
            // old path made *after* `accumulate_progress` had already moved the
            // region, so the preview works the gain out the same way rather
            // than reading the pre-win level.
            let slot = self
                .document
                .region_progress
                .get(region.index())
                .copied()
                .unwrap_or(0);
            let level_cleared = slot.saturating_add(gain) / PROGRESS_TENTHS_PER_LEVEL;
            let cleared_level = self
                .cleared_level(region.index())
                .saturating_add(u8::try_from(level_cleared).unwrap_or(u8::MAX))
                .min(MAX_LEVEL);
            let already = self
                .document
                .cleared_regions
                .get(region.index())
                .copied()
                .unwrap_or(false);
            if cleared_level >= MAX_LEVEL && !already {
                let purse = walk_purse(region, ExplorationHours::Twelve, MAX_LEVEL).scaled(20_000);
                let stones = roll_stones(&mut rng, purse, region.tier());
                let id = ids.take();
                let gear = roll_equipment(id, region.tier(), Rarity::Divine, &mut rng);
                first_clear = Some(FirstClear { stones, gear });
            }
        }
        let event = EventDetail::Proving {
            won,
            art: found.art_name(),
            gear: found.gear_name(),
            seed: found.seed_name(),
            stones: found.stones,
            progress: moved,
            duel: Some(duel.clone()),
        };
        PlannedBattle {
            kind: BattleKind::Proving,
            floor: 0,
            region: region.index() as u8,
            autoclimb: false,
            preview: BattlePreview {
                duel,
                won,
                found,
                event,
                outcome: BattleOutcome::Proving(ProvingApply {
                    tokens,
                    provings,
                    opponent_power: rival.attributes.power(),
                    gain: moved.unwrap_or(0),
                    marrow,
                    first_clear,
                }),
            },
        }
    }

    /// Decide one 破境 without writing anything.
    ///
    /// `None` where the old [`Self::breakthrough`] returned `None`: the realm is
    /// not at its last layer, the 12-hour cooldown is still running, or the
    /// ladder has no next realm.
    fn plan_breakthrough(&mut self, now_unix: i64) -> Option<PlannedBattle> {
        let current = self.realm();
        if self.level() < current.cap_level() {
            return None;
        }
        if !breakthrough_ready(self.document.breakthrough_blocked_until, now_unix) {
            return None;
        }
        let guardian = guardian_for(current)?;
        // The count is taken here, where a fight is actually joined — the same
        // place the old path took it, so a loss costs the attempt either way.
        let attempts = self.breakthrough_attempts.saturating_add(1);
        let mut rng = self.rng(now_unix);
        let report = resolve(self.attributes(), guardian);
        let fighter = self.fighter(&mut rng);
        // The guardian is the same opponent every time it is met: its power is
        // printed before the fight, so a roll per attempt would make that
        // number a lie.
        let rival = opponent::guardian(current, guardian.target);
        let duel = self.duel_with_effects(fighter, rival.fighter.clone(), &mut rng);
        let won = duel.won;
        let mut found = BattleFound::default();
        let mut ids = IdCursor::new(&self.document);
        let mut apply = BreakthroughApply {
            attempts,
            player_power: report.player_power,
            guardian_power: report.guardian_power,
            target: guardian.target,
            failures: self.document.breakthrough_failures,
            blocked_until: self.document.breakthrough_blocked_until,
            win: None,
            lost: None,
        };
        if won {
            let marrow = self.plan_marrow(guardian.power, &mut rng);
            // `enter_realm` rolls on a clock of its own — `self.rng(0)` — so the
            // preview takes the same roll the old path took, at the same point
            // in the sequence.
            let mut rng = self.rng(0);
            let strength = Strength::of(rival.attributes.power());
            // The trophy is the piece the guardian was wearing, not a fresh
            // roll; a bare-handed guardian still pays at its own strength.
            let reward = match rival.gear.first() {
                Some(item) => {
                    let mut item = item.clone();
                    item.id = ids.take();
                    Some(item)
                }
                None => crate::cultivation::drop::roll_quality(&mut rng, strength.factor()).map(
                    |rarity| {
                        let id = ids.take();
                        roll_equipment(id, strength.tier(), rarity, &mut rng)
                    },
                ),
            };
            found.gear = reward;
            found.art = self.plan_take_art(&rival, now_unix);
            if rng.below(1_000) < VICTORY_SEED_PERMILLE {
                let attribute =
                    Attribute::from_index(rng.below(Attribute::ALL.len() as u64) as usize);
                let grade = roll_seed_grade(&mut rng);
                let name = self.plan_seed_name(attribute, strength.tier(), grade, &mut rng);
                found.seed = Some(SeedReward {
                    attribute,
                    tier: strength.tier(),
                    grade,
                    name,
                });
            }
            found.stones = Some(roll_stones(
                &mut rng,
                GUARDIAN_STONES,
                guardian.target.index() as u8 + 1,
            ));
            // The realm's inheritance rule, read before anything moves: the
            // surplus is the growth past the old realm's cap, taxed at half.
            let old_cap = threshold_for(current.cap_level());
            let new_floor = stage_floor(guardian.target.index());
            let surplus = self.document.growth.saturating_sub(old_cap);
            apply.win = Some(BreakthroughWin {
                player_power: self.power(),
                guardian_power: guardian.power,
                target: guardian.target,
                growth: new_floor.max(old_cap + surplus / 2),
                realm: guardian.target.index() as u8,
                marrow,
            });
        } else {
            apply.failures = self.document.breakthrough_failures.saturating_add(1);
            apply.blocked_until = Some(now_unix.saturating_add(BREAKTHROUGH_COOLDOWN_SECONDS));
            // A loss can cost a piece of gear. One in four, and only one.
            apply.lost = (rng.below(1_000) < LOST_GEAR_PERMILLE)
                .then(|| self.plan_drop_one_item(&mut rng))
                .flatten();
        }
        let event = match apply.win {
            Some(ref win) => EventDetail::Breakthrough {
                won: true,
                stage: Some(win.target.index() as u8),
                stones: found.stones,
                gear: found.gear_name(),
                art: found.art_name(),
                seed: found.seed_name(),
                duel: Some(duel.clone()),
            },
            None => EventDetail::Breakthrough {
                won: false,
                stage: Some(apply.target.index() as u8),
                stones: None,
                gear: None,
                art: None,
                seed: None,
                duel: Some(duel.clone()),
            },
        };
        Some(PlannedBattle {
            kind: BattleKind::Breakthrough,
            floor: 0,
            region: 0,
            autoclimb: false,
            preview: BattlePreview {
                duel,
                won,
                found,
                event,
                outcome: BattleOutcome::Breakthrough(apply),
            },
        })
    }

    /// The name a seed would carry, rolled exactly where [`Self::push_seed`]
    /// rolls it: only when the stack is new.
    fn plan_seed_name(
        &self,
        attribute: Attribute,
        tier: u8,
        grade: SeedGrade,
        rng: &mut Rng,
    ) -> Option<String> {
        let existing = self.document.seeds.iter().find(|stack| {
            stack.herb.is_none()
                && stack.attribute == attribute
                && stack.tier == tier
                && stack.grade == grade
        });
        match existing {
            Some(stack) => stack.name.clone(),
            None => Some(seed_name(rng)),
        }
    }

    /// Whether a won fight leaves a 洗髓丹, rolled on the same odds
    /// [`Self::roll_marrow_from_combat`] rolls.
    fn plan_marrow(&self, rival_power: u32, rng: &mut Rng) -> bool {
        let ppm = crate::cultivation::aptitude::marrow_combat_ppm(rival_power, self.power());
        rng.below(1_000_000) < ppm
    }

    /// The 功法 a beaten opponent would hand over, computed without granting it.
    ///
    /// Mirrors [`Self::take_art`]: the opponent's own book when it carried one
    /// and the cultivator can still take it, otherwise a grade roll and a pick
    /// from that grade's catalogue.
    fn plan_take_art(&mut self, rival: &Opponent, now_unix: i64) -> Option<u64> {
        if let Some(art_id) = rival.art
            && self.plan_keep_art_book(art_id).is_some()
        {
            return Some(art_id);
        }
        let grade = {
            let mut rng = self.rng(now_unix);
            crate::cultivation::arts::roll_art_grade(&mut rng)
        };
        self.plan_grant_art(grade, now_unix).map(|(id, _)| id)
    }

    /// Whether [`Self::keep_art_book`] would hand over `art_id`.
    ///
    /// The two ways it answers `None` are an id outside the catalogue and a
    /// 功法 already learned outright; every other book is takable, whether it is
    /// a spare copy of one in hand or a new one.
    fn plan_keep_art_book(&self, art_id: u64) -> Option<String> {
        if !(1..=ART_COUNT).contains(&art_id) || self.document.mastered_arts.contains(&art_id) {
            return None;
        }
        Some(blueprint(art_id).name)
    }

    /// The 功法 [`Self::grant_art`] would pick, without granting it.
    fn plan_grant_art(&mut self, grade: ArtGrade, now_unix: i64) -> Option<(u64, String)> {
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
            return Some((art_id, blueprint(art_id).name));
        }
        None
    }

    /// The piece of gear a lost 破境 would take, named without removing it.
    fn plan_drop_one_item(&self, rng: &mut Rng) -> Option<LostGear> {
        if self.document.equipment.is_empty() {
            return None;
        }
        let index = rng.below(self.document.equipment.len() as u64) as usize;
        let item = &self.document.equipment[index];
        Some(LostGear {
            id: item.id,
            name: item
                .name
                .clone()
                .unwrap_or_else(|| item.slot.name_key().to_owned()),
        })
    }

    /// Write a decided battle into the document.
    ///
    /// Every write here is one the old immediate path made, in the old order:
    /// the count the fight spends, the fight record, the rewards, the log line
    /// and the last outcome. The only thing that moved is *when* they happen —
    /// on the last round rather than on the click. The count belongs here and
    /// not at the start because the surfaces read it as the signal that a
    /// report has landed: [`Self::last_trial`] and friends are written at the
    /// end too, so a count taken early would have the page open last round's
    /// fight while this one is still on screen.
    fn settle_battle(&mut self, run: &BattleRun, now_unix: i64) {
        let BattlePreview {
            duel,
            won,
            found,
            event,
            outcome,
        } = run.preview.clone();
        match outcome {
            BattleOutcome::Trial(apply) => {
                // The count is taken where the old click took it, and for the
                // same reason: it is what tells a surface a report has landed.
                self.document.trial_day = Some(apply.day);
                self.document.trial_attempts_today = apply.attempts_today;
                self.document.trials = apply.trials;
                self.trial_attempts = apply.trial_attempts;
                self.record_duel(won, FightKind::Trial, now_unix);
                if won {
                    self.document.trial_floor = run.floor.min(TRIAL_MAX_FLOOR);
                }
                self.apply_found(&found);
                self.push_event(now_unix, event);
                self.last_trial = Some(TrialOutcome {
                    floor: run.floor,
                    warden_power: apply.warden_power,
                    duel,
                    found: self.found_view(&found),
                });
            }
            BattleOutcome::Proving(apply) => {
                // The banked chance is spent here, as the old click spent it.
                self.document.proving_tokens = apply.tokens;
                self.provings = apply.provings;
                self.record_duel(won, FightKind::Wanderer, now_unix);
                if won {
                    self.apply_found(&found);
                    if apply.marrow {
                        self.document.marrow_pills = self.document.marrow_pills.saturating_add(1);
                        self.push_event(
                            now_unix,
                            EventDetail::MarrowFound {
                                source: MarrowSource::Combat,
                            },
                        );
                    }
                    self.accumulate_progress(usize::from(run.region), apply.gain);
                    if let Some(ref clear) = apply.first_clear {
                        if let Some(slot) = self
                            .document
                            .cleared_regions
                            .get_mut(usize::from(run.region))
                        {
                            *slot = true;
                        }
                        self.document.stones = self.document.stones.saturating_add(clear.stones);
                        self.push_equipment_fixed(clear.gear.clone());
                        self.push_event(
                            now_unix,
                            EventDetail::RegionCleared {
                                region: run.region,
                                gear: clear.gear.name.clone(),
                                stones: clear.stones,
                            },
                        );
                    }
                }
                self.push_event(now_unix, event);
                self.last_proving = Some(ProvingOutcome {
                    duel,
                    found: self.found_view(&found),
                    opponent_power: apply.opponent_power,
                    region: run.region,
                    progress: if won { Some(apply.gain) } else { None },
                });
            }
            BattleOutcome::Breakthrough(apply) => {
                // The attempt is spent where the old click spent it.
                self.breakthrough_attempts = apply.attempts;
                self.record_duel(won, FightKind::Guardian, now_unix);
                if let Some(ref win) = apply.win {
                    if win.marrow {
                        self.document.marrow_pills = self.document.marrow_pills.saturating_add(1);
                        self.push_event(
                            now_unix,
                            EventDetail::MarrowFound {
                                source: MarrowSource::Combat,
                            },
                        );
                    }
                    self.document.growth = win.growth;
                    self.document.realm = win.realm;
                    self.document.breakthrough_failures = 0;
                    self.note_peak_level();
                    self.apply_found(&found);
                    self.last_breakthrough = Some(BreakthroughOutcome {
                        report: BattleReport {
                            player_power: win.player_power,
                            guardian_power: win.guardian_power,
                            won: true,
                        },
                        target: win.target,
                        reward: found.gear.clone(),
                        duel,
                        lost: None,
                        granted_art: found.art_name(),
                        stones: found.stones,
                        seed: found.seed_name(),
                    });
                } else {
                    self.document.breakthrough_failures = apply.failures;
                    self.document.breakthrough_blocked_until = apply.blocked_until;
                    if let Some(ref lost) = apply.lost {
                        // Removed by id rather than by index: the preview named
                        // the piece, and the bag may have been reordered since.
                        if let Some(index) = self
                            .document
                            .equipment
                            .iter()
                            .position(|item| item.id == lost.id)
                        {
                            let item = self.document.equipment.remove(index);
                            self.document
                                .equipped
                                .iter_mut()
                                .filter(|slot| **slot == Some(item.id))
                                .for_each(|slot| *slot = None);
                        }
                    }
                    self.last_breakthrough = Some(BreakthroughOutcome {
                        report: BattleReport {
                            player_power: apply.player_power,
                            guardian_power: apply.guardian_power,
                            won: false,
                        },
                        target: apply.target,
                        reward: None,
                        duel,
                        lost: apply.lost.as_ref().map(|lost| lost.name.clone()),
                        granted_art: None,
                        stones: None,
                        seed: None,
                    });
                }
                self.push_event(now_unix, event);
            }
        }
    }

    /// Pay a decided haul into the bag.
    ///
    /// The order is the old paths' — the finding, then the purse — and the ids
    /// are the ones the preview named, so the bag ends up exactly as the
    /// click-and-settle path left it.
    fn apply_found(&mut self, found: &BattleFound) {
        if let Some(ref reward) = found.seed {
            // The name travels with the reward, so the stack the preview named
            // is the stack that lands; the throwaway rng is only read when the
            // stack is new *and* unnamed, which the preview never lets happen.
            self.push_seed_named(
                reward.attribute,
                reward.tier,
                reward.grade,
                reward.name.clone(),
                &mut Rng::seeded(0),
            );
        }
        if let Some(ref item) = found.gear {
            self.push_equipment_fixed(item.clone());
        }
        if let Some(art_id) = found.art {
            self.grant_art_book(art_id);
        }
        if let Some(stones) = found.stones {
            self.document.stones = self.document.stones.saturating_add(stones);
        }
    }

    /// Own one item whose id is already decided.
    fn push_equipment_fixed(&mut self, item: EquipmentRecord) -> EquipmentRecord {
        let id = item.id.max(1);
        self.document.next_item_id = self.document.next_item_id.max(id.saturating_add(1));
        let item = EquipmentRecord { id, ..item };
        self.document.equipment.push(item.clone());
        item
    }

    /// The haul as a surface reads it, rebuilt from the bag after the writes.
    fn found_view(&self, found: &BattleFound) -> ExplorationFindView {
        ExplorationFindView {
            seed: found.seed.as_ref().and_then(|reward| {
                self.document
                    .seeds
                    .iter()
                    .find(|stack| {
                        stack.herb.is_none()
                            && stack.attribute == reward.attribute
                            && stack.tier == reward.tier
                            && stack.grade == reward.grade
                    })
                    .map(seed_view)
            }),
            gear: found.gear.as_ref().map(|item| self.item_view(item)),
            art: found.art_name(),
            stones: found.stones,
        }
    }
}
