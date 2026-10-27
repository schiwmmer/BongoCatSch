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
    ArtGrade, Attribute, Rng, SEED_RETURN_PERMILLE_PER_RANK, SEED_RETURN_UPGRADE_PERCENT,
    STAGE_COUNT, arts::roll_art_grade,
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

/// The odds one harvest gives a seed back, in parts per thousand.
///
/// One percent per 阶 of the 仙草园: the first 阶 rolls one percent, and the
/// fifteenth — the top of the garden — rolls fifteen. Fifteen was the flat rate
/// a garden always paid, so a garden that has been raised all the way returns a
/// seed about as often as it always did; a young garden returns fewer, and the
/// 阶 the cultivator has spent to buy are the ones that buy the second seed
/// back. Capped at the top 阶, so a rank past the top cannot outrun the ladder.
pub fn seed_return_permille(garden_rank: u8) -> u64 {
    let rank = garden_rank.min(MAX_GARDEN_RANK);
    (rank as u64)
        .saturating_mul(SEED_RETURN_PERMILLE_PER_RANK)
        .min(SEED_RETURN_PERMILLE_PER_RANK * MAX_GARDEN_RANK as u64)
        .max(1)
}

/// The seed one harvest gives back, if it gives one.
///
/// Most harvests pay only their attribute points. [`seed_return_permille`]
/// decides whether a seed comes back at all — one percent per 阶 of the 仙草园 —
/// [`SEED_RETURN_UPGRADE_PERCENT`] decides whether it comes back a tier higher,
/// and the grade rolls fresh, so a garden is the one place a 神阶 seed can be
/// grown into rather than found.
pub fn seed_return(
    attribute: Attribute,
    tier: u8,
    garden_rank: u8,
    rng: &mut Rng,
) -> Option<(Attribute, u8, SeedGrade)> {
    if rng.below(1_000) >= seed_return_permille(garden_rank) {
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
    /// The 潜质 herb this stack grows, when it is one of the two plantable herbs
    /// rather than an attribute herb.
    ///
    /// A herb stack carries a fixed tier — see [`herb_tier`] — and its
    /// `attribute` is a don't-care the surface never reads; every domain rule
    /// that matches on a stack matches on this field first, so a 悟心草 seed
    /// and an attribute seed of the same tier and grade are two different
    /// stacks. An old save's stacks predate the field and read as `None`, which
    /// is exactly what they always were.
    #[serde(default)]
    pub herb: Option<super::AptitudeHerb>,
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
            herb: None,
            tier,
            grade,
            count: 1,
            name: None,
        }
    }

    /// A stack of one 潜质 herb seed, at the tier its herb is fixed at.
    pub const fn herb_single(herb: super::AptitudeHerb, grade: SeedGrade) -> Self {
        Self {
            attribute: Attribute::Strength,
            herb: Some(herb),
            tier: herb_tier(herb),
            grade,
            count: 1,
            name: None,
        }
    }
}

/// The garden tier a 潜质 herb grows at, when it is one of the two plantable
/// herbs; `None` for the pill, which is never planted.
///
/// 慧心草 sits at tier six and 悟心草 at tier eight, so the rarer herb is the
/// slower one: a herb is priced by what its 潜质 band is worth, and the band
/// that is worth more asks for the longer bed.
pub const fn herb_tier(herb: super::AptitudeHerb) -> u8 {
    match herb {
        super::AptitudeHerb::Wisdom => 6,
        super::AptitudeHerb::Insight => 8,
        super::AptitudeHerb::Marrow => 0,
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
    /// The 潜质 herb this crop grows, when it is one of the two plantable herbs.
    ///
    /// A herb crop's harvest feeds the body rather than an attribute, so its
    /// `attribute` is a don't-care read exactly nowhere; see [`SeedStack::herb`]
    /// for why the field has to be matched first.
    #[serde(default)]
    pub herb: Option<super::AptitudeHerb>,
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
    /// Whether the seed failed to take.
    ///
    /// The roll is still made when the seed goes in — that is what makes the
    /// odds worth printing — but the gardener is not told until the crop is half
    /// grown, and the bed stays occupied until it is: a planting that was going
    /// to fail says so at the halfway mark, not the instant the seed is covered.
    /// A save written before the reveal existed never stored a dead crop (the
    /// bed was freed on the spot), so the field reads as false — which is what
    /// those beds always held.
    #[serde(default)]
    pub withered: bool,
    /// The instant the outcome becomes visible: half the growing time.
    #[serde(default)]
    pub revealed_at_unix: i64,
    /// Whether the failure has already been paid and logged. The 造化 and the
    /// line are written once, on the tick that first sees the crop past
    /// [`Self::revealed_at_unix`], and never again.
    #[serde(default)]
    pub settled: bool,
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
            herb: None,
            tier,
            grade,
            name,
            withered: false,
            revealed_at_unix: reveal_at_unix(now_unix, tier),
            settled: false,
            planted_at_unix: now_unix,
            ready_at_unix: now_unix.saturating_add(grow_seconds(tier)),
        }
    }

    /// A 潜质 herb crop planted now, at its herb's fixed tier.
    pub fn herb_planted(
        herb: super::AptitudeHerb,
        grade: SeedGrade,
        name: Option<String>,
        now_unix: i64,
    ) -> Self {
        let tier = herb_tier(herb).clamp(1, MAX_SEED_TIER);
        Self {
            attribute: Attribute::Strength,
            herb: Some(herb),
            tier,
            grade,
            name,
            withered: false,
            revealed_at_unix: reveal_at_unix(now_unix, tier),
            settled: false,
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

    /// Whether the gardener has been told how this crop is doing: half the
    /// growing time is the halfway mark a bed can be judged at.
    pub fn is_revealed(&self, now_unix: i64) -> bool {
        now_unix >= self.revealed_at_unix
    }

    /// Whether the plot is holding a dead crop the gardener can see.
    pub fn stands_dead(&self, now_unix: i64) -> bool {
        self.withered && self.is_revealed(now_unix)
    }
}

/// When a bed says how it is doing: half the crop's growing time.
///
/// The seed's roll happens at planting and the result is hidden until here, so
/// the halfway mark is the first moment the gardener may know. Half rather than
/// the end because the second half of a failed crop is time the bed could be
/// spending on the next seed.
pub fn reveal_at_unix(planted_at_unix: i64, tier: u8) -> i64 {
    planted_at_unix.saturating_add(grow_seconds(tier) / 2)
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

/// The highest rank a garden can be coaxed to.
///
/// Fifteen against a seed's ten, so the top of the garden is five whole ranks
/// above the deepest seed — wide enough that a gardener who has spent on it
/// can plant anything, narrow enough that getting there is the game.
pub const MAX_GARDEN_RANK: u8 = 15;

/// What a seed's grade asks of the garden, in tenths of a rank.
///
/// 黄 / 玄 / 地 / 天 / 神 = 0 / 0.6 / 1.3 / 1.9 / 2.5. The grade is read as
/// *extra depth the gardener has to make up* rather than as a subtraction from
/// the odds: a 神阶 seed is the same plant a 黄阶 one is, grown two and a half
/// ranks deeper, which is why planting one in a garden that has not earned those
/// ranks fails often enough to be a decision.
///
/// In tenths, and integer throughout, because "`f64::powf` rounded" is a phrase
/// that belongs in neither a save file nor a test assertion.
const GRADE_STEP_TENTHS: [i32; 5] = [0, 6, 13, 19, 25];

/// The odds one tenth of a rank above the seed buys, in permille: four, so a
/// whole rank is forty.
const UP_PERMILLE_PER_TENTH: i32 = 4;

/// What one tenth of a rank below the seed leaves: 984‰.
///
/// Compounded ten times that is 0.8503 — the 0.85 a whole rank below used to be
/// worth, kept to three decimals so the odds stay integers. Cutting it into
/// tenths is the whole reason the grade can be charged in ranks instead of in
/// points of success: one curve now answers both questions.
const DOWN_PERMILLE_PER_TENTH: i32 = 984;

/// The odds no planting falls below: eight percent, so even a 神阶 seed of the
/// tenth rank in a first-rank bed is a gamble and not a null-result.
const PLANT_FLOOR_PERMILLE: i32 = 80;

/// What odds a planting stands at, in parts per thousand.
///
/// One comparison, one number: the garden's rank against the seed's tier, with
/// the seed's grade counted as depth the gardener has to make up.
///
/// ```text
/// tenths = 10 × (garden rank − seed tier) − grade step
/// tenths ≥ 0 → 800 + 4 × tenths, capped at 1000      (80% at parity, +4%/rank)
/// tenths < 0 → 800 × 0.984^|tenths|, floored at 80
/// ```
///
/// So parity between garden and seed is eighty percent — the anchor — and it
/// falls away in *both* directions from there: every rank the seed is deeper
/// costs fifteen percent of what is left, every rank the garden has earned pays
/// four points of what is missing. A 神阶 seed is worth exactly its own extra
/// two and a half ranks, which is why a first-rank bed meets one at 13% and a
/// full garden still meets the tenth rank at 90%.
pub fn plant_permille(rank: u8, tier: u8, grade: SeedGrade) -> u32 {
    let depth = (i32::from(rank) - i32::from(tier.clamp(1, MAX_SEED_TIER))) * 10
        - GRADE_STEP_TENTHS[grade.index()];
    if depth >= 0 {
        return (800 + UP_PERMILLE_PER_TENTH * depth).min(1_000) as u32;
    }
    // Every tenth below the seed compounds once. A hundred and fifteen steps at
    // worst (a 神阶 tenth-tier seed in a first-rank bed), which is nothing to
    // count through and exactly reproducible — there is no float in this loop to
    // disagree with the table the page prints.
    let mut odds = 800;
    for _ in 0..-depth {
        odds = (odds * DOWN_PERMILLE_PER_TENTH + 500) / 1_000;
    }
    odds.max(PLANT_FLOOR_PERMILLE) as u32
}

/// Only a planting with odds at least this good is counted towards the safety
/// net: below even money the gardener is gambling, and a gamble is priced by
/// what it pays, not comforted by what comes next.
pub const PITY_MIN_PERMILLE: u32 = 500;

/// How many counted failures in a row it takes before the next planting of that
/// kind comes up whatever the odds say.
pub const PITY_STREAK: u32 = 2;

/// The 造化 the next rank needs, banked from every crop whether it came up or
/// not.
///
/// Five times the rank squared: twenty to reach the third, eighty to reach the
/// fifth, nine hundred and eighty for the last one. The increment grows on
/// purpose — reaching the fourth rank is a season's gardening, the fifteenth is
/// a career's — while the shape stays a thing the surface can put in a sentence.
pub const fn rank_xp(garden_rank: u8) -> i64 {
    let rank = if garden_rank == 0 {
        1_i64
    } else {
        garden_rank as i64
    };
    5 * rank * rank
}

/// Everything that has to have been harvested to stand at this rank: the sum of
/// `rank_xp` below it, `5(n−1)n(2n−1)/6` rather than a loop.
pub const fn rank_xp_total(garden_rank: u8) -> i64 {
    let mut n = garden_rank as i64;
    if n > MAX_GARDEN_RANK as i64 {
        n = MAX_GARDEN_RANK as i64;
    }
    5 * (n - 1) * n * (2 * n - 1) / 6
}

/// What the next rank costs in 灵石: `50 + 30(g−1) + 15(g−1)²`.
///
/// Read against [`rank_xp`] rather than invented, and this is why: a harvested
/// herb is worth its points twice over — the points themselves, or points × 14
/// sold at the market — so a rank priced in 灵石 is really priced in *how much
/// of the harvest the gardener has to sell instead of keeping*. The whole ladder
/// is fifteen thousand seven hundred and fifteen stones, which is twenty-two
/// percent of what the harvest it takes to reach the top would sell for. Twenty
/// two is the number chosen: a cheaper ladder never once asks "keep it or sell
/// it", and a dearer one takes the choice away by making keeping pointless.
pub const fn rank_cost(garden_rank: u8) -> u32 {
    let step = if garden_rank == 0 {
        0_i64
    } else {
        garden_rank as i64 - 1
    };
    (50 + 30 * step + 15 * step * step) as u32
}

/// The rank a gardener with this much 造化 banked stands at.
///
/// Every whole rank whose cumulative threshold the bank has reached, once per
/// launch at most, so a loop over fifteen thresholds costs nothing to read.
pub fn rank_for_xp(garden_xp: i64) -> u8 {
    let mut rank = 1;
    while rank < MAX_GARDEN_RANK && garden_xp >= rank_xp_total(rank + 1) {
        rank += 1;
    }
    rank
}

/// The 造化 a crop pays even when it does not come up: a fifth of what the
/// harvest would have paid, and never less than one.
///
/// Failure has to cost something or the odds are decoration — the seed is gone,
/// and the bed is free again — but it should not cost *everything*, or a gardener
/// who meets a rare seed too early learns to plant nothing at all. The fifth is
/// the lesson: you learn from what killed the seedling.
pub fn wither_points(tier: u8, grade: SeedGrade) -> i32 {
    (harvest_points(tier, grade) + 4) / 5
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

    /// ADR-0098's two anchors, and they pin the whole curve between them: the
    /// garden meeting a seed of its own depth — ungraded — stands at eighty
    /// percent, and even a garden with nothing left to learn still meets a 神阶
    /// seed of the tenth tier at ninety. Everything else in the table is what
    /// those two leave room for.
    #[test]
    fn the_planting_odds_anchor_on_parity_and_on_the_full_garden() {
        for tier in 1..=MAX_SEED_TIER {
            assert_eq!(
                plant_permille(tier, tier, SeedGrade::Yellow),
                800,
                "tier {tier}: parity is eighty percent"
            );
        }
        assert_eq!(
            plant_permille(MAX_GARDEN_RANK, MAX_SEED_TIER, SeedGrade::Divine),
            900,
            "the brief's own calibration"
        );
        // Every grade asks for another half-rank of garden: 0, 0.6, 1.3, 1.9
        // and 2.5 ranks, read off the same curve the tier difference is.
        let at_parity: Vec<u32> = SeedGrade::ALL
            .into_iter()
            .map(|grade| plant_permille(5, 5, grade))
            .collect();
        assert_eq!(at_parity, vec![800, 726, 648, 588, 534], "{at_parity:?}");
        assert!(
            at_parity.windows(2).all(|pair| pair[1] < pair[0]),
            "a better herb is a harder herb"
        );
    }

    /// The lever is the garden's own rank, and it is worth four points up and
    /// fifteen percent down — asymmetric on purpose, because the cost of aiming
    /// too high has to sting more than the gain of waiting until the garden is
    /// ready.
    #[test]
    fn one_rank_is_four_percent_up_and_fifteen_percent_down() {
        let up: Vec<u32> = (0..=5)
            .map(|step| plant_permille(5 + step, 5, SeedGrade::Yellow))
            .collect();
        assert_eq!(up, vec![800, 840, 880, 920, 960, 1_000], "{up:?}");
        // Five ranks above the seed is a certainty and stays one: there is no
        // odds above certain, and the top of the garden is exactly five above
        // the deepest seed.
        assert_eq!(plant_permille(MAX_GARDEN_RANK, 5, SeedGrade::Yellow), 1_000);
        let down: Vec<u32> = (1..=9)
            .map(|step| plant_permille(10 - step, 10, SeedGrade::Yellow))
            .collect();
        assert_eq!(
            down,
            vec![681, 579, 493, 419, 357, 304, 259, 219, 188],
            "{down:?}"
        );
        for pair in down.windows(2) {
            assert!(
                pair[1] < pair[0],
                "every rank deeper has to cost something: {pair:?}"
            );
        }
        // There is a floor, and it is not zero: the worst planting in the game
        // is still a gamble rather than a refusal.
        assert_eq!(plant_permille(1, MAX_SEED_TIER, SeedGrade::Divine), 127);
        assert!(plant_permille(1, MAX_SEED_TIER, SeedGrade::Divine) >= PLANT_FLOOR_PERMILLE as u32);
    }

    /// The whole matrix: fifteen ranks against ten tiers, every cell from
    /// [`plant_permille`]. Written out rather than asserted in a loop because a
    /// loop would pass on any monotone function — this is the table ADR-0098
    /// §2.6 says the surface prints.
    #[test]
    fn the_fifteen_ranks_against_the_ten_tiers() {
        let expected: [[u32; MAX_SEED_TIER as usize]; MAX_GARDEN_RANK as usize] = [
            [800, 681, 579, 493, 419, 357, 304, 259, 219, 188],
            [840, 800, 681, 579, 493, 419, 357, 304, 259, 219],
            [880, 840, 800, 681, 579, 493, 419, 357, 304, 259],
            [920, 880, 840, 800, 681, 579, 493, 419, 357, 304],
            [960, 920, 880, 840, 800, 681, 579, 493, 419, 357],
            [1_000, 960, 920, 880, 840, 800, 681, 579, 493, 419],
            [1_000, 1_000, 960, 920, 880, 840, 800, 681, 579, 493],
            [1_000, 1_000, 1_000, 960, 920, 880, 840, 800, 681, 579],
            [1_000, 1_000, 1_000, 1_000, 960, 920, 880, 840, 800, 681],
            [1_000, 1_000, 1_000, 1_000, 1_000, 960, 920, 880, 840, 800],
            [1_000, 1_000, 1_000, 1_000, 1_000, 1_000, 960, 920, 880, 840],
            [
                1_000, 1_000, 1_000, 1_000, 1_000, 1_000, 1_000, 960, 920, 880,
            ],
            [
                1_000, 1_000, 1_000, 1_000, 1_000, 1_000, 1_000, 1_000, 960, 920,
            ],
            [
                1_000, 1_000, 1_000, 1_000, 1_000, 1_000, 1_000, 1_000, 1_000, 960,
            ],
            [
                1_000, 1_000, 1_000, 1_000, 1_000, 1_000, 1_000, 1_000, 1_000, 1_000,
            ],
        ];
        for (offset, row) in expected.iter().enumerate() {
            let rank = offset as u8 + 1;
            let read: Vec<u32> = (1..=MAX_SEED_TIER)
                .map(|tier| plant_permille(rank, tier, SeedGrade::Yellow))
                .collect();
            assert_eq!(&read[..], &row[..], "rank {rank}");
        }
    }

    /// What the ladder costs, and why: the whole fifteen ranks ask for fifteen
    /// thousand seven hundred and fifteen stones, and twenty-two percent of
    /// what the harvest it takes would be worth sold instead of kept. The second
    /// number is the one chosen; the first is arithmetic.
    #[test]
    fn the_rank_ladder_costs_a_fifth_of_the_harvest() {
        let mut xp = 0_i64;
        let mut stones = 0_u32;
        for rank in 1..MAX_GARDEN_RANK {
            xp += rank_xp(rank);
            stones += rank_cost(rank);
        }
        assert_eq!(xp, rank_xp_total(MAX_GARDEN_RANK));
        assert_eq!(xp, 5_075, "five thousand and seventy-five points");
        assert_eq!(stones, 15_715, "fifteen thousand seven hundred and fifteen");
        // Because XP *is* harvest points, and a herb sells for its points times
        // fourteen, the stones are really "what fraction of the harvest did you
        // sell".
        let percent = u64::from(stones) * 100 / (u64::try_from(xp).unwrap() * 14);
        assert_eq!(percent, 22, "twenty-two percent of the harvest, sold");
        // The thresholds rise, and the increments rise faster still: the last
        // rank is two hundred times the first.
        for rank in 2..MAX_GARDEN_RANK {
            assert!(rank_xp(rank) > rank_xp(rank - 1), "rank {rank}");
            assert!(rank_cost(rank) > rank_cost(rank - 1), "rank {rank}");
        }
        assert_eq!(rank_xp(1), 5, "the first step is one crop");
        assert_eq!(rank_cost(1), 50);
    }

    /// Reading a bank back into a rank is the migration every old save takes,
    /// and it has to agree with the ladder above however the two drift.
    #[test]
    fn a_bank_of_harvests_reads_back_as_the_rank_it_earned() {
        assert_eq!(rank_for_xp(0), 1);
        assert_eq!(rank_for_xp(4), 1, "one short of the second rank");
        assert_eq!(rank_for_xp(5), 2);
        assert_eq!(rank_for_xp(300), 6, "a hundred harvests of gardening");
        assert_eq!(rank_for_xp(5_075), MAX_GARDEN_RANK);
        assert_eq!(rank_for_xp(i64::MAX), MAX_GARDEN_RANK);
        for rank in 2..=MAX_GARDEN_RANK {
            let growing = rank_xp_total(rank);
            assert_eq!(rank_for_xp(growing), rank, "exactly at {growing}");
            assert_eq!(rank_for_xp(growing - 1), rank - 1);
        }
    }

    /// A seedling that dies still teaches something: a fifth of the crop it
    /// would have been, and never nothing.
    #[test]
    fn what_a_dead_seedling_teaches() {
        for tier in 1..=MAX_SEED_TIER {
            for grade in SeedGrade::ALL {
                let due = wither_points(tier, grade);
                assert!(due >= 1, "tier {tier} {grade:?} teaches nothing");
                assert!(
                    due <= harvest_points(tier, grade),
                    "a failure pays more than the crop it stood in for"
                );
            }
        }
        assert_eq!(wither_points(MAX_SEED_TIER, SeedGrade::Divine), 48);
        assert_eq!(wither_points(1, SeedGrade::Yellow), 1);
    }
}
