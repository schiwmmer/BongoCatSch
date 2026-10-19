//! The pomodoro surfaces on the cat: the countdown above its head, and the
//! performance a finished session plays.
//!
//! Both are AppKit work rather than Metal work. The renderer draws the model and
//! nothing else: a countdown that asked the renderer to draw a digit would put
//! text on a surface that has no text pipeline, and the performance would have to
//! fight the model's own animation system for the same frame. As panels, both
//! follow the window through moves, resizes and model switches the same way the
//! stats HUD does.
//!
//! The countdown is the one surface here that takes pointer events. That is the
//! whole reason it exists as a panel rather than as a label on the cat: a user
//! has to be able to reach the session they are running to pause it. Everything
//! that follows from that — the right-button monitor, the hover gate — is
//! written out here rather than being assumed.

use super::*;

use crate::{OverlayPomodoroMenuRequest, OverlayPomodoroTimer};
use objc2_app_kit::NSEventType;
use objc2_foundation::NSString;
use std::time::Duration;

/// How far the countdown's bottom edge overlaps the top of the cat window.
///
/// The countdown and the level badge both sit over the cat's head, so they must
/// not collide: the badge's top is [`BADGE_TOP_FRACTION`] below the window top,
/// so this overlap stays smaller than that and keeps the timer clear of the
/// badge. A previous build floated the countdown a fixed gap *above* the cat's
/// top edge, which read as detached from the model; overlapping the top sliver
/// instead hugs the head the way a desktop-pet readout should.
const COUNTDOWN_OVERLAP_FRACTION: f64 = 0.08;

/// Padding inside the countdown panel.
const COUNTDOWN_PADDING: f64 = 12.0;

/// The width reserved for the `mm:ss` digits.
///
/// Reserving the widest text the countdown can ever show rather than sizing to
/// the current one is what keeps the panel still: a proportional digit font
/// makes `24:59` narrower than `10:00`, and a panel that resized every second
/// would move under the pointer that is trying to right-click it.
const COUNTDOWN_DIGIT_WIDTH: f64 = 62.0;

/// The state shared with the right-button monitor.
///
/// The monitor sees every right-button event in the process, so it holds the
/// window number it belongs to and the request it should raise. `Cell` is what
/// lets a plain `Fn` callback own that: AppKit's local monitor API takes no
/// mutable context, and every callback runs on the main thread.
struct CountdownMonitorState {
    window_number: isize,
    request: Cell<Option<OverlayPomodoroMenuRequest>>,
    sender: SyncSender<OverlayPomodoroMenuRequest>,
}

/// The countdown panel and the monitor that reaches it.
pub(crate) struct PomodoroCountdown {
    panel: Retained<NSPanel>,
    icon: Retained<NSTextField>,
    label: Retained<NSTextField>,
    text: String,
    origin: Option<NSPoint>,
    monitor: Option<Retained<AnyObject>>,
    monitor_state: Rc<CountdownMonitorState>,
}

impl PomodoroCountdown {
    /// Create the countdown panel, hidden and unsized.
    pub(crate) fn create(
        mtm: MainThreadMarker,
        sender: Option<SyncSender<OverlayPomodoroMenuRequest>>,
    ) -> Result<Self, OverlayError> {
        // A fully transparent background and no shadow: the countdown floats
        // above the cat as bare text with no panel box behind it, the same
        // no-chrome treatment the level badge uses. The translucent fill the
        // stats HUD wears read as a box pasted over the model; the countdown is
        // a readout, not a control surface, so it carries no chrome of its own.
        let panel = stats_panel(mtm, 0.0);
        panel.setHasShadow(false);
        // This is the one surface that does not pass the pointer through: a user
        // has to be able to reach the session they are running in order to pause
        // it. Every other status panel is a mirror and cannot be clicked.
        panel.setIgnoresMouseEvents(false);

        let content = NSView::new(mtm);
        content.setWantsLayer(true);
        panel.setContentView(Some(&content));

        // The clock is the system font's own glyph rather than a shipped image:
        // it inherits the user's appearance, needs no asset to keep in step with
        // the icon set, and a countdown that reads as text is still a countdown.
        let icon = NSTextField::labelWithString(&NSString::from_str(COUNTDOWN_ICON), mtm);
        icon.setFont(Some(&NSFont::systemFontOfSize(16.0)));
        apply_text_shadow(&icon);
        content.addSubview(&icon);

        let label = NSTextField::labelWithString(&NSString::from_str("25:00"), mtm);
        // A stable digit width is what makes a changing number readable: a
        // proportional font reflows the panel every second as the digits change
        // width. `NSFont` in this build's feature set carries no monospaced
        // constructor, so the digits take the system font and the panel is sized
        // for the widest `mm:ss` it will ever show, which keeps it from moving
        // either.
        label.setFont(Some(&NSFont::systemFontOfSize(18.0)));
        apply_text_shadow(&label);
        content.addSubview(&label);

        // A panel with no sink can still be shown; it simply cannot be right-clicked,
        // because there is nowhere to send the request. The state is built either way so
        // the frame loop never has to know whether the monitor was installed.
        let monitor_state = Rc::new(CountdownMonitorState {
            window_number: panel.windowNumber(),
            request: Cell::new(None),
            sender: sender
                .clone()
                .unwrap_or_else(|| std::sync::mpsc::sync_channel(1).0),
        });
        let monitor = sender.and_then(|_| {
            install_countdown_monitor(MainThreadMarker::new()?, Rc::clone(&monitor_state))
        });

        Ok(Self {
            panel,
            icon,
            label,
            text: String::new(),
            origin: None,
            monitor,
            monitor_state,
        })
    }

    /// Update the remaining time and pin the panel above the cat.
    pub(crate) fn set_remaining(
        &mut self,
        remaining_seconds: u32,
        paused: bool,
        cat: OverlayWindowBounds,
    ) {
        let minutes = remaining_seconds / 60;
        let seconds = remaining_seconds % 60;
        let text = format!("{minutes:02}:{seconds:02}");
        if text != self.text {
            self.text = text.clone();
            self.label.setStringValue(&NSString::from_str(&text));
            self.label.sizeToFit();
            self.icon.sizeToFit();
            let icon_width = self.icon.frame().size.width;
            let label_height = self.label.frame().size.height;
            let icon_height = self.icon.frame().size.height;
            // The panel is sized once, from the widest text it can show, and
            // never resized again: a panel that resized every second would move
            // under the pointer about to right-click it.
            let content_width = icon_width + COUNTDOWN_DIGIT_WIDTH + COUNTDOWN_PADDING * 3.0;
            let content_height = icon_height.max(label_height) + COUNTDOWN_PADDING * 2.0;
            self.panel
                .setContentSize(NSSize::new(content_width, content_height));
            self.icon
                .setFrameOrigin(NSPoint::new(COUNTDOWN_PADDING, COUNTDOWN_PADDING));
            self.label.setFrameOrigin(NSPoint::new(
                COUNTDOWN_PADDING * 2.0 + icon_width,
                COUNTDOWN_PADDING,
            ));
        }
        // A paused countdown keeps its time and reads as stopped: the icon is the
        // only thing that changes, so the digits never lie about how long is left.
        self.icon.setStringValue(&NSString::from_str(if paused {
            COUNTDOWN_ICON_PAUSED
        } else {
            COUNTDOWN_ICON
        }));
        self.monitor_state.request.set(Some(if paused {
            OverlayPomodoroMenuRequest::OfferResumeAndAbandon
        } else {
            OverlayPomodoroMenuRequest::OfferPause
        }));
        let frame = self.panel.frame();
        let x = f64::from(cat.x) + (f64::from(cat.width) - frame.size.width) / 2.0;
        // Hug the head: the countdown's bottom edge overlaps the top sliver of the
        // cat window rather than floating clear of it. The overlap is a fraction of
        // the window height and stays below the forehead badge, so the timer reads
        // as part of the model without covering the realm badge.
        let y = f64::from(cat.y) + f64::from(cat.height) * (1.0 - COUNTDOWN_OVERLAP_FRACTION);
        let origin = NSPoint::new(x, y);
        if self.origin.is_none_or(|previous| previous != origin) {
            self.panel.setFrameOrigin(origin);
            self.origin = Some(origin);
        }
    }

    pub(crate) fn show(&mut self) {
        if !self.panel.isVisible() {
            self.panel.orderFrontRegardless();
        }
    }

    pub(crate) fn hide(&mut self) {
        self.origin = None;
        if self.panel.isVisible() {
            self.panel.orderOut(None);
        }
    }

    /// The request the last right-click raised, taken once.
    ///
    /// The monitor sets the request and the frame loop reads it, so a click that
    /// arrives between frames is not lost, and one that arrives twice in a frame
    /// is raised once.
    pub(crate) fn take_menu_request(&self) -> Option<OverlayPomodoroMenuRequest> {
        self.monitor_state.request.take()
    }

    /// Whether the pointer is over the panel right now, in AppKit global
    /// coordinates.
    ///
    /// The hover hide asks this before it decides to fade the cat: with a
    /// countdown on screen the user can legitimately rest the pointer on it, and
    /// fading the cat out from under a panel they are about to right-click would
    /// make the panel unreachable.
    pub(crate) fn contains_pointer(&self, x: f64, y: f64) -> bool {
        if !self.panel.isVisible() {
            return false;
        }
        let frame = self.panel.frame();
        x >= frame.origin.x
            && x < frame.origin.x + frame.size.width
            && y >= frame.origin.y
            && y < frame.origin.y + frame.size.height
    }
}

impl Drop for PomodoroCountdown {
    fn drop(&mut self) {
        if let Some(monitor) = self.monitor.take() {
            // SAFETY: the token was created by `install_countdown_monitor` for
            // this panel, and it is removed on the AppKit main thread before the
            // state it reads is dropped.
            unsafe { NSEvent::removeMonitor(&monitor) };
        }
        self.panel.setContentView(None);
        // SAFETY: `setReleasedWhenClosed` transfers the panel's retain to
        // AppKit's close path. The panel is not touched afterwards.
        unsafe { self.panel.setReleasedWhenClosed(true) };
        self.panel.close();
    }
}

/// The clock the countdown shows while it is running, and the paused mark it
/// shows instead.
const COUNTDOWN_ICON: &str = "⏱";
const COUNTDOWN_ICON_PAUSED: &str = "⏸";

/// A soft dark shadow behind the floating countdown text.
///
/// With the panel background gone the glyphs sit directly on whatever is behind
/// the cat, and the system label color washes out on a light wallpaper — the
/// text is only legible over everything if it carries a bit of its own edge.
/// The label's layer rasterizes the string it draws, so the shadow tracks the
/// glyph outlines rather than reading as a box behind the text.
fn apply_text_shadow(label: &NSTextField) {
    label.setWantsLayer(true);
    let Some(layer) = label.layer() else {
        return;
    };
    let layer_object: NonNull<AnyObject> = NonNull::from(&*layer).cast();
    // SAFETY: the label owns its layer, and `CALayer` answers the four shadow
    // setters on every macOS this build targets. objc2-quartz-core's generated
    // bindings carry none of them, so the selectors go through the same
    // `AnyObject` pointer route the stats panel's corner radius uses.
    unsafe {
        let shadow_color = NSColor::blackColor().colorWithAlphaComponent(0.55);
        let (): () = msg_send![layer_object, setShadowColor: &*shadow_color];
        let (): () = msg_send![layer_object, setShadowOpacity: 0.55f32];
        let (): () = msg_send![layer_object, setShadowRadius: 2.0f64];
        let (): () = msg_send![layer_object, setShadowOffset: NSSize::new(0.0, -1.0)];
    }
}

/// Install the right-button monitor for the countdown panel.
///
/// Only a right click counts: a right-button *drag* on the panel is not a menu
/// request, and treating one as a request would pop a menu the user is trying to
/// drag away from.
fn install_countdown_monitor(
    _: MainThreadMarker,
    state: Rc<CountdownMonitorState>,
) -> Option<Retained<AnyObject>> {
    // The monitor filters by the panel it belongs to rather than by "any window":
    // the model window's own right-button monitor is installed at the same time,
    // and both see every right-button event in the process.
    let shared = Rc::clone(&state);
    let handler: RcBlock<dyn Fn(NonNull<NSEvent>) -> *mut NSEvent> =
        RcBlock::new(move |event: NonNull<NSEvent>| {
            // SAFETY: AppKit supplies a valid NSEvent pointer for the duration of
            // this local event-monitor callback, and returning it continues normal
            // dispatch.
            let event_ref = unsafe { event.as_ref() };
            if event_ref.windowNumber() == shared.window_number
                && event_ref.r#type() == NSEventType::RightMouseUp
                && let Some(request) = shared.request.take()
            {
                // A full queue means the application is not reading; dropping the
                // request is better than blocking the main thread on a menu.
                let _ = shared.sender.try_send(request);
            }
            event.as_ptr()
        });
    // SAFETY: the handler returns AppKit's original valid event pointer and is
    // retained by the returned monitor token until the panel is dropped.
    let token = unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::RightMouseUp, &handler)
    }?;
    Some(token)
}

/// How long the finished-session performance plays.
const PERFORMANCE_DURATION: Duration = Duration::from_millis(5_000);

/// How far the cat swings at the start of the performance.
///
/// The swing is in points, not pixels: the overlay window is already scaled by
/// the user's zoom, so a point offset moves with the cat instead of staying a
/// fixed size while the cat grows.
const SWING_AMPLITUDE: f64 = 9.0;

/// How many full swings the performance makes.
///
/// An integer number matters: a swing count that ended mid-arc would leave the
/// cat visibly leaning over, because the offset stops where the curve did.
const SWING_COUNT: u32 = 10;

/// How long the red tint takes to fade in and out, and how strong it gets.
const TINT_PEAK: f64 = 0.42;
const TINT_FADE_IN: Duration = Duration::from_millis(250);
const TINT_FADE_OUT: Duration = Duration::from_millis(900);

/// The finished-session performance: the tomato the cat eats, the red tint, and
/// the swing.
///
/// It is programmed rather than animated through the model, because the model's
/// animation system can only play what a model package declares and this has no
/// motion to declare. Everything here is arithmetic on elapsed time, which is
/// also what makes it deterministic: the same elapsed time always produces the
/// same frame.
pub(crate) struct PomodoroPerformance {
    /// The cat's logical origin before the swing, which is what the swing is
    /// applied on top of.
    logical_origin: NSPoint,
    /// The swing offset currently applied to the real frame.
    applied_offset: NSPoint,
    tint: Retained<NSPanel>,
    tint_layer: Option<Retained<AnyObject>>,
    started: Option<Instant>,
}

impl PomodoroPerformance {
    pub(crate) fn create(mtm: MainThreadMarker) -> Result<Self, OverlayError> {
        // The tint covers the cat's own box rather than the screen: a full-screen
        // red flash is a system-level event, and what the performance is about is
        // the cat.
        let tint = stats_panel(mtm, 0.0);
        tint.setHasShadow(false);
        tint.setIgnoresMouseEvents(true);
        let content = NSView::new(mtm);
        content.setWantsLayer(true);
        if let Some(layer) = content.layer() {
            let layer_object: NonNull<AnyObject> = NonNull::from(&*layer).cast();
            // SAFETY: the view owns its layer, and `CALayer` answers
            // `setBackgroundColor:` on every macOS this build targets.
            unsafe {
                let red = NSColor::systemRedColor().colorWithAlphaComponent(0.0);
                let (): () = msg_send![layer_object, setBackgroundColor: &*red];
            }
        }
        tint.setContentView(Some(&content));
        Ok(Self {
            logical_origin: NSPoint::new(0.0, 0.0),
            applied_offset: NSPoint::new(0.0, 0.0),
            tint,
            tint_layer: content.layer().map(Retained::from),
            started: None,
        })
    }

    /// Begin the performance against the cat's current box.
    ///
    /// `bounds` is the cat's *logical* box: the one the placement correction and
    /// the hover hit test see. The swing is applied on top of it rather than
    /// moving it, which is what keeps a swinging cat from being read as "dragged
    /// off screen" and corrected back.
    pub(crate) fn start(&mut self, bounds: OverlayWindowBounds) {
        self.logical_origin = NSPoint::new(f64::from(bounds.x), f64::from(bounds.y));
        self.applied_offset = NSPoint::new(0.0, 0.0);
        self.started = Some(Instant::now());
        let frame = NSRect::new(
            self.logical_origin,
            NSSize::new(f64::from(bounds.width), f64::from(bounds.height)),
        );
        self.tint.setFrame_display(frame, true);
        self.tint.orderFrontRegardless();
    }

    /// The swing offset for an elapsed performance.
    ///
    /// A raised cosine, so the swing starts and ends at rest: the cat is still
    /// moving at the start would look like the performance had begun before it
    /// did, and a swing that stopped mid-arc would leave it leaning over.
    pub(crate) fn swing_offset(elapsed: Duration) -> NSPoint {
        if elapsed >= PERFORMANCE_DURATION {
            return NSPoint::new(0.0, 0.0);
        }
        let total = PERFORMANCE_DURATION.as_secs_f64();
        let t = elapsed.as_secs_f64();
        // The swing plays over the first three quarters of the performance and
        // settles for the rest, so the red fade-out is not shaking.
        let swing_end = total * 0.75;
        if t >= swing_end {
            return NSPoint::new(0.0, 0.0);
        }
        let phase = t / swing_end;
        let envelope = (1.0 - phase) * (1.0 - phase);
        let oscillation = (t / swing_end * f64::from(SWING_COUNT) * 2.0 * std::f64::consts::PI)
            .cos()
            * (1.0 - (t / swing_end * f64::from(SWING_COUNT) * 2.0 * std::f64::consts::PI).cos())
            * 0.5;
        NSPoint::new(SWING_AMPLITUDE * envelope * oscillation, 0.0)
    }

    /// The red tint's alpha for an elapsed performance.
    pub(crate) fn tint_alpha(elapsed: Duration) -> f64 {
        if elapsed >= PERFORMANCE_DURATION {
            return 0.0;
        }
        let peak = if elapsed < TINT_FADE_IN {
            elapsed.as_secs_f64() / TINT_FADE_IN.as_secs_f64()
        } else {
            let since_peak = elapsed - TINT_FADE_IN;
            if since_peak < TINT_FADE_OUT {
                1.0 - since_peak.as_secs_f64() / TINT_FADE_OUT.as_secs_f64()
            } else {
                return 0.0;
            }
        };
        TINT_PEAK * peak.clamp(0.0, 1.0)
    }

    /// Advance the performance by one frame.
    ///
    /// Returns the swing offset that should be applied to the cat's *real* frame.
    pub(crate) fn advance(&mut self) -> NSPoint {
        let Some(started) = self.started else {
            return NSPoint::new(0.0, 0.0);
        };
        let elapsed = started.elapsed();
        let offset = Self::swing_offset(elapsed);
        let alpha = Self::tint_alpha(elapsed);
        self.set_tint_alpha(alpha);
        if alpha > 0.0 {
            let frame = self.tint.frame();
            self.tint
                .setFrame_display(NSRect::new(frame.origin, frame.size), false);
            if !self.tint.isVisible() {
                self.tint.orderFrontRegardless();
            }
        } else if self.tint.isVisible() {
            self.tint.orderOut(None);
        }
        if elapsed >= PERFORMANCE_DURATION {
            self.started = None;
        }
        self.applied_offset = offset;
        offset
    }

    fn set_tint_alpha(&mut self, alpha: f64) {
        let Some(layer) = self.tint_layer.as_ref() else {
            return;
        };
        let layer_object: NonNull<AnyObject> = NonNull::from(&**layer).cast();
        // SAFETY: the layer is retained by the panel's content view for the whole
        // lifetime of this struct, and `CALayer` answers `setBackgroundColor:`
        // on every macOS this build targets.
        unsafe {
            let color = NSColor::systemRedColor().colorWithAlphaComponent(alpha);
            let (): () = msg_send![layer_object, setBackgroundColor: &*color];
        }
    }
}

impl Drop for PomodoroPerformance {
    fn drop(&mut self) {
        self.tint.setContentView(None);
        // SAFETY: `setReleasedWhenClosed` transfers the panel's retain to
        // AppKit's close path. The panel is not touched afterwards.
        unsafe { self.tint.setReleasedWhenClosed(true) };
        self.tint.close();
    }
}

/// The countdown and the performance as one unit.
///
/// They are created together and dropped together because both are presentation
/// for the same session: a countdown with no performance to follow it, or a
/// performance with nothing to have counted down, is a half-degraded surface.
pub(crate) struct PomodoroSurfaces {
    countdown: Option<PomodoroCountdown>,
    performance: Option<PomodoroPerformance>,
}

impl PomodoroSurfaces {
    /// Create both surfaces, each degrading on its own.
    ///
    /// The countdown carries the monitor, so it is created first: a countdown
    /// that could not be created is also a right-click that cannot be received.
    pub(crate) fn create(
        mtm: MainThreadMarker,
        menu_sender: Option<SyncSender<OverlayPomodoroMenuRequest>>,
    ) -> Result<Self, OverlayError> {
        let countdown = PomodoroCountdown::create(mtm, menu_sender).ok();
        let performance = PomodoroPerformance::create(mtm).ok();
        Ok(Self {
            countdown,
            performance,
        })
    }

    /// Begin the finished-session performance against the cat's logical box.
    pub(crate) fn start_performance(&mut self, bounds: OverlayWindowBounds) {
        if let Some(performance) = self.performance.as_mut() {
            performance.start(bounds);
        }
    }

    /// Advance one frame and return the swing the cat's real frame should carry.
    pub(crate) fn advance(&mut self) -> NSPoint {
        self.performance
            .as_mut()
            .map_or(NSPoint::new(0.0, 0.0), PomodoroPerformance::advance)
    }

    /// Show or hide the countdown.
    pub(crate) fn show_countdown(
        &mut self,
        timer: Option<OverlayPomodoroTimer>,
        bounds: OverlayWindowBounds,
    ) {
        let Some(countdown) = self.countdown.as_mut() else {
            return;
        };
        let Some(timer) = timer else {
            countdown.hide();
            return;
        };
        countdown.set_remaining(timer.remaining_seconds, timer.paused, bounds);
        countdown.show();
    }

    pub(crate) fn hide_countdown(&mut self) {
        if let Some(countdown) = self.countdown.as_mut() {
            countdown.hide();
        }
    }

    /// The right-click the countdown raised, taken once.
    pub(crate) fn take_menu_request(&self) -> Option<OverlayPomodoroMenuRequest> {
        self.countdown
            .as_ref()
            .and_then(PomodoroCountdown::take_menu_request)
    }

    /// Whether the pointer is resting on the countdown.
    pub(crate) fn countdown_is_under_pointer(&self, x: f64, y: f64) -> bool {
        self.countdown
            .as_ref()
            .is_some_and(|countdown| countdown.contains_pointer(x, y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_swing_starts_and_ends_at_rest() {
        assert_eq!(
            PomodoroPerformance::swing_offset(Duration::ZERO),
            NSPoint::new(0.0, 0.0),
            "a swing that starts mid-arc looks like it began before it did"
        );
        assert_eq!(
            PomodoroPerformance::swing_offset(PERFORMANCE_DURATION),
            NSPoint::new(0.0, 0.0)
        );
        assert_eq!(
            PomodoroPerformance::swing_offset(PERFORMANCE_DURATION + Duration::from_secs(1)),
            NSPoint::new(0.0, 0.0),
            "an elapsed performance past its end must not keep swinging"
        );
    }

    #[test]
    fn the_swing_stays_inside_its_amplitude() {
        let step = Duration::from_millis(16);
        let mut elapsed = Duration::ZERO;
        let mut largest: f64 = 0.0;
        while elapsed < PERFORMANCE_DURATION {
            let offset = PomodoroPerformance::swing_offset(elapsed);
            assert!(
                offset.x.abs() <= SWING_AMPLITUDE,
                "the swing exceeded its amplitude at {elapsed:?}: {}",
                offset.x
            );
            largest = largest.max(offset.x.abs());
            elapsed += step;
        }
        assert!(
            largest > SWING_AMPLITUDE * 0.5,
            "the swing never used its amplitude: {largest}"
        );
    }

    #[test]
    fn the_swing_settles_before_the_performance_ends() {
        // The red fade-out is the last thing the user sees, and a shaking cat
        // under it reads as a bug rather than as a performance ending.
        let settle_at = PERFORMANCE_DURATION.mul_f64(0.75);
        assert_eq!(
            PomodoroPerformance::swing_offset(settle_at),
            NSPoint::new(0.0, 0.0)
        );
        assert_eq!(
            PomodoroPerformance::swing_offset(PERFORMANCE_DURATION - Duration::from_millis(1)),
            NSPoint::new(0.0, 0.0)
        );
    }

    #[test]
    fn the_tint_fades_in_holds_and_fades_away() {
        assert_eq!(PomodoroPerformance::tint_alpha(Duration::ZERO), 0.0);
        assert_eq!(
            PomodoroPerformance::tint_alpha(TINT_FADE_IN),
            TINT_PEAK,
            "the tint should be at its peak once the fade-in is over"
        );
        let held = PomodoroPerformance::tint_alpha(Duration::from_millis(600));
        assert!(held > TINT_PEAK * 0.6, "the tint dropped early: {held}");
        assert_eq!(
            PomodoroPerformance::tint_alpha(TINT_FADE_IN + TINT_FADE_OUT),
            0.0
        );
        assert_eq!(
            PomodoroPerformance::tint_alpha(PERFORMANCE_DURATION + Duration::from_secs(1)),
            0.0,
            "an elapsed performance must leave the tint off"
        );
    }

    #[test]
    fn the_tint_never_exceeds_its_peak() {
        let mut elapsed = Duration::ZERO;
        while elapsed < PERFORMANCE_DURATION {
            let alpha = PomodoroPerformance::tint_alpha(elapsed);
            assert!(
                (0.0..=TINT_PEAK).contains(&alpha),
                "the tint left its range at {elapsed:?}: {alpha}"
            );
            elapsed += Duration::from_millis(16);
        }
    }
}
