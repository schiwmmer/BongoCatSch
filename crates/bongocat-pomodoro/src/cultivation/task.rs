//! The repeatable task state: one action the cultivator started and the clock
//! keeps working until they stop it.
//!
//! This is S1+S3 of the Melvor-style redesign
//! (`docs/0103-melvor-idle-task-redesign.md` §4, §5.1, §9): the state machine
//! and the settlement schedule. A task is
//! `{ kind, site/grade, start_unix, ticks_done, tick_seconds }`, it occupies the
//! one task slot, and [`Cultivation::advance_task`] folds the elapsed seconds
//! into whole ticks, paying each one through [`Cultivation::settle_task_tick`].
//! The reward numbers are the §5 formulas per kind: gathering pays
//! [`collect_tick`] (S2), 吐纳 pays [`meditate_tick_growth`] and banks a 试炼
//! chance and a drop every tenth tick (S3), and 打工 pays the §5.3 wage each
//! tick and rolls its 积分 every [`JOB_POINT_EVERY_TICKS`] (S4).
//!
//! 闭关 is the one thing here that is *not* a task: [`SeclusionRun`] runs
//! beside a task rather than in the slot (`docs/0103` §4), on the same tick
//! tempo as 吐纳, and settles its whole run at once with
//! [`SECLUSION_BONUS_PERMILLE`]. It is advanced on the same tick as a task, so a
//! cultivator can gather or work while the sit runs.
//!
//! Offline is capped at [`TASK_OFFLINE_CAP_SECONDS`] and the cap is a hard drop,
//! not a carry: a gap longer than the cap settles the cap and forgets the rest,
//! so a two-week absence cannot be paid in installments by ticking every twelve
//! hours. `ticks_done` and `start_unix` are persisted, which is what makes a
//! repeated [`Cultivation::advance_task`] at the same instant a no-op.

use super::super::jobs::JobKind;
use super::super::{ArtGrade, GROWTH_MULTIPLIER_CAP_PERMILLE, GatherTier, collect_tick};
use super::*;
use serde::{Deserialize, Serialize};

/// The longest one [`Cultivation::advance_task`] will settle, in seconds.
///
/// Twelve hours, the same ceiling the exploration tiers top out at: a task is
/// the real-time layer, and an absence longer than a night is the offline
/// layer's business, not a single settlement's.
pub const TASK_OFFLINE_CAP_SECONDS: i64 = 12 * 60 * 60;

/// A gap longer than this is an absence rather than a stutter, in seconds.
///
/// The worker settles every second while the app is awake, so a gap of a minute
/// can only mean the clock was not being looked at — a sleep, a suspended
/// process, a relaunch. Below it every whole tick is paid in full; at or above
/// it the gap is settled at [`OFFLINE_EFFICIENCY_PERMILLE`].
pub const TASK_ONLINE_GRACE_SECONDS: i64 = 60;

/// What one tick of an absence is worth, in parts per thousand.
///
/// `docs/0104` §2.5: half. The app being open is worth twice the app being
/// closed, which is both the point of leaving it open and what keeps a
/// twelve-hour night inside the 仓库's capacity band (`docs/0104` §4.6).
pub const OFFLINE_EFFICIENCY_PERMILLE: u64 = 500;

/// Growth one 吐纳 tick pays at the first realm, before the 功法 multiplier.
pub const TASK_BASE_GROWTH: u32 = GROWTH_PER_REALM[0];

/// Growth one 吐纳 tick pays at each realm, before the 功法 multiplier.
///
/// `docs/0104` §5.1 replaces the old linear ladder (`2 × (1 + 0.75·realm)`:
/// 2 → 16) with a hand-authored table, 2 → 230. The table is solved from the
/// *days per realm* the 门槛 table (`curve::STAGE_FLOORS`, untouched) already
/// holds: the floors are not geometric — 化神→合道 is a ×3.06 step — so a linear
/// income made the tail unplayable (833 days for 金仙 at the old rate against
/// 116 at the front) while the 12-hour offline 0103 added made the front five
/// times faster. Each entry is the per-tick growth that lands its realm at
/// seven to fifty-eight days of ordinary sitting, with no neighbouring pair more
/// than 1.35 apart.
///
/// The table is indexed by realm, and a hand-edited realm past the ladder reads
/// the last entry rather than nothing.
pub const GROWTH_PER_REALM: [u32; 10] = [2, 3, 6, 8, 15, 35, 55, 88, 142, 230];

/// How many 吐纳 ticks bank one 试炼 chance.
///
/// Ten ticks — a minute at the 6-second tempo — is the training's own faucet
/// (§5.4), paced so a sitting buys a fight every few minutes rather than every
/// breath. The bank is capped at [`TRIAL_CHANCES_CAP`].
pub const TASK_CHANCE_EVERY_TICKS: u64 = 10;

/// The length of one 伐木 or 采石 tick, in seconds.
///
/// `docs/0103` §4: eight seconds, the middle of the four tempos — slower than a
/// job's chore, faster than a breath. Fixed at the start like every task's
/// tempo, and carried on the task so a save written mid-tick keeps its rhythm.
pub const GATHER_TICK_SECONDS: u32 = 8;

/// The length of one 闭关 tick, in seconds: the same tempo as 吐纳.
pub const SECLUSION_TICK_SECONDS: u32 = 6;

/// How many ticks a full 闭关 runs: 25 minutes at [`SECLUSION_TICK_SECONDS`].
pub const SECLUSION_TICKS: u64 = 250;

/// The bonus a settled 闭关 pays on top of its ticks, in parts per thousand.
///
/// Ten percent: the sit keeps the pomodoro's ritual, and its output is the
/// 吐纳 curve's own rather than a second formula (§5.1).
pub const SECLUSION_BONUS_PERMILLE: u32 = 1_100;

/// 灵石 one 打工 tick pays before the grade multiplier, in whole stones.
///
/// `docs/0104` §3 re-anchors the whole economy on the 黄阶 hourly wage of nine
/// hundred 灵石: one stone a tick is `1 × 900 = 900` an hour at 黄阶, and the
/// grade ladder reads 1.6 / 2.8 / 4.5 / 7.2 on top of it. A base of one means a
/// tick's exact wage is no longer a whole number for every grade (玄 is 1.6),
/// so the task carries a thousandth-stone remainder rather than truncating one
/// per tick — see [`Cultivation::pay_work_ticks`].
pub const TASK_JOB_BASE_STONES: u32 = 1;

/// The length of one 打工 tick, in seconds.
///
/// §5.3: four seconds, so an hour is 900 ticks and the grade table reads as a
/// per-hour wage. It is the shortest tempo of the four tasks — a job is a
/// repetitive chore, not a walk — and it is fixed at the start like every
/// task's tempo.
pub const WORK_TICK_SECONDS: u32 = 4;

/// How many 打工 ticks bank one 积分 roll.
///
/// §5.3: a job's lasting gain is a chance rolled every 60 ticks — six minutes
/// at the 4-second tempo — so a shift pays its wage every tick and its point
/// every few minutes rather than once a shift. The roll is read off the tick
/// just paid, so a threshold crossed in two advances is rolled once.
pub const JOB_POINT_EVERY_TICKS: u64 = 60;

/// The four tasks S1 models. 闭关 is deliberately absent here: it is a
/// [`SeclusionRun`], not a task, and does not occupy the slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum TaskKind {
    /// 吐纳: growth, the 修习 half of the ladder.
    Meditate,
    /// 伐木: the forest's resources and 伐木 xp.
    Woodcut,
    /// 采石: the mountain's resources and 采石 xp.
    Stonecut,
    /// 打工: 灵石, at the grade the offer carried.
    Work,
}

impl TaskKind {
    /// The catalog key naming this kind.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Meditate => "cultivation.task.meditate",
            Self::Woodcut => "cultivation.task.woodcut",
            Self::Stonecut => "cultivation.task.stonecut",
            Self::Work => "cultivation.task.work",
        }
    }
}

/// The growth one 吐纳 tick pays, in whole points.
///
/// `docs/0104` §5.1: `round(GROWTH_PER_REALM[realm] × growth_permille / 1000)`.
/// The 功法 multiplier is capped at the same [`GROWTH_MULTIPLIER_CAP_PERMILLE`]
/// [`scaled_growth`] enforces, so a session's 闭关 bonus and a task's tick agree
/// about the ceiling.
///
/// Integer arithmetic in u64, rounded half up: the formula is a design table,
/// and a tick that lands on `.5` is paid the same way every time rather than
/// depending on which side of a float conversion it fell.
pub fn meditate_tick_growth(realm_index: u32, growth_permille: u32) -> u32 {
    let base = GROWTH_PER_REALM[(realm_index as usize).min(GROWTH_PER_REALM.len() - 1)];
    let growth_permille = growth_permille.min(GROWTH_MULTIPLIER_CAP_PERMILLE);
    let scaled = u64::from(base).saturating_mul(u64::from(growth_permille));
    ((scaled + 500) / 1_000) as u32
}

/// The growth `due` whole sit ticks pay, bonus included, rounded half up.
///
/// Pure so the page can print the very number a settled sit pays
/// (`next_session_growth`) without touching the save, and so a 闭关's advance
/// and a 修习 session's settlement cannot drift apart: both are this one
/// arithmetic. The bonus rides the whole sum rather than each tick, which is
/// what makes `250 × 2 × 1100 / 1000` exactly 550.
pub fn sit_growth(per_tick: u32, due: u64) -> u32 {
    ((u64::from(per_tick) * due * u64::from(SECLUSION_BONUS_PERMILLE) + 500) / 1_000) as u32
}

/// One task under way: what it is, where, and how far the clock has carried it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ActiveTask {
    /// Which of the four tasks this is.
    pub kind: TaskKind,
    /// The site a gathering task works; `None` for 吐纳 and 打工.
    #[serde(default)]
    pub site: Option<GatherSite>,
    /// The tier of the gathering point a gathering task works. `Common` for
    /// every save written before S2 and for the non-gathering kinds, which is
    /// what keeps an old save's task loading unchanged.
    #[serde(default)]
    pub tier: GatherTier,
    /// The grade a 打工 task was taken at, as an index into
    /// [`JOB_GRADE_MULTIPLIER_PERMILLE`]; `None` for the others.
    #[serde(default)]
    pub grade: Option<u8>,
    /// The trade a 打工 task was taken at, which is what its 积分 roll is made
    /// on; `None` for the other kinds and for a 打工 task started before S4,
    /// which had no trade to roll and so pays its wage alone.
    #[serde(default)]
    pub job_kind: Option<JobKind>,
    /// The instant the clock was last aligned to a whole tick. Not the instant
    /// the task was started: [`Cultivation::advance_task`] moves it forward by
    /// whole ticks, so the gap to `now` is always under one tick.
    pub start_unix: i64,
    /// How many whole ticks have been paid since the task began. Persisted so a
    /// relaunch cannot pay the same tick twice.
    pub ticks_done: u64,
    /// The sub-stone remainder a 打工 task has banked, in thousandths of a
    /// stone.
    ///
    /// Since [`TASK_JOB_BASE_STONES`] is one, a 玄阶 tick pays 1.6 and a 地阶
    /// 2.8: truncating each tick would eat a third of the 玄阶 wage. The task
    /// banks the exact thousandths and pays whole stones, carrying the rest to
    /// the next tick, so a wage over any stretch is the same as the rate printed
    /// on the board. Zero for every other kind, and for a save written before
    /// the field existed (`serde` default), which lost nothing at the old base.
    #[serde(default)]
    pub stones_remainder_permille: u32,
    /// The length of one tick, in seconds. Fixed at the start and carried with
    /// the task, so a save written mid-tick keeps its own tempo.
    pub tick_seconds: u32,
}

impl ActiveTask {
    /// How far the tick under way has run, in permille (`0..=1000`).
    ///
    /// This is the Melvor-style action bar: it fills from empty to full over one
    /// [`Self::tick_seconds`], and the tick it is filling is the one that pays
    /// when it closes. [`Self::start_unix`] is the last whole-tick edge rather
    /// than the instant the task began (`docs/0103` §4), so the gap to `now` is
    /// the part of the current tick that has already passed — which is what a
    /// surface draws.
    ///
    /// Clamped at both ends: a clock that reads before [`Self::start_unix`] (a
    /// save written mid-tick and read after a relaunch) is empty rather than
    /// negative, and a gap past one whole tick is full rather than wrapping into
    /// the next tick — the domain settles whole ticks before a surface reads, so
    /// a gap that long is a stale reading, not a second tick earned.
    ///
    /// `now_ms` is milliseconds since the Unix epoch — [`Self::start_unix`]
    /// scaled by 1000. Milliseconds and not seconds because the worker settles
    /// on whole seconds: read in whole seconds the bar would step once a second,
    /// and a surface driving it a hundred times a second would repaint the same
    /// value nine times out of ten. The edges are whole seconds, so the fraction
    /// between them is exactly the part of the tick that has passed.
    pub fn progress_permille(&self, now_ms: i64) -> u32 {
        tick_progress_permille(self.start_unix, self.tick_seconds, now_ms)
    }
}

/// How far a whole-tick clock has run, in permille (`0..=1000`).
///
/// One function for every surface that draws this bar: the 吐纳/采集/打工 task
/// bar, a battle's round bar, and the finished-rounds half of the whole-fight
/// bar. Each of those used to carry its own copy of the arithmetic in the page
/// (`web/cultivate.js`, `miniprogram/utils/views.js`) and a copy that drifts is
/// a bar that lies. The wasm bridge exports [`tick_progress_permille`] as
/// `web_task_progress`, so a page asks the kernel rather than re-deriving it.
///
/// [`SeclusionRun`] is the one clock this does not serve: a 闭关 has no bar,
/// only a countdown, and it is read as whole ticks rather than as a filling
/// fraction.
pub fn tick_progress_permille(start_unix: i64, tick_seconds: u32, now_ms: i64) -> u32 {
    // A missing `start_unix` — 0, which is what an old save's absent field
    // deserializes to — reads empty rather than full: the edge is unknown, not
    // infinitely far behind. Without this the clamp would paint a corrupt save
    // as a bar that is permanently one tick done. The mini program's view
    // already reads 0 for the same case, so this is also what keeps the three
    // surfaces agreeing on the one state a user cannot debug.
    if start_unix <= 0 {
        return 0;
    }
    let elapsed = now_ms
        .saturating_sub(start_unix.saturating_mul(1000))
        .max(0) as u64;
    let tick = u64::from(tick_seconds.max(1)).saturating_mul(1000);
    (elapsed.saturating_mul(1000).min(tick.saturating_mul(1000)) / tick).min(1000) as u32
}

/// One 闭关 run: a 25-minute sit that pays like [`SECLUSION_TICKS`] 吐纳 ticks
/// with [`SECLUSION_BONUS_PERMILLE`] on top.
///
/// Deliberately not an [`ActiveTask`]: 闭关 does not occupy the one task slot
/// (`docs/0103` §4), so it can run beside 采集 or 打工. It keeps the same clock
/// shape as a task — [`Self::start_unix`] is the last whole-tick edge, not the
/// instant the sit began — which is what makes a relaunch mid-sit settle only
/// the whole ticks that actually passed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SeclusionRun {
    /// The instant the clock was last aligned to a whole tick.
    pub start_unix: i64,
    /// How many whole ticks have been paid since the sit began. Persisted so a
    /// relaunch cannot pay the same tick twice.
    pub ticks_done: u64,
}

impl Cultivation {
    /// Start a task at the ordinary tier. Refuses while another is running: the
    /// slot is one, and the refusal is recorded so a surface can say "already
    /// working" rather than silently dropping the click.
    ///
    /// This is S1's entry point, kept with its signature so every pre-S2 call
    /// site still compiles: a task started here works [`GatherTier::Common`],
    /// which is what those call sites mean. A gathering task at a deeper tier
    /// goes through [`Self::start_gathering_task`].
    pub fn start_task(
        &mut self,
        kind: TaskKind,
        site: Option<GatherSite>,
        grade: Option<u8>,
        now_unix: i64,
        tick_seconds: u32,
    ) -> Result<ActiveTask, Refusal> {
        self.start_gathering_task(
            kind,
            site,
            GatherTier::Common,
            grade,
            None,
            now_unix,
            tick_seconds,
        )
    }

    /// Start a task at a gathering tier. The tier rides the task so a tick
    /// settles the yield of the point the cultivator actually chose, it is
    /// refused on the same one-slot rule as every other task, and a gathering
    /// point whose unlock level the cultivator has not reached is refused
    /// rather than silently worked at the ordinary tier.
    pub fn start_gathering_task(
        &mut self,
        kind: TaskKind,
        site: Option<GatherSite>,
        tier: GatherTier,
        grade: Option<u8>,
        job_kind: Option<JobKind>,
        now_unix: i64,
        tick_seconds: u32,
    ) -> Result<ActiveTask, Refusal> {
        // A task is hours of the same day as an outing, so the slot is closed
        // while the cultivator is out: a walk, an expedition, or a job a pre-S4
        // save still holds. Each cause keeps its own refusal so the page can say
        // which one, and the one-slot rule below stays `TaskRunning`.
        //
        // S5 adds the battle slot: a fight under way is the cultivator's whole
        // attention, so a task cannot be started beside it either.
        if self.document.battle.is_some() {
            self.note_refusal(Refusal::BattleRunning);
            return Err(Refusal::BattleRunning);
        }
        if self.document.walk.is_some() {
            self.note_refusal(Refusal::TrialOut);
            return Err(Refusal::TrialOut);
        }
        if self.document.expedition.is_some() {
            self.note_refusal(Refusal::ExpeditionOut);
            return Err(Refusal::ExpeditionOut);
        }
        if self.document.work.is_some() {
            self.note_refusal(Refusal::Working);
            return Err(Refusal::Working);
        }
        if self.document.task.is_some() {
            self.note_refusal(Refusal::TaskRunning);
            return Err(Refusal::TaskRunning);
        }
        // A gathering point is a *place*, and a deeper place asks for a deeper
        // hand (`docs/0103` §5.2): a cultivator below the tier's unlock level is
        // refused rather than quietly working a point they have not reached.
        // Only the two gathering kinds carry a trade level to measure — 吐纳 and
        // 打工 take a tier because every task shape has one, and theirs is
        // always 凡, which nothing is gated on.
        //
        // A hand that has not started the trade (level 0) is read as level one
        // here, the same reading `raw_find_tick` gives it when it pays a tick:
        // the §5.2 table's shallowest point asks for level one, and level zero
        // is the bare base rather than a rung below the ladder. Without that, a
        // fresh save could not gather at all.
        let trade_level = match kind {
            TaskKind::Woodcut => self.document.woodcut_level,
            TaskKind::Stonecut => self.document.stonecut_level,
            TaskKind::Meditate | TaskKind::Work => 1,
        };
        if tier.unlock_level() > trade_level.max(1) {
            self.note_refusal(Refusal::TierLocked);
            return Err(Refusal::TierLocked);
        }
        // A zero-second tick would settle an unbounded number of rounds in one
        // advance; a task with no tempo is a caller bug, not a save to migrate.
        assert!(tick_seconds > 0, "a task needs a tick longer than zero");
        let task = ActiveTask {
            kind,
            site,
            tier,
            grade,
            job_kind,
            start_unix: now_unix,
            ticks_done: 0,
            stones_remainder_permille: 0,
            tick_seconds,
        };
        self.document.task = Some(task.clone());
        Ok(task)
    }

    /// Start the 打工 task an offer names.
    ///
    /// The offer's grade and trade ride the task so a tick prices its 灵石 and
    /// rolls its 积分 off the same numbers the board printed, and the slot rule
    /// is [`Self::start_gathering_task`]'s: a cultivator already at a task is
    /// refused rather than silently switched. [`WORK_TICK_SECONDS`] is the §5.3
    /// tempo, fixed here so every 打工 task keeps the board's own rhythm.
    pub fn start_work_task(
        &mut self,
        offer: super::super::JobOffer,
        now_unix: i64,
    ) -> Result<(), Refusal> {
        self.start_gathering_task(
            TaskKind::Work,
            None,
            GatherTier::Common,
            Some(offer.grade.index() as u8),
            Some(offer.kind),
            now_unix,
            WORK_TICK_SECONDS,
        )?;
        Ok(())
    }

    /// The task under way, if any.
    pub fn task(&self) -> Option<ActiveTask> {
        self.document.task.clone()
    }

    /// Stop the task, paying the whole ticks it already finished and dropping
    /// the sub-tick remainder. Returns how many ticks were paid, or `None`
    /// when no task was running.
    pub fn stop_task(&mut self, now_unix: i64) -> Option<u64> {
        let _ = self.document.task.as_ref()?;
        let settled = self.advance_task(now_unix);
        self.document.task = None;
        Some(settled)
    }

    /// Settle every whole tick the elapsed time owes. Returns how many ticks
    /// were paid; zero means nothing was due, and calling it again at the same
    /// instant is a no-op.
    ///
    /// The elapsed seconds are read against [`ActiveTask::start_unix`], which
    /// the call then advances by the settled ticks. That is the idempotency:
    /// the next call at the same `now` sees only the sub-tick remainder. The
    /// offline cap is applied to the gap before the division and the excess
    /// beyond it is dropped, not carried.
    ///
    /// A gap at or above [`TASK_ONLINE_GRACE_SECONDS`] is an absence, and every
    /// tick of it is worth [`OFFLINE_EFFICIENCY_PERMILLE`]: the worker settles
    /// about once a second while the app is awake, so only a clock nobody was
    /// watching can gap that far. The forfeited half is not carried either — the
    /// alignment still moves to the far edge of the whole window, which is what
    /// makes a second call at the same instant a no-op.
    pub fn advance_task(&mut self, now_unix: i64) -> u64 {
        let Some(mut task) = self.document.task.clone() else {
            return 0;
        };
        let unsettled = now_unix.saturating_sub(task.start_unix);
        if unsettled <= 0 {
            return 0;
        }
        let window = unsettled.min(TASK_OFFLINE_CAP_SECONDS);
        // A zero-length tick cannot be settled (`window / 0` panics) and cannot
        // come from a hand-started task; it can only come from a save whose
        // field an older build never wrote, where one second is the harmless
        // reading rather than a crash on launch.
        let tick = i64::from(task.tick_seconds.max(1));
        let whole = (window / tick) as u64;
        if whole == 0 {
            return 0;
        }
        let due = if window >= TASK_ONLINE_GRACE_SECONDS {
            whole.saturating_mul(OFFLINE_EFFICIENCY_PERMILLE) / 1_000
        } else {
            whole
        };
        if due > 0 {
            self.settle_ticks(&mut task, due, now_unix);
        }
        // Align to the far edge of the settled window: the sub-tick remainder
        // stays pending, and whatever lay beyond the cap is forgotten rather
        // than becoming the next call's elapsed time.
        task.start_unix = now_unix - window % tick;
        self.document.task = Some(task);
        due
    }

    /// Pay `due` whole ticks of `task`, oldest first.
    ///
    /// The former path cloned the task out of the document twice per tick and
    /// re-derived the payment each time. A relaunch after twelve hours owes
    /// 10,800 ticks of 打工 — twenty thousand clones to pay one absence.
    ///
    /// 吐纳 and 打工 pay a constant amount per tick, so they are banked as one
    /// segment and only their *thresholds* are visited one by one: 吐纳 banks
    /// its 试炼 chance and drop every tenth tick (`TASK_CHANCE_EVERY_TICKS`),
    /// 打工 rolls its 积分 every sixtieth (`JOB_POINT_EVERY_TICKS`). Visiting
    /// those boundaries in order keeps the rng sequence — and therefore every
    /// roll — identical to the tick-by-tick path.
    ///
    /// 采集 stays tick by tick, deliberately: its yield is read off the trade
    /// level, and that level climbs *inside* the window as the xp lands, so the
    /// per-tick formula is the only thing that reproduces the documented
    /// arithmetic (`docs/0103` §5.2; the 199-stone slice in §9.1 turns on
    /// `bank_collection` paying the old level on the tick that levels up). With
    /// the clones gone that loop is cheap.
    fn settle_ticks(&mut self, task: &mut ActiveTask, due: u64, now_unix: i64) {
        let start = task.ticks_done;
        let end = start.saturating_add(due);
        match task.kind {
            TaskKind::Meditate => {
                let growth = u64::from(self.meditate_growth()).saturating_mul(due);
                let growth = growth.min(u64::from(u32::MAX)) as u32;
                self.document.growth = self.document.growth.saturating_add(growth);
                self.note_peak_level();
                let mut threshold = (start / TASK_CHANCE_EVERY_TICKS + 1) * TASK_CHANCE_EVERY_TICKS;
                while threshold <= end {
                    self.bank_proving_chance();
                    if let Some(drop) = self.roll_drop(now_unix) {
                        self.last_drop = Some(drop);
                    }
                    threshold += TASK_CHANCE_EVERY_TICKS;
                }
                task.ticks_done = end;
            }
            TaskKind::Woodcut | TaskKind::Stonecut => {
                for _ in 0..due {
                    task.ticks_done = task.ticks_done.saturating_add(1);
                    self.settle_task_tick(task, now_unix);
                }
            }
            TaskKind::Work => {
                let grade = task.grade.map_or(ArtGrade::Yellow, |grade| {
                    ArtGrade::ALL[(grade as usize).min(ArtGrade::ALL.len() - 1)]
                });
                self.pay_work_ticks(task, grade, due);
                if let Some(kind) = task.job_kind {
                    let mut threshold = (start / JOB_POINT_EVERY_TICKS + 1) * JOB_POINT_EVERY_TICKS;
                    while threshold <= end {
                        self.roll_work_point(kind, grade, now_unix);
                        threshold += JOB_POINT_EVERY_TICKS;
                    }
                }
                task.ticks_done = end;
            }
        }
    }

    /// Pay `due` whole ticks of a 打工 task, carrying the sub-stone remainder.
    ///
    /// One tick's exact wage is `TASK_JOB_BASE_STONES × grade_permille` in
    /// thousandths of a stone — 1.6 at 玄, 2.8 at 地 — so the task banks the
    /// thousandths and pays whole stones, keeping the rest for the next tick.
    /// The carry rides the task rather than the document because a task's wage is
    /// its own: stopping and starting again must not hand the next grade a
    /// remainder the last one earned.
    fn pay_work_ticks(&mut self, task: &mut ActiveTask, grade: ArtGrade, due: u64) {
        let per_tick = super::super::jobs::job_tick_milli_stones(grade);
        let total = per_tick
            .saturating_mul(due)
            .saturating_add(u64::from(task.stones_remainder_permille));
        let whole = total / 1_000;
        task.stones_remainder_permille = (total % 1_000) as u32;
        self.document.stones = self
            .document
            .stones
            .saturating_add(u32::try_from(whole).unwrap_or(u32::MAX));
    }

    /// The 打工 trade's own faucet: one roll for a lasting gain, at a chance
    /// read off the cultivator's hold on the trade the offer named.
    ///
    /// Shared by the tick path and the batched path so the two cannot disagree
    /// about what a sixtieth tick pays.
    fn roll_work_point(&mut self, kind: super::super::JobKind, grade: ArtGrade, now_unix: i64) {
        let success_rate =
            super::super::jobs::success_rate_permille(kind, self.derived(), self.level(), grade);
        let chance = super::super::jobs::point_chance_permille(success_rate);
        let mut rng = self.rng(now_unix);
        if rng.below(1_000) < chance {
            let slot = super::super::jobs::reward_slot(&mut rng, kind);
            let points = super::super::jobs::points(&mut rng, grade);
            slot.add_to(&mut self.document.job_bonus, points);
        }
    }

    /// The 闭关 under way, if any.
    pub fn seclusion(&self) -> Option<SeclusionRun> {
        self.document.seclusion.clone()
    }

    /// Begin a 闭关. Refuses while one is already running: a cultivator sits one
    /// sit at a time, and the refusal is recorded so a surface can say so rather
    /// than silently dropping the click.
    ///
    /// This does not touch [`Self::document::task`]: the sit is not a task and
    /// does not occupy the slot, which is what lets it run beside 采集 or 打工.
    pub fn start_seclusion(&mut self, now_unix: i64) -> Result<SeclusionRun, Refusal> {
        if self.document.seclusion.is_some() {
            self.note_refusal(Refusal::SeclusionRunning);
            return Err(Refusal::SeclusionRunning);
        }
        let run = SeclusionRun {
            start_unix: now_unix,
            ticks_done: 0,
        };
        self.document.seclusion = Some(run.clone());
        Ok(run)
    }

    /// Settle every whole tick the elapsed time owes to a 闭关. Returns how many
    /// ticks were paid; zero means nothing was due, and calling it again at the
    /// same instant is a no-op.
    ///
    /// A run that reaches [`SECLUSION_TICKS`] ends here rather than waiting for a
    /// stop: 25 minutes is the sit, and the settlement is the whole run's growth
    /// at once. The ticks are capped at what the run has left, so an absence
    /// longer than the sit completes it rather than paying a second sit.
    pub fn advance_seclusion(&mut self, now_unix: i64) -> u64 {
        let Some(run) = self.document.seclusion.clone() else {
            return 0;
        };
        // A hand-edited save can carry a run already at the cap (normalized
        // clamps it there); it is complete, so end it rather than leaving a sit
        // that no tick can ever finish.
        if run.ticks_done >= SECLUSION_TICKS {
            self.finish_seclusion();
            return 0;
        }
        let unsettled = now_unix.saturating_sub(run.start_unix);
        if unsettled <= 0 {
            return 0;
        }
        let tick = i64::from(SECLUSION_TICK_SECONDS);
        let remaining = SECLUSION_TICKS.saturating_sub(run.ticks_done);
        let due = ((unsettled / tick) as u64).min(remaining);
        if due == 0 {
            return 0;
        }
        self.pay_sit_ticks(due);
        let ticks_done = run.ticks_done.saturating_add(due);
        if ticks_done >= SECLUSION_TICKS {
            self.finish_seclusion();
        } else {
            // Align to the far edge of the settled window, exactly as a task
            // does: the sub-tick remainder stays pending for the next call.
            let remainder = unsettled % tick;
            self.document.seclusion = Some(SeclusionRun {
                start_unix: now_unix - remainder,
                ticks_done,
            });
        }
        due
    }

    /// Stop the 闭关 early, paying the whole ticks it finished and dropping the
    /// sub-tick remainder. Returns how many ticks were paid, or `None` when no
    /// sit was running.
    ///
    /// A sit that had already run its 250 ticks is ended by
    /// [`Self::advance_seclusion`] and pays no second chance here; the chance is
    /// paid once per run, by [`Self::finish_seclusion`].
    pub fn stop_seclusion(&mut self, now_unix: i64) -> Option<u64> {
        self.document.seclusion.as_ref()?;
        let settled = self.advance_seclusion(now_unix);
        if self.document.seclusion.is_some() {
            // Still running, so this stop is what ends it: pay the run's chance
            // and clear the state, the same as a completed sit.
            self.finish_seclusion();
        }
        Some(settled)
    }

    /// Pay `due` whole ticks of a sit at the tick's own growth, with the sit's
    /// bonus applied to the whole sum rather than per tick. Returns the growth
    /// paid, so a caller can report it.
    ///
    /// Shared by a 闭关 run's advance and a finished 修习 session, which is the
    /// same sit settled whole: neither gets its own copy of the arithmetic, so
    /// the two can never pay different numbers for the same 25 minutes.
    fn pay_sit_ticks(&mut self, due: u64) -> u32 {
        let growth = sit_growth(self.meditate_growth(), due);
        self.document.growth = self.document.growth.saturating_add(growth);
        self.note_peak_level();
        growth
    }

    /// Bank one 试炼 chance, capped. Shared by a finished 闭关 and a 修习
    /// session, both of which are one sit and so pay one chance.
    fn bank_proving_chance(&mut self) {
        self.document.proving_tokens = self
            .document
            .proving_tokens
            .saturating_add(1)
            .min(TRIAL_CHANCES_CAP);
    }

    /// End the sit and pay its 试炼 chance. Called once per run, whether it
    /// finished on its own at [`SECLUSION_TICKS`] or was stopped early.
    fn finish_seclusion(&mut self) {
        self.document.seclusion = None;
        self.bank_proving_chance();
    }

    /// Settle one finished 修习 session as a whole 闭关.
    ///
    /// A 番茄钟 session and a 闭关 are the same sit (`docs/0103` §4, §5.1): the
    /// session pays [`SECLUSION_TICKS`] 吐纳 ticks with [`SECLUSION_BONUS_PERMILLE`]
    /// on top and the sit's one 试炼 chance, and nothing of the old per-session
    /// growth formula. The session's other work is unchanged — the absence
    /// clock, the day's bookkeeping, the 功法 it trains, its 灵石 and its roll —
    /// because none of those is the sit's output.
    pub fn settle_sit_session(&mut self, now_unix: i64, today: CalendarDay) -> SessionReward {
        let absence = self.apply_absence(today, now_unix);
        let growth = self.pay_sit_ticks(SECLUSION_TICKS);
        self.bank_proving_chance();
        let today_number = today.day_number();
        self.document.last_active_day = Some(today_number);
        self.document.missed_streak = 0;
        self.document.charged_through_day = Some(today_number);
        self.push_event(now_unix, EventDetail::Session { growth });
        self.train_selected(now_unix);
        self.credit_session_earnings();
        let drop = self.roll_drop(now_unix);
        self.last_drop = drop.clone();
        SessionReward {
            growth,
            absence,
            drop,
        }
    }

    /// The growth one 吐纳 tick pays at the cultivator's own realm and 功法.
    fn meditate_growth(&self) -> u32 {
        meditate_tick_growth(
            self.realm().index() as u32,
            self.growth_multiplier_permille(),
        )
    }

    /// Pay one finished tick. Gathering runs the §5.2 per-tick formula through
    /// [`collect_tick`] and banks it the same way a session does; 吐纳 runs the
    /// §5.1 formula and banks its 试炼 chance and drop every tenth tick; 打工
    /// runs the §5.3 wage and rolls its 积分 every [`JOB_POINT_EVERY_TICKS`].
    pub fn settle_task_tick(&mut self, task: &mut ActiveTask, now_unix: i64) {
        match task.kind {
            TaskKind::Meditate => {
                let growth = self.meditate_growth();
                self.document.growth = self.document.growth.saturating_add(growth);
                self.note_peak_level();
                // Every tenth tick is the training's faucet: one 试炼 chance,
                // and one drop roll. `ticks_done` is read off the tick just paid
                // rather than a counter beside it, so a tick settled twice can
                // never bank the threshold twice.
                if task.ticks_done > 0 && task.ticks_done.is_multiple_of(TASK_CHANCE_EVERY_TICKS) {
                    self.bank_proving_chance();
                    // A hit is recorded the way a session's find is: the roll
                    // already wrote its event, and the notice is replaced only
                    // when there is a find to show — a miss must not wipe the
                    // last thing the cultivator actually found.
                    if let Some(drop) = self.roll_drop(now_unix) {
                        self.last_drop = Some(drop);
                    }
                }
            }
            TaskKind::Woodcut | TaskKind::Stonecut => {
                let site = task.site.expect("a gathering task names its site");
                let yield_ = collect_tick(
                    site,
                    task.tier,
                    self.document.woodcut_level,
                    self.document.stonecut_level,
                    self.document.workshop_level,
                    self.document.dojo_level,
                );
                self.bank_collection(site, yield_);
            }
            TaskKind::Work => {
                // The grade is the offer's index into the job table; a task
                // taken without one pays the 黄阶 rate rather than nothing.
                let grade = task.grade.map_or(ArtGrade::Yellow, |grade| {
                    ArtGrade::ALL[(grade as usize).min(ArtGrade::ALL.len() - 1)]
                });
                self.pay_work_ticks(task, grade, 1);
                // Every sixtieth tick is the trade's own faucet: one roll for a
                // lasting gain, at a chance read off the cultivator's hold on
                // the trade the offer named. `ticks_done` is the tick just paid,
                // so a threshold crossed in two advances rolls once and a repeat
                // at the same instant cannot roll it twice. A 打工 task started
                // before S4 carries no trade and pays its wage alone.
                if let Some(kind) = task
                    .job_kind
                    .filter(|_| task.ticks_done.is_multiple_of(JOB_POINT_EVERY_TICKS))
                {
                    self.roll_work_point(kind, grade, now_unix);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::JobOffer;
    use crate::cultivation::resources::{BASE_COMMON_YIELD, resource_cap};

    const TICK: u32 = 6;

    fn start(kind: TaskKind, site: Option<GatherSite>, grade: Option<u8>, now: i64) -> Cultivation {
        let mut cultivation = Cultivation::default();
        cultivation
            .start_task(kind, site, grade, now, TICK)
            .expect("a fresh cultivator starts a task");
        cultivation
    }

    /// Drive the task one tick at a time, the way the worker does while the
    /// app is awake: each call's gap is a single tick, well under the online
    /// grace, so every tick is paid in full.
    fn run_ticks(cultivation: &mut Cultivation, ticks: u64, from: i64) -> i64 {
        let tick = i64::from(cultivation.task().expect("running").tick_seconds);
        let mut now = from;
        for _ in 0..ticks {
            now += tick;
            assert_eq!(cultivation.advance_task(now), 1, "one whole tick");
        }
        now
    }

    #[test]
    fn one_tick_pays_once_and_a_repeat_at_the_same_instant_is_a_no_op() {
        let mut cultivation = start(TaskKind::Meditate, None, None, 1_000);
        assert_eq!(cultivation.document().growth, 0);
        assert_eq!(cultivation.advance_task(1_000 + i64::from(TICK)), 1);
        let paid = cultivation.document().growth;
        assert!(paid > 0, "a tick pays growth");
        assert_eq!(cultivation.task().expect("still running").ticks_done, 1);
        // The same instant again: the clock has already been aligned, so the
        // gap is under one tick and nothing is paid twice.
        assert_eq!(cultivation.advance_task(1_000 + i64::from(TICK)), 0);
        assert_eq!(cultivation.document().growth, paid);
    }

    #[test]
    fn a_half_tick_is_dropped_and_the_next_whole_tick_pays() {
        let mut cultivation = start(TaskKind::Meditate, None, None, 1_000);
        // 1.5 ticks: one is paid, the half is not.
        assert_eq!(cultivation.advance_task(1_000 + i64::from(TICK) + 3), 1);
        let after_one = cultivation.document().growth;
        assert_eq!(cultivation.task().expect("running").ticks_done, 1);
        // The half tick is still pending, so 3 seconds later the second tick
        // lands — the dropped part was only ever the fraction past the second.
        assert_eq!(cultivation.advance_task(1_000 + i64::from(TICK) + 3), 0);
        assert_eq!(cultivation.document().growth, after_one);
        assert_eq!(cultivation.advance_task(1_000 + 2 * i64::from(TICK) + 3), 1);
        assert!(cultivation.document().growth > after_one);
    }

    #[test]
    fn the_current_tick_fills_from_empty_to_full_over_its_own_length() {
        // The action bar is the part of the current tick that has passed. It is
        // read in milliseconds, so a quarter of the way in is exactly 250 and
        // the bar climbs continuously instead of stepping once a second.
        let cultivation = start(TaskKind::Meditate, None, None, 1_000);
        let task = cultivation.task().expect("running");
        let start = 1_000_000;
        let tick = i64::from(TICK) * 1000;
        assert_eq!(task.progress_permille(start), 0, "the tick has not begun");
        assert_eq!(
            task.progress_permille(start + tick / 4),
            250,
            "a quarter of the tick"
        );
        assert_eq!(
            task.progress_permille(start + tick / 2),
            500,
            "half the tick"
        );
        assert_eq!(
            task.progress_permille(start + tick),
            1000,
            "the tick's far edge is full"
        );
    }

    #[test]
    fn a_stale_reading_past_one_tick_is_full_and_never_wraps() {
        // The domain settles whole ticks before a surface reads, so a gap past
        // one tick is a stale clock rather than a second tick earned: the bar
        // stops at full instead of restarting.
        let cultivation = start(TaskKind::Meditate, None, None, 1_000);
        let task = cultivation.task().expect("running");
        assert_eq!(
            task.progress_permille(1_000_000 + 5 * i64::from(TICK) * 1000),
            1000
        );
        // A clock behind the task's own edge is empty, not negative.
        assert_eq!(task.progress_permille(0), 0);
    }

    #[test]
    fn an_absence_longer_than_the_cap_settles_the_cap_and_forgets_the_rest() {
        let mut cultivation = start(TaskKind::Meditate, None, None, 0);
        let tick = i64::from(TICK);
        // Twelve hours of ticks, at the offline half (`docs/0104` §2.5).
        let expected =
            ((TASK_OFFLINE_CAP_SECONDS / tick) as u64) * OFFLINE_EFFICIENCY_PERMILLE / 1_000;
        // Twenty hours away: only the first twelve are paid, at half, once.
        assert_eq!(
            cultivation.advance_task(20 * 60 * 60),
            expected,
            "twelve hours of ticks at half"
        );
        assert_eq!(cultivation.task().expect("running").ticks_done, expected);
        let paid = cultivation.document().growth;
        // The excess past the cap is dropped, not carried: the same instant
        // again pays nothing, and the growth did not double.
        assert_eq!(cultivation.advance_task(20 * 60 * 60), 0);
        assert_eq!(cultivation.document().growth, paid);
    }

    #[test]
    fn a_cap_sized_absence_pays_exactly_that_many_ticks_at_half() {
        let mut cultivation = start(TaskKind::Meditate, None, None, 0);
        let tick = i64::from(TICK);
        let whole = (TASK_OFFLINE_CAP_SECONDS / tick) as u64;
        assert_eq!(
            cultivation.advance_task(TASK_OFFLINE_CAP_SECONDS),
            whole * OFFLINE_EFFICIENCY_PERMILLE / 1_000,
            "twelve hours away is six hours of pay"
        );
        assert_eq!(cultivation.advance_task(TASK_OFFLINE_CAP_SECONDS), 0);
    }

    /// The grace boundary: a gap under a minute is a stutter and every tick is
    /// paid; a gap of a minute is an absence and half of it is.
    #[test]
    fn the_online_grace_is_a_minute_and_half_beyond_it() {
        let mut cultivation = start(TaskKind::Meditate, None, None, 0);
        assert_eq!(cultivation.advance_task(54), 9, "54 秒 = 9 回合，全额");
        // Sixty more seconds owe ten ticks; the absence pays five.
        assert_eq!(cultivation.advance_task(114), 5, "60 秒 = 10 回合，半额");
    }

    #[test]
    fn starting_while_a_task_runs_is_refused_and_the_running_task_is_untouched() {
        let mut cultivation = start(TaskKind::Meditate, None, None, 1_000);
        let running = cultivation.task().expect("running");
        let refused = cultivation.start_task(TaskKind::Work, None, Some(0), 2_000, 4);
        assert_eq!(refused, Err(Refusal::TaskRunning));
        assert_eq!(cultivation.last_refusal, Some(Refusal::TaskRunning));
        assert_eq!(
            cultivation.task(),
            Some(running),
            "the first task is untouched"
        );
    }

    #[test]
    fn stopping_pays_the_finished_ticks_and_clears_the_slot() {
        let mut cultivation = start(TaskKind::Meditate, None, None, 1_000);
        assert_eq!(cultivation.stop_task(1_000 + 3 * i64::from(TICK)), Some(3));
        assert!(cultivation.task().is_none());
        assert_eq!(cultivation.stop_task(1_000 + 4 * i64::from(TICK)), None);
    }

    #[test]
    fn a_gathering_tick_pays_resources_and_a_work_tick_pays_stones() {
        let mut woodcut = start(TaskKind::Woodcut, Some(GatherSite::Forest), None, 0);
        assert_eq!(woodcut.advance_task(i64::from(TICK)), 1);
        assert!(
            woodcut.document().resources.get(Resource::Wood) > 0,
            "a 伐木 tick pays wood"
        );

        let mut work = start(TaskKind::Work, None, Some(0), 0);
        assert_eq!(work.advance_task(i64::from(TICK)), 1);
        assert!(work.document().stones > 0, "a 打工 tick pays stones");
    }

    #[test]
    fn a_tick_whose_find_will_not_fit_reports_what_it_swallowed() {
        // A full purse is the one place a tick's pay is cut down, so the cut is
        // carried out rather than absorbed: the player is told the find was
        // lost, not left staring at a resource that did not move. The default
        // cultivator is 炼气 (realm 0) at 伐木 1 with no buildings, so a 凡木林
        // tick finds [`BASE_COMMON_YIELD`] wood and the 仓库's ceiling is
        // [`resource_cap`] at level zero.
        let cap = resource_cap(0);
        let found = collect_tick(GatherSite::Forest, GatherTier::Common, 1, 1, 0, 0)
            .gained
            .get(Resource::Wood);
        assert_eq!(found, BASE_COMMON_YIELD, "the baseline tick finds two");

        let mut cultivation = start(TaskKind::Woodcut, Some(GatherSite::Forest), None, 0);
        assert_eq!(cultivation.advance_task(i64::from(TICK)), 1);
        let outcome = cultivation
            .last_collect
            .clone()
            .expect("a tick reports its collection");
        assert!(
            outcome.lost.is_empty(),
            "a tick with room under the ceiling loses nothing"
        );

        // Run the task until the purse is full, then one tick more: the find
        // is swallowed whole and the outcome says exactly how much.
        let mut now = i64::from(TICK);
        while cultivation.document().resources.get(Resource::Wood) < cap {
            now += i64::from(TICK);
            cultivation.advance_task(now);
        }
        assert_eq!(cultivation.document().resources.get(Resource::Wood), cap);
        now += i64::from(TICK);
        assert_eq!(cultivation.advance_task(now), 1);
        let outcome = cultivation
            .last_collect
            .expect("a tick reports its collection");
        assert_eq!(outcome.gained.get(Resource::Wood), 0, "no room at all");
        assert_eq!(
            outcome.lost.get(Resource::Wood),
            found,
            "the whole find is reported as lost"
        );
    }

    /// An offer at the given grade, for a 打工 task's tempo and rate.
    fn work_offer(grade: ArtGrade) -> JobOffer {
        JobOffer {
            tier: 1,
            hours: 4,
            grade,
            kind: JobKind::default(),
        }
    }

    #[test]
    fn a_work_task_pays_its_grade_rate_every_tick() {
        // `docs/0104` §3: `1 × 品阶倍率 / 1000` a tick at the 4-second tempo,
        // banked exactly. Five ticks are the smallest number that shows the
        // fractional grades as whole stones: 黄 5, 玄 8, 地 14, 天 20, 神 36.
        let table = [
            (ArtGrade::Yellow, 5),
            (ArtGrade::Profound, 8),
            (ArtGrade::Earth, 14),
            (ArtGrade::Heaven, 22),
            (ArtGrade::Divine, 36),
        ];
        for (grade, five_ticks) in table {
            let mut cultivation = Cultivation::default();
            cultivation
                .start_work_task(work_offer(grade), 0)
                .expect("the job starts");
            let task = cultivation.task().expect("running");
            assert_eq!(task.kind, TaskKind::Work);
            assert_eq!(task.tick_seconds, WORK_TICK_SECONDS);
            assert_eq!(task.grade, Some(grade.index() as u8));
            assert_eq!(task.job_kind, Some(JobKind::default()));
            run_ticks(&mut cultivation, 5, 0);
            assert_eq!(
                cultivation.document().stones,
                five_ticks,
                "{grade:?} pays {five_ticks} over five ticks"
            );
        }
    }

    /// The sub-stone carry: a 玄阶 tick is 1.6, so three ticks are four stones
    /// with eight hundred left over, and two more land the eighth stone exactly.
    #[test]
    fn a_work_tick_carries_the_sub_stone_remainder() {
        let mut cultivation = Cultivation::default();
        cultivation
            .start_work_task(work_offer(ArtGrade::Profound), 0)
            .expect("the job starts");
        run_ticks(&mut cultivation, 3, 0);
        assert_eq!(cultivation.document().stones, 4, "3 × 1.6 = 4.8 → 4 整");
        assert_eq!(
            cultivation
                .task()
                .expect("running")
                .stones_remainder_permille,
            800,
            "余 800‰ 结转"
        );
        run_ticks(&mut cultivation, 2, i64::from(WORK_TICK_SECONDS) * 3);
        assert_eq!(cultivation.document().stones, 8, "5 × 1.6 = 8 整");
        assert_eq!(
            cultivation
                .task()
                .expect("running")
                .stones_remainder_permille,
            0
        );
    }

    /// A full hour is 900 four-second ticks, and the hour pays the tick's
    /// integer rate that many times: the per-hour figure is the per-tick figure
    /// × 900, not the exact product of the multiplier (which truncates).
    #[test]
    fn a_work_task_pays_a_full_hour_at_its_grade_rate() {
        let table = [
            (ArtGrade::Yellow, 900),
            (ArtGrade::Profound, 1_440),
            (ArtGrade::Earth, 2_520),
            (ArtGrade::Heaven, 4_050),
            (ArtGrade::Divine, 6_480),
        ];
        for (grade, per_hour) in table {
            let mut cultivation = Cultivation::default();
            cultivation
                .start_work_task(work_offer(grade), 0)
                .expect("the job starts");
            run_ticks(&mut cultivation, 900, 0);
            assert_eq!(
                cultivation.document().stones,
                per_hour,
                "{grade:?} pays {per_hour} an hour"
            );
        }
    }

    #[test]
    fn a_work_task_rolls_its_points_once_a_threshold() {
        let mut cultivation = Cultivation::default();
        cultivation
            .start_work_task(work_offer(ArtGrade::Yellow), 0)
            .expect("the job starts");
        // Fifty-nine ticks: the wage is paid each time, but no threshold is
        // crossed, so no 积分 roll has happened.
        let now = run_ticks(&mut cultivation, 59, 0);
        assert_eq!(cultivation.roll_sequence, 0, "no roll before the sixtieth");
        // The sixtieth tick crosses it: exactly one roll.
        assert_eq!(
            cultivation.advance_task(now + i64::from(WORK_TICK_SECONDS)),
            1
        );
        assert_eq!(cultivation.roll_sequence, 1, "the threshold rolls once");
        // The next tick is an ordinary one, and the same instant again is a
        // no-op: neither can roll the threshold a second time.
        let now = run_ticks(&mut cultivation, 1, now + i64::from(WORK_TICK_SECONDS));
        assert_eq!(cultivation.advance_task(now), 0);
        assert_eq!(cultivation.roll_sequence, 1);
    }

    #[test]
    fn a_threshold_crossed_in_two_advances_rolls_once() {
        let mut cultivation = Cultivation::default();
        cultivation
            .start_work_task(work_offer(ArtGrade::Yellow), 0)
            .expect("the job starts");
        let now = run_ticks(&mut cultivation, 59, 0);
        assert_eq!(cultivation.roll_sequence, 0);
        // The sixtieth tick lands in the second call; the threshold is read off
        // the tick just paid, so it rolls once rather than once per call.
        assert_eq!(
            cultivation.advance_task(now + i64::from(WORK_TICK_SECONDS)),
            1
        );
        assert_eq!(cultivation.roll_sequence, 1);
    }

    #[test]
    fn a_work_task_without_a_trade_pays_its_wage_alone() {
        // A 打工 task written before S4 has no `job_kind`; it still pays the
        // 黄阶 wage every tick, but there is no trade to roll a gain on.
        let mut cultivation = start(TaskKind::Work, None, Some(0), 0);
        assert_eq!(
            cultivation.task().expect("running").job_kind,
            None,
            "the S1 entry point carries no trade"
        );
        run_ticks(&mut cultivation, 60, 0);
        assert_eq!(
            cultivation.document().stones,
            60 * TASK_JOB_BASE_STONES,
            "the wage is unaffected by the missing trade"
        );
        assert_eq!(cultivation.roll_sequence, 0, "no trade, no roll");
    }

    #[test]
    fn a_gathering_task_settles_its_tier_per_tick() {
        // 灵木林 asks for 伐木 20 (`docs/0104` §4.2), so the hand working it is
        // a level-twenty one.
        let mut cultivation = Cultivation::new(CultivationDocument {
            woodcut_level: 20,
            ..CultivationDocument::default()
        });
        cultivation
            .start_gathering_task(
                TaskKind::Woodcut,
                Some(GatherSite::Forest),
                GatherTier::Spirit,
                None,
                None,
                0,
                TICK,
            )
            .expect("the task starts");
        assert_eq!(
            cultivation.task().expect("running").tier,
            GatherTier::Spirit,
            "the tier rides the task"
        );
        assert_eq!(cultivation.advance_task(i64::from(TICK)), 1);
        // L20 灵木林: 2 × 1.38 × 1.5 = 4.14 → 4.
        assert_eq!(cultivation.document().resources.wood, 4);
    }

    /// A gathering point is a place with an unlock level (`docs/0103` §5.2), so
    /// a cultivator below it is refused rather than silently working a poorer
    /// point: 伐木 1 级进不了灵木林，10 级进得去，30 级进得了混沌神木。
    #[test]
    fn a_tier_above_the_trade_level_is_refused() {
        let start_at = |woodcut_level: u8, stonecut_level: u8| {
            Cultivation::new(CultivationDocument {
                woodcut_level,
                stonecut_level,
                ..CultivationDocument::default()
            })
        };
        let mut cultivation = start_at(1, 1);
        assert_eq!(
            cultivation.start_gathering_task(
                TaskKind::Woodcut,
                Some(GatherSite::Forest),
                GatherTier::Spirit,
                None,
                None,
                0,
                TICK,
            ),
            Err(Refusal::TierLocked),
            "伐木 1 级进不了灵木林"
        );
        assert_eq!(
            cultivation.last_refusal,
            Some(Refusal::TierLocked),
            "the refusal is recorded so the page can say why"
        );
        assert_eq!(
            cultivation.last_refusal.map(Refusal::key),
            Some("cultivation.tier.refuse_locked")
        );
        // The gate is per trade: a 伐木 1 级 hand is still refused the mountain's
        // deeper point, and the refusal leaves the slot free.
        assert_eq!(
            cultivation.start_gathering_task(
                TaskKind::Stonecut,
                Some(GatherSite::Mountain),
                GatherTier::Divine,
                None,
                None,
                0,
                TICK,
            ),
            Err(Refusal::TierLocked),
            "采石 1 级也进不了寒铁窟"
        );
        assert!(
            cultivation.task().is_none(),
            "a refused click starts nothing"
        );

        // 20 级进得了灵木林，80 级进得了混沌神木（`docs/0104` §4.2）。
        let mut trained = start_at(20, 20);
        trained
            .start_gathering_task(
                TaskKind::Woodcut,
                Some(GatherSite::Forest),
                GatherTier::Spirit,
                None,
                None,
                0,
                TICK,
            )
            .expect("伐木 20 级进得了灵木林");
        let mut deepest = start_at(80, 80);
        deepest
            .start_gathering_task(
                TaskKind::Woodcut,
                Some(GatherSite::Forest),
                GatherTier::Chaos,
                None,
                None,
                0,
                TICK,
            )
            .expect("伐木 80 级进得了混沌神木");

        // 凡点永远开着：新存档（0 级）也能开始第一趟采集，否则没有上手路径。
        let mut fresh = Cultivation::default();
        fresh
            .start_gathering_task(
                TaskKind::Woodcut,
                Some(GatherSite::Forest),
                GatherTier::Common,
                None,
                None,
                0,
                TICK,
            )
            .expect("新存档进得了凡木林");
    }

    #[test]
    fn a_task_started_the_s1_way_works_the_common_tier() {
        let mut cultivation = start(TaskKind::Woodcut, Some(GatherSite::Forest), None, 0);
        assert_eq!(
            cultivation.task().expect("running").tier,
            GatherTier::Common,
            "S1's entry point defaults to the ordinary tier"
        );
        assert_eq!(cultivation.advance_task(i64::from(TICK)), 1);
        assert_eq!(
            cultivation.document().resources.wood,
            2,
            "L1 凡木林 pays the bare base"
        );
    }

    #[test]
    fn a_document_written_before_tasks_loaded_has_none() {
        let document: CultivationDocument =
            serde_json::from_str(r#"{"growth":1000}"#).expect("the older shape loads");
        assert_eq!(document.task, None);
    }

    #[test]
    fn a_task_written_before_tiers_loaded_is_common() {
        // The S1 task shape has no `tier`; it reads as the ordinary tier so an
        // old save's running task keeps working unchanged.
        let task: ActiveTask = serde_json::from_str(
            r#"{"kind":"Woodcut","site":"Forest","start_unix":0,"ticks_done":0,"tick_seconds":8}"#,
        )
        .expect("the S1 task shape loads");
        assert_eq!(task.tier, GatherTier::Common);
    }

    #[test]
    fn a_task_written_before_s4_loads_without_a_trade() {
        // The S4 task shape adds `job_kind`; a save written before it reads as
        // no trade, so an old running 打工 task keeps paying its wage alone.
        let task: ActiveTask = serde_json::from_str(
            r#"{"kind":"Work","start_unix":0,"ticks_done":0,"tick_seconds":4}"#,
        )
        .expect("the pre-S4 task shape loads");
        assert_eq!(task.job_kind, None);
        assert_eq!(task.grade, None);
    }

    /// A cultivator standing in the given realm, with the default (×1.000)
    /// 功法 multiplier, so a tick's growth is the realm's own table row.
    fn at_realm(index: u8) -> Cultivation {
        Cultivation::new(CultivationDocument {
            realm: index,
            ..CultivationDocument::default()
        })
    }

    #[test]
    fn every_realm_pays_its_formula_growth_per_tick() {
        // `docs/0104` §5.1's per-tick table, read at a bare ×1.000 multiplier.
        // The hand-authored ladder replaces the old `2 × (1 + 0.75·index)` so
        // every realm lands in seven to fifty-eight days against the 门槛 table,
        // which a linear income could not (`docs/0104` D6).
        let table = [
            (Realm::QiRefining, 2),
            (Realm::Foundation, 3),
            (Realm::GoldenCore, 6),
            (Realm::NascentSoul, 8),
            (Realm::ManaTransformation, 15),
            (Realm::VoidRefinement, 35),
            (Realm::Integration, 55),
            (Realm::Mahayana, 88),
            (Realm::Tribulation, 142),
            (Realm::Immortal, 230),
        ];
        for (realm, expected) in table {
            assert_eq!(
                meditate_tick_growth(realm.index() as u32, 1_000),
                expected,
                "realm {} pays {} a tick",
                realm.index(),
                expected
            );
        }
    }

    #[test]
    fn a_meditate_task_pays_its_own_realm_growth() {
        let mut cultivation = at_realm(2); // 金丹
        let before = cultivation.document().growth;
        cultivation
            .start_task(TaskKind::Meditate, None, None, 0, TICK)
            .expect("the task starts");
        assert_eq!(cultivation.advance_task(i64::from(TICK)), 1);
        assert_eq!(
            cultivation.document().growth,
            before + 6,
            "金丹 pays 6 a tick"
        );
    }

    #[test]
    fn meditate_banks_one_chance_every_tenth_tick() {
        let mut cultivation = start(TaskKind::Meditate, None, None, 0);
        let now = run_ticks(&mut cultivation, 9, 0);
        assert_eq!(cultivation.document().proving_tokens, 0);
        assert_eq!(cultivation.advance_task(now + i64::from(TICK)), 1);
        assert_eq!(cultivation.document().proving_tokens, 1);
        // The same instant again does not cross the threshold twice.
        assert_eq!(cultivation.advance_task(now + i64::from(TICK)), 0);
        assert_eq!(cultivation.document().proving_tokens, 1);
        let now = run_ticks(&mut cultivation, 10, now + i64::from(TICK));
        assert_eq!(cultivation.advance_task(now), 0);
        assert_eq!(cultivation.document().proving_tokens, 2);
    }

    #[test]
    fn a_threshold_crossed_in_two_advances_banks_once() {
        let mut cultivation = start(TaskKind::Meditate, None, None, 0);
        let tick = i64::from(TICK);
        assert_eq!(cultivation.advance_task(9 * tick + 3), 9);
        assert_eq!(cultivation.document().proving_tokens, 0);
        // The tenth tick lands 3 seconds later; the threshold is crossed once.
        assert_eq!(cultivation.advance_task(10 * tick + 3), 1);
        assert_eq!(cultivation.document().proving_tokens, 1);
    }

    #[test]
    fn meditate_chances_stop_at_the_cap() {
        let mut cultivation = start(TaskKind::Meditate, None, None, 0);
        // A hundred ticks is ten thresholds; the bank cannot pass ten.
        run_ticks(&mut cultivation, 100, 0);
        assert_eq!(
            cultivation.document().proving_tokens,
            TRIAL_CHANCES_CAP,
            "ten at most"
        );
        run_ticks(&mut cultivation, 10, 600);
        assert_eq!(cultivation.document().proving_tokens, TRIAL_CHANCES_CAP);
    }

    #[test]
    fn every_tenth_meditate_tick_rolls_the_drop_path() {
        let mut cultivation = start(TaskKind::Meditate, None, None, 0);
        run_ticks(&mut cultivation, 9, 0);
        assert_eq!(
            cultivation.roll_sequence, 0,
            "no drop is rolled before the tenth tick"
        );
        run_ticks(&mut cultivation, 1, 54);
        assert_eq!(cultivation.roll_sequence, 1, "the tenth tick rolls once");
        run_ticks(&mut cultivation, 10, 60);
        assert_eq!(cultivation.roll_sequence, 2, "the twentieth rolls again");
    }

    #[test]
    fn a_full_seclusion_pays_250_ticks_with_the_bonus_and_a_chance() {
        let mut cultivation = Cultivation::default();
        cultivation.start_seclusion(0).expect("the sit starts");
        let tick = i64::from(SECLUSION_TICK_SECONDS);
        // 249 ticks: the sit is still running and has paid no chance yet.
        assert_eq!(cultivation.advance_seclusion(249 * tick), 249);
        assert!(cultivation.seclusion().is_some());
        assert_eq!(
            cultivation.document().proving_tokens,
            0,
            "the chance is paid when the sit ends"
        );
        // The last tick completes it: 250 × 2 × 1.1 = 550.
        assert_eq!(cultivation.advance_seclusion(250 * tick), 1);
        assert!(cultivation.seclusion().is_none());
        assert_eq!(cultivation.document().growth, 550);
        assert_eq!(cultivation.document().proving_tokens, 1);
    }

    #[test]
    fn stopping_a_seclusion_early_pays_the_finished_ticks() {
        let mut cultivation = Cultivation::default();
        cultivation.start_seclusion(0).expect("the sit starts");
        let tick = i64::from(SECLUSION_TICK_SECONDS);
        assert_eq!(cultivation.advance_seclusion(10 * tick), 10);
        // Stop 3 seconds later: no further whole tick, but the run ends.
        assert_eq!(cultivation.stop_seclusion(10 * tick + 3), Some(0));
        assert!(cultivation.seclusion().is_none());
        assert_eq!(cultivation.document().growth, 22, "10 × 2 × 1.1");
        assert_eq!(cultivation.document().proving_tokens, 1);
    }

    #[test]
    fn advancing_a_seclusion_twice_at_the_same_instant_is_a_no_op() {
        let mut cultivation = Cultivation::default();
        cultivation.start_seclusion(0).expect("the sit starts");
        let tick = i64::from(SECLUSION_TICK_SECONDS);
        assert_eq!(cultivation.advance_seclusion(10 * tick), 10);
        let growth = cultivation.document().growth;
        assert_eq!(cultivation.advance_seclusion(10 * tick), 0);
        assert_eq!(cultivation.document().growth, growth);
    }

    #[test]
    fn a_seclusion_runs_beside_a_gathering_task_without_touching_it() {
        let mut cultivation = Cultivation::default();
        cultivation
            .start_task(TaskKind::Woodcut, Some(GatherSite::Forest), None, 0, TICK)
            .expect("the task starts");
        cultivation.start_seclusion(0).expect("the sit starts");
        let now = run_ticks(&mut cultivation, 10, 0);
        assert_eq!(cultivation.advance_seclusion(now), 10);
        // The task is untouched by the sit: still running, still ten ticks, and
        // it paid its own wood.
        let task = cultivation.task().expect("the task is still running");
        assert_eq!(task.ticks_done, 10);
        assert!(cultivation.document().resources.wood > 0);
        assert!(cultivation.seclusion().is_some());
        assert_eq!(
            cultivation.document().growth,
            22,
            "the sit paid its own growth"
        );
    }

    #[test]
    fn starting_a_second_seclusion_is_refused_and_the_first_is_untouched() {
        let mut cultivation = Cultivation::default();
        let run = cultivation.start_seclusion(0).expect("the first sit");
        let refused = cultivation.start_seclusion(1_000);
        assert_eq!(refused, Err(Refusal::SeclusionRunning));
        assert_eq!(cultivation.last_refusal, Some(Refusal::SeclusionRunning));
        assert_eq!(cultivation.seclusion(), Some(run));
    }

    #[test]
    fn a_seclusion_chance_respects_the_cap() {
        let mut cultivation = Cultivation::default();
        cultivation.document.proving_tokens = TRIAL_CHANCES_CAP;
        cultivation.start_seclusion(0).expect("the sit starts");
        assert_eq!(
            cultivation.advance_seclusion(i64::from(SECLUSION_TICK_SECONDS) * 250),
            250
        );
        assert_eq!(cultivation.document().proving_tokens, TRIAL_CHANCES_CAP);
    }

    #[test]
    fn a_document_written_before_seclusion_loaded_has_none() {
        let document: CultivationDocument =
            serde_json::from_str(r#"{"growth":1000}"#).expect("the older shape loads");
        assert_eq!(document.seclusion, None);
    }

    #[test]
    fn a_session_settles_beside_a_meditate_task_without_touching_it() {
        // A 修习 session is a whole 闭关, and it does not occupy the task slot:
        // the two run on the same clock and each pays its own ticks, so a
        // session neither swallows nor double-counts the task's growth.
        let mut cultivation = Cultivation::default();
        cultivation
            .start_task(TaskKind::Meditate, None, None, 0, TICK)
            .expect("the task starts");
        run_ticks(&mut cultivation, 10, 0);
        let task_growth = cultivation.document().growth;
        assert_eq!(task_growth, 20, "10 × 2 at 炼气");
        assert_eq!(
            cultivation.document().proving_tokens,
            1,
            "the tenth tick banked one"
        );

        let reward =
            cultivation.settle_sit_session(i64::from(TICK) * 10, CalendarDay::from_day_number(1));
        assert_eq!(reward.growth, 550, "250 × 2 × 1.1");
        assert_eq!(cultivation.document().growth, task_growth + 550);
        assert_eq!(
            cultivation.document().proving_tokens,
            2,
            "the sit banked its own one"
        );
        // The task is untouched and still running.
        let task = cultivation.task().expect("the task is still running");
        assert_eq!(task.kind, TaskKind::Meditate);
        assert_eq!(task.ticks_done, 10);
    }

    /// The one arithmetic the three surfaces share. `web/cultivate.js` and
    /// `miniprogram/utils/views.js` used to carry their own copy; the wasm
    /// bridge exports this instead, and `just web-check` / `just mp-check`
    /// pin both pages to it.
    #[test]
    fn the_tick_bar_fills_once_and_never_wraps() {
        let start = 1_800_000_000;
        assert_eq!(tick_progress_permille(start, 8, start * 1000), 0, "回合边");
        assert_eq!(tick_progress_permille(start, 8, start * 1000 + 2_000), 250);
        assert_eq!(tick_progress_permille(start, 8, start * 1000 + 4_000), 500);
        assert_eq!(tick_progress_permille(start, 8, start * 1000 + 7_999), 999);
        assert_eq!(tick_progress_permille(start, 8, start * 1000 + 8_000), 1000);
        // A clock past the far edge is a stale reading, not a second tick: it
        // stays full rather than wrapping into the next one.
        assert_eq!(
            tick_progress_permille(start, 8, start * 1000 + 60_000),
            1000
        );
        // A clock behind the edge is empty, not negative.
        assert_eq!(tick_progress_permille(start, 8, start * 1000 - 1_000), 0);
    }

    /// A save written before the field existed deserializes `start_unix` to 0.
    /// That reads empty, the same rule the two pages apply, rather than
    /// painting the bar permanently full.
    #[test]
    fn a_missing_start_reads_empty_rather_than_full() {
        assert_eq!(tick_progress_permille(0, 8, 1_800_000_000_000), 0);
        assert_eq!(tick_progress_permille(-5, 8, 1_800_000_000_000), 0);
    }

    /// The batched offline catch-up must pay exactly what the tick-by-tick
    /// loop paid: same growth, same wage, same rolls, same level-ups, same
    /// events and the same last find. The rng is seeded from the clock plus a
    /// per-roll sequence, so a batch that visited the thresholds out of order,
    /// or skipped one, shows up here rather than in a save someone reloads.
    /// An absence settles half the ticks it owes (`docs/0104` §2.5), and that
    /// half must be paid exactly as the tick-by-tick loop would pay it.
    #[test]
    fn offline_catch_up_pays_half_the_ticks_the_way_the_ticks_would() {
        let now = 1_800_000_000;
        // Past both thresholds (10 and 60), so the window covers several.
        let due = 250_u64;
        let paid = due * OFFLINE_EFFICIENCY_PERMILLE / 1_000;
        let now_after = now + i64::from(TICK) * due as i64;
        let cases = [
            (TaskKind::Meditate, None, None),
            (TaskKind::Woodcut, Some(GatherSite::Forest), None),
            (TaskKind::Work, None, Some(0_u8)),
        ];
        for (kind, site, grade) in cases {
            let mut batched = start(kind, site, grade, now);
            let ticked = settle_tick_by_tick(batched.clone(), paid, now_after);
            assert_eq!(
                batched.advance_task(now_after),
                paid,
                "{kind:?} must pay half the ticks it owes"
            );
            assert_same_as_ticks(&batched, &ticked);
        }
    }

    /// The same equivalence for a 打工 task that carries a trade. Without one
    /// the sixtieth-tick faucet never rolls, so the batch could skip that branch
    /// outright and the plain 打工 case above would still pass.
    #[test]
    fn offline_catch_up_rolls_the_work_faucet_as_the_ticks_did() {
        let now = 1_800_000_000;
        let due = 250_u64;
        let paid = due * OFFLINE_EFFICIENCY_PERMILLE / 1_000;
        let tick = i64::from(WORK_TICK_SECONDS);
        let now_after = now + tick * due as i64;
        let mut batched = Cultivation::default();
        batched
            .start_work_task(work_offer(ArtGrade::Yellow), now)
            .expect("the job starts");
        assert!(
            batched.task().expect("running").job_kind.is_some(),
            "the trade has to ride the task for its faucet to exist"
        );
        let ticked = settle_tick_by_tick(batched.clone(), paid, now_after);
        assert_eq!(batched.advance_task(now_after), paid);
        assert_eq!(
            batched.document().stones,
            paid as u32,
            "黄阶 pays one a tick, four seconds at a time"
        );
        assert_eq!(
            batched.roll_sequence, 2,
            "125 paid ticks hold two sixtieth-tick faucets (60/120), and each \
             one draws exactly one roll"
        );
        assert_same_as_ticks(&batched, &ticked);
    }

    /// The path batching replaced, reproduced exactly: one task rewrite per
    /// tick, and one `now` for all of them — an offline catch-up reads the
    /// clock once, and the roll sequence is what keeps the rolls apart.
    fn settle_tick_by_tick(mut cultivation: Cultivation, due: u64, now_after: i64) -> Cultivation {
        for _ in 0..due {
            let mut current = cultivation.task().expect("still running");
            current.ticks_done = current.ticks_done.saturating_add(1);
            cultivation.settle_task_tick(&mut current, now_after);
            cultivation.document.task = Some(current);
        }
        cultivation
            .document
            .task
            .as_mut()
            .expect("still running")
            .start_unix = now_after;
        cultivation
    }

    /// The two paths end in the same state, down to how many rolls each drew:
    /// a batch that visited the thresholds out of order or skipped one would
    /// leave a different `roll_sequence` even when no roll happened to land.
    fn assert_same_as_ticks(batched: &Cultivation, ticked: &Cultivation) {
        assert_eq!(
            batched.roll_sequence, ticked.roll_sequence,
            "the batch must draw the same rolls, in the same order"
        );
        assert_eq!(
            serde_json::to_value(batched.document()).expect("a document serializes"),
            serde_json::to_value(ticked.document()).expect("a document serializes"),
            "the batch and the per-tick loop must pay the same document"
        );
    }
}
