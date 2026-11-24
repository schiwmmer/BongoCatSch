//! 园圃与装备：种下、收获、升星与穿戴。
//!
//! Split out of `state.rs` as a child module so it keeps access to the
//! document's private fields without widening their visibility.

use super::*;

impl Cultivation {
    /// Plant one owned seed in the first empty plot.
    ///
    /// The roll happens **now**, while the seed is still in hand, and never at
    /// harvest: a bed is committed for its tier's whole wait, and an answer that
    /// late is a punishment the player will simply stop asking for.
    /// A crop that survived its planting cannot be lost afterwards, which is why
    /// the plot below is only ever handed to a planting that has already come up.
    ///
    /// Returns what became of it, or `None` when there was no seed of that kind
    /// or nowhere to put one — nothing is spent in either of those cases.
    pub fn plant(
        &mut self,
        attribute: Attribute,
        tier: u8,
        grade: SeedGrade,
        now_unix: i64,
    ) -> Option<PlantOutcome> {
        self.plant_one(attribute, None, tier, grade, now_unix)
    }

    /// Plant one 潜质 herb seed, at its herb's fixed tier, in the first empty
    /// plot. The same rules as [`Self::plant`] apply — the roll happens now, and
    /// nothing is spent when there was no such seed or no bed to put it in. The
    /// pill is never planted: it opens a gate from the satchel, not from a bed.
    pub fn plant_herb_seed(
        &mut self,
        herb: super::super::AptitudeHerb,
        grade: SeedGrade,
        now_unix: i64,
    ) -> Option<PlantOutcome> {
        if herb == super::super::AptitudeHerb::Marrow {
            return None;
        }
        self.plant_one(
            Attribute::Strength,
            Some(herb),
            super::super::herb_tier(herb),
            grade,
            now_unix,
        )
    }

    /// The planting both paths share: find the stack, spend one seed, and bed
    /// the crop — with the answer to the roll kept in the bed rather than told
    /// on the spot.
    ///
    /// `herb` is `None` for an attribute seed and `Some` for a 潜质 herb seed;
    /// every match on the stack and every event the planting writes carries it,
    /// so a herb seed is never read as an attribute and vice versa.
    fn plant_one(
        &mut self,
        attribute: Attribute,
        herb: Option<super::super::AptitudeHerb>,
        tier: u8,
        grade: SeedGrade,
        now_unix: i64,
    ) -> Option<PlantOutcome> {
        // Decided before anything is taken, because the two reads below borrow
        // the document's own fields and cannot see past themselves.
        let permille = plant_permille(self.document.garden_rank, tier, grade);
        let took = {
            let mut rng = self.rng(now_unix);
            rng.below(1_000) < u64::from(permille)
        };
        let index = self.document.seeds.iter().position(|stack| match herb {
            Some(herb) => stack.herb == Some(herb) && stack.tier == tier && stack.grade == grade,
            None => {
                stack.herb.is_none()
                    && stack.attribute == attribute
                    && stack.tier == tier
                    && stack.grade == grade
            }
        })?;
        // The bed is found before the seed is spent: a full garden must not
        // take the seed for a planting there is nowhere to put.
        let plot_index = self
            .document
            .plots
            .iter()
            .position(|plot| plot.is_empty())?;
        // The plot inherits the herb's name: the seed stack is spent one
        // planting at a time and the name is the only thing that survives it.
        let mut stack = self.document.seeds.remove(index);
        stack.count = stack.count.saturating_sub(1);
        let name = stack.name.clone();
        if stack.count > 0 {
            self.document.seeds.push(stack);
        }
        // The bed takes the crop either way. A seed that was never going to come
        // up still occupies the bed until the halfway mark, because telling the
        // gardener at once made planting a slot machine with the answer printed
        // on the lever: you knew before you had walked away. The 造化, the pity
        // count and the log line are all written by [`Self::settle_garden`] the
        // moment the bed is allowed to speak.
        let plot = &mut self.document.plots[plot_index];
        plot.crop = Some(match herb {
            Some(herb) => {
                let mut crop = CropRecord::herb_planted(
                    herb,
                    grade,
                    name,
                    self.document.garden_rank,
                    now_unix,
                );
                crop.withered = !took;
                crop
            }
            None => {
                let mut crop = CropRecord::planted(
                    attribute,
                    tier,
                    grade,
                    name,
                    self.document.garden_rank,
                    now_unix,
                );
                crop.withered = !took;
                crop
            }
        });
        Some(PlantOutcome { permille })
    }

    /// Let every bed past its halfway mark say how it is doing, and clear the
    /// dead ones that have run their course. Returns whether anything moved.
    ///
    /// Called by the worker's tick beside the walk and the job: a bed is on the
    /// clock, not on a click, so a machine that slept through the halfway mark
    /// still pays what the failure pays — 造化, the pity count and the log line —
    /// the first time it looks.
    pub fn settle_garden(&mut self, now_unix: i64) -> bool {
        let mut moved = false;
        for index in 0..self.document.plots.len() {
            // Copied out rather than borrowed: paying a failure writes an event,
            // which borrows the whole document again.
            let Some(crop) = self.document.plots[index].crop.clone() else {
                continue;
            };
            if !crop.withered {
                continue;
            }
            if crop.is_revealed(now_unix) && !crop.settled {
                let permille = plant_permille(self.document.garden_rank, crop.tier, crop.grade);
                let xp = wither_points(crop.tier, crop.grade);
                self.document.garden_xp = self.document.garden_xp.saturating_add(i64::from(xp));
                self.push_event(
                    now_unix,
                    EventDetail::Withered {
                        attribute: crop.attribute,
                        herb: crop.herb,
                        tier: crop.tier,
                        grade: crop.grade,
                        name: crop.name,
                        permille,
                        xp,
                    },
                );
                if let Some(held) = self.document.plots[index].crop.as_mut() {
                    held.settled = true;
                }
                moved = true;
            }
            // A dead crop is cleared once its own day is up: the harvest it will
            // never give is the moment its claim on the bed dies too, so a plot
            // cannot be lost by a gardener who never clicked 铲除.
            if self.document.plots[index]
                .crop
                .as_ref()
                .is_some_and(|held| held.is_ready(now_unix))
            {
                self.document.plots[index].crop = None;
                moved = true;
            }
        }
        moved
    }

    /// Dig a crop out of its bed before its day is up, freeing the plot for the
    /// next seed. Nothing is paid and nothing comes back: the seed is gone, the
    /// crop is gone, and the bed is what the gardener keeps.
    ///
    /// Any crop can be dug — a dead one the gardener was always allowed to clear,
    /// and a live one the gardener may now give up. A ripe crop cannot: it is a
    /// harvest waiting to be picked, and digging it would throw away the one
    /// thing the bed was for. The surface asks for a ripe crop's bed with the
    /// harvest button instead.
    pub fn dig_plot(&mut self, plot: usize, now_unix: i64) -> Option<()> {
        let held = self
            .document
            .plots
            .get(plot)
            .and_then(|plot| plot.crop.as_ref())?;
        if held.is_ready(now_unix) {
            return None;
        }
        self.document.plots.get_mut(plot)?.crop = None;
        Some(())
    }

    /// Pick every ripe crop the garden is holding, in one command.
    ///
    /// The same harvest [`Self::harvest`] pays, plot by plot, and the plots that
    /// are not ripe are left exactly as they were: a gardener who pressed the
    /// one-click harvest on a garden of half-grown beds gets the ripe ones
    /// pulled and the rest left alone, which is the difference between a button
    /// that does the obvious thing and a button that waits for a garden to be
    /// all-or-nothing.
    ///
    /// Returns the plots that were picked, in the order they stand in the
    /// garden, so a surface can say how many came up rather than only that
    /// something did.
    pub fn harvest_all_ready(&mut self, now_unix: i64) -> Vec<usize> {
        let mut picked = Vec::new();
        for index in 0..self.document.plots.len() {
            if self
                .document
                .plots
                .get(index)
                .and_then(|plot| plot.crop.as_ref())
                .is_some_and(|crop| crop.is_ready(now_unix))
                && self.harvest(index, now_unix).is_some()
            {
                picked.push(index);
            }
        }
        picked
    }

    /// Plant every seed the satchel holds, weakest grade first, until the garden
    /// runs out of beds or the satchel runs out of seeds.
    ///
    /// The order is the grade ladder the shelf and the satchel both read: 黄、
    /// 玄、地、天、神, the cheapest herb first. A stack is planted one seed at a
    /// time — [`Self::plant_one`] spends a single seed and the stack keeps the
    /// rest — so a stack of six first-tier 黄阶 seeds fills six beds, and a
    /// garden of four fills with the first four and leaves the rest in hand.
    ///
    /// A stack the odds refuse is still spent: the roll is made when the seed
    /// goes in, and a seed that did not come up has still taken its bed. That
    /// is the rule every planting follows, and a one-click planting is a run of
    /// plantings rather than a different kind.
    ///
    /// Returns how many seeds went into the ground, so a surface can say what
    /// the click did rather than leaving the gardener to count beds.
    pub fn plant_all_by_grade(&mut self, now_unix: i64) -> u32 {
        let mut planted = 0_u32;
        // The loop runs until a pass plants nothing: a stack of eight seeds is
        // eight plantings, not one, and a satchel of several stacks is read
        // again after every bed because planting spends the stacks it reads.
        loop {
            // The weakest stack still holding a seed is planted next. The
            // satchel is re-read every pass because the previous planting may
            // have spent the stack the last pass read. A pill is skipped: it
            // opens a gate from the satchel rather than a bed, so it is not a
            // planting and must not stop the run.
            let next = self
                .document
                .seeds
                .iter()
                .filter(|stack| stack.count > 0)
                .filter(|stack| stack.herb != Some(super::super::AptitudeHerb::Marrow))
                .min_by(|left, right| {
                    left.grade
                        .cmp(&right.grade)
                        .then_with(|| left.tier.cmp(&right.tier))
                        .then_with(|| left.herb.is_some().cmp(&right.herb.is_some()))
                        .then_with(|| left.attribute.index().cmp(&right.attribute.index()))
                })
                .map(|stack| (stack.attribute, stack.herb, stack.tier, stack.grade));
            let Some((attribute, herb, tier, grade)) = next else {
                break;
            };
            let outcome = match herb {
                Some(herb) => self.plant_herb_seed(herb, grade, now_unix).is_some(),
                None => self.plant(attribute, tier, grade, now_unix).is_some(),
            };
            if !outcome {
                // No bed left: the run is over, and a planting that did not
                // happen is not a reason to keep reading a satchel that
                // cannot give.
                break;
            }
            planted = planted.saturating_add(1);
            if self.document.plots.iter().all(|plot| !plot.is_empty()) {
                break;
            }
        }
        planted
    }

    /// Raise the garden one rank, paid in 灵石 and earned in harvests.
    ///
    /// Two gates rather than one, and deliberately unlike each other: the 造化
    /// comes only from planting and cannot be bought, while the stones come from
    /// *selling what was planted* and cannot be farmed. One is the gardener's
    /// patience, the other is how much of the harvest they were willing to give
    /// up for this instead. See [`rank_cost`] for why the price is what it is.
    pub fn upgrade_garden(&mut self, now_unix: i64) -> Result<u8, TradeRefusal> {
        let rank = self.document.garden_rank;
        if rank >= MAX_GARDEN_RANK {
            return Err(TradeRefusal::GardenCapped);
        }
        if self.document.garden_xp < rank_xp_total(rank + 1) {
            return Err(TradeRefusal::NotEnoughRankXp);
        }
        let price = rank_cost(rank);
        if self.document.stones < price {
            return Err(TradeRefusal::NotEnoughStones);
        }
        self.document.stones -= price;
        self.document.garden_rank = rank.saturating_add(1);
        self.push_event(
            now_unix,
            EventDetail::GardenRank {
                rank: self.document.garden_rank,
                stones: price,
            },
        );
        Ok(self.document.garden_rank)
    }

    /// Pick a ripe crop, if the plot holds one.
    ///
    /// The harvest pays its attribute points — the tier's base times the grade's
    /// multiplier — and then rolls once for a seed: the same attribute, the same
    /// tier or one higher, at a grade of its own. That roll is what keeps a
    /// garden from running dry, and it is deliberately rare next to the points
    /// the crop already paid.
    pub fn harvest(&mut self, plot: usize, now_unix: i64) -> Option<HarvestOutcome> {
        let crop = {
            let plot = self.document.plots.get_mut(plot)?;
            let ready = plot.crop.as_ref()?.is_ready(now_unix);
            if !ready {
                return None;
            }
            let crop = plot.crop.take()?;
            // A dead crop is not a harvest. The bed keeps it only until its day
            // is up, and a click that lands in that window must not pay what the
            // seed failed to earn.
            if crop.withered {
                return None;
            }
            crop
        };
        let points = harvest_points(crop.tier, crop.grade);
        // A 潜质 herb crop feeds the body on the spot — that is the whole point
        // of growing it — rather than raising an attribute. The feed itself is
        // what moves 潜质; the harvest line only reports that the herb came up
        // and what the garden banked for growing it.
        let herb = crop.herb;
        if let Some(herb) = herb {
            self.feed_found_herb(herb, crop.grade, now_unix);
        } else {
            crop.attribute.add_to(&mut self.document.crop_bonus, points);
        }
        // Counted here rather than on the seed roll: a crop that returned nothing
        // was still pulled, and the 造化 dimension is fed by pulling.
        self.document.harvests = self.document.harvests.saturating_add(1);
        // The garden learns from being gardened: the crop's own points are the
        // 造化 towards its next rank, so a herb kept is also a herb counted, and
        // there is no second currency to plant for.
        let xp_before = self.document.garden_xp;
        self.document.garden_xp = self.document.garden_xp.saturating_add(i64::from(points));
        // What the garden banked, as the log prints it: read off the bar that
        // moved rather than assumed equal to the points, so the line stays true
        // if the rank rule ever prices the two apart.
        let xp = i32::try_from(self.document.garden_xp - xp_before).unwrap_or(i32::MAX);
        // A herb crop gives no seed back — it was grown to be taken, not to seed
        // the next bed — so only an attribute crop rolls the return.
        let mut seed_name = None;
        let returned_seed = if herb.is_none() {
            let mut rng = self.rng(now_unix);
            // The return odds are the garden's own rank: one percent per 阶, so
            // the garden pays for its second seed the more it has been tended.
            let seed = seed_return(
                crop.attribute,
                crop.tier,
                self.document.garden_rank,
                &mut rng,
            );
            // The name travels with the stack it landed in, so the notice and the
            // log can both say which herb came back rather than "a seed".
            if let Some((attribute, tier, grade)) = seed {
                seed_name = self.push_seed(attribute, tier, grade, &mut rng).name;
            }
            seed.map(|(attribute, tier, grade)| SeedView {
                attribute,
                herb: None,
                tier,
                grade,
                count: 1,
                name: seed_name.clone(),
                sell_price: seed_sell_price(attribute, tier, grade),
                plant_permille: super::plant_permille(self.document.garden_rank, tier, grade),
            })
        } else {
            None
        };
        self.last_seed_return = returned_seed;
        // The line names the herb and what it paid, because "a crop came up" is
        // a fact the user cannot check against the garden that is now empty.
        self.push_event(
            now_unix,
            EventDetail::Harvested {
                attribute: crop.attribute,
                herb,
                tier: crop.tier,
                grade: crop.grade,
                name: crop.name.clone(),
                points: if herb.is_some() { 0 } else { points },
                xp,
                seed: seed_name,
            },
        );
        // The returned seed travels on the outcome the way it always did: the
        // same attribute, the same tier or one higher, at the grade rolled.
        let outcome_seed = self
            .last_seed_return
            .as_ref()
            .map(|view| (view.attribute, view.tier, view.grade));
        Some(HarvestOutcome {
            crop,
            seed: outcome_seed,
        })
    }

    /// Wear one owned item in its slot, replacing what was there.
    pub fn equip(&mut self, item_id: u64) -> bool {
        let Some(item) = self.item(item_id) else {
            return false;
        };
        let slot = item.slot.index();
        if self.document.equipped.get(slot).copied().flatten() == Some(item_id) {
            return true;
        }
        if let Some(slot) = self.document.equipped.get_mut(slot) {
            *slot = Some(item_id);
        }
        true
    }

    /// Take whatever is worn in a slot off. The item stays owned.
    pub fn unequip(&mut self, slot: EquipSlot) -> bool {
        let Some(entry) = self.document.equipped.get_mut(slot.index()) else {
            return false;
        };
        entry.take().is_some()
    }

    /// Spend 灵石 and the forge's tribute on one attempt at a piece's next star.
    ///
    /// The wager is the same shape whatever the piece: the stones and the
    /// resources are spent first, the star turns over at
    /// [`star_success_permille`] of the star the piece stands at. A failure
    /// drops only a half step: a piece with no half step takes one, dropping
    /// from a whole star to the half below, and a piece already holding a half
    /// step gives it up, dropping back to the whole. The floor is no star and
    /// no half — a piece is never walked back to nothing when its footing is
    /// already the first star's. Refused with the reason when the purse cannot
    /// cover it, the piece is not owned, or the piece already holds every star.
    pub fn star_item(&mut self, item_id: u64, now_unix: i64) -> Result<StarOutcome, TradeRefusal> {
        let Some(item) = self.item(item_id).map(|mut item| {
            item.star = item.star.min(MAX_STAR);
            item.half_star = item.half_star && item.star < MAX_STAR;
            item
        }) else {
            return Err(TradeRefusal::NotOwned);
        };
        if item.star >= MAX_STAR {
            return Err(TradeRefusal::StarCapped);
        }
        let cost = star_up_cost(&item, self.document.smithy_level);
        if self.document.stones < cost.stones {
            return Err(TradeRefusal::NotEnoughStones);
        }
        if !self.document.resources.can_spend_bag(&cost.resources) {
            return Err(TradeRefusal::NotEnoughResources);
        }
        self.document.stones = self.document.stones.saturating_sub(cost.stones);
        self.document.resources.spend_bag(&cost.resources);
        let mut rng = self.rng(now_unix);
        let success = rng.below(1_000) < u64::from(star_success_permille(item.star));
        let from = item.star;
        let from_half = item.half_star;
        //
        // A success always clears any half step the piece carried and gains a
        // full star. A failure drops only a half step: a piece with no half
        // takes one away, dropping part-way back toward the star it was
        // leaving; a piece with a half step gives that step up and drops back
        // to the whole floor. The floor is zero star, no half: a first-star
        // piece that fails stays where it is, paying for the wager alone.
        let (to, to_half) = if success {
            (from.saturating_add(1).min(MAX_STAR), false)
        } else if from_half {
            // Give up the half step: back to the whole star the piece was
            // sitting above. Already at zero when the half came from a first-
            // star failure, which is the floor.
            (from, false)
        } else if from == 0 {
            // The first star, no half: the floor. A failure costs the wager
            // and nothing else, so a piece is never walked back to nothing.
            (0, false)
        } else {
            // Take the half step: a piece drops from a whole star to the half
            // below, one step's worth of progress lost rather than two.
            (from, true)
        };
        if let Some(owned) = self
            .document
            .equipment
            .iter_mut()
            .find(|owned| owned.id == item_id)
        {
            owned.star = to;
            owned.half_star = to_half;
        }
        let outcome = StarOutcome {
            item_id,
            name: item.name.clone(),
            from,
            to,
            success,
            cost: cost.clone(),
        };
        self.star_attempts = self.star_attempts.saturating_add(1);
        self.last_star = Some(outcome.clone());
        self.push_event(
            now_unix,
            EventDetail::Star {
                name: item.name.unwrap_or_default(),
                star: to,
                success,
                cost,
            },
        );
        Ok(outcome)
    }

    /// Spend 灵石 and four materials to move a piece's secondary line onto a
    /// different attribute.
    ///
    /// The reforge never changes the piece's primary, quality, stars, set,
    /// effect or name — only the second attribute line, and only its *shape*:
    /// the total weighted worth of the piece is held constant, so the new line
    /// buys exactly what the old one bought in that attribute's own weight.
    /// A one-line piece has nothing to move and is refused with
    /// [`TradeRefusal::NoSecondaryAttribute`]; the purse can also be short of
    /// 灵石 or of the four materials.
    pub fn reforge_item(
        &mut self,
        item_id: u64,
        now_unix: i64,
    ) -> Result<ReforgeOutcome, TradeRefusal> {
        let Some(item) = self.item(item_id) else {
            return Err(TradeRefusal::NotOwned);
        };
        let Some(from) = crate::cultivation::items::secondary_attribute(&item) else {
            return Err(TradeRefusal::NoSecondaryAttribute);
        };
        let cost = super::shop::reforge_cost(&item, self.document.smithy_level)
            .ok_or(TradeRefusal::NoSecondaryAttribute)?;
        let options = crate::cultivation::items::reforge_options(&item);
        if options.is_empty() {
            return Err(TradeRefusal::NoSecondaryAttribute);
        }
        if self.document.stones < cost.stones {
            return Err(TradeRefusal::NotEnoughStones);
        }
        if !self.document.resources.can_spend_bag(&cost.resources) {
            return Err(TradeRefusal::NotEnoughResources);
        }
        let mut rng = self.rng(now_unix);
        let to = options[rng.below(options.len() as u64) as usize];
        let Some(attributes) = crate::cultivation::items::reforge_attributes(&item, to) else {
            return Err(TradeRefusal::NoSecondaryAttribute);
        };
        let from_points = from.of(item.attributes);
        let to_points = to.of(attributes);
        self.document.stones = self.document.stones.saturating_sub(cost.stones);
        self.document.resources.spend_bag(&cost.resources);
        if let Some(owned) = self
            .document
            .equipment
            .iter_mut()
            .find(|owned| owned.id == item_id)
        {
            owned.attributes = attributes;
        }
        let outcome = ReforgeOutcome {
            item_id,
            name: item.name.clone(),
            from,
            to,
            from_points,
            to_points,
            cost: cost.clone(),
        };
        self.reforge_attempts = self.reforge_attempts.saturating_add(1);
        self.last_reforge = Some(outcome.clone());
        self.push_event(
            now_unix,
            EventDetail::Reforge {
                name: item.name.unwrap_or_default(),
                from,
                to,
                from_points,
                to_points,
                cost,
            },
        );
        Ok(outcome)
    }
}
