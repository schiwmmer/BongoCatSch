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
/// is always on the cat while the cat is presented, and follows it through
/// moves, resizes and model switches. Like the HUD it is AppKit work — the
/// renderer is never asked to draw text.
///
/// The badge rides the same visibility gates as the window itself rather
/// than the pointer: hover hide and idle hide fade the cat out, and a
/// faded cat must not leave a floating number behind.
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

/// The badge text at the current totals: the bare level number. A number
/// needs no localization, so the badge bypasses the catalog entirely.
pub(crate) fn level_badge_status() -> String {
    let (_key, _mouse, total) = bongocat_platform::interaction_totals();
    level_badge_text(bongocat_platform::level_for(total).level)
}

/// The badge text for one level: the plain decimal number.
fn level_badge_text(level: u16) -> String {
    level.to_string()
}

/// The level name keys spelled out in full. The i18n catalog coverage test
/// requires every key the source asks for to appear as a literal, and the level
/// number is the only varying part, so the thousand names are a table rather
/// than a `format!` composition. Ten stages of a hundred levels each: level
/// `n` carries key `n`, and the stage arc lives in the names themselves —
/// every level owns one fully-authored title with no composed prefix and no
/// ordinal tier.
const LEVEL_NAME_KEYS: [&str; 1000] = [
    "stats.level.name.1",
    "stats.level.name.2",
    "stats.level.name.3",
    "stats.level.name.4",
    "stats.level.name.5",
    "stats.level.name.6",
    "stats.level.name.7",
    "stats.level.name.8",
    "stats.level.name.9",
    "stats.level.name.10",
    "stats.level.name.11",
    "stats.level.name.12",
    "stats.level.name.13",
    "stats.level.name.14",
    "stats.level.name.15",
    "stats.level.name.16",
    "stats.level.name.17",
    "stats.level.name.18",
    "stats.level.name.19",
    "stats.level.name.20",
    "stats.level.name.21",
    "stats.level.name.22",
    "stats.level.name.23",
    "stats.level.name.24",
    "stats.level.name.25",
    "stats.level.name.26",
    "stats.level.name.27",
    "stats.level.name.28",
    "stats.level.name.29",
    "stats.level.name.30",
    "stats.level.name.31",
    "stats.level.name.32",
    "stats.level.name.33",
    "stats.level.name.34",
    "stats.level.name.35",
    "stats.level.name.36",
    "stats.level.name.37",
    "stats.level.name.38",
    "stats.level.name.39",
    "stats.level.name.40",
    "stats.level.name.41",
    "stats.level.name.42",
    "stats.level.name.43",
    "stats.level.name.44",
    "stats.level.name.45",
    "stats.level.name.46",
    "stats.level.name.47",
    "stats.level.name.48",
    "stats.level.name.49",
    "stats.level.name.50",
    "stats.level.name.51",
    "stats.level.name.52",
    "stats.level.name.53",
    "stats.level.name.54",
    "stats.level.name.55",
    "stats.level.name.56",
    "stats.level.name.57",
    "stats.level.name.58",
    "stats.level.name.59",
    "stats.level.name.60",
    "stats.level.name.61",
    "stats.level.name.62",
    "stats.level.name.63",
    "stats.level.name.64",
    "stats.level.name.65",
    "stats.level.name.66",
    "stats.level.name.67",
    "stats.level.name.68",
    "stats.level.name.69",
    "stats.level.name.70",
    "stats.level.name.71",
    "stats.level.name.72",
    "stats.level.name.73",
    "stats.level.name.74",
    "stats.level.name.75",
    "stats.level.name.76",
    "stats.level.name.77",
    "stats.level.name.78",
    "stats.level.name.79",
    "stats.level.name.80",
    "stats.level.name.81",
    "stats.level.name.82",
    "stats.level.name.83",
    "stats.level.name.84",
    "stats.level.name.85",
    "stats.level.name.86",
    "stats.level.name.87",
    "stats.level.name.88",
    "stats.level.name.89",
    "stats.level.name.90",
    "stats.level.name.91",
    "stats.level.name.92",
    "stats.level.name.93",
    "stats.level.name.94",
    "stats.level.name.95",
    "stats.level.name.96",
    "stats.level.name.97",
    "stats.level.name.98",
    "stats.level.name.99",
    "stats.level.name.100",
    "stats.level.name.101",
    "stats.level.name.102",
    "stats.level.name.103",
    "stats.level.name.104",
    "stats.level.name.105",
    "stats.level.name.106",
    "stats.level.name.107",
    "stats.level.name.108",
    "stats.level.name.109",
    "stats.level.name.110",
    "stats.level.name.111",
    "stats.level.name.112",
    "stats.level.name.113",
    "stats.level.name.114",
    "stats.level.name.115",
    "stats.level.name.116",
    "stats.level.name.117",
    "stats.level.name.118",
    "stats.level.name.119",
    "stats.level.name.120",
    "stats.level.name.121",
    "stats.level.name.122",
    "stats.level.name.123",
    "stats.level.name.124",
    "stats.level.name.125",
    "stats.level.name.126",
    "stats.level.name.127",
    "stats.level.name.128",
    "stats.level.name.129",
    "stats.level.name.130",
    "stats.level.name.131",
    "stats.level.name.132",
    "stats.level.name.133",
    "stats.level.name.134",
    "stats.level.name.135",
    "stats.level.name.136",
    "stats.level.name.137",
    "stats.level.name.138",
    "stats.level.name.139",
    "stats.level.name.140",
    "stats.level.name.141",
    "stats.level.name.142",
    "stats.level.name.143",
    "stats.level.name.144",
    "stats.level.name.145",
    "stats.level.name.146",
    "stats.level.name.147",
    "stats.level.name.148",
    "stats.level.name.149",
    "stats.level.name.150",
    "stats.level.name.151",
    "stats.level.name.152",
    "stats.level.name.153",
    "stats.level.name.154",
    "stats.level.name.155",
    "stats.level.name.156",
    "stats.level.name.157",
    "stats.level.name.158",
    "stats.level.name.159",
    "stats.level.name.160",
    "stats.level.name.161",
    "stats.level.name.162",
    "stats.level.name.163",
    "stats.level.name.164",
    "stats.level.name.165",
    "stats.level.name.166",
    "stats.level.name.167",
    "stats.level.name.168",
    "stats.level.name.169",
    "stats.level.name.170",
    "stats.level.name.171",
    "stats.level.name.172",
    "stats.level.name.173",
    "stats.level.name.174",
    "stats.level.name.175",
    "stats.level.name.176",
    "stats.level.name.177",
    "stats.level.name.178",
    "stats.level.name.179",
    "stats.level.name.180",
    "stats.level.name.181",
    "stats.level.name.182",
    "stats.level.name.183",
    "stats.level.name.184",
    "stats.level.name.185",
    "stats.level.name.186",
    "stats.level.name.187",
    "stats.level.name.188",
    "stats.level.name.189",
    "stats.level.name.190",
    "stats.level.name.191",
    "stats.level.name.192",
    "stats.level.name.193",
    "stats.level.name.194",
    "stats.level.name.195",
    "stats.level.name.196",
    "stats.level.name.197",
    "stats.level.name.198",
    "stats.level.name.199",
    "stats.level.name.200",
    "stats.level.name.201",
    "stats.level.name.202",
    "stats.level.name.203",
    "stats.level.name.204",
    "stats.level.name.205",
    "stats.level.name.206",
    "stats.level.name.207",
    "stats.level.name.208",
    "stats.level.name.209",
    "stats.level.name.210",
    "stats.level.name.211",
    "stats.level.name.212",
    "stats.level.name.213",
    "stats.level.name.214",
    "stats.level.name.215",
    "stats.level.name.216",
    "stats.level.name.217",
    "stats.level.name.218",
    "stats.level.name.219",
    "stats.level.name.220",
    "stats.level.name.221",
    "stats.level.name.222",
    "stats.level.name.223",
    "stats.level.name.224",
    "stats.level.name.225",
    "stats.level.name.226",
    "stats.level.name.227",
    "stats.level.name.228",
    "stats.level.name.229",
    "stats.level.name.230",
    "stats.level.name.231",
    "stats.level.name.232",
    "stats.level.name.233",
    "stats.level.name.234",
    "stats.level.name.235",
    "stats.level.name.236",
    "stats.level.name.237",
    "stats.level.name.238",
    "stats.level.name.239",
    "stats.level.name.240",
    "stats.level.name.241",
    "stats.level.name.242",
    "stats.level.name.243",
    "stats.level.name.244",
    "stats.level.name.245",
    "stats.level.name.246",
    "stats.level.name.247",
    "stats.level.name.248",
    "stats.level.name.249",
    "stats.level.name.250",
    "stats.level.name.251",
    "stats.level.name.252",
    "stats.level.name.253",
    "stats.level.name.254",
    "stats.level.name.255",
    "stats.level.name.256",
    "stats.level.name.257",
    "stats.level.name.258",
    "stats.level.name.259",
    "stats.level.name.260",
    "stats.level.name.261",
    "stats.level.name.262",
    "stats.level.name.263",
    "stats.level.name.264",
    "stats.level.name.265",
    "stats.level.name.266",
    "stats.level.name.267",
    "stats.level.name.268",
    "stats.level.name.269",
    "stats.level.name.270",
    "stats.level.name.271",
    "stats.level.name.272",
    "stats.level.name.273",
    "stats.level.name.274",
    "stats.level.name.275",
    "stats.level.name.276",
    "stats.level.name.277",
    "stats.level.name.278",
    "stats.level.name.279",
    "stats.level.name.280",
    "stats.level.name.281",
    "stats.level.name.282",
    "stats.level.name.283",
    "stats.level.name.284",
    "stats.level.name.285",
    "stats.level.name.286",
    "stats.level.name.287",
    "stats.level.name.288",
    "stats.level.name.289",
    "stats.level.name.290",
    "stats.level.name.291",
    "stats.level.name.292",
    "stats.level.name.293",
    "stats.level.name.294",
    "stats.level.name.295",
    "stats.level.name.296",
    "stats.level.name.297",
    "stats.level.name.298",
    "stats.level.name.299",
    "stats.level.name.300",
    "stats.level.name.301",
    "stats.level.name.302",
    "stats.level.name.303",
    "stats.level.name.304",
    "stats.level.name.305",
    "stats.level.name.306",
    "stats.level.name.307",
    "stats.level.name.308",
    "stats.level.name.309",
    "stats.level.name.310",
    "stats.level.name.311",
    "stats.level.name.312",
    "stats.level.name.313",
    "stats.level.name.314",
    "stats.level.name.315",
    "stats.level.name.316",
    "stats.level.name.317",
    "stats.level.name.318",
    "stats.level.name.319",
    "stats.level.name.320",
    "stats.level.name.321",
    "stats.level.name.322",
    "stats.level.name.323",
    "stats.level.name.324",
    "stats.level.name.325",
    "stats.level.name.326",
    "stats.level.name.327",
    "stats.level.name.328",
    "stats.level.name.329",
    "stats.level.name.330",
    "stats.level.name.331",
    "stats.level.name.332",
    "stats.level.name.333",
    "stats.level.name.334",
    "stats.level.name.335",
    "stats.level.name.336",
    "stats.level.name.337",
    "stats.level.name.338",
    "stats.level.name.339",
    "stats.level.name.340",
    "stats.level.name.341",
    "stats.level.name.342",
    "stats.level.name.343",
    "stats.level.name.344",
    "stats.level.name.345",
    "stats.level.name.346",
    "stats.level.name.347",
    "stats.level.name.348",
    "stats.level.name.349",
    "stats.level.name.350",
    "stats.level.name.351",
    "stats.level.name.352",
    "stats.level.name.353",
    "stats.level.name.354",
    "stats.level.name.355",
    "stats.level.name.356",
    "stats.level.name.357",
    "stats.level.name.358",
    "stats.level.name.359",
    "stats.level.name.360",
    "stats.level.name.361",
    "stats.level.name.362",
    "stats.level.name.363",
    "stats.level.name.364",
    "stats.level.name.365",
    "stats.level.name.366",
    "stats.level.name.367",
    "stats.level.name.368",
    "stats.level.name.369",
    "stats.level.name.370",
    "stats.level.name.371",
    "stats.level.name.372",
    "stats.level.name.373",
    "stats.level.name.374",
    "stats.level.name.375",
    "stats.level.name.376",
    "stats.level.name.377",
    "stats.level.name.378",
    "stats.level.name.379",
    "stats.level.name.380",
    "stats.level.name.381",
    "stats.level.name.382",
    "stats.level.name.383",
    "stats.level.name.384",
    "stats.level.name.385",
    "stats.level.name.386",
    "stats.level.name.387",
    "stats.level.name.388",
    "stats.level.name.389",
    "stats.level.name.390",
    "stats.level.name.391",
    "stats.level.name.392",
    "stats.level.name.393",
    "stats.level.name.394",
    "stats.level.name.395",
    "stats.level.name.396",
    "stats.level.name.397",
    "stats.level.name.398",
    "stats.level.name.399",
    "stats.level.name.400",
    "stats.level.name.401",
    "stats.level.name.402",
    "stats.level.name.403",
    "stats.level.name.404",
    "stats.level.name.405",
    "stats.level.name.406",
    "stats.level.name.407",
    "stats.level.name.408",
    "stats.level.name.409",
    "stats.level.name.410",
    "stats.level.name.411",
    "stats.level.name.412",
    "stats.level.name.413",
    "stats.level.name.414",
    "stats.level.name.415",
    "stats.level.name.416",
    "stats.level.name.417",
    "stats.level.name.418",
    "stats.level.name.419",
    "stats.level.name.420",
    "stats.level.name.421",
    "stats.level.name.422",
    "stats.level.name.423",
    "stats.level.name.424",
    "stats.level.name.425",
    "stats.level.name.426",
    "stats.level.name.427",
    "stats.level.name.428",
    "stats.level.name.429",
    "stats.level.name.430",
    "stats.level.name.431",
    "stats.level.name.432",
    "stats.level.name.433",
    "stats.level.name.434",
    "stats.level.name.435",
    "stats.level.name.436",
    "stats.level.name.437",
    "stats.level.name.438",
    "stats.level.name.439",
    "stats.level.name.440",
    "stats.level.name.441",
    "stats.level.name.442",
    "stats.level.name.443",
    "stats.level.name.444",
    "stats.level.name.445",
    "stats.level.name.446",
    "stats.level.name.447",
    "stats.level.name.448",
    "stats.level.name.449",
    "stats.level.name.450",
    "stats.level.name.451",
    "stats.level.name.452",
    "stats.level.name.453",
    "stats.level.name.454",
    "stats.level.name.455",
    "stats.level.name.456",
    "stats.level.name.457",
    "stats.level.name.458",
    "stats.level.name.459",
    "stats.level.name.460",
    "stats.level.name.461",
    "stats.level.name.462",
    "stats.level.name.463",
    "stats.level.name.464",
    "stats.level.name.465",
    "stats.level.name.466",
    "stats.level.name.467",
    "stats.level.name.468",
    "stats.level.name.469",
    "stats.level.name.470",
    "stats.level.name.471",
    "stats.level.name.472",
    "stats.level.name.473",
    "stats.level.name.474",
    "stats.level.name.475",
    "stats.level.name.476",
    "stats.level.name.477",
    "stats.level.name.478",
    "stats.level.name.479",
    "stats.level.name.480",
    "stats.level.name.481",
    "stats.level.name.482",
    "stats.level.name.483",
    "stats.level.name.484",
    "stats.level.name.485",
    "stats.level.name.486",
    "stats.level.name.487",
    "stats.level.name.488",
    "stats.level.name.489",
    "stats.level.name.490",
    "stats.level.name.491",
    "stats.level.name.492",
    "stats.level.name.493",
    "stats.level.name.494",
    "stats.level.name.495",
    "stats.level.name.496",
    "stats.level.name.497",
    "stats.level.name.498",
    "stats.level.name.499",
    "stats.level.name.500",
    "stats.level.name.501",
    "stats.level.name.502",
    "stats.level.name.503",
    "stats.level.name.504",
    "stats.level.name.505",
    "stats.level.name.506",
    "stats.level.name.507",
    "stats.level.name.508",
    "stats.level.name.509",
    "stats.level.name.510",
    "stats.level.name.511",
    "stats.level.name.512",
    "stats.level.name.513",
    "stats.level.name.514",
    "stats.level.name.515",
    "stats.level.name.516",
    "stats.level.name.517",
    "stats.level.name.518",
    "stats.level.name.519",
    "stats.level.name.520",
    "stats.level.name.521",
    "stats.level.name.522",
    "stats.level.name.523",
    "stats.level.name.524",
    "stats.level.name.525",
    "stats.level.name.526",
    "stats.level.name.527",
    "stats.level.name.528",
    "stats.level.name.529",
    "stats.level.name.530",
    "stats.level.name.531",
    "stats.level.name.532",
    "stats.level.name.533",
    "stats.level.name.534",
    "stats.level.name.535",
    "stats.level.name.536",
    "stats.level.name.537",
    "stats.level.name.538",
    "stats.level.name.539",
    "stats.level.name.540",
    "stats.level.name.541",
    "stats.level.name.542",
    "stats.level.name.543",
    "stats.level.name.544",
    "stats.level.name.545",
    "stats.level.name.546",
    "stats.level.name.547",
    "stats.level.name.548",
    "stats.level.name.549",
    "stats.level.name.550",
    "stats.level.name.551",
    "stats.level.name.552",
    "stats.level.name.553",
    "stats.level.name.554",
    "stats.level.name.555",
    "stats.level.name.556",
    "stats.level.name.557",
    "stats.level.name.558",
    "stats.level.name.559",
    "stats.level.name.560",
    "stats.level.name.561",
    "stats.level.name.562",
    "stats.level.name.563",
    "stats.level.name.564",
    "stats.level.name.565",
    "stats.level.name.566",
    "stats.level.name.567",
    "stats.level.name.568",
    "stats.level.name.569",
    "stats.level.name.570",
    "stats.level.name.571",
    "stats.level.name.572",
    "stats.level.name.573",
    "stats.level.name.574",
    "stats.level.name.575",
    "stats.level.name.576",
    "stats.level.name.577",
    "stats.level.name.578",
    "stats.level.name.579",
    "stats.level.name.580",
    "stats.level.name.581",
    "stats.level.name.582",
    "stats.level.name.583",
    "stats.level.name.584",
    "stats.level.name.585",
    "stats.level.name.586",
    "stats.level.name.587",
    "stats.level.name.588",
    "stats.level.name.589",
    "stats.level.name.590",
    "stats.level.name.591",
    "stats.level.name.592",
    "stats.level.name.593",
    "stats.level.name.594",
    "stats.level.name.595",
    "stats.level.name.596",
    "stats.level.name.597",
    "stats.level.name.598",
    "stats.level.name.599",
    "stats.level.name.600",
    "stats.level.name.601",
    "stats.level.name.602",
    "stats.level.name.603",
    "stats.level.name.604",
    "stats.level.name.605",
    "stats.level.name.606",
    "stats.level.name.607",
    "stats.level.name.608",
    "stats.level.name.609",
    "stats.level.name.610",
    "stats.level.name.611",
    "stats.level.name.612",
    "stats.level.name.613",
    "stats.level.name.614",
    "stats.level.name.615",
    "stats.level.name.616",
    "stats.level.name.617",
    "stats.level.name.618",
    "stats.level.name.619",
    "stats.level.name.620",
    "stats.level.name.621",
    "stats.level.name.622",
    "stats.level.name.623",
    "stats.level.name.624",
    "stats.level.name.625",
    "stats.level.name.626",
    "stats.level.name.627",
    "stats.level.name.628",
    "stats.level.name.629",
    "stats.level.name.630",
    "stats.level.name.631",
    "stats.level.name.632",
    "stats.level.name.633",
    "stats.level.name.634",
    "stats.level.name.635",
    "stats.level.name.636",
    "stats.level.name.637",
    "stats.level.name.638",
    "stats.level.name.639",
    "stats.level.name.640",
    "stats.level.name.641",
    "stats.level.name.642",
    "stats.level.name.643",
    "stats.level.name.644",
    "stats.level.name.645",
    "stats.level.name.646",
    "stats.level.name.647",
    "stats.level.name.648",
    "stats.level.name.649",
    "stats.level.name.650",
    "stats.level.name.651",
    "stats.level.name.652",
    "stats.level.name.653",
    "stats.level.name.654",
    "stats.level.name.655",
    "stats.level.name.656",
    "stats.level.name.657",
    "stats.level.name.658",
    "stats.level.name.659",
    "stats.level.name.660",
    "stats.level.name.661",
    "stats.level.name.662",
    "stats.level.name.663",
    "stats.level.name.664",
    "stats.level.name.665",
    "stats.level.name.666",
    "stats.level.name.667",
    "stats.level.name.668",
    "stats.level.name.669",
    "stats.level.name.670",
    "stats.level.name.671",
    "stats.level.name.672",
    "stats.level.name.673",
    "stats.level.name.674",
    "stats.level.name.675",
    "stats.level.name.676",
    "stats.level.name.677",
    "stats.level.name.678",
    "stats.level.name.679",
    "stats.level.name.680",
    "stats.level.name.681",
    "stats.level.name.682",
    "stats.level.name.683",
    "stats.level.name.684",
    "stats.level.name.685",
    "stats.level.name.686",
    "stats.level.name.687",
    "stats.level.name.688",
    "stats.level.name.689",
    "stats.level.name.690",
    "stats.level.name.691",
    "stats.level.name.692",
    "stats.level.name.693",
    "stats.level.name.694",
    "stats.level.name.695",
    "stats.level.name.696",
    "stats.level.name.697",
    "stats.level.name.698",
    "stats.level.name.699",
    "stats.level.name.700",
    "stats.level.name.701",
    "stats.level.name.702",
    "stats.level.name.703",
    "stats.level.name.704",
    "stats.level.name.705",
    "stats.level.name.706",
    "stats.level.name.707",
    "stats.level.name.708",
    "stats.level.name.709",
    "stats.level.name.710",
    "stats.level.name.711",
    "stats.level.name.712",
    "stats.level.name.713",
    "stats.level.name.714",
    "stats.level.name.715",
    "stats.level.name.716",
    "stats.level.name.717",
    "stats.level.name.718",
    "stats.level.name.719",
    "stats.level.name.720",
    "stats.level.name.721",
    "stats.level.name.722",
    "stats.level.name.723",
    "stats.level.name.724",
    "stats.level.name.725",
    "stats.level.name.726",
    "stats.level.name.727",
    "stats.level.name.728",
    "stats.level.name.729",
    "stats.level.name.730",
    "stats.level.name.731",
    "stats.level.name.732",
    "stats.level.name.733",
    "stats.level.name.734",
    "stats.level.name.735",
    "stats.level.name.736",
    "stats.level.name.737",
    "stats.level.name.738",
    "stats.level.name.739",
    "stats.level.name.740",
    "stats.level.name.741",
    "stats.level.name.742",
    "stats.level.name.743",
    "stats.level.name.744",
    "stats.level.name.745",
    "stats.level.name.746",
    "stats.level.name.747",
    "stats.level.name.748",
    "stats.level.name.749",
    "stats.level.name.750",
    "stats.level.name.751",
    "stats.level.name.752",
    "stats.level.name.753",
    "stats.level.name.754",
    "stats.level.name.755",
    "stats.level.name.756",
    "stats.level.name.757",
    "stats.level.name.758",
    "stats.level.name.759",
    "stats.level.name.760",
    "stats.level.name.761",
    "stats.level.name.762",
    "stats.level.name.763",
    "stats.level.name.764",
    "stats.level.name.765",
    "stats.level.name.766",
    "stats.level.name.767",
    "stats.level.name.768",
    "stats.level.name.769",
    "stats.level.name.770",
    "stats.level.name.771",
    "stats.level.name.772",
    "stats.level.name.773",
    "stats.level.name.774",
    "stats.level.name.775",
    "stats.level.name.776",
    "stats.level.name.777",
    "stats.level.name.778",
    "stats.level.name.779",
    "stats.level.name.780",
    "stats.level.name.781",
    "stats.level.name.782",
    "stats.level.name.783",
    "stats.level.name.784",
    "stats.level.name.785",
    "stats.level.name.786",
    "stats.level.name.787",
    "stats.level.name.788",
    "stats.level.name.789",
    "stats.level.name.790",
    "stats.level.name.791",
    "stats.level.name.792",
    "stats.level.name.793",
    "stats.level.name.794",
    "stats.level.name.795",
    "stats.level.name.796",
    "stats.level.name.797",
    "stats.level.name.798",
    "stats.level.name.799",
    "stats.level.name.800",
    "stats.level.name.801",
    "stats.level.name.802",
    "stats.level.name.803",
    "stats.level.name.804",
    "stats.level.name.805",
    "stats.level.name.806",
    "stats.level.name.807",
    "stats.level.name.808",
    "stats.level.name.809",
    "stats.level.name.810",
    "stats.level.name.811",
    "stats.level.name.812",
    "stats.level.name.813",
    "stats.level.name.814",
    "stats.level.name.815",
    "stats.level.name.816",
    "stats.level.name.817",
    "stats.level.name.818",
    "stats.level.name.819",
    "stats.level.name.820",
    "stats.level.name.821",
    "stats.level.name.822",
    "stats.level.name.823",
    "stats.level.name.824",
    "stats.level.name.825",
    "stats.level.name.826",
    "stats.level.name.827",
    "stats.level.name.828",
    "stats.level.name.829",
    "stats.level.name.830",
    "stats.level.name.831",
    "stats.level.name.832",
    "stats.level.name.833",
    "stats.level.name.834",
    "stats.level.name.835",
    "stats.level.name.836",
    "stats.level.name.837",
    "stats.level.name.838",
    "stats.level.name.839",
    "stats.level.name.840",
    "stats.level.name.841",
    "stats.level.name.842",
    "stats.level.name.843",
    "stats.level.name.844",
    "stats.level.name.845",
    "stats.level.name.846",
    "stats.level.name.847",
    "stats.level.name.848",
    "stats.level.name.849",
    "stats.level.name.850",
    "stats.level.name.851",
    "stats.level.name.852",
    "stats.level.name.853",
    "stats.level.name.854",
    "stats.level.name.855",
    "stats.level.name.856",
    "stats.level.name.857",
    "stats.level.name.858",
    "stats.level.name.859",
    "stats.level.name.860",
    "stats.level.name.861",
    "stats.level.name.862",
    "stats.level.name.863",
    "stats.level.name.864",
    "stats.level.name.865",
    "stats.level.name.866",
    "stats.level.name.867",
    "stats.level.name.868",
    "stats.level.name.869",
    "stats.level.name.870",
    "stats.level.name.871",
    "stats.level.name.872",
    "stats.level.name.873",
    "stats.level.name.874",
    "stats.level.name.875",
    "stats.level.name.876",
    "stats.level.name.877",
    "stats.level.name.878",
    "stats.level.name.879",
    "stats.level.name.880",
    "stats.level.name.881",
    "stats.level.name.882",
    "stats.level.name.883",
    "stats.level.name.884",
    "stats.level.name.885",
    "stats.level.name.886",
    "stats.level.name.887",
    "stats.level.name.888",
    "stats.level.name.889",
    "stats.level.name.890",
    "stats.level.name.891",
    "stats.level.name.892",
    "stats.level.name.893",
    "stats.level.name.894",
    "stats.level.name.895",
    "stats.level.name.896",
    "stats.level.name.897",
    "stats.level.name.898",
    "stats.level.name.899",
    "stats.level.name.900",
    "stats.level.name.901",
    "stats.level.name.902",
    "stats.level.name.903",
    "stats.level.name.904",
    "stats.level.name.905",
    "stats.level.name.906",
    "stats.level.name.907",
    "stats.level.name.908",
    "stats.level.name.909",
    "stats.level.name.910",
    "stats.level.name.911",
    "stats.level.name.912",
    "stats.level.name.913",
    "stats.level.name.914",
    "stats.level.name.915",
    "stats.level.name.916",
    "stats.level.name.917",
    "stats.level.name.918",
    "stats.level.name.919",
    "stats.level.name.920",
    "stats.level.name.921",
    "stats.level.name.922",
    "stats.level.name.923",
    "stats.level.name.924",
    "stats.level.name.925",
    "stats.level.name.926",
    "stats.level.name.927",
    "stats.level.name.928",
    "stats.level.name.929",
    "stats.level.name.930",
    "stats.level.name.931",
    "stats.level.name.932",
    "stats.level.name.933",
    "stats.level.name.934",
    "stats.level.name.935",
    "stats.level.name.936",
    "stats.level.name.937",
    "stats.level.name.938",
    "stats.level.name.939",
    "stats.level.name.940",
    "stats.level.name.941",
    "stats.level.name.942",
    "stats.level.name.943",
    "stats.level.name.944",
    "stats.level.name.945",
    "stats.level.name.946",
    "stats.level.name.947",
    "stats.level.name.948",
    "stats.level.name.949",
    "stats.level.name.950",
    "stats.level.name.951",
    "stats.level.name.952",
    "stats.level.name.953",
    "stats.level.name.954",
    "stats.level.name.955",
    "stats.level.name.956",
    "stats.level.name.957",
    "stats.level.name.958",
    "stats.level.name.959",
    "stats.level.name.960",
    "stats.level.name.961",
    "stats.level.name.962",
    "stats.level.name.963",
    "stats.level.name.964",
    "stats.level.name.965",
    "stats.level.name.966",
    "stats.level.name.967",
    "stats.level.name.968",
    "stats.level.name.969",
    "stats.level.name.970",
    "stats.level.name.971",
    "stats.level.name.972",
    "stats.level.name.973",
    "stats.level.name.974",
    "stats.level.name.975",
    "stats.level.name.976",
    "stats.level.name.977",
    "stats.level.name.978",
    "stats.level.name.979",
    "stats.level.name.980",
    "stats.level.name.981",
    "stats.level.name.982",
    "stats.level.name.983",
    "stats.level.name.984",
    "stats.level.name.985",
    "stats.level.name.986",
    "stats.level.name.987",
    "stats.level.name.988",
    "stats.level.name.989",
    "stats.level.name.990",
    "stats.level.name.991",
    "stats.level.name.992",
    "stats.level.name.993",
    "stats.level.name.994",
    "stats.level.name.995",
    "stats.level.name.996",
    "stats.level.name.997",
    "stats.level.name.998",
    "stats.level.name.999",
    "stats.level.name.1000",
];

/// The name one level carries: one authored title per level, picked by the
/// level number alone. A level past the table clamps to the last name, which
/// only a build that shrank the table can reach.
fn title_for_level(locale: &str, level: u16) -> String {
    let index = usize::from(level.saturating_sub(1)).min(LEVEL_NAME_KEYS.len() - 1);
    bongocat_i18n::format_text(locale, LEVEL_NAME_KEYS[index], &[])
}

/// The HUD contents for the current totals: the level line, then either the
/// progress to the next level (with the upgrade percentage) or the capped
/// total line, plus the progress fraction for the bar.
pub(crate) fn hud_status(locale: &str) -> (String, f64) {
    let (_key, _mouse, total) = bongocat_platform::interaction_totals();
    let level = bongocat_platform::level_for(total);
    let title = title_for_level(locale, level.level);
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
            text.contains("5 to go"),
            "the progress to the next threshold is present: {text}"
        );
        assert!(
            text.contains("0%"),
            "the upgrade percentage is present: {text}"
        );
        assert!((0.0..=1.0).contains(&progress));
    }

    /// Every one of the thousand levels carries its own authored name — ten
    /// stages of a hundred levels each, no composed prefix and no ordinal
    /// tier — so the whole table is free of repeats, and both ends are pinned
    /// to the English catalog.
    #[test]
    fn every_level_has_a_unique_name() {
        let mut names = std::collections::BTreeSet::new();
        for level in 1..=bongocat_platform::MAX_LEVEL {
            let name = title_for_level("en-US", level);
            assert!(names.insert(name.clone()), "level {level} repeats a name");
        }
        assert_eq!(names.len(), usize::from(bongocat_platform::MAX_LEVEL));
        assert_eq!(title_for_level("en-US", 1), "Newborn little bean");
        assert_eq!(title_for_level("en-US", 1000), "The eternal Cat God");
    }

    /// The forehead badge carries the bare level number: a plain integer
    /// with no prefix and no catalog lookup.
    #[test]
    fn the_badge_shows_only_the_number() {
        assert_eq!(level_badge_text(1), "1");
        assert_eq!(level_badge_text(1000), "1000");
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
