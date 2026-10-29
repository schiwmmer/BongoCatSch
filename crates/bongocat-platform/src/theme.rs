//! Application appearance for the native surfaces the platform layer owns.
//!
//! The product lets the user pick Light, Dark or System, but the surfaces that have to
//! follow that choice are not the ones the product paints: the window frame, the alert a
//! permission request raises, the tray and context menus and the open/save panels all
//! belong to the operating system. This module is the single place that knows what each
//! platform can and cannot restyle, so the UI layer does not have to.
//!
//! What macOS supports (verified 2026-09-18, see ADR-0048):
//!
//! | surface | macOS |
//! | --- | --- |
//! | window frame | application theme |
//! | alerts | application theme |
//! | tray and context menus | application theme |
//! | open/save panels | application theme |
//!
//! macOS reaches every row through one process-wide `NSApplication.appearance`: the
//! appearance is inherited by each window and, through the window, by the alerts, menus
//! and panels it presents. The application must set it at startup through
//! [`apply_process_theme`] before creating the overlay; applying it from a settings window
//! alone would make the overlay's first context menu depend on that window having existed.
//!
use raw_window_handle::HasWindowHandle;

/// The appearance the product asks its native surfaces to use.
///
/// This is a *resolved* choice: the caller decides what `System` means and never passes
/// it down, so nothing below has to re-derive it and two windows cannot disagree.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppTheme {
    Light,
    Dark,
}

impl AppTheme {
    pub const fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }
}

/// The appearance the operating system is currently using for applications.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SystemAppearance {
    Light,
    Dark,
}

/// A failure to restyle a native surface.
///
/// None of these are fatal: a surface that could not be themed keeps the system
/// appearance, which is the documented fallback for every platform.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum NativeThemeError {
    /// The call has to run on the platform's UI thread and did not.
    #[error("{}", Self::WrongThread.as_str())]
    WrongThread,
    /// The window did not expose a handle the platform can use.
    #[error("{}", Self::WindowHandleUnavailable.as_str())]
    WindowHandleUnavailable,
    /// The window's handle belongs to a platform this call does not implement.
    #[error("{}", Self::UnsupportedWindowHandle.as_str())]
    UnsupportedWindowHandle,
    /// The platform call itself failed, or the API is absent on this system.
    #[error("{}", Self::NativeCallFailed.as_str())]
    NativeCallFailed,
}

impl NativeThemeError {
    pub const ALL: [Self; 4] = [
        Self::WrongThread,
        Self::WindowHandleUnavailable,
        Self::UnsupportedWindowHandle,
        Self::NativeCallFailed,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WrongThread => "native_theme_wrong_thread",
            Self::WindowHandleUnavailable => "native_theme_window_handle_unavailable",
            Self::UnsupportedWindowHandle => "native_theme_unsupported_window_handle",
            Self::NativeCallFailed => "native_theme_native_call_failed",
        }
    }
}

/// Applies the product's appearance choice to the process-wide native surfaces.
///
/// `theme` is the resolved choice; `None` means the product follows the operating
/// system, and each platform then restores its own system-driven appearance. This is
/// the startup entry point: it must run before the first product window exists, so a
/// context menu from the overlay does not depend on the settings window having been
/// created first.
///
/// On macOS this sets `NSApplication.appearance`, the one switch every native surface
/// in scope inherits.
pub fn apply_process_theme(theme: Option<AppTheme>) -> Result<(), NativeThemeError> {
    platform::apply_process(theme)
}

/// Applies the product's appearance choice to the native surfaces of this process.
///
/// `theme` is the resolved choice; `None` means the product follows the operating
/// system, and each platform then restores its own system-driven appearance.
///
/// `window` is accepted so the signature is portable, but macOS ignores it: one
/// process-wide appearance already covers every window and every panel a window presents,
/// and a per-window override would leave menus and alerts behind.
pub fn apply_theme(
    window: &impl HasWindowHandle,
    theme: Option<AppTheme>,
) -> Result<(), NativeThemeError> {
    platform::apply(window, theme)
}

/// Prepares the native surfaces that can only follow the *system* theme.
///
/// macOS needs nothing here: the application theme set through
/// [`apply_process_theme`] reaches the alerts, menus and panels as soon as it is
/// set, so there is no separate system-driven switch to prepare. The call is kept
/// so the startup sequence reads the same on every build, and it is a no-op.
///
/// Idempotent: repeated calls repeat nothing.
pub fn init_native_theme() -> Result<(), NativeThemeError> {
    platform::init()
}

mod platform {
    use super::{AppTheme, NativeThemeError, SystemAppearance};
    use objc2::{MainThreadMarker, rc::Retained};
    use objc2_app_kit::{NSAppearance, NSApplication};
    use objc2_foundation::NSString;
    use raw_window_handle::HasWindowHandle;

    /// The appearance names AppKit is asked for, written out rather than read from the
    /// `NSAppearanceName*` statics: those are `extern` statics, so touching them needs an
    /// `unsafe` block, while `appearanceNamed:` and `-isEqualToString:` both compare by
    /// string content. The values are the documented contents of those constants.
    const AQUA: &str = "NSAppearanceNameAqua";
    const DARK_NAMES: [&str; 3] = [
        "NSAppearanceNameDarkAqua",
        "NSAppearanceNameVibrantDark",
        "NSAppearanceNameAccessibilityHighContrastDarkAqua",
    ];

    /// The system appearance is queried only while the product follows it, and every
    /// other surface in this module is reached through `NSApplication.appearance`, so
    /// the query lives here next to the override it depends on.
    pub fn system_appearance() -> SystemAppearance {
        // `window_content_top_inset` sets the precedent for a query that cannot run off
        // the main thread: report the neutral value instead of panicking. Light is that
        // value, and it is also what AppKit reports when no dark appearance is named.
        let Some(marker) = MainThreadMarker::new() else {
            return SystemAppearance::Light;
        };
        let name = NSApplication::sharedApplication(marker)
            .effectiveAppearance()
            .name();
        if DARK_NAMES
            .iter()
            .any(|dark| name.isEqualToString(&NSString::from_str(dark)))
        {
            SystemAppearance::Dark
        } else {
            SystemAppearance::Light
        }
    }

    fn named(name: &str) -> Result<Retained<NSAppearance>, NativeThemeError> {
        let name = NSString::from_str(name);
        // The named appearances are as old as the appearance API itself (10.14), so a
        // missing name means the system is older than the product supports, not a race.
        NSAppearance::appearanceNamed(&name).ok_or(NativeThemeError::NativeCallFailed)
    }

    pub(super) fn apply_process(theme: Option<AppTheme>) -> Result<(), NativeThemeError> {
        let marker = MainThreadMarker::new().ok_or(NativeThemeError::WrongThread)?;
        let appearance = match theme {
            None => None,
            Some(AppTheme::Light) => Some(named(AQUA)?),
            Some(AppTheme::Dark) => Some(named(DARK_NAMES[0])?),
        };
        NSApplication::sharedApplication(marker).setAppearance(appearance.as_deref());
        Ok(())
    }

    pub(super) fn apply(
        _window: &impl HasWindowHandle,
        theme: Option<AppTheme>,
    ) -> Result<(), NativeThemeError> {
        // The window is deliberately unused: `NSApplication.appearance` is inherited by
        // every window, and by the alerts, menus and panels those windows present, which
        // is exactly the set the product wants themed. Setting it per window instead
        // would leave the alert, menu and panel rows on the system theme.
        apply_process(theme)
    }

    pub(super) fn init() -> Result<(), NativeThemeError> {
        // macOS applies the application theme to the system surfaces as soon as the
        // appearance above is set; there is no second switch to prepare.
        Ok(())
    }
}


pub use platform::system_appearance;

#[cfg(test)]
mod tests {
    use super::{AppTheme, NativeThemeError};

    #[test]
    fn error_codes_are_stable_and_unique() {
        let mut codes = NativeThemeError::ALL
            .iter()
            .map(|code| code.as_str())
            .collect::<Vec<_>>();
        assert!(codes.iter().all(|code| code.starts_with("native_theme_")));
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), NativeThemeError::ALL.len());
        assert_eq!(
            NativeThemeError::WrongThread.to_string(),
            "native_theme_wrong_thread"
        );
    }

    #[test]
    fn only_dark_reports_itself_as_dark() {
        assert!(AppTheme::Dark.is_dark());
        assert!(!AppTheme::Light.is_dark());
    }
}
