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
    ArtOutcome, Attribute, Attributes, CropRecord, Cultivation, CultivationDocument, DailyKind,
    DailyOutcome, Derived, DuelReport, DuelRound, EventDetail, EventRecord, ExplorationFindView,
    ExplorationOutcome, Fighter, ItemView, LAYERS_PER_STAGE, OwnedArt, PLAYER_NAME, PlotRecord,
    PomodoroCommand, PomodoroSnapshot, PomodoroState, Rarity, SeedGrade, SeedStack, TrainingView,
    TrialBlocked, TrialOutcome, WANDERER_NAME, threshold_for,
};
use gpui_kit::{ScrollDelta, point};

/// The page mounted on its own.
///
/// The settings window's own frame would put the page behind a sidebar, a search
/// box and a scroll view; the chip under test is what matters here, so the page
/// is the whole frame.
struct CultivationPage(Entity<SettingsView>);

impl Render for CultivationPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = self.0.clone();
        // The page is taller than any window: it is a scrolling column of
        // panels, and the settlement harness has to reproduce that or the rows
        // past the fold are squeezed to nothing and cannot be clicked. The
        // settings frame supplies the same scroll view around this page.
        div()
            .id("cultivation-page-harness")
            .size_full()
            .overflow_y_scroll()
            .test_support()
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

/// Scroll the page until `id` is on screen.
///
/// The page is one column taller than the test window and every panel added to
/// it pushes the ones below further down, so a test that clicks has to bring the
/// row it wants into view first — exactly the way the real settings frame
/// scrolls. Scrolling in small steps and asking after each keeps the tests free
/// of the page's pixel height.
fn scroll_until_visible(visual: &mut VisualTestContext, id: &'static str) {
    for _ in 0..24 {
        let visible = visual.update(|window, _| {
            window
                .try_find(ElementId::from(id))
                .is_some_and(|snapshot| snapshot.visible())
        });
        if visible {
            return;
        }
        visual.update(|window, cx| {
            window.scroll(
                ElementId::from("cultivation-page-harness"),
                ScrollDelta::Lines(point(0.0, -6.0)),
                cx,
            );
            window.render_frame(cx);
        });
    }
    panic!("{id} never came into view");
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
    // The explanation says what the attribute pays, and no longer names a
    // school: neither 法术 nor 武术 survives on the page.
    let desc = bongocat_i18n::text("en-US", bongocat_pomodoro::Attribute::Root.desc_key());
    assert!(
        desc.contains("inner force"),
        "the explanation has to say what the attribute pays: {desc}"
    );
    assert!(
        !desc.contains("spell") && !desc.contains("martial"),
        "a school has no place on the card: {desc}"
    );
}

#[gpui_kit::test]
fn clicking_the_realm_lists_the_whole_ladder(cx: &mut TestAppContext) {
    let (view, visual) = cultivation_page(cx);
    visual.update(|window, cx| window.render_frame(cx));
    assert!(
        !painted(visual, "cultivation-realm-rung-0"),
        "the ladder is not on the page until the realm is clicked"
    );

    visual.update(|window, cx| window.click(ElementId::from("cultivation-realm-name"), cx));
    visual.update(|window, cx| window.render_frame(cx));
    let open = visual.update(|_, cx| view.read(cx).cultivation_ladder_open);
    assert!(open, "the click must unfold the ladder");

    // All ten rungs, in climbing order: the card is the ladder, not a hint.
    for rung in 0..10 {
        assert!(
            painted(visual, format!("cultivation-realm-rung-{rung}")),
            "rung {rung} is missing from the realm card"
        );
    }
    assert!(
        !painted(visual, "cultivation-realm-rung-10"),
        "there are ten realms, not eleven"
    );
}

#[gpui_kit::test]
fn the_two_art_powers_are_shown_beside_the_attribute_totals(cx: &mut TestAppContext) {
    let (_view, visual) = cultivation_page(cx);
    visual.update(|window, cx| window.render_frame(cx));
    // 法术 and 武术 used to sit here and said nothing a user could act on: the
    // two powers that do are the 内功 (a slice of the total) and the 外功 (which
    // pays no attribute at all and would otherwise be invisible).
    assert!(
        painted(visual, "cultivation-power-breakdown"),
        "the level · gear · crop · 内功 · 外功 line must be on the page"
    );
    assert!(
        !painted(visual, "cultivation-power-schools"),
        "the two schools are gone"
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
    scroll_until_visible(visual, "cultivation-explore-attempt");
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("cultivation-explore-attempt", cx);
    });
    let commands = published(&sink);
    assert!(
        matches!(commands.as_slice(), [PomodoroCommand::Explore { .. }]),
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
    scroll_until_visible(visual, "cultivation-breakthrough-attempt");
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

/// A walk under way is drawn as one: the panel counts down to its return and
/// offers the way home instead of a second walk the domain would refuse.
#[gpui_kit::test]
fn a_walk_under_way_offers_to_come_home(cx: &mut TestAppContext) {
    let mut cultivation = at_the_cap();
    cultivation.exploration_ready = false;
    cultivation.exploration_ready_at = None;
    cultivation.walk_early = false;
    let (pomodoro, sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    assert!(
        painted(visual, "cultivation-explore-return"),
        "a walk under way offers the way home"
    );
    assert!(
        !painted(visual, "cultivation-explore-attempt"),
        "and does not offer a second walk it would only refuse"
    );
    scroll_until_visible(visual, "cultivation-explore-return");
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("cultivation-explore-return", cx);
    });
    let commands = published(&sink);
    assert!(
        matches!(commands.as_slice(), [PomodoroCommand::ReturnWalk]),
        "the click asks the worker to call the walk home: {commands:?}"
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

/// The morning ambush opens once and is remembered.
///
/// The popup is a moment the window owns, and a window is rebuilt on every
/// launch: without the day being written down when the popup is acknowledged,
/// every relaunch replayed the last ambush the save has on file as if it had
/// just happened — the bug this guards. Three ways it used to come back: the
/// window rebuilt, the day rolled over under an old event, and a fight the
/// user never asked for opening on a page they only came to look at.
#[gpui_kit::test]
fn the_morning_ambush_opens_once_and_is_remembered(cx: &mut TestAppContext) {
    let clock = cx.dispatcher.clone();
    // A save whose day has already opened with a fight nobody has acknowledged.
    let mut cultivation = at_the_cap();
    cultivation.day = 40;
    cultivation.daily_event_day = Some(40);
    cultivation.daily_events = 3;
    cultivation.last_daily_event = Some(DailyOutcome {
        kind: DailyKind::Ambush,
        won: true,
        duel: Some(walk_report()),
        found: ExplorationFindView::default(),
    });
    let (pomodoro, sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation.clone()));
    let (view, visual) = cultivation_page_with(cx, pomodoro);
    // The poll serves the page the user is on, so the harness has to say it is
    // standing on the ladder.
    view.update(visual, |view, _| {
        let memory = SettingsNavigationMemory::new();
        memory.set_page_index(SettingsNavigationPage::Cultivation.index());
        view.set_navigation_memory(memory);
    });
    visual.update(|window, cx| window.render_frame(cx));
    clock.advance_clock(Duration::from_millis(600));
    visual.run_until_parked();
    let commands = published(&sink);
    assert!(
        matches!(
            commands.as_slice(),
            [PomodoroCommand::AcknowledgeDailyEvent { day: 40 }]
        ),
        "the popup is acknowledged once, with the day it belongs to: {commands:?}"
    );

    // The window rebuilt — the same save, now with the day written down — says
    // nothing. This is the relaunch that used to replay the ambush.
    let mut seen = cultivation;
    seen.daily_event_seen_day = Some(40);
    let (pomodoro, sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(seen.clone()));
    let (view, visual) = cultivation_page_with(cx, pomodoro);
    view.update(visual, |view, _| {
        let memory = SettingsNavigationMemory::new();
        memory.set_page_index(SettingsNavigationPage::Cultivation.index());
        view.set_navigation_memory(memory);
    });
    visual.update(|window, cx| window.render_frame(cx));
    clock.advance_clock(Duration::from_millis(600));
    visual.run_until_parked();
    assert!(
        published(&sink).is_empty(),
        "a day that has been seen is not opened again"
    );

    // And a later day never replays an older day's ambush: the event has to be
    // the day's own.
    let mut older = seen;
    older.day = 41;
    let (pomodoro, sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(older));
    let (view, visual) = cultivation_page_with(cx, pomodoro);
    view.update(visual, |view, _| {
        let memory = SettingsNavigationMemory::new();
        memory.set_page_index(SettingsNavigationPage::Cultivation.index());
        view.set_navigation_memory(memory);
    });
    visual.update(|window, cx| window.render_frame(cx));
    clock.advance_clock(Duration::from_millis(600));
    visual.run_until_parked();
    assert!(
        published(&sink).is_empty(),
        "an older day's ambush is not replayed on a later one"
    );
}

/// The wager is a button like any other, and a click on it reaches the worker
/// with the piece's id: the price on the button is the domain's own — half of
/// what the piece sells for — so the number the user reads is the number the
/// purse pays. A capped piece offers no button at all rather than a wager it
/// cannot hold.
#[gpui_kit::test]
fn clicking_a_star_asks_the_worker_for_the_wager(cx: &mut TestAppContext) {
    let mut cultivation = at_the_cap();
    let divine = ItemView {
        id: 42,
        slot: bongocat_pomodoro::EquipSlot::Weapon,
        tier: 5,
        rarity: Rarity::Divine,
        attributes: Attributes::new(0, 120, 40, 0),
        bonus: Derived::default(),
        equipped: false,
        star: 3,
        effect: Some(bongocat_pomodoro::GearEffect::Frost),
        star_cost: Some(350),
        name: Some("玄霜剑【寒霜】".to_owned()),
        sell_price: 700,
    };
    cultivation.inventory = vec![
        divine.clone(),
        ItemView {
            id: 43,
            star_cost: None,
            ..divine
        },
    ];
    let (pomodoro, sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    assert!(
        painted(visual, "cultivation-star-42"),
        "a piece with a star owed offers the wager"
    );
    assert!(
        !painted(visual, "cultivation-star-43"),
        "and a full piece offers nothing to wager on"
    );
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("cultivation-star-42", cx);
    });
    let commands = published(&sink);
    assert!(
        matches!(
            commands.as_slice(),
            [PomodoroCommand::StarEquipment { item_id: 42 }]
        ),
        "the click asks the worker for the wager: {commands:?}"
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
        events: vec![
            EventRecord {
                at_unix: 1_000,
                detail: EventDetail::Breakthrough {
                    won: true,
                    stage: Some(1),
                    stones: None,
                },
            },
            EventRecord {
                at_unix: 2_000,
                detail: EventDetail::Walk {
                    outcome: ExplorationOutcome::Gear,
                    art: None,
                    gear: Some("青锋".to_owned()),
                    seed: None,
                    stones: Some(5),
                },
            },
            EventRecord {
                at_unix: 3_000,
                detail: EventDetail::Session { growth: 500 },
            },
        ],
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
        "cultivation-event-0",
        "cultivation-event-1",
        "cultivation-event-2",
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

/// The day's specials are marked on the shelf: a row that is cut says so, and a
/// row that is not carries no badge at all.
#[gpui_kit::test]
fn the_shelf_says_which_rows_are_on_special(cx: &mut TestAppContext) {
    let cultivation = Cultivation::new(CultivationDocument {
        stones: 5_000,
        ..CultivationDocument::default()
    })
    .view();
    let special = cultivation
        .shop_offers
        .iter()
        .find(|entry| entry.discount_permille > 0)
        .map(|entry| entry.slot)
        .expect("a shelf always marks two rows");
    let plain = cultivation
        .shop_offers
        .iter()
        .find(|entry| entry.discount_permille == 0)
        .map(|entry| entry.slot)
        .expect("and leaves the rest at their price");
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    assert!(
        painted(visual, format!("cultivation-shop-special-{special}")),
        "the cut is marked beside the price it is a cut of"
    );
    assert!(
        !painted(visual, format!("cultivation-shop-special-{plain}")),
        "a row at its list price says nothing"
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
    assert!(painted(visual, "cultivation-power-breakdown"));
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
        "cultivation-mastered-art-payoff-1",
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

/// 命中 and 闪避 are plain numbers now, so the sum behind them is in the same
/// unit the chip prints: a card that explains "命中 200" with a percent sign
/// would be explaining a number with a different one.
#[test]
fn a_plain_row_is_explained_in_plain_numbers() {
    let view = Cultivation::default().view();
    let hit = view
        .derived_breakdowns
        .iter()
        .find(|breakdown| breakdown.key == "cultivation.hit")
        .expect("命中 has an arithmetic behind it");
    let formula = cultivation_settings::derived_formula(hit, "zh-CN");
    assert!(
        !formula.contains('%'),
        "a plain number row carries no percent sign: {formula}"
    );
    assert!(
        formula.contains("100"),
        "the base the domain actually starts from is on the card: {formula}"
    );
}

/// The record keeps the page to five lines and hands the rest to the popup.
///
/// Sixteen events on a page that is already a ladder would push everything
/// below them off the first screenful, so the sixth line on belongs to the
/// popup: the page draws five rows and the control, and the control's click
/// opens the history.
#[gpui_kit::test]
fn the_record_shows_five_events_and_offers_the_rest(cx: &mut TestAppContext) {
    let events: Vec<EventRecord> = (0..7)
        .map(|at| EventRecord {
            at_unix: at,
            detail: EventDetail::Session { growth: 500 },
        })
        .collect();
    let cultivation = Cultivation::new(CultivationDocument {
        events,
        ..CultivationDocument::default()
    })
    .view();
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    for id in [
        "cultivation-event-0",
        "cultivation-event-4",
        "cultivation-record-show-all",
    ] {
        assert!(painted(visual, id), "the page must draw {id}");
    }
    assert!(
        !painted(visual, "cultivation-event-5"),
        "the sixth event belongs to the popup, not the page"
    );
    visual.update(|window, cx| window.click("cultivation-record-show-all", cx));
    visual.update(|_, cx| {
        assert!(
            view.read(cx).cultivation_events_open,
            "the control's click opens the history"
        );
    });
}

/// A 功法 trained to its first 阶 is 已习得 as far as it has gone: it shows in
/// the learned panel with the 阶 it stands at, even though the training is not
/// finished. A book still at rank zero is not learned at all and shows in the
/// shelf alone.
#[gpui_kit::test]
fn the_learned_panel_lists_a_part_trained_art(cx: &mut TestAppContext) {
    let cultivation = Cultivation::new(CultivationDocument {
        arts: vec![OwnedArt {
            art_id: 1,
            rank: 1,
            progress: 0,
            outcome: None,
            failures: 0,
        }],
        ..CultivationDocument::default()
    })
    .view();
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    assert!(
        painted(visual, "cultivation-mastered-art-1"),
        "a partly trained 功法 is 已习得 as far as it has gone"
    );
}

/// The garden offers the next plot while it has one to sell: the four free
/// beds come with the garden, and the fifth is the first one 灵石 buys.
#[gpui_kit::test]
fn the_garden_offers_the_next_plot_for_sale(cx: &mut TestAppContext) {
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(
        Cultivation::new(CultivationDocument::default()).view(),
    ));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    assert!(
        painted(visual, "cultivation-garden"),
        "the garden is on the page"
    );
    assert!(
        painted(visual, "cultivation-buy-plot"),
        "the fifth plot is for sale"
    );
}

/// A seed says what it pays wherever it is looked at: in the satchel waiting to
/// be planted, and in the ground already growing.
///
/// The facts are the same three at every stage — the attribute, the points and
/// how long it takes — because it is the same seed; a page that only explained
/// a seed while it was for sale would leave the user guessing about the one
/// they already own.
#[gpui_kit::test]
fn a_seed_says_what_it_pays_at_every_stage(cx: &mut TestAppContext) {
    let cultivation = Cultivation::new(CultivationDocument {
        seeds: vec![SeedStack {
            attribute: Attribute::Root,
            tier: 3,
            grade: SeedGrade::Earth,
            count: 1,
            name: Some("青芝".to_owned()),
        }],
        plots: vec![PlotRecord {
            crop: Some(CropRecord::planted(
                Attribute::Mana,
                2,
                SeedGrade::Heaven,
                Some("玄莲".to_owned()),
                0,
            )),
        }],
        ..CultivationDocument::default()
    })
    .view();
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    visual.update(|window, cx| window.click("cultivation-seed-name-0-3-2", cx));
    visual.update(|window, cx| window.render_frame(cx));
    assert!(
        painted(visual, "cultivation-seed-detail-0-3-2"),
        "a seed in the satchel says what it pays"
    );
    visual.update(|window, cx| window.click("cultivation-plot-name-0", cx));
    visual.update(|window, cx| window.render_frame(cx));
    assert!(
        painted(visual, "cultivation-plot-detail-0"),
        "a seed in the ground says what it pays"
    );
}

/// The history popup mounted on its own, the way the fight popup is: the
/// settings frame would put it behind a sidebar and a scroll view, and the
/// popup is what is under test.
struct EventsPopup(Vec<EventRecord>);

impl Render for EventsPopup {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("events-harness")
            .size_full()
            .child(cultivation_settings::events_overlay(
                &self.0,
                "en-US",
                Tokens::from_theme(cx),
                Rc::new(|_, _| {}),
            ))
    }
}

/// The popup carries every event the save remembers, not the five the page
/// keeps: a history that quietly stopped where the page does is not full, so
/// the test counts past the cap.
#[gpui_kit::test]
fn the_history_popup_lists_every_event(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let events: Vec<EventRecord> = (0..7)
        .map(|at| EventRecord {
            at_unix: at,
            detail: EventDetail::Session { growth: 500 },
        })
        .collect();
    let (_, visual) = cx
        .add_window_view(move |window, cx| Root::new(cx.new(|_| EventsPopup(events)), window, cx));
    visual.update(|window, cx| window.render_frame(cx));
    for id in [
        "cultivation-events-panel",
        "cultivation-events-list",
        "cultivation-history-event-0",
        "cultivation-history-event-6",
    ] {
        assert!(painted(visual, id), "the popup must paint {id}");
    }
}

/// A 功法 on the shelf says where its training stands: the 阶 the grade has,
/// what the next one costs in pomodoros, and how far into it the book is.
/// A ladder that does not say how far along it is cannot be planned around.
#[gpui_kit::test]
fn the_art_row_shows_the_rank_ladder_and_its_progress(cx: &mut TestAppContext) {
    let cultivation = Cultivation::new(CultivationDocument {
        arts: vec![OwnedArt {
            art_id: 1,
            rank: 1,
            progress: 1,
            outcome: None,
            failures: 0,
        }],
        ..CultivationDocument::default()
    })
    .view();
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    for id in [
        "cultivation-art-1",
        "cultivation-art-payoff-1",
        "cultivation-art-progress-1",
    ] {
        assert!(painted(visual, id), "the page must draw {id}");
    }
}

/// The tower is a button like the walk: a click has to reach the worker.
#[gpui_kit::test]
fn clicking_the_tower_asks_the_worker_for_a_floor(cx: &mut TestAppContext) {
    let cultivation = Cultivation::new(CultivationDocument {
        growth: threshold_for(20),
        ..CultivationDocument::default()
    })
    .view();
    let (pomodoro, sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    for id in [
        "cultivation-trial",
        "cultivation-trial-floor",
        "cultivation-trial-status",
        "cultivation-trial-attempt",
    ] {
        assert!(painted(visual, id), "the tower must draw {id}");
    }
    // The tower is the third panel from the bottom of a long column, and the
    // rows above it are what push its button past the test window's fold. The
    // real settings frame scrolls here, so the harness does too.
    scroll_until_visible(visual, "cultivation-trial-attempt");
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("cultivation-trial-attempt", cx);
    });
    let commands = published(&sink);
    assert!(
        matches!(commands.as_slice(), [PomodoroCommand::Tower]),
        "the click asks the worker for a floor: {commands:?}"
    );
}

/// A closed tower says which wall was hit and cannot be clicked through: the
/// day's floors and the walk are rules, not suggestions.
#[gpui_kit::test]
fn a_closed_tower_says_why_and_publishes_nothing(cx: &mut TestAppContext) {
    let mut cultivation = Cultivation::default().view();
    cultivation.trial_attempts_left = 0;
    cultivation.trial_blocked = Some(TrialBlocked::Drained);
    let (pomodoro, sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    assert!(
        painted(visual, "cultivation-trial-status"),
        "the reason has to be on the page"
    );
    scroll_until_visible(visual, "cultivation-trial-attempt");
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("cultivation-trial-attempt", cx);
    });
    assert!(
        published(&sink).is_empty(),
        "a spent day cannot be clicked past"
    );
}

/// The notices above the log are not the log: a 功法 turning over is recorded
/// once, in the history, and a sticky line saying the same sentence again read
/// as two events.
#[gpui_kit::test]
fn the_log_is_not_repeated_above_itself(cx: &mut TestAppContext) {
    let mut cultivation = Cultivation::default().view();
    cultivation.last_training = Some(TrainingView {
        art_id: 1,
        name: "a book".to_owned(),
        outcome: ArtOutcome::RankedUp,
        rank: 1,
    });
    cultivation.events = vec![EventRecord {
        at_unix: 1_000,
        detail: EventDetail::Training {
            name: "a book".to_owned(),
            outcome: ArtOutcome::RankedUp,
            rank: 1,
        },
    }];
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    assert!(
        !painted(visual, "cultivation-notice-art"),
        "the training line belongs to the log, not above it"
    );
    assert!(
        painted(visual, "cultivation-event-0"),
        "the log still has to say it"
    );
}

/// A trial floor is replayed like every other fight, and the popup names the
/// floor and what clearing it paid.
#[gpui_kit::test]
fn the_trial_popup_names_the_floor_and_what_it_paid(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut duel = CultivationDuel::new(
        CultivationDuelSource::Trial,
        cultivation_duel::CultivationDuelPayload::Trial(TrialOutcome {
            floor: 9,
            duel: walk_report(),
            found: ExplorationFindView::default(),
        }),
    );
    // A verdict under a half-read fight spoils it, so the replay is shown to its
    // end before the payoff is asked for.
    duel.reveal_all();
    let (_, visual) =
        cx.add_window_view(move |window, cx| Root::new(cx.new(|_| DuelPopup(duel)), window, cx));
    visual.update(|window, cx| window.render_frame(cx));
    for id in [
        "cultivation-duel-health-player",
        "cultivation-duel-side-opponent",
        "cultivation-duel-log",
        "cultivation-duel-trial-floor",
    ] {
        assert!(painted(visual, id), "the trial popup must paint {id}");
    }
}

/// The learned panel answers "what am I working on": three rows and a way to
/// see the rest, rather than a list that grows with the shelf.
#[gpui_kit::test]
fn the_learned_panel_shows_three_and_unfolds(cx: &mut TestAppContext) {
    let cultivation = Cultivation::new(CultivationDocument {
        mastered_arts: vec![11, 12, 13, 14, 15],
        ..CultivationDocument::default()
    })
    .view();
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    for id in [
        "cultivation-mastered-art-11",
        "cultivation-mastered-art-12",
        "cultivation-mastered-art-13",
        "cultivation-mastered-toggle",
    ] {
        assert!(painted(visual, id), "the panel must draw {id}");
    }
    assert!(
        !painted(visual, "cultivation-mastered-art-14"),
        "the fourth 功法 waits behind the fold"
    );
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("cultivation-mastered-toggle", cx);
    });
    visual.update(|window, cx| window.render_frame(cx));
    for id in ["cultivation-mastered-art-14", "cultivation-mastered-art-15"] {
        assert!(painted(visual, id), "unfolding must draw {id}");
    }
}

/// The breakthrough prompt is the one line the page raises its voice for, and
/// it shows only when there is a wall to break.
///
/// A hint printed under a realm still being climbed was a sentence the user
/// read every day and could do nothing about — so the banner is drawn at the
/// very top of the page for the cap alone, and the panel below it, which holds
/// the fight, exists only when a guardian is actually standing there.
#[gpui_kit::test]
fn the_breakthrough_prompt_appears_only_at_a_full_realm(cx: &mut TestAppContext) {
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(at_the_cap()));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    assert!(
        painted(visual, "cultivation-breakthrough-banner"),
        "a full realm has to shout"
    );
    assert!(
        painted(visual, "cultivation-breakthrough"),
        "and the fight itself is on the page"
    );

    // A save still climbing is told nothing: no banner, no panel.
    let (pomodoro, _sink) =
        PomodoroClient::with_snapshot(pomodoro_snapshot(Cultivation::default().view()));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    assert!(
        !painted(visual, "cultivation-breakthrough-banner"),
        "a realm still being climbed gets no prompt"
    );
    assert!(
        !painted(visual, "cultivation-breakthrough"),
        "and no fight it cannot pick"
    );
}

/// The 试炼 sits under the walk, and its button answers to the banked chances
/// rather than to a clock.
#[gpui_kit::test]
fn the_trial_panel_spends_a_banked_chance_and_nothing_without_one(cx: &mut TestAppContext) {
    let mut cultivation = Cultivation::default().view();
    cultivation.proving_tokens = 0;
    let (pomodoro, sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    assert!(
        painted(visual, "cultivation-proving"),
        "the 试炼 must be on the page"
    );
    scroll_until_visible(visual, "cultivation-proving-attempt");
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("cultivation-proving-attempt", cx);
    });
    assert!(
        published(&sink).is_empty(),
        "an empty bank cannot be clicked past"
    );

    let mut cultivation = Cultivation::default().view();
    cultivation.proving_tokens = 3;
    let (pomodoro, sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    scroll_until_visible(visual, "cultivation-proving-attempt");
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("cultivation-proving-attempt", cx);
    });
    let commands = published(&sink);
    assert!(
        matches!(commands.as_slice(), [PomodoroCommand::Prove]),
        "a banked chance asks the worker for the fight: {commands:?}"
    );
}

/// The walk's three lengths are a choice the click carries: picking 4 hours and
/// walking asks the worker for a four-hour walk, not for the default eight.
#[gpui_kit::test]
fn picking_a_walk_length_is_what_the_click_asks_for(cx: &mut TestAppContext) {
    let (pomodoro, sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(at_the_cap()));
    let (view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    scroll_until_visible(visual, "cultivation-explore-hours-4");
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("cultivation-explore-hours-4", cx);
    });
    assert_eq!(
        visual.update(|_, cx| view.read(cx).cultivation_explore_hours),
        4,
        "the chip is what the page remembers"
    );
    scroll_until_visible(visual, "cultivation-explore-attempt");
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("cultivation-explore-attempt", cx);
    });
    let commands = published(&sink);
    assert!(
        matches!(
            commands.as_slice(),
            [PomodoroCommand::Explore {
                hours: bongocat_pomodoro::ExplorationHours::Four
            }]
        ),
        "the click carries the length that was picked: {commands:?}"
    );
}
