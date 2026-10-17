//! Locating files the app ships beside its own executable.
//!
//! Three layouts, one answer. A macOS app bundle keeps them in
//! `Contents/Resources`, a Windows install keeps them in a `resources`
//! directory beside the executable, and a build-tree run keeps them in the
//! repository's own `resources` directory. The candidates are tried in that
//! order and the first that exists wins, so a bundle never silently falls back
//! to the repository it was built from.
//!
//! Callers pass a repository-relative path (`pomodoro/tomato-filled.png`), the
//! same spelling the manifest and the bundle layout use.

use std::path::PathBuf;

/// Resolve a bundled resource by its repository-relative path.
///
/// Falls back to the repository-relative spelling when nothing exists yet —
/// the caller gets a path it can report rather than an empty guess.
pub fn bundled_resource(relative: &str) -> PathBuf {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            // macOS app bundle: <bundle>/Contents/MacOS/<exe> -> <bundle>/Contents/Resources.
            candidates.push(directory.join("../Resources").join(relative));
            // Windows install and any bare binary: beside the executable.
            candidates.push(directory.join(relative));
        }
    }
    // Build-tree run: the repository root, two levels up from this crate.
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../")
            .join(relative),
    );

    candidates
        .into_iter()
        .find(|candidate| candidate.exists())
        .unwrap_or_else(|| PathBuf::from(relative))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_repository_resource_resolves_to_an_existing_file() {
        // The tomato icons ship in the repository's resources directory, so a
        // build-tree test run must find them through the manifest candidate.
        let path = super::bundled_resource("pomodoro/tomato-filled.png");
        assert!(
            path.exists(),
            "the filled tomato icon should resolve in a build-tree run: {}",
            path.display()
        );
        assert!(
            path.ends_with("pomodoro/tomato-filled.png"),
            "the resolved path keeps the requested spelling: {}",
            path.display()
        );
    }

    #[test]
    fn a_missing_resource_falls_back_to_the_requested_spelling() {
        // Nothing to find: the answer is still a path the caller can show,
        // never an empty string or a panic.
        let path = super::bundled_resource("pomodoro/does-not-exist.png");
        assert_eq!(
            path,
            std::path::PathBuf::from("pomodoro/does-not-exist.png")
        );
    }
}
