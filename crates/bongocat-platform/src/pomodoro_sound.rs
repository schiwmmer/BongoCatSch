//! The chime a finished focus session plays.
//!
//! It is a short two-tone clip compiled into the binary rather than read from a
//! directory beside the executable. That is deliberate: the sound is decoration,
//! so the worst thing that can happen to it is silence, and a build that forgot to
//! stage the asset would otherwise fail only on the user's first finished
//! session. `include_bytes` makes a missing asset a compile error instead.
//!
//! The clip plays through `NSSound` rather than the motion audio worker. The
//! worker owns the output device for the model's own audio and reclaims it as
//! soon as that stops; borrowing it here would mean a session finishing cutting
//! a model's voice off mid-clip.

/// The chime: a fifth apart, high note first, with the low one ringing on.
///
/// It is generated rather than recorded, so the two notes are exactly in tune and
/// the file is small enough to ship inside the binary.
pub const POMODORO_COMPLETE_SOUND: &[u8] =
    include_bytes!("../../../resources/audio/pomodoro-complete.flac");

/// Play the chime, or do nothing.
///
/// There is no result on purpose: a machine with no audio device, a busy output
/// device, or a denied audio permission all end the same way, and none of them
/// should stop the performance or the recorded completion that accompany it.
#[cfg(target_os = "macos")]
pub fn play_pomodoro_complete_sound() {
    use objc2_app_kit::NSSound;

    // `NSSound` plays a path rather than a buffer, so the clip is staged into a
    // temporary file. The name carries the process id so two BongoCats starting
    // at once do not write over each other's copy.
    let Ok(path) = stage(POMODORO_COMPLETE_SOUND) else {
        return;
    };
    let ns_path = objc2_foundation::NSString::from_str(&path.to_string_lossy());
    // The sound retains what it needs from the file itself, so the file is
    // removed immediately afterwards rather than kept alive for the clip's
    // duration.
    if let Some(sound) = NSSound::soundNamed(&ns_path) {
        sound.play();
    }
    // The file is removed either way: a played sound has already been read, and
    // an unplayed one was never going to play. Leaving it behind would fill the
    // temporary directory with one file per finished session.
    let _ = std::fs::remove_file(&path);
}

#[cfg(target_os = "macos")]
fn stage(bytes: &[u8]) -> std::io::Result<std::path::PathBuf> {
    use std::io::Write;

    let path = std::env::temp_dir().join(format!("bongocat-pomodoro-{}.flac", std::process::id()));
    let mut file = std::fs::File::create(&path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(path)
}

#[cfg(target_os = "windows")]
pub fn play_pomodoro_complete_sound() {
    // Windows has no pomodoro surface at all (ADR-0068 and ADR-0090), so there is
    // no session that could finish here. The function exists so the call site
    // does not need a platform branch.
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_chime_is_a_flac_clip_of_a_plausible_length() {
        // A chime that is not there, or is a directory rather than a clip, would
        // compile and then be silent at runtime.
        assert!(
            super::POMODORO_COMPLETE_SOUND.len() > 1_000,
            "the chime is suspiciously small: {} bytes",
            super::POMODORO_COMPLETE_SOUND.len()
        );
        assert_eq!(
            &super::POMODORO_COMPLETE_SOUND[..4],
            b"fLaC",
            "the chime must be a FLAC clip: the audio path decodes FLAC only"
        );
    }
}
