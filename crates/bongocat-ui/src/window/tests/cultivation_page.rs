//! The cultivation page's attribute chips and the hint they carry.
//!
//! The hint is the one control on this page a user has to discover by accident,
//! so it gets a frame of its own: the pointer arriving on a chip has to put the
//! explanation on screen, not merely set a flag nobody draws.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use super::*;
use crate::window::cultivation_settings;

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
                PomodoroClient::unavailable(),
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
