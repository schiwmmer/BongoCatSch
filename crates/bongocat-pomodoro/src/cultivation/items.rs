//! Gear: the four slots, the four grades, and the one roll that fills them.
//!
//! An item stores the attributes it grants rather than the numbers it was
//! rolled from. Retuning the table therefore changes future drops only, which is
//! the same rule the configuration's field evolution follows: what a user
//! already owns keeps working, and nothing silently re-scores it.

use serde::{Deserialize, Serialize};

use super::{Attribute, Attributes, Rng};

/// How many equipment slots a cultivator has. One item per slot at a time.
pub const SLOT_COUNT: usize = 4;

/// Where a piece of gear is worn.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EquipSlot {
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

    /// How this slot splits its points over two attributes, in percent.
    ///
    /// A weapon is offence, the robe and the trousers are poise, and the boots
    /// are footwork with a little force behind it. The split is what makes the
    /// four slots read differently instead of being four copies of one bonus.
    const fn split(self) -> (Attribute, u32, Attribute, u32) {
        match self {
            Self::Weapon => (Attribute::Strength, 70, Attribute::Spirit, 30),
            Self::Clothes => (Attribute::Root, 75, Attribute::Agility, 25),
            Self::Pants => (Attribute::Agility, 55, Attribute::Root, 45),
            Self::Boots => (Attribute::Agility, 60, Attribute::Strength, 40),
        }
    }
}

/// How good a drop is. The grades are also the star count a surface shows.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Rarity {
    Common,
    Fine,
    Rare,
    Immortal,
}

impl Rarity {
    /// Every grade, worst to best.
    pub const ALL: [Self; 4] = [Self::Common, Self::Fine, Self::Rare, Self::Immortal];

    /// The grade's point multiplier in tenths.
    const fn multiplier(self) -> u32 {
        match self {
            Self::Common => 10,
            Self::Fine => 16,
            Self::Rare => 24,
            Self::Immortal => 38,
        }
    }

    /// How many stars a surface draws for this grade, one to four.
    pub const fn stars(self) -> usize {
        match self {
            Self::Common => 1,
            Self::Fine => 2,
            Self::Rare => 3,
            Self::Immortal => 4,
        }
    }

    /// Roll a grade: common most of the time, immortal once in a hundred drops.
    fn roll(rng: &mut Rng) -> Self {
        match rng.below(1_000) {
            0..700 => Self::Common,
            700..920 => Self::Fine,
            920..990 => Self::Rare,
            _ => Self::Immortal,
        }
    }
}

/// One owned piece of gear.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EquipmentRecord {
    pub id: u64,
    pub slot: EquipSlot,
    /// The realm it dropped for, one-based: realm one drops tier one.
    pub tier: u8,
    pub rarity: Rarity,
    #[serde(default)]
    pub attributes: Attributes,
}

impl EquipmentRecord {
    /// The attributes this item grants.
    pub const fn attributes(self) -> Attributes {
        self.attributes
    }

    /// The combat power this item alone buys.
    pub fn power(self) -> u32 {
        self.attributes.power()
    }

    /// How many stars the item shows.
    pub const fn stars(self) -> usize {
        self.rarity.stars()
    }
}

/// The points a tier's drop is built from before the grade multiplies them.
///
/// Linear in the tier so a later realm's gear is worth more than an earlier
/// realm's best roll, which is what keeps a drop a reward rather than clutter.
fn base_points(tier: u8) -> u32 {
    let tier = u32::from(tier.clamp(1, 10));
    45 + 25 * (tier - 1)
}

/// Roll one piece of gear for a realm.
///
/// `tier` is the realm's one-based position. The slot and the grade are rolled
/// here, so a caller only decides which realm the drop belongs to.
pub fn roll_equipment(id: u64, tier: u8, rng: &mut Rng) -> EquipmentRecord {
    let slot = EquipSlot::from_index(rng.below(SLOT_COUNT as u64) as usize);
    let rarity = Rarity::roll(rng);
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
    EquipmentRecord {
        id,
        slot,
        tier: tier.clamp(1, 10),
        rarity,
        attributes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drop_lands_in_a_slot_and_carries_a_grade() {
        let mut rng = Rng::seeded(7);
        let item = roll_equipment(1, 3, &mut rng);
        assert_eq!(item.id, 1);
        assert_eq!(item.tier, 3);
        assert!(item.attributes.power() > 0);
        assert!((1..=4).contains(&item.stars()));
    }

    #[test]
    fn a_better_grade_is_worth_more_than_a_worse_one() {
        let points = |rarity: Rarity| {
            let points = base_points(1) * rarity.multiplier() / 10;
            assert!(points > 0);
            points
        };
        assert!(points(Rarity::Common) < points(Rarity::Fine));
        assert!(points(Rarity::Fine) < points(Rarity::Rare));
        assert!(points(Rarity::Rare) < points(Rarity::Immortal));
    }

    #[test]
    fn every_slot_splits_its_points_over_two_attributes() {
        for slot in EquipSlot::ALL {
            let (primary, primary_percent, secondary, secondary_percent) = slot.split();
            assert_ne!(primary, secondary);
            assert_eq!(primary_percent + secondary_percent, 100);
        }
    }
}
