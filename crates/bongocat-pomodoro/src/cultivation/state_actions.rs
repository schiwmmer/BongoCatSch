//! 采集、出行、会话与建筑：一回合的采集、一次专注的产出、一座建筑的升级。
//!
//! Split out of `state.rs` as a child module so it keeps access to the
//! document's private fields without widening their visibility.

use super::*;

impl Cultivation {
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
        // S5: a fight under way is the cultivator's whole attention, so an
        // expedition cannot be set out beside it either.
        if self.document.battle.is_some() {
            self.note_refusal(Refusal::BattleRunning);
            return None;
        }
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
        // A task is hours of the same day as an outing, so the road is closed
        // while one is running.
        if self.document.task.is_some() {
            self.note_refusal(Refusal::TaskRunning);
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
    ///
    /// `pub(super)` so the task module's per-tick gathering can pay the same
    /// way too, rather than growing a second copy of the ceiling and the xp.
    pub(super) fn bank_collection(
        &mut self,
        site: GatherSite,
        yield_: CollectionYield,
    ) -> CollectOutcome {
        // The 仓库's ceiling is per resource: a purse at the top of one kind
        // keeps the find of the other, so a full purse is never a lost day.
        let cap = resource_cap(self.document.warehouse_level);
        let mut gained = ResourceBag::default();
        let mut lost = ResourceBag::default();
        for resource in Resource::ALL {
            let room = cap.saturating_sub(self.document.resources.get(resource));
            let found = yield_.gained.get(resource);
            gained.add(resource, found.min(room));
            // The swallowed part is reported, not absorbed: a tick that found
            // three wood and banked none looks identical to a tick that found
            // nothing unless the difference is carried out here.
            lost.add(resource, found - found.min(room));
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
            lost,
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

    /// How fast a replayed fight plays.
    pub fn replay_speed(&self) -> ReplaySpeed {
        self.document.duel_replay_speed
    }

    /// Choose how fast a replayed fight plays, as the divisor of the reveal
    /// interval.
    ///
    /// Refused when the divisor is not one the popup offers: the control can
    /// only ever send one of the four, and a command that could set a fifth
    /// would leave the popup playing at a speed its own buttons cannot show.
    pub fn set_replay_speed(&mut self, divisor: u8) -> bool {
        let Some(speed) = ReplaySpeed::from_divisor(divisor) else {
            return false;
        };
        self.document.duel_replay_speed = speed;
        true
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
    pub(super) fn resources_view(&self) -> ResourcesView {
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
}
