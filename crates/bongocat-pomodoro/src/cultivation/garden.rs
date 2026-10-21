//! The herb garden: seeds, the plots they are planted in, and the harvest that
//! permanently raises an attribute.
//!
//! A crop matures on the wall clock rather than on focus time, so a session
//! planted before lunch is pickable after it. The growth time scales with the
//! seed's tier, and steeply: a third-tier seed is an afternoon's project, and
//! every tier past it triples the wait, which is what keeps a late-realm seed
//! worth the wait — and worth owning several plots for.

use serde::{Deserialize, Serialize};

use super::{Attribute, Rng, SEED_RETURN_PERMILLE, SEED_RETURN_UPGRADE_PERCENT, STAGE_COUNT};

/// How many plots a cultivator starts with. Every plot is independent, so a
/// user can keep several crops of different lengths growing at once.
pub const FREE_PLOT_COUNT: usize = 4;

/// How many plots a garden can hold at all: the four free ones and five that
/// are bought with 灵石.
pub const MAX_PLOT_COUNT: usize = 9;

/// What the next plot costs, by how many have been bought already: the fifth
/// plot is the cheapest and every one after it doubles.
pub const PLOT_PRICES: [u32; MAX_PLOT_COUNT - FREE_PLOT_COUNT] = [150, 300, 600, 1200, 2400];

/// What buying one more plot costs, given how many the garden already holds.
///
/// `None` once the garden is full: a price for a plot that cannot exist would
/// be a button the domain has to keep refusing.
pub fn plot_price(owned: usize) -> Option<u32> {
    owned
        .checked_sub(FREE_PLOT_COUNT)?
        .lt(&PLOT_PRICES.len())
        .then(|| PLOT_PRICES[owned - FREE_PLOT_COUNT])
}

/// The highest seed tier: one per realm, so a seed's tier names the realm that
/// dropped it.
pub const MAX_SEED_TIER: u8 = STAGE_COUNT as u8;

/// How long a seed of this tier needs to mature, in seconds.
///
/// The third tier takes four hours, the fourth twelve, and every tier past
/// that triples again — a high-realm seed is a project, not a lunch break. The
/// two tiers below the anchor are the same curve read backwards: about half an
/// hour and about eighty minutes.
pub fn grow_seconds(tier: u8) -> i64 {
    match tier.clamp(1, MAX_SEED_TIER) {
        1 => 1_800,
        2 => 4_800,
        t => 14_400 * 3i64.pow(u32::from(t - 3)),
    }
}

/// The attribute points one harvest of this tier pays.
pub fn harvest_points(tier: u8) -> i32 {
    5 + 3 * i32::from(tier.clamp(1, MAX_SEED_TIER))
}

/// The seed one harvest gives back, if it gives one.
///
/// Most harvests pay only their attribute points. [`SEED_RETURN_PERMILLE`]
/// decides whether a seed comes back at all, and
/// [`SEED_RETURN_UPGRADE_PERCENT`] decides whether it comes back a tier higher —
/// which is the slow way a garden becomes self-sustaining.
pub fn seed_return(attribute: Attribute, tier: u8, rng: &mut Rng) -> Option<(Attribute, u8)> {
    if rng.below(1_000) >= SEED_RETURN_PERMILLE {
        return None;
    }
    let tier = if rng.below(100) < SEED_RETURN_UPGRADE_PERCENT {
        tier.saturating_add(1).min(MAX_SEED_TIER)
    } else {
        tier.clamp(1, MAX_SEED_TIER)
    };
    Some((attribute, tier))
}

/// A stack of identical seeds: one attribute, one tier.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SeedStack {
    pub attribute: Attribute,
    pub tier: u8,
    pub count: u32,
    /// The herb's xianxia-given name, if the stack carries one.
    #[serde(default)]
    pub name: Option<String>,
}

impl SeedStack {
    /// A stack of one seed.
    pub const fn single(attribute: Attribute, tier: u8) -> Self {
        Self {
            attribute,
            tier,
            count: 1,
            name: None,
        }
    }
}

/// The epithets a herb may carry.
const SEED_PREFIXES: &[&str] = &[
    "青", "翠", "灵", "仙", "玉", "玄", "碧", "紫", "朱", "赤", "白", "金", "幽", "寒", "月", "星",
    "云", "露", "霜", "雪", "龙", "凤", "麒麟", "朱雀", "玄武", "青龙", "白虎", "太", "九", "昊",
    "幻", "虚", "梦", "瑶", "璇", "苍",
];

/// The herbs a garden grows.
const SEED_NOUNS: &[&str] = &[
    "草", "花", "莲", "芝", "参", "果", "叶", "根", "藤", "兰", "菊", "梅", "桃", "苓", "蕈", "荷",
    "蓉", "蕊", "蔓", "芷", "蔻",
];

/// A xianxia name for one dropped seed.
///
/// The prefix and the noun are rolled independently, so the pool holds hundreds of
/// distinct herb names — well over five hundred — for a garden to draw from.
pub(crate) fn seed_name(rng: &mut Rng) -> String {
    let prefix = SEED_PREFIXES[rng.below(SEED_PREFIXES.len() as u64) as usize];
    let noun = SEED_NOUNS[rng.below(SEED_NOUNS.len() as u64) as usize];
    format!("{prefix}{noun}")
}

/// A crop growing in a plot, or one ready to be picked.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CropRecord {
    /// The attribute the harvest will raise.
    pub attribute: Attribute,
    pub tier: u8,
    /// The herb's xianxia-given name, carried over from the seed stack it was
    /// planted from: the plot says what is growing in it, and the harvest line
    /// can say which herb came up. An old save's crops predate the field and
    /// read as the bare attribute label they always showed.
    #[serde(default)]
    pub name: Option<String>,
    pub planted_at_unix: i64,
    pub ready_at_unix: i64,
}

impl CropRecord {
    /// A crop planted now, maturing after its tier's growing time.
    pub fn planted(attribute: Attribute, tier: u8, name: Option<String>, now_unix: i64) -> Self {
        let tier = tier.clamp(1, MAX_SEED_TIER);
        Self {
            attribute,
            tier,
            name,
            planted_at_unix: now_unix,
            ready_at_unix: now_unix.saturating_add(grow_seconds(tier)),
        }
    }

    /// Whether the crop can be picked at this instant.
    pub fn is_ready(&self, now_unix: i64) -> bool {
        now_unix >= self.ready_at_unix
    }

    /// How long is left before the crop is ready; zero once it is.
    pub fn remaining_seconds(&self, now_unix: i64) -> u32 {
        u32::try_from(self.ready_at_unix.saturating_sub(now_unix).max(0)).unwrap_or(u32::MAX)
    }
}

/// One plot: empty, growing, or ready.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PlotRecord {
    #[serde(default)]
    pub crop: Option<CropRecord>,
}

impl PlotRecord {
    /// Whether a crop can be planted here.
    pub const fn is_empty(&self) -> bool {
        self.crop.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_crop_is_ready_only_after_its_growing_time() {
        let crop = CropRecord::planted(Attribute::Root, 2, None, 1_000);
        assert_eq!(crop.ready_at_unix, 1_000 + 4_800);
        assert!(!crop.is_ready(1_000));
        assert!(!crop.is_ready(5_799));
        assert!(crop.is_ready(5_800));
        assert_eq!(crop.remaining_seconds(1_000), 4_800);
        assert_eq!(crop.remaining_seconds(9_999), 0);
    }

    #[test]
    fn a_higher_tier_seed_takes_longer_and_pays_more() {
        assert!(grow_seconds(2) > grow_seconds(1));
        assert!(harvest_points(2) > harvest_points(1));
        assert!(harvest_points(10) > harvest_points(9));
    }

    #[test]
    fn the_growing_time_anchors_on_tiers_three_and_four() {
        assert_eq!(grow_seconds(3), 4 * 3_600, "a third-tier seed takes 4h");
        assert_eq!(grow_seconds(4), 12 * 3_600, "a fourth-tier seed takes 12h");
        assert_eq!(grow_seconds(5), 36 * 3_600, "and the tripling carries on");
    }

    #[test]
    fn the_fifth_plot_costs_and_the_tenth_does_not_exist() {
        assert_eq!(plot_price(3), None, "the four free plots are free");
        assert_eq!(plot_price(4), Some(150));
        assert_eq!(plot_price(5), Some(300));
        assert_eq!(plot_price(8), Some(2400));
        assert_eq!(plot_price(9), None, "nine is the whole garden");
    }
}
