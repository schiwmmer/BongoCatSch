//! Gear: the four slots, the eight qualities, the stars, and the one roll that
//! fills them.
//!
//! An item stores the attributes it grants — and the combat numbers it grants
//! outright — rather than the numbers it was rolled from. Retuning the table
//! therefore changes future drops only, which is the same rule the
//! configuration's field evolution follows: what a user already owns keeps
//! working, and nothing silently re-scores it.
//!
//! Three axes decide what a piece is, and they are independent on purpose:
//!
//! * **品质** — the eight grades, white to crimson. It sizes everything at roll
//!   time: the attributes, the outright combat numbers, the price and how often
//!   the piece appears at all. The top two grades are 神装 and carry an effect.
//! * **星级** — up to ten stars, bought one attempt at a time with 灵石. Every
//!   star is worth five to eight percent of everything the piece already pays,
//!   and an attempt can fail and drop a star back. It never changes the price
//!   and can never be sold back: stars are a sink, not an investment.
//! * **阶级** — the realm's own tier, which is what keeps a later realm's gear
//!   worth more than an earlier realm's best roll.

use serde::{Deserialize, Serialize};

use super::{Attribute, Attributes, Derived, Rng};

/// How many equipment slots a cultivator has. One item per slot at a time.
pub const SLOT_COUNT: usize = 4;

/// The most stars a piece of gear can hold.
pub const MAX_STAR: u8 = 10;

/// Where a piece of gear is worn.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EquipSlot {
    #[default]
    Weapon,
    Clothes,
    Pants,
    Boots,
}

impl EquipSlot {
    /// Every slot, in display order.
    pub const ALL: [Self; SLOT_COUNT] = [Self::Weapon, Self::Clothes, Self::Pants, Self::Boots];

    /// The slot at a position in [`Self::ALL`], clamped to the last one.
    pub fn from_index(index: usize) -> Self {
        Self::ALL[index.min(SLOT_COUNT - 1)]
    }

    /// The slot's position in [`Self::ALL`].
    pub const fn index(self) -> usize {
        match self {
            Self::Weapon => 0,
            Self::Clothes => 1,
            Self::Pants => 2,
            Self::Boots => 3,
        }
    }

    /// The catalog key that names this slot.
    pub const fn name_key(self) -> &'static str {
        match self {
            Self::Weapon => "cultivation.slot.weapon",
            Self::Clothes => "cultivation.slot.clothes",
            Self::Pants => "cultivation.slot.pants",
            Self::Boots => "cultivation.slot.boots",
        }
    }

    /// How this slot splits its attribute points over two attributes, in
    /// percent.
    ///
    /// A weapon is offence, the robe and the trousers are poise, and the boots
    /// are footwork with a little force behind it. The split is what makes the
    /// four slots read differently instead of being four copies of one bonus.
    const fn split(self) -> (Attribute, u32, Attribute, u32) {
        match self {
            Self::Weapon => (Attribute::Strength, 70, Attribute::Mana, 30),
            Self::Clothes => (Attribute::Root, 75, Attribute::Agility, 25),
            Self::Pants => (Attribute::Agility, 55, Attribute::Root, 45),
            Self::Boots => (Attribute::Agility, 60, Attribute::Strength, 40),
        }
    }

    /// The two combat numbers this slot adds outright, primary first.
    ///
    /// The attributes are what a piece *is*; these are what it *does*. A weapon
    /// swings, a robe soaks, the trousers keep the wearer upright and the boots
    /// keep them quick — so every slot reads differently in a fight and not only
    /// on the attribute panel.
    const fn combat_split(self) -> (DerivedLine, u32, DerivedLine, u32) {
        match self {
            Self::Weapon => (DerivedLine::Attack, 70, DerivedLine::Spell, 30),
            Self::Clothes => (DerivedLine::Health, 70, DerivedLine::Resist, 30),
            Self::Pants => (DerivedLine::Evade, 55, DerivedLine::Health, 45),
            Self::Boots => (DerivedLine::Speed, 60, DerivedLine::Hit, 40),
        }
    }
}

/// How good a drop is: the eight qualities, worst to best.
///
/// The first four keep the names the ladder always used, and a document that
/// carries an older `immortal` piece still reads it — the grade it was rolled
/// under stays the grade it is, only the table above it grew.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Rarity {
    /// 白 · 普通.
    #[default]
    Common,
    /// 绿 · 优秀.
    Fine,
    /// 蓝 · 精良.
    Rare,
    /// 紫 · 史诗. An older save's best grade reads as this one.
    Immortal,
    /// 橙 · 传说.
    Legendary,
    /// 金 · 神话.
    Mythic,
    /// 红 · 神装: carries an effect.
    Divine,
    /// 大红 · 绝世: the crimson 神装, with the deeper effect.
    Transcendent,
}

impl Rarity {
    /// Every quality, worst to best.
    pub const ALL: [Self; 8] = [
        Self::Common,
        Self::Fine,
        Self::Rare,
        Self::Immortal,
        Self::Legendary,
        Self::Mythic,
        Self::Divine,
        Self::Transcendent,
    ];

    /// The quality's position in [`Self::ALL`].
    pub const fn index(self) -> usize {
        match self {
            Self::Common => 0,
            Self::Fine => 1,
            Self::Rare => 2,
            Self::Immortal => 3,
            Self::Legendary => 4,
            Self::Mythic => 5,
            Self::Divine => 6,
            Self::Transcendent => 7,
        }
    }

    /// The quality at a position in [`Self::ALL`], clamped to the last one.
    pub fn from_index(index: usize) -> Self {
        Self::ALL[index.min(Self::ALL.len() - 1)]
    }

    /// The quality's point multiplier in tenths.
    ///
    /// Visible past this module because the shop prices gear by it: a shop with
    /// its own idea of what a quality is worth would drift from the drops it is
    /// an alternative to. The steps are all at least a fifth: two adjacent
    /// qualities are never "about the same", which is what keeps the colour on
    /// the row honest.
    pub(crate) const fn multiplier(self) -> u32 {
        match self {
            Self::Common => 10,
            Self::Fine => 13,
            Self::Rare => 16,
            Self::Immortal => 20,
            Self::Legendary => 24,
            Self::Mythic => 29,
            Self::Divine => 35,
            Self::Transcendent => 42,
        }
    }

    /// The quality's colour, as `0xRRGGBB`: white, green, blue, purple, orange,
    /// gold, red, crimson. A surface paints the row with it.
    pub const fn color(self) -> u32 {
        match self {
            Self::Common => 0xB9_BD_C7,
            Self::Fine => 0x4C_AF_50,
            Self::Rare => 0x21_96_F3,
            Self::Immortal => 0x9C_27_B0,
            Self::Legendary => 0xFF_98_00,
            Self::Mythic => 0xFF_C1_07,
            Self::Divine => 0xF5_22_2D,
            Self::Transcendent => 0xA8_07_1A,
        }
    }

    /// The catalog key that names this quality.
    pub const fn name_key(self) -> &'static str {
        match self {
            Self::Common => "cultivation.quality.common",
            Self::Fine => "cultivation.quality.fine",
            Self::Rare => "cultivation.quality.rare",
            Self::Immortal => "cultivation.quality.immortal",
            Self::Legendary => "cultivation.quality.legendary",
            Self::Mythic => "cultivation.quality.mythic",
            Self::Divine => "cultivation.quality.divine",
            Self::Transcendent => "cultivation.quality.transcendent",
        }
    }

    /// Whether this quality is a 神装: the two grades that carry an effect.
    pub const fn is_divine(self) -> bool {
        self.index() >= Self::Divine.index()
    }
}

/// The extra attack effect a 神装 carries.
///
/// An effect is a debuff the *other* side of a fight walks in with: it is read
/// when the duel is built, so the number the panel shows and the number the
/// fight uses stay the same number. The two 神装 grades carry the same four
/// effects at different depths — the crimson one bites a third harder.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GearEffect {
    /// 寒霜: the enemy strikes softer and misses more.
    Frost,
    /// 雷亟: the enemy acts slower.
    Thunder,
    /// 碎甲: the enemy's 法抗 is torn open.
    Sunder,
    /// 蛊蚀: the enemy walks in already worn down.
    Weaken,
}

impl GearEffect {
    /// Every effect.
    pub const ALL: [Self; 4] = [Self::Frost, Self::Thunder, Self::Sunder, Self::Weaken];

    /// The effect at a position in [`Self::ALL`], clamped to the last one.
    pub fn from_index(index: usize) -> Self {
        Self::ALL[index.min(Self::ALL.len() - 1)]
    }

    /// The catalog key that names this effect.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Frost => "cultivation.effect.frost",
            Self::Thunder => "cultivation.effect.thunder",
            Self::Sunder => "cultivation.effect.sunder",
            Self::Weaken => "cultivation.effect.weaken",
        }
    }

    /// The effect's short tag, the one that rides in the item's name.
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Frost => "寒霜",
            Self::Thunder => "雷亟",
            Self::Sunder => "碎甲",
            Self::Weaken => "蛊蚀",
        }
    }

    /// How deep the effect bites at a quality, in parts per thousand of the
    /// enemy's own number.
    ///
    /// Public because a surface prints it: the effect's line on the page says
    /// what it does with the very number the duel reads, so a 神装 and a 绝世
    /// of the same effect do not read as the same effect.
    pub const fn depth_permille(self, rarity: Rarity) -> u32 {
        let base = match self {
            Self::Frost => 150,
            Self::Thunder => 250,
            Self::Sunder => 300,
            Self::Weaken => 100,
        };
        if rarity.index() >= Rarity::Transcendent.index() {
            base * 4 / 3
        } else {
            base
        }
    }
}

/// One of the seven combat numbers a piece can add outright.
///
/// Named rather than positional so a slot's line cannot silently swap two
/// numbers: the value a line receives is written where the line is named.
/// 内力 and 潜质 are left out on purpose: they are the two numbers a fight
/// reads least, and a robe that paid 潜质 would be a robe that reads as a
/// 功法.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DerivedLine {
    Attack,
    Health,
    Spell,
    Resist,
    Speed,
    Hit,
    Evade,
}

impl DerivedLine {
    /// Add to this line of a set of combat numbers.
    fn add_to(self, bonus: &mut Derived, amount: u32) {
        let slot = match self {
            Self::Attack => &mut bonus.attack,
            Self::Health => &mut bonus.health,
            Self::Spell => &mut bonus.spell,
            Self::Resist => &mut bonus.resist,
            Self::Speed => &mut bonus.speed,
            Self::Hit => &mut bonus.hit,
            Self::Evade => &mut bonus.evade,
        };
        *slot = slot.saturating_add(amount);
    }
}

/// The named families a piece of gear may belong to.
///
/// A set is the only kinship the naming promises: two 曜日 pieces are one set,
/// and the set's name is the prefix every one of them carries — so a row that
/// reads 曜日剑 and a row that reads 曜日袍 are two pieces of one set, by the
/// name alone. A piece outside a set keeps the old random epithet and belongs
/// to nothing.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GearSet {
    /// 曜日: 攻 and 法, the sun's two edges.
    #[default]
    Sun,
    /// 寒渊: 血 and 抗, the depth that holds.
    Abyss,
    /// 惊雷: 速 and 命, the strike that comes before the sound.
    Storm,
    /// 沧澜: 法 and 抗, the tide that wears a shore.
    Tide,
    /// 磐岳: 血 and 闪, the mountain that does not move.
    Crag,
    /// 疾风: 命 and 闪, the wind that is not there to be hit.
    Gale,
    /// 玄铁: 攻 and 血, the blade that is also a wall.
    Iron,
    /// 幽冥: 法 and 血, the breath that both wounds and holds.
    Nether,
}

impl GearSet {
    /// Every set, in display order.
    pub const ALL: [Self; 8] = [
        Self::Sun,
        Self::Abyss,
        Self::Storm,
        Self::Tide,
        Self::Crag,
        Self::Gale,
        Self::Iron,
        Self::Nether,
    ];

    /// The set at a position in [`Self::ALL`], clamped to the last one.
    pub fn from_index(index: usize) -> Self {
        Self::ALL[index.min(Self::ALL.len() - 1)]
    }

    /// The set's name: the prefix its pieces carry, and the word a surface
    /// prints beside them.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Sun => "曜日",
            Self::Abyss => "寒渊",
            Self::Storm => "惊雷",
            Self::Tide => "沧澜",
            Self::Crag => "磐岳",
            Self::Gale => "疾风",
            Self::Iron => "玄铁",
            Self::Nether => "幽冥",
        }
    }

    /// The combat numbers a set pays, and how much one extra worn piece is
    /// worth per tier in each: `(line, amount)`.
    ///
    /// The amount is per extra piece per tier, so a three-piece 曜日 set at
    /// tier five pays two extras at five tiers of the pair. The lines are the
    /// set's own — a 寒渊 set never pays 攻 because a set that paid everything
    /// would be a set that was one number with eight names.
    const fn lines(self) -> &'static [(DerivedLine, u32)] {
        match self {
            Self::Sun => &[(DerivedLine::Attack, 6), (DerivedLine::Spell, 3)],
            Self::Abyss => &[(DerivedLine::Health, 10), (DerivedLine::Resist, 4)],
            Self::Storm => &[(DerivedLine::Speed, 5), (DerivedLine::Hit, 3)],
            Self::Tide => &[(DerivedLine::Spell, 7), (DerivedLine::Resist, 2)],
            Self::Crag => &[(DerivedLine::Health, 8), (DerivedLine::Evade, 3)],
            Self::Gale => &[(DerivedLine::Hit, 4), (DerivedLine::Evade, 4)],
            Self::Iron => &[(DerivedLine::Attack, 5), (DerivedLine::Health, 5)],
            Self::Nether => &[(DerivedLine::Spell, 6), (DerivedLine::Health, 4)],
        }
    }

    /// The set bonus a set of worn pieces pays, at a tier.
    ///
    /// Two pieces is one extra, three is two: the bonus is what *beyond the
    /// first* the set is worth, so a lone 曜日 blade is not a set and pays
    /// nothing. The tier is the best the set holds — a piece at a high realm
    /// is the same set as a piece at a low one, and it is the high one that
    /// sets how deep the kinship bites.
    fn bonus(self, extra: u32, tier: u8) -> Derived {
        let mut total = Derived::default();
        for (line, amount) in self.lines() {
            line.add_to(&mut total, extra * u32::from(tier.max(1)) * amount);
        }
        total
    }
}

/// The combat numbers the **worn** gear pays as set bonuses.
///
/// A set is read off what is worn, not what is owned: three 曜日 pieces in a
/// satchel are a collection, three on the body are a set. Counts a set twice
/// across slots — a 曜日 blade and a 曜日 robe are two pieces of one set.
pub fn gear_set_bonus(worn: &[EquipmentRecord]) -> Derived {
    let mut total = Derived::default();
    let mut sets: std::collections::BTreeMap<GearSet, Vec<&EquipmentRecord>> =
        std::collections::BTreeMap::new();
    for item in worn {
        if let Some(set) = item.set {
            sets.entry(set).or_default().push(item);
        }
    }
    for (set, pieces) in sets {
        if pieces.len() < 2 {
            continue;
        }
        let extra = u32::try_from(pieces.len() - 1).unwrap_or(u32::MAX);
        let tier = pieces.iter().map(|piece| piece.tier).max().unwrap_or(1);
        total = total.plus(set.bonus(extra, tier));
    }
    total
}

/// One active 装备 set, as a surface reads it: the set, how many of its pieces
/// are worn, and what the wearing pays.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GearSetView {
    pub set: GearSet,
    /// How many pieces of the set are worn. Never one — a lone piece is a
    /// collection, and only two or more are a set.
    pub pieces: u8,
    pub bonus: Derived,
}

/// Every active 装备 set, one entry per set with two or more pieces worn.
///
/// The total of the `bonus` fields is exactly [`gear_set_bonus`] — the same
/// grouping run the same way, so the surface can print "曜日 ×2 攻+30 法+15"
/// per set without recomputing the arithmetic itself.
pub fn gear_set_views(worn: &[EquipmentRecord]) -> Vec<GearSetView> {
    let mut sets: std::collections::BTreeMap<GearSet, Vec<&EquipmentRecord>> =
        std::collections::BTreeMap::new();
    for item in worn {
        if let Some(set) = item.set {
            sets.entry(set).or_default().push(item);
        }
    }
    let mut views = Vec::new();
    for (set, pieces) in sets {
        if pieces.len() < 2 {
            continue;
        }
        let extra = u32::try_from(pieces.len() - 1).unwrap_or(u32::MAX);
        let tier = pieces.iter().map(|piece| piece.tier).max().unwrap_or(1);
        views.push(GearSetView {
            set,
            pieces: u8::try_from(pieces.len()).unwrap_or(u8::MAX),
            bonus: set.bonus(extra, tier),
        });
    }
    views
}

/// One owned piece of gear.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct EquipmentRecord {
    pub id: u64,
    #[serde(default)]
    pub slot: EquipSlot,
    /// The realm it dropped for, one-based: realm one drops tier one.
    pub tier: u8,
    #[serde(default)]
    pub rarity: Rarity,
    #[serde(default)]
    pub attributes: Attributes,
    /// The combat numbers this piece adds outright, before the stars multiply
    /// them. An older save's gear has none and reads as the attributes alone.
    #[serde(default)]
    pub bonus: Derived,
    /// The stars bought with 灵石. Zero is a piece nobody has starred: the
    /// first attempt is what puts the first star on.
    #[serde(default)]
    pub star: u8,
    /// The effect a 神装 carries. `None` on every quality below 神装.
    #[serde(default)]
    pub effect: Option<GearEffect>,
    /// The xianxia-given name the drop carried. Absent on documents written
    /// before names existed, and on items the user has always owned unnamed.
    #[serde(default)]
    pub name: Option<String>,
    /// The set this piece belongs to, if it carries one. `None` is a piece
    /// outside any set, and an old save's gear reads that way.
    #[serde(default)]
    pub set: Option<GearSet>,
}

/// The epithets a blade, robe or boot may carry: one of these prefixes a noun to
/// make a name. There are enough of them that every slot below offers well over
/// five hundred distinct names, so the ladder reads like a cultivation arsenal
/// rather than a table of bonuses.
const EQUIPMENT_PREFIXES: &[&str] = &[
    "玄", "太", "九", "赤", "青", "紫", "苍", "白", "黑", "金", "玉", "寒", "幽", "天", "乾", "坤",
    "混", "无", "破", "斩", "御", "龙", "凤", "麒麟", "朱雀", "玄武", "青龙", "白虎", "昊", "凌",
    "霄", "星", "月", "雷", "风", "云", "霜", "血", "魔", "仙", "灵", "圣", "道", "虚", "焚", "寂",
    "太虚", "混沌", "碧落", "黄泉",
];

/// The nouns each slot draws from. Kept separate so a weapon never reads like a
/// robe and a boot never reads like a blade.
const WEAPON_NOUNS: &[&str] = &[
    "剑", "刀", "枪", "戟", "锤", "斧", "弓", "鞭", "锏", "戈", "矛", "棍", "棒", "钩", "钺", "槊",
    "飞剑", "神剑", "魔刃", "仙剑",
];
const CLOTHES_NOUNS: &[&str] = &[
    "袍", "衣", "甲", "衫", "裳", "袂", "氅", "铠", "道袍", "法衣", "仙衣", "战袍", "圣衣", "云衣",
    "霓裳", "羽衣",
];
const PANTS_NOUNS: &[&str] = &[
    "裤",
    "裙",
    "胫甲",
    "护腿",
    "战袴",
    "灵裤",
    "云裙",
    "玄裤",
    "鳞裤",
    "缚腿",
    "武裤",
    "风行裤",
    "流云裤",
    "踏风裤",
];
const BOOTS_NOUNS: &[&str] = &[
    "靴",
    "履",
    "战靴",
    "飞靴",
    "云履",
    "风靴",
    "灵靴",
    "踏云靴",
    "追风靴",
    "龙鳞靴",
    "虎纹靴",
    "五行靴",
    "流光靴",
    "破空靴",
    "御风靴",
    "星陨靴",
];

/// A xianxia name for one dropped piece of gear.
///
/// The slot picks the noun pool and the roll picks the epithet, so the same
/// prefix and noun can never collide across slots and every slot alone offers
/// hundreds of names. The draw is deterministic in the roll, which keeps a drop
/// reproducible from its seed.
pub(crate) fn equipment_name(slot: EquipSlot, set: Option<GearSet>, rng: &mut Rng) -> String {
    let nouns = match slot {
        EquipSlot::Weapon => WEAPON_NOUNS,
        EquipSlot::Clothes => CLOTHES_NOUNS,
        EquipSlot::Pants => PANTS_NOUNS,
        EquipSlot::Boots => BOOTS_NOUNS,
    };
    // A set's prefix is its name: two pieces of one set share the prefix, so
    // the set is readable off the name. A piece without a set keeps the old
    // random epithet — the set and the epithet are the two ways a name reads.
    let prefix = match set {
        Some(set) => set.name(),
        None => EQUIPMENT_PREFIXES[rng.below(EQUIPMENT_PREFIXES.len() as u64) as usize],
    };
    let noun = nouns[rng.below(nouns.len() as u64) as usize];
    format!("{prefix}{noun}")
}

/// A dropped piece's full display name, with a 神装's effect riding in it.
///
/// The effect is written into the name rather than bolted on beside it: a name
/// is what the event log, the notice and the shop row all print, and an effect
/// that only existed in the detail panel would be invisible in all three.
pub(crate) fn equipment_display_name(
    slot: EquipSlot,
    set: Option<GearSet>,
    effect: Option<GearEffect>,
    rng: &mut Rng,
) -> String {
    let name = equipment_name(slot, set, rng);
    match effect {
        Some(effect) => format!("{name}【{}】", effect.tag()),
        None => name,
    }
}

impl EquipmentRecord {
    /// The attributes this item grants as rolled, before the stars.
    pub const fn attributes(&self) -> Attributes {
        self.attributes
    }

    /// The attributes this item grants with its stars folded in.
    ///
    /// What the fight reads and what the page prints are the same number: the
    /// stars multiply the stored attributes rather than being a second row
    /// beside them.
    pub fn effective_attributes(&self) -> Attributes {
        scale_attributes(self.attributes, self.star_multiplier_permille())
    }

    /// The combat numbers this item adds outright, stars folded in.
    pub fn effective_bonus(&self) -> Derived {
        scale_derived(self.bonus, self.star_multiplier_permille())
    }

    /// The combat power this item alone buys, stars folded in.
    pub fn power(&self) -> u32 {
        self.effective_attributes()
            .power()
            .saturating_add(self.effective_bonus_power())
    }

    /// What the item's outright combat numbers are worth as power.
    ///
    /// The flat lines are read at the same rate the attributes are: a point of
    /// 攻击力 is a point of 攻击力 however it was bought.
    fn effective_bonus_power(&self) -> u32 {
        let bonus = self.effective_bonus();
        bonus
            .attack
            .saturating_add(bonus.health / 4)
            .saturating_add(bonus.spell)
            .saturating_add(bonus.resist / 2)
            .saturating_add(bonus.speed)
            .saturating_add(bonus.hit / 10)
            .saturating_add(bonus.evade / 10)
            .saturating_add(bonus.inner / 8)
    }

    /// How many stars this item holds.
    pub const fn star(&self) -> u8 {
        if self.star > MAX_STAR {
            MAX_STAR
        } else {
            self.star
        }
    }

    /// Whether another star can be attempted.
    pub const fn star_capped(&self) -> bool {
        self.star >= MAX_STAR
    }

    /// What every star bought so far is worth together, in parts per thousand.
    pub fn star_multiplier_permille(&self) -> u32 {
        star_multiplier_permille(self.id, self.star())
    }
}

/// What one star is worth on this item, in parts per thousand: fifty to eighty.
///
/// Counted from the id so the ladder is stable — the same item always stars by
/// the same steps — and spread over the range so two ten-star pieces are not
/// identical.
fn star_step_permille(id: u64, star: u8) -> u32 {
    let mixed = mix(id ^ u64::from(star).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    50 + (mixed % 31) as u32
}

/// What the stars bought so far are worth together, in parts per thousand.
///
/// Compounded rather than flat: the tenth star of a ten-star piece is the
/// tenth multiplication, not a tenth of one, which is what makes the deep stars
/// feel deep.
pub fn star_multiplier_permille(id: u64, star: u8) -> u32 {
    let mut total: i64 = 1_000;
    for step in 1..=star.min(MAX_STAR) {
        total = total * i64::from(1_000 + star_step_permille(id, step)) / 1_000;
    }
    u32::try_from(total).unwrap_or(u32::MAX)
}

/// The chance one attempt at the next star comes off, by the star the piece
/// stands at: ninety percent for the first, thirty-one for the last.
///
/// A table rather than a curve, because the shape is the design: the early
/// stars are a formality, the last three are a decision. Failure drops one
/// star back — one star is never lost, so the floor is the first star rather
/// than nothing.
pub const STAR_SUCCESS_PERMILLE: [u32; MAX_STAR as usize] =
    [900, 850, 790, 730, 670, 610, 540, 470, 390, 310];

/// The chance one attempt at the star after `star` succeeds.
pub const fn star_success_permille(star: u8) -> u32 {
    let index = if star >= MAX_STAR {
        (MAX_STAR - 1) as usize
    } else {
        star as usize
    };
    STAR_SUCCESS_PERMILLE[index]
}

/// Scale a set of attributes by parts per thousand, rounding to nearest.
fn scale_attributes(attributes: Attributes, permille: u32) -> Attributes {
    let scale = |value: i32| -> i32 {
        let product = i64::from(value) * i64::from(permille);
        let rounded = (product + 500) / 1_000;
        i32::try_from(rounded).unwrap_or(i32::MAX)
    };
    Attributes::new(
        scale(attributes.root),
        scale(attributes.strength),
        scale(attributes.mana),
        scale(attributes.agility),
    )
}

/// Scale a set of combat numbers by parts per thousand, rounding to nearest.
fn scale_derived(bonus: Derived, permille: u32) -> Derived {
    let scale = |value: u32| -> u32 {
        let product = u64::from(value) * u64::from(permille);
        u32::try_from((product + 500) / 1_000).unwrap_or(u32::MAX)
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

/// The top of the 成色 band a rolled piece can land in, in parts per thousand.
///
/// Every piece carries a little of its own character: the budget it is built
/// from is nudged up or down by a few percent, so two blades of one tier are two
/// blades rather than one printed twice. The ceiling is published because the
/// guardian's wardrobe has to read its own allowance against the same band — a
/// piece that rolled rich still has to fit inside the share of the body it is
/// allowed to add.
pub(crate) const MASTERWORK_CEILING_PERMILLE: u32 = 1_060;

/// The points a tier's drop is built from before the quality multiplies them.
///
/// Linear in the tier so a later realm's gear is worth more than an earlier
/// realm's best roll, which is what keeps a drop a reward rather than clutter.
pub(crate) fn base_points(tier: u8) -> u32 {
    let tier = u32::from(tier.clamp(1, 10));
    45 + 25 * (tier - 1)
}

/// What one point of an attribute is worth in the power table, in tenths.
fn attribute_weight(attribute: Attribute) -> i64 {
    match attribute {
        Attribute::Root => super::ROOT_WEIGHT,
        Attribute::Strength => super::STRENGTH_WEIGHT,
        Attribute::Mana => super::MANA_WEIGHT,
        Attribute::Agility => super::AGILITY_WEIGHT,
    }
}

/// How many points of one attribute a slice of the power budget buys.
fn points_for(budget: i64, weight: i64) -> i32 {
    i32::try_from((budget + weight / 2) / weight).unwrap_or(i32::MAX)
}

/// How many attribute lines a quality's piece carries.
///
/// A white blade is one number done well; everything from 精良 up carries the
/// slot's second attribute as well. The count climbing with the quality is
/// what keeps a 普通 piece a starting point rather than a watered-down copy of
/// a 绝世 one — and what makes two pieces of the same slot and tier read as
/// different objects rather than as the same object at two sizes.
const fn attribute_lines(rarity: Rarity) -> usize {
    if rarity.index() >= 2 { 2 } else { 1 }
}

/// How many outright combat lines a quality's piece carries.
///
/// The outright numbers — 攻击力, 生命值, 命中 and the rest — are what the
/// deeper qualities are *for*: white and green gear pays attributes alone, a
/// 史诗 or 传说 adds one combat number, and 神话 and up add both. Nothing
/// below 神装 ever carries an effect, whatever else it pays.
const fn combat_lines(rarity: Rarity) -> usize {
    let index = rarity.index();
    (index >= 3) as usize + (index >= 5) as usize
}

/// Roll one piece of gear at a tier, at an already-rolled quality.
///
/// `tier` is the rung the piece stands on and `rarity` is the quality the
/// source's roll landed on — both decided by the caller, because both are read
/// off the strength the piece came from (see [`super::drop`]). The slot, the
/// effect, the attributes and the outright combat numbers are rolled here, so a
/// caller only decides how deep and how good the find is.
pub fn roll_equipment(id: u64, tier: u8, rarity: Rarity, rng: &mut Rng) -> EquipmentRecord {
    roll_equipment_with(id, tier, rarity, roll_set(rng), rng)
}

/// Roll a piece that belongs to no set: the guardian's way in.
///
/// A guardian's gear is trimmed to a fixed share of the realm's power, and a
/// set bonus would be a second, untrimmed source on top of it — the wall would
/// stand a little higher on the days it drew a set. So a guardian's pieces
/// carry none, and the naming falls back to the random epithet.
pub(crate) fn roll_equipment_solo(
    id: u64,
    tier: u8,
    rarity: Rarity,
    rng: &mut Rng,
) -> EquipmentRecord {
    roll_equipment_with(id, tier, rarity, None, rng)
}

fn roll_equipment_with(
    id: u64,
    tier: u8,
    rarity: Rarity,
    set: Option<GearSet>,
    rng: &mut Rng,
) -> EquipmentRecord {
    let slot = EquipSlot::from_index(rng.below(SLOT_COUNT as u64) as usize);
    // 成色: the slot and the quality decide what a piece is worth, not what it
    // reads. Six percent either way on the budget means two blades of one tier
    // are two blades — the shelf used to print the same numbers for every piece
    // of a slot, which made a page of finds read as one find repeated.
    let points = base_points(tier) * rarity.multiplier() / 10;
    let points = (points * (940 + rng.below(121) as u32) / 1_000).max(1);
    let (primary, primary_percent, secondary, secondary_percent) = slot.split();
    // The budget is the slot's nominal split *as power*, not as points: moving a
    // point from 力量 to 身法 keeps the piece worth the same even though it reads
    // a bigger number, so a red 身法 boot is not a quieter red 力量 boot.
    let budget = i64::from(points)
        * (i64::from(primary_percent) * attribute_weight(primary)
            + i64::from(secondary_percent) * attribute_weight(secondary))
        / 100;
    let mut attributes = Attributes::default();
    if attribute_lines(rarity) >= 2 {
        // The split itself sways a little per piece, inside a band that keeps the
        // slot's character: a weapon is still mostly 力量, it just is not always
        // seventy percent of it.
        let sway = 920 + rng.below(161) as i64;
        let primary_share = (i64::from(primary_percent) * sway / 1_000).clamp(300, 900);
        primary.add_to(
            &mut attributes,
            points_for(budget * primary_share / 1_000, attribute_weight(primary)),
        );
        secondary.add_to(
            &mut attributes,
            points_for(
                budget * (1_000 - primary_share) / 1_000,
                attribute_weight(secondary),
            ),
        );
    } else {
        // One line, the whole budget: the piece is what it is, said once.
        primary.add_to(
            &mut attributes,
            points_for(budget, attribute_weight(primary)),
        );
    }
    debug_assert!(secondary_percent + primary_percent == 100);
    // The outright combat lines: a third of the points, split the same way the
    // attributes are — when the quality pays them at all. A white robe soaks
    // nothing outright; a crimson one soaks and strikes both.
    let (combat_primary, combat_primary_percent, combat_secondary, combat_secondary_percent) =
        slot.combat_split();
    let mut bonus = Derived::default();
    match combat_lines(rarity) {
        0 => {}
        1 => combat_primary.add_to(&mut bonus, points / 3),
        _ => {
            let combat_points = points / 3;
            let combat_primary_points = combat_points * combat_primary_percent / 100;
            let combat_secondary_points = combat_points.saturating_sub(combat_primary_points);
            combat_primary.add_to(&mut bonus, combat_primary_points);
            combat_secondary.add_to(&mut bonus, combat_secondary_points);
        }
    }
    debug_assert!(combat_secondary_percent + combat_primary_percent == 100);
    // A 神装 always carries an effect; nothing below one ever does.
    let effect = if rarity.is_divine() {
        Some(GearEffect::from_index(
            rng.below(GearEffect::ALL.len() as u64) as usize,
        ))
    } else {
        None
    };
    EquipmentRecord {
        id,
        slot,
        tier: tier.clamp(1, 10),
        rarity,
        attributes,
        bonus,
        star: 0,
        effect,
        name: Some(equipment_display_name(slot, set, effect, rng)),
        set,
    }
}

/// Whether a fresh drop carries a set, and which one.
///
/// Half the finds belong to nothing — a set that every piece belonged to would
/// be the only kind of gear there is, and a 曜日 that is never not a set is a
/// 曜日 that is not a choice. The set is rolled off the same stream that names
/// the piece, so a drop stays reproducible from its seed.
fn roll_set(rng: &mut Rng) -> Option<GearSet> {
    if rng.below(2) == 0 {
        None
    } else {
        Some(GearSet::from_index(
            rng.below(GearSet::ALL.len() as u64) as usize
        ))
    }
}

/// SplitMix64's finalizer, used to spread an id's stars over the step range.
fn mix(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two pieces of one slot, one tier and one quality are two pieces.
    ///
    /// The budget is spent as power rather than as points, so a piece that leans
    /// on 身法 reads a bigger number than one that leans on 力量 for the same
    /// worth — and the split itself sways, so no two blades print the same pair.
    #[test]
    fn two_pieces_of_one_tier_are_two_pieces() {
        let mut blades = std::collections::BTreeSet::new();
        for id in 1..=400u64 {
            let mut rng = Rng::seeded(id);
            let item = roll_equipment(id, 5, Rarity::Rare, &mut rng);
            if item.slot == EquipSlot::Weapon {
                blades.insert((item.attributes.strength, item.attributes.mana));
            }
        }
        assert!(
            blades.len() > 3,
            "every 精良 blade of one tier reads the same pair: {blades:?}"
        );
    }

    /// The piece's worth stays on its tier whatever it leans on: the budget is
    /// read in power, so a heavier number and a lighter one are the same piece.
    #[test]
    fn a_pieces_worth_is_its_tier_not_its_numbers() {
        let nominal = i64::from(base_points(5) * Rarity::Rare.multiplier() / 10);
        let expected = nominal * (70 * 26 + 30 * 26) / 100;
        for id in 1..=200u64 {
            let mut rng = Rng::seeded(id);
            let item = roll_equipment(id, 5, Rarity::Rare, &mut rng);
            if item.slot != EquipSlot::Weapon {
                continue;
            }
            let weight =
                i64::from(item.attributes.strength) * 26 + i64::from(item.attributes.mana) * 26;
            // The band, plus the rounding each of the two attribute lines costs
            // on its way from power back to whole points.
            let low = expected * 940 / 1_000 - 40;
            let high = expected * 1_060 / 1_000 + 40;
            assert!(
                (low..=high).contains(&weight),
                "a 精良 blade of tier five is worth {weight}, outside {low}..={high}"
            );
        }
    }

    #[test]
    fn a_drop_lands_in_a_slot_and_carries_the_quality_it_was_handed() {
        let mut rng = Rng::seeded(7);
        let item = roll_equipment(1, 3, Rarity::Fine, &mut rng);
        assert_eq!(item.id, 1);
        assert_eq!(item.tier, 3);
        assert_eq!(item.rarity, Rarity::Fine, "the roll does not re-roll it");
        assert!(item.attributes.power() > 0);
        assert!(!item.star_capped());
        assert_eq!(item.star(), 0);
        // What the piece pays beyond its attributes is the quality's to say:
        // see `the_deeper_qualities_pay_more_lines`.
    }

    /// How many of the four attributes a piece pays, as a count.
    fn attribute_lines_of(item: &EquipmentRecord) -> usize {
        [
            item.attributes.root,
            item.attributes.strength,
            item.attributes.mana,
            item.attributes.agility,
        ]
        .into_iter()
        .filter(|value| *value > 0)
        .count()
    }

    /// How many of the nine combat numbers a piece pays outright, as a count.
    fn combat_lines_of(item: &EquipmentRecord) -> usize {
        let bonus = item.bonus;
        [
            bonus.attack,
            bonus.health,
            bonus.inner,
            bonus.spell,
            bonus.resist,
            bonus.speed,
            bonus.hit,
            bonus.evade,
            bonus.aptitude,
        ]
        .into_iter()
        .filter(|value| *value > 0)
        .count()
    }

    /// The ladder the eight qualities are read by: white and green are one
    /// attribute done well, 精良 and up carry the slot's second, the outright
    /// combat numbers arrive at 史诗 and the second one at 神话.
    ///
    /// Every piece of a quality obeys it, so the roll is walked until each
    /// quality has been seen and each bucket is checked whole.
    #[test]
    fn the_deeper_qualities_pay_more_lines() {
        let mut attributes: std::collections::BTreeMap<Rarity, usize> =
            std::collections::BTreeMap::new();
        let mut combat: std::collections::BTreeMap<Rarity, usize> =
            std::collections::BTreeMap::new();
        for seed in 0..8_000_u64 {
            let rarity = Rarity::from_index(seed as usize % Rarity::ALL.len());
            let item = roll_equipment(seed, 5, rarity, &mut Rng::seeded(seed));
            let attribute_lines = attribute_lines_of(&item);
            let combat_lines = combat_lines_of(&item);
            // A quality's pieces never disagree about how many lines they pay.
            assert_eq!(
                *attributes.entry(item.rarity).or_insert(attribute_lines),
                attribute_lines,
                "{:?} pays a varying number of attributes",
                item.rarity
            );
            assert_eq!(
                *combat.entry(item.rarity).or_insert(combat_lines),
                combat_lines,
                "{:?} pays a varying number of outright lines",
                item.rarity
            );
        }
        for rarity in Rarity::ALL {
            let expected_attributes = if rarity.index() >= 2 { 2 } else { 1 };
            let expected_combat =
                usize::from(rarity.index() >= 3) + usize::from(rarity.index() >= 5);
            assert_eq!(
                attributes.get(&rarity).copied(),
                Some(expected_attributes),
                "{rarity:?} pays the wrong number of attributes"
            );
            assert_eq!(
                combat.get(&rarity).copied(),
                Some(expected_combat),
                "{rarity:?} pays the wrong number of outright lines"
            );
        }
    }

    /// The rule the eight qualities exist for: no two adjacent grades are
    /// "about the same". A fifth is the smallest step the table takes.
    #[test]
    fn adjacent_qualities_are_at_least_a_fifth_apart() {
        for pair in Rarity::ALL.windows(2) {
            let [worse, better] = [pair[0], pair[1]];
            assert!(
                u64::from(better.multiplier()) * 100 >= u64::from(worse.multiplier()) * 120,
                "{better:?} is not a clear step above {worse:?}"
            );
        }
    }

    /// The eight qualities travel as themselves through a save, and an older
    /// one's best grade still reads: the ladder grew, the old names stayed.
    #[test]
    fn the_quality_ladder_is_bottom_heavy_and_every_grade_has_a_colour() {
        let mut colours = std::collections::BTreeSet::new();
        for rarity in Rarity::ALL {
            assert!(
                colours.insert(rarity.color()),
                "{rarity:?} repeats a colour"
            );
        }
        assert_eq!(colours.len(), Rarity::ALL.len());
    }

    /// Only a 神装 carries an effect, and every 神装 does.
    #[test]
    fn only_a_divine_piece_carries_an_effect() {
        for seed in 0..4_000_u64 {
            let rarity = Rarity::from_index(seed as usize % Rarity::ALL.len());
            let mut rng = Rng::seeded(seed);
            let item = roll_equipment(seed, 5, rarity, &mut rng);
            assert_eq!(
                item.rarity.is_divine(),
                item.effect.is_some(),
                "{:?} and its effect disagree",
                item.rarity
            );
            // A 神装's effect rides in its name: the log prints the name.
            if let Some(effect) = item.effect {
                let name = item.name.as_deref().unwrap_or_default();
                assert!(
                    name.contains(effect.tag()),
                    "{name} does not name {}",
                    effect.tag()
                );
            }
        }
    }

    #[test]
    fn the_crimson_effect_bites_deeper_than_the_red_one() {
        for effect in GearEffect::ALL {
            assert!(
                effect.depth_permille(Rarity::Transcendent) > effect.depth_permille(Rarity::Divine),
                "{effect:?} does not bite deeper in crimson"
            );
        }
    }

    /// The stars are the second axis: five to eight percent a piece, whatever
    /// the quality did.
    #[test]
    fn every_star_is_worth_five_to_eight_percent() {
        for id in [1_u64, 42, 999, 123_456] {
            for star in 1..=MAX_STAR {
                let step = star_step_permille(id, star);
                assert!(
                    (50..=80).contains(&step),
                    "star {star} of {id} is worth {step}"
                );
            }
        }
    }

    #[test]
    fn ten_stars_are_worth_clearly_more_than_one() {
        for id in [1_u64, 42, 999] {
            let one = star_multiplier_permille(id, 1);
            let ten = star_multiplier_permille(id, 10);
            assert!((1_050..=1_080).contains(&one), "one star is {one}");
            // Five to eight percent a star compounds to between a half again
            // and two and a half times over ten stars.
            assert!(
                ten >= one * 3 / 2 && ten <= one * 5 / 2,
                "ten stars are {ten} over one at {one}"
            );
        }
    }

    /// The gamble's shape, written down before it is run: the early stars are a
    /// formality and the last one is not.
    #[test]
    fn the_last_star_is_the_hardest_and_the_first_is_a_formality() {
        assert!(
            star_success_permille(0) >= 850,
            "the first star is a coin toss"
        );
        assert_eq!(star_success_permille(MAX_STAR - 1), 310);
        for star in 0..MAX_STAR - 1 {
            assert!(
                star_success_permille(star) > star_success_permille(star + 1),
                "star {star} is not harder than the one before it"
            );
        }
    }

    #[test]
    fn every_slot_splits_its_points_over_two_attributes() {
        for slot in EquipSlot::ALL {
            let (primary, primary_percent, secondary, secondary_percent) = slot.split();
            assert_ne!(primary, secondary);
            assert_eq!(primary_percent + secondary_percent, 100);
            let (
                combat_primary,
                combat_primary_percent,
                combat_secondary,
                combat_secondary_percent,
            ) = slot.combat_split();
            assert_ne!(combat_primary, combat_secondary);
            assert_eq!(combat_primary_percent + combat_secondary_percent, 100);
        }
    }
}
