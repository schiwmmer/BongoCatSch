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
    Attributes, EquipmentRecord, MAX_LEVEL, PLOT_COUNT, PlotRecord, SLOT_COUNT, STAGE_COUNT,
    SeedStack,
    curve::{level_for_growth, stage_floor},
};

/// Everything a cultivator keeps between launches.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CultivationDocument {
    /// Lifetime growth, less what the missed days took.
    #[serde(default)]
    pub growth: u32,
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
    /// The plots. Padded to [`PLOT_COUNT`].
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
}

impl CultivationDocument {
    /// The document as written back out: the slots and the plots are the fixed
    /// size the surfaces draw, the realm is on the ladder, and the growth never
    /// sits below the floor of the realm it is in.
    pub(crate) fn normalized(mut self) -> Self {
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
            self.allocated.spirit.max(0),
            self.allocated.agility.max(0),
        );
        self.equipped.resize(SLOT_COUNT, None);
        self.plots.resize(PLOT_COUNT, PlotRecord::default());
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
        assert_eq!(document.plots.len(), PLOT_COUNT);
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
}
