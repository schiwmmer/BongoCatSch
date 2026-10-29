//! 功法: the five hundred techniques a cultivator can learn.
//!
//! A 功法 is one of two kinds. An 内功 is a body of training: every 阶 it gains
//! pays attributes, and it is never swung at anyone. An 外功 is a technique: it
//! is what a fight actually spends 内力 on, and its 阶 decides how hard it lands
//! and how dear it is to use.
//!
//! The five hundred are not stored. An id is enough to rebuild one exactly, so a
//! save file carries only what the cultivator owns — which id, which 阶, how many
//! pomodoros banked — and a 功法 the user has never met costs nothing but the
//! arithmetic that names it.
//!
//! Learning is tied to the pomodoro rather than to a button: a session already
//! finished is the only honest measure of "time spent training", and a 潜质-driven
//! failure chance is what keeps the ladder from being a queue you walk to the end
//! of. A failed attempt costs the session and nothing else — progress a user has
//! already banked is never taken back.

use serde::{Deserialize, Serialize};

use super::{Attribute, Attributes, Derived, Rng};

/// How many 功法 the ladder holds: five hundred, graded as below.
pub const ART_COUNT: u64 = 500;

/// The 潜质 a book of each grade pays per 阶, in tenths: 0.2 / 0.5 / 0.8 /
/// 1.2 / 2.0.
///
/// 潜质 is the one number a build can be built around, so a book that raises it
/// has to be worth the rank it costs: a 黄阶 one is a slow trickle, a 神阶 one
/// pays a whole point a rank. The span is the brief's own — two tenths to two
/// points — and the grade is the whole difference between them.
pub const APTITUDE_ART_TENTHS_PER_RANK: [i32; 5] = [2, 5, 8, 12, 20];

/// The ten 功法 whose study raises 潜质 — two per grade, and no others.
///
/// 潜质 is the ladder's scarcest number, so only the books written for it move
/// it: ten out of five hundred, which is what makes finding one an event rather
/// than a rounding difference. Every id here is an 内功 (an odd id, so the book
/// is trained rather than swung) and every one sits inside its grade's band.
pub const APTITUDE_ART_IDS: [u64; 10] = [17, 93, 233, 307, 371, 425, 447, 479, 491, 499];

/// What one 阶 of this 功法 pays in 潜质, in tenths — zero for the four hundred
/// and ninety that are not written for it.
pub fn aptitude_tenths_per_rank(art_id: u64) -> i32 {
    if !APTITUDE_ART_IDS.contains(&art_id) {
        return 0;
    }
    APTITUDE_ART_TENTHS_PER_RANK[grade_for_id(art_id).index()]
}

/// The weight an unowned 功法 carries when one is picked out of its grade's band.
///
/// An aptitude book is worth half of any other: the brief's "掉落概率比其他功法低
/// 一倍". Two numbers rather than a percentage so the pick stays integer
/// arithmetic, and so the ratio is exactly a half rather than a rounding of it.
pub(crate) fn art_pick_weight(art_id: u64) -> u64 {
    if aptitude_tenths_per_rank(art_id) > 0 {
        1
    } else {
        2
    }
}

/// The five grades, weakest first: 黄阶、玄阶、地阶、天阶、神阶.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ArtGrade {
    Yellow,
    Profound,
    Earth,
    Heaven,
    Divine,
}

impl ArtGrade {
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

    /// The catalog key that names this grade.
    pub const fn name_key(self) -> &'static str {
        match self {
            Self::Yellow => "cultivation.art.grade.yellow",
            Self::Profound => "cultivation.art.grade.profound",
            Self::Earth => "cultivation.art.grade.earth",
            Self::Heaven => "cultivation.art.grade.heaven",
            Self::Divine => "cultivation.art.grade.divine",
        }
    }

    /// How many 阶 a 功法 of this grade has: a 黄阶 art is three and done, a 神阶
    /// one runs to ten. The grade is what makes the deep ones a project.
    pub const fn ranks(self) -> u8 {
        match self {
            Self::Yellow => 3,
            Self::Profound => 5,
            Self::Earth => 6,
            Self::Heaven => 8,
            Self::Divine => 10,
        }
    }

    /// The grade's colour, as `0xRRGGBB`: 黄、蓝、紫、橙、红.
    ///
    /// One hue per grade and none repeated, because the colour is the only
    /// thing that tells two rows apart before the name is read: a shelf of five
    /// 功法 whose grades all look the same is a shelf the user has to read word
    /// by word. The ramp runs the way the grade does — the warmest, reddest
    /// grade is the rarest one.
    pub const fn color(self) -> u32 {
        match self {
            Self::Yellow => 0xE0_B0_20,
            Self::Profound => 0x3B_82_F6,
            Self::Earth => 0xA8_55_F7,
            Self::Heaven => 0xF9_7316,
            Self::Divine => 0xEF_44_44,
        }
    }
}

/// Which of the two kinds a 功法 is.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ArtKind {
    /// 内功: trained, never swung. Every 阶 pays attributes.
    Internal,
    /// 外功: used in a fight, at a cost in 内力.
    External,
}

impl ArtKind {
    /// The catalog key that names this kind.
    pub const fn name_key(self) -> &'static str {
        match self {
            Self::Internal => "cultivation.art.kind.internal",
            Self::External => "cultivation.art.kind.external",
        }
    }
}

/// One of the five hundred, rebuilt from its id.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtBlueprint {
    pub id: u64,
    pub name: String,
    pub kind: ArtKind,
    pub grade: ArtGrade,
    /// What one 阶 of an 内功 pays. Zero for an 外功, which pays damage instead.
    pub gain: Attributes,
    /// What one 阶 of an 内功 pays outright in combat numbers — 攻击力, 生命值,
    /// 命中 and the rest, the figures a fight reads directly. Zero for an 外功
    /// and for the 内功 whose shape is pure attributes.
    pub bonus: Derived,
    /// What one 外功 costs in 内力. Zero for an 内功.
    pub cost: u32,
    /// What one 外功 is worth, in parts per thousand of the wielder's 攻击力.
    ///
    /// The printed figure, and the centre of the band the blow is drawn from.
    pub multiplier_permille: u32,
    /// How many blows one use throws, for an 外功.
    pub hits: u8,
    /// The band one blow is drawn from, in parts per thousand of the printed
    /// worth: `lo` and `hi` both.
    pub band_permille: (u32, u32),
    /// What a landing blow does besides the hurt, for an 外功.
    pub effect: Option<StrikeEffect>,
    /// What one 阶 pays in 潜质, in tenths. Zero for every book but the ten
    /// written for it — see [`APTITUDE_ART_IDS`].
    pub aptitude_tenths: i32,
}

/// The grade a given id carries.
///
/// The ladder is bottom-heavy on purpose: two hundred 黄阶 arts to thirty 神阶
/// ones, so a beginner meets 功法 constantly and a 神阶 art stays an event.
fn grade_for_id(id: u64) -> ArtGrade {
    match id {
        1..=200 => ArtGrade::Yellow,
        201..=350 => ArtGrade::Profound,
        351..=440 => ArtGrade::Earth,
        441..=485 => ArtGrade::Heaven,
        _ => ArtGrade::Divine,
    }
}

const ART_PREFIXES: [&str; 40] = [
    "太乙", "九霄", "玄天", "混元", "太极", "两仪", "四象", "五行", "六合", "七星", "八卦", "九宫",
    "天罡", "地煞", "紫霞", "青云", "寒冰", "烈焰", "雷霆", "金刚", "般若", "无相", "逍遥", "凌波",
    "踏雪", "龙象", "虎啸", "鹤鸣", "龟息", "蛇行", "摘星", "揽月", "焚天", "镇海", "御风", "断岳",
    "流云", "磐石", "幽兰", "赤霄",
];

const INTERNAL_NOUNS: [&str; 12] = [
    "心法",
    "真诀",
    "内功",
    "吐纳",
    "归元功",
    "凝神功",
    "养气诀",
    "玄功",
    "道典",
    "胎息法",
    "洗髓经",
    "抱朴诀",
];

const EXTERNAL_NOUNS: [&str; 12] = [
    "掌法", "拳法", "剑法", "刀法", "指法", "腿法", "鞭法", "枪法", "棍法", "爪法", "戟法", "袖功",
];

/// Rebuild the 功法 an id names.
///
/// The name, the grade, the kind and the numbers all come from the same seeded
/// roll, so one id is one 功法 forever: a save can name it by id alone and a
/// future build reading that save sees the same art.
pub fn blueprint(id: u64) -> ArtBlueprint {
    let id = id.clamp(1, ART_COUNT);
    let grade = grade_for_id(id);
    // The name is counted out in a fixed radix rather than rolled: five hundred
    // draws from a pool of a few thousand would collide by birthday alone, and a
    // 功法 has to be tellable apart from another by its name.
    let index = (id - 1) as usize;
    let (kind, rest) = (
        if index.is_multiple_of(2) {
            ArtKind::Internal
        } else {
            ArtKind::External
        },
        index / 2,
    );
    // Forty prefixes across five hundred arts: the prefix *is* the 流派 (see
    // [`PREFIX_ARCHETYPE`]), so it has to be the fastest-moving part of the name
    // or the shelf shows one family over and over. It used to be the slowest —
    // the radix only ever reached the first three prefixes, and every 功法 in the
    // game was a 太乙, a 九霄 or an 玄天. The noun carries the rest of the
    // uniqueness: forty prefixes against twelve nouns is four hundred and eighty
    // names for two hundred and fifty slots a kind, so no two arts share one.
    let prefix_index = rest % ART_PREFIXES.len();
    let noun_index = (rest / ART_PREFIXES.len()) % INTERNAL_NOUNS.len();
    let noun = match kind {
        ArtKind::Internal => INTERNAL_NOUNS[noun_index],
        ArtKind::External => EXTERNAL_NOUNS[noun_index],
    };
    let prefix = ART_PREFIXES[prefix_index];
    let name = format!("{prefix}{noun}");
    let mut rng = Rng::seeded(id.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x0005_DEEC_E66D);
    let (gain, bonus, cost, multiplier_permille, hits, band_permille, effect) = match kind {
        ArtKind::Internal => {
            let (gain, bonus) = internal_gain(prefix_index, grade, &mut rng);
            (gain, bonus, 0, 0, 1, (0, 0), None)
        }
        ArtKind::External => {
            let strike = external_strike(grade, &mut rng);
            (
                Attributes::default(),
                Derived::default(),
                strike.cost,
                strike.multiplier_permille,
                strike.hits,
                strike.band_permille,
                strike.effect,
            )
        }
    };
    ArtBlueprint {
        id,
        name,
        kind,
        grade,
        gain,
        bonus,
        cost,
        multiplier_permille,
        hits,
        band_permille,
        effect,
        aptitude_tenths: aptitude_tenths_per_rank(id),
    }
}

/// What one 阶 of an 内功 pays: a handful of points, in one attribute or two,
/// in one combat number or two, or in both at once.
///
/// Three shapes rather than one, so the shelf reads as five hundred different
/// techniques instead of one bonus with five hundred names: a pure-attribute
/// art ("the 寒冰 one raises 法力"), a pure-combat art that pays 攻击力 or 命中
/// outright, and a hybrid that pays a little of each. The combat lines are
/// sized at the rate the attribute behind them would have paid — a point of
/// 攻击力 stands in for a third of a point of 力量 — so the three shapes are
/// worth the same at the same grade and the choice between them is flavour,
/// not a trap.
/// The per-阶 payout by grade: 3, 8, 12, 18, 27.
///
/// Raised from 2, 6, 9, 14, 21 — a third again on every rung — because the two
/// halves of the ladder have to be read against each other: an hour in the
/// ground pays what an hour in the ground pays, and a session at the desk has
/// to be worth more than a herb somebody planted. 玄 stays the one and a third
/// of the old four the brief anchored it on, and every step after it is at
/// least half again the one below, so a 神阶 内功 is not a 黄阶 one with a
/// longer climb.
const INTERNAL_MAGNITUDES: [i32; 5] = [3, 8, 12, 18, 27];

fn internal_gain(prefix_index: usize, grade: ArtGrade, rng: &mut Rng) -> (Attributes, Derived) {
    // One budget per grade, whatever the art: the average attribute weight is
    // what the old "magnitude points of one attribute" was worth on average, so
    // the ladder's mean power does not move while every single art does. A 力量
    // technique and a 身法 technique now pay different point totals for the same
    // power — 22 against 39 at 神阶 — which is the difference the shelf needed:
    // five hundred names over one number was five hundred copies of one art.
    let budget = internal_budget(grade);
    // 成色: two books of one 流派 are still two books. Six percent either way is
    // enough that no two rows print the same number, and small enough that a
    // 流派 is a family rather than a lie about being identical.
    let budget = budget * (940 + rng.below(121) as i64) / 1_000;
    // The shape decides how much of the budget is spent on attributes and how
    // much on outright combat numbers. The split is a split of one budget — a
    // hybrid used to pay a full attribute line *and* a full combat line, which
    // made one art in four worth twice its neighbours.
    let shape = rng.below(100);
    let attribute_share = if shape < 40 {
        1_000
    } else if shape < 75 {
        0
    } else {
        500
    };
    let attribute_budget = budget * attribute_share / 1_000;
    let combat_budget = budget - attribute_budget;
    let archetype = archetype_of(prefix_index);
    // The 流派's own split carries a fifth of jitter, so two 刚柔 arts are not
    // the same split to the point.
    let primary_share = archetype.sway(rng);
    let mut gain = Attributes::default();
    let mut bonus = Derived::default();
    for (index, (attribute, nominal)) in archetype.attributes.iter().enumerate() {
        let share = if index == 0 {
            nominal * primary_share / 1_000
        } else {
            nominal * (2_000 - primary_share) / 1_000
        };
        let part = attribute_budget * i64::from(share) / 1_000;
        // A family always pays its own attributes. At 黄阶 one point of 身法 is
        // already the rounding of a whole share, so a book that swayed hard would
        // otherwise read as a 养 book with no footwork at all.
        let points = if *nominal > 0 && attribute_budget > 0 {
            points_for(part, weight_of(*attribute)).max(1)
        } else {
            points_for(part, weight_of(*attribute))
        };
        if points > 0 {
            attribute.add_to(&mut gain, points);
        }
    }
    for (line, share) in archetype.lines {
        let part = combat_budget * i64::from(share) / 1_000;
        if part > 0 {
            line.add_to(&mut bonus, points_for(part, weight_of(line.attribute())));
        }
    }
    (gain, bonus)
}

/// The power one 阶 of an 内功 may spend, in weighted tenths.
///
/// [`INTERNAL_MAGNITUDES`] is what the art pays in points when it spends them on
/// the average attribute — the old flat payout — so the budget is that number
/// times the mean of the four weights. Every shape and every 流派 spends exactly
/// this, which is what makes the choice between them flavour rather than a trap.
fn internal_budget(grade: ArtGrade) -> i64 {
    // (19 + 26 + 26 + 15) / 4 = 21.5, kept as tenths so the arithmetic stays
    // whole.
    i64::from(INTERNAL_MAGNITUDES[grade.index()]) * 215 / 10
}

/// How many points of one attribute a slice of the budget buys.
fn points_for(budget: i64, weight: i64) -> i32 {
    i32::try_from((budget + weight / 2) / weight).unwrap_or(i32::MAX)
}

/// What one point of an attribute is worth in the power table, in tenths.
fn weight_of(attribute: Attribute) -> i64 {
    match attribute {
        Attribute::Root => super::ROOT_WEIGHT,
        Attribute::Strength => super::STRENGTH_WEIGHT,
        Attribute::Mana => super::MANA_WEIGHT,
        Attribute::Agility => super::AGILITY_WEIGHT,
    }
}

/// One 流派: which attributes a family of 功法 pays, and which combat numbers it
/// pays when it pays those instead.
///
/// The prefix on the name *is* the 流派 — see [`PREFIX_ARCHETYPE`] — so two 九霄
/// arts share a profile by construction and a player can read the family off the
/// name. That is the only kinship the naming promises: a set bonus is a separate
/// feature and the names must not imply one that does not exist.
struct Archetype {
    /// Two attribute shares, out of a thousand. The second may be zero.
    attributes: [(Attribute, u32); 2],
    /// Combat-line shares, out of a thousand. A zero share is a line this
    /// family does not pay.
    lines: [(CombatLine, u32); 3],
    /// How far the split may sway from its nominal figure, out of a thousand:
    /// 1_000 means it does not sway at all.
    sway_permille: u32,
}

impl Archetype {
    /// The primary share this one book uses, out of two thousand (so a
    /// thousand is the nominal split and the second attribute takes the rest).
    fn sway(&self, rng: &mut Rng) -> u32 {
        let slack = 1_000 - self.sway_permille.min(1_000);
        if slack == 0 {
            return 1_000;
        }
        // The double is the arithmetic of "share and its remainder": the second
        // attribute is given 2000 minus this, so a thousand is an even split of
        // the sway budget rather than of the pair.
        1_000 + rng.below(u64::from(slack) + 1) as u32
    }
}

/// The eight families, and the five prefixes that name each one.
///
/// Aligned with [`ART_PREFIXES`]: an art's family is looked up by the position
/// of its prefix, so the name and the numbers can never disagree.
const ARCHETYPES: [Archetype; 8] = [
    // 刚: force, and the wounds force leaves.
    Archetype {
        attributes: [(Attribute::Strength, 1_000), (Attribute::Strength, 0)],
        lines: [
            (CombatLine::Attack, 700),
            (CombatLine::Health, 300),
            (CombatLine::Attack, 0),
        ],
        sway_permille: 0,
    },
    // 术: spellwork and the resistance to it.
    Archetype {
        attributes: [(Attribute::Mana, 1_000), (Attribute::Mana, 0)],
        lines: [
            (CombatLine::Spell, 700),
            (CombatLine::Resist, 300),
            (CombatLine::Spell, 0),
        ],
        sway_permille: 0,
    },
    // 养: the root, and the breath it stores.
    Archetype {
        attributes: [(Attribute::Root, 1_000), (Attribute::Root, 0)],
        lines: [
            (CombatLine::Inner, 700),
            (CombatLine::Health, 300),
            (CombatLine::Inner, 0),
        ],
        sway_permille: 0,
    },
    // 轻: footwork.
    Archetype {
        attributes: [(Attribute::Agility, 1_000), (Attribute::Agility, 0)],
        lines: [
            (CombatLine::Speed, 500),
            (CombatLine::Hit, 250),
            (CombatLine::Evade, 250),
        ],
        sway_permille: 0,
    },
    // 刚柔: force with footwork behind it.
    Archetype {
        attributes: [(Attribute::Strength, 650), (Attribute::Agility, 350)],
        lines: [
            (CombatLine::Attack, 600),
            (CombatLine::Speed, 400),
            (CombatLine::Attack, 0),
        ],
        sway_permille: 200,
    },
    // 玄门: spellwork carried by the root.
    Archetype {
        attributes: [(Attribute::Mana, 600), (Attribute::Root, 400)],
        lines: [
            (CombatLine::Spell, 700),
            (CombatLine::Inner, 300),
            (CombatLine::Spell, 0),
        ],
        sway_permille: 200,
    },
    // 守拙: poise, and the footwork that keeps it standing.
    Archetype {
        attributes: [(Attribute::Root, 600), (Attribute::Agility, 400)],
        lines: [
            (CombatLine::Health, 500),
            (CombatLine::Evade, 500),
            (CombatLine::Health, 0),
        ],
        sway_permille: 200,
    },
    // 双修: a sword in one hand and a spell in the other.
    Archetype {
        attributes: [(Attribute::Strength, 500), (Attribute::Mana, 500)],
        lines: [
            (CombatLine::Attack, 500),
            (CombatLine::Spell, 500),
            (CombatLine::Attack, 0),
        ],
        sway_permille: 200,
    },
];

/// Which family each of [`ART_PREFIXES`] names, in the same order.
///
/// 金刚 龙象 虎啸 断岳 镇海 are 刚; 烈焰 焚天 雷霆 紫霞 九霄 are 术; 太乙 混元
/// 太极 龟息 般若 are 养; 逍遥 凌波 踏雪 御风 蛇行 are 轻; 天罡 地煞 四象 六合
/// 鹤鸣 are 刚柔; 玄天 无相 流云 幽兰 揽月 are 玄门; 两仪 五行 八卦 九宫 七星
/// are 守拙; 青云 寒冰 摘星 磐石 赤霄 are 双修.
const PREFIX_ARCHETYPE: [usize; 40] = [
    2, 1, 5, 2, 2, 6, 4, 6, 4, 6, 6, 6, 4, 4, 1, 7, 7, 1, 1, 0, 2, 5, 3, 3, 3, 0, 0, 4, 2, 3, 7, 5,
    1, 0, 3, 0, 5, 7, 5, 7,
];

/// The family one prefix names.
fn archetype_of(prefix_index: usize) -> &'static Archetype {
    &ARCHETYPES[PREFIX_ARCHETYPE[prefix_index.min(PREFIX_ARCHETYPE.len() - 1)]]
}

/// What one use of an 外功 is worth, and how it hands the worth over.
///
/// A move used to be one number — the multiplier — so every 玄阶 外功 was the same
/// swing under another name. Two other axes give the five hundred something to
/// differ on: the shape (how many blows one use is thrown in) and the effect (what
/// a blow that lands does to the rest of the fight.
pub(crate) struct ExternalStrike {
    /// What one use costs in 内力.
    pub cost: u32,
    /// The printed worth, in parts per thousand of the wielder's 攻击力.
    pub multiplier_permille: u32,
    /// How many blows one use throws.
    pub hits: u8,
    /// The band each blow is drawn from, in parts per thousand of the worth.
    pub band_permille: (u32, u32),
    /// What a landing blow does besides the hurt, for an 外功.
    pub effect: Option<StrikeEffect>,
}

/// Roll the move one 外功 carries, from its id.
///
/// The shape and the effect come off the same seeded stream that prices the art,
/// so one id stays one 外功 forever — and the shelf holds real variety: two 神阶
/// 外功 differ in how many blows they throw, in the band each is drawn from,
/// and in what a landing blow lays on.
fn external_strike(grade: ArtGrade, rng: &mut Rng) -> ExternalStrike {
    let shape = StrikeShape::from_index(rng.below(StrikeShape::ALL.len() as u64) as usize);
    // The 成色 of this copy of the book: a few percent either side of what the
    // grade is worth on average, so two books of one grade are not one number
    // printed twice.
    let worth = external_multiplier(grade) * (940 + rng.below(121) as u32) / 1_000;
    ExternalStrike {
        cost: external_cost(grade),
        multiplier_permille: worth.max(1),
        hits: shape.hits(),
        band_permille: shape.band(),
        effect: strike_effect(grade, rng),
    }
}

/// The effect one 外功 carries, or none at all.
///
/// One in four carries nothing extra and pays for it in raw worth. The other
/// three lay something on the fight that the blows alone would not: chill the
/// target's footwork, quicken your own, or steady the aim. The strength grows
/// with the grade, so a 神阶 外功 bites even where its name does not show it.
fn strike_effect(grade: ArtGrade, rng: &mut Rng) -> Option<StrikeEffect> {
    let depth = 120 + u32::try_from(grade.index()).unwrap_or(0) * 20;
    let roll = |rng: &mut Rng, span: u32| rng.below(u64::from(span) + 1) as u32;
    match rng.below(4) {
        0 => None,
        1 => Some(StrikeEffect::Slow {
            permille: depth + roll(rng, 40),
        }),
        2 => Some(StrikeEffect::Haste {
            permille: depth + roll(rng, 20),
        }),
        _ => Some(StrikeEffect::Focus {
            hit: 40 + u32::try_from(grade.index()).unwrap_or(0) * 20 + roll(rng, 40),
        }),
    }
}

/// Which of the eight combat numbers one 阶 of an 内功 can pay outright.
///
/// 潜质 is deliberately not among them: it is the number a whole build turns on,
/// and the brief rations it to the ten books written for it — a hybrid art that
/// rolled a point of it would be a second, quiet way into the same ladder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CombatLine {
    Attack,
    Health,
    Inner,
    Spell,
    Resist,
    Speed,
    Hit,
    Evade,
}

impl CombatLine {
    /// The attribute this line stands in for, so its share of the budget can be
    /// turned into points at the right rate.
    const fn attribute(self) -> Attribute {
        match self {
            Self::Attack | Self::Health => Attribute::Strength,
            Self::Spell | Self::Resist => Attribute::Mana,
            Self::Inner => Attribute::Root,
            Self::Speed | Self::Hit | Self::Evade => Attribute::Agility,
        }
    }

    /// Add this line's worth to a set of combat numbers.
    ///
    /// The rate is the one the attribute behind the line pays: one 阶's
    /// `magnitude` points of 力量 are three points of 攻击力 and twenty of
    /// 生命值, and a combat-paying art pays exactly those outputs. So a
    /// combat art and an attribute art of the same grade stand on the same
    /// scale, and neither shape is the trap.
    fn add_to(self, bonus: &mut Derived, magnitude: i32) {
        let magnitude = magnitude.max(0);
        let rate = match self {
            Self::Attack => 3,
            Self::Health => 20,
            Self::Inner => 8,
            Self::Spell => 3,
            Self::Resist => 2,
            Self::Speed => 2,
            Self::Hit => 10,
            Self::Evade => 10,
        };
        self.add_points(bonus, u32::try_from(magnitude * rate).unwrap_or(u32::MAX));
    }

    /// Add to this line's own number directly, without the conversion the
    /// budget reads at. The set bonus is an output — a family that is attack
    /// pays attack, not the 力量 behind it — so it adds to the number itself.
    fn add_points(self, bonus: &mut Derived, points: u32) {
        let slot = match self {
            Self::Attack => &mut bonus.attack,
            Self::Health => &mut bonus.health,
            Self::Inner => &mut bonus.inner,
            Self::Spell => &mut bonus.spell,
            Self::Resist => &mut bonus.resist,
            Self::Speed => &mut bonus.speed,
            Self::Hit => &mut bonus.hit,
            Self::Evade => &mut bonus.evade,
        };
        *slot = slot.saturating_add(points);
    }
}

/// What one use of an 外功 costs in 内力: the higher the grade, the dearer.
///
/// Costly enough that a swing is a decision the 内力 pool feels — an art fired
/// on every round was a multiplier running at no price.
fn external_cost(grade: ArtGrade) -> u32 {
    16 + grade.index() as u32 * 12
}

/// What an 外功 is worth, in parts per thousand of the 攻击力 behind it.
///
/// The grade is the whole difference: 1.2× for 黄, 2.0× for 玄 — the brief's
/// "玄 is one and a half times what it was" applied to the old 1_350‰ — then a
/// third more per grade after that. A 神阶 strike at 4.8× is a different act
/// from a 黄阶 one at 1.2×, which is what makes finding one worth the ten-
/// thousandth chance it takes.
///
/// This is the figure a row prints, and the *centre* of the band a blow rolls
/// in — see [`StrikeShape::band`]. A 神阶 外功 is not worth a fixed number of blows
/// per use; it is worth a draw from a band, which is what makes two of them
/// different objects rather than two copies of one number.
fn external_multiplier(grade: ArtGrade) -> u32 {
    const MULTIPLIERS: [u32; 5] = [1_200, 2_025, 2_700, 3_600, 4_800];
    MULTIPLIERS[grade.index()]
}

/// How a use of an 外功 throws its damage: one blow, or a flurry of them.
///
/// A move is not only a number — two 玄阶 外功 that both land for two and a half
/// blows are the same fight twice, and the shelf would read as five hundred
/// copies of one technique. The shape is the second axis: how many blows a use
/// spends its worth across, and how wide the band around each one is.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum StrikeShape {
    /// One clean blow, at its printed worth.
    Single,
    /// Three quick blows, each about a third — the same worth spread thin,
    /// and the band around each one is narrower than the whole's.
    Flurry,
    /// One blow drawn from a band twice as wide as a single's: the thrower
    /// is gambling the throw rather than the swing.
    Charge,
}

impl StrikeShape {
    /// Every shape.
    pub const ALL: [Self; 3] = [Self::Single, Self::Flurry, Self::Charge];

    /// The shape at a position in [`Self::ALL`].
    pub fn from_index(index: usize) -> Self {
        Self::ALL[index.min(Self::ALL.len() - 1)]
    }

    /// How many blows one use throws.
    pub const fn hits(self) -> u8 {
        match self {
            Self::Single | Self::Charge => 1,
            Self::Flurry => 3,
        }
    }

    /// The band one blow is drawn from, in parts per thousand of the printed
    /// worth: `lo` and `hi` both, and `hi >= lo`.
    ///
    /// The band is what makes the same 外功 worth a different number each time
    /// it is thrown. A 连击 spreads its worth over three draws at a third
    /// apiece — three narrow draws add up closer to their mean than one wide
    /// one does, so a flurry pays *steadier* damage for a slightly smaller
    /// expected blow. A 蓄力 does the opposite: one draw from a band twice as
    /// wide, the same expectation and twice the gamble.
    const fn band(self) -> (u32, u32) {
        match self {
            Self::Single => (940, 1_060),
            Self::Flurry => (280, 340),
            Self::Charge => (550, 1_700),
        }
    }
}

/// What one landing blow does besides the hurt.
///
/// A 外功 that only multiplies is a number with a name; these are the names
/// that change how the rest of the fight reads. Each is written in the log the
/// first time it lands and holds from then on, so a fight against a 寒劲 art is a
/// fight against a slow hand rather than against a second, identical swing.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum StrikeEffect {
    /// 寒劲: the target's 速度 falls.
    Slow {
        /// The share of its 速度 it loses, in parts per thousand.
        permille: u32,
    },
    /// 轻身: the thrower's 速度 rises.
    Haste {
        /// The share of its 速度 it gains, in parts per thousand.
        permille: u32,
    },
    /// 凝神: the thrower's 命中 rises — a plain number, read against the
    /// other side's 闪避.
    Focus {
        /// The 命中 it adds.
        hit: u32,
    },
}

impl StrikeEffect {}

/// How many pomodoros the given 阶 prints: 1, 2, 3, 4, 5, 7, 10, 14, 20, 29.
///
/// Geometric, because the alternative is either a 10-阶 art that finishes in an
/// afternoon or a flat cost that makes the tenth 阶 no more of an achievement
/// than the first. The ratio is 1.45: steep enough to be felt, shallow enough
/// that a 神阶 art is a season rather than a decade.
///
/// This is the **printed** cost, the one a body with no 潜质 pays three times
/// over — see [`super::aptitude::required_pomodoros`] for what a session
/// actually has to buy.
pub fn printed_pomodoros(rank: u8) -> u32 {
    let exponent = f64::from(rank.saturating_sub(1));
    (1.45_f64.powf(exponent)).ceil() as u32
}

/// How many pomodoros the given 阶 asks of *this* body, after 潜质.
pub fn required_pomodoros(rank: u8, aptitude_tenths: i32) -> u32 {
    super::aptitude::required_pomodoros(printed_pomodoros(rank), aptitude_tenths)
}

/// What reaching the last 阶 of a 功法 pays on top, in parts per thousand, by
/// grade: 黄阶 fifteen percent, 神阶 forty.
///
/// A 内功 pays its attributes multiplied by this — the art becomes more of
/// itself rather than a second art. An 外功 adds this share of 攻击力 to every
/// blow it lands, on top of the multiplier its 阶 already bought. The grade is
/// what sizes the bonus, so the deep arts are worth the season they take.
pub const MASTERY_PERMILLE: [u32; 5] = [150, 200, 250, 300, 400];

/// The bonus a completed 功法 of this grade pays.
pub const fn mastery_permille(grade: ArtGrade) -> u32 {
    MASTERY_PERMILLE[grade.index()]
}

/// One 阶's attribute payout with a mastery multiplier folded in.
///
/// A 内功 that has not reached its last 阶 pays exactly what its blueprint
/// says; a completed one pays [`MASTERY_PERMILLE`] more per 阶. The product is
/// rounded rather than truncated: a 黄阶 art pays six or seven points, and a
/// fifteen-percent bonus that floors to zero would read as a lie on a row that
/// promises one.
fn paid(value: i32, times: i64, mastery: i64) -> i32 {
    let product = i64::from(value) * times * mastery;
    let rounded = (product + 500) / 1_000;
    i32::try_from(rounded).unwrap_or(i32::MAX)
}

/// How often each grade is met on the road, in parts per million.
///
/// The ladder of *discovery* is steeper than the ladder of power on purpose:
/// 黄六成、玄两成五、地一成、天四成不足、神万分之一. A 神阶 art is not a lottery
/// with a good prize, it is an event a save may never see — which is what makes
/// seeing one worth a story. The five add up to exactly one million, so the
/// last grade's share is the brief's ten-thousandth and not a rounding of it.
pub const ART_GRADE_WEIGHTS_PER_MILLION: [u32; 5] = [600_000, 250_000, 110_000, 39_900, 100];

/// Which grade a found 功法 carries.
///
/// One roll for every channel that hands out an art — a breakthrough, a walk, a
/// tower floor — so the odds a user reads are the odds that ran. Nothing here
/// looks at the realm: a beginner can meet a 神阶 art, and that is the point of
/// the ten-thousandth.
pub(crate) fn roll_art_grade(rng: &mut Rng) -> ArtGrade {
    let roll = rng.below(1_000_000);
    let mut seen = 0u64;
    for (index, weight) in ART_GRADE_WEIGHTS_PER_MILLION.iter().enumerate() {
        seen += u64::from(*weight);
        if roll < seen {
            return ArtGrade::ALL[index];
        }
    }
    ArtGrade::Divine
}

/// A 功法 the cultivator owns.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OwnedArt {
    /// Which of the five hundred this is.
    pub art_id: u64,
    /// The 阶 reached. Zero is where every 功法 starts: owning the book is not
    /// having learned any of it, and the first 阶 is what training buys.
    pub rank: u8,
    /// Pomodoros banked toward the next 阶.
    pub progress: u32,
    /// How many attempts at this 功法 have failed.
    ///
    /// Counted for the whole book, not for the 阶 it was spent on: a bad run is
    /// the body learning *this* technique, so the help it earns carries past a
    /// 阶 rather than being handed back at the turn. It is never reset — the
    /// count stands until the art is finished — and it is per book, so one hard
    /// 功法 being read badly twenty times does not make the next one easier.
    #[serde(default)]
    pub failures: u32,
    /// How many copies of this book are on the shelf.
    ///
    /// 修炼 is the reader's, not the paper's: the 阶, the pomodoros banked toward
    /// the next one and the failures already spent belong to the one book being
    /// read, so a second copy does not open a second ladder. It is a spare —
    /// worth the stones a sale brings, and nothing else. Saves written before
    /// books could be found twice carry no count and hold one.
    #[serde(default = "one_copy")]
    pub copies: u32,
    /// What an attempt just came to.
    pub outcome: Option<ArtOutcome>,
}

/// The count an [`OwnedArt`] written before duplicates existed is read as.
const fn one_copy() -> u32 {
    1
}

/// How many 功法 the recent ring remembers, newest first.
///
/// The learned panel draws three, so eight is more than enough to survive the
/// three being sold, finished or folded away, and short enough to stay a ring.
pub const RECENT_ARTS_CAP: usize = 8;

/// What one attempt at learning came to.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ArtOutcome {
    /// The attempt failed. Nothing is lost but the session.
    Failed,
    /// The session counted. Progress moved, the 阶 did not.
    Progressed,
    /// The 阶 turned over.
    RankedUp,
    /// The last 阶 turned over: the art is complete.
    Mastered,
}

/// Put one finished pomodoro into the art being trained.
///
/// A failure costs the session and nothing else: progress already banked is the
/// user's, and a rule that took it back would turn a run of bad luck into a
/// punishment for having tried.
pub fn train(owned: &mut OwnedArt, aptitude_tenths: i32, rng: &mut Rng) -> ArtOutcome {
    let art = blueprint(owned.art_id);
    let max_rank = art.grade.ranks();
    if owned.rank >= max_rank {
        owned.outcome = Some(ArtOutcome::Mastered);
        return ArtOutcome::Mastered;
    }
    // 潜质 and the failures this book has already cost set the chance the
    // session lands: one roll against `base × 1.01^(潜质 + 失败)`. A failure
    // costs the session and nothing else.
    let success = super::aptitude::success_permille(art.grade, aptitude_tenths, owned.failures);
    if rng.below(1_000) >= u64::from(success) {
        // The failure is banked against the 功法: the next attempt is one percent
        // likelier, and every attempt after that likelier again — at this 阶 and
        // at each 阶 still to come.
        owned.failures = owned.failures.saturating_add(1);
        owned.outcome = Some(ArtOutcome::Failed);
        return ArtOutcome::Failed;
    }
    owned.progress = owned.progress.saturating_add(1);
    let needed = required_pomodoros(owned.rank, aptitude_tenths);
    if owned.progress < needed {
        owned.outcome = Some(ArtOutcome::Progressed);
        return ArtOutcome::Progressed;
    }
    owned.progress = 0;
    // The failures are not reset: a 阶 turning over is the same book still being
    // read, and the runs already spent on it are what make the next 阶 easier.
    owned.rank = owned.rank.saturating_add(1);
    let outcome = if owned.rank >= max_rank {
        ArtOutcome::Mastered
    } else {
        ArtOutcome::RankedUp
    };
    owned.outcome = Some(outcome);
    outcome
}

/// The attributes every owned 内功 has paid so far.
///
/// One 阶 pays the art's gain once, so a 神阶 内功 at its tenth 阶 has paid ten
/// times over — which is what makes finishing one worth the ninety-odd
/// pomodoros. A completed art pays a mastery bonus on every one of those 阶:
/// see [`MASTERY_PERMILLE`].
pub fn art_attributes(owned: &[OwnedArt]) -> Attributes {
    let mut total = Attributes::default();
    for art in owned {
        let blueprint = blueprint(art.art_id);
        if blueprint.kind != ArtKind::Internal {
            continue;
        }
        let times = i64::from(art.rank);
        let mastery = if art.rank >= blueprint.grade.ranks() {
            1_000 + i64::from(mastery_permille(blueprint.grade))
        } else {
            1_000
        };
        total.root = total
            .root
            .saturating_add(paid(blueprint.gain.root, times, mastery));
        total.strength =
            total
                .strength
                .saturating_add(paid(blueprint.gain.strength, times, mastery));
        total.mana = total
            .mana
            .saturating_add(paid(blueprint.gain.mana, times, mastery));
        total.agility = total
            .agility
            .saturating_add(paid(blueprint.gain.agility, times, mastery));
    }
    total
}

/// What every book on the shelf has paid in 潜质 so far, in tenths.
///
/// The counterpart of [`art_attributes`] for the one number that is not an
/// attribute: a 阶 of one of the ten books written for it pays its grade's
/// tenths, a completed one pays the mastery share on every 阶, and everything
/// else pays nothing at all.
pub fn art_aptitude_tenths(owned: &[OwnedArt]) -> i32 {
    let mut total: i32 = 0;
    for art in owned {
        let blueprint = blueprint(art.art_id);
        if blueprint.aptitude_tenths == 0 {
            continue;
        }
        let mastery = if art.rank >= blueprint.grade.ranks() {
            1_000 + i64::from(mastery_permille(blueprint.grade))
        } else {
            1_000
        };
        total = total.saturating_add(paid(
            blueprint.aptitude_tenths,
            i64::from(art.rank),
            mastery,
        ));
    }
    total
}

/// What the learned 功法 pay in 潜质, in tenths: every 阶 of the grade, with the
/// mastery share folded in.
pub fn mastered_art_aptitude_tenths(mastered: &[u64]) -> i32 {
    let mut total: i32 = 0;
    for id in mastered {
        let blueprint = blueprint(*id);
        if blueprint.aptitude_tenths == 0 {
            continue;
        }
        let mastery = 1_000 + i64::from(mastery_permille(blueprint.grade));
        total = total.saturating_add(paid(
            blueprint.aptitude_tenths,
            i64::from(blueprint.grade.ranks()),
            mastery,
        ));
    }
    total
}

/// What one book has paid in 潜质 at a given 阶, in tenths — the figure a row
/// prints beside the art it belongs to.
pub fn art_aptitude_tenths_at(art_id: u64, rank: u8) -> i32 {
    let blueprint = blueprint(art_id);
    if blueprint.aptitude_tenths == 0 {
        return 0;
    }
    let mastery = if rank >= blueprint.grade.ranks() {
        1_000 + i64::from(mastery_permille(blueprint.grade))
    } else {
        1_000
    };
    paid(blueprint.aptitude_tenths, i64::from(rank), mastery)
}

/// What a learned book pays in 潜质 in all, in tenths.
pub fn mastered_art_aptitude_payout(art_id: u64) -> i32 {
    let blueprint = blueprint(art_id);
    if blueprint.aptitude_tenths == 0 {
        return 0;
    }
    let mastery = 1_000 + i64::from(mastery_permille(blueprint.grade));
    paid(
        blueprint.aptitude_tenths,
        i64::from(blueprint.grade.ranks()),
        mastery,
    )
}

/// The attributes every **已习得** 内功 pays, for good.
///
/// A learned 功法 is complete: it sits at its grade's last 阶 and pays the
/// mastery bonus on every one of those 阶. The two lists are added together by
/// the caller, so a book still being trained and a 功法 already learned pay
/// their own share and neither counts the other's.
///
/// The mastery is what makes the two lists worth keeping apart: this is paid
/// for as long as the save lives, and selling the book it was learned from
/// does not take it away.
pub fn mastered_art_attributes(mastered: &[u64]) -> Attributes {
    let mut total = Attributes::default();
    for id in mastered {
        let blueprint = blueprint(*id);
        if blueprint.kind != ArtKind::Internal {
            continue;
        }
        let times = i64::from(blueprint.grade.ranks());
        let mastery = 1_000 + i64::from(mastery_permille(blueprint.grade));
        total.root = total
            .root
            .saturating_add(paid(blueprint.gain.root, times, mastery));
        total.strength =
            total
                .strength
                .saturating_add(paid(blueprint.gain.strength, times, mastery));
        total.mana = total
            .mana
            .saturating_add(paid(blueprint.gain.mana, times, mastery));
        total.agility = total
            .agility
            .saturating_add(paid(blueprint.gain.agility, times, mastery));
    }
    total
}

/// The attributes one learned 内功 actually pays, every 阶 with its mastery
/// share folded in.
///
/// This is the number the page prints beside the 功法's name — the sum the
/// attributes panel counts, not the per-阶 blueprint figure — so the card and
/// the arithmetic cannot disagree. An 外功 pays no attributes; the caller shows
/// its multiplier and cost instead.
pub fn mastered_art_payout(art_id: u64) -> Attributes {
    let blueprint = blueprint(art_id);
    let times = i64::from(blueprint.grade.ranks());
    let mastery = 1_000 + i64::from(mastery_permille(blueprint.grade));
    Attributes::new(
        paid(blueprint.gain.root, times, mastery),
        paid(blueprint.gain.strength, times, mastery),
        paid(blueprint.gain.mana, times, mastery),
        paid(blueprint.gain.agility, times, mastery),
    )
}

/// The combat numbers one learned 内功 actually pays, every 阶 with its mastery
/// share folded in.
///
/// The twin of [`mastered_art_payout`] for the outright lines: the page prints
/// it beside the 功法's name the same way, so the card and the arithmetic
/// cannot disagree.
pub fn mastered_art_bonus_payout(art_id: u64) -> Derived {
    let blueprint = blueprint(art_id);
    let times = i64::from(blueprint.grade.ranks());
    let mastery = 1_000 + i64::from(mastery_permille(blueprint.grade));
    scale_bonus(blueprint.bonus, times, mastery)
}

/// The combat numbers one 内功 pays as it stands, at the 阶 its training has
/// reached.
pub fn art_bonus_at(art_id: u64, rank: u8) -> Derived {
    let blueprint = blueprint(art_id);
    scale_bonus(blueprint.bonus, i64::from(rank), 1_000)
}

/// What one extra book of a 功法 family is worth, per point of the family's
/// best grade, in the family's primary combat number.
///
/// The step is deliberately a quiet one: the set is a bonus for collecting a
/// school, not a second copy of every book in it.
const ART_SET_STEP: u32 = 4;

/// The grade a 功法 family first opens at: 玄阶, the second of the five.
///
/// A 黄阶 book is the first one off the shelf and the first one outgrown, so
/// a family two 黄阶 books could open is a school every cultivator has in
/// their first hour — which is a shelf, not a school. The share the 黄阶 held
/// goes to the grades above it: from 玄阶 up every book counts, and the
/// deeper the shelf the more its families pay.
pub(crate) const SET_ART_GRADE: ArtGrade = ArtGrade::Profound;

/// Whether a book is deep enough to count towards its family.
///
/// Crate-visible so a test can use the same gate the bonus does, instead of
/// re-deriving it.
pub(crate) fn book_counts_towards_a_set(art_id: u64) -> bool {
    blueprint(art_id).grade >= SET_ART_GRADE
}

/// The set bonus the 功法 pay: for every 前缀 family with two or more books on
/// the shelf, the family's primary combat number is paid again at a share that
/// grows with how many books the family holds and with the best grade among
/// them.
///
/// The prefix *is* the set — see [`ART_PREFIXES`] — so two 九霄 books are one
/// family by construction and the set's name is the name its books share. The
/// bonus is a combat number because the family's identity is a combat line:
/// the 刚 family is attack, the 轻 family is footwork, and a family that paid
/// its set in 根骨 would be paying a number its books never promised.
///
/// Only 玄阶 and deeper books are counted: a family is a school the cultivator
/// kept, and a 黄阶 book is one they were always going to replace.
pub fn art_set_bonus(owned: &[OwnedArt]) -> Derived {
    let mut total = Derived::default();
    let mut families: std::collections::BTreeMap<usize, Vec<&OwnedArt>> =
        std::collections::BTreeMap::new();
    for art in owned {
        if !book_counts_towards_a_set(art.art_id) {
            continue;
        }
        families.entry(prefix_of(art.art_id)).or_default().push(art);
    }
    for (prefix_index, books) in families {
        if books.len() < 2 {
            continue;
        }
        let archetype = archetype_of(prefix_index);
        let primary = archetype.lines[0].0;
        let best = books
            .iter()
            .map(|book| blueprint(book.art_id).grade)
            .max()
            .unwrap_or(ArtGrade::Yellow);
        let magnitude = INTERNAL_MAGNITUDES[best.index()];
        let extra = u32::try_from(books.len() - 1).unwrap_or(u32::MAX);
        primary.add_points(&mut total, extra * magnitude as u32 * ART_SET_STEP);
    }
    total
}

/// One active 功法 family, as a surface reads it: the 前缀, how many books the
/// family holds, and what the holding pays.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtSetView {
    /// The 前缀 index — the set's name is [`ART_PREFIXES`]`[prefix]`.
    pub prefix: usize,
    /// How many books the family holds. Never one — a lone book is not a set.
    pub books: u8,
    pub bonus: Derived,
}

/// Every active 功法 family, one entry per 前缀 with two or more books.
///
/// The total of the `bonus` fields is exactly [`art_set_bonus`] — the same
/// grouping run the same way, so the surface can print "九霄 ×2 攻+…" per set
/// without recomputing the arithmetic itself.
pub fn art_set_views(owned: &[OwnedArt]) -> Vec<ArtSetView> {
    let mut families: std::collections::BTreeMap<usize, Vec<&OwnedArt>> =
        std::collections::BTreeMap::new();
    for art in owned {
        if !book_counts_towards_a_set(art.art_id) {
            continue;
        }
        families.entry(prefix_of(art.art_id)).or_default().push(art);
    }
    let mut views = Vec::new();
    for (prefix_index, books) in families {
        if books.len() < 2 {
            continue;
        }
        let archetype = archetype_of(prefix_index);
        let primary = archetype.lines[0].0;
        let best = books
            .iter()
            .map(|book| blueprint(book.art_id).grade)
            .max()
            .unwrap_or(ArtGrade::Yellow);
        let magnitude = INTERNAL_MAGNITUDES[best.index()];
        let extra = u32::try_from(books.len() - 1).unwrap_or(u32::MAX);
        views.push(ArtSetView {
            prefix: prefix_index,
            books: u8::try_from(books.len()).unwrap_or(u8::MAX),
            bonus: {
                let mut bonus = Derived::default();
                primary.add_points(&mut bonus, extra * magnitude as u32 * ART_SET_STEP);
                bonus
            },
        });
    }
    views
}

/// The prefix an 功法 id carries: the fastest-moving part of its name, and the
/// set it belongs to. See [`ART_PREFIXES`] — every book of one prefix is one
/// family, so the grouping can be read straight off the id.
pub fn prefix_of(id: u64) -> usize {
    let index = (id.clamp(1, ART_COUNT) - 1) as usize;
    (index / 2) % ART_PREFIXES.len()
}

/// The name a 前缀 family is set to — the word its books share, and the one a
/// surface prints beside the family's bonus.
pub fn art_prefix_name(prefix: usize) -> &'static str {
    ART_PREFIXES[prefix % ART_PREFIXES.len()]
}

/// The combat numbers every owned 内功 has paid so far, outright.
pub fn art_bonus(owned: &[OwnedArt]) -> Derived {
    let mut total = Derived::default();
    for art in owned {
        let blueprint = blueprint(art.art_id);
        if blueprint.kind != ArtKind::Internal {
            continue;
        }
        let times = i64::from(art.rank);
        let mastery = if art.rank >= blueprint.grade.ranks() {
            1_000 + i64::from(mastery_permille(blueprint.grade))
        } else {
            1_000
        };
        total = total.plus(scale_bonus(blueprint.bonus, times, mastery));
    }
    total
}

/// The combat numbers every **已习得** 内功 pays, for good.
pub fn mastered_art_bonus(mastered: &[u64]) -> Derived {
    let mut total = Derived::default();
    for id in mastered {
        let blueprint = blueprint(*id);
        if blueprint.kind != ArtKind::Internal {
            continue;
        }
        let times = i64::from(blueprint.grade.ranks());
        let mastery = 1_000 + i64::from(mastery_permille(blueprint.grade));
        total = total.plus(scale_bonus(blueprint.bonus, times, mastery));
    }
    total
}

/// One 阶's outright payout with a mastery multiplier folded in, rounded.
fn scale_bonus(bonus: Derived, times: i64, mastery: i64) -> Derived {
    let scale = |value: u32| -> u32 {
        let product = i64::from(value) * times * mastery;
        let rounded = (product + 500) / 1_000;
        u32::try_from(rounded).unwrap_or(u32::MAX)
    };
    Derived {
        attack: scale(bonus.attack),
        health: scale(bonus.health),
        inner: scale(bonus.inner),
        spell: scale(bonus.spell),
        resist: scale(bonus.resist),
        speed: scale(bonus.speed),
        hit: scale(bonus.hit),
        evade: scale(bonus.evade),
        aptitude: scale(bonus.aptitude),
    }
}

/// The attributes one 内功 pays as it stands, at the 阶 its training has
/// reached: every 阶 in so far at the blueprint's rate, and no mastery while
/// any is missing.
///
/// This is what the learned-arts panel prints beside a book that is still
/// being read — the same sum the attributes panel counts, not a promise about
/// 阶 the training has not paid for yet.
pub fn art_payout_at(art_id: u64, rank: u8) -> Attributes {
    let blueprint = blueprint(art_id);
    let times = i64::from(rank);
    Attributes::new(
        paid(blueprint.gain.root, times, 1_000),
        paid(blueprint.gain.strength, times, 1_000),
        paid(blueprint.gain.mana, times, 1_000),
        paid(blueprint.gain.agility, times, 1_000),
    )
}

/// The damage one 外功 does behind a given 攻击力.
pub fn external_damage(attack: u32, multiplier_permille: u32) -> u32 {
    u64::from(attack)
        .saturating_mul(u64::from(multiplier_permille))
        .saturating_div(1_000) as u32
}

/// What one 外功 is worth as *power*: the extra damage it lands per blow over a
/// bare hand, read at the rate 攻击力 is worth power.
///
/// The same conversion [`super::Derived::power`] uses — one point of 攻击力 is
/// `STRENGTH_WEIGHT` power per `ATTACK_PER_STRENGTH` points — so the figure the
/// page prints as "外功 +N" is the figure the fight would actually move by, not
/// a second scale invented for the panel.
pub fn external_power_gain(attack: u32, multiplier_permille: u32) -> u32 {
    let extra = external_damage(attack, multiplier_permille.saturating_sub(1_000));
    let rate = super::STRENGTH_WEIGHT * 1_000 / super::ATTACK_PER_STRENGTH;
    u32::try_from(u64::from(extra) * u64::try_from(rate).unwrap_or(0) / 1_000).unwrap_or(u32::MAX)
}

/// One 功法 as a surface reads it, with the blueprint folded in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtView {
    pub id: u64,
    pub name: String,
    pub kind: ArtKind,
    pub grade: ArtGrade,
    pub rank: u8,
    pub max_rank: u8,
    pub progress: u32,
    /// What the next 阶 asks for.
    pub required: u32,
    /// The chance one attempt lands, as this body faces it, in parts per
    /// thousand — `base × 1.01^(潜质 + 失败)`.
    pub success_permille: u32,
    /// How many attempts at this 功法 have failed so far. What the rate above
    /// has been raised by, and what the row says when it is not zero. The count
    /// belongs to the whole book and is never reset at a 阶 turn.
    pub failures: u32,
    /// What one 阶 pays, for an 内功.
    pub gain: Attributes,
    /// What one 阶 pays outright in combat numbers, for an 内功: the 攻击力,
    /// 生命值, 命中 and the rest a fight reads directly. Zero for an 外功.
    pub bonus: Derived,
    /// What one use costs, for an 外功.
    pub cost: u32,
    /// What one use is worth, for an 外功.
    pub multiplier_permille: u32,
    /// How many blows one use throws, for an 外功.
    pub hits: u8,
    /// The band each blow is drawn from, for an 外功: `(0, 0)` is a move that
    /// pays exactly what its row says.
    pub band_permille: (u32, u32),
    /// What a landing blow does besides the hurt, for an 外功.
    pub effect: Option<StrikeEffect>,
    /// What one 阶 pays in 潜质, in tenths: zero for every book but the ten
    /// written for it.
    pub aptitude_tenths: i32,
    /// What the art's last 阶 pays on top, in parts per thousand: zero until
    /// every 阶 is in. An 内功 pays its attributes multiplied by this, an 外功
    /// adds this share of 攻击力 to every blow.
    pub mastery_permille: u32,
    /// Whether this is the art the next finished pomodoro trains.
    pub selected: bool,
    /// How many copies of the book are on the shelf. One means the row is the
    /// book itself; two means a spare, sellable and readable for nothing new.
    pub copies: u32,
}

/// One **已习得** 功法 as a surface reads it.
///
/// A learned 功法 has no progress left to show and no 阶 left to climb, so it
/// carries what it *is* rather than what it is still becoming: its name, its
/// grade, and the mastery it pays for good. The number a surface needs in
/// order to draw it is the same one the fight uses, so the two cannot disagree.
/// A book still being trained shows here too once its first 阶 is in — it is
/// learned as far as it has gone — and then `complete` is false and `rank` is
/// where the training stands rather than the grade's last 阶.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MasteredArtView {
    pub id: u64,
    pub name: String,
    pub kind: ArtKind,
    pub grade: ArtGrade,
    /// The 阶 it stands at: the grade's last for a complete 功法, the training's
    /// current one for a book still being read.
    pub rank: u8,
    /// How many 阶 the grade has in all.
    pub max_rank: u8,
    /// Whether every 阶 is in. An incomplete 功法 pays no mastery yet.
    pub complete: bool,
    /// What the last 阶 pays on top, in parts per thousand: zero while the
    /// 功法 is incomplete.
    pub mastery_permille: u32,
    /// What one 阶 pays, for an 内功.
    pub gain: Attributes,
    /// What one 阶 pays outright in combat numbers, for an 内功. Zero for an
    /// 外功 and for the 内功 whose shape is pure attributes.
    pub bonus: Derived,
    /// What one use costs, for an 外功.
    pub cost: u32,
    /// What one use is worth, for an 外功.
    pub multiplier_permille: u32,
    /// How many blows one use throws, for an 外功.
    pub hits: u8,
    /// The band each blow is drawn from, for an 外功: `(0, 0)` is a move that
    /// pays exactly what its row says.
    pub band_permille: (u32, u32),
    /// What a landing blow does besides the hurt, for an 外功.
    pub effect: Option<StrikeEffect>,
    /// What one 阶 pays in 潜质, in tenths: zero for every book but the ten
    /// written for it.
    pub aptitude_tenths: i32,
    /// Whether the tome's sale has been collected: the 功法 itself is not for
    /// sale a second time, though nothing it pays is ever taken away. A book
    /// still being trained has no collected sale and no sale here — its row is
    /// a report, and the shelf owns the selling.
    pub sold: bool,
}

/// Fold an owned art and its blueprint into what a surface draws.
pub fn art_view(owned: &OwnedArt, aptitude_tenths: i32, selected: bool) -> ArtView {
    let art = blueprint(owned.art_id);
    ArtView {
        id: art.id,
        name: art.name,
        kind: art.kind,
        grade: art.grade,
        rank: owned.rank,
        max_rank: art.grade.ranks(),
        progress: owned.progress,
        required: required_pomodoros(owned.rank, aptitude_tenths),
        success_permille: super::aptitude::success_permille(
            art.grade,
            aptitude_tenths,
            owned.failures,
        ),
        failures: owned.failures,
        gain: art.gain,
        bonus: art.bonus,
        cost: art.cost,
        multiplier_permille: art.multiplier_permille,
        hits: art.hits,
        band_permille: art.band_permille,
        effect: art.effect,
        aptitude_tenths: art.aptitude_tenths,
        mastery_permille: if owned.rank >= art.grade.ranks() {
            mastery_permille(art.grade)
        } else {
            0
        },
        selected,
        // A zero would be a book nobody can hold; an old save that somehow
        // carried one is read as a single copy rather than as a row that sells
        // into the negative.
        copies: owned.copies.max(1),
    }
}

/// Fold a learned 功法 into what a surface draws.
///
/// The id is enough: a learned 功法 is complete, so its 阶 is its grade's last
/// and its mastery is the one its grade pays.
pub fn mastered_art_view(art_id: u64) -> MasteredArtView {
    let art = blueprint(art_id);
    MasteredArtView {
        id: art.id,
        name: art.name,
        kind: art.kind,
        grade: art.grade,
        rank: art.grade.ranks(),
        max_rank: art.grade.ranks(),
        complete: true,
        mastery_permille: mastery_permille(art.grade),
        gain: art.gain,
        bonus: art.bonus,
        cost: art.cost,
        multiplier_permille: art.multiplier_permille,
        hits: art.hits,
        band_permille: art.band_permille,
        effect: art.effect,
        aptitude_tenths: art.aptitude_tenths,
        // The caller that knows the save marks the collected sales; a view
        // built from the id alone has nothing sold to speak of.
        sold: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two 外功 of one grade are two moves: they differ in how many blows they
    /// throw, in the band each is drawn from, and in what a landing one lays on.
    /// A grade used to print one number for every book that carried it.
    #[test]
    fn no_two_外功_of_a_grade_are_the_same_move() {
        let mut moves = std::collections::BTreeSet::new();
        let mut worths = std::collections::BTreeSet::new();
        for id in 1..=ART_COUNT {
            let art = blueprint(id);
            if art.kind != ArtKind::External || art.grade != ArtGrade::Divine {
                continue;
            }
            moves.insert((art.hits, art.band_permille, art.effect));
            worths.insert(art.multiplier_permille);
        }
        assert!(
            moves.len() > 3,
            "every 神阶 外功 is the same move: {moves:?}"
        );
        assert!(
            worths.len() > 3,
            "every 神阶 外功 is worth the same: {worths:?}"
        );
        // A 连击 throws three and a 单 one: the shape is the axis the count
        // differs on, and both are on the shelf.
        let hits: std::collections::BTreeSet<u8> = (1..=ART_COUNT)
            .map(blueprint)
            .filter(|art| art.kind == ArtKind::External)
            .map(|art| art.hits)
            .collect();
        assert!(hits.contains(&1) && hits.contains(&3), "shelf: {hits:?}");
        // And the effects are all three of them, nowhere near one shelf of none.
        let effects: std::collections::BTreeSet<u32> = (1..=ART_COUNT)
            .map(blueprint)
            .filter_map(|art| art.effect)
            .map(|effect| match effect {
                StrikeEffect::Slow { .. } => 0,
                StrikeEffect::Haste { .. } => 1,
                StrikeEffect::Focus { .. } => 2,
            })
            .collect();
        assert_eq!(effects.len(), 3, "every effect is somewhere: {effects:?}");
    }

    /// A prefix is a 流派: every 功法 sharing one pays the same attributes, and
    /// the eight families are all different.
    ///
    /// This is what the name promises and nothing more. Two 九霄 arts are of one
    /// family and pay one profile, so the prefix is a real kinship a player can
    /// read off the shelf — but it is not a set bonus, and the naming does not
    /// pretend to one.
    #[test]
    fn a_prefix_is_a_family_and_the_families_differ() {
        let mut profiles: std::collections::BTreeMap<
            String,
            std::collections::BTreeSet<[bool; 4]>,
        > = std::collections::BTreeMap::new();
        let mut named: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for id in 1..=ART_COUNT {
            let art = blueprint(id);
            let prefix: String = art.name.chars().take(2).collect();
            named.insert(prefix.clone());
            // 外功 pays no attributes and combat-only 内功 pays none either: the
            // family's attribute profile is what is being read here.
            if art.kind != ArtKind::Internal || art.gain.total() == 0 {
                continue;
            }
            let profile = [
                art.gain.root > 0,
                art.gain.strength > 0,
                art.gain.mana > 0,
                art.gain.agility > 0,
            ];
            profiles.entry(prefix).or_default().insert(profile);
        }
        for (prefix, found) in &profiles {
            assert_eq!(found.len(), 1, "{prefix} 一门的功法加成两种属性: {found:?}");
        }
        // Every prefix in the table is on some name: the shelf used to reach only
        // the first three of the forty, which is why every 功法 was a 九霄.
        let missing: Vec<&str> = ART_PREFIXES
            .into_iter()
            .filter(|prefix| !named.contains(*prefix))
            .collect();
        assert!(
            missing.is_empty(),
            "these families have no 功法: {missing:?}"
        );
        let families: std::collections::BTreeSet<[bool; 4]> =
            profiles.values().flatten().copied().collect();
        assert_eq!(families.len(), ARCHETYPES.len(), "流派不重样: {families:?}");
    }

    /// Two books of one family and one grade are still two books: the 成色 band
    /// is what keeps a shelf of five hundred names from being one art.
    #[test]
    fn no_two_arts_of_a_grade_print_the_same_number() {
        let mut totals = std::collections::BTreeSet::new();
        for id in 1..=ART_COUNT {
            let art = blueprint(id);
            if art.kind == ArtKind::Internal
                && art.grade == ArtGrade::Divine
                && art.gain.total() > 0
            {
                totals.insert(art.gain.total());
            }
        }
        assert!(
            totals.len() > 1,
            "every 神阶 内功 pays the same total: {totals:?}"
        );
    }

    #[test]
    fn an_id_is_the_same_art_every_time() {
        let first = blueprint(137);
        let again = blueprint(137);
        assert_eq!(first, again, "a save names an art by its id");
        assert!(!first.name.is_empty());
    }

    #[test]
    fn the_five_hundred_are_five_hundred_distinct_names() {
        let mut names = std::collections::BTreeSet::new();
        for id in 1..=ART_COUNT {
            names.insert(blueprint(id).name);
        }
        assert_eq!(
            names.len(),
            ART_COUNT as usize,
            "every 功法 must be told apart by its name"
        );
    }

    #[test]
    fn the_grades_are_bottom_heavy() {
        let mut counts = [0u32; 5];
        for id in 1..=ART_COUNT {
            counts[blueprint(id).grade.index()] += 1;
        }
        assert_eq!(counts[0], 200, "黄阶");
        assert_eq!(counts[4], 15, "神阶 is rare");
        assert!(counts[0] > counts[1] && counts[1] > counts[4]);
    }

    #[test]
    fn the_rank_costs_climb_geometrically() {
        let costs: Vec<u32> = (1..=10).map(printed_pomodoros).collect();
        assert_eq!(costs, vec![1, 2, 3, 4, 5, 7, 10, 14, 20, 29]);
        // A 黄阶 art is six sessions; a 神阶 one is a season, not a decade.
        assert_eq!(costs[..3].iter().sum::<u32>(), 6);
        assert_eq!(costs.iter().sum::<u32>(), 95);
    }

    /// The printed cost is what a body with no 潜质 pays three times over; the
    /// two are the same function, so the arithmetic a surface prints and the
    /// arithmetic a session charges cannot disagree.
    #[test]
    fn the_printed_cost_is_the_zero_aptitude_cost_over_three() {
        for rank in 1..=10u8 {
            let printed = printed_pomodoros(rank);
            let asked = required_pomodoros(rank, 0);
            assert_eq!(
                asked,
                super::super::aptitude::required_pomodoros(printed, 0),
                "rank {rank}"
            );
            assert!(asked >= printed * 2, "a bare body pays the threefold");
        }
    }

    #[test]
    fn aptitude_sets_the_success_rate_and_saturates() {
        assert_eq!(
            super::super::aptitude::success_permille(ArtGrade::Divine, 0, 0),
            50,
            "a 神阶 art is a one-in-twenty read at no 潜质"
        );
        assert!(
            super::super::aptitude::success_permille(ArtGrade::Divine, 1_000, 0)
                > super::super::aptitude::success_permille(ArtGrade::Divine, 0, 0),
            "潜质 is the lever"
        );
        assert!(
            super::super::aptitude::success_permille(ArtGrade::Divine, 0, 20)
                > super::super::aptitude::success_permille(ArtGrade::Divine, 0, 0),
            "and a run of failures is the same lever from the other side"
        );
        assert_eq!(
            super::super::aptitude::success_permille(ArtGrade::Yellow, 10_000, 0),
            super::super::aptitude::SUCCESS_CAP_PERMILLE,
            "a rate past certainty is held to certainty"
        );
    }

    /// Every grade's per-阶 payout is clearly larger than the one below, and 玄
    /// sits a third above the figure it used to carry.
    #[test]
    fn the_grade_payouts_are_far_apart_and_anchor_on_profound() {
        let magnitudes = INTERNAL_MAGNITUDES;
        assert_eq!(magnitudes[1], 8, "玄阶 pays eight a 阶");
        for pair in magnitudes.windows(2) {
            assert!(
                pair[1] * 2 >= pair[0] * 3,
                "adjacent grades must be half again apart: {magnitudes:?}"
            );
        }
        // Each grade's arts actually carry that budget, and no two of them print
        // the same numbers: the payout is spent at the art's own rates, so a 力量
        // technique and a 身法 technique pay different point totals for the same
        // power — which is the whole point of a shelf of five hundred names.
        for (index, grade) in ArtGrade::ALL.into_iter().enumerate() {
            let arts: Vec<ArtBlueprint> = (1..=ART_COUNT)
                .map(blueprint)
                .filter(|art| art.grade == grade && art.kind == ArtKind::Internal)
                .collect();
            assert!(!arts.is_empty(), "{grade:?} has no 内功");
            let best = arts
                .iter()
                .map(|art| art.gain.power() + art.bonus.power())
                .max()
                .unwrap_or(0);
            assert!(
                i32::try_from(best).unwrap_or(i32::MAX) >= INTERNAL_MAGNITUDES[index] * 2,
                "{grade:?} pays its magnitude in power: {best}"
            );
            let printed: std::collections::BTreeSet<(i32, i32, i32, i32)> = arts
                .iter()
                .map(|art| {
                    (
                        art.gain.root,
                        art.gain.strength,
                        art.gain.mana,
                        art.gain.agility,
                    )
                })
                .collect();
            assert!(
                printed.len() > 1,
                "every 内功 of {grade:?} pays the same four numbers: {printed:?}"
            );
        }
        // An 外功's multiplier rises with the grade just as steeply.
        let multipliers: Vec<u32> = ArtGrade::ALL
            .into_iter()
            .filter_map(|grade| {
                (1..=ART_COUNT)
                    .map(blueprint)
                    .find(|art| art.grade == grade && art.kind == ArtKind::External)
                    .map(|art| art.multiplier_permille)
            })
            .collect();
        assert_eq!(multipliers.len(), 5);
        // The anchor is a *centre* now, not the number: a copy carries a few
        // percent of its own, so the row sits either side of the grade's figure
        // and two 玄阶 外功 are worth two things.
        let centres = [1_200_u32, 2_025, 2_700, 3_600, 4_800];
        for (index, value) in multipliers.iter().enumerate() {
            let centre = centres[index];
            assert!(
                *value * 1_000 >= centre * 940 && *value * 1_000 <= centre * 1_060,
                "{index} 外功 is worth {value}, outside the 成色 band around {centre}"
            );
        }
        // Read on the centres rather than on one sample apiece: the ladder is
        // what is 1.3 apart, and a copy's 成色 is a few percent either side of
        // its own rung.
        for pair in centres.windows(2) {
            assert!(pair[1] * 10 >= pair[0] * 13, "显著差异: {centres:?}");
        }
    }

    /// The five discovery weights add up to exactly one million and the 神阶
    /// share is the brief's ten-thousandth.
    #[test]
    fn the_grades_are_found_far_apart_and_divine_is_one_in_ten_thousand() {
        let total: u32 = ART_GRADE_WEIGHTS_PER_MILLION.iter().sum();
        assert_eq!(total, 1_000_000, "the roll must always answer");
        assert_eq!(ART_GRADE_WEIGHTS_PER_MILLION[4], 100, "神阶 万分之一");
        for pair in ART_GRADE_WEIGHTS_PER_MILLION.windows(2) {
            assert!(
                pair[0] >= pair[1] * 2,
                "neighbouring grades must not be near-equal: {:?}",
                ART_GRADE_WEIGHTS_PER_MILLION
            );
        }
        // Every grade is reachable from the roll.
        let mut seen = [0u32; 5];
        let mut rng = Rng::seeded(0xC0FF_EE12_3456_789A);
        for _ in 0..200_000 {
            seen[roll_art_grade(&mut rng).index()] += 1;
        }
        assert!(seen.iter().all(|count| *count > 0), "{seen:?}");
    }

    #[test]
    fn a_failed_attempt_costs_the_session_and_nothing_else() {
        // One attempt in twelve fails a 黄阶 art. Each probe starts from a fresh
        // art with two pomodoros already banked, so the assertion is about what
        // a failure does to progress rather than about where a run of luck ends.
        let failed = (0..1_000_u64)
            .find_map(|seed| {
                let mut probe = OwnedArt {
                    art_id: 1,
                    rank: 1,
                    progress: 2,
                    outcome: None,
                    failures: 0,
                    copies: 1,
                };
                match train(&mut probe, 0, &mut Rng::seeded(seed)) {
                    ArtOutcome::Failed => Some(probe),
                    _ => None,
                }
            })
            .expect("some seed must fail a 黄阶 attempt");
        assert_eq!(failed.outcome, Some(ArtOutcome::Failed));
        assert_eq!(failed.progress, 2, "banked progress is never taken back");
        assert_eq!(failed.rank, 1);
    }

    /// A failure is banked against the whole 功法: the next attempt is likelier,
    /// and the count survives a 阶 turning over — it is the book being read, not
    /// the rung it was read at.
    #[test]
    fn a_failed_attempt_is_banked_against_the_whole_art() {
        let art_id = (1..=ART_COUNT)
            .find(|id| blueprint(*id).grade == ArtGrade::Divine)
            .expect("a 神阶 art is in the catalog");
        let mut owned = OwnedArt {
            art_id,
            rank: 1,
            progress: 0,
            failures: 0,
            copies: 1,
            outcome: None,
        };
        let base = art_view(&owned, 0, false).success_permille;
        let mut rng = Rng::seeded(0xA17);
        // A 神阶 art at zero 潜质 lands one time in twenty, so a miss comes
        // quickly and the bank is one deep.
        let mut guard = 0;
        while owned.failures == 0 {
            let _ = train(&mut owned, 0, &mut rng);
            guard += 1;
            assert!(guard < 5_000, "a 神阶 art eventually misses");
        }
        let banked = art_view(&owned, 0, false);
        assert_eq!(banked.failures, owned.failures);
        assert!(
            banked.success_permille > base,
            "the next attempt is likelier: {} against {base}",
            banked.success_permille
        );
        let at_the_turn = owned.failures;
        // The count is not handed back when the 阶 turns over.
        let mut guard = 0;
        while owned.rank == 1 {
            let _ = train(&mut owned, 0, &mut rng);
            guard += 1;
            assert!(guard < 100_000, "the first 阶 is one session at most");
        }
        assert!(
            owned.failures >= at_the_turn,
            "the run carries past the 阶: {} against {at_the_turn}",
            owned.failures
        );
        assert_eq!(
            art_view(&owned, 0, false).success_permille,
            super::super::aptitude::success_permille(ArtGrade::Divine, 0, owned.failures),
            "the rate reads the whole book's count"
        );
    }

    #[test]
    fn a_run_of_good_luck_turns_the_rank_over() {
        let mut owned = OwnedArt {
            art_id: 1,
            rank: 1,
            progress: 0,
            outcome: None,
            failures: 0,
            copies: 1,
        };
        // 潜质 100 leaves a 黄阶 art at the success cap; a body that good is a
        // body whose training keeps landing.
        let mut rng = Rng::seeded(0xABCD);
        let mut ranked = false;
        for _ in 0..200 {
            if train(&mut owned, 1_000, &mut rng) == ArtOutcome::RankedUp {
                ranked = true;
                break;
            }
        }
        assert!(ranked, "the first 阶 costs one pomodoro");
        assert_eq!(owned.rank, 2);
        assert_eq!(owned.progress, 0, "the counter resets for the next 阶");
    }

    #[test]
    fn an_internal_art_pays_attributes_and_an_external_one_does_not() {
        let mut owned = vec![OwnedArt {
            art_id: 1,
            rank: 2,
            progress: 0,
            outcome: None,
            failures: 0,
            copies: 1,
        }];
        let art = blueprint(1);
        if art.kind == ArtKind::Internal {
            // Two 阶 in and the third still owed: exactly twice the gain.
            assert_eq!(art_attributes(&owned).total(), art.gain.total() * 2);
        } else {
            assert_eq!(art_attributes(&owned), Attributes::default());
            assert!(art.cost > 0 && art.multiplier_permille > 1_000);
        }
        // A completed 内功 pays for every 阶 it reached — and the mastery
        // bonus on every one of them, because the last 阶 is what completes it.
        owned[0].rank = art.grade.ranks();
        if art.kind == ArtKind::Internal {
            let times = i64::from(art.grade.ranks());
            let mastery = 1_000 + i64::from(mastery_permille(art.grade));
            let expected = Attributes {
                root: paid(art.gain.root, times, mastery),
                strength: paid(art.gain.strength, times, mastery),
                mana: paid(art.gain.mana, times, mastery),
                agility: paid(art.gain.agility, times, mastery),
            };
            assert_eq!(art_attributes(&owned), expected);
        }
    }

    #[test]
    fn a_completed_internal_art_pays_more_than_its_ranks_alone() {
        // An 内功 whose shape is attributes: the mastery share is paid on every
        // 阶, so completing the last one is worth more than the 阶 alone. The
        // combat-paying shapes have no attribute gain to master and are not
        // what this one is about.
        let art = (1..=ART_COUNT)
            .map(blueprint)
            .find(|art| art.kind == ArtKind::Internal && art.gain.total() > 0)
            .expect("some 内功 pays attributes");
        let ranks = art.grade.ranks();
        let unfinished = vec![OwnedArt {
            art_id: art.id,
            rank: ranks - 1,
            progress: 0,
            outcome: None,
            failures: 0,
            copies: 1,
        }];
        let finished = vec![OwnedArt {
            art_id: art.id,
            rank: ranks,
            progress: 0,
            outcome: None,
            failures: 0,
            copies: 1,
        }];
        let before = art_attributes(&unfinished).total();
        let after = art_attributes(&finished).total();
        let ranks_alone = art.gain.total() * u32::from(ranks);
        assert!(
            after > ranks_alone,
            "the mastery bonus is paid, not floored away: {after} against {ranks_alone}"
        );
        assert!(
            after > before + art.gain.total(),
            "the last 阶 pays its own gain and the bonus besides"
        );
    }

    #[test]
    fn a_mastery_bonus_grows_with_the_grade() {
        let bonuses: Vec<u32> = ArtGrade::ALL.into_iter().map(mastery_permille).collect();
        assert_eq!(bonuses, vec![150, 200, 250, 300, 400]);
        assert!(
            bonuses[0] < bonuses[4],
            "a 神阶 art is the one worth finishing"
        );
    }

    /// Five grades, five colours: 黄、蓝、紫、橙、红 and none repeated, so a
    /// shelf of five 功法 is readable before a word of it is read.
    #[test]
    fn every_grade_has_its_own_colour() {
        let colours: Vec<u32> = ArtGrade::ALL
            .into_iter()
            .map(|grade| grade.color())
            .collect();
        let unique: std::collections::BTreeSet<u32> = colours.iter().copied().collect();
        assert_eq!(
            unique.len(),
            5,
            "each grade paints differently: {colours:?}"
        );
        // The ladder runs warmer at the top: the 神阶 colour leans on its red
        // far harder than the 黄阶 one does, so "the red one is the rare one" is
        // true to look at and not just to read.
        let warmth = |value: u32| {
            let red = i32::try_from((value >> 16) & 0xFF).unwrap_or(0);
            let green = i32::try_from((value >> 8) & 0xFF).unwrap_or(0);
            red - green
        };
        assert!(
            warmth(colours[4]) > warmth(colours[0]),
            "神阶 must read hotter than 黄阶"
        );
    }

    #[test]
    fn an_external_art_hits_harder_than_a_bare_hand() {
        assert_eq!(external_damage(100, 1_200), 120);
        assert_eq!(external_damage(0, 3_000), 0);
    }

    /// The brief's list: ten books and no more, two per grade, every one of them
    /// an 内功 sitting inside its own grade's band.
    #[test]
    fn only_ten_books_raise_aptitude_and_two_of_each_grade() {
        assert_eq!(APTITUDE_ART_IDS.len(), 10);
        let unique: std::collections::BTreeSet<u64> = APTITUDE_ART_IDS.iter().copied().collect();
        assert_eq!(
            unique.len(),
            APTITUDE_ART_IDS.len(),
            "a book is listed twice"
        );
        let mut per_grade = [0_usize; 5];
        for id in APTITUDE_ART_IDS {
            assert!((1..=ART_COUNT).contains(&id), "{id} is off the catalog");
            let art = blueprint(id);
            per_grade[art.grade.index()] += 1;
            assert_eq!(art.kind, ArtKind::Internal, "{id} is swung, not trained");
            assert!(art.aptitude_tenths > 0);
        }
        assert_eq!(
            per_grade,
            [2, 2, 2, 2, 2],
            "two books a grade: {per_grade:?}"
        );
        // Every other book pays nothing at all: 潜质 has one source and ten
        // names, and a hybrid shape that rolled it would be a second source.
        let others_with_aptitude = (1..=ART_COUNT)
            .filter(|id| !APTITUDE_ART_IDS.contains(id))
            .filter(|id| blueprint(*id).aptitude_tenths != 0)
            .count();
        assert_eq!(others_with_aptitude, 0);
    }

    /// The band runs 0.2 to 2.0 a 阶, and the grade alone decides where in it a
    /// book sits.
    #[test]
    fn an_aptitude_book_pays_two_tenths_to_two_points_a_rank() {
        assert_eq!(APTITUDE_ART_TENTHS_PER_RANK[0], 2, "0.2 a rank");
        assert_eq!(APTITUDE_ART_TENTHS_PER_RANK[4], 20, "2.0 a rank");
        for pair in ArtGrade::ALL.windows(2) {
            assert!(
                APTITUDE_ART_TENTHS_PER_RANK[pair[1].index()]
                    > APTITUDE_ART_TENTHS_PER_RANK[pair[0].index()],
                "{:?} pays no more than {:?}",
                pair[1],
                pair[0]
            );
        }
        // What a book has actually paid grows with the 阶 it has reached, and a
        // completed one pays its mastery share on top.
        let id = APTITUDE_ART_IDS[4];
        let ranks = blueprint(id).grade.ranks();
        let mut previous = 0;
        for rank in 0..=ranks {
            let paid = art_aptitude_tenths_at(id, rank);
            assert!(
                paid >= previous,
                "rank {rank} paid less than the one before"
            );
            previous = paid;
        }
        assert_eq!(
            art_aptitude_tenths_at(1, 3),
            0,
            "an ordinary book pays nothing"
        );
        assert_eq!(mastered_art_aptitude_tenths(&[id]), previous);
    }

    /// The books are found at half the odds of any other: the weight that picks
    /// one out of its grade's band is one against two.
    #[test]
    fn an_aptitude_book_is_found_half_as_often() {
        assert_eq!(art_pick_weight(APTITUDE_ART_IDS[0]), 1);
        assert_eq!(art_pick_weight(1), 2);
        // Measured on the pick itself: a grade's band, drawn with those weights,
        // hands out aptitude books at half the rate a flat pick would.
        let grade = ArtGrade::Yellow;
        let band: Vec<u64> = (1..=ART_COUNT)
            .filter(|id| blueprint(*id).grade == grade)
            .collect();
        let aptitude = band
            .iter()
            .filter(|id| aptitude_tenths_per_rank(**id) > 0)
            .count();
        let flat = aptitude as f64 / band.len() as f64;
        let weighted =
            (aptitude as f64 * 1.0) / (aptitude as f64 + (band.len() - aptitude) as f64 * 2.0);
        assert!(
            (weighted / flat - 0.5).abs() < 0.01,
            "weighted {weighted} against flat {flat}"
        );
    }
}
