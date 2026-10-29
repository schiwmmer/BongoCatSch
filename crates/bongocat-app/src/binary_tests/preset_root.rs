//! Resolving the model packages this build ships.

use super::*;

#[test]
fn bundled_preset_models_resolve_from_contents_resources() {
    assert_eq!(
        bundled_preset_root(Path::new(
            "/Applications/BongoCat.app/Contents/MacOS/bongocat-app"
        )),
        Some(PathBuf::from(
            "/Applications/BongoCat.app/Contents/Resources/models"
        ))
    );
    assert_eq!(
        bundled_preset_root(Path::new("/tmp/target/release/bongocat-app")),
        None
    );
}
