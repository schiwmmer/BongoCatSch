//! 锁妖塔与试炼：层数、次数、横扫与守关者。
//!
//! Split out of `state.rs` as a child module so it keeps access to the
//! document's private fields without widening their visibility.

use super::*;

impl Cultivation {
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
    pub(super) fn working(&self, now_unix: i64) -> bool {
        self.document
            .work
            .is_some_and(|work| !work.is_done(now_unix))
    }

    /// What calling the outing under way home would cost, in seconds.
    ///
    /// The same price for both kinds of outing, read off the one shared rule: a
    /// third of the time already spent. Zero while at home, so a caller can read
    /// it without asking whether anyone is out first.
    pub(super) fn return_trip_now(&self, now_unix: i64) -> i64 {
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
    ///
    /// S5: the floor is a real-time battle now. This entry is its thin
    /// immediate form — [`Self::start_battle`] decides the whole fight with the
    /// same rolls in the same order, and [`Self::settle_battle_now`] writes the
    /// decided result, so a click settles exactly what it settled before.
    pub fn trial(&mut self, now_unix: i64, day: i64) -> Option<TrialOutcome> {
        self.start_battle(BattleKind::Trial, now_unix, day, None, false)
            .ok()?;
        self.settle_battle_now(now_unix)?;
        self.last_trial.clone()
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
        // A sweep is a stretch of fights, so it answers to the battle slot as
        // much as a single climb does: a fight already on screen is not a moment
        // to skip another ten.
        if self.document.battle.is_some() {
            self.note_refusal(Refusal::BattleRunning);
            return None;
        }
        self.settle_walk(now_unix);
        self.settle_legacy_work(now_unix);
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
        let mut since_art = self.document.trial_since_art;
        for step in 0..floors {
            let floor = from_floor + step;
            let rival = opponent::warden(floor, &mut rng);
            let strength = Strength::of(rival.attributes.power());
            let reward = roll_reward_with_pity(&mut rng, since_art);
            // Every swept floor is a cleared floor, so it moves the 功法 pity
            // exactly the way a fought one does (`docs/0104` §6.1).
            since_art = if reward == TrialReward::Art {
                0
            } else {
                since_art.saturating_add(1)
            };
            match reward {
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
            // always pays a little. Paid once ever — the sweep only moves the
            // frontier forward (`docs/0104` §6.1) — and scaled by the floor's
            // own depth rather than its realm.
            let amount = roll_stones(&mut rng, trial_purse(floor), 1);
            self.document.stones = self.document.stones.saturating_add(amount);
            found.stones = found.stones.saturating_add(amount);
        }
        self.document.trial_floor = (from_floor + floors - 1).min(TRIAL_MAX_FLOOR);
        self.document.trial_since_art = since_art;
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
    ///
    /// S5: the 试炼 is a real-time battle now. This entry is its thin immediate
    /// form — [`Self::start_battle`] decides the whole fight with the same rolls
    /// in the same order, and [`Self::settle_battle_now`] writes the decided
    /// result, so a click settles exactly what it settled before.
    pub fn proving(&mut self, now_unix: i64, region: Option<u8>) -> Option<ProvingOutcome> {
        self.start_battle(BattleKind::Proving, now_unix, 0, region, false)
            .ok()?;
        self.settle_battle_now(now_unix)?;
        self.last_proving.clone()
    }
}
