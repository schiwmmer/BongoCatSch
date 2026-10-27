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
use super::{
    ART_COUNT, AptitudeHerb, ArtGrade, Attribute, EquipmentRecord, Rarity, Realm, SeedGrade,
};
/// How many goods the shop offers at most in one day.
///
/// Five is the floor a shop needs to look like a shop rather than a slot
/// machine: below it, a realm that can only use one kind of gear leaves the rest
/// of the page dead. Ten is the ceiling — the ten a shelf rolls — because past
/// it the day stops being a choice and becomes a reading list. The 潜质 herbs
/// share the seed rows rather than adding rows of their own, so the shelf's
/// width is the base stock's width and nothing else. The bought-slot record is
/// cut to this number, so it has to be the true maximum or a purchase past the
/// cut could be made twice.
pub const SHOP_MAX_OFFERS: usize = SHOP_BASE_MAX_OFFERS;
/// The fewest the shop may offer on a quiet day.
pub const SHOP_MIN_OFFERS: usize = 5;
/// The most rows a day's shelf rolls — gear, herbs and 功法.
const SHOP_BASE_MAX_OFFERS: usize = 10;

/// How often a day's shelf carries a 洗髓丹: one day in two hundred.
///
/// A *day* chance rather than a slot chance: a pill is one event, and rolling it
/// per slot would quietly multiply the odds by however many rows the shelf
/// happens to have. One in two hundred is roughly one sighting every six or
/// seven months of daily shopping — rare enough that finding one is news, common
/// enough that a build saving for a gate can plan on seeing it. It is the last
/// of the 潜质 items still rolled on its own: 悟心草 and 慧心草 are ordinary
/// shelf stock now, and only the pill is an event.
pub const MARROW_SHELF_ONE_IN: u64 = 200;

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
        /// The herb's grade. A shelf keeps the grade it rolled, so the row a
        /// buyer reads is the herb that lands in their satchel.
        grade: SeedGrade,
        name: Option<String>,
    },
    /// One 功法, offered by id out of the catalog.
    Art { art_id: u64 },
    /// One 潜质 item: 悟心草、慧心草 or 洗髓丹.
    ///
    /// Kept apart from [`Self::Seed`] because the two are not the same kind of
    /// thing: a seed is planted and pays an attribute when it comes up, while
    /// these are consumed on the spot and move the one number 功法 learning
    /// turns on. The two herbs carry a grade like any seed — a 神级 慧心草 pays
    /// two and a half times the 地阶 one — so the row names the grade the buyer
    /// is paying for; the pill carries none.
    Herb {
        herb: AptitudeHerb,
        /// The herb's grade. An old save's shelf predates the field and reads
        /// as 地阶, the anchor its numbers were.
        #[serde(default)]
        grade: SeedGrade,
    },
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
            Self::Seed { tier, grade, .. } => seed_price(*tier, *grade),
            Self::Art { art_id } => art_price(super::blueprint(*art_id).grade),
            Self::Herb { herb, grade } => herb.price_for(*grade),
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
            Self::Herb { .. } => "cultivation.shop.herb",
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

/// What one attribute point of a seed's harvest costs on the shelf, in 灵石.
///
/// Fourteen. Read off the other ladder: a 玄阶 功法 pays thirty points over its
/// five 阶 for seven hundred 灵石 — twenty-three a point — and a seed sits under
/// it at fourteen because the seed makes the user wait for the season it takes.
/// Pricing a seed by what it *pays* rather than by its tier is what stops the
/// cheap row from being the good row: a 黄阶 first-tier herb pays a point and
/// costs fourteen, and the hundred-point crop costs what a hundred points are
/// worth whoever grew them.
pub const SEED_STONES_PER_POINT: u32 = 14;

/// What one herb costs.
///
/// Its points, at [`SEED_STONES_PER_POINT`]. The grade is already inside the
/// points, so the shelf's five grades are five prices apart without a second
/// table to keep in step — and a 神阶 seed of the tenth tier is the dearest row
/// on the shelf by exactly as much as it pays more.
pub fn seed_price(tier: u8, grade: SeedGrade) -> u32 {
    u32::try_from(super::garden::harvest_points(tier.clamp(1, 10), grade))
        .unwrap_or(u32::MAX)
        .saturating_mul(SEED_STONES_PER_POINT)
        .max(1)
}

/// What one attempt at the next star costs: half of what the piece sells for.
///
/// Read off the sell price rather than the asking price, so the number on the
/// button is the number the user can check against the 卖 button on the same
/// row. It does not climb with the star the piece already holds: every attempt
/// is the same wager, which is what makes a run of failures readable as a run
/// of the same bet rather than as an escalating one.
///
/// Stars are a sink and never an investment: the sell price is the piece's
/// tier and quality alone, so a ten-star piece sells for exactly what an
/// unstarred one does. Buying stars back at a profit would make the 灵石 purse
/// a slot machine.
pub fn star_up_cost(item: &EquipmentRecord) -> u32 {
    ShopOffer::Equipment(item.clone()).sell_price() / 2
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
pub fn art_price(grade: ArtGrade) -> u32 {
    // The grade's own band: 黄 sits with the second tier, 神 with the tenth, so
    // a 功法 is compared against the gear a cultivator of its grade would wear.
    let tier = u8::try_from((grade.index() + 1) * 2).unwrap_or(10).min(10);
    equipment_price(tier, Rarity::Common) * ART_OVER_GEAR_NUMERATOR / ART_OVER_GEAR_DENOMINATOR
}

/// What one finished session pays into the purse, at the first realm.
///
/// The currency is a way to sell what a walk turned up, not a second growth
/// ladder, so this is the *salary* and not a jackpot: a session pays both growth
/// and 灵石, which is only safe while the 灵石 cannot buy the growth that was
/// skipped — and nothing in the shop sells growth.
///
/// Multiplied by the realm, the same way growth itself is: a 金丹's afternoon is
/// worth more than a 炼气's, and the shop's prices climb with the realm too, so
/// the salary keeps its weight against the shelf instead of shrinking to nothing.
pub const SESSION_EARNINGS: u32 = 16;

/// How many offers a realm's shop rolls, from the day alone.
///
/// Five to ten, decided by the day's own number rather than a counter: a save
/// cannot ask for a better shop by being reloaded, and two users on the same day
/// in the same realm see the same shelf.
pub fn offers_for_day(day_number: i64, realm: Realm) -> usize {
    let seed = u64::from(day_number as u32).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ realm_seed(realm);
    let span = (SHOP_BASE_MAX_OFFERS - SHOP_MIN_OFFERS + 1) as u64;
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
    // The shelf has no opponent, so its strength is the realm it stocks for:
    // a garment on the shelf is rolled at what a body of that realm is worth,
    // which is the same ruler a fight's drop is read against.
    let strength = super::drop::realm_power(realm);
    let factor = super::drop::power_factor_permille(strength);
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
            0 => {
                // The seed row. One pool for the nine attributes and the two
                // 潜质 herbs, so a 悟心草 turns up exactly as often as a 根骨
                // seed does — the rare herb is a rarer *thing*, not a rarer
                // row, and a herb that needed its own one-day-in-four gate
                // would be a row the shelf is mostly not allowed to have.
                let kinds = Attribute::ALL.len() as u64 + 2;
                let pick = rng.below(kinds) as usize;
                if pick < Attribute::ALL.len() {
                    ShopOffer::Seed {
                        attribute: Attribute::from_index(pick),
                        // One tier either side of the realm's own: a herb a tier
                        // down is a reachable one for the garden, one a tier up
                        // is a goal.
                        tier: tier
                            .saturating_add(1)
                            .saturating_sub(u8::try_from(rng.below(3)).unwrap_or(0))
                            .clamp(1, 10),
                        // The grade is the discovery roll, the same five-grade
                        // ladder a dropped seed reads: a shelf sells what the
                        // garden grows.
                        grade: super::garden::roll_seed_grade(&mut rng),
                        name: None,
                    }
                } else {
                    ShopOffer::Herb {
                        herb: if pick == Attribute::ALL.len() {
                            AptitudeHerb::Insight
                        } else {
                            AptitudeHerb::Wisdom
                        },
                        // The same discovery roll the seed row above made: a
                        // 潜质 herb is a seed in everything but the plot, so the
                        // grade the shelf shows is the grade the body will pay.
                        grade: super::garden::roll_seed_grade(&mut rng),
                    }
                }
            }
            1 => {
                let mut item = super::items::roll_equipment(
                    0,
                    tier.saturating_add(u8::try_from(rng.below(2)).unwrap_or(0))
                        .min(10),
                    super::drop::roll_quality(&mut rng, factor).unwrap_or(Rarity::Common),
                    &mut rng,
                );
                item.name = Some(super::items::equipment_display_name(
                    item.slot,
                    item.effect,
                    &mut rng,
                ));
                ShopOffer::Equipment(item)
            }
            _ => ShopOffer::Art {
                art_id: shop_art_id(&mut rng, tier),
            },
        };
        offers.push(offer);
    }
    // The one 潜质 row still rolled on its own: a 洗髓丹 is a day's news, not a
    // shelf row, and rolling it per slot would multiply the odds by the shelf's
    // width. The two herbs are in the seed rows above.
    if rng.below(MARROW_SHELF_ONE_IN) == 0 {
        offers.push(ShopOffer::Herb {
            herb: AptitudeHerb::Marrow,
            // A pill pays no 潜质, so it carries no grade: the anchor is as good
            // a placeholder as any, and the price and gain rules read the herb
            // rather than the grade.
            grade: SeedGrade::Earth,
        });
    }
    offers
}

/// Which 功法 the shop offers, given the realm it is stocking for.
///
/// Weighted toward the realm's own grade band, exactly where [`ArtGrade`] is
/// graded for breakthroughs, and never the rarest band: 神级 belongs to walks
/// that get lucky, not to a shelf. Inside the band the pick is weighted the way
/// every other pick is — a 潜质 book at half the odds of any other — so the
/// shelf cannot become the cheap way into the one scarce number.
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
        return 1;
    }
    let total: u64 = band
        .iter()
        .map(|id| super::arts::art_pick_weight(*id))
        .sum();
    let mut roll = rng.below(total.max(1));
    for id in band {
        let weight = super::arts::art_pick_weight(id);
        if roll < weight {
            return id;
        }
        roll -= weight;
    }
    1
}

/// How many of the day's offers are cut in price.
///
/// Two a day, and never the whole shelf: a shelf where everything is on sale is
/// a shelf with no price, and one special on a ten-row shelf would be missed by
/// anyone who did not count. Two is what a user can hold in mind while comparing
/// the row above with the row below.
pub const DAILY_SPECIAL_COUNT: usize = 2;

/// The deepest cut a special can be, in parts per thousand of the list price.
///
/// Six tenths — 6折 — and no deeper: the shop already sells at half what it buys
/// for, so a special that went near the sell-back price would be a way to buy
/// 灵石 rather than goods. The band runs 6折 to 8折, five points of a percent
/// apart, so a special is worth checking and not always worth clearing the purse
/// for.
pub const SPECIAL_DEEPEST_PERMILLE: u32 = 600;

/// The shallowest cut a special can be.
pub const SPECIAL_SHALLOWEST_PERMILLE: u32 = 800;

/// What a price is cut to at this discount, rounding down.
///
/// Down rather than up, so the number on the button is never dearer than the
/// fraction promises.
pub fn discounted_price(price: u32, permille: u32) -> u32 {
    u64::from(price)
        .saturating_mul(u64::from(permille))
        .wrapping_div(1_000) as u32
}

/// Which slots of a day's shelf are on special, and at what cut.
///
/// As pure as the shelf itself, and from the same day and realm: a special a save
/// could reload into is not a special, and the shelf it discounts has to be the
/// shelf the buyer is looking at. The cut is rolled after the slots, so two
/// specials on the same day differ as often as not.
pub fn special_slots(day_number: i64, realm: Realm, offers: usize) -> Vec<(usize, u32)> {
    if offers == 0 {
        return Vec::new();
    }
    let mut rng = super::Rng::seeded(special_seed(day_number, realm));
    let count = DAILY_SPECIAL_COUNT.min(offers);
    let mut picked: Vec<usize> = Vec::with_capacity(count);
    while picked.len() < count {
        let slot = rng.below(offers as u64) as usize;
        if !picked.contains(&slot) {
            picked.push(slot);
        }
    }
    picked.sort_unstable();
    let span = u64::from(SPECIAL_SHALLOWEST_PERMILLE - SPECIAL_DEEPEST_PERMILLE + 1);
    picked
        .into_iter()
        .map(|slot| {
            let permille = SPECIAL_DEEPEST_PERMILLE + rng.below(span) as u32;
            (slot, permille)
        })
        .collect()
}

/// A realm's contribution to the day's shelf, so two realms never share a shelf.
fn realm_seed(realm: Realm) -> u64 {
    0x517C_C1B7_2722_0A95u64.wrapping_mul(realm.index() as u64 + 1)
}

/// The day's seed for the specials, kept apart from the shelf's so a special
/// never reshuffles the goods it is cutting.
fn special_seed(day_number: i64, realm: Realm) -> u64 {
    let day = u64::from(day_number as u32).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    day ^ realm_seed(realm).rotate_left(17) ^ 0x0000_5EED_5EED_5EED
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
    /// What the row asks for it — the list price, or the special's cut of it.
    pub price: u32,
    /// The price before any special, so a row can say what the cut was worth.
    pub list_price: u32,
    /// The cut this row carries in parts per thousand, when it is a special.
    /// Zero for a row at its list price.
    pub discount_permille: u32,
    /// The shop's own price for it, so a surface can print "卖 N" without
    /// re-deriving the margin.
    pub sell_price: u32,
    pub bought: bool,
    /// The slot this offer sits in, which is what a buy command names.
    pub slot: usize,
    /// How this row's 功法 stands against the cultivator's own copies, for the
    /// rows that are 功法 at all. `None` for everything else — a seed the buyer
    /// already has a stack of is not a duplicate, it is a stack.
    pub art_ownership: Option<ArtOwnership>,
}

/// What the shelf says about a 功法 the cultivator already knows.
///
/// The two cases sell differently and the row has to say which one it is: a
/// book still on the shelf is a spare — buyable, and worth the stones a sale
/// brings — while a 功法 learned outright has no page left, so the shelf will
/// not take stones for it at all.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtOwnership {
    /// A book already on the shelf, still being read.
    Shelved,
    /// A 功法 learned outright: every 阶 is in.
    Mastered,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two 潜质 herbs are ordinary shelf stock: each of them turns up about
    /// as often as one particular attribute seed does, and the pill — the one
    /// 潜质 item still rolled on its own — stays a one-in-two-hundred day.
    #[test]
    fn the_two_herbs_share_the_seed_rows_and_the_pill_is_the_rarest_row() {
        let days = 4_000_i64;
        let (mut insight, mut wisdom, mut marrow, mut seeds) = (0_u32, 0_u32, 0_u32, 0_u32);
        for day in 0..days {
            for offer in roll_offers(day, Realm::from_index(0), day as u64) {
                match offer {
                    ShopOffer::Herb { herb, .. } => match herb {
                        AptitudeHerb::Insight => insight += 1,
                        AptitudeHerb::Wisdom => wisdom += 1,
                        AptitudeHerb::Marrow => marrow += 1,
                    },
                    ShopOffer::Seed { .. } => seeds += 1,
                    _ => {}
                }
            }
        }
        // One attribute out of the four, so a herb with the same odds as "any
        // other seed" lands on one quarter of the seed rows.
        let per_attribute = f64::from(seeds) / Attribute::ALL.len() as f64;
        for (label, count) in [("悟心草", insight), ("慧心草", wisdom)] {
            let ratio = f64::from(count) / per_attribute;
            assert!(
                (0.7..=1.4).contains(&ratio),
                "{label} turns up {ratio:.2} times as often as one attribute seed: \
                 {count} against {seeds} seeds"
            );
        }
        // The pill is still a sighting rather than a row: one day in two
        // hundred, measured over the days themselves.
        let rate = f64::from(marrow) / days as f64;
        assert!(
            (rate - 1.0 / 200.0).abs() < 0.02,
            "洗髓丹 one day in two hundred: {marrow} of {days}"
        );
        assert!(marrow < wisdom, "the pill is the rarest of the three");
    }

    /// The three 潜质 items are priced apart, and the pill is the dearest thing
    /// the shop sells: five times the rarer herb, because it is the one row a
    /// build saves for rather than buys on a whim.
    #[test]
    fn the_aptitude_items_are_priced_apart() {
        // A 潜质 herb is dearer than a 神阶 seed of the middle of the ladder: the
        // number a whole build turns on is not a garden row.
        assert!(AptitudeHerb::Insight.price() > seed_price(4, SeedGrade::Divine));
        assert!(AptitudeHerb::Marrow.price() > AptitudeHerb::Wisdom.price());
        assert!(AptitudeHerb::Wisdom.price() > AptitudeHerb::Insight.price());
        assert_eq!(AptitudeHerb::Marrow.price(), 10_000);
        assert_eq!(AptitudeHerb::Insight.price(), 1_200);
        assert_eq!(AptitudeHerb::Wisdom.price(), 2_000);
    }

    /// A herb's grade prices the offer the same way it prices the gain: 地阶 is
    /// the anchor, 神阶 two and a half times. A better grade is a dearer row,
    /// and the pill is priced at its own flat rate whatever the grade.
    #[test]
    fn a_herb_offer_is_priced_at_its_grade() {
        let insight = ShopOffer::Herb {
            herb: AptitudeHerb::Insight,
            grade: SeedGrade::Divine,
        };
        assert_eq!(
            insight.price(),
            3_000,
            "a 神级 悟心草 is two and a half the 地阶"
        );
        let earth = ShopOffer::Herb {
            herb: AptitudeHerb::Insight,
            grade: SeedGrade::Earth,
        };
        assert_eq!(earth.price(), 1_200, "the 地阶 is the anchor");
        assert!(insight.price() > earth.price());
        let pill = ShopOffer::Herb {
            herb: AptitudeHerb::Marrow,
            grade: SeedGrade::Yellow,
        };
        assert_eq!(pill.price(), 10_000, "a pill carries no grade to price at");
    }
}
