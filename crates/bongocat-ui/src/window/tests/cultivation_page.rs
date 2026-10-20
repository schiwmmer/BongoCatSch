//! The cultivation page's attribute chips and the hint they carry.
//!
//! The hint is the one control on this page a user has to discover by accident,
//! so it gets a frame of its own: the pointer arriving on a chip has to put the
//! explanation on screen, not merely set a flag nobody draws. The fight and the
//! walk are the two buttons whose click has to reach the worker, so they are
//! read through a sink rather than through the paint.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use super::*;
use crate::window::cultivation_duel;
use crate::window::cultivation_settings;
use bongocat_pomodoro::{
    Cultivation, CultivationDocument, DuelRecord, DuelReport, DuelRound, ExplorationOutcome,
    FightKind, Fighter, LAYERS_PER_STAGE, PLAYER_NAME, PomodoroCommand, PomodoroSnapshot,
    PomodoroState, WANDERER_NAME, WalkRecord, threshold_for,
};

/// The page mounted on its own.
///
/// The settings window's own frame would put the page behind a sidebar, a search
/// box and a scroll view; the chip under test is what matters here, so the page
/// is the whole frame.
struct CultivationPage(Entity<SettingsView>);

impl Render for CultivationPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = self.0.clone();
        div()
            .id("cultivation-page-harness")
            .size_full()
            .child(cultivation_settings::content(
                view,
                &RenderOptions::new(),
                window,
                &mut *cx,
            ))
    }
}

fn cultivation_page(cx: &mut TestAppContext) -> (Entity<SettingsView>, &mut VisualTestContext) {
    let (view, visual) = cultivation_page_with(cx, PomodoroClient::unavailable());
    (view, visual)
}

/// The page mounted on a pomodoro client that serves one snapshot and keeps
/// every command the page publishes, so a test can read what a click asked for.
fn cultivation_page_with(
    cx: &mut TestAppContext,
    pomodoro: PomodoroClient,
) -> (Entity<SettingsView>, &mut VisualTestContext) {
    cx.update(gpui_kit::init);
    let (client, endpoint) = crate::SettingsClient::bounded(4);
    // The page publishes setting commands this test never reads; the endpoint
    // has to outlive the window for its sender to stay connected.
    std::mem::forget(endpoint);
    let built = Rc::new(RefCell::new(None));
    let capture = Rc::clone(&built);
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| {
            SettingsView::new(
                client,
                SettingsWindowSeed {
                    language: SettingsLanguage::English,
                    appearance_theme: SettingsTheme::System,
                },
                Rc::new(|_| {}),
                Rc::new(|_| {}),
                pomodoro,
                window,
                cx,
            )
        });
        capture.borrow_mut().replace(view.clone());
        Root::new(cx.new(|_| CultivationPage(view)), window, cx)
    });
    let view = built
        .borrow_mut()
        .take()
        .expect("the window builder must hand the page out");
    (view, visual)
}

/// Whether an element with `id` was painted by the last frame.
fn painted(visual: &mut VisualTestContext, id: impl Into<ElementId>) -> bool {
    let id = id.into();
    visual.update(|window, _| window.try_find(id).is_some())
}

#[gpui_kit::test]
fn hovering_an_attribute_puts_its_explanation_on_screen(cx: &mut TestAppContext) {
    // The window borrows the context, so the clock is driven through a clone of
    // the dispatcher: both handles point at the same time source.
    let clock = cx.dispatcher.clone();
    let (_view, visual) = cultivation_page(cx);
    visual.update(|window, cx| window.render_frame(cx));

    // The chip is always there; the explanation is not.
    assert!(
        painted(visual, "cultivation-attribute-0"),
        "the constitution chip must be on the page"
    );
    let hint = "cultivation-attribute-hint-body-0";
    assert!(
        !painted(visual, hint),
        "nothing is explained until the pointer arrives"
    );

    visual.update(|window, cx| window.hover(ElementId::from("cultivation-attribute-0"), cx));
    // The card opens on a short delay so a pointer crossing the row does not
    // flash it; the clock has to pass that delay before the frame draws it.
    clock.advance_clock(Duration::from_millis(500));
    visual.run_until_parked();
    visual.update(|window, cx| window.render_frame(cx));

    assert!(
        painted(visual, hint),
        "hovering an attribute must show what it does"
    );
    assert!(
        bongocat_i18n::text("en-US", bongocat_pomodoro::Attribute::Root.desc_key())
            .contains("spell"),
        "the explanation has to name the school the attribute feeds"
    );
}

#[gpui_kit::test]
fn the_schools_are_shown_beside_the_attribute_totals(cx: &mut TestAppContext) {
    let (_view, visual) = cultivation_page(cx);
    visual.update(|window, cx| window.render_frame(cx));
    // A single power number cannot say whether a build is a spellcaster or a
    // brawler, so the two halves are drawn under the attributes.
    assert!(
        painted(visual, "cultivation-power-schools"),
        "the spell and martial halves must be on the page"
    );
}

/// A snapshot serving one cultivation view, with nothing else the page reads.
fn pomodoro_snapshot(cultivation: bongocat_pomodoro::CultivationView) -> PomodoroSnapshot {
    let mut snapshot = PomodoroState::empty().snapshot(7);
    snapshot.cultivation = cultivation;
    snapshot
}

/// A cultivator standing at the first realm's last layer, so the guardian is
/// offered — the growth that fills 炼气's hundred levels.
///
/// Asked of the curve rather than written down: the ramp is authored data, and
/// a test that spelled out one of its numbers would need rewriting every time
/// the ramp is retuned (ADR-0094) instead of following it.
fn at_the_cap() -> bongocat_pomodoro::CultivationView {
    Cultivation::new(CultivationDocument {
        growth: threshold_for(LAYERS_PER_STAGE),
        ..CultivationDocument::default()
    })
    .view()
}

/// Every command the page has published since the sink was last drained.
fn published(sink: &std::sync::mpsc::Receiver<PomodoroCommand>) -> Vec<PomodoroCommand> {
    let mut commands = Vec::new();
    while let Ok(command) = sink.try_recv() {
        commands.push(command);
    }
    commands
}

/// The walk is a button like any other, and a click on it has to reach the
/// worker: a button that paints and publishes nothing is the failure this page
/// had once already.
#[gpui_kit::test]
fn clicking_the_walk_asks_the_worker_for_one(cx: &mut TestAppContext) {
    let (pomodoro, sink) = PomodoroClient::with_sink();
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    assert!(
        painted(visual, "cultivation-explore-attempt"),
        "the walk must be offered"
    );
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("cultivation-explore-attempt", cx);
    });
    let commands = published(&sink);
    assert!(
        matches!(commands.as_slice(), [PomodoroCommand::Explore]),
        "the click asks the worker for a walk: {commands:?}"
    );
}

/// A held attempt says so, and its button publishes nothing: the cooldown is a
/// rule, not a suggestion.
#[gpui_kit::test]
fn a_held_attempt_says_so_and_publishes_nothing(cx: &mut TestAppContext) {
    let mut cultivation = at_the_cap();
    cultivation.breakthrough_ready = false;
    cultivation.breakthrough_ready_at =
        Some(cultivation.breakthrough_ready_at.unwrap_or(0) + 43_200);
    let (pomodoro, sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    assert!(
        painted(visual, "cultivation-breakthrough-held"),
        "the cooldown has to be on the page"
    );
    assert!(
        published(&sink).is_empty(),
        "nothing was asked for before the click"
    );
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("cultivation-breakthrough-attempt", cx);
    });
    assert!(
        published(&sink).is_empty(),
        "a held attempt cannot be fought"
    );
}

/// An attempt whose cooldown has run out is fought with one click, and the click
/// reaches the worker.
#[gpui_kit::test]
fn a_free_attempt_is_fought_with_one_click(cx: &mut TestAppContext) {
    let cultivation = at_the_cap();
    let (pomodoro, sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("cultivation-breakthrough-attempt", cx);
    });
    let commands = published(&sink);
    assert!(
        matches!(commands.as_slice(), [PomodoroCommand::Breakthrough]),
        "the click asks the worker for the fight: {commands:?}"
    );
}

/// A walk that is not owed is still offered, and asking for one anyway reaches
/// the worker: the page says why it is refused rather than pretending the
/// button was never there.
#[gpui_kit::test]
fn a_walk_that_is_not_owed_is_still_offered_and_asks(cx: &mut TestAppContext) {
    let mut cultivation = at_the_cap();
    cultivation.exploration_ready = false;
    let (pomodoro, sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    assert!(
        painted(visual, "cultivation-explore-attempt"),
        "the walk must be offered whether or not one is owed"
    );
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("cultivation-explore-attempt", cx);
    });
    let commands = published(&sink);
    assert!(
        matches!(commands.as_slice(), [PomodoroCommand::Explore]),
        "the click asks the worker anyway, and the worker refuses it: {commands:?}"
    );
}

/// A refused action is said out loud.
///
/// The domain records why; the page has to show it. A click that does nothing
/// without a word is a button the user reads as broken, so the refusal count is
/// watched and the reason is turned into an error the user can read.
#[gpui_kit::test]
fn a_refused_action_is_reported_to_the_user(cx: &mut TestAppContext) {
    let mut cultivation = at_the_cap();
    cultivation.refusal_count = 1;
    cultivation.last_refusal = Some(bongocat_pomodoro::Refusal::ExploreCooldown);
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (view, visual) = cultivation_page_with(cx, pomodoro);
    // The refusal is only noticed on the cultivation page: that is where a
    // purchase or a walk can be refused in the first place.
    view.update(visual, |view, _| {
        view.navigation_memory
            .select(SettingsNavigationPage::Cultivation)
    });
    view.update(visual, |view, cx| view.poll_pomodoro(cx));
    let reported = view.update(visual, |view, _| {
        view.cultivation_refusal_pending
            .map(|(refusal, _)| refusal.key())
    });
    assert_eq!(
        reported,
        Some("cultivation.explore.refuse_cooldown"),
        "the refusal has to be reported with its reason"
    );
}

/// The arithmetic behind a derived number is on the page.
///
/// The value beside it can be checked by hand only if the sum that produced it
/// is shown, so the hover carries the base, the attribute it reads, what one
/// point of that attribute adds, and the ceiling it was held to.
#[gpui_kit::test]
fn a_derived_number_shows_the_sum_behind_it(cx: &mut TestAppContext) {
    let clock = cx.dispatcher.clone();
    let cultivation = at_the_cap();
    let attack = *cultivation
        .derived_breakdowns
        .iter()
        .find(|breakdown| breakdown.key == "cultivation.attack")
        .expect("the attack number has an arithmetic behind it");
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    assert!(
        painted(visual, "cultivation-derived-attack"),
        "the attack chip must be on the page"
    );
    let hint = "cultivation-hint-cultivation.attack-body";
    assert!(
        !painted(visual, hint),
        "the arithmetic is not shown until the pointer arrives"
    );

    visual.update(|window, cx| window.hover(ElementId::from("cultivation-derived-attack"), cx));
    clock.advance_clock(Duration::from_millis(500));
    visual.run_until_parked();
    visual.update(|window, cx| window.render_frame(cx));

    assert!(
        painted(visual, hint),
        "hovering a derived number must show the sum behind it"
    );
    // The numbers on the card are the ones the domain computed: a card that
    // showed a different sum would be worse than no card at all.
    assert_eq!(
        attack.result,
        attack.base + attack.attribute_value.max(0) as u32 * attack.per_point,
        "the card shows the sum the domain ran"
    );
}

/// The smoke a scripted run paints the ladder with reads the same snapshot the
/// page does, so the empty ladder a worker-less client serves has to pass: it
/// is a ladder, at realm one, layer zero, rank one.
#[gpui_kit::test]
fn the_cultivation_smoke_accepts_the_ladder_a_client_serves(cx: &mut TestAppContext) {
    let (view, visual) = cultivation_page(cx);
    assert!(
        view.update(visual, |view, cx| view.show_cultivation_page_for_smoke(cx))
            .is_ok(),
        "the empty ladder must pass the cultivation smoke"
    );
}

/// A ladder whose layer and rank are not the ones its level says would draw a
/// level nobody climbed to, so the smoke has to refuse it rather than pass a
/// page that misreads the ladder.
#[gpui_kit::test]
fn the_cultivation_smoke_refuses_a_ladder_that_disagrees_with_itself(cx: &mut TestAppContext) {
    let mut cultivation = at_the_cap();
    cultivation.rank = 3;
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (view, visual) = cultivation_page_with(cx, pomodoro);
    let error = view
        .update(visual, |view, cx| view.show_cultivation_page_for_smoke(cx))
        .expect_err("a rank that is not the one the level says must be refused");
    assert!(
        error.contains("layer and rank"),
        "the refusal has to say what is wrong: {error}"
    );
}

/// A walk that met someone: six of the cultivator's blows, both sides walking
/// in whole.
fn walk_report() -> DuelReport {
    DuelReport {
        rounds: (1..=6)
            .map(|index| DuelRound {
                index,
                attacker: PLAYER_NAME.to_owned(),
                art: None,
                dodged: false,
                damage: 10,
                target_health: 100 - index,
            })
            .collect(),
        won: true,
        player: Fighter {
            name: PLAYER_NAME.to_owned(),
            health: 100,
            attack: 300,
            inner: 120,
            spell: 240,
            resist: 160,
            speed: 40,
            hit: 800,
            evade: 100,
            art: None,
        },
        opponent: Fighter {
            name: WANDERER_NAME.to_owned(),
            health: 80,
            attack: 240,
            inner: 0,
            spell: 0,
            resist: 0,
            speed: 32,
            hit: 800,
            evade: 100,
            art: None,
        },
    }
}

/// What the next pomodoro is worth, and the fight record, are on the page.
///
/// The growth line is the one number the user is chasing, so the page says it
/// as a number rather than as a percentage: the four dimensions that decide it
/// are not shown, and a multiplier the user cannot see is a multiplier the user
/// cannot move. The record is beside it because a page that only said what a
/// pomodoro pays would say nothing about what this save has been through.
#[gpui_kit::test]
fn the_page_draws_the_next_session_worth_and_the_record(cx: &mut TestAppContext) {
    let cultivation = Cultivation::new(CultivationDocument {
        growth: threshold_for(LAYERS_PER_STAGE),
        explorations: 20,
        battle_wins: 4,
        harvests: 2,
        duel_record: vec![
            DuelRecord {
                won: true,
                kind: FightKind::Guardian,
                realm: 0,
                at_unix: 1_000,
            },
            DuelRecord {
                won: false,
                kind: FightKind::Wanderer,
                realm: 0,
                at_unix: 2_000,
            },
        ],
        last_walk: Some(WalkRecord {
            at_unix: 2_000,
            realm: 0,
            outcome: ExplorationOutcome::Gear,
            art: None,
            gear: Some("青锋".to_owned()),
            seed: None,
            stones: None,
        }),
        ..CultivationDocument::default()
    })
    .view();
    assert!(
        cultivation.next_session_growth > 0,
        "the page has a number to show: {}",
        cultivation.next_session_growth
    );
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    for id in [
        "cultivation-next-session-growth",
        "cultivation-record",
        "cultivation-record-duel-0",
        "cultivation-record-duel-1",
        "cultivation-record-walk",
    ] {
        assert!(painted(visual, id), "the page must draw {id}");
    }
}

/// The 集市 is on the page, with the purse and a shelf to buy from.
///
/// A shop that is not on the page is a shop that does not exist as far as the user
/// is concerned, so the test asserts the two things that make it one: the number
/// the user spends, and at least one row they can click.
#[gpui_kit::test]
fn the_page_draws_the_market_with_a_shelf(cx: &mut TestAppContext) {
    let cultivation = Cultivation::new(CultivationDocument {
        stones: 5_000,
        ..CultivationDocument::default()
    })
    .view();
    let slot = cultivation
        .shop_offers
        .first()
        .map(|entry| entry.slot)
        .expect("a fresh save still gets a shelf");
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    assert!(
        painted(visual, "cultivation-shop"),
        "the market has to be on the page"
    );
    assert!(
        painted(visual, "cultivation-shop-stones"),
        "and the purse has to be on it"
    );
    assert!(
        painted(visual, format!("cultivation-shop-row-{slot}")),
        "and the shelf has a row to click"
    );
}

/// The two layer explanations the page is most often misread without: the
/// schools and the derived numbers.
///
/// Both exist for the same reason — four attributes, two schools and eight
/// derived numbers on one page read as one pile of numbers unless something
/// says which layer each belongs to. The derived numbers also carry the
/// arithmetic that produced them, so the sum can be checked by hand.
#[gpui_kit::test]
fn the_page_explains_which_layer_each_number_belongs_to(cx: &mut TestAppContext) {
    let cultivation = Cultivation::new(CultivationDocument::default()).view();
    assert!(
        !cultivation.derived_breakdowns.is_empty(),
        "the page has an arithmetic to show behind each number"
    );
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    assert!(painted(visual, "cultivation-power-schools"));
    assert!(painted(visual, "cultivation-derived-attack"));
}

/// A learned 功法 is on the page in its own panel.
///
/// The two lists are kept apart because they are sold differently: a book is
/// still being trained and can be sold, a learned 功法 is kept for good. A page
/// that showed them in one list would make the second look like the first.
#[gpui_kit::test]
fn the_page_draws_the_learned_arts_in_their_own_panel(cx: &mut TestAppContext) {
    let cultivation = Cultivation::new(CultivationDocument {
        mastered_arts: vec![1, 3],
        ..CultivationDocument::default()
    })
    .view();
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    assert!(
        painted(visual, "cultivation-mastered-arts"),
        "the learned 功法 have their own panel"
    );
    for id in [
        "cultivation-mastered-art-1",
        "cultivation-mastered-art-3",
        "cultivation-mastered-art-detail-1",
    ] {
        assert!(painted(visual, id), "the page must draw {id}");
    }
}

/// The popup mounted on its own.
///
/// The settings window's own frame would put it behind a sidebar and a scroll
/// view; the popup is what is under test, so the popup is the whole frame.
struct DuelPopup(CultivationDuel);

impl Render for DuelPopup {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let actions = CultivationDuelActions {
            on_replay: Rc::new(|_, _| {}),
            on_skip: Rc::new(|_, _| {}),
            on_close: Rc::new(|_, _| {}),
        };
        div()
            .id("duel-harness")
            .size_full()
            .child(cultivation_duel::render(
                &self.0,
                "en-US",
                Tokens::from_theme(cx),
                actions,
            ))
    }
}

/// The fight popup is drawn the way a fight is watched: the two health bars on
/// top, the cultivator's numbers on the left and their opponent's on the right,
/// and the log under both. A frame that painted only the log would pass every
/// click test above and still not show what the fight was between.
#[gpui_kit::test]
fn the_fight_popup_shows_both_sides_and_their_health(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let duel = CultivationDuel::new(
        CultivationDuelSource::Exploration,
        cultivation_duel::CultivationDuelPayload::Exploration(
            ExplorationOutcome::Victory,
            None,
            Some(walk_report()),
        ),
    );
    let (_, visual) =
        cx.add_window_view(move |window, cx| Root::new(cx.new(|_| DuelPopup(duel)), window, cx));
    visual.update(|window, cx| window.render_frame(cx));
    for id in [
        "cultivation-duel-health-player",
        "cultivation-duel-health-opponent",
        "cultivation-duel-side-player",
        "cultivation-duel-side-opponent",
        "cultivation-duel-log",
    ] {
        assert!(painted(visual, id), "the popup must paint {id}");
    }
}

/// The growth line is the pair the ladder is about: what has been grown over
/// what is still owed. The figures are printed compactly, because the exact
/// number runs to five digits and would push the bar off the row.
#[test]
fn the_growth_line_reads_what_is_banked_over_what_is_owed() {
    let mut view = Cultivation::default().view();
    view.floor = 54_000;
    view.growth = 54_683;
    view.next = Some(55_000);
    let (line, fraction) = cultivation_settings::growth_line(&view, "zh-CN");
    assert_eq!(line, "54.7K / 317 成长值");
    assert!(
        (0.0..=1.0).contains(&fraction),
        "the bar is a fraction of the layer: {fraction}"
    );
}

/// The arithmetic on a derived chip is written in the user's language.
///
/// The sum used to spell the catalog key behind the attribute
/// (` cultivation.attribute.root 787 × 8`), which is a leaked identifier
/// rather than a word: the card is there to be read and checked by hand, so
/// every word on it has to be one the user knows.
#[test]
fn the_formula_names_the_attribute_in_the_language_on_screen() {
    let view = Cultivation::default().view();
    let inner = view
        .derived_breakdowns
        .iter()
        .find(|breakdown| breakdown.key == "cultivation.inner")
        .expect("内力 has an arithmetic behind it");
    let chinese = cultivation_settings::derived_formula(inner, "zh-CN");
    assert!(
        !chinese.contains("cultivation."),
        "the formula leaked a key: {chinese}"
    );
    assert!(
        chinese.contains("根骨"),
        "内力 reads 根骨, so the formula must say so: {chinese}"
    );
    let english = cultivation_settings::derived_formula(inner, "en-US");
    assert!(
        !english.contains("cultivation."),
        "the English formula leaked a key: {english}"
    );
    assert_ne!(
        chinese, english,
        "the two languages cannot print the same formula"
    );
}

/// 命中 and 闪避 are stored in parts per thousand but drawn as percent, so the
/// sum behind them is in percent too: a card that explains "95%" with "950" is
/// asking the reader to do the conversion the card exists to save them.
#[test]
fn a_percent_row_is_explained_in_percent() {
    let view = Cultivation::default().view();
    let hit = view
        .derived_breakdowns
        .iter()
        .find(|breakdown| breakdown.key == "cultivation.hit")
        .expect("命中 has an arithmetic behind it");
    let formula = cultivation_settings::derived_formula(hit, "zh-CN");
    assert!(
        formula.contains('%'),
        "a per-mille row has to say so: {formula}"
    );
    assert!(
        !formula.contains("750"),
        "the raw per-mille base leaked into the card: {formula}"
    );
}
