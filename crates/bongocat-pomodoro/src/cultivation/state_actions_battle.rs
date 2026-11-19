//! 每日事件、破境与战斗：点击算出的预演，回合只推进时间，最后一回合落账。
//!
//! Split out of `state.rs` as a child module so it keeps access to the
//! document's private fields without widening their visibility.

use super::*;

impl Cultivation {
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
    ///
    /// S5: the fight is a real-time battle now, and this entry is its thin
    /// immediate form — [`Self::start_battle`] decides the whole fight with the
    /// same rolls in the same order, and [`Self::settle_battle_now`] writes the
    /// decided result. Nothing about the judgment moved; only the timing did.
    pub fn breakthrough(&mut self, now_unix: i64) -> Option<BreakthroughOutcome> {
        self.start_battle(BattleKind::Breakthrough, now_unix, 0, None, false)
            .ok()?;
        self.settle_battle_now(now_unix)?;
        self.last_breakthrough.clone()
    }

    /// Roll one 洗髓丹 after a won fight.
    ///
    /// The odds climb with the opponent's power relative to the cultivator's —
    /// one in a hundred thousand at parity, one in a thousand against something
    /// three times as strong — so the pill is the reward for punching up rather
    /// than for farming the easiest thing on the page.
    pub(super) fn roll_marrow_from_combat(
        &mut self,
        rival_power: u32,
        rng: &mut Rng,
        now_unix: i64,
    ) -> bool {
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
    pub(super) fn roll_marrow_from_fortune(&mut self, rng: &mut Rng, now_unix: i64) -> bool {
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
    /// `pub(super)` because a battle's apply phase writes the same record a
    /// fight does (`battle_run.rs`); the two must not keep two ledgers.
    pub(super) fn record_duel(&mut self, won: bool, kind: FightKind, now_unix: i64) {
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
    ///
    /// The replay it may carry is a window of its own: only the newest
    /// [`BATTLE_REPLAY_CAP`] fights are replayable, so an event pushed past that
    /// window is trimmed of its report. The line stays — the fight's finds,
    /// its purse and its outcome are the event — but the blows are what the
    /// replay alone could show, and a save that kept every fight since the
    /// first pomodoro would weigh more than the fights are worth.
    pub(super) fn push_event(&mut self, now_unix: i64, detail: EventDetail) {
        self.document.events.insert(
            0,
            EventRecord {
                at_unix: now_unix,
                detail,
            },
        );
        self.document.events.truncate(EVENT_LOG_CAP);
        // Walk the log newest first and count the fights still carrying their
        // report. The first BATTLE_REPLAY_CAP of them are the replay window;
        // every fight after it is trimmed, oldest last so the cut is the same
        // one the user would have made by hand.
        let mut replays = 0_usize;
        for event in &mut self.document.events {
            if event.detail.is_battle() {
                replays += 1;
                if replays > BATTLE_REPLAY_CAP {
                    event.detail.clear_duel();
                }
            }
        }
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
            // band each is drawn from, and what a landing one lays on.
            hits: art.hits,
            band_permille: art.band_permille,
            effect: art.effect,
            // A completed 外功 spends its mastery share on every blow it lands;
            // an art one 阶 short of its last pays only its multiplier.
            mastery_permille: if rank >= art.grade.ranks() {
                mastery_permille(art.grade)
            } else {
                0
            },
        })
    }

    /// Open the realm the guardian defended and pay its trophy.
    ///
    /// `cfg(test)`: S5 moved the live win into the battle preview/apply split
    /// (`battle_run::plan_breakthrough` decides it, `settle_battle` writes it),
    /// so the only callers left are the two tests that pin the realm's
    /// inheritance rule and the trophy's provenance directly.
    #[cfg(test)]
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
}
