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
/// The ramp is authored, not derived: the first realm is a hundred sessions of
/// ordinary use, and every realm after it costs roughly twice the last. Every
/// difference is a multiple of [`LAYERS_PER_STAGE`] so each realm's hundred
/// layers are an integer step apart.
const STAGE_FLOORS: [u32; STAGE_COUNT] = [
    0, 1_200, 4_000, 10_000, 22_000, 45_000, 90_000, 170_000, 320_000, 600_000,
];

/// Where the last realm's hundredth layer sits: the top of the ladder.
const TOP_GROWTH: u32 = 1_100_000;

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

    #[test]
    fn the_ladder_opens_where_the_realm_table_says() {
        assert_eq!(threshold_for(1), 0);
        assert_eq!(threshold_for(101), 1_200, "the second realm");
        assert_eq!(threshold_for(201), 4_000);
        assert_eq!(
            threshold_for(MAX_LEVEL),
            1_095_000,
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
}
