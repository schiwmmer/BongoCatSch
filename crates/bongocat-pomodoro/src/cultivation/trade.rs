//! What a trade can refuse, and what one finished session pays.
//!
//! The transitions themselves live beside the rest of the state machine's, in
//! [`super::state`]: selling a worn item or a trained 功法 means reading the
//! document's private fields, and a module that cannot see them would have to
//! either take a `&mut Cultivation` and reach through it or be given access it
//! does not need. What is left here is the part a surface and a test can read
//! without touching a save: the reasons a refusal exists, and the one number that
//! is not a price.

use super::shop::SESSION_EARNINGS;

/// What a purchase or a sale could not do.
///
/// A refusal with a reason rather than a bare false, because the page has to say
/// something: "不是灵石不够" and "这件已经卖掉了" are different sentences and the
/// user needs to know which one happened.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TradeRefusal {
    /// The purse does not hold the asking price.
    NotEnoughStones,
    /// That slot is empty, or the shelf moved under the click.
    NoSuchOffer,
    /// Already bought today.
    AlreadyBought,
    /// No such item, or it is worn right now.
    NotOwned,
    /// Worn gear cannot be sold out from under the fight that is wearing it.
    Equipped,
    /// The 功法 being trained cannot be sold: the 修炼 dimension reads it, and a
    /// dimension reading a thing that is no longer there is a save that lies.
    Training,
    /// The 功法 is already owned or already learned, so there is nothing to
    /// buy: a second copy of a 功法 the cultivator has is not a thing the page
    /// could say anything true about.
    AlreadyOwned,
}

impl TradeRefusal {
    /// The catalog key saying why.
    pub const fn key(self) -> &'static str {
        match self {
            Self::NotEnoughStones => "cultivation.shop.refuse_stones",
            Self::NoSuchOffer => "cultivation.shop.refuse_offer",
            Self::AlreadyBought => "cultivation.shop.refuse_bought",
            Self::NotOwned => "cultivation.shop.refuse_owned",
            Self::Equipped => "cultivation.shop.refuse_equipped",
            Self::Training => "cultivation.shop.refuse_training",
            Self::AlreadyOwned => "cultivation.shop.refuse_already",
        }
    }
}

/// What one finished session pays into the purse.
pub fn session_earnings() -> u32 {
    SESSION_EARNINGS
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::{Cultivation, CultivationDocument, Rarity, Realm, ShopOffer};

    /// The margin is what stops a shop from being a money printer: buy at the
    /// asking price, sell straight back, and a save that can do it grows 灵石
    /// without playing.
    #[test]
    fn nothing_the_shop_sells_is_worth_buying_to_resell() {
        for tier in 1..=10u8 {
            for rarity in Rarity::ALL {
                let offer = ShopOffer::Equipment(crate::cultivation::EquipmentRecord {
                    id: 0,
                    slot: crate::cultivation::EquipSlot::Weapon,
                    tier,
                    rarity,
                    attributes: Default::default(),
                    name: None,
                });
                assert!(
                    offer.sell_price() < offer.price(),
                    "tier {tier} {:?}: {} must be under {}",
                    rarity,
                    offer.sell_price(),
                    offer.price()
                );
            }
        }
        for tier in 1..=10u8 {
            let offer = ShopOffer::Seed {
                attribute: crate::cultivation::Attribute::Root,
                tier,
                name: None,
            };
            assert!(offer.sell_price() < offer.price(), "seed tier {tier}");
        }
        for id in 1..=crate::cultivation::ART_COUNT {
            let offer = ShopOffer::Art { art_id: id };
            assert!(
                offer.sell_price() < offer.price(),
                "art {id} must not buy back at its own price"
            );
        }
    }

    /// The shelf is a function of the day and the realm, so it cannot be rerolled
    /// by reloading, and two reads of one day agree.
    #[test]
    fn the_shelf_is_the_same_every_time_it_is_looked_at() {
        let cultivation = Cultivation::default();
        let first = cultivation.shop(20_000);
        let second = cultivation.shop(20_000);
        assert!(!first.is_empty(), "the shop stocks something");
        assert_eq!(first.len(), second.len());
        for (a, b) in first.iter().zip(second.iter()) {
            assert_eq!(a.price, b.price);
            assert_eq!(a.offer, b.offer);
        }
    }

    /// Five to ten offers, every day: below five a realm that can use one kind of
    /// gear leaves the rest dead, above ten the day stops being a choice.
    #[test]
    fn a_day_offers_between_five_and_ten_goods() {
        for day in 20_000..20_200 {
            let count = crate::cultivation::offers_for_day(day, Realm::from_level(1));
            assert!(
                (crate::cultivation::SHOP_MIN_OFFERS..=crate::cultivation::SHOP_MAX_OFFERS)
                    .contains(&count),
                "day {day} offered {count}"
            );
        }
    }

    /// Two realms never share a shelf: the goods are rolled for the realm the
    /// buyer stands in, so a 金丹 is not offered 炼气 cloth.
    #[test]
    fn two_realms_do_not_stock_the_same_goods() {
        let first = crate::cultivation::roll_offers(20_000, Realm::from_level(1), 20_000);
        let deepest = crate::cultivation::roll_offers(20_000, Realm::from_level(900), 20_000);
        assert_ne!(first, deepest);
    }

    /// A refused purchase changes nothing at all — not the purse, not the shelf,
    /// not the inventory. A click that cannot be paid for must be a no-op, or the
    /// page and the save disagree about what the user owns.
    #[test]
    fn a_purchase_that_cannot_be_paid_for_changes_nothing() {
        let mut cultivation = Cultivation::new(CultivationDocument {
            stones: 0,
            ..CultivationDocument::default()
        });
        let shelf = cultivation.shop(20_000);
        let slot = shelf
            .iter()
            .find(|offer| offer.price > 0)
            .expect("a priced offer")
            .slot;
        let refusal = cultivation
            .buy_from_shop(slot, 20_000, 1_000)
            .expect_err("no stones, no goods");
        assert_eq!(refusal, TradeRefusal::NotEnoughStones);
        assert_eq!(cultivation.stones(), 0, "no purse changed");
        assert!(cultivation.document().equipment.is_empty());
        assert!(cultivation.document().seeds.is_empty());
    }

    /// What a finished session pays is credited once, and only once.
    #[test]
    fn a_finished_session_pays_into_the_purse() {
        let mut cultivation = Cultivation::default();
        assert_eq!(cultivation.stones(), 0);
        cultivation.on_session_completed(1_000, crate::CalendarDay::from_day_number(20), 25);
        assert_eq!(cultivation.stones(), session_earnings());
    }

    /// Worn gear cannot be sold out from under the fight wearing it, and the
    /// trained 功法 cannot be sold at all — 修炼 reads it every session.
    #[test]
    fn what_is_worn_or_being_trained_is_not_for_sale() {
        let cultivation = Cultivation::default();
        let item = crate::cultivation::EquipmentRecord {
            id: 1,
            slot: crate::cultivation::EquipSlot::Weapon,
            tier: 1,
            rarity: Rarity::Common,
            attributes: Default::default(),
            name: None,
        };
        let mut document = cultivation.document().clone();
        document.equipment.push(item);
        let mut cultivation = Cultivation::new(document);
        cultivation.equip(1);
        assert_eq!(
            cultivation.sell_equipment(1).expect_err("worn"),
            TradeRefusal::Equipped
        );

        let mut art = Cultivation::new(CultivationDocument {
            arts: vec![crate::cultivation::OwnedArt {
                art_id: 1,
                rank: 1,
                progress: 0,
                outcome: None,
            }],
            selected_art: Some(1),
            ..CultivationDocument::default()
        });
        assert_eq!(
            art.sell_art(1).expect_err("being trained"),
            TradeRefusal::Training
        );
    }

    /// A sale pays the shop's price and takes the goods away.
    #[test]
    fn selling_pays_the_seller_price_and_drops_the_goods() {
        let cultivation = Cultivation::new(CultivationDocument {
            stones: 0,
            ..CultivationDocument::default()
        });
        let mut document = cultivation.document().clone();
        document.seeds.push(crate::cultivation::SeedStack::single(
            crate::cultivation::Attribute::Root,
            2,
        ));
        let mut cultivation = Cultivation::new(document);
        let paid = cultivation
            .sell_seed(crate::cultivation::Attribute::Root, 2)
            .expect("a seed was sold");
        assert!(paid > 0);
        assert_eq!(cultivation.stones(), paid);
        assert!(
            cultivation.document().seeds.is_empty(),
            "the last seed took its stack with it"
        );
    }
}
