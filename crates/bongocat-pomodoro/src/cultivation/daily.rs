//! 晨间: the one thing a day hands the cultivator before anything else.
//!
//! A ladder that only moves when the user remembers to sit down is a chore; the
//! daily event is the ladder noticing the user instead. Once per local day — on
//! the first tick after launch, or at midnight on a machine left running — one
//! thing happens: someone attacks on the road, something is found, or the day
//! passes quietly with a coin or two in the dust.
//!
//! The split is deliberate and it is the whole design: **an ambush is more than
//! twice as likely as a gift**, because a gift is the one reward in the ladder
//! that costs nothing at all. A direct reward that showed up as often as a fight
//! would make fighting a hobby, and the daily event a slot machine.

use serde::{Deserialize, Serialize};

use super::Rng;
use super::explore::{StonePurse, WANDERER_STONES};

/// How often the day opens with someone attacking, in parts per thousand.
pub const DAILY_AMBUSH_PERMILLE: u64 = 280;

/// How often the day simply hands something over, in parts per thousand.
///
/// Under [`DAILY_AMBUSH_PERMILLE`] on purpose: see the module note. A gift is
/// the rarest thing in the ladder's calendar and the only one paid without a
/// fight or a session behind it.
pub const DAILY_GIFT_PERMILLE: u64 = 120;

/// The purse a quiet day leaves: a coin or two, enough that the line on the
/// page is worth reading and not enough to be a faucet.
pub(crate) const DAILY_TRINKET_STONES: StonePurse = StonePurse { min: 3, max: 6 };

/// The purse a gift event pays when it pays money at all.
pub(crate) const DAILY_GIFT_STONES: StonePurse = StonePurse { min: 25, max: 50 };

/// What an ambush pays when it is beaten: the same purse a wanderer met on a
/// walk carries, because it is the same kind of encounter.
pub(crate) const DAILY_AMBUSH_STONES: StonePurse = WANDERER_STONES;

/// What a gift hands over, when the day picks a gift at all.
///
/// Money first, then a herb, then gear: a gift is the ladder's softest payout,
/// so it leans on the thing the shop also takes rather than on the two things a
/// fight is for.
const GIFT_STONES_PERMILLE: u64 = 500;
const GIFT_SEED_PERMILLE: u64 = 350;
const GIFT_GEAR_PERMILLE: u64 = 150;

/// A gift always hands something over: the three shares fill the roll exactly.
const _: () = assert!(GIFT_STONES_PERMILLE + GIFT_SEED_PERMILLE + GIFT_GEAR_PERMILLE == 1_000);

/// What the day opened with.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum DailyKind {
    /// Someone attacked on the road, and the fight is on the record.
    Ambush,
    /// The day handed something over.
    Gift,
    /// Nothing happened but a coin or two in the dust.
    Trinket,
}

impl DailyKind {
    /// The catalog key naming how the day opened.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Ambush => "cultivation.daily.ambush",
            Self::Gift => "cultivation.daily.gift",
            Self::Trinket => "cultivation.daily.trinket",
        }
    }
}

/// What a gift event hands over.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GiftKind {
    Stones,
    Seed,
    Gear,
}

/// Which of the three the day is.
///
/// One roll answers it: an ambush, a gift, or a quiet day. The rest of the
/// thousand — six hundred — is the quiet day, which is what keeps the other two
/// feeling like something.
pub(crate) fn roll_kind(rng: &mut Rng) -> DailyKind {
    let roll = rng.below(1_000);
    if roll < DAILY_AMBUSH_PERMILLE {
        return DailyKind::Ambush;
    }
    if roll < DAILY_AMBUSH_PERMILLE + DAILY_GIFT_PERMILLE {
        return DailyKind::Gift;
    }
    DailyKind::Trinket
}

/// What a gift hands over.
pub(crate) fn roll_gift(rng: &mut Rng) -> GiftKind {
    let roll = rng.below(1_000);
    if roll < GIFT_STONES_PERMILLE {
        return GiftKind::Stones;
    }
    if roll < GIFT_STONES_PERMILLE + GIFT_SEED_PERMILLE {
        return GiftKind::Seed;
    }
    GiftKind::Gear
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rule the module exists for: a gift is rarer than an ambush.
    const _: () = assert!(DAILY_GIFT_PERMILLE < DAILY_AMBUSH_PERMILLE);

    #[test]
    fn a_day_opens_on_every_kind_sooner_or_later() {
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..1_000_u64 {
            seen.insert(roll_kind(&mut Rng::seeded(seed)));
        }
        for kind in [DailyKind::Ambush, DailyKind::Gift, DailyKind::Trinket] {
            assert!(seen.contains(&kind), "{kind:?} never came up");
        }
        let counts = |kind| {
            (0..5_000_u64)
                .filter(|seed| roll_kind(&mut Rng::seeded(*seed)) == kind)
                .count()
        };
        assert!(
            counts(DailyKind::Ambush) > counts(DailyKind::Gift),
            "an ambush has to be the likelier of the two"
        );
    }

    #[test]
    fn a_gift_can_be_money_or_a_herb_or_gear() {
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..1_000_u64 {
            seen.insert(match roll_gift(&mut Rng::seeded(seed)) {
                GiftKind::Stones => 0,
                GiftKind::Seed => 1,
                GiftKind::Gear => 2,
            });
        }
        assert_eq!(seen.len(), 3, "every kind of gift has to be reachable");
    }
}
