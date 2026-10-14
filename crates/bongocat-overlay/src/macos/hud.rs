//! The hover stats HUD: a small read-only panel under the cat.
//!
//! When the pointer rests on the overlay window, a non-activating panel with
//! the lifetime interaction count and the current level appears just below
//! it, and disappears when the pointer leaves. The HUD is an AppKit panel
//! with an `NSTextField`, not Metal work: the model's renderer is never
//! asked to draw text, and a frame that shows the HUD costs nothing but a
//! possible `setFrameOrigin`.
//!
//! The panel ignores mouse events, so resting the pointer on the HUD itself
//! is the same as resting it on the cat — the hover-hide hit test still sees
//! the pointer inside the overlay box, which is what keeps the HUD stable
//! while the user reads it. When hover hide is configured, the cat fades out
//! while the HUD stays up: the cat steps aside and the counters are what the
//! pointer found.
//!
//! The text is localized with the system language (`system_language`)
//! resolved onto a shipped catalog. The settings window resolves the user's
//! chosen language from the configuration; the session has no configuration
//! access, and the HUD is a glanceable surface, so the system catalog is the
//! accepted trade-off rather than threading a locale through the runtime
//! snapshot.

use super::*;

/// Horizontal and vertical padding around the label inside the panel.
const HUD_PADDING: f64 = 14.0;
/// The gap between the overlay window's bottom edge (or top edge, when the
/// HUD does not fit below) and the HUD.
const HUD_GAP: f64 = 8.0;
const HUD_CORNER_RADIUS: f64 = 10.0;
/// The HUD never gets narrower than this, so a short level line still reads
/// as a small panel rather than a sliver.
const HUD_MINIMUM_WIDTH: f64 = 96.0;
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
        // A translucent window background keeps the HUD readable over any
        // desktop without authoring its own theme; the label color follows
        // the same system appearance.
        panel.setBackgroundColor(Some(
            &NSColor::windowBackgroundColor().colorWithAlphaComponent(0.92),
        ));
        panel.setLevel(NSStatusWindowLevel);
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary,
        );
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

/// The shipped catalog the system language resolves onto.
pub(crate) fn stats_hud_locale() -> &'static str {
    let system = bongocat_platform::system_language();
    system.resolve(system).code()
}

/// The HUD text and upgrade progress for the system language at the current
/// totals.
pub(crate) fn stats_hud_status() -> (String, f64) {
    hud_status(stats_hud_locale())
}

/// The level title keys spelled out in full. The i18n catalog coverage test
/// requires every key the source asks for to appear as a literal, and the
/// title number is the only varying part, so the ten titles are a table
/// rather than a `format!` composition.
const LEVEL_TITLE_KEYS: [&str; 10] = [
    "stats.level.title.1",
    "stats.level.title.2",
    "stats.level.title.3",
    "stats.level.title.4",
    "stats.level.title.5",
    "stats.level.title.6",
    "stats.level.title.7",
    "stats.level.title.8",
    "stats.level.title.9",
    "stats.level.title.10",
];

/// The ten titles spread across the thousand levels: every hundred levels
/// climbs one title, so the words still mean something at 1000 levels.
fn title_key_for_level(level: u16) -> &'static str {
    let tier = usize::from(level.saturating_sub(1)) / 100;
    LEVEL_TITLE_KEYS[tier.min(LEVEL_TITLE_KEYS.len() - 1)]
}

/// The HUD contents for the current totals: the level line, then either the
/// progress to the next level (with the upgrade percentage) or the capped
/// total line, plus the progress fraction for the bar.
pub(crate) fn hud_status(locale: &str) -> (String, f64) {
    let (_key, _mouse, total) = bongocat_platform::interaction_totals();
    let level = bongocat_platform::level_for(total);
    let title_key = title_key_for_level(level.level);
    let title = bongocat_i18n::format_text(locale, title_key, &[]);
    let mut text = bongocat_i18n::format_text(
        locale,
        "stats.hud.level",
        &[("level", level.level.to_string()), ("title", title)],
    );
    text.push('\n');
    let total_text = format_thousands(total);
    let (second_line, progress) = match level.next {
        Some(next) => {
            let span = next.saturating_sub(level.floor).max(1);
            let percent = (total.saturating_sub(level.floor)) * 100 / span;
            let percent = percent.min(100);
            let fraction = (total.saturating_sub(level.floor)) as f64 / span as f64;
            let line = bongocat_i18n::format_text(
                locale,
                "stats.hud.progress",
                &[
                    ("total", total_text),
                    ("remaining", format_thousands(next.saturating_sub(total))),
                    ("percent", percent.to_string()),
                ],
            );
            (line, fraction)
        }
        None => (
            bongocat_i18n::format_text(locale, "stats.hud.total", &[("total", total_text)]),
            1.0,
        ),
    };
    text.push_str(&second_line);
    (text, progress)
}

/// Group the integer digits in threes (`12340` → `12,340`), the only number
/// formatting the HUD needs.
fn format_thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_group_from_the_right() {
        assert_eq!(format_thousands(0), "0");
        assert_eq!(format_thousands(999), "999");
        assert_eq!(format_thousands(1_000), "1,000");
        assert_eq!(format_thousands(12_340), "12,340");
        assert_eq!(format_thousands(123_456_789), "123,456,789");
    }

    /// The HUD text carries the level and the totals through the catalog:
    /// with the English catalog the placeholders pin the key set.
    #[test]
    fn hud_status_contains_the_level_the_progress_line_and_the_percent() {
        let (text, progress) = hud_status("en-US");
        assert!(text.contains("Lv.1"), "the level line is present: {text}");
        assert!(text.contains('\n'), "the text has two lines: {text}");
        assert!(
            text.contains("1 to level up"),
            "the progress to the next threshold is present: {text}"
        );
        assert!(
            text.contains("0%"),
            "the upgrade percentage is present: {text}"
        );
        assert!((0.0..=1.0).contains(&progress));
    }

    /// The ten titles cover the thousand levels a hundred at a time, and the
    /// cap clamps into the last title.
    #[test]
    fn title_tiers_map_one_hundred_levels_each() {
        assert_eq!(title_key_for_level(1), "stats.level.title.1");
        assert_eq!(title_key_for_level(100), "stats.level.title.1");
        assert_eq!(title_key_for_level(101), "stats.level.title.2");
        assert_eq!(title_key_for_level(500), "stats.level.title.5");
        assert_eq!(title_key_for_level(1000), "stats.level.title.10");
    }

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
