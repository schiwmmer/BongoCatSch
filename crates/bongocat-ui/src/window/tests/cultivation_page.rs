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

/// The four disciplines, their total, and the fight record are all on the page.
///
/// Both sections exist to turn a number the user cannot move into numbers they
/// can: a page that drew neither would still say what a pomodoro pays, and
/// nothing about how it got there. So the save handed to the page has trained all
/// four dimensions and fought a few fights, and every part has to be on screen.
#[gpui_kit::test]
fn the_page_draws_the_disciplines_and_the_record(cx: &mut TestAppContext) {
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
        }),
        ..CultivationDocument::default()
    })
    .view();
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    for id in [
        "cultivation-disciplines",
        "cultivation-disciplines-total",
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
/// disciplines and the two schools.
///
/// Both exist for the same reason — four attributes, four disciplines, two
/// schools and eight derived numbers on one page read as one pile of numbers
/// unless something says which layer each belongs to.
#[gpui_kit::test]
fn the_page_explains_which_layer_each_number_belongs_to(cx: &mut TestAppContext) {
    let cultivation = Cultivation::new(CultivationDocument::default()).view();
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    assert!(painted(visual, "cultivation-disciplines"));
    assert!(painted(visual, "cultivation-power-schools"));
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
