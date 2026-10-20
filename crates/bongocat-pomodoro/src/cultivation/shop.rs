//! 灵石: one currency, and the shop that spends it.
//!
//! The ladder pays growth, which only ever moves the cultivator up one ladder.
//! That is the right shape for a system the user advances by sitting down, and
//! the wrong shape for everything else a 修仙 page wants to offer: gear they will
//! not find for another two realms, a 功法 they have not been lucky enough to
//! meet, a herb whose tier is out of reach today. So the drops keep being drops
//! and a currency is added beside them: a drop you cannot use is worth something,
//! and a shop is where "something" becomes the thing you actually wanted.
//!
//! Three rules hold the design together, and each one exists because breaking it
//! breaks the ladder rather than the shop:
//!
//! 1. **Selling never manufactures growth.** A sold item is gone; the growth it
//!    would have paid is not banked anywhere. So the loop is "play to find, sell
//!    what you cannot use, buy what you can" — a user who plays no pomodoros has
//!    nothing to sell, which is the correct answer.
//! 2. **The shop stocks what the ladder can use.** Every offer is rolled for the
//!    realm the buyer stands in, so a 金丹 never sees 炼气 cloth at a price that
//!    makes a 炼气 drop worthless.
//! 3. **Selling is at a discount to buying.** A shop that bought at the asking
//!    price would be a money printer: buy the 功法 nobody has, sell it straight
//!    back. `SELL_NUMERATOR / SELL_DENOMINATOR` is where that is closed.
//!
//! The stock is a pure function of the day and the realm, so a save cannot carry
//! a shop it rolled by hand and the same day always offers the same goods. What
//! has been bought is stored instead, which is the only state the shop really has.

use serde::{Deserialize, Serialize};

use super::items::base_points;
use super::{ART_COUNT, Attribute, EquipmentRecord, Rarity, Realm};

/// How many goods the shop offers at most in one day.
///
/// Five is the floor a shop needs to look like a shop rather than a slot machine:
/// below it, a realm that can only use one kind of gear leaves the rest of the
/// page dead. Ten is the ceiling — past it the day stops being a choice and
/// becomes a reading list.
pub const SHOP_MAX_OFFERS: usize = 10;
/// The fewest the shop may offer on a quiet day.
pub const SHOP_MIN_OFFERS: usize = 5;

/// What the shop buys an item for, in tenths of the asking price.
///
/// Half. The seller's price has to be visibly the shop's price rather than the
/// buyer's, and a half is the one ratio a user can check in their head — a user
/// who cannot tell at a glance what selling costs will not sell. It is also what
/// closes the resale loop: buying at the asking price and selling straight back
/// returns half of it, every time, so the shop can never be a money printer.
pub const SELL_NUMERATOR: u32 = 5;
pub const SELL_DENOMINATOR: u32 = 10;

/// One thing the shop is offering today.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ShopOffer {
    /// A piece of gear for a realm, rolled the same way a drop would be.
    Equipment(EquipmentRecord),
    /// One herb.
    Seed {
        attribute: Attribute,
        tier: u8,
        name: Option<String>,
    },
    /// One 功法, offered by id out of the catalog.
    Art { art_id: u64 },
}

impl ShopOffer {
    /// What one session's worth of 灵石 this offer costs.
    ///
    /// Priced off the same numbers that roll the item rather than off a separate
    /// table: a tier of gear is worth what its points are worth, so the shop
    /// cannot drift away from the drops it is meant to be an alternative to.
    pub fn price(&self) -> u32 {
        match self {
            Self::Equipment(item) => equipment_price(item.tier, item.rarity),
            Self::Seed { tier, .. } => seed_price(*tier),
            Self::Art { art_id } => art_price(super::blueprint(*art_id).grade),
        }
    }

    /// What the shop pays for it.
    ///
    /// Strictly below [`Self::price`] by [`SELL_NUMERATOR`] over
    /// [`SELL_DENOMINATOR`], rounded down: a seller's price that rounds up to the
    /// buyer's would make the margin a rounding accident.
    pub fn sell_price(&self) -> u32 {
        (u64::from(self.price()) * u64::from(SELL_NUMERATOR) / u64::from(SELL_DENOMINATOR)) as u32
    }

    /// The catalog key naming what kind of thing this is.
    pub fn kind_key(&self) -> &'static str {
        match self {
            Self::Equipment(_) => "cultivation.shop.equipment",
            Self::Seed { .. } => "cultivation.shop.seed",
            Self::Art { .. } => "cultivation.shop.art",
        }
    }
}

/// What a piece of gear costs.
///
/// Priced off the tier's own points and the grade's multiplier, and off nothing
/// else: two shops selling the same tier of the same grade at different prices
/// would be a shop with a bug in it, and the points are already the number that
/// says what a piece is worth.
///
/// The curve is quadratic in the tier's points, and steep enough that adjacent
/// tiers stay distinguishable — at a flatter curve a 二阶 and a 三阶 piece cost
/// almost the same, which makes the ladder's own steps invisible in the shop.
/// A grade multiplies that price by its own factor rather than squaring it, so
/// an 极品 piece is a luxury and not a second ladder bolted onto the first.
pub fn equipment_price(tier: u8, rarity: Rarity) -> u32 {
    let points = base_points(tier);
    let base = points * points / 60 + 40;
    base * rarity.multiplier() / 10
}

/// What one herb costs.
///
/// Linear in the tier, and under a garment of the same tier at every step: a
/// herb is repeatable and a garment is not, so the same 灵石 should not buy the
/// same amount of either.
pub fn seed_price(tier: u8) -> u32 {
    let tier = u32::from(tier.clamp(1, 10));
    30 * tier + 20
}

/// How much dearer than its own tier of gear a 功法 is, in halves.
///
/// Two and a half times: enough that the shelf still says a 功法 is the bigger
/// buy — it is permanent, and every 阶 of it pays for the life of the save —
/// without making it a thing only a lottery win can reach.
const ART_OVER_GEAR_NUMERATOR: u32 = 5;
const ART_OVER_GEAR_DENOMINATOR: u32 = 2;

/// What one 功法 costs.
///
/// Priced off the garment it competes with, not off a table of its own: a grade
/// of 功法 is stocked beside the gear of the realm that grade belongs to, and it
/// costs [`ART_OVER_GEAR_NUMERATOR`] halves of that gear. The shelf has to read
/// as one market — an art priced at twenty times the garment next to it is not
/// an alternative the user weighs, it is a lottery ticket on the same page.
///
/// It stays above the gear and the herbs because it is worth more: a garment is
/// one of four slots and a herb is one planting, while a 功法 is permanent.
pub fn art_price(grade: super::ArtGrade) -> u32 {
    // The grade's own band: 黄 sits with the second tier, 神 with the tenth, so
    // a 功法 is compared against the gear a cultivator of its grade would wear.
    let tier = u8::try_from((grade.index() + 1) * 2).unwrap_or(10).min(10);
    equipment_price(tier, Rarity::Common) * ART_OVER_GEAR_NUMERATOR / ART_OVER_GEAR_DENOMINATOR
}

/// What one finished session pays into the purse.
///
/// Small on purpose. The currency is a way to sell what a walk turned up, not a
/// second growth ladder: a session that pays both growth and 灵石 would let a
/// user sit down for 灵石 and then use the 灵石 to buy the growth they skipped.
pub const SESSION_EARNINGS: u32 = 12;

/// How many offers a realm's shop rolls, from the day alone.
///
/// Five to ten, decided by the day's own number rather than a counter: a save
/// cannot ask for a better shop by being reloaded, and two users on the same day
/// in the same realm see the same shelf.
pub fn offers_for_day(day_number: i64, realm: Realm) -> usize {
    let seed = u64::from(day_number as u32).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ realm_seed(realm);
    let span = (SHOP_MAX_OFFERS - SHOP_MIN_OFFERS + 1) as u64;
    SHOP_MIN_OFFERS + (mix(seed) % span) as usize
}

/// Roll one realm's shelf for a day.
///
/// Deterministic in `(day_number, realm)`: the shop is regenerated from those
/// two numbers rather than stored, so a document cannot carry a shelf that never
/// would have rolled, and a document saved on Tuesday shows Tuesday's shelf when
/// it is opened on Wednesday with the day already advanced past it.
pub fn roll_offers(day_number: i64, realm: Realm, seed: u64) -> Vec<ShopOffer> {
    let count = offers_for_day(day_number, realm);
    let tier = u8::try_from(realm.index()).unwrap_or(0).saturating_add(1);
    let mut rng = super::Rng::seeded(seed ^ realm_seed(realm));
    let mut offers = Vec::with_capacity(count);
    for slot in 0..count {
        // The shelf is mostly what this realm can use, with one slot given to
        // something better or worse: a shelf of five identical tiers is not a
        // choice either, and a 神级 garment showing up in 炼气 would be a gift
        // rather than a market.
        let kind = match slot % 4 {
            0 => 1u8, // gear
            1 => 0,   // herb
            2 => 2,   // 功法
            _ => {
                if rng.below(4) == 0 {
                    1
                } else {
                    0
                }
            }
        };
        let offer = match kind {
            0 => ShopOffer::Seed {
                attribute: Attribute::from_index(rng.below(Attribute::ALL.len() as u64) as usize),
                // One tier either side of the realm's own: a herb a tier down is a
                // reachable one for the garden, one a tier up is a goal.
                tier: tier
                    .saturating_add(1)
                    .saturating_sub(u8::try_from(rng.below(3)).unwrap_or(0))
                    .clamp(1, 10),
                name: None,
            },
            1 => {
                let mut item = super::items::roll_equipment(
                    0,
                    tier.saturating_add(u8::try_from(rng.below(2)).unwrap_or(0))
                        .min(10),
                    &mut rng,
                );
                item.name = Some(super::items::equipment_name(item.slot, &mut rng));
                ShopOffer::Equipment(item)
            }
            _ => ShopOffer::Art {
                art_id: shop_art_id(&mut rng, tier),
            },
        };
        offers.push(offer);
    }
    offers
}

/// Which 功法 the shop offers, given the realm it is stocking for.
///
/// Weighted toward the realm's own grade band, exactly where [`ArtGrade`] is
/// graded for breakthroughs, and never the rarest band: 神级 belongs to walks
/// that get lucky, not to a shelf.
fn shop_art_id(rng: &mut super::Rng, tier: u8) -> u64 {
    let grade = match tier {
        0 | 1 => 0,
        2 | 3 => 1,
        4 | 5 => 2,
        _ => 3,
    };
    let band: Vec<u64> = (1..=ART_COUNT)
        .filter(|id| super::blueprint(*id).grade.index() == grade)
        .collect();
    if band.is_empty() {
        1
    } else {
        band[rng.below(band.len() as u64) as usize]
    }
}

/// A realm's contribution to the day's seed, so two realms never share a shelf.
fn realm_seed(realm: Realm) -> u64 {
    0x517C_C1B7_2722_0A95u64.wrapping_mul(realm.index() as u64 + 1)
}

/// SplitMix64's finalizer, used only to spread a day's number over the range.
fn mix(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

/// A day's shelf as a surface reads it: the offer, what it costs, and whether it
/// is still on the shelf.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShopOfferView {
    pub offer: ShopOffer,
    pub price: u32,
    /// The shop's own price for it, so a surface can print "卖 N" without
    /// re-deriving the margin.
    pub sell_price: u32,
    pub bought: bool,
    /// The slot this offer sits in, which is what a buy command names.
    pub slot: usize,
}
