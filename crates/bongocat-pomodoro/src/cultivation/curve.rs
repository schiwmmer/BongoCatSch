//! The growth curve: how much lifetime growth opens each level, and the inverse.
//!
//! The curve is a table of realm floors with a linear ramp across each realm's
//! hundred layers. That shape is deliberate. A single formula over a thousand
//! levels either makes the early layers crawl or the late ones trivial, while a
//! per-realm band lets the pacing be authored one realm at a time and still
//! stay a pure integer function both directions of which agree exactly.
//!
//! Every band spans a multiple of a hundred growth, which is what makes the
//! inverse exact: layer `k` of a realm opens at `floor + k * step`, so a growth
//! total maps back onto one level with no rounding argument to have.

use super::STAGE_COUNT;

/// How many layers each realm holds.
pub const LAYERS_PER_STAGE: u16 = 100;

/// How many ranks a layer is split into. A realm is therefore
/// [`LAYERS_PER_REALM`] layers of ten ranks, which is how the surfaces read a
/// level: 89 is "8 层 9 级", not "layer 89".
pub const RANKS_PER_LAYER: u16 = 10;

/// How many layers one realm holds: ten, of ten ranks each.
pub const LAYERS_PER_REALM: u16 = LAYERS_PER_STAGE / RANKS_PER_LAYER;

/// The highest level the ladder reaches: ten realms of a hundred layers.
pub const MAX_LEVEL: u16 = (STAGE_COUNT as u16) * LAYERS_PER_STAGE;

/// The growth each realm opens at.
///
/// The ramp is authored, not derived, and it is deliberately steep: a realm is
/// days-to-weeks of ordinary use, not an evening. The first realm is about
/// twenty sessions of ordinary use, every realm after it costs roughly twice
/// the last, and the last realm is a multi-year horizon rather than a sprint.
/// Every difference is a multiple of [`LAYERS_PER_STAGE`] so each realm's
/// hundred layers are an integer step apart.
///
/// The ramp was re-authored (rev 3) around the realms a focused cultivator
/// actually sits in: 化神 costs 15 000 a layer, which is what four ordinary
/// sessions of the day pay, so a working day is roughly three layers and a
/// long one still under ten. The early realms keep their rev-2 pace so the
/// first few days stay a quick tutorial; the climb is what a realm is.
const STAGE_FLOORS: [u32; STAGE_COUNT] = [
    0, 10_000, 30_000, 80_000, 280_000, 1_780_000, 4_780_000, 10_780_000, 22_780_000, 46_780_000,
];

/// Where the last realm's hundredth layer sits: the top of the ladder.
const TOP_GROWTH: u32 = 94_780_000;

/// Which ramp the stored growth was counted on.
///
/// `1` is the first ramp (the one whose floors ended at 600_000), `2` is the
/// ramp that ended at 18_000_000, `3` is [`STAGE_FLOORS`]. A document that
/// predates this field reads as `1`, so its growth is re-denominated once —
/// see [`migrate_growth_v1`] — while a fresh in-memory default reads as `0`,
/// which already means "counted on the current ramp" and is simply stamped on
/// the way out.
pub(crate) const CURVE_REV: u32 = 3;

/// The growth the given realm opens at. Indices past the ladder clamp to the
/// top growth, which only a document written by a longer ladder can reach.
pub fn stage_floor(stage: usize) -> u32 {
    STAGE_FLOORS.get(stage).copied().unwrap_or(TOP_GROWTH)
}

/// The growth a realm's hundred layers are spaced by.
fn stage_step(stage: usize) -> u32 {
    let start = stage_floor(stage);
    let end = if stage + 1 < STAGE_COUNT {
        stage_floor(stage + 1)
    } else {
        TOP_GROWTH
    };
    (end.saturating_sub(start) / u32::from(LAYERS_PER_STAGE)).max(1)
}

/// The growth lifetime growth `level` opens at. Levels outside `1..=MAX_LEVEL`
/// are clamped.
pub fn threshold_for(level: u16) -> u32 {
    let level = level.clamp(1, MAX_LEVEL);
    let stage = usize::from(level - 1) / usize::from(LAYERS_PER_STAGE);
    let layer = u32::from((level - 1) % LAYERS_PER_STAGE);
    stage_floor(stage) + layer * stage_step(stage)
}

/// A level read the way a cultivator says it: the layer inside its realm, and
/// the rank inside that layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LayerRank {
    /// The layer, zero-based: the first ten levels of a realm are layer 0.
    pub layer: u8,
    /// The rank inside the layer, one-based: 1..=10.
    pub rank: u8,
}

/// How a level is said: level 89 of a realm is "8 层 9 级".
///
/// The ladder itself still counts in [`LAYERS_PER_STAGE`] single levels; this is
/// only how one of them is read out loud, so no persisted number changes shape.
pub fn layer_rank(level: u16) -> LayerRank {
    let within = u32::from(level.saturating_sub(1)) % u32::from(LAYERS_PER_STAGE);
    LayerRank {
        layer: (within / u32::from(RANKS_PER_LAYER)) as u8,
        rank: (within % u32::from(RANKS_PER_LAYER) + 1) as u8,
    }
}

/// The growth one layer of the given realm costs.
///
/// Public because a penalty has to be spoken in the ladder's own unit: what a
/// missed day costs is a number of *layers of the realm you are in*, not a
/// number of growth points invented beside the curve.
pub fn layer_growth(realm: u8) -> u32 {
    stage_step(usize::from(realm))
}

/// Where a growth total sits on the ladder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LevelProgress {
    /// The highest level this growth has paid for.
    pub level: u16,
    /// The growth this level opened at; progress is measured from here.
    pub floor: u32,
    /// The growth the next level opens at; `None` at the top of the ladder.
    pub next: Option<u32>,
}

/// The level a growth total has reached.
///
/// The highest level whose [`threshold_for`] is at or below the total: the realm
/// is found by scanning the floors, then the layer by dividing the remainder by
/// that realm's step. Both directions agree because the ramp inside a realm is
/// exactly that division.
pub fn level_for_growth(growth: u32) -> LevelProgress {
    let mut stage = 0;
    for (index, &floor) in STAGE_FLOORS.iter().enumerate() {
        if floor <= growth {
            stage = index;
        } else {
            break;
        }
    }
    let start = STAGE_FLOORS[stage];
    let step = stage_step(stage);
    let layer = (growth.saturating_sub(start) / step).min(u32::from(LAYERS_PER_STAGE - 1));
    let level = (stage as u16) * LAYERS_PER_STAGE + layer as u16 + 1;
    LevelProgress {
        level,
        floor: threshold_for(level),
        next: (level < MAX_LEVEL).then(|| threshold_for(level + 1)),
    }
}

/// The first ramp, kept only so growth counted on it can be re-denominated.
///
/// A document written before [`CURVE_REV`] existed carries growth on these
/// floors. The level that growth reached must survive the move to the current
/// ramp — a retune raises the cost of levels *ahead*, never takes back the ones
/// already paid — so this mirror is the read side of the migration and nothing
/// else may call it.
mod v1 {
    use super::{LAYERS_PER_STAGE, STAGE_COUNT};

    /// The floors as they were first authored, ending at 600_000.
    const STAGE_FLOORS: [u32; STAGE_COUNT] = [
        0, 1_200, 4_000, 10_000, 22_000, 45_000, 90_000, 170_000, 320_000, 600_000,
    ];

    /// The old top of the ladder.
    const TOP_GROWTH: u32 = 1_100_000;

    fn stage_floor(stage: usize) -> u32 {
        STAGE_FLOORS.get(stage).copied().unwrap_or(TOP_GROWTH)
    }

    fn stage_step(stage: usize) -> u32 {
        let start = stage_floor(stage);
        let end = if stage + 1 < STAGE_COUNT {
            stage_floor(stage + 1)
        } else {
            TOP_GROWTH
        };
        (end.saturating_sub(start) / u32::from(LAYERS_PER_STAGE)).max(1)
    }

    /// The old threshold of a level, on the old ramp.
    pub(super) fn threshold_for(level: u16) -> u32 {
        let level = level.clamp(1, super::MAX_LEVEL);
        let stage = usize::from(level - 1) / usize::from(LAYERS_PER_STAGE);
        let layer = u32::from((level - 1) % LAYERS_PER_STAGE);
        stage_floor(stage) + layer * stage_step(stage)
    }

    /// Where a growth total sat on the old ramp: its level and the two
    /// thresholds that level sat between.
    pub(super) fn level_for_growth(growth: u32) -> (u16, u32, u32) {
        let mut stage = 0;
        for (index, &floor) in STAGE_FLOORS.iter().enumerate() {
            if floor <= growth {
                stage = index;
            } else {
                break;
            }
        }
        let start = STAGE_FLOORS[stage];
        let step = stage_step(stage);
        let layer = (growth.saturating_sub(start) / step).min(u32::from(LAYERS_PER_STAGE - 1));
        let level = (stage as u16) * LAYERS_PER_STAGE + layer as u16 + 1;
        let floor = threshold_for(level);
        let next = if level < super::MAX_LEVEL {
            threshold_for(level + 1)
        } else {
            floor
        };
        (level, floor, next)
    }
}

/// The second ramp, kept only so growth counted on it can be re-denominated.
///
/// This is the mirror of [`v1`] for the ramp this file carried before rev 3:
/// the floors that ended at 18_000_000. Only [`migrate_growth_v2`] reads it.
mod v2 {
    use super::{LAYERS_PER_STAGE, STAGE_COUNT};

    /// The floors as they were in rev 2, ending at 18_000_000.
    const STAGE_FLOORS: [u32; STAGE_COUNT] = [
        0, 10_000, 30_000, 80_000, 200_000, 450_000, 1_000_000, 2_100_000, 4_400_000, 9_000_000,
    ];

    /// The rev-2 top of the ladder.
    const TOP_GROWTH: u32 = 18_000_000;

    fn stage_floor(stage: usize) -> u32 {
        STAGE_FLOORS.get(stage).copied().unwrap_or(TOP_GROWTH)
    }

    fn stage_step(stage: usize) -> u32 {
        let start = stage_floor(stage);
        let end = if stage + 1 < STAGE_COUNT {
            stage_floor(stage + 1)
        } else {
            TOP_GROWTH
        };
        (end.saturating_sub(start) / u32::from(LAYERS_PER_STAGE)).max(1)
    }

    pub(super) fn threshold_for(level: u16) -> u32 {
        let level = level.clamp(1, super::MAX_LEVEL);
        let stage = usize::from(level - 1) / usize::from(LAYERS_PER_STAGE);
        let layer = u32::from((level - 1) % LAYERS_PER_STAGE);
        stage_floor(stage) + layer * stage_step(stage)
    }

    pub(super) fn level_for_growth(growth: u32) -> (u16, u32, u32) {
        let mut stage = 0;
        for (index, &floor) in STAGE_FLOORS.iter().enumerate() {
            if floor <= growth {
                stage = index;
            } else {
                break;
            }
        }
        let start = STAGE_FLOORS[stage];
        let step = stage_step(stage);
        let layer = (growth.saturating_sub(start) / step).min(u32::from(LAYERS_PER_STAGE - 1));
        let level = (stage as u16) * LAYERS_PER_STAGE + layer as u16 + 1;
        let floor = threshold_for(level);
        let next = if level < super::MAX_LEVEL {
            threshold_for(level + 1)
        } else {
            floor
        };
        (level, floor, next)
    }
}

/// Re-denominate growth counted on the v1 ramp onto the current one.
///
/// The level is what must survive: the total is rewritten to where that same
/// level opens on the current ramp, plus the same fraction of the way to the
/// next level. Growth past the v1 top keeps its level and lands on the current
/// top; a level at the current top simply is the current top threshold.
pub(crate) fn migrate_growth_v1(growth: u32) -> u32 {
    let (level, floor, next) = v1::level_for_growth(growth);
    let new_floor = threshold_for(level);
    let new_next = if level < MAX_LEVEL {
        threshold_for(level + 1)
    } else {
        new_floor
    };
    if new_next <= new_floor || next <= floor {
        return new_floor;
    }
    let span = u64::from(next - floor);
    let paid = u64::from(growth - floor);
    let new_span = u64::from(new_next - new_floor);
    let carried = (paid * new_span) / span;
    new_floor.saturating_add(u32::try_from(carried).unwrap_or(u32::MAX))
}

/// Re-denominate growth counted on the v2 ramp onto the current one.
///
/// The level is what must survive, exactly as [`migrate_growth_v1`] does for
/// the ramp before it: the total is rewritten to where that same level opens
/// on the current ramp, plus the same fraction of the way to the next level.
/// Growth past the v2 top keeps its level and lands on the current top; a
/// level at the current top simply is the current top threshold.
pub(crate) fn migrate_growth_v2(growth: u32) -> u32 {
    let (level, floor, next) = v2::level_for_growth(growth);
    let new_floor = threshold_for(level);
    let new_next = if level < MAX_LEVEL {
        threshold_for(level + 1)
    } else {
        new_floor
    };
    if new_next <= new_floor || next <= floor {
        return new_floor;
    }
    let span = u64::from(next - floor);
    let paid = u64::from(growth - floor);
    let new_span = u64::from(new_next - new_floor);
    let carried = (paid * new_span) / span;
    new_floor.saturating_add(u32::try_from(carried).unwrap_or(u32::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_level_is_read_as_a_layer_of_ten_ranks() {
        let rank = |level| layer_rank(level);
        assert_eq!(rank(1), LayerRank { layer: 0, rank: 1 });
        assert_eq!(
            rank(89),
            LayerRank { layer: 8, rank: 9 },
            "the way the user says it: 炼气 8 层 9 级"
        );
        assert_eq!(rank(10), LayerRank { layer: 0, rank: 10 });
        assert_eq!(rank(11), LayerRank { layer: 1, rank: 1 });
        // A realm is ten layers; the hundred-and-first level starts the next one
        // back at its first layer.
        assert_eq!(rank(100), LayerRank { layer: 9, rank: 10 });
        assert_eq!(rank(101), LayerRank { layer: 0, rank: 1 });
    }

    /// The one worked example the brief gives, checked end to end: 元婴 is the
    /// fourth realm, so its 4 层 7 阶 is level 347 of 1000. The conversion is
    /// `(realm − 1) × 100 + layer × 10 + rank`, with the layer counted from 0
    /// exactly as the page prints it.
    #[test]
    fn a_realm_layer_and_rank_add_up_to_the_level_the_brief_names() {
        let level = 3 * 100 + 4 * 10 + 7;
        assert_eq!(level, 347);
        assert_eq!(super::super::Realm::from_level(level).index(), 3);
        let said = layer_rank(level);
        assert_eq!((said.layer, said.rank), (4, 7), "元婴 · 4 层 7 阶");
        // And the level is what the growth curve says it is.
        assert_eq!(level_for_growth(threshold_for(level)).level, level);
        // The whole ladder is ten realms of a hundred levels.
        assert_eq!(MAX_LEVEL, 1_000);
    }

    #[test]
    fn the_ladder_opens_where_the_realm_table_says() {
        assert_eq!(threshold_for(1), 0);
        assert_eq!(threshold_for(101), 10_000, "the second realm");
        assert_eq!(threshold_for(201), 30_000);
        assert_eq!(
            threshold_for(MAX_LEVEL),
            94_300_000,
            "the last layer of the last realm"
        );
    }

    #[test]
    fn every_level_inverts_from_its_own_threshold() {
        let mut previous = 0;
        for level in 1..=MAX_LEVEL {
            let threshold = threshold_for(level);
            assert!(threshold >= previous, "the curve regressed at {level}");
            assert_eq!(
                level_for_growth(threshold).level,
                level,
                "inversion failed at {level}"
            );
            previous = threshold;
        }
    }

    #[test]
    fn a_growth_total_between_two_levels_belongs_to_the_lower_one() {
        let just_below = threshold_for(50) - 1;
        assert_eq!(level_for_growth(just_below).level, 49);
        assert_eq!(level_for_growth(u32::MAX).level, MAX_LEVEL);
        assert_eq!(level_for_growth(u32::MAX).next, None);
    }

    #[test]
    fn migration_keeps_the_level_and_its_fraction_of_the_next() {
        // 4_986 sat between v1's thresholds of level 217 (4_960) and 218
        // (5_020): 26/60 of the way up. The move must land in the same place
        // on the current ramp, not at the level and not past it.
        assert_eq!(migrate_growth_v1(4_986), 38_216);
        assert_eq!(level_for_growth(migrate_growth_v1(4_986)).level, 217);
        // A total exactly on a v1 threshold carries no fraction.
        assert_eq!(
            migrate_growth_v1(v1::threshold_for(217)),
            threshold_for(217)
        );
    }

    #[test]
    fn migration_v2_keeps_the_level_and_its_fraction_of_the_next() {
        // 300_000 sat exactly on v2's threshold for level 441 (化神 5 层 1 阶):
        // the move lands on the same level of the new ramp, carrying no fraction.
        assert_eq!(migrate_growth_v2(300_000), 880_000);
        assert_eq!(level_for_growth(migrate_growth_v2(300_000)).level, 441);
        // A total partway up keeps its fraction of the way to the next level.
        // v2: 301_250 is 1250/2500 of the way from 300_000 to 302_500; the new
        // ramp's layer is 15 000 wide, so the move lands 7 500 past its floor.
        let moved = migrate_growth_v2(301_250);
        assert_eq!(moved, 887_500);
        assert_eq!(level_for_growth(moved).level, 441);
        // Growth at the very top stays on the top level.
        let at_top = migrate_growth_v2(18_000_000);
        assert_eq!(level_for_growth(at_top).level, MAX_LEVEL);
    }

    #[test]
    fn migration_never_lands_below_the_level_it_keeps() {
        for growth in [
            0,
            1,
            60,
            1_188,
            4_986,
            100_000,
            1_095_000,
            1_100_000,
            u32::MAX,
        ] {
            let moved = migrate_growth_v1(growth);
            let (level, _, _) = v1::level_for_growth(growth);
            assert_eq!(
                level_for_growth(moved).level,
                level,
                "the level moved at growth {growth}"
            );
        }
    }
}
