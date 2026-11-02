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
    ArtGrade, ArtOutcome, ArtOwnership, Attribute, Attributes, Cost, CropRecord, Cultivation,
    CultivationDocument, DailyKind, DailyOutcome, Derived, DerivedSlot, DuelReport, DuelRound,
    EventDetail, EventRecord, ExplorationFindView, ExplorationOutcome, Fighter, ItemView, JobKind,
    JobRun, LAYERS_PER_STAGE, OwnedArt, PLAYER_NAME, PlotRecord, PomodoroCommand, PomodoroSnapshot,
    PomodoroState, ProvingBlocked, Rarity, Realm, ResourceBag, SeedGrade, SeedStack, SessionMode,
    ShopOffer, TrainingView, TrialBlocked, TrialOutcome, WANDERER_NAME, threshold_for,
};
use gpui_kit::{ScrollDelta, point, px};

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
///
/// The scroll is asked for in both directions: the hints a page test walks
/// through are not in page order (the derived numbers sit at the top and the
/// shop at the bottom), so a frame that only ever scrolled down could not come
/// back to a rule it had already passed.
fn scroll_until_visible(visual: &mut VisualTestContext, id: &'static str) {
    for _ in 0..48 {
        // Which way to walk, on this step: the target's own bounds against the
        // viewport's, read through the same element snapshot the visibility
        // check reads. `visible` is the fast path — a row on screen needs no
        // travel — and the sign of `travel` is the direction a hidden row sits
        // in. `-1` is above (scrolling up), `1` is below (scrolling down).
        let (visible, travel) = visual.update(|window, _| {
            let Some(snapshot) = window.try_find(ElementId::from(id)) else {
                return (false, 0);
            };
            if snapshot.visible() {
                return (true, 0);
            }
            let bounds = snapshot.bounds();
            let viewport = window
                .try_find(ElementId::from("cultivation-page-harness"))
                .map(|frame| frame.bounds())
                .unwrap_or(bounds);
            let travel = if bounds.origin.y + bounds.size.height < viewport.origin.y {
                -1
            } else {
                1
            };
            (false, travel)
        });
        if visible {
            // One more line: a row that has only just come into view sits
            // against the fold, and a click aimed at its centre can land
            // outside the window. The extra line settles it inside without
            // scrolling a short page past its end — the scroll clamps.
            visual.update(|window, cx| {
                window.scroll(
                    ElementId::from("cultivation-page-harness"),
                    ScrollDelta::Lines(point(0.0, -1.0)),
                    cx,
                );
                window.render_frame(cx);
            });
            return;
        }
        // A row above the viewport is scrolled up to (a positive delta), a row
        // below it down (a negative one); either way the scroll clamps at the
        // page's ends rather than walking past them.
        let delta = if travel < 0 { 6.0 } else { -6.0 };
        visual.update(|window, cx| {
            window.scroll(
                ElementId::from("cultivation-page-harness"),
                ScrollDelta::Lines(point(0.0, delta)),
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
    // pays no attribute at all).
    assert!(
        painted(visual, "cultivation-power-breakdown"),
        "the level · placed · gear · crop · 内功 line must be on the page"
    );
    // The 外功 line under the attributes is gone on purpose: the swings are
    // priced on the learned-功法 panel, and a second copy of the number here
    // only asked the user which of the two to believe.
    assert!(
        !painted(visual, "cultivation-power-external-line"),
        "外功 belongs to the learned-功法 panel, not to the attribute totals"
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

/// The three session-mode buttons arm what the next finished pomodoro pays:
/// clicking 伐木 publishes the mode, and the armed one is the one that is
/// disabled — a mode already chosen is not offered again.
#[gpui_kit::test]
fn the_mode_buttons_arm_the_next_session(cx: &mut TestAppContext) {
    let (pomodoro, sink) = PomodoroClient::with_sink();
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    for mode in ["Study", "Woodcut", "Mine"] {
        assert!(
            painted(visual, format!("cultivation-mode-{mode}")),
            "the mode row offers {mode}"
        );
    }
    scroll_until_visible(visual, "cultivation-mode-Woodcut");
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("cultivation-mode-Woodcut", cx);
    });
    let commands = published(&sink);
    assert!(
        matches!(
            commands.as_slice(),
            [PomodoroCommand::SelectSessionMode {
                mode: SessionMode::Woodcut
            }]
        ),
        "clicking 伐木 arms the forest: {commands:?}"
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
    scroll_until_visible(visual, "cultivation-breakthrough-attempt");
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

/// The board of jobs is on the page, three rows deep, and each row can be taken.
#[gpui_kit::test]
fn the_board_of_jobs_is_on_the_page(cx: &mut TestAppContext) {
    let (_view, visual) = cultivation_page(cx);
    visual.update(|window, cx| window.render_frame(cx));
    scroll_until_visible(visual, "cultivation-jobs");
    assert!(painted(visual, "cultivation-jobs"), "the board is drawn");
    for slot in 0..3 {
        assert!(
            painted(visual, format!("cultivation-job-{slot}")),
            "slot {slot} is missing from the board"
        );
    }
    scroll_until_visible(visual, "cultivation-job-take-0");
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("cultivation-job-take-0", cx);
    });
}

/// The job in hand keeps its place on the board but reads as taken: the take
/// button is gone, a 接取中 tag is in its place, and the working line above the
/// board still says what the work will pay. The board cannot be read as
/// offering a second copy of work already under way.
#[gpui_kit::test]
fn a_taken_job_reads_as_taken_not_open(cx: &mut TestAppContext) {
    let mut cultivation = Cultivation::default().view();
    if let Some(job) = cultivation.jobs.first_mut() {
        job.working = true;
    }
    cultivation.work = Some(JobRun {
        started_unix: 0,
        ends_unix: 3_600,
        tier: 1,
        hours: 1,
        grade: ArtGrade::Yellow,
        kind: JobKind::default(),
        early: false,
    });
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    scroll_until_visible(visual, "cultivation-jobs");
    assert!(
        painted(visual, "cultivation-job-taken-0"),
        "the job in hand reads as taken on the board"
    );
    assert!(
        !painted(visual, "cultivation-job-take-0"),
        "a taken job has no take button"
    );
}

/// The door is the page's first answer to what is going on: 在家 the cultivator
/// can act — a fight, a walk, a job — and 外出 everything but the road home
/// waits. Both states draw the same row, so it is read twice: once at home,
/// once out on a walk.
#[gpui_kit::test]
fn the_door_reads_where_the_cultivator_is(cx: &mut TestAppContext) {
    let home = Cultivation::default().view();
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(home));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    scroll_until_visible(visual, "cultivation-at-home");
    assert!(
        painted(visual, "cultivation-at-home"),
        "the door is drawn while the cultivator is at home"
    );
    assert!(
        !painted(visual, "cultivation-return-early"),
        "at home there is no outing to call back"
    );
    let mut away = Cultivation::default().view();
    away.at_home = false;
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(away));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    scroll_until_visible(visual, "cultivation-at-home");
    assert!(
        painted(visual, "cultivation-at-home"),
        "the door is drawn while the cultivator is out"
    );
    // Every outing is called back the same way, so the control is drawn off the
    // door rather than off which outing it is.
    assert!(
        painted(visual, "cultivation-return-early"),
        "an outing can always be called home"
    );
    // The longest status line the door can carry is a job's, and the control has
    // to stay beside it and inside the page: a button pushed off the right edge
    // is a button nobody can press.
    let mut working = Cultivation::default().view();
    working.at_home = false;
    working.work = Some(JobRun {
        started_unix: 0,
        ends_unix: 8 * 3_600,
        tier: 3,
        hours: 8,
        grade: ArtGrade::Profound,
        kind: JobKind::default(),
        early: false,
    });
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(working));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    scroll_until_visible(visual, "cultivation-at-home");
    let page = visual.update(|window, _| window.find("cultivation-page-harness").bounds());
    let door = visual.update(|window, _| window.find("cultivation-at-home").bounds());
    let button = visual.update(|window, _| window.find("cultivation-return-early").bounds());
    assert!(
        (button.center().y - door.center().y).abs() <= px(1.0),
        "the control sits on the door's own line: {button:?} against {door:?}"
    );
    assert!(
        button.origin.x + button.size.width <= page.origin.x + page.size.width,
        "the control stays inside the page: {button:?} against {page:?}"
    );
}

/// Every rule on the page is read beside its title, not filed behind a hover.
///
/// The hints used to sit in hover cards, which made them a thing a user had to
/// discover by accident; a walk taken without reading the road's rules was a
/// walk taken blind. They are now part of the header, so a frame that paints the
/// panel paints its rule too — and the rule is one line: a hint that wrapped to a
/// second would cost the panel the row this move saved.
#[gpui_kit::test]
fn every_rule_is_read_beside_its_title(cx: &mut TestAppContext) {
    let (_view, visual) = cultivation_page(cx);
    visual.update(|window, cx| window.render_frame(cx));
    for hint in [
        "cultivation-explore-hint",
        "cultivation-trial-hint",
        "cultivation-proving-hint",
        "cultivation-jobs-hint",
        "cultivation-derived_hint",
        "cultivation-shop-shelf",
    ] {
        scroll_until_visible(visual, hint);
        visual.update(|window, cx| window.render_frame(cx));
        assert!(
            painted(visual, hint),
            "{hint} has to be on screen with its title, not behind a hover"
        );
        // One line of text is one line tall. The harness window is far narrower
        // than a desktop's, so a hint that fits two lines here would fit three
        // out there — the ellipsis is what keeps it to one.
        let height = visual.update(|window, _| window.find(hint).bounds().size.height);
        assert!(
            height <= px(24.0),
            "{hint} wrapped to {height:?}; a rule is one line or it is a paragraph"
        );
    }
    // And none of them went back to a card that only opens on a pointer.
    for card in [
        "cultivation-hint-explore-body",
        "cultivation-hint-trial-body",
        "cultivation-hint-proving-body",
    ] {
        assert!(!painted(visual, card), "{card} should no longer exist");
    }
}

/// A finished job says what it actually came home with, windfall included.
///
/// The wage in flight is the figure the board printed — 潜质 has not had its
/// turn — but once the job is paid the line has to name the real purse, or the
/// crit the domain rolled would be invisible.
#[gpui_kit::test]
fn a_finished_job_says_what_it_paid(cx: &mut TestAppContext) {
    let mut cultivation = at_the_cap();
    cultivation.last_work = Some(bongocat_pomodoro::WorkPaid {
        stones: 108,
        hours: 12,
        grade: ArtGrade::Earth,
        kind: JobKind::default(),
        success_rate: 900,
        gain: Some((DerivedSlot::Attack, 3)),
    });
    let (pomodoro, sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    scroll_until_visible(visual, "cultivation-jobs-paid");
    assert!(
        painted(visual, "cultivation-jobs-paid"),
        "the last job's purse, and its gain, have to be readable"
    );
    // A job in hand draws the countdown instead, so the two lines never compete.
    assert!(
        !painted(visual, "cultivation-jobs-working"),
        "a paid job and a job in hand are not both on the page"
    );
    assert!(published(&sink).is_empty(), "and no command was published");
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
        matches!(commands.as_slice(), [PomodoroCommand::ReturnEarly]),
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
        star_cost: Some(Cost {
            stones: 350,
            resources: ResourceBag {
                wood: 3,
                stone: 2,
                iron: 0,
                coal: 0,
            },
        }),
        name: Some("玄霜剑【寒霜】".to_owned()),
        set: None,
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
                    gear: None,
                    art: None,
                    seed: None,
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
    assert_eq!(
        cultivation.power_breakdown.parts_total(),
        cultivation.power,
        "the breakdown has to reach the number in the corner: {:?}",
        cultivation.power_breakdown
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
            copies: 1,
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

/// A fifth plot sits alone on its own row, and must answer to its own index:
/// the plots are four to a row, so the second row starts at index four, not at
/// the index the first row ended on. A bed that reuses an earlier index draws
/// its harvest button under the wrong id and the click harvests the wrong plot
/// (or nothing, if the reused bed is not yet ripe) — the exact report of a
/// ripe 凤花 on its own row that would not answer.
#[gpui_kit::test]
fn a_plot_on_its_own_row_keeps_its_own_index(cx: &mut TestAppContext) {
    let mut plots: Vec<_> = (0..4)
        .map(|i| PlotRecord {
            crop: Some(CropRecord::planted(
                Attribute::Mana,
                2,
                SeedGrade::Earth,
                Some(format!("草{i}")),
                1,
                0,
            )),
        })
        .collect();
    plots.push(PlotRecord {
        crop: Some(CropRecord::planted(
            Attribute::Mana,
            2,
            SeedGrade::Earth,
            Some("凤花".to_owned()),
            1,
            0,
        )),
    });
    let cultivation = Cultivation::new(CultivationDocument {
        plots,
        ..CultivationDocument::default()
    })
    .view();
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    // The bug drew the fifth bed as index 3: its cell and harvest button then
    // carried ids `...-3` and the ripe crop answered to a bed that was not its
    // own. Both ids must belong to the fifth bed itself.
    assert!(
        painted(visual, "cultivation-plot-4"),
        "the fifth plot must draw its own cell, not reuse the fourth index"
    );
    assert!(
        painted(visual, "cultivation-harvest-4"),
        "the ripe fifth plot must offer its own harvest button"
    );
    assert!(
        !painted(visual, "cultivation-harvest-5"),
        "there is no sixth bed to harvest"
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
            herb: None,
        }],
        plots: vec![PlotRecord {
            crop: Some(CropRecord::planted(
                Attribute::Mana,
                2,
                SeedGrade::Heaven,
                Some("玄莲".to_owned()),
                1,
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

/// A bed past its halfway mark may be holding a dead crop, and says so: the cell
/// names the failure, drops the payoff line it will never pay, and offers the one
/// action a dead bed has left.
#[gpui_kit::test]
fn a_withered_plot_says_so_and_offers_the_dig(cx: &mut TestAppContext) {
    let dead = CropRecord {
        withered: true,
        ..CropRecord::planted(Attribute::Mana, 2, SeedGrade::Heaven, None, 1, 0)
    };
    // Read well past the halfway mark: `view()` asks the domain at instant zero,
    // where no bed has been allowed to speak yet.
    let cultivation = Cultivation::new(CultivationDocument {
        plots: vec![PlotRecord { crop: Some(dead) }],
        ..CultivationDocument::default()
    })
    .view_on(1_000_000, 0);
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    for id in ["cultivation-plot-withered-0", "cultivation-dig-0"] {
        assert!(painted(visual, id), "a dead bed must draw {id}");
    }
    assert!(
        !painted(visual, "cultivation-harvest-0"),
        "a dead crop is never a harvest button"
    );
    // The name is still there — the gardener needs to know which seed died — but
    // the row it opens would promise a payoff the harvest will not make.
    visual.update(|window, cx| window.click("cultivation-plot-name-0", cx));
    visual.update(|window, cx| window.render_frame(cx));
    assert!(
        !painted(visual, "cultivation-plot-detail-0"),
        "a dead bed has no harvest facts to show"
    );
}

/// The satchel counts what it holds, lists four stacks lowest tier first two to
/// a row, and keeps the rest one click away.
///
/// A satchel of twenty stacks is a wall on a page that is already a ladder, and
/// the cheap end is the end a young garden can actually bring up — so that is
/// the end it shows. The count beside the title is every seed in every stack,
/// which is the number the rows cannot be counted to.
///
/// The tiers run one to six while the grades are deliberately out of step with
/// them (a first-tier 神阶 seed sits next to a second-tier 黄阶 one), so a panel
/// that sorted by grade instead of by tier would push the 神阶 first-tier seed to
/// the last row — the assertions below would catch it.
#[gpui_kit::test]
fn the_seed_panel_counts_its_stacks_and_unfolds(cx: &mut TestAppContext) {
    let seed = |tier: u8, grade: SeedGrade| SeedStack {
        attribute: Attribute::Root,
        herb: None,
        tier,
        grade,
        count: 2,
        name: None,
    };
    let cultivation = Cultivation::new(CultivationDocument {
        seeds: vec![
            seed(1, SeedGrade::Divine),
            seed(2, SeedGrade::Yellow),
            seed(3, SeedGrade::Profound),
            seed(4, SeedGrade::Earth),
            seed(5, SeedGrade::Heaven),
            seed(6, SeedGrade::Yellow),
        ],
        ..CultivationDocument::default()
    })
    .view();
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    for id in [
        "cultivation-seeds-total",
        "cultivation-seed-row-0",
        "cultivation-seed-row-1",
        "cultivation-seed-name-0-1-4",
        "cultivation-seed-name-0-2-0",
        "cultivation-seed-name-0-3-1",
        "cultivation-seed-name-0-4-2",
        "cultivation-seeds-toggle",
    ] {
        assert!(painted(visual, id), "the satchel must draw {id}");
    }
    // Four, in two rows of two: the fifth and sixth kinds wait behind the fold.
    assert!(
        !painted(visual, "cultivation-seed-name-0-5-3"),
        "the fifth stack waits behind the fold"
    );
    assert!(
        !painted(visual, "cultivation-seed-name-0-6-0"),
        "the dearest stack waits behind the fold"
    );
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("cultivation-seeds-toggle", cx);
    });
    visual.update(|window, cx| window.render_frame(cx));
    assert!(
        painted(visual, "cultivation-seed-name-0-6-0"),
        "unfolding must draw the stack it was hiding"
    );
}

/// The 种植 and 卖 keys of a seed column line up down the page whatever the
/// price is.
///
/// A seed row is two cells and a cell's two keys sit at the end of the space
/// the seed's name leaves over — so a 卖 key carrying a three-digit price and a
/// neighbour carrying a one-digit one would push their 种植 keys to different
/// columns. The two keys are one fixed-width row, so the 种植 keys all start on
/// the same line and the 卖 keys all end on the same line.
#[gpui_kit::test]
fn the_seed_keys_line_up_across_rows(cx: &mut TestAppContext) {
    let seed = |tier: u8, grade: SeedGrade| SeedStack {
        attribute: Attribute::Root,
        herb: None,
        tier,
        grade,
        count: 1,
        name: None,
    };
    let cultivation = Cultivation::new(CultivationDocument {
        seeds: vec![
            seed(1, SeedGrade::Yellow),  // sells for 0
            seed(2, SeedGrade::Yellow),  // fills the row
            seed(9, SeedGrade::Divine),  // sells for 72
            seed(10, SeedGrade::Divine), // sells for 120
        ],
        ..CultivationDocument::default()
    })
    .view();
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    // Lowest tier first, two to a row: the first and third stacks are the same
    // column, and their keys are the pair a price difference would scatter.
    let plant_first = rendered_bounds(visual, ElementId::from("cultivation-plant-0-1-0"));
    let plant_third = rendered_bounds(visual, ElementId::from("cultivation-plant-0-9-4"));
    let one_pixel = px(1.0);
    assert!(
        plant_first.left() >= plant_third.left() - one_pixel
            && plant_first.left() <= plant_third.left() + one_pixel,
        "a column's 种植 keys must start on the same line whatever the price"
    );
    let sell_first = rendered_bounds(visual, ElementId::from("cultivation-sell-seed-0-1-0"));
    let sell_third = rendered_bounds(visual, ElementId::from("cultivation-sell-seed-0-9-4"));
    assert!(
        sell_first.right() >= sell_third.right() - one_pixel
            && sell_first.right() <= sell_third.right() + one_pixel,
        "a column's 卖 keys must end on the same line whatever the price"
    );
    assert_ne!(
        plant_first.top(),
        plant_third.top(),
        "the two keys compared come from different rows"
    );
}

/// The success rate line also carries how long the crop would take to come up.
///
/// The growing time is rendered beside the rate so a gardener comparing two
/// seeds sees both the odds and the wait at a glance. This checks the growing
/// element itself is drawn to the right of the rate line — a rate that lost its
/// growing time would still be a line, so only the growing element proves both
/// facts are there.
#[gpui_kit::test]
fn a_seed_row_shows_the_growing_time_beside_the_odds(cx: &mut TestAppContext) {
    let seed = |tier: u8, grade: SeedGrade| SeedStack {
        attribute: Attribute::Root,
        herb: None,
        tier,
        grade,
        count: 1,
        name: None,
    };
    let cultivation = Cultivation::new(CultivationDocument {
        seeds: vec![seed(1, SeedGrade::Yellow)],
        ..CultivationDocument::default()
    })
    .view();
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    let odds = rendered_bounds(visual, ElementId::from("cultivation-seed-odds-0-1-0"));
    let growing = rendered_bounds(
        visual,
        ElementId::from("cultivation-seed-odds-0-1-0-growing"),
    );
    assert!(
        odds.size.width > px(0.) && odds.size.height > px(0.),
        "the odds line must draw"
    );
    assert!(
        growing.size.width > px(0.) && growing.size.height > px(0.),
        "the growing time must draw beside the rate"
    );
    assert!(
        growing.left() > odds.left(),
        "the growing time must sit to the right of the rate"
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
            copies: 1,
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
    // A fresh save is no stronger than the first warden, so the climb is the
    // ordinary fight rather than a sweep — the sweep gets a frame of its own.
    let cultivation = Cultivation::default().view();
    assert!(
        !cultivation.trial_sweepable,
        "a fresh save must not be offered the sweep"
    );
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

/// The satchel lists the 潜质 items it actually holds: a permanent "洗髓丹 ×0"
/// row would be an empty shelf talking about an item the user does not have.
#[gpui_kit::test]
fn the_satchel_hides_the_pill_at_zero(cx: &mut TestAppContext) {
    let mut cultivation = Cultivation::default().view();
    cultivation.aptitude_herbs = [0, 0, 0];
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    // `AptitudeHerb::ALL` order: 悟心草, 慧心草, 洗髓丹 — the pill is index 2.
    assert!(
        !painted(visual, "cultivation-held-herb-2"),
        "a pill nobody holds is not a row in the satchel"
    );
}

/// A pill that is held is a row with its price on it: the button spends that
/// gate's own number of pills, and the number is what the label says.
#[gpui_kit::test]
fn the_satchel_shows_a_held_pill_and_its_gate_price(cx: &mut TestAppContext) {
    let mut cultivation = Cultivation::default().view();
    cultivation.aptitude_herbs = [0, 0, 2];
    cultivation.marrow_gate_cost = Some(1);
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    assert!(
        painted(visual, "cultivation-held-herb-2"),
        "the held pill is listed"
    );
    assert!(
        painted(visual, "cultivation-use-marrow"),
        "with the button that opens the gate"
    );
}

/// A closed tower still answers the click: the climb is asked for, the domain
/// refuses, and the page shows the refusal as a toast — a spent day or an
/// unfinished road are rules the click finds out about, not lines that guess.
#[gpui_kit::test]
fn a_closed_tower_asks_and_the_domain_refuses(cx: &mut TestAppContext) {
    let mut cultivation = Cultivation::default().view();
    cultivation.trial_attempts_left = 0;
    cultivation.trial_blocked = Some(TrialBlocked::Drained);
    let (pomodoro, sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    assert!(
        painted(visual, "cultivation-trial-status"),
        "the allowance has to be on the page"
    );
    scroll_until_visible(visual, "cultivation-trial-attempt");
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("cultivation-trial-attempt", cx);
    });
    assert!(
        matches!(published(&sink).as_slice(), [PomodoroCommand::Tower]),
        "the click still asks the worker, and the domain refuses with a toast"
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
            warden_power: 12_345,
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

/// The 试炼 sits under the walk, and its button always asks the worker: a banked
/// chance buys the fight, an empty bank is refused by the domain and answered
/// with a toast.
#[gpui_kit::test]
fn the_trial_panel_asks_for_the_fight_or_a_refusal(cx: &mut TestAppContext) {
    let mut cultivation = Cultivation::default().view();
    cultivation.proving_tokens = 0;
    cultivation.proving_blocked = Some(ProvingBlocked::Spent);
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
        matches!(published(&sink).as_slice(), [PomodoroCommand::Prove]),
        "the click still asks the worker, and the domain refuses with a toast"
    );

    let mut cultivation = Cultivation::default().view();
    cultivation.proving_tokens = 3;
    cultivation.proving_blocked = None;
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

/// A job in hand closes the courtyard the way a walk does: the click is still
/// asked, the domain refuses, and the page answers with a toast rather than
/// spending a chance on a fight the cultivator cannot be in.
#[gpui_kit::test]
fn the_trial_panel_asks_and_the_domain_refuses_at_a_job(cx: &mut TestAppContext) {
    let mut cultivation = Cultivation::default().view();
    cultivation.proving_tokens = 3;
    cultivation.proving_blocked = Some(ProvingBlocked::Working);
    let (pomodoro, sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    scroll_until_visible(visual, "cultivation-proving-attempt");
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("cultivation-proving-attempt", cx);
    });
    assert!(
        matches!(published(&sink).as_slice(), [PomodoroCommand::Prove]),
        "the click still asks the worker, and the domain refuses with a toast"
    );
}

/// One overwhelmingly strong cultivator is offered the sweep: the button says
/// 横扫, and its click asks the worker for the whole stretch rather than one
/// fight — the climb the tower would otherwise have dragged ten fights out of.
#[gpui_kit::test]
fn the_trial_panel_offers_the_sweep_when_the_warden_is_no_contest(cx: &mut TestAppContext) {
    let cultivation = Cultivation::new(CultivationDocument {
        growth: threshold_for(500),
        realm: Realm::from_level(500).index() as u8,
        ..CultivationDocument::default()
    })
    .view();
    assert!(
        cultivation.trial_sweepable,
        "the fixture has to be a body the warden is no contest for"
    );
    assert!(
        cultivation.trial_blocked.is_none(),
        "and the tower has to be open to offer the sweep"
    );
    let (pomodoro, sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    scroll_until_visible(visual, "cultivation-trial-attempt");
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("cultivation-trial-attempt", cx);
    });
    let commands = published(&sink);
    assert!(
        matches!(commands.as_slice(), [PomodoroCommand::TowerSweep]),
        "a no-contest stretch is swept, not climbed: {commands:?}"
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

/// A 功法 already on the shelf is marked 已有, and the price stays: what the
/// stones buy is a spare, so the row is a sale rather than a refusal.
#[gpui_kit::test]
fn the_shelf_marks_a_book_the_cultivator_already_has(cx: &mut TestAppContext) {
    let mut cultivation = Cultivation::new(CultivationDocument {
        stones: 5_000,
        ..CultivationDocument::default()
    })
    .view();
    let slot = cultivation
        .shop_offers
        .first()
        .map(|entry| entry.slot)
        .expect("a fresh save still gets a shelf");
    // Dress the first row as a 功法 the cultivator is already reading: the
    // shelf itself rolls its own stock, and what this test is about is what the
    // row says once a book is in hand.
    cultivation.shop_offers[0].offer = ShopOffer::Art { art_id: 1 };
    cultivation.shop_offers[0].art_ownership = Some(ArtOwnership::Shelved);
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    assert!(
        painted(visual, format!("cultivation-shop-owned-{slot}")),
        "the row says the book is already held"
    );
    assert!(
        painted(visual, format!("cultivation-shop-buy-{slot}")),
        "and keeps its price: the purchase is a spare"
    );
}

/// A 功法 learned outright has no page left: the shelf says so in place of the
/// price, rather than offering a click that could only be refused.
#[gpui_kit::test]
fn a_learned_art_has_no_price_on_the_shelf(cx: &mut TestAppContext) {
    let mut cultivation = Cultivation::new(CultivationDocument {
        stones: 5_000,
        ..CultivationDocument::default()
    })
    .view();
    let slot = cultivation
        .shop_offers
        .first()
        .map(|entry| entry.slot)
        .expect("a fresh save still gets a shelf");
    cultivation.shop_offers[0].offer = ShopOffer::Art { art_id: 1 };
    cultivation.shop_offers[0].art_ownership = Some(ArtOwnership::Mastered);
    let (pomodoro, _sink) = PomodoroClient::with_snapshot(pomodoro_snapshot(cultivation));
    let (_view, visual) = cultivation_page_with(cx, pomodoro);
    visual.update(|window, cx| window.render_frame(cx));
    assert!(
        painted(visual, format!("cultivation-shop-mastered-{slot}")),
        "the row says the 功法 is learned"
    );
    assert!(
        !painted(visual, format!("cultivation-shop-buy-{slot}")),
        "and offers no price for a book with nothing left in it"
    );
    assert!(
        !painted(visual, format!("cultivation-shop-owned-{slot}")),
        "the 已有 mark belongs to a book still on the shelf"
    );
}
