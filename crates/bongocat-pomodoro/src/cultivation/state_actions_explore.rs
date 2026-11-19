//! 外出与探索：启程、提前收工、到点结算与区域进度。
//!
//! Split out of `state.rs` as a child module so it keeps access to the
//! document's private fields without widening their visibility.

use super::*;

impl Cultivation {
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
        // S5: a fight under way is the cultivator's whole attention, so the
        // road is closed while one runs — the same wall a task start answers to.
        if self.document.battle.is_some() {
            self.note_refusal(Refusal::BattleRunning);
            return None;
        }
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
        // A task is hours of the same day too, so the road is closed while one
        // is running: a cultivator cannot walk and work at once.
        if self.document.task.is_some() {
            self.note_refusal(Refusal::TaskRunning);
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
        // and the page prints — so a walk that moves one percent is `1.0%`
        // rather than a parts-per-thousand number the surface would round.
        // The base draw rides beside it, so the page can say how the walk
        // itself went rather than only how far it moved the region.
        let progress_before = self
            .document
            .region_progress
            .get(region.index())
            .copied()
            .unwrap_or(0);
        let requirement = super::super::explore::region_requirement(region, progress_before * 10);
        let (gain, band) = if outcome == ExplorationOutcome::Defeat {
            (0, super::super::explore::WalkGainBand::Slow)
        } else {
            let (base, tenths) =
                super::super::explore::walk_gain_tenths(&mut rng, hours, self.power(), requirement);
            let band = super::super::explore::walk_gain_band(base);
            (tenths, band)
        };
        // The hint is picked from the band's own phrasings, so a week of walks
        // does not read as the same sentence over and over. A defeat carries no
        // hint: the walk did not happen, and a sentence about how it went would
        // be a sentence about a walk that was not taken.
        let hint = if outcome == ExplorationOutcome::Defeat {
            None
        } else {
            let keys = band.hint_keys();
            Some(keys[rng.below(keys.len() as u64) as usize].to_string())
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
            hint: hint.clone(),
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
                hint,
                duel: self.last_exploration_duel.clone(),
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
    ///
    /// `pub(super)` because a 试炼's apply phase settles the same progress a
    /// walk does (`battle_run.rs`), and the two must not keep two copies.
    pub(super) fn accumulate_progress(&mut self, region_index: usize, gain: u32) {
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
    pub(super) fn region_views(&self) -> Vec<super::RegionView> {
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
}
