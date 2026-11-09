//! The state machine's transitions: planting, fighting, gathering and trading.
//!
//! Split out of `state.rs` as a child module so it keeps access to the
//! document's private fields without widening their visibility.

use super::*;

impl Cultivation {
    /// Plant one owned seed in the first empty plot.
    ///
    /// The roll happens **now**, while the seed is still in hand, and never at
    /// harvest: a tenth-rank seed asks for ten days of the user's bed, and an
    /// answer ten days late is a punishment they will simply stop asking for.
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

    /// Send the cultivator out on a gathering expedition of `hours` at `site`.
    ///
    /// One outing at a time: an expedition is refused while another is under
    /// way, or while a walk or a job holds the cultivator — the same walls a
    /// walk climbs, because an expedition is hours of the same day. The yield
    /// is settled on the clock by [`Self::settle_expedition`], never on the
    /// click that set off.
    pub fn start_expedition(
        &mut self,
        site: GatherSite,
        hours: super::super::ExplorationHours,
        now_unix: i64,
    ) -> Option<ExpeditionInProgress> {
        // An expedition whose time is up is settled before a new one is judged,
        // so an old expedition already home cannot block a fresh one.
        self.settle_expedition(now_unix);
        if self.document.expedition.is_some() {
            self.note_refusal(Refusal::ExpeditionOut);
            return None;
        }
        if self.document.walk.is_some() {
            self.note_refusal(Refusal::ExpeditionOut);
            return None;
        }
        if self.document.work.is_some() {
            self.note_refusal(Refusal::ExpeditionOut);
            return None;
        }
        let expedition = ExpeditionInProgress::started(now_unix, site, hours);
        self.document.expedition = Some(expedition);
        Some(expedition)
    }

    /// The expedition under way, if the cultivator is out gathering.
    pub fn expedition(&self) -> Option<ExpeditionInProgress> {
        self.document.expedition
    }

    /// Settle the expedition that is home: pay what the realm's hourly rate made
    /// over the hours chosen, and let the cultivator be at home again.
    ///
    /// Returns whether anything was settled. An expedition still out answers
    /// `false` and nothing moves; an expedition called home comes back empty,
    /// because a called expedition is one that was not worked.
    pub fn settle_expedition(&mut self, now_unix: i64) -> bool {
        let Some(expedition) = self.document.expedition else {
            return false;
        };
        if !expedition.is_home(now_unix) {
            return false;
        }
        self.document.expedition = None;
        if expedition.early {
            // Called home: no yield, no record. The site was not worked, which
            // is exactly what it pays for having been called.
            return true;
        }
        let hours = expedition.length();
        let yield_ = expedition_yield(
            expedition.site,
            self.document.woodcut_level,
            self.document.stonecut_level,
            self.document.workshop_level,
            self.document.dojo_level,
            self.realm().index() as u32,
            hours,
        );
        let outcome = self.bank_collection(expedition.site, yield_);
        self.push_event(
            now_unix,
            EventDetail::Expedition {
                site: expedition.site,
                hours: hours.hours(),
                gained: outcome.gained.clone(),
                woodcut_xp: outcome.woodcut_xp,
                stonecut_xp: outcome.stonecut_xp,
            },
        );
        true
    }

    /// One finished gathering session: what a pomodoro spent at `site` pays.
    ///
    /// The session is worth [`gather_duration_permille`] thousandths of a base
    /// round at the cultivator's realm, so a higher realm's pomodoro gathers
    /// more — the same trained hands that would gather ten wood in an hour
    /// gather thirty-two at 仙人. The purse's ceiling applies per resource, and
    /// the trades bank xp, the same way a manual gathering does.
    pub fn collect_session(
        &mut self,
        site: GatherSite,
        now_unix: i64,
        today: CalendarDay,
    ) -> CollectOutcome {
        // The session is the realm's length, run at the realm's own rate: the
        // two are separate numbers — a 番茄钟 is 1.00 to 3.25 hours long, and
        // every one of those hours runs at 1.00 to 2.35 rounds — so a 仙人
        // session turns up 7.6 rounds against 炼气's one.
        let realm = self.realm().index() as u32;
        let duration =
            rounds_per_hour_permille(realm).saturating_mul(gather_duration_permille(realm)) / 1_000;
        let yield_ = collect_scaled(
            site,
            self.document.woodcut_level,
            self.document.stonecut_level,
            self.document.workshop_level,
            self.document.dojo_level,
            duration,
        );
        let outcome = self.bank_collection(site, yield_);
        // A gathering session is a real session: it keeps the absence clock from
        // charging the day, but it pays nothing of the 修习 ladder — no growth,
        // no 功法 turn, no 灵石 cut, no 试炼 chance, no roll.
        let today_number = today.day_number();
        self.document.last_active_day = Some(today_number);
        self.document.missed_streak = 0;
        self.document.charged_through_day = Some(today_number);
        self.push_event(
            now_unix,
            EventDetail::SessionGather {
                site,
                gained: outcome.gained.clone(),
                woodcut_xp: outcome.woodcut_xp,
                stonecut_xp: outcome.stonecut_xp,
            },
        );
        outcome
    }

    /// Bank a collection into the purse and the trades: the 仓库's ceiling per
    /// resource, the xp, and the levels the xp buys. Shared by a manual click
    /// and a finished gathering session, so the two pay the same way.
    fn bank_collection(&mut self, site: GatherSite, yield_: CollectionYield) -> CollectOutcome {
        // The 仓库's ceiling is per resource: a purse at the top of one kind
        // keeps the find of the other, so a full purse is never a lost day.
        let cap = resource_cap(self.document.warehouse_level);
        let mut gained = ResourceBag::default();
        for resource in Resource::ALL {
            let room = cap.saturating_sub(self.document.resources.get(resource));
            gained.add(resource, yield_.gained.get(resource).min(room));
        }
        self.document.resources.add_bag(&gained);
        // The trades bank xp; the while-loop lets a big 道场 climb two ranks.
        self.document.woodcut_xp = self.document.woodcut_xp.saturating_add(yield_.woodcut_xp);
        self.document.stonecut_xp = self.document.stonecut_xp.saturating_add(yield_.stonecut_xp);
        while self.document.woodcut_level < MAX_TRADE_LEVEL
            && skill_ready(self.document.woodcut_level, self.document.woodcut_xp)
        {
            self.document.woodcut_level += 1;
        }
        while self.document.stonecut_level < MAX_TRADE_LEVEL
            && skill_ready(self.document.stonecut_level, self.document.stonecut_xp)
        {
            self.document.stonecut_level += 1;
        }
        let outcome = CollectOutcome {
            site,
            gained,
            woodcut_xp: yield_.woodcut_xp,
            stonecut_xp: yield_.stonecut_xp,
            woodcut_level: self.document.woodcut_level,
            stonecut_level: self.document.stonecut_level,
        };
        self.last_collect = Some(outcome.clone());
        outcome
    }

    /// What a finished pomodoro pays. One thing and one thing only: a session
    /// either cultivates, or gathers at one site, never both.
    pub fn session_mode(&self) -> SessionMode {
        self.document.session_mode
    }

    /// Choose what the next finished pomodoro pays.
    pub fn set_session_mode(&mut self, mode: SessionMode) {
        self.document.session_mode = mode;
    }

    /// Raise one building a rank for 灵石 and the gathered tribute.
    ///
    /// Refused at the top of a building's ladder, or when the purse cannot
    /// cover the rank's whole price — a rank is atomic, so a purse with the
    /// money but not the ore is told it is short of the ore.
    pub fn upgrade_building(
        &mut self,
        building: Building,
        now_unix: i64,
    ) -> Result<BuildingOutcome, TradeRefusal> {
        let level = self.building_level(building);
        if level >= building.max_level() {
            return Err(TradeRefusal::BuildingCapped);
        }
        let cost = building_upgrade_cost(building, level);
        if self.document.stones < cost.stones {
            return Err(TradeRefusal::NotEnoughStones);
        }
        if !self.document.resources.can_spend_bag(&cost.resources) {
            return Err(TradeRefusal::NotEnoughResources);
        }
        self.document.stones = self.document.stones.saturating_sub(cost.stones);
        self.document.resources.spend_bag(&cost.resources);
        *self.building_level_mut(building) = level.saturating_add(1);
        let outcome = BuildingOutcome {
            building,
            from: level,
            to: level.saturating_add(1),
            cost,
        };
        self.last_building = Some(outcome.clone());
        self.push_event(
            now_unix,
            EventDetail::BuildingUpgrade {
                building,
                level: outcome.to,
                cost: outcome.cost.clone(),
            },
        );
        Ok(outcome)
    }

    /// Raise a building up to `times` ranks in one command.
    ///
    /// Each rank is a full atomic upgrade — the same price and the same refusals
    /// a single one meets — and the loop stops at the first the purse cannot
    /// cover or at the top of the ladder. The ranks actually bought come back,
    /// and the only refusal a surface hears is a click that bought nothing.
    pub fn upgrade_building_many(
        &mut self,
        building: Building,
        times: u8,
        now_unix: i64,
    ) -> Result<Vec<BuildingOutcome>, TradeRefusal> {
        let mut bought = Vec::new();
        for _ in 0..times {
            match self.upgrade_building(building, now_unix) {
                Ok(outcome) => bought.push(outcome),
                Err(refusal) if bought.is_empty() => return Err(refusal),
                Err(_) => break,
            }
        }
        Ok(bought)
    }

    /// The rank one building stands at.
    pub(crate) fn building_level(&self, building: Building) -> u8 {
        match building {
            Building::Workshop => self.document.workshop_level,
            Building::Warehouse => self.document.warehouse_level,
            Building::Smithy => self.document.smithy_level,
            Building::Dojo => self.document.dojo_level,
        }
    }

    /// The rank one building stands at, writable.
    fn building_level_mut(&mut self, building: Building) -> &mut u8 {
        match building {
            Building::Workshop => &mut self.document.workshop_level,
            Building::Warehouse => &mut self.document.warehouse_level,
            Building::Smithy => &mut self.document.smithy_level,
            Building::Dojo => &mut self.document.dojo_level,
        }
    }

    /// The resource economy as the settings page reads it.
    fn resources_view(&self) -> ResourcesView {
        let trade = |level: u8, xp: u32| TradeView {
            level,
            xp,
            xp_to_next: skill_xp_total(level.saturating_add(1)),
        };
        let building = |building: Building, level: u8| BuildingView {
            level,
            capped: level >= building.max_level(),
            upgrade_cost: (level < building.max_level())
                .then(|| building_upgrade_cost(building, level)),
        };
        ResourcesView {
            bag: self.document.resources.clone(),
            cap: resource_cap(self.document.warehouse_level),
            woodcut: trade(self.document.woodcut_level, self.document.woodcut_xp),
            stonecut: trade(self.document.stonecut_level, self.document.stonecut_xp),
            workshop: building(Building::Workshop, self.document.workshop_level),
            warehouse: building(Building::Warehouse, self.document.warehouse_level),
            smithy: building(Building::Smithy, self.document.smithy_level),
            dojo: building(Building::Dojo, self.document.dojo_level),
            expedition: self.document.expedition.map(|expedition| ExpeditionView {
                site: expedition.site,
                hours: expedition.hours,
                ends_unix: expedition.ends_unix,
                early: expedition.early,
            }),
        }
    }

    /// Mark the day's morning event as seen, so the window does not replay it.
    ///
    /// A moment the surface owns: the popup is dismissed once and the day is
    /// written down, which is what lets a window opened again the same
    /// afternoon stay quiet about a fight that was already shown.
    pub fn acknowledge_daily_event(&mut self, day: i64) {
        self.document.daily_event_seen_day = Some(day);
    }

    /// Fight the guardian standing at the end of the current realm.
    ///
    /// Returns `None` when there is nothing to fight: the realm is not at its
    /// last layer, or the ladder has no next realm. A loss costs nothing but
    /// the attempt; a win opens the next realm, lifts the growth to that
    /// realm's floor and pays one piece of the new tier's gear.
    pub fn breakthrough(&mut self, now_unix: i64) -> Option<BreakthroughOutcome> {
        let current = self.realm();
        if self.level() < current.cap_level() {
            return None;
        }
        if !breakthrough_ready(self.document.breakthrough_blocked_until, now_unix) {
            return None;
        }
        let guardian = guardian_for(current)?;
        // The count is taken here, where a fight is actually joined: it is what
        // tells a surface there is a report it has not shown yet, and "the
        // snapshot revision moved" cannot say that — a running pomodoro bumps
        // it every second.
        self.breakthrough_attempts = self.breakthrough_attempts.saturating_add(1);
        let mut rng = self.rng(now_unix);
        let report = resolve(self.attributes(), guardian);
        // The fight is what decides, not the number above: the panel prints the
        // comparison so the user knows where they stand, and then the two of
        // them actually trade blows.
        let fighter = self.fighter(&mut rng);
        // The guardian is the same opponent every time it is met: its power is
        // printed before the fight, so a roll per attempt would make that number
        // a lie. What it wears is what makes it harder than the naked rung, and
        // that gear is what a win takes.
        let rival = opponent::guardian(current, guardian.target);
        let duel = self.duel_with_effects(fighter, rival.fighter.clone(), &mut rng);
        // Won fights are what train the 战斗 dimension; a loss is not counted
        // there, and only ever shows up in the record below.
        self.record_duel(duel.won, FightKind::Guardian, now_unix);
        if !duel.won {
            self.document.breakthrough_failures =
                self.document.breakthrough_failures.saturating_add(1);
            self.document.breakthrough_blocked_until =
                Some(now_unix.saturating_add(BREAKTHROUGH_COOLDOWN_SECONDS));
            // A loss can cost a piece of gear. One in four, and only one: the
            // point is that a loss stings, not that it sets the cultivator back.
            let lost = (rng.below(1_000) < LOST_GEAR_PERMILLE)
                .then(|| self.drop_one_item(&mut rng))
                .flatten();
            let outcome = BreakthroughOutcome {
                report,
                target: guardian.target,
                reward: None,
                duel,
                lost,
                granted_art: None,
                stones: None,
                seed: None,
            };
            self.last_breakthrough = Some(outcome.clone());
            // The realm it failed to open is still named: a loss is history
            // too, and one line with the realm on it beats two kinds of line.
            self.push_event(
                now_unix,
                EventDetail::Breakthrough {
                    won: false,
                    stage: Some(guardian.target.index() as u8),
                    stones: None,
                    gear: None,
                    art: None,
                    seed: None,
                },
            );
            return Some(outcome);
        }
        // The guardian is the strongest thing on the page, so a win here is
        // where the pill's odds are best — and it is rolled before the realm
        // opens, so the newest line in the log is the find rather than the
        // breakthrough it came with.
        self.roll_marrow_from_combat(guardian.power, &mut rng, now_unix);
        self.enter_realm(guardian, &rival, duel, now_unix);
        let outcome = self.last_breakthrough.clone();
        // The log is written after the settlement so the line carries the
        // realm that was opened *and* everything that came with it — purse,
        // 功法, gear and herb: the event is one line about one event, not two
        // half-lines about a fight and a buy. A win that left its finds out of
        // the log left a bag the user could not check against it.
        self.push_event(
            now_unix,
            EventDetail::Breakthrough {
                won: true,
                stage: Some(guardian.target.index() as u8),
                stones: outcome.as_ref().and_then(|outcome| outcome.stones),
                gear: outcome
                    .as_ref()
                    .and_then(|outcome| outcome.reward.as_ref())
                    .and_then(|item| item.name.clone()),
                art: outcome
                    .as_ref()
                    .and_then(|outcome| outcome.granted_art.clone()),
                seed: outcome.as_ref().and_then(|outcome| outcome.seed.clone()),
            },
        );
        outcome
    }

    /// Roll one 洗髓丹 after a won fight.
    ///
    /// The odds climb with the opponent's power relative to the cultivator's —
    /// one in a hundred thousand at parity, one in a thousand against something
    /// three times as strong — so the pill is the reward for punching up rather
    /// than for farming the easiest thing on the page.
    fn roll_marrow_from_combat(&mut self, rival_power: u32, rng: &mut Rng, now_unix: i64) -> bool {
        let player = self.power();
        let ppm = super::super::aptitude::marrow_combat_ppm(rival_power, player);
        if rng.below(1_000_000) >= ppm {
            return false;
        }
        self.document.marrow_pills = self.document.marrow_pills.saturating_add(1);
        self.push_event(
            now_unix,
            EventDetail::MarrowFound {
                source: MarrowSource::Combat,
            },
        );
        true
    }

    /// Roll one 洗髓丹 out of a quiet source: a walk that met nobody, or the
    /// day's own event. One in ten thousand, the same number the shelf uses.
    fn roll_marrow_from_fortune(&mut self, rng: &mut Rng, now_unix: i64) -> bool {
        if rng.below(super::super::aptitude::MARROW_FORTUNE_ONE_IN) != 0 {
            return false;
        }
        self.document.marrow_pills = self.document.marrow_pills.saturating_add(1);
        self.push_event(
            now_unix,
            EventDetail::MarrowFound {
                source: MarrowSource::Fortune,
            },
        );
        true
    }

    /// Roll whether a defeat costs the cultivator a piece of their own gear.
    ///
    /// A loss is meant to sting: the walk is already forfeit, and on top of it
    /// the road now and then takes a piece of what the cultivator was carrying.
    /// The odds are well below a victory's find rate — a defeat is the price of
    /// losing, not a worse way to win — so a loss costs a piece one time in five
    /// and the other four are only the walk. A piece worn is taken off its slot
    /// as it goes, so the satchel and the body never disagree about what is on.
    pub(crate) fn roll_defeat_drop(&mut self, rng: &mut Rng, now_unix: i64) {
        if rng.below(1_000) >= super::super::explore::DEFEAT_DROP_PERMILLE {
            return;
        }
        let named: Vec<usize> = self
            .document
            .equipment
            .iter()
            .enumerate()
            .filter(|(_, item)| item.name.is_some())
            .map(|(i, _)| i)
            .collect();
        if named.is_empty() {
            return;
        }
        // Every named piece is a candidate: a loss that always took the oldest
        // would be a loss the user could plan around.
        let index = named[rng.below(named.len() as u64) as usize];
        let item: EquipmentRecord = self.document.equipment.remove(index);
        // A piece that was worn leaves its slot, so the body reads as it is.
        if let Some(slot) = self.document.equipped.get_mut(item.slot.index())
            && *slot == Some(item.id)
        {
            *slot = None;
        }
        let name = item.name.clone().unwrap_or_default();
        self.push_event(now_unix, EventDetail::EquipmentLost { name });
    }

    /// Write one fought fight into the record, newest first.
    /// A win is also what trains the 战斗 dimension, so the count and the line of
    /// history are written together: a record that disagreed with the dimension
    /// would be two truths about the same fight.
    fn record_duel(&mut self, won: bool, kind: FightKind, now_unix: i64) {
        if won {
            self.document.battle_wins = self.document.battle_wins.saturating_add(1);
        }
        self.document.duel_record.insert(
            0,
            DuelRecord {
                won,
                kind,
                realm: self.realm().index() as u8,
                at_unix: now_unix,
            },
        );
        self.document.duel_record.truncate(DUEL_RECORD_CAP);
    }

    /// Write one event into the page's log, newest first.
    ///
    /// The log is a window like the duel record: sixteen lines is what a page
    /// reads at a glance, and a save that kept every event since the first
    /// pomodoro would be a diary nobody opens.
    pub(super) fn push_event(&mut self, now_unix: i64, detail: EventDetail) {
        self.document.events.insert(
            0,
            EventRecord {
                at_unix: now_unix,
                detail,
            },
        );
        self.document.events.truncate(EVENT_LOG_CAP);
    }

    /// The multiplier every finished session is currently worth, in parts per
    /// thousand.
    ///
    /// Read from the four disciplines rather than carried, so the page and the
    /// formula cannot disagree about what a session pays.
    pub(super) fn growth_multiplier_permille(&self) -> u32 {
        growth_permille(
            self.realm(),
            self.trained_art_rank(),
            self.document.explorations,
            self.document.battle_wins,
            self.document.harvests,
        )
    }

    /// The cultivator as a fighter, with the 外功 they would use.
    pub(crate) fn fighter(&self, rng: &mut Rng) -> Fighter {
        let derived = self.derived();
        let art = self.strike(rng);
        Fighter::from_derived(PLAYER_NAME.to_owned(), derived, art)
    }

    /// The duel the cultivator fights, with the effects their 神装 and their
    /// complete set carry.
    ///
    /// An effect is a debuff the other side walks in with, read when the duel
    /// is built: the number the panel shows the user before they press the
    /// button has to be the number that decides, and a hidden post-fight
    /// adjustment would make the shown numbers a lie about the fight that
    /// happened.
    pub(crate) fn duel_with_effects(
        &self,
        fighter: Fighter,
        mut rival: Fighter,
        rng: &mut Rng,
    ) -> DuelReport {
        for (effect, depth) in self.active_effects() {
            let bite = |value: u32| -> u32 {
                (u64::from(value) * u64::from(1_000 - depth) / 1_000) as u32
            };
            match effect {
                GearEffect::Frost => {
                    rival.attack = bite(rival.attack);
                    rival.hit = bite(rival.hit);
                }
                GearEffect::Thunder => rival.speed = bite(rival.speed),
                GearEffect::Sunder => rival.resist = bite(rival.resist),
                GearEffect::Weaken => rival.health = bite(rival.health),
            }
        }
        duel(fighter, rival, rng)
    }

    /// The debuffs the body carries into a fight: the deepest 神装 effect,
    /// plus the mastery of a complete four-piece set.
    ///
    /// The set's is an *extra* effect, not a replacement: the four debuffs
    /// name different numbers of the rival, so a complete set always bites
    /// something on its own. If both name the same effect, only the deeper
    /// bite is kept — a set is not a second layer of the same debuff.
    pub(crate) fn active_effects(&self) -> Vec<(GearEffect, u32)> {
        let mut effects: Vec<(GearEffect, u32)> = Vec::new();
        if let Some((effect, rarity)) = self.strongest_effect() {
            effects.push((effect, effect.depth_permille(rarity)));
        }
        let worn: Vec<_> = self.equipped_items().collect();
        if let Some(effect) = crate::cultivation::items::gear_set_mastery_effect(&worn) {
            let depth = effect.depth_permille(Rarity::Divine);
            if let Some(existing) = effects.iter_mut().find(|(other, _)| *other == effect) {
                existing.1 = existing.1.max(depth);
            } else {
                effects.push((effect, depth));
            }
        }
        effects
    }

    /// The effect the worn gear carries, if any: the deepest one, and the
    /// quality it came from.
    fn strongest_effect(&self) -> Option<(GearEffect, Rarity)> {
        self.equipped_items()
            .filter_map(|item| item.effect.map(|effect| (effect, item.rarity)))
            .max_by_key(|(_, rarity)| *rarity)
    }

    /// The 外功 the cultivator brings to a fight: randomly one of the three
    /// strongest they can swing.
    ///
    /// "Strongest" is what one swing is worth: the multiplier the 外功 lands,
    /// with the mastery share folded in for one learned to the end. A book
    /// still part-trained competes on its multiplier alone, and one that has
    /// not been trained to its first 阶 does not compete at all — owning a
    /// book is not swinging it. The training's selection does not pre-empt the
    /// roll either: the strongest three fight it out, not the book the session
    /// happens to be reading. With fewer than three the roll runs over what
    /// there is, and with none the fists do the work.
    fn strike(&self, rng: &mut Rng) -> Option<ArtStrike> {
        let mut candidates: Vec<(u64, u8, u32)> = self
            .document
            .arts
            .iter()
            .filter(|owned| blueprint(owned.art_id).kind == ArtKind::External && owned.rank >= 1)
            .map(|owned| {
                let art = blueprint(owned.art_id);
                (owned.art_id, owned.rank, art.multiplier_permille)
            })
            .collect();
        for id in &self.document.mastered_arts {
            let art = blueprint(*id);
            if art.kind == ArtKind::External {
                candidates.push((
                    *id,
                    art.grade.ranks(),
                    art.multiplier_permille + mastery_permille(art.grade),
                ));
            }
        }
        if candidates.is_empty() {
            return None;
        }
        candidates.sort_by_key(|&(_, _, strength)| std::cmp::Reverse(strength));
        candidates.truncate(3);
        let (art_id, rank, _) = candidates[rng.below(candidates.len() as u64) as usize];
        let art = blueprint(art_id);
        Some(ArtStrike {
            name: art.name,
            multiplier_permille: art.multiplier_permille,
            cost: art.cost,
            // The move as the book prints it: how many blows a use throws, the
            // band each is drawn from, and what a landing one lays on. The mark
            // is the fight's, not the book's — it is spent in the fight the 外功
            // is carried into, and a fresh fight starts unmarked.
            hits: art.hits,
            band_permille: art.band_permille,
            effect: art.effect,
            marked: false,
            // A completed 外功 spends its mastery share on every blow it lands;
            // an art one 阶 short of its last pays only its multiplier.
            mastery_permille: if rank >= art.grade.ranks() {
                mastery_permille(art.grade)
            } else {
                0
            },
        })
    }

    /// Give up one owned item and say which. Equipped or not, one item only —
    /// and the cultivator is never left with nothing they can wear.
    fn drop_one_item(&mut self, rng: &mut Rng) -> Option<String> {
        if self.document.equipment.is_empty() {
            return None;
        }
        let index = rng.below(self.document.equipment.len() as u64) as usize;
        let item = self.document.equipment.remove(index);
        let name = item
            .name
            .clone()
            .unwrap_or_else(|| item.slot.name_key().to_owned());
        self.document
            .equipped
            .iter_mut()
            .filter(|slot| **slot == Some(item.id))
            .for_each(|slot| *slot = None);
        Some(name)
    }

    /// Open the realm the guardian defended and pay its trophy.
    pub(super) fn enter_realm(
        &mut self,
        guardian: Guardian,
        rival: &Opponent,
        duel: DuelReport,
        now_unix: i64,
    ) {
        let target = guardian.target;
        // The power that fought, read before anything moves: entering the realm
        // lifts the level ceiling and pays a trophy, so a report read afterwards
        // would credit the win to the reward rather than to the gear.
        let player_power = self.power();
        // A cultivator who held the realm back keeps the growth they earned,
        // but the part that was banked past the old realm's cap is only half
        // carried into the new one. The cap is the last level of the realm
        // they just left; anything above it is surplus, and surplus is taxed
        // at 50%. The new floor is the realm they just opened, so the final
        // growth is the greater of that floor and the old cap plus half the
        // surplus.
        let old_cap = threshold_for(self.realm().cap_level());
        let new_floor = stage_floor(target.index());
        let surplus = self.document.growth.saturating_sub(old_cap);
        let inherited = surplus / 2;
        self.document.growth = new_floor.max(old_cap + inherited);
        self.document.realm = target.index() as u8;
        self.document.breakthrough_failures = 0;
        // The realm lifts the level ceiling, so the layers the growth already
        // bought become real levels — and real free points — now.
        self.note_peak_level();
        let mut rng = self.rng(0);
        // The guardian's own strength, not the realm it guarded: what a win pays
        // is read off the body that was beaten, which is why a guardian met at
        // its realm's cap pays at that realm's depth.
        let strength = Strength::of(rival.attributes.power());
        // The trophy is the piece the guardian was wearing, not a fresh roll:
        // what the fight was against and what it leaves behind are one thing.
        let reward = match rival.gear.first() {
            Some(item) => Some(self.adopt_equipment(item.clone())),
            // A guardian met bare-handed (it can happen at the shallow end,
            // where a piece of gear would outweigh the whole body) still pays
            // for the realm it opened.
            None => self.push_equipment(strength, &mut rng),
        };
        // A win always pays a 功法 — that is the promise the fight is made for —
        // and it is the book the guardian was swinging.
        let granted_art = self.take_art(rival, target, now_unix);
        // The herb is named here rather than only folded into the bag: it is the
        // rarest thing a realm hands over, and a log that stayed silent about it
        // left the user finding a seed they could not account for.
        let mut seed = None;
        if rng.below(1_000) < VICTORY_SEED_PERMILLE {
            let attribute = Attribute::from_index(rng.below(Attribute::ALL.len() as u64) as usize);
            let grade = roll_seed_grade(&mut rng);
            seed = self
                .push_seed(attribute, strength.tier(), grade, &mut rng)
                .name;
        }
        // The guardian's purse, drawn after the 功法 and the gear because those
        // are what a breakthrough is *for*: the money is what a realm's treasury
        // adds on top, not the reason to fight.
        let stones = Some(roll_stones(
            &mut rng,
            GUARDIAN_STONES,
            target.index() as u8 + 1,
        ));
        if let Some(amount) = stones {
            self.document.stones = self.document.stones.saturating_add(amount);
        }
        let outcome = BreakthroughOutcome {
            report: BattleReport {
                player_power,
                guardian_power: guardian.power,
                won: true,
            },
            target,
            reward,
            duel,
            lost: None,
            granted_art,
            stones,
            seed,
        };
        self.last_breakthrough = Some(outcome);
    }

    /// What beating someone on the road pays, handed to the satchel and
    /// reported.
    ///
    /// Shared by the walk and the morning ambush because it is the same
    /// encounter either way: the same weighting, the same strength, the same
    /// three kinds. Two copies of this roll would be two chances for one of them
    /// to drift out of tune with the panel that explains it.
    pub(super) fn award_wanderer_find(
        &mut self,
        rival: &Opponent,
        rng: &mut Rng,
        now_unix: i64,
    ) -> ExplorationFindView {
        let mut found = ExplorationFindView::default();
        // A beaten opponent is the source, so what it hands over is read off its
        // own strength rather than off the realm the fight happened in: beating
        // something bigger than the save is how a save gets gear from deeper in.
        let strength = Strength::of(rival.attributes.power());
        match roll_victory(rng) {
            // What the roll decides is *which* of the beaten fighter's things
            // comes home with you — the things themselves were never a fresh
            // roll, they were on the body that lost.
            ExplorationOutcome::Art => {
                found.art = self.take_art(rival, self.realm(), now_unix);
            }
            ExplorationOutcome::Gear => {
                if let Some(item) = self.take_gear(rival, strength, rng) {
                    found.gear = Some(self.item_view(&item));
                }
            }
            ExplorationOutcome::Seed => {
                let attribute =
                    Attribute::from_index(rng.below(Attribute::ALL.len() as u64) as usize);
                let grade = roll_seed_grade(rng);
                found.seed = Some(self.push_seed(attribute, strength.tier(), grade, rng));
            }
            _ => {}
        }
        found
    }

    /// Set out on one walk, for the length chosen.
    ///
    /// Nothing is paid here: the road pays when the walk comes home, which is
    /// [`Self::settle_walk`] and the worker's tick. Returns the walk that
    /// started, and `None` while another is still out — with the refusal noted
    /// so the page can say why. A walk whose time has already passed is settled
    /// first, so a save that was never ticked (a launch after a night away) can
    /// still set out the moment it is opened.
    pub fn explore(
        &mut self,
        now_unix: i64,
        hours: super::super::ExplorationHours,
    ) -> Option<super::super::WalkInProgress> {
        let region = super::super::explore::Region::from_realm(self.realm());
        self.explore_region(now_unix, hours, region)
    }

    /// Set out for a chosen region.
    ///
    /// The road is open to the realm the cultivator stands in and every realm
    /// below it: a save that has climbed to 金丹 may still walk 炼气's road for
    /// its goods, and a save cannot walk ahead of itself. A cleared region stays
    /// open — walking it again pays the ordinary purses, and the one-off haul
    /// for first clearing it has already been taken.
    pub fn explore_region(
        &mut self,
        now_unix: i64,
        hours: super::super::ExplorationHours,
        region: super::super::explore::Region,
    ) -> Option<super::super::WalkInProgress> {
        self.settle_walk(now_unix);
        if self.document.walk.is_some() {
            self.note_refusal(Refusal::ExploreCooldown);
            return None;
        }
        // An expedition is hours of the same day, so the road is closed while
        // the cultivator is out gathering.
        if self.document.expedition.is_some() {
            self.note_refusal(Refusal::ExpeditionOut);
            return None;
        }
        // A job holds the cultivator the way a walk does — it is hours of the
        // same day — so the road is closed while one is being worked.
        if self.document.work.is_some() {
            self.note_refusal(Refusal::Working);
            return None;
        }
        // A region ahead of the cultivator's own realm is a road they have not
        // reached: the ladder is climbed, not skipped.
        if region.index() > self.realm().index() {
            self.note_refusal(Refusal::RegionLocked);
            return None;
        }
        // The walk starts one level above whatever is already cleared in the
        // region, capped at the region's top, so a walk never over-levels it.
        let cleared = self
            .document
            .explored_levels
            .get(region.index())
            .copied()
            .unwrap_or(0);
        let level = (cleared + 1).min(super::super::explore::MAX_LEVEL);
        let walk =
            super::super::WalkInProgress::started(now_unix, hours, region.index() as u8, level);
        self.document.last_exploration_unix = Some(now_unix);
        // The length is written down beside the instant, because the page draws
        // the chip the user picked last: a four-hour walk read back as a
        // twelve-hour one would draw the wrong chip under the right countdown.
        self.document.last_exploration_hours = hours.hours();
        // The choice is remembered: the page reopens on the road last walked,
        // and the next walk offers it again.
        self.document.exploration_region = region.index() as u8;
        self.document.walk = Some(walk);
        Some(walk)
    }

    /// Call the outing under way home early — a walk or a shift at a job.
    ///
    /// Whichever is holding the cultivator, the rule is the same one: the find
    /// is forfeit (the road is not walked, the work is not worked), and the trip
    /// back is a third of the time already spent. So calling an outing back is a
    /// way to stop paying attention to it rather than a way to undo it, and the
    /// page can offer the same button whether the cultivator is on the road or
    /// at a job.
    pub fn return_early(&mut self, now_unix: i64) -> bool {
        if let Some(expedition) = self.document.expedition {
            if expedition.early {
                return false;
            }
            self.document.expedition = Some(expedition.called_home(now_unix));
            return true;
        }
        if let Some(walk) = self.document.walk {
            if walk.early {
                return false;
            }
            self.document.walk = Some(walk.called_home(now_unix));
            return true;
        }
        if let Some(work) = self.document.work {
            if work.early {
                return false;
            }
            self.document.work = Some(work.called_home(now_unix));
            return true;
        }
        self.note_refusal(Refusal::NotOut);
        false
    }

    /// Settle the walk that is home: roll what it turned up, pay it, and let the
    /// cultivator be at home again.
    ///
    /// Returns whether anything was settled. A walk still out answers `false`
    /// and nothing moves; a walk called home comes back empty, because a called
    /// walk is a walk that was not taken.
    pub fn settle_walk(&mut self, now_unix: i64) -> bool {
        let Some(walk) = self.document.walk else {
            return false;
        };
        if !walk.is_home(now_unix) {
            return false;
        }
        self.document.walk = None;
        if walk.early {
            // Called home: no find, no purse, no record. The walk did not
            // happen, which is exactly what it pays for having been called.
            self.document.last_exploration_unix = Some(now_unix);
            return true;
        }
        let hours = walk.length();
        let region_index = walk.region;
        let level = walk.level;
        let region = super::super::explore::Region::from_index(region_index as usize);
        self.document.last_exploration_unix = Some(now_unix);
        // Counted here rather than on the outcome: a walk that found nothing is
        // still a walk a surface has not shown yet.
        self.document.explorations = self.document.explorations.saturating_add(1);
        // The fight the last walk met is forgotten before this one is settled:
        // a walk that meets nobody must carry no fight into its popup, or the
        // popup replays an older walk's battle as this walk's verdict — which
        // is how a quiet afternoon that turned up a handful of stones came to
        // be announced as 败北 under a won fight's log.
        self.last_exploration_duel = None;
        let mut rng = self.rng(now_unix);
        // The roll now reads the region and the level: combat comes first, with
        // the region's and level's own encounter rate, and the quiet remainder
        // is the usual seed-and-gear-and-art split.
        let mut outcome =
            super::super::explore::roll_region_walk(&mut rng, hours, region_index, level);
        // The strength is the region's rolled tier band: a walk at the region's
        // tier finds tier-matched goods, and the tier is raised as the level
        // steps up. 境界越高, 奖励越多, and the tier band is what the goods
        // follow.
        let strength = Strength::of(super::super::drop::tier_base_power(region));
        let mut found = ExplorationFindView::default();
        match outcome {
            ExplorationOutcome::Seed => {
                let attribute =
                    Attribute::from_index(rng.below(Attribute::ALL.len() as u64) as usize);
                let grade = roll_seed_grade(&mut rng);
                found.seed = Some(self.push_seed(attribute, strength.tier(), grade, &mut rng));
            }
            ExplorationOutcome::Gear => {
                if let Some(item) = self.push_equipment(strength, &mut rng) {
                    found.gear = Some(self.item_view(&item));
                }
            }
            ExplorationOutcome::Art => {
                // Which grade turns up is the discovery roll, not the realm's
                // band: a walk can hand a beginner a 神阶 book, at the same
                // ten-thousandth the shelf and the guardian use.
                let grade = {
                    let mut grade_rng = self.rng(now_unix);
                    super::super::arts::roll_art_grade(&mut grade_rng)
                };
                found.art = self.grant_art(grade, now_unix).map(|art| art.name);
            }
            ExplorationOutcome::Victory => {
                let fighter = self.fighter(&mut rng);
                // A wanderer is rolled for the region and level: nobody promised
                // who the road would send, and its numbers are only ever shown
                // after the fact.
                let rival =
                    super::super::explore::region_wanderer(region, level, rng.below(u64::MAX));
                let duel = self.duel_with_effects(fighter, rival.fighter.clone(), &mut rng);
                if duel.won {
                    found = self.award_wanderer_find(&rival, &mut rng, now_unix);
                    // A beaten wanderer can leave a 洗髓丹 behind: the odds are
                    // read off how far it outclassed the cultivator.
                    self.roll_marrow_from_combat(rival.attributes.power(), &mut rng, now_unix);
                } else {
                    outcome = ExplorationOutcome::Defeat;
                    // The loss is not free: the road now and then takes a piece
                    // of the cultivator's own gear, so a defeat costs more than
                    // the walk it was.
                    self.roll_defeat_drop(&mut rng, now_unix);
                }
                self.record_duel(duel.won, FightKind::Wanderer, now_unix);
                self.last_exploration_duel = Some(duel);
            }
            ExplorationOutcome::Defeat
            | ExplorationOutcome::Nothing
            | ExplorationOutcome::Stones => {
                // A victory hand above is the only thing that advances the
                // level; every other outcome leaves the level where it stood.
            }
        }
        // A kind that was rolled but paid nothing is a walk that found nothing:
        // the quality ladder's remainder is exactly the find that did not
        // happen, and a line saying "found gear" over an empty satchel would be
        // a promise the roll broke.
        if outcome == ExplorationOutcome::Gear && found.gear.is_none() {
            outcome = ExplorationOutcome::Nothing;
        }
        // A quiet walk is a random event like the morning's: one in ten thousand
        // turns up a 洗髓丹. A fight's own roll is above, and a loss finds
        // nothing — the pill is not a consolation prize.
        if !matches!(
            outcome,
            ExplorationOutcome::Victory | ExplorationOutcome::Defeat
        ) {
            self.roll_marrow_from_fortune(&mut rng, now_unix);
        }
        // How far this walk moves the region: the body read against what the
        // floor asks where the region currently stands. A defeat moves nothing.
        // The gain is read in tenths of a percent — the unit the slot carries
        // and the page prints — so a walk that moves half a percent is `0.5%`
        // rather than a parts-per-thousand number the surface would round.
        let progress_before = self
            .document
            .region_progress
            .get(region.index())
            .copied()
            .unwrap_or(0);
        let requirement = super::super::explore::region_requirement(region, progress_before * 10);
        let gain = if outcome == ExplorationOutcome::Defeat {
            0
        } else {
            super::super::explore::walk_gain_tenths(&mut rng, hours, self.power(), requirement)
        };
        // 灵石 are rolled beside the finding rather than in place of it: an
        // outing can come back with a herb and a handful of stones both, and
        // the purse is fed by the region and level the walk was taken at.
        //
        // Which purse is what tells the two outings apart: a quiet walk walks
        // the region's own purse, and beating a wanderer pays the combat purse
        // sitting above it. A walk that was beaten pays nothing at all — a purse
        // that filled up on a defeat would make being beaten a way of earning.
        //
        // The purse then follows the ground covered: a standard walk — ten per
        // mille of the region — pays one purse, and a walk that moved five times
        // as far pays five. The floor is one purse, so a body that barely
        // cleared the floor still comes home with something.
        let share = u64::from(gain.max(10) / 10);
        let purse = if outcome == ExplorationOutcome::Victory {
            super::super::explore::combat_purse(region, hours, level)
        } else {
            super::super::explore::walk_purse(region, hours, level)
        }
        .scaled(share * 1_000);
        let stones = if outcome == ExplorationOutcome::Defeat {
            None
        } else {
            Some(roll_stones(&mut rng, purse, region.tier()))
        };
        if let Some(amount) = stones {
            self.document.stones = self.document.stones.saturating_add(amount);
            found.stones = Some(amount);
            // A walk that turned up stones and nothing else reads as having
            // turned up stones, not as having turned up nothing.
            if outcome == ExplorationOutcome::Nothing {
                outcome = ExplorationOutcome::Stones;
            }
        }
        // The walk moves the region by what the body earned. Crossing the last
        // level pays the one-off haul for having walked the road end to end.
        self.document.last_walk_gain = gain;
        if outcome != ExplorationOutcome::Defeat {
            self.accumulate_progress(region.index(), gain);
            self.first_clear_haul(region, &mut rng, now_unix);
        }
        // The settlement is written before the popup is shown and outlives it: the
        // popup is forgotten on quit, the page's record is not.
        self.document.last_walk = Some(WalkRecord {
            at_unix: now_unix,
            realm: self.realm().index() as u8,
            outcome,
            region: walk.region,
            level: walk.level,
            art: found.art.clone(),
            gear: found.gear.as_ref().and_then(|item| item.name.clone()),
            seed: found.seed.as_ref().and_then(|seed| seed.name.clone()),
            stones,
        });
        self.last_exploration = Some(outcome);
        self.last_exploration_found = Some(found);
        // The walk joins the page's log beside the popups: finds and all, so a
        // week of walks reads as a week rather than as one line of "last walk".
        self.push_event(
            now_unix,
            EventDetail::Walk {
                outcome,
                art: self.document.last_walk.as_ref().and_then(|w| w.art.clone()),
                gear: self
                    .document
                    .last_walk
                    .as_ref()
                    .and_then(|w| w.gear.clone()),
                seed: self
                    .document
                    .last_walk
                    .as_ref()
                    .and_then(|w| w.seed.clone()),
                stones,
                gain,
            },
        );
        true
    }

    /// Mark a level of a region as cleared — only advances, never retreats — so
    /// a walk that beats the level's fight leaves the rung below it standing and
    /// the next walk one level deeper. A save that has never walked a region's
    /// first level holds a zero in its slot, and a walk at the region's cap
    /// level stays at the cap: a rung is cleared by standing above it, and a
    /// cap holds steady.
    fn advance_explored_level(&mut self, region_index: usize, level: u8) {
        let slot = self
            .document
            .explored_levels
            .get_mut(region_index)
            .expect("explored_levels slot exists for every region");
        if level > *slot {
            *slot = level;
        }
    }

    /// Add a walk's progress to the region's current level. When the progress
    /// crosses a whole level — a hundred tenths of a percent — the level is
    /// cleared and the excess rolls into the next.
    fn accumulate_progress(&mut self, region_index: usize, gain: u32) {
        let slot = self
            .document
            .region_progress
            .get_mut(region_index)
            .expect("region_progress slot exists for every region");
        let total = slot.saturating_add(gain);
        let level_cleared = total / super::super::explore::PROGRESS_TENTHS_PER_LEVEL;
        *slot = total % super::super::explore::PROGRESS_TENTHS_PER_LEVEL;
        for _ in 0..level_cleared {
            let level = self
                .document
                .explored_levels
                .get(region_index)
                .copied()
                .unwrap_or(0);
            if level < super::super::explore::MAX_LEVEL {
                self.advance_explored_level(region_index, level + 1);
            }
        }
    }

    /// The one-off haul for walking a region end to end, the first time.
    ///
    /// A cleared region stays open: walking it again pays its ordinary purses,
    /// which is what a region below one's realm is for. What is paid once is
    /// the haul for having walked the road to its end — a purse worth twenty
    /// long walks, and one piece of the realm's finest gear at the region's own
    /// tier, so the haul matches the road it came from.
    fn first_clear_haul(
        &mut self,
        region: super::super::explore::Region,
        rng: &mut Rng,
        now_unix: i64,
    ) {
        let index = region.index();
        if self.cleared_level(index) < super::super::explore::MAX_LEVEL {
            return;
        }
        if self
            .document
            .cleared_regions
            .get(index)
            .copied()
            .unwrap_or(false)
        {
            return;
        }
        if let Some(slot) = self.document.cleared_regions.get_mut(index) {
            *slot = true;
        }
        let purse = super::super::explore::walk_purse(
            region,
            super::super::ExplorationHours::Twelve,
            super::super::explore::MAX_LEVEL,
        )
        .scaled(20_000);
        let stones = roll_stones(rng, purse, region.tier());
        self.document.stones = self.document.stones.saturating_add(stones);
        let id = self.next_id();
        let item = roll_equipment(id, region.tier(), super::super::Rarity::Divine, rng);
        let gear = item.name.clone();
        self.document.equipment.push(item);
        self.push_event(
            now_unix,
            EventDetail::RegionCleared {
                region: index as u8,
                gear,
                stones,
            },
        );
    }

    /// Every region the page lists, shallowest first.
    ///
    /// A region is listed once the realm has reached it: the road is walked down
    /// the ladder, so a save sees the region it stands in and every one below.
    /// The rows carry what the walk needs — the completion, the level it stands
    /// at and what that level asks — because the page prints all three and a
    /// surface that computed them itself could disagree with the walk.
    fn region_views(&self) -> Vec<super::RegionView> {
        let reached = self.realm().index();
        super::super::explore::Region::ALL
            .into_iter()
            .enumerate()
            .map(|(index, region)| {
                let cleared_levels = self.cleared_level(index);
                // The slot carries tenths of a percent, so the level's progress
                // is read straight off it rather than converted from parts per
                // thousand. A whole level is a hundred tenths.
                let level_progress = self
                    .document
                    .region_progress
                    .get(index)
                    .copied()
                    .unwrap_or(0)
                    .min(super::super::explore::PROGRESS_TENTHS_PER_LEVEL);
                let walked = u32::from(cleared_levels)
                    * super::super::explore::PROGRESS_TENTHS_PER_LEVEL
                    + level_progress;
                super::RegionView {
                    region,
                    cleared_levels,
                    level_progress,
                    // The whole region's completion, in tenths of a percent:
                    // ten levels of a hundred tenths each is a thousand, so a
                    // cleared region reads `100.0%` rather than `100%`.
                    percent: walked.min(1_000),
                    level: super::super::explore::region_level_at(region, level_progress * 10),
                    requirement: super::super::explore::region_requirement(
                        region,
                        level_progress * 10,
                    ),
                    cleared: self
                        .document
                        .cleared_regions
                        .get(index)
                        .copied()
                        .unwrap_or(false),
                    unlocked: index <= reached,
                    current: usize::from(self.document.exploration_region) == index,
                }
            })
            .collect()
    }

    /// The furthest level cleared in a region. A zero means the region has not
    /// been entered yet.
    pub fn cleared_level(&self, region_index: usize) -> u8 {
        self.document
            .explored_levels
            .get(region_index)
            .copied()
            .unwrap_or(0)
    }

    /// The board of jobs on offer, for a local day.
    ///
    /// The board is a function of the day and of the realm the cultivator is
    /// worth rather than a stored list: a board written three realms ago would be
    /// pocket change today, and a board a user never opened should not be the one
    /// they are handed next week. The counter is what a paid re-roll moves.
    pub fn jobs(&self, day: i64) -> Vec<super::super::JobOffer> {
        let tier = super::super::drop::tier_for_power(self.power());
        let mut rng = Rng::seeded(
            (day as u64)
                .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                .wrapping_add(
                    u64::from(self.document.jobs_roll).wrapping_mul(0xBF58_476D_1CE4_E5B9),
                ),
        );
        super::super::jobs::roll_jobs(&mut rng, tier)
    }

    /// Take the job in that slot of the board.
    ///
    /// Refused while the cultivator is out on a walk or already at a job: both are
    /// hours of the same day, and a cultivator who can be in two places at once
    /// has no reason to choose between them.
    pub fn accept_job(
        &mut self,
        slot: usize,
        now_unix: i64,
        day: i64,
    ) -> Result<super::super::JobRun, Refusal> {
        if self.document.walk.is_some() {
            return Err(Refusal::TrialOut);
        }
        // An expedition is hours of the same day, so the board is closed while
        // the cultivator is out gathering.
        if self.document.expedition.is_some() {
            return Err(Refusal::ExpeditionOut);
        }
        if self.document.work.is_some() {
            return Err(Refusal::Working);
        }
        let Some(offer) = self.jobs(day).get(slot).copied() else {
            return Err(Refusal::JobGone);
        };
        let run = super::super::JobRun::started(offer, now_unix);
        self.document.work = Some(run);
        Ok(run)
    }

    /// Pay for a fresh board.
    ///
    /// The price is what stops the board being a slot machine: three draws at a
    /// ten-thousandth, re-rolled until a 神阶 job turns up, would cost nothing at
    /// all if this were free.
    pub fn refresh_jobs(&mut self) -> Result<u32, TradeRefusal> {
        let tier = super::super::drop::tier_for_power(self.power());
        let cost = super::super::jobs::refresh_cost(tier);
        if self.document.stones < cost {
            return Err(TradeRefusal::NotEnoughStones);
        }
        self.document.stones = self.document.stones.saturating_sub(cost);
        self.document.jobs_roll = self.document.jobs_roll.saturating_add(1);
        Ok(cost)
    }

    /// Settle a job that has come due. Returns what it paid.
    ///
    /// Called by the worker's tick the way a walk's return is: a job is hours of
    /// the cultivator's day, so it comes home on the clock rather than on a click,
    /// and an app that was shut for the whole of it has to pay on the next launch
    /// rather than lose the hours.
    pub fn settle_work(&mut self, now_unix: i64) -> Option<WorkPaid> {
        let run = self.document.work?;
        if !run.is_done(now_unix) {
            return None;
        }
        self.document.work = None;
        // A shift called home is a shift not worked: no wage, no point, no
        // record, and no fresh board — the job in hand is the one that was left,
        // so calling a shift back cannot be a free reroll either.
        if run.early {
            return None;
        }
        // The wage is the figure the board printed, and the wage is the wage:
        // nothing about how the work came off changes what it was. What the work
        // decides is the lasting gain, off the cultivator's hold on the
        // attribute the job asked for.
        let wage = run.offer().stones();
        let grade = run.grade;
        let kind = run.kind;
        let level = self.level();
        let success_rate =
            super::super::jobs::success_rate_permille(kind, self.derived(), level, grade);
        let mut rng = self.rng(now_unix);
        // The gain: one point to as many as the grade, at a chance that is the
        // success rate over twenty — rolled whether the work came off or not,
        // because a trade teaches even when it does not pay well.
        let chance = super::super::jobs::point_chance_permille(success_rate);
        let gain = if rng.below(1_000) < chance {
            let slot = super::super::jobs::reward_slot(&mut rng, kind);
            let points = super::super::jobs::points(&mut rng, grade);
            slot.add_to(&mut self.document.job_bonus, points);
            Some((slot, points))
        } else {
            None
        };
        self.document.stones = self.document.stones.saturating_add(wage);
        // A finished job hands over a new board: the outing is over, and the next
        // three are what the cultivator walks back into.
        self.document.jobs_roll = self.document.jobs_roll.saturating_add(1);
        let paid = WorkPaid {
            stones: wage,
            hours: run.hours,
            grade,
            kind,
            success_rate,
            gain,
        };
        self.last_work = Some(paid);
        self.push_event(
            now_unix,
            EventDetail::Work {
                stones: wage,
                hours: run.hours,
                grade,
                success_rate,
                gain,
            },
        );
        Some(paid)
    }

    /// Whether the cultivator is at home right now: no walk and no expedition
    /// is under way.
    ///
    /// The walk and the expedition are their own cooldowns — the road pays when
    /// it comes home, the site pays when the expedition ends — so being home is
    /// the whole gate. An outing whose time has passed is settled by the
    /// worker's tick, and by [`Self::explore`] and [`Self::start_expedition`]
    /// on the way in, so this never says "out" about one that is already back.
    pub fn exploration_ready(&self, now_unix: i64) -> bool {
        match self.document.walk {
            // A walk whose time is up is home whether or not the tick has come
            // round to settle it yet: a page that said "still out" about a walk
            // that is back would be reading the clock wrong, not the walk.
            Some(walk) => walk.is_home(now_unix),
            None => match self.document.expedition {
                // The same reading for an expedition: a page that said "still
                // out" about one already back would be lying the same way.
                Some(expedition) => expedition.is_home(now_unix),
                None => true,
            },
        }
    }

    /// The walk under way, if the cultivator is out.
    pub fn walk(&self) -> Option<super::super::WalkInProgress> {
        self.document.walk
    }

    /// The instant the walk under way comes home, while one is out.
    pub fn walk_home_at(&self) -> Option<i64> {
        self.document.walk.map(|walk| walk.ends_unix)
    }

    /// The floor the next attempt climbs.
    pub fn trial_next_floor(&self) -> u16 {
        self.document.trial_floor.saturating_add(1)
    }

    /// How many floors the local day still offers.
    ///
    /// Reset by the day rather than by elapsed time: five floors belong to a
    /// date the way the shelf does, so a save that has not played since
    /// yesterday is owed five today whatever the hour.
    pub fn trial_attempts_left(&self, day: i64) -> u32 {
        if self.document.trial_day != Some(day) {
            return TRIAL_ATTEMPTS_PER_DAY;
        }
        TRIAL_ATTEMPTS_PER_DAY.saturating_sub(self.document.trial_attempts_today)
    }

    /// Whether a job in hand is still holding the cultivator.
    ///
    /// The same reading the road applies to a walk: an outing whose hours have
    /// passed is over whether or not the tick has come round to settle it, so a
    /// fight refused because of a job that is already done would be reading the
    /// clock wrong. The tick pays the wage; this only says where the
    /// cultivator is, and the tower and the 试炼 ask it the same question.
    fn working(&self, now_unix: i64) -> bool {
        self.document
            .work
            .is_some_and(|work| !work.is_done(now_unix))
    }

    /// What calling the outing under way home would cost, in seconds.
    ///
    /// The same price for both kinds of outing, read off the one shared rule: a
    /// third of the time already spent. Zero while at home, so a caller can read
    /// it without asking whether anyone is out first.
    fn return_trip_now(&self, now_unix: i64) -> i64 {
        if let Some(expedition) = self
            .document
            .expedition
            .filter(|expedition| !expedition.is_home(now_unix))
        {
            return super::super::explore::return_trip_seconds(
                now_unix.saturating_sub(expedition.started_unix),
            );
        }
        if let Some(walk) = self.document.walk.filter(|walk| !walk.is_home(now_unix)) {
            return super::super::explore::return_trip_seconds(
                now_unix.saturating_sub(walk.started_unix),
            );
        }
        if let Some(work) = self.document.work.filter(|work| !work.is_done(now_unix)) {
            return super::super::explore::return_trip_seconds(
                now_unix.saturating_sub(work.started_unix),
            );
        }
        0
    }

    /// Why the tower is closed right now, or `None` when a floor can be climbed.
    ///
    /// Four different walls, and the page prints which one it hit: out on a
    /// walk, at a job, out of the day's attempts, or past the last floor. A
    /// button that went quiet without saying which would read as broken.
    pub fn trial_blocked(&self, now_unix: i64, day: i64) -> Option<TrialBlocked> {
        if self.trial_next_floor() > TRIAL_MAX_FLOOR {
            return Some(TrialBlocked::Top);
        }
        if !self.exploration_ready(now_unix) {
            return Some(TrialBlocked::Out);
        }
        if self.working(now_unix) {
            return Some(TrialBlocked::Working);
        }
        if self.trial_attempts_left(day) == 0 {
            return Some(TrialBlocked::Drained);
        }
        None
    }

    /// Why the 试炼 cannot be fought right now, or `None` when it can.
    ///
    /// The two home fights ask the same question in the same order, so the page
    /// and the domain cannot disagree about the reason: still on the road, then
    /// still at work, then out of chances.
    pub fn proving_blocked(&self, now_unix: i64) -> Option<ProvingBlocked> {
        if !self.exploration_ready(now_unix) {
            return Some(ProvingBlocked::Out);
        }
        if self.working(now_unix) {
            return Some(ProvingBlocked::Working);
        }
        if self.document.proving_tokens == 0 {
            return Some(ProvingBlocked::Spent);
        }
        None
    }

    /// Climb one floor of the 试炼 tower.
    ///
    /// The gate is the day's attempts and the cultivator's whereabouts, not a
    /// cost: a floor is refused while a walk is still out — the tower is climbed
    /// at home — and while the day's five are spent. A win clears the floor,
    /// moves the frontier up one and rolls what it turned up; a loss costs
    /// nothing but the attempt, and leaves the frontier where it stood, so the
    /// tower is a wall to be worn down rather than a ladder that drops.
    pub fn trial(&mut self, now_unix: i64, day: i64) -> Option<TrialOutcome> {
        // A walk that is home is settled first: the courtyard is closed to a
        // walk that is *out*, not to one whose find has not been read yet. The
        // same for the job in hand — the fight waits for the road, not for the
        // tick that pays the wage.
        self.settle_walk(now_unix);
        self.settle_work(now_unix);
        if let Some(blocked) = self.trial_blocked(now_unix, day) {
            self.note_refusal(blocked.refusal());
            return None;
        }
        // The day is stamped on the first floor of it, so the count the page
        // reads and the count the next attempt spends cannot disagree.
        if self.document.trial_day != Some(day) {
            self.document.trial_day = Some(day);
            self.document.trial_attempts_today = 0;
        }
        let floor = self.trial_next_floor();
        self.document.trial_attempts_today = self.document.trial_attempts_today.saturating_add(1);
        self.document.trials = self.document.trials.saturating_add(1);
        self.trial_attempts = self.trial_attempts.saturating_add(1);
        let mut rng = self.rng(now_unix);
        let fighter = self.fighter(&mut rng);
        // A floor's warden is drawn from a band around the floor's own rung each
        // attempt: the page prints the band's middle, and the duel settles the
        // draw — so the climb is a gamble at the frontier, not a formality.
        let rival = opponent::warden(floor, &mut rng);
        let duel = self.duel_with_effects(fighter, rival.fighter.clone(), &mut rng);
        self.record_duel(duel.won, FightKind::Trial, now_unix);
        let mut found = ExplorationFindView::default();
        if duel.won {
            self.document.trial_floor = floor.min(TRIAL_MAX_FLOOR);
            // The warden is the source, so what the floor pays is read off the
            // body that defended it: a floor reached past the save's own realm
            // hands over that realm's goods, which is how the tower is a way
            // down the ladder rather than a second one.
            let strength = Strength::of(rival.attributes.power());
            match roll_reward(&mut rng) {
                TrialReward::Seed => {
                    let attribute =
                        Attribute::from_index(rng.below(Attribute::ALL.len() as u64) as usize);
                    let grade = roll_seed_grade(&mut rng);
                    found.seed = Some(self.push_seed(attribute, strength.tier(), grade, &mut rng));
                }
                TrialReward::Gear => {
                    if let Some(item) = self.take_gear(&rival, strength, &mut rng) {
                        found.gear = Some(self.item_view(&item));
                    }
                }
                TrialReward::Art => {
                    found.art = self.take_art(&rival, floor_realm(floor), now_unix);
                }
                TrialReward::Nothing => {}
            }
            // The floor's purse, drawn beside the finding: a cleared floor
            // always pays a little, so a run of empty rolls is still a run that
            // bought something.
            // Opened at the floor's own realm, so a deeper floor pays more even
            // within one realm's climb.
            if let Some(amount) = Some(roll_stones(
                &mut rng,
                TRIAL_STONES,
                floor_realm(floor).index() as u8 + 1,
            )) {
                self.document.stones = self.document.stones.saturating_add(amount);
                found.stones = Some(amount);
            }
        }
        self.push_event(
            now_unix,
            EventDetail::Trial {
                floor,
                won: duel.won,
                art: found.art.clone(),
                gear: found.gear.as_ref().and_then(|item| item.name.clone()),
                seed: found.seed.as_ref().and_then(|seed| seed.name.clone()),
                stones: found.stones,
            },
        );
        let outcome = TrialOutcome {
            floor,
            warden_power: warden_power(floor),
            duel,
            found,
        };
        self.last_trial = Some(outcome.clone());
        Some(outcome)
    }

    /// Whether the next floor would be a sweep rather than a fight: the
    /// cultivator is past the warden by a margin the floor cannot answer —
    /// [`TRIAL_SWEEP_MARGIN_PERMILLE`] over what the floor prints — so the
    /// floor is no contest and the tower offers the skip. A save merely dressed
    /// at its own level is not offered it: the sweep is for a body that has
    /// outgrown the ladder, not for one that stands on it.
    pub fn trial_sweepable(&self) -> bool {
        let floor = self.trial_next_floor();
        if floor > TRIAL_MAX_FLOOR {
            return false;
        }
        // Measured against the number the page prints beside the floor, so the
        // skip and the display cannot disagree about how strong the floor is.
        // That number is the band's middle: the sweep is offered when even the
        // hard end of the draw would be a formality.
        self.power() > warden_power(floor).saturating_mul(TRIAL_SWEEP_MARGIN_PERMILLE) / 1_000
    }

    /// Sweep the next stretch of the tower: one overwhelmingly strong
    /// cultivator clears up to ten floors in one action, the fights skipped
    /// because none of them was a contest, and every swept floor's reward paid
    /// at once.
    ///
    /// The same walls the single climb answers to close the sweep too, and one
    /// sweep spends one of the day's attempts — the floors are the reward for
    /// being the stronger body, not a second ration of the day.
    pub fn trial_sweep(&mut self, now_unix: i64, day: i64) -> Option<TrialSweepOutcome> {
        self.settle_walk(now_unix);
        self.settle_work(now_unix);
        if let Some(blocked) = self.trial_blocked(now_unix, day) {
            self.note_refusal(blocked.refusal());
            return None;
        }
        let from_floor = self.trial_next_floor();
        if !self.trial_sweepable() {
            self.note_refusal(Refusal::TrialNotSweepable);
            return None;
        }
        if self.document.trial_day != Some(day) {
            self.document.trial_day = Some(day);
            self.document.trial_attempts_today = 0;
        }
        let floors = 10u16.min(TRIAL_MAX_FLOOR.saturating_sub(from_floor).saturating_add(1));
        self.document.trial_attempts_today = self.document.trial_attempts_today.saturating_add(1);
        self.document.trials = self.document.trials.saturating_add(1);
        self.trial_attempts = self.trial_attempts.saturating_add(1);
        let mut rng = self.rng(now_unix);
        let mut found = TrialSweepOutcome {
            from_floor,
            floors,
            ..TrialSweepOutcome::default()
        };
        for step in 0..floors {
            let floor = from_floor + step;
            let rival = opponent::warden(floor, &mut rng);
            let strength = Strength::of(rival.attributes.power());
            match roll_reward(&mut rng) {
                TrialReward::Seed => {
                    let attribute =
                        Attribute::from_index(rng.below(Attribute::ALL.len() as u64) as usize);
                    let grade = roll_seed_grade(&mut rng);
                    self.push_seed(attribute, strength.tier(), grade, &mut rng);
                    found.seeds = found.seeds.saturating_add(1);
                }
                TrialReward::Gear => {
                    if self.take_gear(&rival, strength, &mut rng).is_some() {
                        found.gear = found.gear.saturating_add(1);
                    }
                }
                TrialReward::Art => {
                    if self
                        .take_art(&rival, floor_realm(floor), now_unix)
                        .is_some()
                    {
                        found.arts = found.arts.saturating_add(1);
                    }
                }
                TrialReward::Nothing => {}
            }
            // The floor's purse, drawn beside the finding: a cleared floor
            // always pays a little, so a swept stretch always buys something.
            let amount = roll_stones(&mut rng, TRIAL_STONES, floor_realm(floor).index() as u8 + 1);
            self.document.stones = self.document.stones.saturating_add(amount);
            found.stones = found.stones.saturating_add(amount);
        }
        self.document.trial_floor = (from_floor + floors - 1).min(TRIAL_MAX_FLOOR);
        self.push_event(
            now_unix,
            EventDetail::TrialSweep {
                from_floor,
                floors,
                seeds: found.seeds,
                gear: found.gear,
                arts: found.arts,
                stones: found.stones,
            },
        );
        let outcome = found.clone();
        self.last_trial_sweep = Some(outcome.clone());
        Some(outcome)
    }

    /// Spend one 试炼 chance on a fight tied to the road the cultivator walks.
    ///
    /// The courtyard is fought at home, so the same two walls the tower has
    /// close it too: a walk still on the road and a job still in hand are hours
    /// of the same day, and the cultivator is only in one place at a time. What
    /// this fight adds is its own ration — a banked chance, one per finished
    /// pomodoro, ten at most — and the opponent is drawn from the current
    /// region's own requirement, multiplied by
    /// [`super::proving::TRIAL_CHALLENGE_MIN_PERMILLE`] to
    /// [`super::proving::TRIAL_CHALLENGE_MAX_PERMILLE`]: a 试炼 is a fight
    /// tied to the road, not a second ladder.
    ///
    /// A win pays the road's own purse for **one hour** at that region — the
    /// same table a walk reads — so a finished pomodoro is worth a short walk
    /// on the road the save is already on. The find is the 锁妖塔's base table,
    /// so the two differ in when they pay, not in what.
    pub fn proving(&mut self, now_unix: i64) -> Option<ProvingOutcome> {
        // The walk that is home is settled before the courtyard is asked, the
        // way the tower does it: the gate is where the cultivator is, not
        // whether the tick has come round.
        self.settle_walk(now_unix);
        self.settle_work(now_unix);
        if let Some(blocked) = self.proving_blocked(now_unix) {
            self.note_refusal(blocked.refusal());
            return None;
        }
        self.document.proving_tokens = self.document.proving_tokens.saturating_sub(1);
        self.provings = self.provings.saturating_add(1);
        let mut rng = self.rng(now_unix);
        let fighter = self.fighter(&mut rng);
        // The region the 试炼 is tied to is the one the cultivator stands in:
        // the road it is walking, not a region picked from the ladder.
        let region = super::super::explore::Region::from_realm(self.realm());
        let level = self
            .cleared_level(region.index())
            .saturating_add(1)
            .min(super::super::explore::MAX_LEVEL);
        // The opponent stands where the region's progress stands, multiplied
        // by a draw between one and a half and three: the road's own
        // requirement is the floor, and the spread is what makes a 试炼 a fight
        // rather than a formality. The requirement is read in parts per
        // thousand, which is what the region's progress is converted to here.
        let progress = self
            .document
            .region_progress
            .get(region.index())
            .copied()
            .unwrap_or(0);
        let requirement = super::super::explore::region_requirement(region, progress * 10);
        let span = super::super::proving::TRIAL_CHALLENGE_MAX_PERMILLE
            - super::super::proving::TRIAL_CHALLENGE_MIN_PERMILLE
            + 1;
        let challenge =
            super::super::proving::TRIAL_CHALLENGE_MIN_PERMILLE + rng.below(u64::from(span)) as u32;
        let target = requirement.saturating_mul(challenge) / 1_000;
        let tier = region.tier();
        // The opponent is levelled to the target rather than to the cultivator:
        // the fight is tied to the road, so its body is the road's own
        // requirement through the draw, not the save's own rung.
        let name = super::super::trial::TRIAL_OPPONENT_NAME.to_owned();
        let seed = rng.below(u64::MAX);
        // `level_near` returns the rung whose body is nearest the target; the
        // full opponent is then wound up from that rung, the way a floor's
        // warden is.
        let rung = opponent::level_near(target, tier, name.clone(), seed);
        let rival = opponent::opponent(opponent::Stance::Wanderer, rung, tier, name, seed);
        let duel = self.duel_with_effects(fighter, rival.fighter.clone(), &mut rng);
        self.record_duel(duel.won, FightKind::Wanderer, now_unix);
        let mut found = ExplorationFindView::default();
        if duel.won {
            let strength = Strength::of(rival.attributes.power());
            match roll_reward(&mut rng) {
                TrialReward::Seed => {
                    let attribute =
                        Attribute::from_index(rng.below(Attribute::ALL.len() as u64) as usize);
                    let grade = roll_seed_grade(&mut rng);
                    found.seed = Some(self.push_seed(attribute, strength.tier(), grade, &mut rng));
                }
                TrialReward::Gear => {
                    if let Some(item) = self.take_gear(&rival, strength, &mut rng) {
                        found.gear = Some(self.item_view(&item));
                    }
                }
                TrialReward::Art => {
                    found.art = self.take_art(&rival, self.realm(), now_unix);
                }
                TrialReward::Nothing => {}
            }
            // The purse is the road's own for one hour at the region the 试炼
            // is tied to: a finished pomodoro is worth a short walk on the road
            // the save is already on, and the purse follows the road's tier
            // exactly as a walk's does.
            let stones = roll_stones(
                &mut rng,
                super::super::explore::proving_purse(region, level),
                tier,
            );
            self.document.stones = self.document.stones.saturating_add(stones);
            found.stones = Some(stones);
            // A beaten challenger can leave a 洗髓丹 behind, on the same odds
            // every other fight rolls: the pill is not a tower's privilege.
            self.roll_marrow_from_combat(rival.attributes.power(), &mut rng, now_unix);
        }
        self.push_event(
            now_unix,
            EventDetail::Proving {
                won: duel.won,
                art: found.art.clone(),
                gear: found.gear.as_ref().and_then(|item| item.name.clone()),
                seed: found.seed.as_ref().and_then(|seed| seed.name.clone()),
                stones: found.stones,
            },
        );
        let outcome = ProvingOutcome {
            duel,
            found,
            opponent_power: rival.attributes.power(),
            region: region.index() as u8,
        };
        self.last_proving = Some(outcome.clone());
        Some(outcome)
    }

    /// What the day opened with, once per local day.
    ///
    /// Called by the worker's tick, not by a click: the day has to arrive on its
    /// own, whether the machine was launched this morning or left running over
    /// midnight. `None` on every tick after the first of a day, so the trick is
    /// idempotent without the caller having to remember anything.
    ///
    /// An ambush is a real fight — the wanderer off the road, the same engine and
    /// the same record — and a gift or a coin is paid straight into the satchel.
    /// Nothing is ever taken away: the morning can only add.
    pub fn daily_event(&mut self, now_unix: i64, day: i64) -> Option<DailyOutcome> {
        if self.document.daily_event_day == Some(day) {
            return None;
        }
        self.document.daily_event_day = Some(day);
        self.daily_events = self.daily_events.saturating_add(1);
        let mut rng = self.rng(now_unix);
        let kind = roll_kind(&mut rng);
        let mut outcome = DailyOutcome {
            kind,
            won: true,
            duel: None,
            found: ExplorationFindView::default(),
        };
        let tier = self.realm().index() as u8 + 1;
        match kind {
            DailyKind::Ambush => {
                let fighter = self.fighter(&mut rng);
                let rival = opponent::wanderer(self.realm(), rng.below(u64::MAX));
                let duel = self.duel_with_effects(fighter, rival.fighter.clone(), &mut rng);
                self.record_duel(duel.won, FightKind::Wanderer, now_unix);
                outcome.won = duel.won;
                if duel.won {
                    outcome.found = self.award_wanderer_find(&rival, &mut rng, now_unix);
                    // The ambush is a fight like any other, so it rolls the
                    // pill on the same odds: the stronger the ambusher, the
                    // better the chance it was carrying one.
                    self.roll_marrow_from_combat(rival.attributes.power(), &mut rng, now_unix);
                    let amount = roll_stones(&mut rng, DAILY_AMBUSH_STONES, tier);
                    self.document.stones = self.document.stones.saturating_add(amount);
                    outcome.found.stones = Some(amount);
                }
                outcome.duel = Some(duel);
            }
            DailyKind::Gift => match roll_gift(&mut rng) {
                GiftKind::Stones => {
                    let amount = roll_stones(&mut rng, DAILY_GIFT_STONES, tier);
                    self.document.stones = self.document.stones.saturating_add(amount);
                    outcome.found.stones = Some(amount);
                }
                GiftKind::Seed => {
                    let attribute =
                        Attribute::from_index(rng.below(Attribute::ALL.len() as u64) as usize);
                    // A gift is the quiet kind of source, so it is the
                    // cultivator's own strength that decides what the day hands
                    // over: nothing was fought for it.
                    let strength = Strength::of(self.power());
                    let grade = roll_seed_grade(&mut rng);
                    outcome.found.seed =
                        Some(self.push_seed(attribute, strength.tier(), grade, &mut rng));
                }
                GiftKind::Gear => {
                    let strength = Strength::of(self.power());
                    match self.push_equipment(strength, &mut rng) {
                        Some(item) => outcome.found.gear = Some(self.item_view(&item)),
                        // A gift always hands something over — that is what makes
                        // it a gift — so a wardrobe that rolled up empty becomes
                        // the purse it would have been worth.
                        None => {
                            let amount = roll_stones(&mut rng, DAILY_GIFT_STONES, tier);
                            self.document.stones = self.document.stones.saturating_add(amount);
                            outcome.found.stones = Some(amount);
                        }
                    }
                }
            },
            DailyKind::Trinket => {
                let amount = roll_stones(&mut rng, DAILY_TRINKET_STONES, tier);
                self.document.stones = self.document.stones.saturating_add(amount);
                outcome.found.stones = Some(amount);
            }
        }
        // A gift or a trinket is a random event, so it carries the same one in
        // ten thousand the shelf does. An ambush rolls its own odds above, and a
        // lost ambush finds nothing.
        if !matches!(kind, DailyKind::Ambush) {
            self.roll_marrow_from_fortune(&mut rng, now_unix);
        }
        let found = outcome.found.clone();
        self.push_event(
            now_unix,
            EventDetail::Daily {
                kind,
                won: outcome.won,
                art: found.art,
                gear: found.gear.as_ref().and_then(|item| item.name.clone()),
                seed: found.seed.as_ref().and_then(|seed| seed.name.clone()),
                stones: found.stones,
            },
        );
        self.last_daily_event = Some(outcome.clone());
        Some(outcome)
    }

    /// The guardian offered right now, or `None` while the realm still has
    /// layers to climb.
    pub fn guardian(&self) -> Option<Guardian> {
        let realm = self.realm();
        (self.level() >= realm.cap_level())
            .then(|| guardian_for(realm))
            .flatten()
    }

    /// Read the snapshot a surface renders, without a clock.
    ///
    /// Only the tests reach for this: a surface has a clock and asks for
    /// [`Self::view_on`], so the cooldowns it reads are the real ones.
    pub fn view(&self) -> CultivationView {
        self.view_on(0, 0)
    }

    /// Read the snapshot a surface renders.
    ///
    /// The local day is passed in rather than derived from the timestamp,
    /// because the day is a local one: the session counter resets on it, and
    /// the shelf is rolled from it. A surface that worked the day out for
    /// itself could be a timezone's worth of hours out — which would show
    /// yesterday's shelf and price the next pomodoro as the second of the day
    /// when it is really the first.
    pub fn view_on(&self, now_unix: i64, day: i64) -> CultivationView {
        let realm = self.realm();
        let level = self.level();
        let said = layer_rank(level);
        let floor = threshold_for(level);
        let next = (level < realm.cap_level()).then(|| threshold_for(level + 1));
        let level_attributes = self.level_attributes();
        let equipment_attributes = self.equipment_attributes();
        let crop_attributes = self.crop_attributes();
        let allocated_attributes = self.allocated_attributes();
        let art_attributes = self.art_attributes();
        let attributes = level_attributes
            .plus(equipment_attributes)
            .plus(crop_attributes)
            .plus(allocated_attributes)
            .plus(art_attributes);
        let bonus = self.equipment_bonus().plus(self.art_bonus());
        let attribute_derived = attributes.derived();
        // 潜质 is its own fractional number now, not a slice of 根骨: the
        // outright lines are folded into it by the one reader, so the rate a
        // row prints and the roll a session makes are the same value.
        let aptitude_tenths = self.aptitude_tenths();
        let derived = attribute_derived.plus(bonus);
        let arts: Vec<ArtView> = self
            .document
            .arts
            .iter()
            .map(|art| {
                art_view(
                    art,
                    aptitude_tenths,
                    self.document.selected_art == Some(art.art_id),
                    self.document.dojo_level,
                )
            })
            .collect();
        let mut mastered_arts: Vec<MasteredArtView> = self
            .document
            .mastered_arts
            .iter()
            .map(|id| MasteredArtView {
                sold: self.document.sold_mastered_arts.contains(id),
                ..mastered_art_view(*id)
            })
            .collect();
        // A book trained to its first 阶 is 已习得 as far as it has gone: it
        // reports here with the 阶 it stands at and no mastery yet, while the
        // shelf keeps the training and the selling.
        for owned in &self.document.arts {
            if owned.rank == 0 {
                continue;
            }
            mastered_arts.push(MasteredArtView {
                rank: owned.rank,
                max_rank: blueprint(owned.art_id).grade.ranks(),
                complete: false,
                mastery_permille: 0,
                sold: false,
                ..mastered_art_view(owned.art_id)
            });
        }
        // Furthest along first: the three on the page are the books nearest to
        // paying, and a 神阶 art at its ninth 阶 of twenty-seven is not nearer
        // done than a 黄阶 art at its second of three — so the ratio is what
        // ranks them, not the 阶. Equal ratios rank the better grade, because a
        // 神阶 half-read is worth more than a 黄阶 half-read; a tie there falls
        // back to the recent ring, which keeps the rest in the order they were
        // last worked on.
        let recent = &self.document.recent_arts;
        let rank_of = |id: u64| {
            recent
                .iter()
                .position(|held| *held == id)
                .unwrap_or(usize::MAX)
        };
        mastered_arts.sort_by(|left, right| {
            // rank/max_rank compared by cross-multiplication, so the ratio stays
            // exact in integers and no float ever decides the order.
            let left_share = u32::from(left.rank) * u32::from(right.max_rank);
            let right_share = u32::from(right.rank) * u32::from(left.max_rank);
            right_share
                .cmp(&left_share)
                .then_with(|| right.grade.cmp(&left.grade))
                .then_with(|| rank_of(left.id).cmp(&rank_of(right.id)))
        });
        let mut equipped: [Option<ItemView>; SLOT_COUNT] = array::from_fn(|_| None);
        let mut inventory = Vec::new();
        for item in &self.document.equipment {
            let view = self.item_view(item);
            if view.equipped {
                equipped[item.slot.index()] = Some(view);
            } else {
                inventory.push(view);
            }
        }
        inventory.sort_by_key(|item| std::cmp::Reverse(item.id));
        let worn: Vec<EquipmentRecord> = self.equipped_items().collect();
        let mut seeds: Vec<SeedView> = self.document.seeds.iter().map(seed_view).collect();
        seeds.sort_by(|left, right| {
            right
                .tier
                .cmp(&left.tier)
                .then_with(|| right.grade.cmp(&left.grade))
                .then_with(|| left.attribute.index().cmp(&right.attribute.index()))
        });
        CultivationView {
            level,
            realm,
            layer: said.layer,
            rank: said.rank,
            growth: self.document.growth,
            floor,
            next,
            attributes,
            derived,
            level_attributes,
            equipment_attributes,
            crop_attributes,
            allocated_attributes,
            art_attributes,
            free_points: self.free_points(),
            power: self.power(),
            power_breakdown: self.power_breakdown(),
            equipped,
            inventory,
            gear_sets: gear_set_views(&worn),
            seeds,
            plots: self
                .document
                .plots
                .iter()
                .map(|plot| PlotView {
                    crop: plot.crop.as_ref().map(|crop| CropView {
                        attribute: crop.attribute,
                        herb: crop.herb,
                        tier: crop.tier,
                        grade: crop.grade,
                        name: crop.name.clone(),
                        ready_at_unix: crop.ready_at_unix,
                        withered: crop.stands_dead(now_unix),
                    }),
                })
                .collect(),
            // What the next plot would cost, and nothing once the garden is
            // full: the buy button reads this rather than recomputing the
            // table, so the number on it is the number the domain charges.
            next_plot_price: plot_price(self.document.plots.len()),
            garden_rank: self.document.garden_rank,
            garden_xp: self.document.garden_xp,
            garden_xp_needed: if self.document.garden_rank < MAX_GARDEN_RANK {
                rank_xp_total(self.document.garden_rank.saturating_add(1))
            } else {
                0
            },
            garden_upgrade_cost: (self.document.garden_rank < MAX_GARDEN_RANK)
                .then(|| rank_cost(self.document.garden_rank)),
            guardian: self.guardian().map(|guardian| GuardianView {
                target: guardian.target,
                power: guardian.power,
            }),
            last_breakthrough: self.last_breakthrough.clone(),
            last_absence: self.last_absence,
            last_work: self.last_work,
            work: self.document.work,
            job_refresh_cost: super::super::jobs::refresh_cost(super::super::drop::tier_for_power(
                self.power(),
            )),
            jobs: self
                .jobs(day)
                .into_iter()
                .enumerate()
                .map(|(slot, offer)| {
                    let working = self.document.work.is_some_and(|run| {
                        run.tier == offer.tier
                            && run.hours == offer.hours
                            && run.grade == offer.grade
                    });
                    let level = self.level();
                    let derived = self.derived();
                    let (first, second) = offer.kind.slots();
                    JobView {
                        slot,
                        tier: offer.tier,
                        hours: offer.hours,
                        grade: offer.grade,
                        stones: offer.stones(),
                        working,
                        kind: offer.kind,
                        requirement: super::super::jobs::requirement(first, level, offer.grade),
                        requirement_second: second
                            .map(|slot| super::super::jobs::requirement(slot, level, offer.grade)),
                        current_first: first.of(derived),
                        current_second: second.map(|slot| slot.of(derived)),
                        success_rate: super::super::jobs::success_rate_permille(
                            offer.kind,
                            derived,
                            level,
                            offer.grade,
                        ),
                    }
                })
                .collect(),
            last_drop: self.last_drop.clone().map(|drop| match drop {
                Drop::Equipment(item) => DropView::Equipment(ItemView {
                    id: item.id,
                    slot: item.slot,
                    tier: item.tier,
                    rarity: item.rarity,
                    attributes: item.effective_attributes(),
                    bonus: item.effective_bonus(),
                    equipped: false,
                    star: item.star(),
                    effect: item.effect,
                    star_cost: (!item.star_capped())
                        .then(|| star_up_cost(&item, self.document.smithy_level)),
                    name: item.name.clone(),
                    set: item.set,
                    sell_price: shop::ShopOffer::Equipment(item.clone()).sell_price(),
                }),
                Drop::Seed {
                    attribute,
                    tier,
                    grade,
                } => DropView::Seed {
                    attribute,
                    tier,
                    grade,
                },
                Drop::Art { name } => DropView::Art { name },
                Drop::Herb { herb, .. } => DropView::Herb { herb },
                Drop::Stones(stones) => DropView::Stones(stones),
            }),
            last_seed_return: self.last_seed_return.clone(),
            arts,
            mastered_arts,
            art_sets: art_set_views(&self.document.arts),
            art_power: self.art_power(),
            external_power: self.external_power(),
            aptitude_tenths,
            aptitude_herbs: self.aptitude_herbs(),
            marrow_gate_cost: super::super::aptitude::next_gate(aptitude_tenths)
                .map(|(_, cost)| cost),
            total_levels: MAX_LEVEL,
            realm_full: self.level() >= self.realm().cap_level(),
            selected_art: self.document.selected_art,
            last_training: self.last_training.clone(),
            last_exploration_duel: self.last_exploration_duel.clone(),
            last_exploration: self.last_exploration,
            last_exploration_found: self.last_exploration_found.clone(),
            exploration_ready: self.exploration_ready(now_unix),
            // The door the log reads through is the same door the tower and the
            // 试炼 ask: a walk whose time is up or a job whose hours have passed
            // is home whether or not the tick has settled it yet, so the status
            // line agrees with what the page actually lets the cultivator do.
            at_home: self.exploration_ready(now_unix) && !self.working(now_unix),
            breakthrough_ready: breakthrough_ready(
                self.document.breakthrough_blocked_until,
                now_unix,
            ),
            exploration_ready_at: self.walk_home_at(),
            exploration_hours: super::super::ExplorationHours::from_hours(
                self.document.last_exploration_hours,
            ),
            // Whether the walk under way has already been called home: the page
            // draws the button as "on its way back" rather than offering to
            // call a called walk.
            walk_early: self.document.walk.is_some_and(|walk| walk.early),
            // And the same for a shift: a called shift is still a shift until
            // the trip home is over, so the page says so rather than letting a
            // second call look live and do nothing.
            work_early: self.document.work.is_some_and(|work| work.early),
            outing_return_seconds: self.return_trip_now(now_unix),
            breakthrough_ready_at: self.document.breakthrough_blocked_until,
            breakthrough_attempts: self.breakthrough_attempts,
            explorations: self.document.explorations,
            trial_floor: self.document.trial_floor,
            trial_next_floor: self.trial_next_floor(),
            trial_warden_power: warden_power(self.trial_next_floor()),
            trial_attempts_left: self.trial_attempts_left(day),
            trial_blocked: self.trial_blocked(now_unix, day),
            last_trial: self.last_trial.clone(),
            trial_sweepable: self.trial_sweepable(),
            last_trial_sweep: self.last_trial_sweep.clone(),
            trial_attempts: self.trial_attempts,
            proving_tokens: self.document.proving_tokens,
            proving_token_cap: TRIAL_CHANCES_CAP,
            proving_blocked: self.proving_blocked(now_unix),
            provings: self.provings,
            last_proving: self.last_proving.clone(),
            last_daily_event: self.last_daily_event.clone(),
            daily_event_day: self.document.daily_event_day,
            daily_events: self.daily_events,
            daily_event_seen_day: self.document.daily_event_seen_day,
            star_attempts: self.star_attempts,
            last_star: self.last_star.clone(),
            last_collect: self.last_collect.clone(),
            last_building: self.last_building.clone(),
            exploration_region: usize::from(self.document.exploration_region)
                .min(super::super::explore::Region::ALL.len() - 1)
                as u8,
            exploration_level: {
                let region = super::super::explore::Region::from_realm(self.realm());
                self.cleared_level(region.index())
                    .saturating_add(1)
                    .min(super::super::explore::MAX_LEVEL)
            },
            walk_level: self.document.walk.as_ref().map_or(0, |walk| walk.level),
            explored_levels: self.document.explored_levels,
            region_progress: self.document.region_progress,
            regions: self.region_views(),
            walk_gain: self.document.last_walk_gain,
            resources: self.resources_view(),
            session_mode: self.session_mode(),
            next_session_growth: self.next_session_growth(day),
            derived_breakdowns: attribute_derived.breakdowns(attributes, bonus, level),
            duel_record: self.document.duel_record.clone(),
            last_walk: self.document.last_walk.clone(),
            events: self.document.events.clone(),
            stones: self.document.stones,
            last_refusal: self.last_refusal,
            refusal_count: self.refusal_count,
            day,
            shop_offers: self.shop(day),
        }
    }

    /// What the next finished session pays, in growth.
    ///
    /// The number rather than the multiplier the four disciplines add up to: the
    /// disciplines are not shown, so a percentage would be a rule the user can
    /// neither see nor move. This is the growth the next pomodoro is worth, and
    /// it is read from the same formula the session itself is paid by, so the
    /// two cannot disagree.
    ///
    /// The session number is the one the next session will actually be: the
    /// counter resets when the day changes, so a session that lands on a new day
    /// is the first of that day rather than the next of this one.
    fn next_session_growth(&self, day: i64) -> u32 {
        let stage = self.realm().index() as u32 + 1;
        let session_number = if self.document.last_active_day == Some(day) {
            self.document.session_count_today.saturating_add(1)
        } else {
            1
        };
        scaled_growth(
            super::super::session_growth(session_number, stage),
            self.growth_multiplier_permille(),
        )
    }

    /// The 阶 of the 功法 a finished session trains, or zero when none is chosen.
    ///
    /// The 修炼 dimension is trained by this and nothing else, so both the bar and
    /// the session multiplier read it from here.
    fn trained_art_rank(&self) -> u8 {
        self.document
            .selected_art
            .and_then(|id| self.document.arts.iter().find(|art| art.art_id == id))
            .map_or(0, |art| art.rank)
    }

    /// One owned item as a surface reads it, worn or not.
    fn item_view(&self, item: &EquipmentRecord) -> ItemView {
        ItemView {
            id: item.id,
            slot: item.slot,
            tier: item.tier,
            rarity: item.rarity,
            // The numbers the piece actually pays, stars folded in: what the
            // fight reads and what the row prints are the same numbers.
            attributes: item.effective_attributes(),
            bonus: item.effective_bonus(),
            equipped: self
                .document
                .equipped
                .get(item.slot.index())
                .copied()
                .flatten()
                == Some(item.id),
            star: item.star(),
            effect: item.effect,
            // What one more star costs, and nothing once the piece holds every
            // star it can: a button that asked for a star that cannot be had
            // would be a button that lies.
            star_cost: (!item.star_capped())
                .then(|| star_up_cost(item, self.document.smithy_level)),
            name: item.name.clone(),
            set: item.set,
            sell_price: shop::ShopOffer::Equipment(item.clone()).sell_price(),
        }
    }
}
