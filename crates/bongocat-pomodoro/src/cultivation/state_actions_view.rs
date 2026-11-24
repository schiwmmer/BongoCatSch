//! 快照与查询：把状态机读成界面要的那一份视图。
//!
//! Split out of `state.rs` as a child module so it keeps access to the
//! document's private fields without widening their visibility.

use super::super::CultivationDayBaseline;
use super::*;
use crate::cultivation::reforge_cost;

impl Cultivation {
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
                duel: outcome.duel.clone(),
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

    /// Record the level and combat power the local day opens with.
    ///
    /// A new day has no baseline yet, so the first time the page reads the
    /// ladder after midnight the current values become that day's starting
    /// point. The absence charge has already been applied by the worker by the
    /// time the day opens, so the penalty belongs to the day that missed its
    /// sessions rather than to the new day's progress.
    pub fn ensure_day_baseline(&mut self, day: i64) {
        if self
            .document
            .today_baseline
            .is_some_and(|baseline| baseline.day == day)
        {
            return;
        }
        self.document.today_baseline = Some(CultivationDayBaseline {
            day,
            level: self.level(),
            power: self.power(),
        });
    }

    /// How many levels the cultivator has gained since the local day opened.
    ///
    /// A save written before the baseline existed reports zero rather than a
    /// delta from the whole save's history; the next view takes the current
    /// level as the day's starting point.
    fn today_level_delta(&self, day: i64) -> i32 {
        self.document
            .today_baseline
            .filter(|baseline| baseline.day == day)
            .map_or(0, |baseline| {
                self.level() as i64 - i64::from(baseline.level)
            }) as i32
    }

    /// How much combat power the cultivator has gained since the local day
    /// opened. The same rule as [`Self::today_level_delta`] applies to a save
    /// written before the baseline existed.
    fn today_power_delta(&self, day: i64) -> i32 {
        self.document
            .today_baseline
            .filter(|baseline| baseline.day == day)
            .map_or(0, |baseline| {
                self.power() as i64 - i64::from(baseline.power)
            }) as i32
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
        let mut seeds: Vec<SeedView> = self
            .document
            .seeds
            .iter()
            .map(|stack| seed_view(stack, self.document.garden_rank))
            .collect();
        seeds.sort_by(|left, right| {
            right
                .tier
                .cmp(&left.tier)
                .then_with(|| right.grade.cmp(&left.grade))
                .then_with(|| left.attribute.index().cmp(&right.attribute.index()))
        });
        let rules = RulesView {
            max_star: MAX_STAR,
            seclusion_ticks: u32::try_from(SECLUSION_TICKS).unwrap_or(u32::MAX),
            seclusion_seconds: u32::try_from(
                SECLUSION_TICKS.saturating_mul(u64::from(SECLUSION_TICK_SECONDS)),
            )
            .unwrap_or(u32::MAX),
            region_count: u8::try_from(crate::cultivation::Region::ALL.len()).unwrap_or(u8::MAX),
            region_levels: crate::cultivation::LEVELS_PER_REGION,
            explore_hours: crate::cultivation::ExplorationHours::ALL
                .into_iter()
                .map(|hours| ExploreHourView {
                    hours: hours.hours(),
                    multiplier_permille: u32::try_from(hours.multiplier_permille())
                        .unwrap_or(u32::MAX),
                })
                .collect(),
            gather_depths: crate::cultivation::GatherTier::ALL
                .into_iter()
                .map(|tier| GatherDepthView {
                    tier,
                    unlock_level: tier.unlock_level(),
                    base_permille: tier.base_permille(),
                })
                .collect(),
        };
        CultivationView {
            level,
            realm,
            rules,
            today_level_delta: self.today_level_delta(day),
            today_power_delta: self.today_power_delta(day),
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
            // S5: the real-time battle under way, carried whole so a surface can
            // draw the fight it already decided and the rounds it has left.
            battle: self.document.battle.clone(),
            // S1/S3: the real-time task and the 闭关, carried whole for the same
            // reason — a surface draws the task it is running and the sit beside
            // it, rather than recomputing either from the clock.
            task: self.document.task.clone(),
            seclusion: self.document.seclusion.clone(),
            last_breakthrough: self.last_breakthrough.clone(),
            last_absence: self.last_absence,
            last_work: self.last_work,
            work: self.document.work,
            job_refresh_cost: super::super::jobs::refresh_cost(self.realm().index() as u8 + 1),
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
                        hourly_stones: offer.hourly_stones(),
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
                    // A drop is not owned yet: the row can show what it is,
                    // not offer a reforge before it is picked up.
                    reforge_cost: None,
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
            reforge_attempts: self.reforge_attempts,
            last_reforge: self.last_reforge.clone(),
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
            next_session_growth: self.next_session_growth(),
            derived_breakdowns: attribute_derived.breakdowns(attributes, bonus, level),
            duel_record: self.document.duel_record.clone(),
            duel_replay_speed: self.document.duel_replay_speed,
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
    /// it is read from the same arithmetic the session is paid by, so the two
    /// cannot disagree.
    ///
    /// A session is a whole 闭关 (`docs/0103` §5.1), so the number is the sit's
    /// own: [`SECLUSION_TICKS`] ticks at the sit's bonus. It does not climb with
    /// the session of the day — the old per-session escalation is gone with the
    /// old formula.
    fn next_session_growth(&self) -> u32 {
        sit_growth(
            meditate_tick_growth(
                self.realm().index() as u32,
                self.growth_multiplier_permille(),
            ),
            SECLUSION_TICKS,
        )
    }

    /// The 阶 of the 功法 a finished session trains, or zero when none is chosen.
    ///
    /// The 修炼 dimension is trained by this and nothing else, so both the bar and
    /// the session multiplier read it from here.
    pub(super) fn trained_art_rank(&self) -> u8 {
        self.document
            .selected_art
            .and_then(|id| self.document.arts.iter().find(|art| art.art_id == id))
            .map_or(0, |art| art.rank)
    }

    /// One owned item as a surface reads it, worn or not.
    ///
    /// `pub(super)` because a battle's apply phase rebuilds the haul a surface
    /// reads from the bag (`battle_run.rs`), and it has to read the item the
    /// same way every other path does.
    pub(super) fn item_view(&self, item: &EquipmentRecord) -> ItemView {
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
            // What one reforge costs, and `None` for a piece with only a
            // primary line: there is nothing to move.
            reforge_cost: reforge_cost(item, self.document.smithy_level),
            name: item.name.clone(),
            set: item.set,
            sell_price: shop::ShopOffer::Equipment(item.clone()).sell_price(),
        }
    }
}
