//! The pomodoro timer: tasks, one running focus session, and the history the
//! monthly calendar is derived from.
//!
//! The crate owns two things and nothing else. The first is the *document*:
//! the tasks a user created, how many focus sessions each has completed, and
//! when each session finished. The second is the *timer*: the one focus session
//! that may be running, paused, or absent.
//!
//! The split is deliberate and is the reason the timer is not in the document.
//! A running countdown is a live measurement, not a fact worth keeping: the
//! user can abandon it, pause it, or walk away from the machine, and every one
//! of those endings makes the stored remainder a number that was true at some
//! moment nobody asked about. Restoring it on the next launch would restart a
//! countdown whose deadline passed while the application was not running. What
//! is worth keeping is the completed work, so the document records that and the
//! user re-starts a full session from a task instead.
//!
//! The state machine is a plain value ([`PomodoroState::advance`]) with no clock
//! and no IO: every transition takes the current wall time as an argument and
//! the countdown advances by an explicit elapsed [`std::time::Duration`]. That
//! is what lets the tests cover a twenty-five minute session in microseconds,
//! and cover a wake-up that arrives late, without a worker being involved.

#![forbid(unsafe_code)]

mod calendar;
mod cultivation;
mod document;
mod service;
mod state;
#[cfg(test)]
mod tests;

/// The shortest session a user may configure.
///
/// Below a minute a countdown is not a focus session at all, and one that
/// rounded to zero seconds would complete before it was ever seen.
pub const MIN_FOCUS_MINUTES: u32 = 1;

/// The longest session a user may configure.
pub const MAX_FOCUS_MINUTES: u32 = 120;

/// The session length a fresh install starts with.
pub const DEFAULT_FOCUS_MINUTES: u32 = 25;

/// The longest name one task may carry.
///
/// The bound exists so a row in the task list cannot be widened past the window
/// by a pasted document title.
pub const MAX_TASK_NAME_CHARS: usize = 80;

/// How many commands may be queued before a caller is told the queue is full.
///
/// A pomodoro command is a click on a button, not a per-frame signal, so the
/// queue only has to absorb a burst of impatient clicking; the countdown itself
/// is driven by the clock, not by the queue.
pub const COMMAND_CAPACITY: usize = 16;

pub use calendar::{
    CalendarDay, CalendarDayTotal, CalendarTaskActivity, CivilDate, MonthGrid, month_grid,
    month_totals,
};
pub use cultivation::{
    ABSENCE_BASE_PENALTY, ART_COUNT, AbsenceCharge, ArtBlueprint, ArtGrade, ArtKind, ArtOutcome,
    ArtStrike, ArtView, Attribute, Attributes, BREAKTHROUGH_COOLDOWN_SECONDS, BattleReport,
    BreakthroughOutcome, CropView, Cultivation, CultivationDocument, CultivationView,
    DUEL_RECORD_CAP, DUEL_ROUND_CAP, DailyKind, DailyOutcome, Derived, DerivedBreakdown,
    Discipline, Drop, DropView, DuelRecord, DuelReport, DuelRound, ENCOUNTER_PERMILLE,
    EQUIPMENT_DROP_PERMILLE, EVENT_LOG_CAP, EXPLORATION_COOLDOWN_SECONDS, EquipSlot,
    EquipmentRecord, EventDetail, EventRecord, ExplorationFindView, ExplorationOutcome,
    FREE_PLOT_COUNT, FREE_POINTS_PER_LEVEL, FightKind, Fighter, GROWTH_MULTIPLIER_CAP_PERMILLE,
    GUARDIAN_NAME, Guardian, GuardianView, HarvestOutcome, ItemView, LAYERS_PER_REALM,
    LAYERS_PER_STAGE, LayerRank, LevelProgress, MAX_LEVEL, MAX_PLOT_COUNT, MAX_SEED_TIER,
    MasteredArtView, OwnedArt, PLAYER_NAME, PlotView, RANKS_PER_LAYER, Rarity, Realm, Refusal,
    SEED_DROP_PERMILLE, SEED_RETURN_PERMILLE, SESSION_BASE_GROWTH, SESSION_EARNINGS,
    SHOP_MAX_OFFERS, SHOP_MIN_OFFERS, SLOT_COUNT, SaleKind, SeedView, SessionReward, ShopOffer,
    ShopOfferView, TRIAL_ATTEMPTS_PER_DAY, TRIAL_MAX_FLOOR, TRIAL_OPPONENT_NAME, TradeRefusal,
    TrainingView, TrialBlocked, TrialOutcome, WALK_RECORD_CAP, WANDERER_NAME, WalkRecord,
    absence_penalty, art_attributes, art_payout_at, art_price, art_view, base_attributes,
    blueprint, discipline_bonus, discipline_cap, effective_failure_permille, equipment_price,
    external_damage, floor_level, growth_permille, guardian_fighter, layer_rank, level_for_growth,
    mastered_art_attributes, mastered_art_payout, mastered_art_view, offers_for_day,
    required_pomodoros, roll_offers, scaled_growth, seed_price, session_earnings, session_growth,
    threshold_for, wanderer_fighter, warden_power,
};
pub use document::POMODORO_TASKS_SCHEMA_VERSION;
pub use service::{
    PomodoroClient, PomodoroDiagnostics, PomodoroErrorCode, PomodoroPublishError, PomodoroService,
    PomodoroShutdownError, PomodoroStartError, PomodoroWorkerState,
};
pub use state::{
    CalendarTotals, PomodoroCommand, PomodoroCompletion, PomodoroSnapshot, PomodoroState,
    PomodoroTaskStatus, PomodoroTaskView, Timer, TimerState, TimerView, clamp_focus_minutes,
    sanitize_task_name,
};
