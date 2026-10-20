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

use super::{Attribute, Attributes, Rng};

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

    /// The chance one attempt at the next 阶 comes to nothing, in parts per
    /// thousand. Higher grades are harder to learn, which is the whole reason a
    /// 潜质 build is a choice rather than a trap.
    pub const fn failure_permille(self) -> u32 {
        match self {
            Self::Yellow => 80,
            Self::Profound => 120,
            Self::Earth => 180,
            Self::Heaven => 260,
            Self::Divine => 350,
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
    let (gain, cost, multiplier_permille) = match kind {
        ArtKind::Internal => (internal_gain(grade, &mut rng), 0, 0),
        ArtKind::External => (
            Attributes::default(),
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
        cost,
        multiplier_permille,
    }
}

/// What one 阶 of an 内功 pays: a handful of points, in one attribute or two.
///
/// Two attributes rather than four, so an art has a character — "the 寒冰 one
/// raises 法力" — instead of being four numbers with a name in front of them.
fn internal_gain(grade: ArtGrade, rng: &mut Rng) -> Attributes {
    let magnitude = (grade.index() as i32 + 1) * 2;
    let primary = Attribute::from_index(rng.below(4) as usize);
    let mut gain = Attributes::default();
    primary.add_to(&mut gain, magnitude);
    // Every third grade splits the payout, so the deep arts are the versatile
    // ones rather than more of the same.
    if grade.index() >= 2 {
        let secondary = Attribute::from_index(rng.below(4) as usize);
        secondary.add_to(&mut gain, magnitude / 2);
    }
    gain
}

/// What one use of an 外功 costs in 内力: the higher the grade, the dearer.
fn external_cost(grade: ArtGrade) -> u32 {
    8 + grade.index() as u32 * 6
}

/// What an 外功 is worth, in parts per thousand of the 攻击力 behind it.
fn external_multiplier(grade: ArtGrade) -> u32 {
    1_200 + grade.index() as u32 * 150
}

/// How many pomodoros the given 阶 takes: 1, 2, 3, 4, 5, 7, 10, 14, 20, 29.
///
/// Geometric, because the alternative is either a 10-阶 art that finishes in an
/// afternoon or a flat cost that makes the tenth 阶 no more of an achievement
/// than the first. The ratio is 1.45: steep enough to be felt, shallow enough
/// that a 神阶 art is a season rather than a decade.
pub fn required_pomodoros(rank: u8) -> u32 {
    let exponent = f64::from(rank.saturating_sub(1));
    (1.45_f64.powf(exponent)).ceil() as u32
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

/// How much a failed attempt is softened by 潜质, at most.
const APTITUDE_DISCOUNT_CAP: u32 = 150;

/// The chance one attempt fails, as this cultivator faces it.
///
/// 潜质 takes a point off the failure rate per point, up to a hundred and fifty
/// — enough to make a 根骨 build the better learner without making 神阶 trivial.
pub fn effective_failure_permille(grade: ArtGrade, aptitude: u32) -> u32 {
    grade
        .failure_permille()
        .saturating_sub(aptitude.min(APTITUDE_DISCOUNT_CAP))
}

/// A 功法 the cultivator owns.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OwnedArt {
    /// Which of the five hundred this is.
    pub art_id: u64,
    /// The 阶 reached. One is the first: an owned art is already begun.
    pub rank: u8,
    /// Pomodoros banked toward the next 阶.
    pub progress: u32,
    /// What an attempt just came to.
    pub outcome: Option<ArtOutcome>,
}

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
pub fn train(owned: &mut OwnedArt, aptitude: u32, rng: &mut Rng) -> ArtOutcome {
    let art = blueprint(owned.art_id);
    let max_rank = art.grade.ranks();
    if owned.rank >= max_rank {
        owned.outcome = Some(ArtOutcome::Mastered);
        return ArtOutcome::Mastered;
    }
    if rng.below(1_000) < u64::from(effective_failure_permille(art.grade, aptitude)) {
        owned.outcome = Some(ArtOutcome::Failed);
        return ArtOutcome::Failed;
    }
    owned.progress = owned.progress.saturating_add(1);
    let needed = required_pomodoros(owned.rank);
    if owned.progress < needed {
        owned.outcome = Some(ArtOutcome::Progressed);
        return ArtOutcome::Progressed;
    }
    owned.progress = 0;
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

/// The damage one 外功 does behind a given 攻击力.
pub fn external_damage(attack: u32, multiplier_permille: u32) -> u32 {
    u64::from(attack)
        .saturating_mul(u64::from(multiplier_permille))
        .saturating_div(1_000) as u32
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
    /// The failure chance as this cultivator faces it, in parts per thousand.
    pub failure_permille: u32,
    /// What one 阶 pays, for an 内功.
    pub gain: Attributes,
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MasteredArtView {
    pub id: u64,
    pub name: String,
    pub kind: ArtKind,
    pub grade: ArtGrade,
    /// The last 阶 of the grade, which is the one it stands at.
    pub rank: u8,
    /// What the last 阶 pays on top, in parts per thousand.
    pub mastery_permille: u32,
    /// What one 阶 pays, for an 内功.
    pub gain: Attributes,
    /// What one use costs, for an 外功.
    pub cost: u32,
    /// What one use is worth, for an 外功.
    pub multiplier_permille: u32,
}

/// Fold an owned art and its blueprint into what a surface draws.
pub fn art_view(owned: &OwnedArt, aptitude: u32, selected: bool) -> ArtView {
    let art = blueprint(owned.art_id);
    ArtView {
        id: art.id,
        name: art.name,
        kind: art.kind,
        grade: art.grade,
        rank: owned.rank,
        max_rank: art.grade.ranks(),
        progress: owned.progress,
        required: required_pomodoros(owned.rank),
        failure_permille: effective_failure_permille(art.grade, aptitude),
        gain: art.gain,
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
        mastery_permille: mastery_permille(art.grade),
        gain: art.gain,
        cost: art.cost,
        multiplier_permille: art.multiplier_permille,
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
        let costs: Vec<u32> = (1..=10).map(required_pomodoros).collect();
        assert_eq!(costs, vec![1, 2, 3, 4, 5, 7, 10, 14, 20, 29]);
        // A 黄阶 art is six sessions; a 神阶 one is a season, not a decade.
        assert_eq!(costs[..3].iter().sum::<u32>(), 6);
        assert_eq!(costs.iter().sum::<u32>(), 95);
    }

    #[test]
    fn aptitude_softens_the_failure_and_only_so_far() {
        let base = ArtGrade::Divine.failure_permille();
        assert_eq!(effective_failure_permille(ArtGrade::Divine, 0), base);
        assert_eq!(
            effective_failure_permille(ArtGrade::Divine, 100),
            base - 100
        );
        assert_eq!(
            effective_failure_permille(ArtGrade::Divine, 10_000),
            base - APTITUDE_DISCOUNT_CAP,
            "潜质 cannot make a 神阶 art safe"
        );
        assert_eq!(
            effective_failure_permille(ArtGrade::Yellow, 999),
            0,
            "a 黄阶 art is within reach of a gifted beginner"
        );
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

    #[test]
    fn a_run_of_good_luck_turns_the_rank_over() {
        let mut owned = OwnedArt {
            art_id: 1,
            rank: 1,
            progress: 0,
            outcome: None,
        };
        // 黄阶 fails 80‰ of the time; a seed that never draws below it is a
        // cultivator whose training keeps landing.
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
        // Art 1 is 黄阶: three 阶 of a two-point gain, and completing them pays
        // the fifteen-percent mastery share on top.
        let art = blueprint(1);
        assert_eq!(art.kind, ArtKind::Internal, "art 1 is an 内功");
        let ranks = art.grade.ranks();
        let unfinished = vec![OwnedArt {
            art_id: 1,
            rank: ranks - 1,
            progress: 0,
            outcome: None,
        }];
        let finished = vec![OwnedArt {
            art_id: 1,
            rank: ranks,
            progress: 0,
            outcome: None,
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

    #[test]
    fn an_external_art_hits_harder_than_a_bare_hand() {
        assert_eq!(external_damage(100, 1_200), 120);
        assert_eq!(external_damage(0, 3_000), 0);
    }
}
