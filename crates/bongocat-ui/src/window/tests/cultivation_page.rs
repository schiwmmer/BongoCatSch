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
use crate::window::cultivation_settings;
use bongocat_pomodoro::{
    Cultivation, CultivationDocument, PomodoroCommand, PomodoroSnapshot, PomodoroState,
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
fn at_the_cap() -> bongocat_pomodoro::CultivationView {
    Cultivation::new(CultivationDocument {
        growth: 1_188,
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
