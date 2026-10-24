//! The herb garden: seeds, the plots they are planted in, and the harvest that
//! permanently raises an attribute.
//!
//! A crop matures on the wall clock rather than on focus time, so a session
//! planted before lunch is pickable after it. The growth time scales with the
//! seed's tier, and steeply: a third-tier seed is an afternoon's project, and
//! every tier past it triples the wait, which is what keeps a late-realm seed
//! worth the wait — and worth owning several plots for.
//!
//! A seed carries two numbers, and they answer different questions. Its **tier**
//! is how deep it stands — which realm's patch it came off, which is also how
//! long it takes to come up. Its **品阶** is how good it is: 黄、玄、地、天、神,
//! the same five rungs a 功法 is graded on and the same five colours. A 神阶 seed
//! of an early tier is worth more than a 黄阶 one of the same tier and no slower
//! to grow, which is exactly what makes the grade worth finding.

use serde::{Deserialize, Serialize};

use super::{
    ArtGrade, Attribute, Rng, SEED_RETURN_PERMILLE, SEED_RETURN_UPGRADE_PERCENT, STAGE_COUNT,
    arts::roll_art_grade,
};

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
///
/// The grade does not enter: a 神阶 seed of the third tier is still an
/// afternoon's crop. What the grade buys is the harvest, not the wait, so the
/// plot a user gives up is the same plot whatever comes up out of it.
pub fn grow_seconds(tier: u8) -> i64 {
    match tier.clamp(1, MAX_SEED_TIER) {
        1 => 1_800,
        2 => 4_800,
        t => 14_400 * 3i64.pow(u32::from(t - 3)),
    }
}

/// The five rungs a seed is graded on: 黄、玄、地、天、神.
///
/// The same five, in the same order and the same colours, as a 功法 — a 地阶
/// herb and a 地阶 功法 are the same word about the same shelf. The grade is the
/// herb's *quality*: it multiplies what the harvest pays and nothing else, so a
/// 神阶 seed is a better plant rather than a different plant.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SeedGrade {
    /// 黄: the ordinary herb, worth two fifths of the anchor.
    Yellow,
    /// 玄: worth seven tenths.
    Profound,
    /// 地: the anchor itself — every seed a save held before grades existed is
    /// read as this one, so an upgraded save's garden pays what it always paid.
    #[default]
    Earth,
    /// 天: worth half again.
    Heaven,
    /// 神: worth more than twice the anchor, and the rarest thing a garden grows.
    Divine,
}

impl SeedGrade {
    /// Every grade, weakest first.
    pub const ALL: [Self; 5] = [
        Self::Yellow,
        Self::Profound,
        Self::Earth,
        Self::Heaven,
        Self::Divine,
    ];

    /// The grade's position in [`Self::ALL`].
    pub const fn index(self) -> usize {
        match self {
            Self::Yellow => 0,
            Self::Profound => 1,
            Self::Earth => 2,
            Self::Heaven => 3,
            Self::Divine => 4,
        }
    }

    /// The grade at a position, clamped to the last one.
    pub fn from_index(index: usize) -> Self {
        Self::ALL[index.min(Self::ALL.len() - 1)]
    }

    /// The seed grade a 功法 grade names: the two ladders are one ladder.
    pub const fn from_art_grade(grade: ArtGrade) -> Self {
        match grade {
            ArtGrade::Yellow => Self::Yellow,
            ArtGrade::Profound => Self::Profound,
            ArtGrade::Earth => Self::Earth,
            ArtGrade::Heaven => Self::Heaven,
            ArtGrade::Divine => Self::Divine,
        }
    }

    /// The catalog key that names this grade — the 功法 ladder's own words, so a
    /// 地阶 herb and a 地阶 book cannot drift apart in the catalog.
    pub const fn name_key(self) -> &'static str {
        self.as_art_grade().name_key()
    }

    /// The grade as the 功法 ladder's own value.
    pub const fn as_art_grade(self) -> ArtGrade {
        match self {
            Self::Yellow => ArtGrade::Yellow,
            Self::Profound => ArtGrade::Profound,
            Self::Earth => ArtGrade::Earth,
            Self::Heaven => ArtGrade::Heaven,
            Self::Divine => ArtGrade::Divine,
        }
    }

    /// The grade's colour, as `0xRRGGBB`: the same five a 功法 uses.
    pub const fn color(self) -> u32 {
        self.as_art_grade().color()
    }

    /// What the grade multiplies a harvest by, in parts per thousand.
    ///
    /// 400 / 700 / 1000 / 1500 / 2200 — every step at least a third more than
    /// the one below it, so no two grades are "about the same" on the row. The
    /// anchor is 地 at a thousand, which is what the old garden paid: the grade
    /// was added under existing numbers rather than on top of them.
    pub const fn multiplier_permille(self) -> u32 {
        match self {
            Self::Yellow => 400,
            Self::Profound => 700,
            Self::Earth => 1_000,
            Self::Heaven => 1_500,
            Self::Divine => 2_200,
        }
    }
}

/// Which grade a dropped seed carries.
///
/// The 功法 ladder's own five-grade roll — 六成黄、两成五玄、一成地、四分天、
/// 万分之一神 — because the brief asks for the same odds and one table is the
/// only way two rolls cannot drift apart.
pub(crate) fn roll_seed_grade(rng: &mut Rng) -> SeedGrade {
    SeedGrade::from_art_grade(roll_art_grade(rng))
}

/// The attribute points one harvest of this tier and grade pays.
///
/// The tier sets the base — [`harvest_points_at_earth`] — and the grade
/// multiplies it, rounded rather than floored: a 黄阶 first-tier herb pays three
/// points rather than two, because a herb that paid nothing would read as a
/// failed harvest.
pub fn harvest_points(tier: u8, grade: SeedGrade) -> i32 {
    let base = i64::from(harvest_points_at_earth(tier));
    let scaled = base * i64::from(grade.multiplier_permille());
    let rounded = (scaled + 500) / 1_000;
    i32::try_from(rounded.max(1)).unwrap_or(i32::MAX)
}

/// What a 地阶 seed of this tier pays: the garden's anchor, and the figure every
/// seed a save held before grades existed already paid.
pub fn harvest_points_at_earth(tier: u8) -> i32 {
    5 + 3 * i32::from(tier.clamp(1, MAX_SEED_TIER))
}

/// The seed one harvest gives back, if it gives one.
///
/// Most harvests pay only their attribute points. [`SEED_RETURN_PERMILLE`]
/// decides whether a seed comes back at all, [`SEED_RETURN_UPGRADE_PERCENT`]
/// decides whether it comes back a tier higher — and the grade rolls fresh, so a
/// garden is the one place a 神阶 seed can be grown into rather than found.
pub fn seed_return(
    attribute: Attribute,
    tier: u8,
    rng: &mut Rng,
) -> Option<(Attribute, u8, SeedGrade)> {
    if rng.below(1_000) >= SEED_RETURN_PERMILLE {
        return None;
    }
    let tier = if rng.below(100) < SEED_RETURN_UPGRADE_PERCENT {
        tier.saturating_add(1).min(MAX_SEED_TIER)
    } else {
        tier.clamp(1, MAX_SEED_TIER)
    };
    Some((attribute, tier, roll_seed_grade(rng)))
}

/// A stack of identical seeds: one attribute, one tier, one grade.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SeedStack {
    pub attribute: Attribute,
    pub tier: u8,
    /// The herb's grade. A seed from before grades existed reads as 地阶 — the
    /// numbers it already paid — so an upgraded save's garden is worth exactly
    /// what it was.
    #[serde(default)]
    pub grade: SeedGrade,
    pub count: u32,
    /// The herb's xianxia-given name, if the stack carries one.
    #[serde(default)]
    pub name: Option<String>,
}

impl SeedStack {
    /// A stack of one seed.
    pub const fn single(attribute: Attribute, tier: u8, grade: SeedGrade) -> Self {
        Self {
            attribute,
            tier,
            grade,
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
    /// The grade the seed carried, which is what the harvest will pay at.
    #[serde(default)]
    pub grade: SeedGrade,
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
    pub fn planted(
        attribute: Attribute,
        tier: u8,
        grade: SeedGrade,
        name: Option<String>,
        now_unix: i64,
    ) -> Self {
        let tier = tier.clamp(1, MAX_SEED_TIER);
        Self {
            attribute,
            tier,
            grade,
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
        let crop = CropRecord::planted(Attribute::Root, 2, SeedGrade::Earth, None, 1_000);
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
        assert!(harvest_points(2, SeedGrade::Earth) > harvest_points(1, SeedGrade::Earth));
        assert!(harvest_points(10, SeedGrade::Earth) > harvest_points(9, SeedGrade::Earth));
    }

    /// The brief's anchor: the numbers a save already grew are the 地阶 ones,
    /// and the four other grades are read against them.
    #[test]
    fn the_earth_grade_is_the_number_the_garden_already_paid() {
        for tier in 1..=MAX_SEED_TIER {
            assert_eq!(
                harvest_points(tier, SeedGrade::Earth),
                5 + 3 * i32::from(tier)
            );
        }
        assert_eq!(SeedGrade::Earth.multiplier_permille(), 1_000);
        assert_eq!(
            SeedGrade::default(),
            SeedGrade::Earth,
            "an old seed is 地阶"
        );
    }

    /// Every grade is a clear step from the one below it, and the whole ladder
    /// spans more than five times from 黄 to 神.
    #[test]
    fn the_five_grades_are_far_apart() {
        for pair in SeedGrade::ALL.windows(2) {
            let (worse, better) = (pair[0], pair[1]);
            assert!(
                better.multiplier_permille() * 100 >= worse.multiplier_permille() * 130,
                "{better:?} is not a clear step above {worse:?}"
            );
        }
        assert!(
            SeedGrade::Divine.multiplier_permille() >= SeedGrade::Yellow.multiplier_permille() * 5,
            "神 has to be worth several 黄"
        );
        for tier in [1_u8, 5, 10] {
            let payouts: Vec<i32> = SeedGrade::ALL
                .into_iter()
                .map(|grade| harvest_points(tier, grade))
                .collect();
            assert!(
                payouts.windows(2).all(|pair| pair[1] > pair[0]),
                "tier {tier} pays the grades out of order: {payouts:?}"
            );
        }
        // A 神阶 seed of the first tier beats a 黄阶 one of the last: the grade
        // is worth more than the patch it grew in.
        assert!(
            harvest_points(1, SeedGrade::Divine) > harvest_points(MAX_SEED_TIER, SeedGrade::Yellow)
        );
    }

    /// The grades share the 功法 ladder's odds and its five colours.
    #[test]
    fn the_grades_are_the_arts_own_ladder() {
        for grade in SeedGrade::ALL {
            assert_eq!(grade.name_key(), grade.as_art_grade().name_key());
            assert_eq!(grade.color(), grade.as_art_grade().color());
        }
        assert_eq!(
            SeedGrade::from_art_grade(ArtGrade::Divine),
            SeedGrade::Divine
        );
        // The roll reaches every grade, and the bottom one is the commonest.
        let mut counts = [0_u32; 5];
        let mut rng = Rng::seeded(0x0000_5EED_0BAD_F00D);
        for _ in 0..20_000 {
            counts[roll_seed_grade(&mut rng).index()] += 1;
        }
        assert!(counts.iter().all(|count| *count > 0), "{counts:?}");
        assert!(
            counts[0] > counts[1] && counts[1] > counts[2] && counts[2] > counts[3],
            "the grades are not bottom-heavy: {counts:?}"
        );
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
