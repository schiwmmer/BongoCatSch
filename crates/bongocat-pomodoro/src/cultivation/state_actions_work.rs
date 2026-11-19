//! 打工：活计板、接活、刷新与旧档结算。
//!
//! Split out of `state.rs` as a child module so it keeps access to the
//! document's private fields without widening their visibility.

use super::*;

impl Cultivation {
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

    /// Take the job in that slot of the board, as the 打工 task it names.
    ///
    /// Refused while the cultivator is out on a walk or an expedition: both are
    /// hours of the same day, and a cultivator who can be in two places at once
    /// has no reason to choose between them. A job is a task now (`docs/0103`
    /// §4, §5.3, §9-S4), so the one-slot rule is [`Self::start_work_task`]'s:
    /// a cultivator already at a task is refused with [`Refusal::TaskRunning`].
    ///
    /// Returns the offer that was taken, which is the row the board printed;
    /// the task it started rides [`ActiveTask::grade`] and
    /// [`ActiveTask::job_kind`] and is read back with [`Self::task`].
    pub fn accept_job(
        &mut self,
        slot: usize,
        now_unix: i64,
        day: i64,
    ) -> Result<super::super::JobOffer, Refusal> {
        if self.document.walk.is_some() {
            return Err(Refusal::TrialOut);
        }
        // An expedition is hours of the same day, so the board is closed while
        // the cultivator is out gathering.
        if self.document.expedition.is_some() {
            return Err(Refusal::ExpeditionOut);
        }
        // A save written before S4 can still hold an hour-job in hand. It is
        // settled by [`Self::settle_legacy_work`] on the tick, and until then
        // it holds the cultivator the way it always did; new jobs never write
        // this field again.
        if self.document.work.is_some() {
            return Err(Refusal::Working);
        }
        // A job is a task now, so the one slot is the task's: a cultivator
        // already at a task cannot take another, whatever that task is.
        if self.document.task.is_some() {
            return Err(Refusal::TaskRunning);
        }
        let Some(offer) = self.jobs(day).get(slot).copied() else {
            return Err(Refusal::JobGone);
        };
        self.start_work_task(offer, now_unix)?;
        Ok(offer)
    }

    /// Pay for a fresh board.
    ///
    /// The price is what stops the board being a slot machine: three draws at a
    /// ten-thousandth, re-rolled until a 神阶 job turns up, would cost nothing at
    /// all if this were free.
    pub fn refresh_jobs(&mut self) -> Result<u32, TradeRefusal> {
        let cost = super::super::jobs::refresh_cost();
        if self.document.stones < cost {
            return Err(TradeRefusal::NotEnoughStones);
        }
        self.document.stones = self.document.stones.saturating_sub(cost);
        self.document.jobs_roll = self.document.jobs_roll.saturating_add(1);
        Ok(cost)
    }

    /// Settle an hour-job a save written before S4 still holds. Returns what it
    /// paid.
    ///
    /// **Legacy path.** S4 turned a job into a 打工 task (`docs/0103` §9), and
    /// [`Self::accept_job`] no longer writes `document.work`; a job
    /// taken from here on pays tick by tick through [`Self::settle_task_tick`].
    /// This remains only so an in-flight shift from an older save still comes
    /// home and pays on the tick, rather than being stranded by the redesign;
    /// it goes with the field in S8.
    ///
    /// Called by the worker's tick the way a walk's return is: a job is hours of
    /// the cultivator's day, so it comes home on the clock rather than on a click,
    /// and an app that was shut for the whole of it has to pay on the next launch
    /// rather than lose the hours.
    pub fn settle_legacy_work(&mut self, now_unix: i64) -> Option<WorkPaid> {
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
}
