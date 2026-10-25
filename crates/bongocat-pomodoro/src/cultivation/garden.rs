//! The herb garden: seeds, the plots they are planted in, and the harvest that
//! permanently raises an attribute.
//!
//! A crop matures on the wall clock rather than on focus time, so a session
//! planted before lunch is pickable after it. The growth time scales with the
//! seed's tier: eight hours at the first, eighteen at the third, a day at the
//! fourth, and ten days at the tenth. The ramp past the fourth tier is gentle
//! on purpose — the harvest is paid by the hour (see
//! [`harvest_points`] and [`HARVEST_PERMILLE_PER_HOUR`]), so a tier that waited
//! three times as long as the one below would pay three times as much, and a
//! trebling ladder would put the tenth tier a year out and make a legendary
//! seed a promise nobody could keep.
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
/// Eight hours, twelve, eighteen, a day, and then a climb to ten days at the
/// tenth. Every wait is long enough that the harvest is a *crop* rather than a
/// notification: the payout is linear in the wait, so an eight-hour first tier
/// is what lets the grade ladder be read at all — half an hour of anything
/// rounds to a single point, and five grades that all read "1" are five grades
/// the page cannot tell apart.
///
/// The grade does not enter: a 神阶 seed of the third tier is still an
/// eighteen-hour crop. What the grade buys is the harvest, not the wait, so the
/// plot a user gives up is the same plot whatever comes up out of it.
const GROW_SECONDS: [i64; MAX_SEED_TIER as usize] = [
    28_800,  // 1: eight hours
    43_200,  // 2: twelve hours
    64_800,  // 3: eighteen hours
    86_400,  // 4: a day
    129_600, // 5: thirty-six hours
    172_800, // 6: two days
    259_200, // 7: three days
    345_600, // 8: four days
    518_400, // 9: six days
    864_000, // 10: ten days
];

/// How long a seed of this tier needs to mature, in seconds.
pub fn grow_seconds(tier: u8) -> i64 {
    GROW_SECONDS[usize::from(tier.clamp(1, MAX_SEED_TIER) - 1)]
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
    /// 黄: the ordinary herb, worth a quarter of the anchor.
    Yellow,
    /// 玄: worth three fifths of it.
    Profound,
    /// 地: the anchor itself — every seed a save held before grades existed is
    /// read as this one, so an upgraded save's garden pays what it always paid.
    #[default]
    Earth,
    /// 天: worth half again as much.
    Heaven,
    /// 神: worth two and a half times the anchor, and the rarest thing a garden
    /// grows.
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
    /// 250 / 600 / 1000 / 1600 / 2500 — every step at least half again the one
    /// below it, and the whole ladder ten times from 黄 to 神, so the grade is
    /// what the row is worth and not a rounding difference. The anchor is 地 at
    /// a thousand, which is what the old garden paid: the grade was added under
    /// existing numbers rather than on top of them.
    ///
    /// The bottom rungs are deliberately poor. A patch is paid by the hour, so
    /// a 黄阶 seed pays a quarter of what a 地阶 one does for the same wait —
    /// the herb a beginner can afford is the one that grows the least, which is
    /// what keeps "plant the cheapest thing, always" from being the answer the
    /// garden prints. At the first tier that quarter is a single point for
    /// eight hours; at the tenth it is twenty-four for ten days.
    pub const fn multiplier_permille(self) -> u32 {
        match self {
            Self::Yellow => 250,
            Self::Profound => 600,
            Self::Earth => 1_000,
            Self::Heaven => 1_600,
            Self::Divine => 2_500,
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

/// What one hour in the ground pays, in parts per thousand of an attribute
/// point, at the 地 anchor grade: 0.4.
///
/// Read off the other ladder rather than invented, and then cut. A 玄阶 功法
/// pays its grade's magnitude for a rank — eight points over one to three
/// sessions — while a crop that paid even a point an hour would hand out a
/// hundred points for four days in the ground, and a garden would be the loudest
/// ladder in the game rather than the quiet one. Four tenths of a point an hour
/// puts a 地阶 plot at three points for its first tier, ten for a day at the
/// fourth and ninety-six for ten days at the tenth: a crop is worth a fraction
/// of a session for every session it takes, which is what keeps the desk the
/// place the numbers actually come from.
pub const HARVEST_PERMILLE_PER_HOUR: i64 = 400;

/// The attribute points one harvest of this tier and grade pays.
///
/// Linear in the wait: the tier's growing time, times [`HARVEST_PERMILLE_PER_HOUR`],
/// times the grade's own multiplier. Linear rather than a per-tier table
/// because it is the only shape in which a longer crop is never a worse crop —
/// under the old flat curve a first-tier herb paid twelve points an hour and a
/// tenth-tier one paid half a point, and the garden's answer was to buy the
/// cheapest seed on the shelf and plant it for ever.
///
/// Rounded rather than floored, and never below one: a herb that paid nothing
/// would read as a failed harvest rather than as a cheap one.
pub fn harvest_points(tier: u8, grade: SeedGrade) -> i32 {
    let seconds = grow_seconds(tier);
    // Thousandths of a point, so three integer factors cannot lose the fraction
    // between them: seconds × permille-per-hour × grade-permille.
    let milli = seconds
        .saturating_mul(HARVEST_PERMILLE_PER_HOUR)
        .saturating_mul(i64::from(grade.multiplier_permille()))
        / 3_600_000;
    i32::try_from((milli + 500) / 1_000)
        .unwrap_or(i32::MAX)
        .max(1)
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
        let ready = 1_000 + grow_seconds(2);
        let crop = CropRecord::planted(Attribute::Root, 2, SeedGrade::Earth, None, 1_000);
        assert_eq!(crop.ready_at_unix, ready);
        assert!(!crop.is_ready(1_000));
        assert!(!crop.is_ready(ready - 1));
        assert!(crop.is_ready(ready));
        assert_eq!(
            crop.remaining_seconds(1_000),
            u32::try_from(grow_seconds(2)).unwrap()
        );
        assert_eq!(crop.remaining_seconds(ready + 1), 0);
    }

    #[test]
    fn a_higher_tier_seed_takes_longer_and_pays_more() {
        assert!(grow_seconds(2) > grow_seconds(1));
        assert!(harvest_points(2, SeedGrade::Earth) > harvest_points(1, SeedGrade::Earth));
        assert!(harvest_points(10, SeedGrade::Earth) > harvest_points(9, SeedGrade::Earth));
    }

    /// The anchor: 地 is a thousand permille of the base rate, and every other
    /// grade is read against it. The whole curve — wait and payout — is pinned
    /// here, because the two halves were retuned together: a table that moved
    /// one without the other would still pass a per-tier monotonicity check.
    #[test]
    fn the_earth_grade_is_the_number_the_garden_pays() {
        let earth: Vec<i32> = (1..=MAX_SEED_TIER)
            .map(|tier| harvest_points(tier, SeedGrade::Earth))
            .collect();
        assert_eq!(
            earth,
            vec![3, 5, 7, 10, 14, 19, 29, 38, 58, 96],
            "a 地阶 plot pays its hours"
        );
        assert_eq!(
            grow_seconds(4),
            24 * 3_600,
            "a fourth-tier seed takes a day"
        );
        assert_eq!(SeedGrade::Earth.multiplier_permille(), 1_000);
        assert_eq!(
            SeedGrade::default(),
            SeedGrade::Earth,
            "an old seed is 地阶"
        );
    }

    /// Every grade is a clear step from the one below it, and the whole ladder
    /// spans ten times from 黄 to 神.
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
        // Every tier tells the five apart, the first one included: an
        // eight-hour first tier is exactly long enough that 黄 reads 1 and 神
        // reads 8, which is the whole reason the wait was lengthened.
        for tier in 1..=MAX_SEED_TIER {
            let payouts: Vec<i32> = SeedGrade::ALL
                .into_iter()
                .map(|grade| harvest_points(tier, grade))
                .collect();
            assert!(
                payouts.windows(2).all(|pair| pair[1] > pair[0]),
                "tier {tier} pays the grades out of order: {payouts:?}"
            );
        }
        assert_eq!(
            SeedGrade::ALL
                .into_iter()
                .map(|grade| harvest_points(1, grade))
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 5, 8],
            "the first tier is a ladder, not a row of ones"
        );
        // The cheap seed is not the efficient one: an hour of a 黄阶 plot pays a
        // quarter of an hour of a 地阶 plot, and the four grades of any tier keep
        // that ratio.
        for tier in 1..=MAX_SEED_TIER {
            assert!(
                harvest_points(tier, SeedGrade::Yellow) < harvest_points(tier, SeedGrade::Earth),
                "tier {tier}"
            );
        }
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
    fn the_growing_time_anchors_on_tiers_one_and_ten() {
        assert_eq!(grow_seconds(1), 8 * 3_600, "a first-tier seed takes 8h");
        assert_eq!(grow_seconds(3), 18 * 3_600, "a third-tier seed takes 18h");
        assert_eq!(
            grow_seconds(4),
            24 * 3_600,
            "a fourth-tier seed takes a day"
        );
        assert_eq!(grow_seconds(10), 10 * 24 * 3_600, "and ten days at the top");
        // Monotone, so no tier is ever a cheaper wait than the one below it.
        for tier in 2..=MAX_SEED_TIER {
            assert!(grow_seconds(tier) > grow_seconds(tier - 1), "tier {tier}");
        }
        // And the payout per hour stays inside a narrow band: no tier is a
        // bargain in time, which is what used to make the cheapest seed the
        // only one worth planting.
        for tier in 1..=MAX_SEED_TIER {
            let per_hour = harvest_points(tier, SeedGrade::Earth) * 1_000
                / (grow_seconds(tier) / 3_600) as i32;
            assert!(
                (300..=500).contains(&per_hour),
                "tier {tier} pays {per_hour} thousandths of a point an hour"
            );
        }
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
