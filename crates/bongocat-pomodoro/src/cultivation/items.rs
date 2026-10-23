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

    /// How often this quality appears from one source, in parts per thousand.
    ///
    /// Every source's row adds up to a thousand, and the rows are not the same
    /// row: a shelf is where good gear is *bought*, so it leans richer than the
    /// road does, and the day's free gift leans poorest of the three — a gift
    /// that paid like a shop would make the shop pointless.
    pub(crate) const fn drop_permille(self, source: DropSource) -> u32 {
        use DropSource::{Combat, Random, Shop};
        match source {
            Random => match self {
                Self::Common => 620,
                Self::Fine => 220,
                Self::Rare => 95,
                Self::Immortal => 40,
                Self::Legendary => 17,
                Self::Mythic => 6,
                Self::Divine => 1,
                Self::Transcendent => 1,
            },
            Combat => match self {
                Self::Common => 540,
                Self::Fine => 250,
                Self::Rare => 125,
                Self::Immortal => 55,
                Self::Legendary => 22,
                Self::Mythic => 6,
                Self::Divine => 1,
                Self::Transcendent => 1,
            },
            Shop => match self {
                Self::Common => 399,
                Self::Fine => 250,
                Self::Rare => 180,
                Self::Immortal => 95,
                Self::Legendary => 50,
                Self::Mythic => 20,
                Self::Divine => 4,
                Self::Transcendent => 2,
            },
        }
    }
}

/// Which channel a piece of gear came out of.
///
/// Three channels, three tables: the day's own gift, the fights, and the shelf.
/// Buying reads the shelf's table — a purchase takes what the refresh rolled —
/// so the four ways a piece arrives are covered by these three rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DropSource {
    /// The day's free event: a gift, or the quiet day's trinket.
    Random,
    /// A fight that was won: a wanderer, a guardian, a trial floor.
    Combat,
    /// The shop's daily refresh, and therefore every purchase off it.
    Shop,
}

/// Roll one quality for a source.
pub(crate) fn roll_rarity(rng: &mut Rng, source: DropSource) -> Rarity {
    let roll = rng.below(1_000);
    let mut acc = 0;
    for rarity in Rarity::ALL {
        acc += rarity.drop_permille(source);
        if roll < u64::from(acc) {
            return rarity;
        }
    }
    Rarity::Common
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
pub(crate) fn equipment_name(slot: EquipSlot, rng: &mut Rng) -> String {
    let nouns = match slot {
        EquipSlot::Weapon => WEAPON_NOUNS,
        EquipSlot::Clothes => CLOTHES_NOUNS,
        EquipSlot::Pants => PANTS_NOUNS,
        EquipSlot::Boots => BOOTS_NOUNS,
    };
    let prefix = EQUIPMENT_PREFIXES[rng.below(EQUIPMENT_PREFIXES.len() as u64) as usize];
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
    effect: Option<GearEffect>,
    rng: &mut Rng,
) -> String {
    let name = equipment_name(slot, rng);
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

/// The points a tier's drop is built from before the quality multiplies them.
///
/// Linear in the tier so a later realm's gear is worth more than an earlier
/// realm's best roll, which is what keeps a drop a reward rather than clutter.
pub(crate) fn base_points(tier: u8) -> u32 {
    let tier = u32::from(tier.clamp(1, 10));
    45 + 25 * (tier - 1)
}

/// Roll one piece of gear for a realm, out of a channel.
///
/// `tier` is the realm's one-based position and `source` is the channel the
/// piece came out of, which is what decides how good the quality roll leans.
/// The slot, the quality, the effect, the attributes and the outright combat
/// numbers are all rolled here, so a caller only decides which realm the drop
/// belongs to and where it came from.
pub fn roll_equipment(id: u64, tier: u8, source: DropSource, rng: &mut Rng) -> EquipmentRecord {
    let slot = EquipSlot::from_index(rng.below(SLOT_COUNT as u64) as usize);
    let rarity = roll_rarity(rng, source);
    let points = base_points(tier) * rarity.multiplier() / 10;
    let (primary, primary_percent, secondary, secondary_percent) = slot.split();
    let primary_points = points * primary_percent / 100;
    let secondary_points = points.saturating_sub(primary_points);
    let mut attributes = Attributes::default();
    primary.add_to(
        &mut attributes,
        i32::try_from(primary_points).unwrap_or(i32::MAX),
    );
    secondary.add_to(
        &mut attributes,
        i32::try_from(secondary_points).unwrap_or(i32::MAX),
    );
    debug_assert!(secondary_percent + primary_percent == 100);
    // The outright combat lines: a third of the points, split the same way the
    // attributes are, so a piece reads the same shape in a fight as it does on
    // the attribute panel.
    let (combat_primary, combat_primary_percent, combat_secondary, combat_secondary_percent) =
        slot.combat_split();
    let combat_points = points / 3;
    let combat_primary_points = combat_points * combat_primary_percent / 100;
    let combat_secondary_points = combat_points.saturating_sub(combat_primary_points);
    let mut bonus = Derived::default();
    combat_primary.add_to(&mut bonus, combat_primary_points);
    combat_secondary.add_to(&mut bonus, combat_secondary_points);
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
        name: Some(equipment_display_name(slot, effect, rng)),
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

    #[test]
    fn a_drop_lands_in_a_slot_and_carries_a_quality() {
        let mut rng = Rng::seeded(7);
        let item = roll_equipment(1, 3, DropSource::Combat, &mut rng);
        assert_eq!(item.id, 1);
        assert_eq!(item.tier, 3);
        assert!(item.attributes.power() > 0);
        assert!(!item.star_capped());
        assert_eq!(item.star(), 0);
        // Every piece adds its attributes and its outright combat numbers both.
        assert!(item.bonus.attack + item.bonus.health + item.bonus.speed > 0);
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

    /// Every channel's row is a whole row, and the three channels are not the
    /// same row: the shelf leans richer than the road, the day's gift poorest.
    #[test]
    fn every_channel_rolls_a_whole_thousand_and_the_channels_differ() {
        for source in [DropSource::Random, DropSource::Combat, DropSource::Shop] {
            let total: u32 = Rarity::ALL
                .iter()
                .map(|rarity| rarity.drop_permille(source))
                .sum();
            assert_eq!(total, 1_000, "{source:?} does not roll a whole thousand");
        }
        // The shop is where good gear is bought, so it beats the road on the
        // two 神装 grades and on 传说.
        for rarity in [
            Rarity::Legendary,
            Rarity::Mythic,
            Rarity::Divine,
            Rarity::Transcendent,
        ] {
            assert!(
                rarity.drop_permille(DropSource::Shop) > rarity.drop_permille(DropSource::Combat),
                "{rarity:?} is not commoner on the shelf than on the road"
            );
            assert!(
                rarity.drop_permille(DropSource::Combat)
                    >= rarity.drop_permille(DropSource::Random),
                "{rarity:?} is not commoner in a fight than in a gift"
            );
        }
        assert!(
            Rarity::Common.drop_permille(DropSource::Random)
                > Rarity::Common.drop_permille(DropSource::Combat)
                && Rarity::Common.drop_permille(DropSource::Combat)
                    > Rarity::Common.drop_permille(DropSource::Shop),
            "white gear has to be the road's usual and the shelf's floor"
        );
    }

    #[test]
    fn every_quality_is_reachable_from_every_channel() {
        for source in [DropSource::Random, DropSource::Combat, DropSource::Shop] {
            let mut seen = std::collections::BTreeSet::new();
            for seed in 0..2_000_u64 {
                seen.insert(roll_rarity(&mut Rng::seeded(seed), source));
            }
            assert_eq!(seen.len(), Rarity::ALL.len(), "{source:?} lost a quality");
        }
    }

    /// Only a 神装 carries an effect, and every 神装 does.
    #[test]
    fn only_a_divine_piece_carries_an_effect() {
        let mut divine_without = 0;
        let mut mortal_with = 0;
        for seed in 0..4_000_u64 {
            let mut rng = Rng::seeded(seed);
            let item = roll_equipment(seed, 5, DropSource::Combat, &mut rng);
            match (item.rarity.is_divine(), item.effect) {
                (true, None) => divine_without += 1,
                (false, Some(_)) => mortal_with += 1,
                _ => {}
            }
        }
        assert_eq!(divine_without, 0, "a 神装 without its effect");
        assert_eq!(mortal_with, 0, "an effect below 神装");
        // A 神装's effect rides in its name: the log prints the name.
        for seed in 0..4_000_u64 {
            let mut rng = Rng::seeded(seed);
            let item = roll_equipment(seed, 5, DropSource::Shop, &mut rng);
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
