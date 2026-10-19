//! The hover stats HUD: a small read-only panel under the cat.
//!
//! When the pointer rests on the overlay window, a non-activating panel with
//! the cultivation ladder and the pomodoro counts appears just below it, and
//! disappears when the pointer leaves. The HUD is an AppKit panel with an
//! `NSTextField`, not Metal work: the model's renderer is never asked to draw
//! text, and a frame that shows the HUD costs nothing but a possible
//! `setFrameOrigin`.
//!
//! The panel ignores mouse events, so resting the pointer on the HUD itself
//! is the same as resting it on the cat — the hover-hide hit test still sees
//! the pointer inside the overlay box, which is what keeps the HUD stable
//! while the user reads it. When hover hide is configured, the cat fades out
//! while the HUD stays up: the cat steps aside and the counters are what the
//! pointer found.
//!
//! The panel draws a string it was handed, not a catalog entry: the
//! application composes the realm, the layer, the growth line and the pomodoro
//! counts, and the session passes them on. The overlay owns no language and no
//! ladder, so a HUD and a settings page cannot disagree about either.

use super::*;

/// Horizontal and vertical padding around the label inside the panel.
const HUD_PADDING: f64 = 14.0;
/// The gap between the overlay window's bottom edge (or top edge, when the
/// HUD does not fit below) and the HUD.
const HUD_GAP: f64 = 8.0;
pub(crate) const HUD_CORNER_RADIUS: f64 = 10.0;
/// The HUD never gets narrower than this, so a short level line still reads
/// as a small panel rather than a sliver.
const HUD_MINIMUM_WIDTH: f64 = 150.0;
/// The upgrade progress bar sits under the text: its height and the gap
/// above it.
const HUD_BAR_HEIGHT: f64 = 6.0;
const HUD_BAR_GAP: f64 = 7.0;

pub(crate) struct StatsHud {
    panel: Retained<NSPanel>,
    label: Retained<NSTextField>,
    bar: Retained<NSProgressIndicator>,
    text: String,
    progress: f64,
    origin: Option<NSPoint>,
}

impl StatsHud {
    /// Create the panel with its initial text, hidden.
    pub(crate) fn create(mtm: MainThreadMarker, text: &str) -> Result<Self, OverlayError> {
        let panel = stats_panel(mtm, 0.92);
        // The HUD is a mirror of the counters, not a control surface: pointer
        // events pass through, so it can never steal a click or block the
        // cat's own hover detection.
        panel.setIgnoresMouseEvents(true);

        let content = NSView::new(mtm);
        content.setWantsLayer(true);
        // SAFETY: the view owns its layer, and `CALayer` answers both
        // `setCornerRadius:` and `setMasksToBounds:` on every macOS this build
        // targets. objc2-quartz-core's generated bindings carry neither
        // property nor the `Message` conformance, so the selectors go through
        // the same `AnyObject` pointer route the Metal completion handler in
        // `renderer.rs` uses.
        if let Some(layer) = content.layer() {
            let layer_object: NonNull<AnyObject> = NonNull::from(&*layer).cast();
            unsafe {
                let (): () = msg_send![layer_object, setCornerRadius: HUD_CORNER_RADIUS];
                let (): () = msg_send![layer_object, setMasksToBounds: true];
            }
        }
        panel.setContentView(Some(&content));

        let label = NSTextField::labelWithString(&objc2_foundation::NSString::from_str(text), mtm);
        label.setFont(Some(&NSFont::systemFontOfSize(13.0)));
        content.addSubview(&label);

        // The upgrade bar is the system determinate indicator: it draws the
        // accent color itself, so the HUD stays native instead of inventing
        // its own chrome.
        let bar = NSProgressIndicator::new(mtm);
        bar.setIndeterminate(false);
        bar.setMinValue(0.0);
        bar.setMaxValue(1.0);
        bar.setDoubleValue(0.0);
        content.addSubview(&bar);

        let mut hud = Self {
            panel,
            label,
            bar,
            text: String::new(),
            progress: -1.0,
            origin: None,
        };
        hud.set_status(text, 0.0);
        Ok(hud)
    }

    /// Update the text, the upgrade progress, and fit the panel around them.
    /// The content size change keeps the panel's origin, so the next
    /// [`Self::show_below`] re-anchors it against the current overlay box.
    pub(crate) fn set_status(&mut self, text: &str, progress: f64) {
        let progress = progress.clamp(0.0, 1.0);
        if text != self.text {
            self.text = text.to_owned();
            self.label
                .setStringValue(&objc2_foundation::NSString::from_str(text));
            self.label.sizeToFit();
            let label_width = self.label.frame().size.width;
            let label_height = self.label.frame().size.height;
            let content_width = (label_width + HUD_PADDING * 2.0).max(HUD_MINIMUM_WIDTH);
            let content_height = label_height + HUD_BAR_GAP + HUD_BAR_HEIGHT + HUD_PADDING * 2.0;
            self.panel
                .setContentSize(NSSize::new(content_width, content_height));
            // Center the label above the bar: the bar spans the padded
            // content width, the label sits between the top padding and the
            // bar gap.
            let bar_width = content_width - HUD_PADDING * 2.0;
            self.label.setFrameOrigin(NSPoint::new(
                (content_width - label_width) / 2.0,
                HUD_PADDING + HUD_BAR_HEIGHT + HUD_BAR_GAP,
            ));
            self.bar.setFrame(NSRect::new(
                NSPoint::new(HUD_PADDING, HUD_PADDING),
                NSSize::new(bar_width, HUD_BAR_HEIGHT),
            ));
        }
        if progress != self.progress {
            self.progress = progress;
            self.bar.setDoubleValue(progress);
        }
    }

    /// Show the HUD centered below the overlay window's box, flipping above
    /// it when the box does not fit on any screen below, and clamping it
    /// inside the screens' union horizontally.
    pub(crate) fn show_below(&mut self, cat: OverlayWindowBounds, screens: &[OverlayScreenBounds]) {
        let frame = self.panel.frame();
        let mut x = f64::from(cat.x) + (f64::from(cat.width) - frame.size.width) / 2.0;
        let mut y = f64::from(cat.y) - HUD_GAP - frame.size.height;
        let candidate = OverlayWindowBounds::new(
            rounded_i32(x).unwrap_or(cat.x),
            rounded_i32(y).unwrap_or(cat.y),
            rounded_u32(frame.size.width).unwrap_or(1),
            rounded_u32(frame.size.height).unwrap_or(1),
        );
        if !bounds_inside_screens(screens, candidate) {
            y = f64::from(cat.y) + f64::from(cat.height) + HUD_GAP;
        }
        if let Some(union) = screens_union(screens) {
            let minimum_x = f64::from(union.x);
            let maximum_x =
                (f64::from(union.x) + f64::from(union.width) - frame.size.width).max(minimum_x);
            x = x.clamp(minimum_x, maximum_x);
        }
        let origin = NSPoint::new(x, y);
        if self.origin.is_none_or(|previous| previous != origin) {
            self.panel.setFrameOrigin(origin);
            self.origin = Some(origin);
        }
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
}

/// The borderless, non-activating, click-through status panel every stats
/// surface shares: a translucent background over the desktop, no shadow
/// flicker, and pointer events passing straight through so a surface can
/// never steal a click or block the cat's own hover detection.
pub(crate) fn stats_panel(mtm: MainThreadMarker, background_alpha: f64) -> Retained<NSPanel> {
    let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
    let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
        NSPanel::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1.0, 1.0)),
        style,
        NSBackingStoreType::Buffered,
        false,
    );
    panel.setTitle(&objc2_foundation::NSString::from_str("BongoCat Stats"));
    panel.setOpaque(false);
    panel.setHasShadow(true);
    panel.setAnimationBehavior(NSWindowAnimationBehavior::None);
    // A translucent window background keeps the surface readable over any
    // desktop without authoring its own theme; the label color follows the
    // same system appearance.
    panel.setBackgroundColor(Some(
        &NSColor::windowBackgroundColor().colorWithAlphaComponent(background_alpha),
    ));
    panel.setLevel(NSStatusWindowLevel);
    panel.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary,
    );
    panel.setIgnoresMouseEvents(true);
    panel
}

/// The level number riding the cat's forehead: the bare number, no chrome —
/// no background pill, no shadow, just the label floating on the fur — that
/// is on the cat whenever the cat is presented, and follows it through
/// moves, resizes and model switches. Like the HUD it is AppKit work — the
/// renderer is never asked to draw text.
///
/// The badge answers to its own pointer gate rather than the window's hover
/// hide: the session hides it once the pointer has rested on the cat for two
/// seconds and shows it again the moment the pointer leaves, so a pointer
/// merely crossing the cat never takes the number with it. The window's own
/// visibility still governs it: a cat the user hid must not leave a floating
/// number behind.
pub(crate) struct LevelBadge {
    panel: Retained<NSPanel>,
    label: Retained<NSTextField>,
    text: String,
    origin: Option<NSPoint>,
}

/// Padding around the badge label. The panel is invisible, so the padding
/// only keeps the label's own frame off the panel edges.
const BADGE_PADDING: f64 = 6.0;
/// The badge's top edge sits this fraction of the cat window's height below
/// the window's top: the middle of the bongo cat's forehead, under the ears.
const BADGE_TOP_FRACTION: f64 = 0.12;

impl LevelBadge {
    /// Create the badge panel, hidden and unsized; the first
    /// [`Self::set_status`] sizes and places it.
    pub(crate) fn create(mtm: MainThreadMarker) -> Result<Self, OverlayError> {
        // A fully transparent background and no shadow: the number floats on
        // the cat's fur with nothing behind it.
        let panel = stats_panel(mtm, 0.0);
        panel.setHasShadow(false);

        let content = NSView::new(mtm);
        panel.setContentView(Some(&content));

        let label = NSTextField::labelWithString(&objc2_foundation::NSString::from_str(""), mtm);
        label.setFont(Some(&NSFont::boldSystemFontOfSize(10.0)));
        content.addSubview(&label);

        Ok(Self {
            panel,
            label,
            text: String::new(),
            origin: None,
        })
    }

    /// Update the text and pin the badge to the cat's forehead. The content
    /// size change keeps the panel's origin, so the pin below re-anchors it
    /// against the current overlay box every call.
    pub(crate) fn set_status(&mut self, text: &str, cat: OverlayWindowBounds) {
        if text != self.text {
            self.text = text.to_owned();
            self.label
                .setStringValue(&objc2_foundation::NSString::from_str(text));
            self.label.sizeToFit();
            let label_width = self.label.frame().size.width;
            let label_height = self.label.frame().size.height;
            let content_width = label_width + BADGE_PADDING * 2.0;
            let content_height = label_height + BADGE_PADDING * 2.0;
            self.panel
                .setContentSize(NSSize::new(content_width, content_height));
            self.label
                .setFrameOrigin(NSPoint::new(BADGE_PADDING, BADGE_PADDING));
        }
        let frame = self.panel.frame();
        let x = f64::from(cat.x) + (f64::from(cat.width) - frame.size.width) / 2.0;
        let y = f64::from(cat.y) + f64::from(cat.height) * (1.0 - BADGE_TOP_FRACTION)
            - frame.size.height;
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
}

/// The bounding box of every screen, in the same global coordinates the
/// overlay windows use, or `None` when no screen is readable.
fn screens_union(screens: &[OverlayScreenBounds]) -> Option<OverlayWindowBounds> {
    let mut minimum_x = i32::MAX;
    let mut minimum_y = i32::MAX;
    let mut maximum_x = i32::MIN;
    let mut maximum_y = i32::MIN;
    for screen in screens {
        let right = screen.x.saturating_add_unsigned(screen.width);
        let top = screen.y.saturating_add_unsigned(screen.height);
        minimum_x = minimum_x.min(screen.x);
        minimum_y = minimum_y.min(screen.y);
        maximum_x = maximum_x.max(right);
        maximum_y = maximum_y.max(top);
    }
    if maximum_x <= minimum_x || maximum_y <= minimum_y {
        return None;
    }
    Some(OverlayWindowBounds::new(
        minimum_x,
        minimum_y,
        maximum_x.saturating_sub(minimum_x) as u32,
        maximum_y.saturating_sub(minimum_y) as u32,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_screen_union_spans_every_display() {
        let two = vec![
            OverlayScreenBounds {
                x: 0,
                y: 0,
                width: 1_920,
                height: 1_080,
            },
            OverlayScreenBounds {
                x: 1_920,
                y: -600,
                width: 2_560,
                height: 1_440,
            },
        ];
        let union = screens_union(&two).expect("union");
        assert_eq!(union.x, 0);
        assert_eq!(union.y, -600);
        assert_eq!(union.width, 4_480);
        // The union is a bounding box: the second display's top (840) sits
        // inside the first one's (1,080), so the height is 1,080 - (-600).
        assert_eq!(union.height, 1_680);
        assert!(screens_union(&[]).is_none());
    }
}
