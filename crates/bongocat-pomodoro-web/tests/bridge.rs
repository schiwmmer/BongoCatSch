//! The contract between the wasm bridge and the page.
//!
//! The page reads one JSON snapshot and stores one JSON document, so those two
//! shapes are the interface: a field renamed here without notice would leave a
//! blank panel rather than a compile error. These tests pin the names the page
//! actually reads, and the round trip that keeps a save across a reload.

use std::time::Duration;

use bongocat_pomodoro::{PomodoroCommand, PomodoroState};

fn state_at(now: i64) -> PomodoroState {
    let mut state = PomodoroState::empty_at_with_seed(8 * 3600, 0x5EED_1E55);
    state.apply(
        PomodoroCommand::CreateTask {
            name: String::from("写番茄钟网站"),
            estimated_pomodoros: 2,
        },
        now,
    );
    state
}

#[test]
fn a_snapshot_carries_every_key_the_page_reads() {
    let now = 1_800_000_000;
    let state = state_at(now);
    let snapshot = serde_json::to_value(state.snapshot_at(1, now)).expect("snapshot serializes");

    for key in [
        "revision",
        "tasks",
        "focus_minutes",
        "total_pomodoros",
        "cultivation",
    ] {
        assert!(snapshot.get(key).is_some(), "snapshot is missing `{key}`");
    }
    assert_eq!(snapshot["focus_minutes"], 25);
    assert_eq!(snapshot["tasks"][0]["estimated_pomodoros"], 2);

    let cultivation = &snapshot["cultivation"];
    for key in [
        "level",
        "realm",
        "layer",
        "rank",
        "growth",
        "next",
        "attributes",
        "derived",
        "free_points",
        "power",
        "power_breakdown",
        "equipped",
        "inventory",
        "seeds",
        "plots",
        "garden_rank",
        "stones",
        "jobs",
        "arts",
        "mastered_arts",
        "resources",
        "session_mode",
        "trial_floor",
        "proving_tokens",
        "shop_offers",
        "duel_record",
    ] {
        assert!(
            cultivation.get(key).is_some(),
            "cultivation is missing `{key}`"
        );
    }

    // The breakdown has to add up to the number printed beside it, which is the
    // one claim the status page makes about arithmetic.
    let breakdown = &cultivation["power_breakdown"];
    let parts: u64 = [
        "level",
        "allocated",
        "equipment",
        "equipment_bonus",
        "crop",
        "art",
        "art_bonus",
    ]
    .iter()
    .map(|key| breakdown[*key].as_u64().expect("a number"))
    .sum();
    assert_eq!(parts, breakdown["total"].as_u64().expect("total"));
    assert_eq!(breakdown["total"], cultivation["power"]);
}

#[test]
fn a_document_survives_the_json_round_trip() {
    let now = 1_800_000_000;
    let mut state = state_at(now);
    let task_id = state.snapshot_at(0, now).tasks[0].id;

    state.apply(PomodoroCommand::StartTask { task_id }, now);
    let finished = state.advance(Duration::from_secs(25 * 60), now + 25 * 60);
    assert!(finished.is_some(), "a full session completes");

    let stored = state.document_json();
    let restored = PomodoroState::from_document_json_with_seed(&stored, 8 * 3600, 1);

    let before = state.snapshot_at(0, now + 25 * 60);
    let after = restored.snapshot_at(0, now + 25 * 60);
    assert_eq!(after.tasks.len(), before.tasks.len());
    assert_eq!(
        after.tasks[0].completed_pomodoros,
        before.tasks[0].completed_pomodoros
    );
    assert_eq!(after.total_pomodoros, before.total_pomodoros);
    assert_eq!(after.cultivation.growth, before.cultivation.growth);
    assert_eq!(after.cultivation.level, before.cultivation.level);
    assert_eq!(after.cultivation.stones, before.cultivation.stones);
    // The birth roll is stored, so a relaunch never rerolls 潜质.
    assert_eq!(
        after.cultivation.aptitude_tenths,
        before.cultivation.aptitude_tenths
    );
}

#[test]
fn a_fresh_save_gets_its_own_aptitude() {
    // Two seeds, two bodies: the birth roll is the one thing a host supplies,
    // because the platform has no clock to draw it from.
    let first = PomodoroState::empty_at_with_seed(8 * 3600, 1).snapshot_at(0, 0);
    let second = PomodoroState::empty_at_with_seed(8 * 3600, 0xDEAD_BEEF).snapshot_at(0, 0);
    assert_ne!(
        first.cultivation.aptitude_tenths,
        second.cultivation.aptitude_tenths
    );
}

#[test]
fn an_unreadable_document_reads_as_a_fresh_one() {
    // A store that was cleared, truncated by a crash, or written by a build this
    // one cannot read must still start the page rather than blank it.
    let state = PomodoroState::from_document_json_with_seed("{not json", 8 * 3600, 7);
    let snapshot = state.snapshot_at(0, 1_800_000_000);
    assert!(snapshot.tasks.is_empty());
    assert_eq!(snapshot.cultivation.level, 1);
    assert_eq!(snapshot.focus_minutes, 25);
}
