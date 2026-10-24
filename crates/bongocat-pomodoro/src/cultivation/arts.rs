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
    pub multiplier_permille: u32,
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

const ART_SUFFIXES: [&str; 8] = ["", "诀", "经", "录", "谱", "篇", "总纲", "残卷"];

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
    let suffix = ART_SUFFIXES[rest % ART_SUFFIXES.len()];
    let noun_index = (rest / ART_SUFFIXES.len()) % INTERNAL_NOUNS.len();
    let noun = match kind {
        ArtKind::Internal => INTERNAL_NOUNS[noun_index],
        ArtKind::External => EXTERNAL_NOUNS[noun_index],
    };
    let prefix =
        ART_PREFIXES[(rest / (ART_SUFFIXES.len() * INTERNAL_NOUNS.len())) % ART_PREFIXES.len()];
    let name = format!("{prefix}{noun}{suffix}");
    let mut rng = Rng::seeded(id.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x0005_DEEC_E66D);
    let (gain, bonus, cost, multiplier_permille) = match kind {
        ArtKind::Internal => {
            let (gain, bonus) = internal_gain(grade, &mut rng);
            (gain, bonus, 0, 0)
        }
        ArtKind::External => (
            Attributes::default(),
            Derived::default(),
            external_cost(grade),
            external_multiplier(grade),
        ),
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
/// The per-阶 payout by grade: 2, 6, 9, 14, 21.
///
/// 玄 is one and a half times the old four — the brief's own anchor — and every
/// step after it is at least half again as large as the one below, so a 神阶
/// 内功 is not a 黄阶 one with a longer climb. Neighbouring grades are never
/// "about the same": the 玄→地 step is the shallowest at ×1.5.
const INTERNAL_MAGNITUDES: [i32; 5] = [2, 6, 9, 14, 21];

fn internal_gain(grade: ArtGrade, rng: &mut Rng) -> (Attributes, Derived) {
    let magnitude = INTERNAL_MAGNITUDES[grade.index()];
    let shape = rng.below(100);
    let mut gain = Attributes::default();
    let mut bonus = Derived::default();
    let pays_attributes = !(40..75).contains(&shape);
    let pays_combat = shape >= 40;
    if pays_attributes {
        let primary = Attribute::from_index(rng.below(4) as usize);
        primary.add_to(&mut gain, magnitude);
        // Every third grade splits the payout, so the deep arts are the
        // versatile ones rather than more of the same.
        if grade.index() >= 2 {
            let secondary = Attribute::from_index(rng.below(4) as usize);
            secondary.add_to(&mut gain, magnitude / 2);
        }
    }
    if pays_combat {
        let line = CombatLine::from_index(rng.below(CombatLine::ALL.len() as u64) as usize);
        line.add_to(&mut bonus, magnitude);
        // The deep grades split the combat payout the same way the attribute
        // one is split, so a 神阶 hybrid is the versatile one.
        if grade.index() >= 2 {
            let secondary =
                CombatLine::from_index(rng.below(CombatLine::ALL.len() as u64) as usize);
            secondary.add_to(&mut bonus, magnitude / 2);
        }
    }
    (gain, bonus)
}

/// Which of the nine combat numbers one 阶 of an 内功 can pay outright.
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
    Aptitude,
}

impl CombatLine {
    const ALL: [Self; 9] = [
        Self::Attack,
        Self::Health,
        Self::Inner,
        Self::Spell,
        Self::Resist,
        Self::Speed,
        Self::Hit,
        Self::Evade,
        Self::Aptitude,
    ];

    fn from_index(index: usize) -> Self {
        Self::ALL[index.min(Self::ALL.len() - 1)]
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
        let (slot, rate) = match self {
            Self::Attack => (&mut bonus.attack, 3),
            Self::Health => (&mut bonus.health, 20),
            Self::Inner => (&mut bonus.inner, 8),
            Self::Spell => (&mut bonus.spell, 3),
            Self::Resist => (&mut bonus.resist, 2),
            Self::Speed => (&mut bonus.speed, 2),
            Self::Hit => (&mut bonus.hit, 10),
            Self::Evade => (&mut bonus.evade, 10),
            Self::Aptitude => (&mut bonus.aptitude, 2),
        };
        *slot = slot.saturating_add((magnitude * rate) as u32);
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
fn external_multiplier(grade: ArtGrade) -> u32 {
    const MULTIPLIERS: [u32; 5] = [1_200, 2_025, 2_700, 3_600, 4_800];
    MULTIPLIERS[grade.index()]
}

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
    /// How many attempts at the *current* 阶 have failed.
    ///
    /// Reset the moment the 阶 turns over: the compensation a bad run earns is
    /// worth something on the rung it was paid on and nothing on the next one,
    /// or a single unlucky 阶 would follow a book all the way to its last.
    #[serde(default)]
    pub failures: u32,
    /// What an attempt just came to.
    pub outcome: Option<ArtOutcome>,
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
    // 潜质 sets the chance the session lands at all: one roll against the rate
    // this grade carries at this body's 潜质. A failure costs the session and
    // nothing else.
    let success = super::aptitude::compensated_success_permille(
        super::aptitude::success_permille(art.grade, aptitude_tenths),
        owned.failures,
    );
    if rng.below(1_000) >= u64::from(success) {
        // The failure is banked against this 阶: the next attempt at it is one
        // percent likelier, and the one after that likelier again.
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
    // A new 阶 is a fresh climb: the failures that bought the last one stay
    // behind with it.
    owned.failures = 0;
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
    /// thousand — 潜质's rate already multiplied by the run of failures this 阶
    /// has cost.
    pub success_permille: u32,
    /// How many attempts at this 阶 have failed. What the rate above has been
    /// raised by, and what the row says when it is not zero.
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
    /// What the art's last 阶 pays on top, in parts per thousand: zero until
    /// every 阶 is in. An 内功 pays its attributes multiplied by this, an 外功
    /// adds this share of 攻击力 to every blow.
    pub mastery_permille: u32,
    /// Whether this is the art the next finished pomodoro trains.
    pub selected: bool,
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
        success_permille: super::aptitude::compensated_success_permille(
            super::aptitude::success_permille(art.grade, aptitude_tenths),
            owned.failures,
        ),
        failures: owned.failures,
        gain: art.gain,
        bonus: art.bonus,
        cost: art.cost,
        multiplier_permille: art.multiplier_permille,
        mastery_permille: if owned.rank >= art.grade.ranks() {
            mastery_permille(art.grade)
        } else {
            0
        },
        selected,
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
        // The caller that knows the save marks the collected sales; a view
        // built from the id alone has nothing sold to speak of.
        sold: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            super::super::aptitude::success_permille(ArtGrade::Divine, 0),
            50,
            "a 神阶 art is a one-in-twenty read at no 潜质"
        );
        assert!(
            super::super::aptitude::success_permille(ArtGrade::Divine, 1_000)
                > super::super::aptitude::success_permille(ArtGrade::Divine, 0),
            "潜质 is the lever"
        );
        assert_eq!(
            super::super::aptitude::success_permille(ArtGrade::Yellow, 10_000),
            super::super::aptitude::SUCCESS_CAP_PERMILLE,
            "a rate past certainty is held to certainty"
        );
    }

    /// Every grade's per-阶 payout is clearly larger than the one below, and 玄
    /// is exactly one and a half times the old figure of four.
    #[test]
    fn the_grade_payouts_are_far_apart_and_anchor_on_profound() {
        let magnitudes = INTERNAL_MAGNITUDES;
        assert_eq!(magnitudes[1], 6, "玄阶 is the old 4 × 1.5");
        for pair in magnitudes.windows(2) {
            assert!(
                pair[1] * 2 >= pair[0] * 3,
                "adjacent grades must be half again apart: {magnitudes:?}"
            );
        }
        // Each grade's arts actually carry that magnitude: the 内功 of a grade
        // pays one attribute by it, so the blueprint says the table ran.
        for (index, grade) in ArtGrade::ALL.into_iter().enumerate() {
            let art = (1..=ART_COUNT)
                .map(blueprint)
                .find(|art| {
                    art.grade == grade && art.kind == ArtKind::Internal && art.gain.total() > 0
                })
                .expect("every grade has an attribute-paying 内功");
            let primary = [
                art.gain.root,
                art.gain.strength,
                art.gain.mana,
                art.gain.agility,
            ]
            .into_iter()
            .max()
            .unwrap_or(0);
            assert!(
                primary >= magnitudes[index],
                "{:?} pays at least its magnitude: {primary}",
                grade
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
        assert_eq!(multipliers[1], 2_025, "玄阶 外功 is 1350 × 1.5");
        for pair in multipliers.windows(2) {
            assert!(pair[1] * 10 >= pair[0] * 13, "显著差异: {multipliers:?}");
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

    /// A failure is banked against the 阶 it happened on: the next attempt at
    /// that 阶 is likelier, and a new 阶 starts from the bare 潜质 rate again.
    #[test]
    fn a_failed_attempt_is_banked_against_this_rank_and_forgotten_on_the_next() {
        let art_id = (1..=ART_COUNT)
            .find(|id| blueprint(*id).grade == ArtGrade::Divine)
            .expect("a 神阶 art is in the catalog");
        let mut owned = OwnedArt {
            art_id,
            rank: 1,
            progress: 0,
            failures: 0,
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
        // The run is forgotten the moment this 阶 turns over.
        let mut guard = 0;
        while owned.rank == 1 {
            let _ = train(&mut owned, 0, &mut rng);
            guard += 1;
            assert!(guard < 100_000, "the first 阶 is one session at most");
        }
        assert_eq!(owned.failures, 0, "a new 阶 starts from the bare rate");
        assert_eq!(
            art_view(&owned, 0, false).success_permille,
            super::super::aptitude::success_permille(ArtGrade::Divine, 0)
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
        }];
        let finished = vec![OwnedArt {
            art_id: art.id,
            rank: ranks,
            progress: 0,
            outcome: None,
            failures: 0,
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
}
