//! The web bridge: one JSON string in, one JSON string out.
//!
//! The desktop app drives this state machine through a worker thread that owns
//! a command queue and writes the document to disk. A browser has neither, so
//! this crate does the two things the host needs instead: it folds a command
//! into the same [`PomodoroState`] the desktop uses, and it hands back the same
//! snapshot the desktop renders — through a byte buffer the page reads out of
//! linear memory.
//!
//! There is deliberately no `wasm-bindgen` here. The whole interface is six
//! `extern "C"` functions and a megabyte of shared input buffer, which is less
//! machinery than a generated glue layer and one fewer tool version to pin.
//!
//! Two desktop concerns are absent by design:
//!
//! - **No interaction counters.** `PomodoroCommand::RecordInput` is the raw
//!   keyboard and mouse credit; a browser page does not own the keyboard, so the
//!   command is never parsed and growth comes from finished sessions alone.
//! - **No file system.** The page keeps the document string in local storage,
//!   and [`bongocat_pomodoro::PomodoroState::from_document_json`] reads it back.

// `#[unsafe(no_mangle)]` is an unsafe attribute under edition 2024 because the
// linker trusts a name it did not mangle. That is exactly the point of this
// crate: the exported symbols *are* the browser's entry points, and the names
// are the whole contract. Nothing here declares an `unsafe` block, dereferences
// a pointer or holds a raw handle — the buffer pointers cross the boundary once
// and are only ever used by the page's own `Uint8Array` view.

use std::{cell::RefCell, time::Duration};

use bongocat_pomodoro::{
    AptitudeHerb, Attribute, Building, EquipSlot, ExplorationHours, GatherSite, PomodoroCommand,
    PomodoroState, SeedGrade, SessionMode,
};
use serde_json::Value;

/// The size of the shared input buffer.
///
/// A command is a few hundred bytes and a stored document is well under a
/// megabyte of JSON, so one fixed buffer covers both without an allocator
/// handshake across the language boundary.
const INPUT_CAPACITY: usize = 2 << 20;

thread_local! {
    static INPUT: RefCell<Vec<u8>> = RefCell::new(vec![0; INPUT_CAPACITY]);
    static OUTPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// The machine plus the revision the page uses to decide whether to redraw.
struct App {
    state: PomodoroState,
    revision: u64,
}

/// What a command did, as the page reads it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Applied {
    Ok,
    /// The operation name is not one this bridge exposes.
    Unknown,
    /// The state has not been booted yet.
    NoState,
}

impl Applied {
    const fn code(self) -> i32 {
        match self {
            Self::Ok => 0,
            Self::Unknown => 1,
            Self::NoState => 2,
        }
    }
}

fn read_input(len: u32) -> String {
    let len = (len as usize).min(INPUT_CAPACITY);
    INPUT.with(|cell| {
        let buffer = cell.borrow();
        String::from_utf8_lossy(&buffer[..len]).into_owned()
    })
}

fn write_output(text: String) {
    OUTPUT.with(|cell| *cell.borrow_mut() = text.into_bytes());
}

/// Expose the shared input buffer so the page can write into it.
///
/// The pointer stays valid for the life of the module: the buffer is a
/// thread-local `Vec` that is allocated once and never reallocated.
#[unsafe(no_mangle)]
pub extern "C" fn web_input_ptr() -> *mut u8 {
    INPUT.with(|cell| cell.borrow_mut().as_mut_ptr())
}

/// Where the last result starts.
#[unsafe(no_mangle)]
pub extern "C" fn web_output_ptr() -> *const u8 {
    OUTPUT.with(|cell| cell.borrow().as_ptr())
}

/// How many bytes the last result holds.
#[unsafe(no_mangle)]
pub extern "C" fn web_output_len() -> u32 {
    OUTPUT.with(|cell| cell.borrow().len() as u32)
}

/// Load a stored document and settle the time that passed while the page was
/// closed.
///
/// `document_len` is zero for a fresh install. The save in flight comes back as
/// it was left — the desktop's rule — and the seconds since the last write are
/// charged to it here.
#[unsafe(no_mangle)]
pub extern "C" fn web_boot(document_len: u32, utc_offset_seconds: i32, now: f64, seed: f64) -> u32 {
    let now_unix = now as i64;
    // The birth roll of a fresh save wants a seed, and a browser has no
    // `SystemTime` to draw one from: the page passes a random number in.
    let seed = seed as u64;
    let document = read_input(document_len);
    let mut state = if document.is_empty() {
        PomodoroState::empty_at_with_seed(utc_offset_seconds, seed)
    } else {
        PomodoroState::from_document_json_with_seed(&document, utc_offset_seconds, seed)
    };
    state.resume_live_session(now_unix);
    APP.with(|cell| {
        *cell.borrow_mut() = Some(App { state, revision: 1 });
    });
    1
}

/// Fold one command into the state. The payload is the command as JSON.
#[unsafe(no_mangle)]
pub extern "C" fn web_command(len: u32, now: f64) -> i32 {
    let now_unix = now as i64;
    let payload = read_input(len);
    let Ok(value) = serde_json::from_str::<Value>(&payload) else {
        return Applied::Unknown.code();
    };
    let Some(command) = parse_command(&value) else {
        return Applied::Unknown.code();
    };
    APP.with(|cell| {
        let mut slot = cell.borrow_mut();
        let Some(app) = slot.as_mut() else {
            return Applied::NoState.code();
        };
        app.state.apply(command, now_unix);
        app.revision = app.revision.saturating_add(1);
        Applied::Ok.code()
    })
}

/// Advance the clock: settle what the day owed, then fold elapsed time into the
/// countdown.
///
/// The order matches the desktop worker: the absence charge, the walk, the job,
/// the expedition, the garden and the morning event all settle before the
/// countdown moves, so a session that finishes on the same tick that brought the
/// cultivator home still pays out.
#[unsafe(no_mangle)]
pub extern "C" fn web_tick(now: f64, elapsed_ms: u32) -> i32 {
    let now_unix = now as i64;
    APP.with(|cell| {
        let mut slot = cell.borrow_mut();
        let Some(app) = slot.as_mut() else {
            return Applied::NoState.code();
        };
        let mut changed = false;
        changed |= app.state.apply_absence(now_unix);
        changed |= app.state.apply_walk_return(now_unix);
        changed |= app.state.apply_work_return(now_unix);
        changed |= app.state.apply_expedition_return(now_unix);
        changed |= app.state.apply_garden_reveal(now_unix);
        changed |= app.state.apply_daily_event(now_unix);
        let completed = app
            .state
            .advance(Duration::from_millis(u64::from(elapsed_ms)), now_unix)
            .is_some();
        if changed || completed {
            app.revision = app.revision.saturating_add(1);
        }
        // Bit 0 is "a session finished", bit 1 is "something else moved" — the
        // page redraws the ladder on either, and saves only when one is set.
        (i32::from(completed)) | (i32::from(changed) << 1)
    })
}

/// Write the snapshot the page renders into the output buffer.
#[unsafe(no_mangle)]
pub extern "C" fn web_render(now: f64) {
    let now_unix = now as i64;
    APP.with(|cell| {
        let slot = cell.borrow();
        let Some(app) = slot.as_ref() else {
            write_output(String::from("null"));
            return;
        };
        let snapshot = app.state.snapshot_at(app.revision, now_unix);
        write_output(serde_json::to_string(&snapshot).unwrap_or_else(|_| String::from("null")));
    });
}

/// The revision the page last read, so a poll can skip an unchanged snapshot.
#[unsafe(no_mangle)]
pub extern "C" fn web_revision() -> u32 {
    APP.with(|cell| {
        cell.borrow()
            .as_ref()
            .map_or(0, |app| app.revision.min(u64::from(u32::MAX)) as u32)
    })
}

/// Write the document the page stores into the output buffer.
#[unsafe(no_mangle)]
pub extern "C" fn web_export() {
    APP.with(|cell| {
        let slot = cell.borrow();
        match slot.as_ref() {
            Some(app) => write_output(app.state.document_json()),
            None => write_output(String::from("{}")),
        }
    });
}

/// Write the completed sessions per local day, as `[{day,pomodoros,tasks}]`.
///
/// The month grid is drawn from this rather than from the task list: a task
/// keeps only the timestamps of its own sessions, so counting them there would
/// miss the sessions of tasks that have since been deleted.
#[unsafe(no_mangle)]
pub extern "C" fn web_calendar(utc_offset_seconds: i32) {
    APP.with(|cell| {
        let slot = cell.borrow();
        let Some(app) = slot.as_ref() else {
            write_output(String::from("[]"));
            return;
        };
        let days: Vec<String> = app
            .state
            .day_totals(utc_offset_seconds)
            .into_iter()
            .map(|(day, total)| {
                format!(
                    "{{\"day\":{},\"pomodoros\":{},\"tasks\":{}}}",
                    day.day_number(),
                    total.pomodoros,
                    total.tasks
                )
            })
            .collect();
        write_output(format!("[{}]", days.join(",")));
    });
}

fn parse_command(value: &Value) -> Option<PomodoroCommand> {
    let op = value.get("op")?.as_str()?;
    let command = match op {
        "create_task" => PomodoroCommand::CreateTask {
            name: value.get("name")?.as_str()?.to_owned(),
            estimated_pomodoros: u32_value(value, "estimated")?,
        },
        "start_task" => PomodoroCommand::StartTask {
            task_id: u64_value(value, "task_id")?,
        },
        "pause" => PomodoroCommand::Pause,
        "resume" => PomodoroCommand::Resume,
        "abandon" => PomodoroCommand::Abandon,
        "set_task_done" => PomodoroCommand::SetTaskDone {
            task_id: u64_value(value, "task_id")?,
            done: value.get("done")?.as_bool()?,
        },
        "set_focus_minutes" => PomodoroCommand::SetFocusMinutes {
            minutes: u32_value(value, "minutes")?,
        },
        "update_task" => PomodoroCommand::UpdateTask {
            task_id: u64_value(value, "task_id")?,
            name: value.get("name")?.as_str()?.to_owned(),
            estimated_pomodoros: u32_value(value, "estimated")?,
        },
        "delete_task" => PomodoroCommand::DeleteTask {
            task_id: u64_value(value, "task_id")?,
        },
        "plant_seed" => PomodoroCommand::PlantSeed {
            attribute: attribute(value)?,
            tier: u8::try_from(u32_value(value, "tier")?).ok()?,
            grade: grade(value)?,
        },
        "plant_herb_seed" => PomodoroCommand::PlantHerbSeed {
            herb: herb(value)?,
            grade: grade(value)?,
        },
        "harvest_plot" => PomodoroCommand::HarvestPlot {
            plot: usize_value(value, "plot")?,
        },
        "dig_plot" => PomodoroCommand::DigPlot {
            plot: usize_value(value, "plot")?,
        },
        "buy_plot" => PomodoroCommand::BuyPlot,
        "upgrade_garden" => PomodoroCommand::UpgradeGarden,
        "equip_item" => PomodoroCommand::EquipItem {
            item_id: u64_value(value, "item_id")?,
        },
        "unequip_slot" => PomodoroCommand::UnequipSlot { slot: slot(value)? },
        "allocate_attribute" => PomodoroCommand::AllocateAttribute {
            attribute: attribute(value)?,
            points: u32_value(value, "points")?,
        },
        "breakthrough" => PomodoroCommand::Breakthrough,
        "select_art" => PomodoroCommand::SelectArt {
            art_id: value.get("art_id").and_then(Value::as_u64),
        },
        "explore" => PomodoroCommand::Explore {
            hours: hours(value)?,
        },
        "return_early" => PomodoroCommand::ReturnEarly,
        "accept_job" => PomodoroCommand::AcceptJob {
            slot: usize_value(value, "slot")?,
        },
        "refresh_jobs" => PomodoroCommand::RefreshJobs,
        "tower" => PomodoroCommand::Tower,
        "tower_sweep" => PomodoroCommand::TowerSweep,
        "prove" => PomodoroCommand::Prove,
        "buy_from_shop" => PomodoroCommand::BuyFromShop {
            slot: usize_value(value, "slot")?,
        },
        "sell_equipment" => PomodoroCommand::SellEquipment {
            item_id: u64_value(value, "item_id")?,
        },
        "sell_art" => PomodoroCommand::SellArt {
            art_id: u64_value(value, "art_id")?,
        },
        "sell_seed" => PomodoroCommand::SellSeed {
            attribute: attribute(value)?,
            tier: u8::try_from(u32_value(value, "tier")?).ok()?,
            grade: grade(value)?,
        },
        "sell_herb_seed" => PomodoroCommand::SellHerbSeed {
            herb: herb(value)?,
            grade: grade(value)?,
        },
        "star_equipment" => PomodoroCommand::StarEquipment {
            item_id: u64_value(value, "item_id")?,
        },
        "start_expedition" => PomodoroCommand::StartExpedition {
            site: site(value)?,
            hours: hours(value)?,
        },
        "upgrade_building" => PomodoroCommand::UpgradeBuilding {
            building: building(value)?,
        },
        "upgrade_building_many" => PomodoroCommand::UpgradeBuildingMany {
            building: building(value)?,
            times: u8::try_from(u32_value(value, "times")?).ok()?,
        },
        "select_session_mode" => PomodoroCommand::SelectSessionMode { mode: mode(value)? },
        "use_aptitude_herb" => PomodoroCommand::UseAptitudeHerb { herb: herb(value)? },
        "use_marrow_pill" => PomodoroCommand::UseMarrowPill,
        "acknowledge_daily_event" => PomodoroCommand::AcknowledgeDailyEvent {
            day: value.get("day")?.as_i64()?,
        },
        _ => return None,
    };
    Some(command)
}

fn u32_value(value: &Value, key: &str) -> Option<u32> {
    u32::try_from(value.get(key)?.as_u64()?).ok()
}

fn u64_value(value: &Value, key: &str) -> Option<u64> {
    value.get(key)?.as_u64()
}

fn usize_value(value: &Value, key: &str) -> Option<usize> {
    usize::try_from(value.get(key)?.as_u64()?).ok()
}

fn attribute(value: &Value) -> Option<Attribute> {
    Some(match value.get("attribute")?.as_str()? {
        "root" => Attribute::Root,
        "strength" => Attribute::Strength,
        "mana" => Attribute::Mana,
        "agility" => Attribute::Agility,
        _ => return None,
    })
}

fn grade(value: &Value) -> Option<SeedGrade> {
    Some(match value.get("grade")?.as_str()? {
        "yellow" => SeedGrade::Yellow,
        "profound" => SeedGrade::Profound,
        "earth" => SeedGrade::Earth,
        "heaven" => SeedGrade::Heaven,
        "divine" => SeedGrade::Divine,
        _ => return None,
    })
}

fn herb(value: &Value) -> Option<AptitudeHerb> {
    Some(match value.get("herb")?.as_str()? {
        "insight" => AptitudeHerb::Insight,
        "wisdom" => AptitudeHerb::Wisdom,
        "marrow" => AptitudeHerb::Marrow,
        _ => return None,
    })
}

fn slot(value: &Value) -> Option<EquipSlot> {
    Some(match value.get("slot")?.as_str()? {
        "weapon" => EquipSlot::Weapon,
        "clothes" => EquipSlot::Clothes,
        "pants" => EquipSlot::Pants,
        "boots" => EquipSlot::Boots,
        _ => return None,
    })
}

fn site(value: &Value) -> Option<GatherSite> {
    Some(match value.get("site")?.as_str()? {
        "forest" => GatherSite::Forest,
        "mountain" => GatherSite::Mountain,
        "mine" => GatherSite::Mine,
        _ => return None,
    })
}

fn building(value: &Value) -> Option<Building> {
    Some(match value.get("building")?.as_str()? {
        "workshop" => Building::Workshop,
        "warehouse" => Building::Warehouse,
        "smithy" => Building::Smithy,
        "dojo" => Building::Dojo,
        _ => return None,
    })
}

fn mode(value: &Value) -> Option<SessionMode> {
    Some(match value.get("mode")?.as_str()? {
        "study" => SessionMode::Study,
        "woodcut" => SessionMode::Woodcut,
        "mine" => SessionMode::Mine,
        _ => return None,
    })
}

fn hours(value: &Value) -> Option<ExplorationHours> {
    Some(match u32_value(value, "hours")? {
        4 => ExplorationHours::Four,
        8 => ExplorationHours::Eight,
        12 => ExplorationHours::Twelve,
        _ => return None,
    })
}
