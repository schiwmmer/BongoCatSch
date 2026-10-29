//! Handing a link to the operating system.
//!
//! Every "open this address" control in the product goes through here: the
//! About page's project and feedback rows, the update window's release-page
//! button, and every link in the release notes. They share this module because
//! they share one hazard, which is invisible until it kills the process.
//!
//! # Why the hand-off cannot happen inside the callback
//!
//! The `open` call is synchronous and blocks until the browser it started has
//! taken the URL, so running it inside a GPUI button or link callback freezes
//! the window while it cannot paint.
//!
//! # The rule
//!
//! The blocking platform call runs on a background executor, so the thread that
//! owns the borrow never blocks. Only the outcome comes back.
//!
//! `opener::reveal` spawns its own worker thread, and the clipboard writes do
//! not block; the directory launches happen on the settings worker, off the UI
//! thread entirely.

use bongocat_platform::ExternalUrlOpenError;
use gpui_kit::{BackgroundExecutor, Task};

use crate::{SettingsError, SettingsErrorCode};

/// Hand an HTTPS URL to the operating system, off the GUI thread.
///
/// The caller supplies the executor because a view callback has one and a render
/// helper does not, and because the returned [`Task`] is what lets the About page
/// report a failure back into its own view.
///
/// The platform adapter re-validates the scheme, so a URL arriving from a parsed
/// manifest is refused exactly like a product-owned constant; this decides
/// nothing about what may be opened.
pub(crate) fn open(url: String, executor: &BackgroundExecutor) -> Task<Result<(), SettingsError>> {
    open_with(url, executor, bongocat_platform::open_external_url)
}

/// [`Self::open`] with the launch injected, so the scheduling this module exists
/// to guarantee can be asserted without a test run opening a browser.
pub(crate) fn open_with(
    url: String,
    executor: &BackgroundExecutor,
    launch: impl FnOnce(&str) -> Result<(), ExternalUrlOpenError> + Send + 'static,
) -> Task<Result<(), SettingsError>> {
    executor.spawn(async move {
        launch(&url).map_err(|_| SettingsError::new(SettingsErrorCode::ExternalLinkOpenFailed))
    })
}
