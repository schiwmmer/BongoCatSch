#![allow(clippy::print_stdout)]

#[path = "src/product_icon_contract.rs"]
mod product_icon_contract;

use product_icon_contract::{validate_icns, validate_png};
use std::path::{Path, PathBuf};


fn main() {
    println!("cargo::rerun-if-changed=src/product_icon_contract.rs");
    link_swift_runtime();

    let manifest_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo must provide CARGO_MANIFEST_DIR"),
    );
    let resources_dir = manifest_dir.join("../../resources/icons");
    validate_icon(
        &resources_dir.join("logo-macos.icns"),
        "macOS product icon",
        validate_icns,
    );
    validate_icon(
        &resources_dir.join("tray-macos.png"),
        "macOS status icon",
        validate_png,
    );

}

/// Puts the Swift runtime on the loader path of this package's binaries.
///
/// The macOS permission flow links a Swift static library, and every binary that links it needs
/// `@rpath/libswift_Concurrency.dylib` and friends at load time. `permission-flow`'s own build
/// script emits the same flag, but a library dependency's `rustc-link-arg` does not reach the
/// binary that finally links, so each package whose binaries link it has to add it — see
/// `tools/tests/test_swift_runtime_rpath.py`. Without it the process aborts before `main`:
///
///   dyld: Library not loaded: @rpath/libswift_Concurrency.dylib
///   Reason: no LC_RPATH's found
fn link_swift_runtime() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo::rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    }
}

fn validate_icon(path: &Path, description: &str, validate: fn(&[u8]) -> Result<(), &'static str>) {
    println!("cargo::rerun-if-changed={}", path.display());
    let bytes = std::fs::read(path).unwrap_or_else(|error| {
        panic!(
            "could not read {description} at {}: {error}",
            path.display()
        )
    });
    validate(&bytes)
        .unwrap_or_else(|error| panic!("invalid {description} at {}: {error}", path.display()));
}
