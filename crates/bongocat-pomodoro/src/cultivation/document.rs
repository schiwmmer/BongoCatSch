//! The persisted cultivation state.
//!
//! It is a field of the pomodoro document rather than a file of its own: the
//! ladder is fed by the focus history that document already holds, and one file
//! means one atomic write covering both — a session can never be recorded
//! without the growth it paid. Every field defaults when absent, so a document
//! written before cultivation existed loads as a cultivator who has just
//! started, and nothing about the earlier shape breaks.

use serde::{Deserialize, Serialize};

use super::{
    ART_COUNT, Attributes, EquipmentRecord, FREE_PLOT_COUNT, MAX_LEVEL, MAX_PLOT_COUNT, OwnedArt,
    PlotRecord, SLOT_COUNT, STAGE_COUNT, SeedStack, blueprint,
    curve::{self, level_for_growth, stage_floor},
    duel::{DUEL_RECORD_CAP, DuelRecord},
    events::{EVENT_LOG_CAP, EventRecord},
    explore::WalkRecord,
};

/// The [`curve::CURVE_REV`] a document carries when nothing was stored: a fresh
/// in-memory default, already counted on the current ramp. A document written
/// before the field existed reads as `1` — the first ramp — and is migrated.
fn first_curve_rev() -> u32 {
    1
}

/// Everything a cultivator keeps between launches.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CultivationDocument {
    /// Lifetime growth, less what the missed days took.
    #[serde(default)]
    pub growth: u32,
    /// Which growth ramp `growth` was counted on; see [`curve::CURVE_REV`].
    #[serde(default = "first_curve_rev")]
    pub curve_rev: u32,
    /// The realm the cultivator has entered, as its ladder position.
    #[serde(default)]
    pub realm: u8,
    /// The local day of the last finished session, or `None` before the first.
    #[serde(default)]
    pub last_active_day: Option<i64>,
    /// How many consecutive days without a session have already been charged.
    /// This is the doubling exponent, not a count of all missed days.
    #[serde(default)]
    pub missed_streak: u32,
    /// How many sessions have been completed on `last_active_day`, so the growth
    /// of the next one knows which number it is. Reset to zero whenever a session
    /// lands on a new local day, which is decided against `last_active_day`.
    #[serde(default)]
    pub session_count_today: u32,
    /// The last local day whose absence has been charged, so a re-launch on the
    /// same day cannot charge the same day twice.
    #[serde(default)]
    pub charged_through_day: Option<i64>,
    /// Every piece of gear owned, equipped or not.
    #[serde(default)]
    pub equipment: Vec<EquipmentRecord>,
    /// The item worn in each slot, as its id. Padded to [`SLOT_COUNT`].
    #[serde(default)]
    pub equipped: Vec<Option<u64>>,
    /// The id the next drop takes. Derived on load as well, so a hand-edited
    /// document still cannot hand out one id twice.
    #[serde(default)]
    pub next_item_id: u64,
    /// Seeds waiting to be planted.
    #[serde(default)]
    pub seeds: Vec<SeedStack>,
    /// The plots. Never fewer than [`FREE_PLOT_COUNT`], never more than
    /// [`MAX_PLOT_COUNT`]: the four free ones every save starts with, plus
    /// whatever has been bought since.
    #[serde(default)]
    pub plots: Vec<PlotRecord>,
    /// The attribute points the harvests have paid, permanently.
    #[serde(default)]
    pub crop_bonus: Attributes,
    /// The highest level this cultivator has ever reached.
    ///
    /// Free points are paid against this rather than the current level: an
    /// absence penalty can lower the level, but it cannot take back a point the
    /// user already earned and spent.
    #[serde(default)]
    pub peak_level: u16,
    /// The free points the user has placed, by attribute.
    #[serde(default)]
    pub allocated: Attributes,
    /// How many attempts at the current realm's guardian have failed. Flavour
    /// only: a failure never takes anything away.
    #[serde(default)]
    pub breakthrough_failures: u32,
    /// The 功法 owned, named by the id of the one it is out of the five hundred.
    ///
    /// Only the id, the 阶 and the banked progress are stored: everything else
    /// about a 功法 is rebuilt from its id, so a save cannot drift from the
    /// catalog and a 功法 the user has never met costs nothing.
    ///
    /// These are the **功法书** — the books the cultivator is still learning
    /// from. A book that has been trained to 圆满 leaves this list and becomes
    /// an entry in [`Self::mastered_arts`]; see that field for why.
    #[serde(default)]
    pub arts: Vec<OwnedArt>,
    /// The 功法 the cultivator has learned outright, named by id.
    ///
    /// A 功法 that reaches 圆满 stops being a book and becomes a thing the
    /// cultivator knows: it leaves [`Self::arts`] and is written down here
    /// instead. The two are kept apart on purpose. A book can be sold, and
    /// selling one must not take the 功法 it taught with it — otherwise the
    /// attributes a completed 内功 pays would be lost the moment the user
    /// tidied up their shelf. And a book is what a 功法 is learned *from*: with
    /// no book there is nothing to train, which is why the training list and
    /// the learned list cannot be the same list.
    ///
    /// Stored by id alone, because a learned 功法 is complete: its 阶 is its
    /// grade's last one and its progress is nothing, both of which the id
    /// rebuilds exactly.
    #[serde(default)]
    pub mastered_arts: Vec<u64>,
    /// The 功法 the next finished pomodoro trains, if one is chosen.
    #[serde(default)]
    pub selected_art: Option<u64>,
    /// The instant a lost breakthrough can be attempted again.
    #[serde(default)]
    pub breakthrough_blocked_until: Option<i64>,
    /// When the last walk happened, which is what gates the next one.
    #[serde(default)]
    pub last_exploration_unix: Option<i64>,
    /// How many walks have been taken — the 游历 dimension's own count, kept in
    /// the document because a dimension that forgot its training on every restart
    /// would never fill its bar.
    #[serde(default)]
    pub explorations: u32,
    /// How many fights have been won — the 战斗 dimension's count. Losses do not
    /// count: the dimension is trained by winning, not by having been swung at.
    #[serde(default)]
    pub battle_wins: u32,
    /// How many crops have been pulled — the 造化 dimension's count.
    #[serde(default)]
    pub harvests: u32,
    /// The fights fought, newest first, capped at [`DUEL_RECORD_CAP`].
    ///
    /// Persisted rather than left to the popup because the popup is a moment: it
    /// is forgotten on quit, and a page that could not say how the last few
    /// fights went would be showing the ladder without its history.
    #[serde(default)]
    pub duel_record: Vec<DuelRecord>,
    /// What the last walk settled, kept past the quit that ends its popup.
    #[serde(default)]
    pub last_walk: Option<WalkRecord>,
    /// The 灵石 in the purse.
    #[serde(default)]
    pub stones: u32,
    /// The local day whose shelf the bought slots belong to.
    ///
    /// The shelf itself is rolled from the day and the realm rather than stored,
    /// so this is the only thing that has to be right about it: a document saved
    /// on one day and opened on another has bought slots from a shelf nobody can
    /// be looking at any more, and clearing them is the whole reason the day is
    /// written down.
    #[serde(default)]
    pub shop_day: Option<i64>,
    /// Which slots of that day's shelf have been bought, as slot indexes.
    #[serde(default)]
    pub shop_bought: Vec<u8>,
    /// What has happened lately, newest first, capped at
    /// [`EVENT_LOG_CAP`](super::EVENT_LOG_CAP).
    ///
    /// The popups are moments and the duel record only knows fights; this is
    /// the page's one line of memory for everything else a cultivator does —
    /// sessions, 功法 turning over, goods bought and sold, crops pulled.
    #[serde(default)]
    pub events: Vec<EventRecord>,
    /// The learned 功法 whose tome sale has been collected, named by id.
    ///
    /// Selling a learned 功法 sells the tome, not the knowledge: the 功法
    /// stays in [`Self::mastered_arts`] and pays exactly what it paid. What is
    /// sold once cannot be sold twice — a tome the market buys every time it is
    /// asked about is a money printer — so the sale is written down here.
    #[serde(default)]
    pub sold_mastered_arts: Vec<u64>,
}

impl CultivationDocument {
    /// Bring `growth` onto the current ramp, once.
    ///
    /// The move keeps the level — and its fraction of the way to the next one —
    /// so a retune raises only the cost of levels ahead. `curve_rev == 1` is a
    /// document from the first ramp; `0` is a fresh default that never touched
    /// a ramp and is simply stamped. Anything already at [`curve::CURVE_REV`]
    /// is left exactly as it is, which is what makes a second run a no-op.
    fn migrate_curve(&mut self) {
        if self.curve_rev >= curve::CURVE_REV {
            return;
        }
        if self.curve_rev == 1 {
            self.growth = curve::migrate_growth_v1(self.growth);
        }
        self.curve_rev = curve::CURVE_REV;
    }

    /// The document as written back out: the slots and the plots are the fixed
    /// size the surfaces draw, the realm is on the ladder, and the growth never
    /// sits below the floor of the realm it is in.
    pub(crate) fn normalized(mut self) -> Self {
        self.migrate_curve();
        let realm = usize::from(self.realm).min(STAGE_COUNT - 1);
        self.realm = realm as u8;
        self.growth = self.growth.max(stage_floor(realm));
        let top_level = ((realm as u16) + 1) * super::LAYERS_PER_STAGE;
        debug_assert!(top_level <= MAX_LEVEL);
        // A hand-edited document cannot claim a peak it never reached, and it
        // cannot claim more points than the peak paid. Clamping here rather than
        // in the state machine keeps the invariant on the value that is written
        // back out.
        let level = level_for_growth(self.growth)
            .level
            .min(top_level)
            .min(MAX_LEVEL);
        self.peak_level = self.peak_level.max(level).min(MAX_LEVEL);
        self.allocated = Attributes::new(
            self.allocated.root.max(0),
            self.allocated.strength.max(0),
            self.allocated.mana.max(0),
            self.allocated.agility.max(0),
        );
        // A hand-edited save could carry a thousand lines of history; the record
        // is a window, so it is cut back to the window here rather than trusted.
        self.duel_record.truncate(DUEL_RECORD_CAP);
        // The event log is a window too, and a hand-edited save could carry a
        // season of entries; cut it back to the window here rather than trust it.
        self.events.truncate(EVENT_LOG_CAP);
        // A collected tome sale is a marker, not a list that grows: out-of-range
        // ids cannot name a 功法, and a duplicate would read as two sales.
        let mut sold: Vec<u64> = Vec::with_capacity(self.sold_mastered_arts.len());
        for id in self.sold_mastered_arts.drain(..) {
            if (1..=ART_COUNT).contains(&id) && !sold.contains(&id) {
                sold.push(id);
            }
        }
        self.sold_mastered_arts = sold;
        // A hand-edited or stale document could carry a thousand bought slots for
        // a shelf of ten; the shelf is a window, so is this.
        self.shop_bought.truncate(super::SHOP_MAX_OFFERS);
        self.equipped.resize(SLOT_COUNT, None);
        // The garden holds between the four free plots and the nine a save can
        // buy: a hand-edited document with fewer is topped up, one with more
        // than the garden can hold is cut back.
        self.plots.truncate(MAX_PLOT_COUNT);
        self.plots
            .resize(FREE_PLOT_COUNT.max(self.plots.len()), PlotRecord::default());
        let highest = self.equipment.iter().map(|item| item.id).max().unwrap_or(0);
        self.next_item_id = self.next_item_id.max(highest.saturating_add(1));
        // An equipped id that names nothing — a document edited by hand, or an
        // item removed by a future build — reads as an empty slot rather than a
        // slot whose owner cannot be found.
        for slot in &mut self.equipped {
            if slot.is_some_and(|id| !self.equipment.iter().any(|item| item.id == id)) {
                *slot = None;
            }
        }
        // A 功法 out of range, or one kept twice, cannot be read: the first
        // entry for an id wins and the 阶 is clamped to the grade that id
        // carries. A selection naming an art that is not owned reads as none.
        let mut seen: Vec<u64> = Vec::with_capacity(self.arts.len());
        self.arts.retain(|art| {
            if art.art_id < 1 || art.art_id > ART_COUNT || seen.contains(&art.art_id) {
                return false;
            }
            seen.push(art.art_id);
            true
        });
        for art in &mut self.arts {
            let max_rank = blueprint(art.art_id).grade.ranks();
            // Zero is where every 功法 starts: a book on the shelf has been
            // obtained, not begun, and the first 阶 is what training buys.
            art.rank = art.rank.clamp(0, max_rank);
            if art.rank >= max_rank {
                art.progress = 0;
            }
        }
        // A 功法 that has reached its last 阶 is no longer a book: it is one the
        // cultivator knows. This is where a save written before the two lists
        // were split moves its finished 功法 across, so an old save reads as
        // having learned what it had in fact finished rather than as still
        // carrying a book it has already read to the end.
        let mut mastered: Vec<u64> = Vec::with_capacity(self.mastered_arts.len());
        for id in self.mastered_arts.drain(..) {
            if (1..=ART_COUNT).contains(&id) && !mastered.contains(&id) {
                mastered.push(id);
            }
        }
        self.arts.retain(|art| {
            if art.rank < blueprint(art.art_id).grade.ranks() {
                return true;
            }
            if !mastered.contains(&art.art_id) {
                mastered.push(art.art_id);
            }
            false
        });
        self.mastered_arts = mastered;
        if self
            .selected_art
            .is_some_and(|id| !self.arts.iter().any(|art| art.art_id == id))
        {
            self.selected_art = None;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_document_is_a_fresh_cultivator() {
        let document = CultivationDocument::default().normalized();
        assert_eq!(document.growth, 0);
        assert_eq!(document.realm, 0);
        assert_eq!(document.equipped.len(), SLOT_COUNT);
        assert_eq!(document.plots.len(), FREE_PLOT_COUNT);
        assert!(document.equipment.is_empty());
        assert_eq!(document.next_item_id, 1, "the first drop takes id one");
        assert_eq!(document.peak_level, 1, "level one has paid nothing yet");
        assert_eq!(document.allocated, Attributes::default());
    }

    #[test]
    fn a_document_written_before_the_free_points_existed_pays_the_levels_it_already_has() {
        let parsed: CultivationDocument =
            serde_json::from_str(r#"{"growth":1000}"#).expect("the older shape loads");
        let document = parsed.normalized();
        assert_eq!(
            document.peak_level, 84,
            "the peak is the level the stored growth already reached"
        );
        assert_eq!(document.allocated, Attributes::default());
    }

    #[test]
    fn a_hand_edited_allocation_never_holds_a_negative_point() {
        let document = CultivationDocument {
            allocated: Attributes::new(-3, 1, 0, 0),
            ..CultivationDocument::default()
        }
        .normalized();
        assert_eq!(
            document.allocated,
            Attributes::new(0, 1, 0, 0),
            "a negative place would otherwise pay a point that was never earned"
        );
    }

    #[test]
    fn a_document_written_before_a_field_existed_still_loads() {
        let parsed: CultivationDocument =
            serde_json::from_str(r#"{"growth":1500,"realm":1}"#).expect("the older shape loads");
        assert_eq!(parsed.growth, 1_500);
        assert!(parsed.equipment.is_empty());
        assert!(parsed.seeds.is_empty());
        assert_eq!(parsed.crop_bonus, Attributes::default());
    }

    #[test]
    fn a_realm_past_the_ladder_is_clamped() {
        let document = CultivationDocument {
            realm: 200,
            ..CultivationDocument::default()
        }
        .normalized();
        assert_eq!(usize::from(document.realm), STAGE_COUNT - 1);
    }

    #[test]
    fn an_equipped_id_that_names_nothing_reads_as_an_empty_slot() {
        let document = CultivationDocument {
            equipped: vec![Some(99)],
            ..CultivationDocument::default()
        }
        .normalized();
        assert_eq!(document.equipped.len(), SLOT_COUNT);
        assert!(document.equipped.iter().all(Option::is_none));
    }

    #[test]
    fn a_document_on_the_first_ramp_moves_onto_the_current_one_keeping_its_level() {
        // The stored shape of a document written before `curve_rev` existed:
        // growth counted on the first ramp, no marker.
        let parsed: CultivationDocument =
            serde_json::from_str(r#"{"growth":4986,"realm":2}"#).expect("the older shape loads");
        assert_eq!(
            parsed.curve_rev, 1,
            "the absent field reads as the first ramp"
        );
        let document = parsed.normalized();
        assert_eq!(
            level_for_growth(document.growth).level,
            217,
            "the level the stored growth reached survives the move"
        );
        assert_eq!(document.peak_level, 217);
        assert_eq!(document.curve_rev, curve::CURVE_REV);
    }

    #[test]
    fn normalizing_twice_changes_nothing() {
        let parsed: CultivationDocument =
            serde_json::from_str(r#"{"growth":4986,"realm":2}"#).expect("the older shape loads");
        let once = parsed.clone().normalized();
        let twice = once.clone().normalized();
        assert_eq!(once, twice, "the migration runs once and only once");
    }

    #[test]
    fn a_fresh_default_is_already_on_the_current_ramp() {
        // A default has counted nothing and must not be read as a first-ramp
        // document: growth earned after it would otherwise be re-denominated.
        let document = CultivationDocument {
            growth: 4_986,
            ..CultivationDocument::default()
        }
        .normalized();
        assert_eq!(document.growth, 4_986, "a fresh default is never migrated");
        assert_eq!(document.curve_rev, curve::CURVE_REV);
    }
}
