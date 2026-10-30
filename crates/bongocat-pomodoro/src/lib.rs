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
    ABSENCE_LAYERS_PER_DAY, APTITUDE_ART_IDS, APTITUDE_ART_TENTHS_PER_RANK, ART_COUNT,
    AbsenceCharge, AptitudeHerb, AptitudeUse, ArtBlueprint, ArtGrade, ArtKind, ArtOutcome,
    ArtOwnership, ArtSetView, ArtStrike, ArtView, Attribute, Attributes,
    BREAKTHROUGH_COOLDOWN_SECONDS, BattleReport, BonusFind, BreakthroughOutcome, BuildingOutcome,
    BuildingView, CollectOutcome, CropRecord, CropView, Cultivation, CultivationDocument,
    CultivationView, DUEL_RECORD_CAP, DUEL_ROUND_CAP, DailyKind, DailyOutcome, Derived,
    DerivedBreakdown, DerivedSlot, Discipline, Drop, DropView, DuelRecord, DuelReport, DuelRound,
    ENCOUNTER_PERMILLE, EQUIPMENT_DROP_PERMILLE, EVENT_LOG_CAP, EquipSlot, EquipmentRecord,
    EventDetail, EventRecord, ExplorationFindView, ExplorationHours, ExplorationOutcome,
    FREE_PLOT_COUNT, FREE_POINTS_PER_LEVEL, FightKind, Fighter, GROWTH_MULTIPLIER_CAP_PERMILLE,
    GUARDIAN_NAME, GearEffect, GearSet, GearSetView, Guardian, GuardianView, HarvestOutcome,
    HerbStack, HerbView, ItemView, JOB_MAX_HOURS, JOB_MIN_HOURS, JOB_OFFER_COUNT, JobKind,
    JobOffer, JobRun, JobView, LAYERS_PER_REALM, LAYERS_PER_STAGE, LayerRank, LevelProgress,
    MAX_GARDEN_RANK, MAX_LEVEL, MAX_PLOT_COUNT, MAX_SEED_TIER, MAX_STAR, MarrowSource,
    MasteredArtView, OwnedArt, PLAYER_NAME, POWER_FACTOR_MAX_PERMILLE, POWER_FACTOR_MIN_PERMILLE,
    PlantOutcome, PlotRecord, PlotView, PowerBreakdown, ProvingBlocked, ProvingOutcome,
    QUALITY_BASE_PER_BILLION, QUALITY_STEP_DIVISOR, RANKS_PER_LAYER, Rarity, Realm, Refusal,
    ResourcesView, SEED_DROP_PERMILLE, SEED_RETURN_PERMILLE_PER_RANK, SEED_STONES_PER_POINT,
    SESSION_BASE_GROWTH, SESSION_BONUS_PERMILLE, SESSION_EARNINGS, SHOP_MAX_OFFERS,
    SHOP_MIN_OFFERS, SLOT_COUNT, STAR_SUCCESS_PERMILLE, SaleKind, SeedGrade, SeedStack, SeedView,
    SessionReward, ShopOffer, ShopOfferView, StarOutcome, StrikeEffect, StrikeShape,
    TRIAL_ATTEMPTS_PER_DAY, TRIAL_CHANCES_CAP, TRIAL_MAX_FLOOR, TRIAL_OPPONENT_NAME, TradeRefusal,
    TradeView, TrainingView, TrialBlocked, TrialOutcome, TrialSweepOutcome, WALK_RECORD_CAP,
    WANDERER_NAME, WalkInProgress, WalkRecord, WorkPaid, absence_penalty, art_aptitude_tenths,
    art_aptitude_tenths_at, art_attributes, art_bonus, art_bonus_at, art_payout_at,
    art_prefix_name, art_price, art_set_bonus, art_set_views, art_view, base_attributes,
    base_success_permille, blueprint, discipline_bonus, discipline_cap, equipment_price,
    external_damage, external_power_gain, floor_level, gear_set_bonus, gear_set_views,
    grow_seconds, grow_seconds_at, growth_permille, harvest_points, herb_tier, job_stones,
    layer_growth, layer_rank, level_for_growth, mastered_art_aptitude_payout,
    mastered_art_aptitude_tenths, mastered_art_attributes, mastered_art_bonus,
    mastered_art_bonus_payout, mastered_art_payout, mastered_art_view, offers_for_day,
    plant_permille, power_factor_permille, prefix_of, printed_pomodoros,
    quality_chance_per_billion, quality_per_billion, rank_cost, rank_for_xp, rank_xp,
    rank_xp_total, realm_power, refresh_cost, required_pomodoros, roll_offers, scaled_growth,
    seed_price, seed_return_permille, session_earnings, session_growth, star_multiplier_permille,
    star_success_permille, star_up_cost, success_permille, threshold_for, tier_for_power,
    warden_power, wither_points,
};
// The resource economy — gathered at the sites, spent at the forge — is a
// surface the settings page reads and the session drives, so it rides the
// crate's public surface with the rest of the ladder.
pub use cultivation::{
    BASE_COMMON_YIELD, BASE_RARE_YIELD, BASE_RESOURCE_CAP, BASE_SKILL_XP, BUILDING_STONES_PER_RANK,
    Building, COAL_UNLOCK_LEVEL, CollectionYield, Cost, GatherSite, GatherSkill, IRON_UNLOCK_LEVEL,
    MAX_BUILDING_LEVEL, MAX_TRADE_LEVEL, Resource, ResourceBag, STAR_IRON_FROM_STAR,
    STAR_IRON_HIGH_STAR, STAR_STONE_PER_STAR, STAR_WOOD_PER_STAR, building_upgrade_cost, collect,
    resource_cap, skill_ready, skill_xp_total, smithy_multiplier_permille,
};
pub use document::{LiveSessionRecord, POMODORO_TASKS_SCHEMA_VERSION};
pub use service::{
    PomodoroClient, PomodoroDiagnostics, PomodoroErrorCode, PomodoroPublishError, PomodoroService,
    PomodoroShutdownError, PomodoroStartError, PomodoroWorkerState,
};
pub use state::{
    CalendarTotals, PomodoroCommand, PomodoroCompletion, PomodoroSnapshot, PomodoroState,
    PomodoroTaskStatus, PomodoroTaskView, Timer, TimerState, TimerView, clamp_focus_minutes,
    sanitize_task_name,
};
