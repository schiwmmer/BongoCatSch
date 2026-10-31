//! 资源: what the cultivator gathers, the skills that gather it, and the
//! buildings that make gathering worth doing.
//!
//! Four resources — 木材, 石块, 铁, 煤 — are gathered at three sites and feed
//! the buildings that upgrade the gear and the garden. The model here is the
//! *arithmetic*: what one collection turns up, and what the skills and the
//! buildings do to it. The spending — what a star costs, what a building costs
//! — lives beside the thing it buys, in [`super::items`] and [`super::shop`];
//! this module only says how much a gathering yields and how fast a skill
//! climbs.
//!
//! Three rules hold the design together:
//!
//! * **A skill is a multiplier, not a gate.** 伐木 and 采石 each add five
//!   percent of yield per level — the reward of a skill is more of what it
//!   already gathers — and only the *rare* finds (铁, 煤) are gated, because
//!   they are the two a skill has to be trained for to be worth finding.
//! * **The buildings are on top of the skill.** 工坊 multiplies the yield, 道场
//!   multiplies the skill's own xp, 仓库 widens the purse and 铁匠铺 lightens
//!   the anvil — four levers a user can raise in any order, and the page can
//!   say what each one does.
//! * **The rare finds are the reason to keep gathering.** A skill past its
//!   unlock still gathers the common find; the rare one is the point of going
//!   deeper, so it is never handed out before the skill earned it.

use serde::{Deserialize, Serialize};

/// The four gathered resources.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Resource {
    /// 木材: the forest's own find, common.
    Wood,
    /// 石块: the mountain's own find, common.
    Stone,
    /// 铁: a rare find, unlocked at 采石 level three.
    Iron,
    /// 煤: a rare find, unlocked at 伐木 level five.
    Coal,
}

impl Resource {
    /// The four, in the order the page reads them.
    pub const ALL: [Resource; 4] = [Self::Wood, Self::Stone, Self::Iron, Self::Coal];

    /// The catalog key naming this resource. A surface translates it; the domain
    /// never holds a translated word.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Wood => "cultivation.resource.wood",
            Self::Stone => "cultivation.resource.stone",
            Self::Iron => "cultivation.resource.iron",
            Self::Coal => "cultivation.resource.coal",
        }
    }

    /// Whether this resource is a rare find rather than a common one.
    pub const fn rare(self) -> bool {
        matches!(self, Self::Iron | Self::Coal)
    }
}

/// A purse of resources, one count per kind.
///
/// Stored as a bag rather than four loose fields so the arithmetic reads the
/// same everywhere: a collection adds to it, a building cost spends from it,
/// and a surface reads it whole.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResourceBag {
    pub wood: u32,
    pub stone: u32,
    pub iron: u32,
    pub coal: u32,
}

impl ResourceBag {
    /// What the purse holds of one resource.
    pub const fn get(&self, resource: Resource) -> u32 {
        match resource {
            Resource::Wood => self.wood,
            Resource::Stone => self.stone,
            Resource::Iron => self.iron,
            Resource::Coal => self.coal,
        }
    }

    /// Add a find to the purse.
    pub fn add(&mut self, resource: Resource, amount: u32) {
        match resource {
            Resource::Wood => self.wood = self.wood.saturating_add(amount),
            Resource::Stone => self.stone = self.stone.saturating_add(amount),
            Resource::Iron => self.iron = self.iron.saturating_add(amount),
            Resource::Coal => self.coal = self.coal.saturating_add(amount),
        }
    }

    /// Whether the purse can cover `amount` of one resource.
    pub fn can_spend(&self, resource: Resource, amount: u32) -> bool {
        self.get(resource) >= amount
    }

    /// Take `amount` of one resource. `false` — and nothing taken — if the
    /// purse is short.
    pub fn spend(&mut self, resource: Resource, amount: u32) -> bool {
        if !self.can_spend(resource, amount) {
            return false;
        }
        match resource {
            Resource::Wood => self.wood -= amount,
            Resource::Stone => self.stone -= amount,
            Resource::Iron => self.iron -= amount,
            Resource::Coal => self.coal -= amount,
        }
        true
    }

    /// Whether the purse can cover every kind a `tribute` asks for.
    pub fn can_spend_bag(&self, tribute: &ResourceBag) -> bool {
        Resource::ALL
            .iter()
            .all(|&resource| self.can_spend(resource, tribute.get(resource)))
    }

    /// Take a whole `tribute`. `false` — and nothing taken — if the purse is
    /// short of any one kind; the whole tribute is atomic so a star is never
    /// half-paid.
    pub fn spend_bag(&mut self, tribute: &ResourceBag) -> bool {
        if !self.can_spend_bag(tribute) {
            return false;
        }
        for resource in Resource::ALL {
            let amount = tribute.get(resource);
            if amount > 0 {
                self.spend(resource, amount);
            }
        }
        true
    }

    /// Add a whole find to the purse.
    pub fn add_bag(&mut self, find: &ResourceBag) {
        for resource in Resource::ALL {
            self.add(resource, find.get(resource));
        }
    }

    /// Everything the purse holds.
    pub const fn total(&self) -> u32 {
        self.wood + self.stone + self.iron + self.coal
    }

    /// Whether the purse holds nothing at all.
    pub const fn is_empty(&self) -> bool {
        self.total() == 0
    }
}

/// The two gathering trades.
///
/// A third site — the 矿洞 — works both trades at once, which is why it is not
/// itself a skill: the skills are 伐木 and 采石, and the mine is a place the
/// two trained hands both know how to read.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum GatherSkill {
    /// 伐木: the forest trade, and the one that learns to find 煤.
    Woodcut,
    /// 采石: the mountain trade, and the one that learns to find 铁.
    Stonecut,
}

impl GatherSkill {
    /// The two trades, in the order the page reads them.
    pub const ALL: [GatherSkill; 2] = [Self::Woodcut, Self::Stonecut];

    /// The catalog key naming this trade.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Woodcut => "cultivation.skill.woodcut",
            Self::Stonecut => "cultivation.skill.stonecut",
        }
    }
}

/// The three places a resource is gathered.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum GatherSite {
    /// 森林: 伐木, and the rare 煤 once the trade is trained.
    Forest,
    /// 山: 采石, and the rare 铁 once the trade is trained.
    Mountain,
    /// 矿洞: both trades at once, 铁 and 煤.
    Mine,
}

impl GatherSite {
    /// The catalog key naming this site.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Forest => "cultivation.site.forest",
            Self::Mountain => "cultivation.site.mountain",
            Self::Mine => "cultivation.site.mine",
        }
    }
}

/// The four buildings a cultivator raises.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Building {
    /// 工坊: every gathering yields ten percent more per level.
    Workshop,
    /// 仓库: the purse holds a quarter more per level.
    Warehouse,
    /// 铁匠铺: a star costs ten percent less per level.
    Smithy,
    /// 道场: gathering pays ten percent more skill xp per level.
    Dojo,
}

impl Building {
    /// The four, in the order the page reads them.
    pub const ALL: [Building; 4] = [Self::Workshop, Self::Warehouse, Self::Smithy, Self::Dojo];

    /// The catalog key naming this building.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Workshop => "cultivation.building.workshop",
            Self::Warehouse => "cultivation.building.warehouse",
            Self::Smithy => "cultivation.building.smithy",
            Self::Dojo => "cultivation.building.dojo",
        }
    }

    /// What one level of this building is worth, in parts per thousand of the
    /// thing it multiplies. 工坊 and 道场 are ten percent, 仓库 a quarter,
    /// 铁匠铺 ten percent (of a cost, so it *lightens*).
    pub const fn permille(self) -> u32 {
        match self {
            Self::Workshop | Self::Smithy | Self::Dojo => 100,
            Self::Warehouse => 250,
        }
    }
}

/// The most levels any building can hold.
pub const MAX_BUILDING_LEVEL: u8 = 10;

/// The most levels a trade (伐木, 采石) can hold. Same ceiling as a building,
/// but a separate name: the two are raised by different currencies.
pub const MAX_TRADE_LEVEL: u8 = 10;

/// How many of the common find one collection turns up, before the skill and
/// the 工坊.
pub const BASE_COMMON_YIELD: u32 = 10;

/// How many of a rare find one collection turns up, before the skill and the
/// 工坊. Lower than the common find on purpose: a rare find is the point of
/// gathering deeper, so it is *rarer*, not merely different.
pub const BASE_RARE_YIELD: u32 = 3;

/// The 采石 level at which 铁 starts turning up.
pub const IRON_UNLOCK_LEVEL: u8 = 3;

/// The 伐木 level at which 煤 starts turning up.
pub const COAL_UNLOCK_LEVEL: u8 = 5;

/// What one gathering pays as skill xp, before the 道场.
pub const BASE_SKILL_XP: u32 = 100;

/// The skill xp a level costs, at the level it is being earned for.
///
/// Flat per level: a level's price is the level's own number of a hundred, so
/// the first level is the cheapest and each one is a step up — the same shape
/// the garden's 造化 uses. Ten levels is a thousand and a hundred points.
pub fn skill_xp_total(level: u8) -> u32 {
    let level = u32::from(level.max(1));
    level * 100 + level.saturating_mul(level - 1) * 25
}

/// What one gathering pays for one trade, given the two trade levels, the 工坊
/// and the 道场.
///
/// `site` decides what is gathered: a forest pays 木材 (and 煤 once 伐木 has
/// earned it), a mountain pays 石块 (and 铁 once 采石 has), a mine pays both
/// rare finds. The common find is the trade's own and scales with that trade's
/// level; the rare find scales with the *gathering* level too, because a
/// trained hand brings home more of what it is trained to find.
pub struct CollectionYield {
    /// What the purse gains.
    pub gained: ResourceBag,
    /// The 伐木 xp this collection banks. The forest trains 伐木 and nothing
    /// else, the mountain trains 采石 and nothing else, and the mine trains both.
    pub woodcut_xp: u32,
    /// The 采石 xp this collection banks.
    pub stonecut_xp: u32,
}

impl CollectionYield {
    /// The xp one trade banks from this collection.
    pub const fn xp_for(&self, skill: GatherSkill) -> u32 {
        match skill {
            GatherSkill::Woodcut => self.woodcut_xp,
            GatherSkill::Stonecut => self.stonecut_xp,
        }
    }
}

/// One collection at `site`, by a cultivator with the two trade levels and the
/// two building levels that touch it.
pub fn collect(
    site: GatherSite,
    woodcut: u8,
    stonecut: u8,
    workshop: u8,
    dojo: u8,
) -> CollectionYield {
    let (woodcut_xp, stonecut_xp) = match site {
        GatherSite::Forest => (scale_xp(dojo), 0),
        GatherSite::Mountain => (0, scale_xp(dojo)),
        // The mine works both trades and pays both rare finds: it is the place
        // a cultivator goes when the two trained hands are worth more together.
        GatherSite::Mine => (scale_xp(dojo), scale_xp(dojo)),
    };
    let (skill_level, common, rare, rare_unlocked) = match site {
        GatherSite::Forest => (
            woodcut,
            Resource::Wood,
            Resource::Coal,
            woodcut >= COAL_UNLOCK_LEVEL,
        ),
        GatherSite::Mountain => (
            stonecut,
            Resource::Stone,
            Resource::Iron,
            stonecut >= IRON_UNLOCK_LEVEL,
        ),
        // The mine pays only the two rare finds: it is the place a cultivator
        // goes when the two trained hands are worth more together, and what it
        // turns up is what the two trades have earned — no common find at all.
        GatherSite::Mine => {
            let iron = stonecut >= IRON_UNLOCK_LEVEL;
            let coal = woodcut >= COAL_UNLOCK_LEVEL;
            let mut gained = ResourceBag::default();
            let yield_ = |level: u8| scale_yield(BASE_RARE_YIELD, level, workshop);
            if iron {
                gained.add(Resource::Iron, yield_(stonecut));
            }
            if coal {
                gained.add(Resource::Coal, yield_(woodcut));
            }
            return CollectionYield {
                gained,
                woodcut_xp,
                stonecut_xp,
            };
        }
    };
    let mut gained = ResourceBag::default();
    gained.add(
        common,
        scale_yield(BASE_COMMON_YIELD, skill_level, workshop),
    );
    if rare_unlocked {
        gained.add(rare, scale_yield(BASE_RARE_YIELD, skill_level, workshop));
    }
    CollectionYield {
        gained,
        woodcut_xp,
        stonecut_xp,
    }
}

/// How many thousandths of a base round each realm above the first adds to a
/// gathering session's length.
///
/// The base is one hour; a 修习 session pays growth, but a session spent at a
/// site is worth a whole session's worth of *gathering*, and a higher realm is
/// worth more of it — one quarter of an hour per realm.
pub const GATHER_HOURS_PER_REALM_PERMILLE: u32 = 250;

/// The length of one gathering session, in thousandths of a base round, at a
/// given realm.
///
/// The base is one hour, and each realm above the first adds a quarter of it,
/// so 炼气 (index 0) is one hour and 仙人 (index 9) is three and a quarter.
pub fn gather_duration_permille(realm_index: u32) -> u32 {
    1_000 + realm_index.saturating_mul(GATHER_HOURS_PER_REALM_PERMILLE)
}

/// One gathering session of `duration_permille` thousandths of a base round:
/// what [`collect`] pays for one round, scaled by the session's length. A
/// session of 3.25 hours pays 3.25 rounds, so the same trained hands that
/// would gather ten wood in an hour gather thirty-two in the session.
pub fn collect_scaled(
    site: GatherSite,
    woodcut: u8,
    stonecut: u8,
    workshop: u8,
    dojo: u8,
    duration_permille: u32,
) -> CollectionYield {
    let base = collect(site, woodcut, stonecut, workshop, dojo);
    let mut gained = ResourceBag::default();
    for resource in Resource::ALL {
        let count = base.gained.get(resource);
        if count > 0 {
            gained.add(resource, count.saturating_mul(duration_permille) / 1_000);
        }
    }
    CollectionYield {
        gained,
        woodcut_xp: base.woodcut_xp.saturating_mul(duration_permille) / 1_000,
        stonecut_xp: base.stonecut_xp.saturating_mul(duration_permille) / 1_000,
    }
}

/// What an expedition of `hours` at `site` pays.
///
/// The realm sets the *per-hour* rate — the same [`gather_duration_permille`]
/// a gathering session reads, so a 仙人 hand is worth 3.25 rounds an hour
/// against 炼气's one — and the length is the fixed choice the user pulled.
/// A four-hour expedition is four hours of the realm's rate; a twelve-hour one
/// is twelve. The 仓库's ceiling still clips the purse, the way any collection
/// does.
pub fn expedition_yield(
    site: GatherSite,
    woodcut: u8,
    stonecut: u8,
    workshop: u8,
    dojo: u8,
    realm_index: u32,
    hours: super::explore::ExplorationHours,
) -> CollectionYield {
    let permille = gather_duration_permille(realm_index).saturating_mul(u32::from(hours.hours()));
    collect_scaled(site, woodcut, stonecut, workshop, dojo, permille)
}

/// What a finished pomodoro pays.
///
/// The mode is one thing and one thing only: a session either cultivates, or
/// gathers at one site, never both — a pomodoro is a unit of effort, and effort
/// spent 伐木 is not effort spent 修习. This is what the user picks before the
/// session starts; the completion reads it and pays the one thing it names.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum SessionMode {
    /// The ordinary session: growth, the selected 功法's turn, the 灵石 cut, a
    /// 试炼 chance and the one roll.
    #[default]
    Study,
    /// The session is spent 伐木: it pays the forest's yield and 伐木 xp.
    Woodcut,
    /// The session is spent 采石: it pays the mountain's yield and 采石 xp.
    Mine,
}

impl SessionMode {
    /// The site a gathering session works, or `None` for a 修习 session.
    pub const fn site(self) -> Option<GatherSite> {
        match self {
            Self::Study => None,
            Self::Woodcut => Some(GatherSite::Forest),
            Self::Mine => Some(GatherSite::Mountain),
        }
    }

    /// The catalog key naming this mode.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Study => "cultivation.mode.study",
            Self::Woodcut => "cultivation.mode.woodcut",
            Self::Mine => "cultivation.mode.mine",
        }
    }
}

/// The yield of one find at a given trade level and 工坊 level.
fn scale_yield(base: u32, trade_level: u8, workshop: u8) -> u32 {
    let trade = 1_000 + u32::from(trade_level) * 50;
    let workshop_m = 1_000 + u32::from(workshop) * Building::Workshop.permille();
    base.saturating_mul(trade).saturating_mul(workshop_m) / 1_000 / 1_000
}

/// The skill xp one gathering pays at a given 道场 level.
fn scale_xp(dojo: u8) -> u32 {
    let dojo_m = 1_000 + u32::from(dojo) * Building::Dojo.permille();
    BASE_SKILL_XP.saturating_mul(dojo_m) / 1_000
}

/// Whether `level` has enough banked xp to reach `level + 1`.
pub fn skill_ready(level: u8, xp: u32) -> bool {
    xp >= skill_xp_total(level.saturating_add(1))
}

/// A price the purse pays: a 灵石 wager and the raw tribute the forge burns.
///
/// One type for both the 升星 wager and a building's upgrade, so a surface reads
/// the whole price one way and the state machine spends it atomically — a star
/// is never half-paid, and neither is a 工坊.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Cost {
    /// The 灵石 wager.
    pub stones: u32,
    /// The resources the forge burns, wood and stone at every star, iron once
    /// the star being reached is a high one.
    pub resources: ResourceBag,
}

/// Wood each 升星 attempt burns.
pub const STAR_WOOD_PER_STAR: u32 = 3;

/// Stone each 升星 attempt burns.
pub const STAR_STONE_PER_STAR: u32 = 2;

/// Iron an attempt at a high star burns, on top of the wood and the stone.
pub const STAR_IRON_HIGH_STAR: u32 = 2;

/// The star at which an attempt starts burning iron: the star being reached.
///
/// A piece at five stars reaching its sixth is the first attempt that does;
/// the sixth through the tenth are the high stars, and each one is a find the
/// 采石 trade had to be trained to bring home.
pub const STAR_IRON_FROM_STAR: u8 = 6;

/// The 铁匠铺's discount on the 灵石 wager of one 升星 attempt, in parts per
/// thousand.
///
/// Multiplicative — 0.9^level — so a level-ten forge cuts the wager to about
/// a third instead of to nothing: a discount that reached zero would make the
/// last stars free, and a 升星 that costs nothing is not a 升星. The resource
/// tribute is the forge's raw material and does not get cheaper: what the
/// smithy's craft saves is the 灵石, not the ore it has to have.
pub fn smithy_multiplier_permille(smithy_level: u8) -> u32 {
    let mut permille = 1_000u32;
    for _ in 0..smithy_level.min(MAX_BUILDING_LEVEL) {
        permille = permille.saturating_mul(900) / 1_000;
    }
    permille
}

/// How much of one resource a purse with no 仓库 holds. Per kind, so the purse
/// can hold two hundred of each of the four — a cap is a shape, not a total.
pub const BASE_RESOURCE_CAP: u32 = 200;

/// The purse's ceiling for one resource at a given 仓库 level: a quarter more
/// per level, so a full 仓库 holds three and a half times a bare purse.
pub fn resource_cap(warehouse_level: u8) -> u32 {
    BASE_RESOURCE_CAP
        .saturating_mul(1_000 + u32::from(warehouse_level) * Building::Warehouse.permille())
        / 1_000
}

/// The stone cost of one 工坊 rank, the way a rank is priced everywhere: the
/// rank's own number of a forty, so the first rank is the cheapest and each
/// one is a step up. (Kept beside the building so the price and the building
/// cannot drift.)
pub const BUILDING_STONES_PER_RANK: u32 = 40;

/// What one rank of a building costs at `current_level`, the rank being bought
/// being the next one.
///
/// A rank wants stones and wood and stone, and iron once the rank being bought
/// is a high one — the same shape as a 升星, because a building is the same
/// kind of spend: the forge's craft for the 灵石, the gathered tribute for the
/// material.
pub fn building_upgrade_cost(current_level: u8) -> Cost {
    let rank = u32::from(current_level) + 1;
    let mut resources = ResourceBag::default();
    resources.add(Resource::Wood, 4 * rank);
    resources.add(Resource::Stone, 4 * rank);
    if rank >= u32::from(STAR_IRON_FROM_STAR) {
        resources.add(Resource::Iron, 2 * rank);
    }
    Cost {
        stones: BUILDING_STONES_PER_RANK * rank,
        resources,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_forest_gathers_wood_and_never_rare_until_the_trade_is_trained() {
        let early = collect(GatherSite::Forest, 1, 1, 0, 0);
        assert!(early.gained.wood > 0);
        assert_eq!(early.gained.coal, 0, "伐木 at one has not earned 煤 yet");
        assert_eq!(early.xp_for(GatherSkill::Woodcut), 100);

        let trained = collect(GatherSite::Forest, COAL_UNLOCK_LEVEL, 1, 0, 0);
        assert!(trained.gained.wood > 0);
        assert!(trained.gained.coal > 0, "伐木 at five finds 煤");
    }

    #[test]
    fn a_mountain_gathers_stone_and_unlocks_iron_at_three() {
        let early = collect(GatherSite::Mountain, 1, 2, 0, 0);
        assert!(early.gained.stone > 0);
        assert_eq!(early.gained.iron, 0, "采石 at two has not earned 铁 yet");

        let trained = collect(GatherSite::Mountain, 1, IRON_UNLOCK_LEVEL, 0, 0);
        assert!(trained.gained.iron > 0);
    }

    #[test]
    fn the_mine_pays_what_the_two_trades_have_earned() {
        let untrained = collect(GatherSite::Mine, 1, 1, 0, 0);
        assert!(
            untrained.gained.is_empty(),
            "the mine pays nothing to hands that have earned neither find"
        );

        let trained = collect(GatherSite::Mine, COAL_UNLOCK_LEVEL, IRON_UNLOCK_LEVEL, 0, 0);
        assert!(trained.gained.iron > 0);
        assert!(trained.gained.coal > 0);
    }

    #[test]
    fn a_trade_level_and_the_workshop_compound_the_yield() {
        let base = collect(GatherSite::Forest, 1, 1, 0, 0).gained.wood;
        let skilled = collect(GatherSite::Forest, 5, 1, 0, 0).gained.wood;
        assert!(
            skilled > base,
            "five levels of 伐木 should gather more than one: {skilled} vs {base}"
        );

        let built = collect(GatherSite::Forest, 1, 1, 5, 0).gained.wood;
        assert!(
            built > base,
            "a five-level 工坊 should gather more: {built} vs {base}"
        );
    }

    #[test]
    fn the_dojo_raises_the_xp_a_gathering_pays() {
        let base = collect(GatherSite::Forest, 1, 1, 0, 0).xp_for(GatherSkill::Woodcut);
        let trained = collect(GatherSite::Forest, 1, 1, 0, 5).xp_for(GatherSkill::Woodcut);
        assert!(trained > base, "道场 xp {trained} vs {base}");
    }

    #[test]
    fn a_level_costs_more_than_the_one_before_it() {
        let mut prev = 0;
        for level in 1..=8 {
            let cost = skill_xp_total(level);
            assert!(cost > prev, "level {level} is not dearer than the last");
            prev = cost;
        }
    }

    #[test]
    fn the_bag_spends_only_what_it_holds() {
        let mut bag = ResourceBag::default();
        bag.add(Resource::Wood, 5);
        assert!(bag.can_spend(Resource::Wood, 5));
        assert!(!bag.can_spend(Resource::Wood, 6));
        assert!(!bag.can_spend(Resource::Iron, 1));
        assert!(bag.spend(Resource::Wood, 5));
        assert_eq!(bag.wood, 0);
        assert!(!bag.spend(Resource::Wood, 1), "nothing left to spend");
    }

    #[test]
    fn a_whole_tribute_is_spent_atomically_or_not_at_all() {
        let mut purse = ResourceBag::default();
        purse.add(Resource::Wood, 2);
        purse.add(Resource::Stone, 2);
        let tribute = Cost {
            stones: 0,
            resources: ResourceBag {
                wood: 3,
                stone: 2,
                iron: 0,
                coal: 0,
            },
        };
        assert!(!purse.can_spend_bag(&tribute.resources), "one wood short");
        purse.add(Resource::Wood, 1);
        assert!(purse.can_spend_bag(&tribute.resources));
        assert!(purse.spend_bag(&tribute.resources));
        assert_eq!(purse.wood, 0, "the tribute took every wood");
        assert_eq!(purse.stone, 0);
        assert!(!purse.spend_bag(&tribute.resources), "nothing left");
    }

    #[test]
    fn the_warehouse_widens_the_purse() {
        assert_eq!(resource_cap(0), BASE_RESOURCE_CAP);
        assert!(
            resource_cap(5) > resource_cap(0),
            "a warehouse should hold more than a bare purse"
        );
        assert!(
            resource_cap(10) > resource_cap(5),
            "and a taller one still more"
        );
    }

    #[test]
    fn the_mine_trains_both_trades() {
        let yield_ = collect(GatherSite::Mine, 1, 1, 0, 0);
        assert!(yield_.xp_for(GatherSkill::Woodcut) > 0);
        assert!(yield_.xp_for(GatherSkill::Stonecut) > 0);
        let forest = collect(GatherSite::Forest, 1, 1, 0, 0);
        assert!(forest.xp_for(GatherSkill::Woodcut) > 0);
        assert_eq!(forest.xp_for(GatherSkill::Stonecut), 0);
    }

    #[test]
    fn a_building_rank_costs_more_than_the_one_before_it() {
        let mut prev = 0;
        for level in 0..=8 {
            let cost = building_upgrade_cost(level);
            let total = cost.stones + cost.resources.total();
            assert!(total > prev, "rank {level} is not dearer than the last");
            prev = total;
        }
        // The high ranks are where the iron begins, the same as a 升星.
        assert_eq!(building_upgrade_cost(0).resources.iron, 0);
        assert!(
            building_upgrade_cost(STAR_IRON_FROM_STAR - 1)
                .resources
                .iron
                > 0,
            "a rank bought at the high star's level burns iron"
        );
    }

    #[test]
    fn a_gathering_session_is_the_realm_length_in_hours() {
        assert_eq!(gather_duration_permille(0), 1_000, "炼气 is one hour");
        assert_eq!(gather_duration_permille(9), 3_250, "仙人 is 3.25 hours");
        for realm in 0..=9 {
            let permille = gather_duration_permille(realm);
            assert!(
                (1_000..=3_250).contains(&permille),
                "a session is an hour and a quarter per realm, not {permille}"
            );
            if realm > 0 {
                assert_eq!(
                    permille,
                    gather_duration_permille(realm - 1) + 250,
                    "each realm adds a quarter hour"
                );
            }
        }
    }

    #[test]
    fn a_scaled_session_pays_proportionally() {
        let one = collect(GatherSite::Forest, 1, 1, 0, 0);
        let three = collect_scaled(GatherSite::Forest, 1, 1, 0, 0, 3_000);
        assert_eq!(three.gained.wood, one.gained.wood * 3);
        assert_eq!(three.woodcut_xp, one.woodcut_xp * 3);
        assert_eq!(
            three.stonecut_xp,
            one.stonecut_xp * 3,
            "forest trains 伐木 alone"
        );
        // The rare find scales too, so a trained hand's deeper haul is not a
        // flat number.
        let trained = collect(GatherSite::Mountain, 1, IRON_UNLOCK_LEVEL, 0, 0);
        let trained_session =
            collect_scaled(GatherSite::Mountain, 1, IRON_UNLOCK_LEVEL, 0, 0, 2_500);
        assert_eq!(trained_session.gained.iron, trained.gained.iron * 25 / 10);
        assert_eq!(trained_session.gained.stone, trained.gained.stone * 25 / 10);
    }

    #[test]
    fn the_mode_names_one_site_or_none() {
        assert_eq!(SessionMode::Study.site(), None, "修习 pays no gathering");
        assert_eq!(SessionMode::Woodcut.site(), Some(GatherSite::Forest));
        assert_eq!(SessionMode::Mine.site(), Some(GatherSite::Mountain));
        assert_ne!(
            SessionMode::Woodcut.site(),
            SessionMode::Mine.site(),
            "伐木 and 采石 are different sites"
        );
    }
}
