//! The read-only surfaces the settings window and the shop read from.
//!
//! Split out of `state.rs` as a child module so it keeps access to the
//! document's private fields without widening their visibility.

use super::*;

// The shop's transitions live here rather than beside the shop's types: each one
// reads and writes the document's private fields, and a separate module could
// only do that by reaching through a `&mut Cultivation` it was handed.
impl Cultivation {
    /// The day's shelf, as a surface reads it.
    ///
    /// Rolled from the day and the realm, so the same day always shows the same
    /// goods and a document cannot carry a shelf it never would have rolled. The
    /// bought slots are cleared here rather than in the domain's own transition:
    /// a user who opens the page on a new day — or after a 破境 re-rolled the
    /// same day's shelf — has, for that page, a new shelf.
    pub fn shop(&self, today: i64) -> Vec<ShopOfferView> {
        let realm = self.realm();
        let bought: &[u8] = if self.shop_bought_valid(today) {
            &self.document.shop_bought
        } else {
            &[]
        };
        let offers = shop::roll_offers(today, realm, self.shop_seed(today));
        // The day's specials are read off the same day and realm as the shelf,
        // so the row's cut and the price a purchase charges are one number seen
        // twice rather than two guesses at the same discount.
        let specials = shop::special_slots(today, realm, offers.len());
        offers
            .into_iter()
            .enumerate()
            .map(|(slot, offer)| {
                let list_price = offer.price();
                let discount_permille = specials
                    .iter()
                    .find(|(special, _)| *special == slot)
                    .map_or(0, |(_, permille)| *permille);
                let price = if discount_permille == 0 {
                    list_price
                } else {
                    shop::discounted_price(list_price, discount_permille)
                };
                ShopOfferView {
                    offer: offer.clone(),
                    price,
                    list_price,
                    discount_permille,
                    sell_price: offer.sell_price(),
                    bought: bought.contains(&(slot as u8)),
                    slot,
                    art_ownership: match &offer {
                        ShopOffer::Art { art_id }
                            if self.document.mastered_arts.contains(art_id) =>
                        {
                            Some(shop::ArtOwnership::Mastered)
                        }
                        ShopOffer::Art { art_id }
                            if self.document.arts.iter().any(|art| art.art_id == *art_id) =>
                        {
                            Some(shop::ArtOwnership::Shelved)
                        }
                        _ => None,
                    },
                }
            })
            .collect()
    }

    /// Buy one of the day's offers.
    ///
    /// The slot is what a command names, not the item, because the shelf is
    /// regenerated from the day: naming the item would mean a click on an item
    /// from yesterday's shelf could buy today's item in the same slot.
    pub fn buy_from_shop(
        &mut self,
        slot: usize,
        today: i64,
        now_unix: i64,
    ) -> Result<ShopOffer, TradeRefusal> {
        self.roll_shop_day(today);
        let realm = self.realm();
        let shelf = shop::roll_offers(today, realm, self.shop_seed(today));
        let Some(offer) = shelf.get(slot).cloned() else {
            return Err(TradeRefusal::NoSuchOffer);
        };
        if self.document.shop_bought.contains(&(slot as u8)) {
            return Err(TradeRefusal::AlreadyBought);
        }
        // What the shelf asks, special and all: the button's number is the one
        // the purchase charges, or the cut would be a label on nothing.
        let price = match shop::special_slots(today, realm, shelf.len())
            .into_iter()
            .find(|(special, _)| *special == slot)
        {
            Some((_, permille)) => shop::discounted_price(offer.price(), permille),
            None => offer.price(),
        };
        if self.document.stones < price {
            return Err(TradeRefusal::NotEnoughStones);
        }
        // A 功法 learned outright is the one book the shelf will not sell: there
        // is no page left in it, so a copy would teach nothing and could not even
        // be read. A 功法 still on the shelf is a different matter — the buyer is
        // buying a *spare*, the row says 已有 so the stones are spent knowingly,
        // and the copy lands beside the one being read.
        if let ShopOffer::Art { art_id } = &offer
            && self.document.mastered_arts.contains(art_id)
        {
            return Err(TradeRefusal::AlreadyOwned);
        }
        self.document.stones -= price;
        self.document.shop_bought.push(slot as u8);
        // The goods land the same way a drop's do, so a bought 功法 counts toward
        // 修炼 and a bought herb plants, rather than sitting in a drawer.
        let bought = offer.clone();
        // The log says what was bought, and what it cost: named when the thing
        // has a name, the kind when it does not, and the price beside both —
        // a purchase that does not say what it paid is one the buyer cannot
        // check against the shelf.
        let what = match &bought {
            ShopOffer::Equipment(item) => SaleKind::Equipment {
                name: item.name.clone(),
                attributes: Some(item.effective_attributes()),
            },
            ShopOffer::Seed {
                attribute,
                name,
                tier,
                grade,
            } => SaleKind::Seed {
                name: name.clone(),
                tier: *tier,
                grade: *grade,
                // The herb's own attribute, kept beside its name: the log line
                // prints what a harvest of it pays, and that needs the attribute
                // rather than the words in the name.
                attribute: Some(*attribute),
            },
            ShopOffer::Art { art_id } => SaleKind::Art {
                name: Some(blueprint(*art_id).name),
                art_id: Some(*art_id),
            },
            ShopOffer::Herb { herb, .. } => SaleKind::Herb { herb: *herb },
        };
        match bought {
            ShopOffer::Equipment(item) => {
                let mut item = item;
                item.id = self.next_id();
                self.document.equipment.push(item);
            }
            ShopOffer::Seed {
                attribute,
                tier,
                grade,
                name,
            } => {
                let mut rng = self.rng(now_unix);
                self.push_seed_named(attribute, tier, grade, name, &mut rng);
            }
            ShopOffer::Art { art_id } => {
                // The book that was on the shelf, not another one of its grade:
                // the shelf names the 功法, and buying a different one would be
                // a market that sells what it does not show.
                self.grant_art_book(art_id);
            }
            ShopOffer::Herb { herb, grade } => {
                // A 潜质 herb is bought as a seed: it has to be grown before it
                // can be taken, so the row hands over the seed the way it hands
                // over any other seed. A 洗髓丹 lands in the bag, because a pill
                // is spent by opening a gate from the satchel. The grade the row
                // showed is the grade the seed carries and the body pays.
                match herb {
                    super::super::AptitudeHerb::Marrow => {
                        self.document.marrow_pills = self.document.marrow_pills.saturating_add(1);
                        self.push_event(
                            now_unix,
                            EventDetail::MarrowFound {
                                source: MarrowSource::Shop,
                            },
                        );
                    }
                    _ => {
                        self.push_herb_seed(herb, grade);
                    }
                }
            }
        }
        self.push_event(
            now_unix,
            EventDetail::Bought {
                what,
                stones: price,
            },
        );
        Ok(offer)
    }

    /// Sell one piece of gear for 灵石.
    pub fn sell_equipment(&mut self, item_id: u64, now_unix: i64) -> Result<u32, TradeRefusal> {
        let Some(index) = self
            .document
            .equipment
            .iter()
            .position(|item| item.id == item_id)
        else {
            return Err(TradeRefusal::NotOwned);
        };
        if self
            .document
            .equipped
            .iter()
            .flatten()
            .any(|worn| *worn == item_id)
        {
            return Err(TradeRefusal::Equipped);
        }
        let item: EquipmentRecord = self.document.equipment.remove(index);
        let stones = shop::ShopOffer::Equipment(item.clone()).sell_price();
        self.document.stones = self.document.stones.saturating_add(stones);
        self.push_event(
            now_unix,
            EventDetail::Sold {
                what: SaleKind::Equipment {
                    name: item.name.clone(),
                    attributes: Some(item.effective_attributes()),
                },
                stones,
            },
        );
        Ok(stones)
    }

    /// Sell one 功法 for 灵石 — a book, or the tome of a learned one.
    ///
    /// A **copy** is what goes first: a spare sitting beside the book being read
    /// is sold on its own, and the reading — 阶、进度、失败 — never moves with it.
    /// The last copy of a book being trained is refused: 修炼 reads it every
    /// session, and the record of a session that trained a 功法 no longer owned
    /// would be a lie.
    ///
    /// A **已习得** 功法 is sellable too, and selling one sells the tome, not
    /// the knowledge: the 功法 stays in [`CultivationDocument::mastered_arts`],
    /// its attributes and its swing stay exactly what they were, and the stones
    /// are the market's price for the paper. The sale is collected once — a
    /// tome the market buys again every time it is asked about would be a
    /// money printer — so the id is written into
    /// [`CultivationDocument::sold_mastered_arts`] and a second attempt is
    /// refused.
    pub fn sell_art(&mut self, art_id: u64, now_unix: i64) -> Result<u32, TradeRefusal> {
        let copies = self
            .document
            .arts
            .iter()
            .find(|art| art.art_id == art_id)
            .map_or(0, |art| art.copies.max(1));
        if copies > 1 {
            // A spare goes, the book being read stays: the two are the same
            // 功法 and the same ladder, and the sale is priced as one book.
            if let Some(art) = self
                .document
                .arts
                .iter_mut()
                .find(|art| art.art_id == art_id)
            {
                art.copies = copies - 1;
            }
        } else if copies == 1 {
            if self.document.selected_art == Some(art_id) {
                return Err(TradeRefusal::Training);
            }
            self.document.arts.retain(|art| art.art_id != art_id);
        } else {
            // A learned 功法 is not in the books; if it is not learned either,
            // there is nothing here to sell.
            if !self.document.mastered_arts.contains(&art_id) {
                return Err(TradeRefusal::NotOwned);
            }
            if self.document.sold_mastered_arts.contains(&art_id) {
                return Err(TradeRefusal::AlreadySold);
            }
            self.document.sold_mastered_arts.push(art_id);
        }
        let stones = shop::ShopOffer::Art { art_id }.sell_price();
        self.document.stones = self.document.stones.saturating_add(stones);
        self.push_event(
            now_unix,
            EventDetail::Sold {
                what: SaleKind::Art {
                    name: Some(blueprint(art_id).name),
                    art_id: Some(art_id),
                },
                stones,
            },
        );
        Ok(stones)
    }

    /// Sell one seed from a stack for 灵石.
    ///
    /// One at a time rather than the whole stack: a stack is nine seeds and one
    /// of them, and which of them the user wants to keep is not a question a
    /// single "sell all" button can ask.
    pub fn sell_seed(
        &mut self,
        attribute: super::super::Attribute,
        tier: u8,
        grade: SeedGrade,
        now_unix: i64,
    ) -> Result<u32, TradeRefusal> {
        let Some(index) = self.document.seeds.iter().position(|stack| {
            stack.attribute == attribute && stack.tier == tier && stack.grade == grade
        }) else {
            return Err(TradeRefusal::NotOwned);
        };
        if self.document.seeds[index].count == 0 {
            return Err(TradeRefusal::NotOwned);
        }
        let stack: &mut SeedStack = &mut self.document.seeds[index];
        stack.count -= 1;
        let stones = shop::ShopOffer::Seed {
            attribute,
            tier,
            grade,
            name: stack.name.clone(),
        }
        .sell_price();
        let name = stack.name.clone();
        if stack.count == 0 {
            self.document.seeds.remove(index);
        }
        self.document.stones = self.document.stones.saturating_add(stones);
        self.push_event(
            now_unix,
            EventDetail::Sold {
                what: SaleKind::Seed {
                    name,
                    tier,
                    grade,
                    attribute: Some(attribute),
                },
                stones,
            },
        );
        Ok(stones)
    }

    /// Sell one 潜质 herb seed from a stack for 灵石.
    ///
    /// One at a time, the way [`Self::sell_seed`] sells one attribute seed: the
    /// herb seed is priced as the herb it grows into, not as a tier of
    /// attribute points.
    pub fn sell_herb_seed(
        &mut self,
        herb: super::super::AptitudeHerb,
        grade: SeedGrade,
        now_unix: i64,
    ) -> Result<u32, TradeRefusal> {
        let tier = super::super::herb_tier(herb);
        let Some(index) = self.document.seeds.iter().position(|stack| {
            stack.herb == Some(herb) && stack.tier == tier && stack.grade == grade
        }) else {
            return Err(TradeRefusal::NotOwned);
        };
        if self.document.seeds[index].count == 0 {
            return Err(TradeRefusal::NotOwned);
        }
        let stack: &mut SeedStack = &mut self.document.seeds[index];
        stack.count -= 1;
        let stones = shop::ShopOffer::Herb { herb, grade }.sell_price();
        if stack.count == 0 {
            self.document.seeds.remove(index);
        }
        self.document.stones = self.document.stones.saturating_add(stones);
        self.push_event(
            now_unix,
            EventDetail::Sold {
                what: SaleKind::Herb { herb },
                stones,
            },
        );
        Ok(stones)
    }
    ///
    /// The price doubles with every plot bought, and the garden holds twelve at
    /// most: a plot is a permanent capacity, so the last one costs what a
    /// season of sessions earns rather than what one afternoon does. A refusal
    /// is remembered the way every trade's is, so the page can say why.
    pub fn buy_plot(&mut self, now_unix: i64) -> Result<u32, TradeRefusal> {
        let Some(price) = plot_price(self.document.plots.len()) else {
            return Err(TradeRefusal::PlotLimit);
        };
        if self.document.stones < price {
            return Err(TradeRefusal::NotEnoughStones);
        }
        self.document.stones -= price;
        self.document.plots.push(PlotRecord::default());
        self.push_event(
            now_unix,
            EventDetail::Bought {
                what: SaleKind::Plot,
                stones: price,
            },
        );
        Ok(price)
    }

    /// Roll the shop's day forward if it has not been done yet.
    /// Whether the recorded bought slots still describe the shelf on display.
    ///
    /// They are valid only against the day *and* the realm they were recorded
    /// under, because that is what the shelf is rolled from. A 破境 lifts the
    /// realm inside the same day, re-rolling the shelf in place; the indexes in
    /// [`CultivationDocument::shop_bought`] are positions in that older shelf,
    /// and honouring them against the new one would mark a good the cultivator
    /// never bought — a 慧心草 struck through as already bought.
    fn shop_bought_valid(&self, today: i64) -> bool {
        self.document.shop_day == Some(today)
            && self.document.shop_realm == Some(self.document.realm)
    }

    fn roll_shop_day(&mut self, today: i64) {
        let realm = self.document.realm;
        if self.document.shop_day != Some(today) || self.document.shop_realm != Some(realm) {
            self.document.shop_day = Some(today);
            self.document.shop_realm = Some(realm);
            self.document.shop_bought.clear();
        }
    }

    /// The seed the day's shelf is rolled from.
    ///
    /// The day alone, so the shelf is a pure function of `(day, realm)` — the same
    /// property the tests pin. Folding in the user's own counter would have spread
    /// two saves apart, but it would also have moved the shelf every time the user
    /// bought something, which is a shop that rearranges itself mid-transaction.
    fn shop_seed(&self, today: i64) -> u64 {
        today as u64
    }

    /// Credit one finished session's 灵石.
    ///
    /// The only 灵石 that arrives without selling something. Small enough that a
    /// user who never sells is poor on purpose: the currency is a way to convert a
    /// walk into something useful, and a faucet that outran the walks would make
    /// the shop a second ladder.
    pub(crate) fn credit_session_earnings(&mut self) {
        let earnings = session_earnings(self.realm());
        self.document.stones = self.document.stones.saturating_add(earnings);
    }

    /// The purse, for a surface.
    pub fn stones(&self) -> u32 {
        self.document.stones
    }
}
