//! The version of the operating system this build is running on.
//!
//! The product knows its own version at compile time, but almost nothing about
//! a bug report can be acted on without the system it came from: a rendering
//! difference and a TCC decision both depend on the exact macOS build, and
//! neither can be inferred from the app's own version. This module is the one
//! place that asks the operating system.
//!
//! macOS answers both questions in its own notation, which is not rewritten
//! into any other:
//!
//! | | macOS |
//! | --- | --- |
//! | [`version`](OperatingSystemVersion::version) | `kern.osproductversion` — "15.6" |
//! | [`build`](OperatingSystemVersion::build) | `kern.osversion` — "24G90" |
//!
//! The version is read through `sysctlbyname` rather than `NSProcessInfo` on
//! purpose: `operatingSystemVersion` answers with the compatibility version a
//! process linked against an older SDK sees, while the kernel's own `kern.os*`
//! keys always name the system the process is running on. The build is read
//! from `kern.osversion` rather than the older `kern.osbuildversion`: both
//! carry the same build identifier where both exist, and `kern.osbuildversion`
//! is absent on current macOS releases, so the key that is still there is the
//! one to ask.

/// The running operating system's version, in the notation its own API uses.
///
/// Both fields are kept separate because they answer different questions: the
/// version is the one a release note and a help article name, while the build
/// is the only form that pins one exact installation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperatingSystemVersion {
    /// The version the platform itself reports: "15.6" on macOS.
    pub version: String,
    /// The build identity: "24G90" on macOS.
    pub build: String,
}

/// The version of the operating system this process is running on.
///
/// `None` when the platform cannot report one. That is not a failure the caller
/// has to handle: a system that does not name its own version still produces a
/// usable bug report, so the caller reports the version it does have and leaves
/// this out rather than inventing a value. A version that is present is never
/// empty and never carries a terminator — the platform's own separators decide
/// its shape.
pub fn operating_system_version() -> Option<OperatingSystemVersion> {
    platform::version()
}

mod platform {
    use super::OperatingSystemVersion;
    use std::{ffi::CString, ptr};

    /// The two kernel keys the marketing version and the build identifier live
    /// under. They are `char` sysctls, so each one is a NUL-terminated string.
    const PRODUCT_VERSION_KEY: &str = "kern.osproductversion";
    const BUILD_VERSION_KEY: &str = "kern.osversion";

    pub(super) fn version() -> Option<OperatingSystemVersion> {
        Some(OperatingSystemVersion {
            version: sysctl_string(PRODUCT_VERSION_KEY)?,
            build: sysctl_string(BUILD_VERSION_KEY)?,
        })
    }

    /// Read one NUL-terminated `char` sysctl as a Rust `String`.
    ///
    /// `sysctlbyname` is read in its documented two-call form: a call with a null
    /// value pointer reports the buffer size it needs, and a second call fills
    /// that buffer. Reporting no value is the answer for every way the two calls
    /// can disagree — a key the running system does not have, a size that grows
    /// between the calls, a value that is not valid UTF-8 — because a truncated
    /// or unterminated build identifier is worse than a missing one.
    ///
    /// The key is copied into a [`CString`] rather than passed as a `&str`
    /// pointer: `sysctlbyname` reads a C string, and a `&str` is not
    /// NUL-terminated, so the name it would read runs into whatever follows it.
    fn sysctl_string(key: &str) -> Option<String> {
        let key = CString::new(key).ok()?;
        let mut size = 0;
        // SAFETY: `key` is a NUL-terminated name that outlives the call, and a
        // null value pointer with a null new value asks `sysctlbyname` to write
        // the required buffer size through `size` and change nothing else.
        let queried = unsafe {
            libc::sysctlbyname(
                key.as_ptr(),
                ptr::null_mut(),
                ptr::from_mut(&mut size),
                ptr::null_mut(),
                0,
            )
        };
        if queried != 0 || size == 0 {
            return None;
        }
        let mut buffer = vec![0_u8; size];
        // SAFETY: `buffer` is a live allocation of the `size` writable bytes the
        // same key just asked for, and `size` is a live `size_t` the call writes
        // the byte count it stored through. The value pointer is null, so the
        // call cannot write to `key` or anywhere else. A `char` sysctl stores
        // bytes rather than `c_char`s, so the buffer is read back as bytes.
        let read = unsafe {
            libc::sysctlbyname(
                key.as_ptr(),
                buffer.as_mut_ptr().cast(),
                ptr::from_mut(&mut size),
                ptr::null_mut(),
                0,
            )
        };
        if read != 0 {
            return None;
        }
        // The stored byte count covers the NUL terminator. A shorter answer than
        // the first call promised is a value that changed underneath both
        // calls, and is reported as no value for the same reason.
        let stored = size.min(buffer.len());
        let bytes = &buffer[..stored];
        let end = bytes
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(bytes.len());
        String::from_utf8(bytes[..end].to_vec()).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::operating_system_version;

    /// macOS has to answer, not just the host this test runs on: a system that
    /// names no version produces a report that is missing the one field a triage
    /// reads first, and nothing else would notice. The `None` return is for a
    /// system that answers unreliably, not for a platform that has no way to
    /// answer — and this module only compiles where there is one.
    #[test]
    fn the_running_system_reports_a_complete_version() {
        let version =
            operating_system_version().expect("macOS reports a version, and this build is macOS");
        assert!(!version.version.is_empty());
        assert!(!version.build.is_empty());
        for value in [&version.version, &version.build] {
            assert!(!value.contains('\0'), "a terminator leaked into {value}");
            assert!(
                value
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric()
                        || matches!(character, '.' | '_' | '-')),
                "{value} is not a plain version string"
            );
        }
    }

    /// The two fields answer different questions, so a value that is present in
    /// both would mean one of them is a restatement of the other. A system whose
    /// build happens to equal its version is not one to trust either way.
    #[test]
    fn the_version_and_the_build_are_different_facts() {
        if let Some(version) = operating_system_version() {
            assert_ne!(version.version, version.build);
        }
    }
}
