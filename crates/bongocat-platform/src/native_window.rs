//! The failure vocabulary of the window-level native calls.
//!
//! The product keeps one pre-rendered window per purpose (settings, update) and swaps
//! its visibility instead of destroying it, so the hide/show pair is implemented once
//! and reports the same failures. The variants stay in one shared type rather than one
//! per call: `bongocat-ui` calls the pair through a single code path and only maps the
//! error to a stable settings error code.
//!
//! `WrongThread` is produced by the macOS calls, which are AppKit main-thread calls, and
//! `DockVisibilityUpdateFailed` by the one that owns the Dock.

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum NativeWindowError {
    #[error("the native window handle is unavailable")]
    HandleUnavailable,
    #[error("the native window handle is not a supported platform window")]
    UnsupportedHandle,
    #[error("the native window call was made off the owner thread")]
    WrongThread,
    #[error("the Dock visibility did not update")]
    DockVisibilityUpdateFailed,
}
