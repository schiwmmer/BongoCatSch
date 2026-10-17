//! The sounds the pomodoro surfaces play.
//!
//! They are short generated clips compiled into the binary rather than read
//! from a directory beside the executable. That is deliberate: the sounds are
//! decoration, so the worst thing that can happen to them is silence, and a
//! build that forgot to stage the asset would otherwise fail only on the
//! user's first finished session. `include_bytes` makes a missing asset a
//! compile error instead.
//!
//! Every clip plays through `NSSound` rather than the motion audio worker. The
//! worker owns the output device for the model's own audio and reclaims it as
//! soon as that stops; borrowing it here would mean a session finishing
//! cutting a model's voice off mid-clip.

/// The chime a finished focus session plays: a fifth apart, high note first,
/// with the low one ringing on.
///
/// It is generated rather than recorded, so the two notes are exactly in tune
/// and the file is small enough to ship inside the binary.
pub const POMODORO_COMPLETE_SOUND: &[u8] =
    include_bytes!("../../../resources/audio/pomodoro-complete.flac");

/// The confirmation a task marked done plays: a soft two-note rising figure.
///
/// Quieter and shorter than the session chime on purpose — completing a task
/// is a bookkeeping act the user performs themselves, not a countdown that
/// finished on its own.
pub const POMODORO_TASK_DONE_SOUND: &[u8] =
    include_bytes!("../../../resources/audio/task-done.wav");

/// The fanfare a level-up plays: a bright four-note arpeggio.
///
/// The longest and loudest of the three, because a level crossing is the one
/// event the user did not directly ask for.
pub const POMODORO_LEVEL_UP_SOUND: &[u8] = include_bytes!("../../../resources/audio/level-up.wav");

/// Play the chime for a finished focus session, or do nothing.
///
/// There is no result on purpose: a machine with no audio device, a busy
/// output device, or a denied audio permission all end the same way, and none
/// of them should stop the performance or the recorded completion that
/// accompany it.
pub fn play_pomodoro_complete_sound() {
    play(POMODORO_COMPLETE_SOUND, "flac");
}

/// Play the confirmation for a task marked done, or do nothing.
pub fn play_task_done_sound() {
    play(POMODORO_TASK_DONE_SOUND, "wav");
}

/// Play the fanfare for a level-up, or do nothing.
pub fn play_level_up_sound() {
    play(POMODORO_LEVEL_UP_SOUND, "wav");
}

/// Play one clip, or do nothing.
///
/// `NSSound` resolves a clip by the name it is staged under, so the extension
/// has to name the clip's real format — the session chime is FLAC and stages
/// under that name, the two generated WAVs under theirs.
#[cfg(target_os = "macos")]
fn play(bytes: &[u8], extension: &str) {
    use objc2_app_kit::NSSound;

    // `NSSound` plays a path rather than a buffer, so the clip is staged into
    // a temporary file. The name carries the process id so two BongoCats
    // starting at once do not write over each other's copy.
    let Ok(path) = stage(bytes, extension) else {
        return;
    };
    let ns_path = objc2_foundation::NSString::from_str(&path.to_string_lossy());
    // The sound retains what it needs from the file itself, so the file is
    // removed immediately afterwards rather than kept alive for the clip's
    // duration.
    if let Some(sound) = NSSound::soundNamed(&ns_path) {
        sound.play();
    }
    // The file is removed either way: a played sound has already been read,
    // and an unplayed one was never going to play. Leaving it behind would
    // fill the temporary directory with one file per event.
    let _ = std::fs::remove_file(&path);
}

/// Play one clip on a platform that has no pomodoro surface.
///
/// Windows has no pomodoro surface at all (ADR-0068 and ADR-0090), so there is
/// no session that could finish here. The functions exist so the call sites
/// do not need a platform branch.
#[cfg(target_os = "windows")]
fn play(_bytes: &[u8], _extension: &str) {}

#[cfg(target_os = "macos")]
fn stage(bytes: &[u8], extension: &str) -> std::io::Result<std::path::PathBuf> {
    use std::io::Write;

    let path = std::env::temp_dir().join(format!(
        "bongocat-pomodoro-{}.{}",
        std::process::id(),
        extension
    ));
    let mut file = std::fs::File::create(&path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_chime_is_a_flac_clip_of_a_plausible_length() {
        // A chime that is not there, or is a directory rather than a clip,
        // would compile and then be silent at runtime.
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

    #[test]
    fn the_task_done_confirmation_is_a_wav_clip_of_a_plausible_length() {
        assert!(
            super::POMODORO_TASK_DONE_SOUND.len() > 1_000,
            "the confirmation is suspiciously small: {} bytes",
            super::POMODORO_TASK_DONE_SOUND.len()
        );
        assert_eq!(
            &super::POMODORO_TASK_DONE_SOUND[..4],
            b"RIFF",
            "the confirmation must be a WAV clip: NSSound resolves it by that name"
        );
        assert_eq!(
            &super::POMODORO_TASK_DONE_SOUND[8..12],
            b"WAVE",
            "the confirmation must be a WAVE file"
        );
    }

    #[test]
    fn the_level_up_fanfare_is_a_wav_clip_longer_than_the_confirmation() {
        // The fanfare is the celebration; if the two ever end up the same
        // clip the user cannot tell the events apart, which is the whole
        // point of shipping two sounds.
        assert!(
            super::POMODORO_LEVEL_UP_SOUND.len() > 1_000,
            "the fanfare is suspiciously small: {} bytes",
            super::POMODORO_LEVEL_UP_SOUND.len()
        );
        assert_eq!(
            &super::POMODORO_LEVEL_UP_SOUND[..4],
            b"RIFF",
            "the fanfare must be a WAV clip"
        );
        assert!(
            super::POMODORO_LEVEL_UP_SOUND.len() > super::POMODORO_TASK_DONE_SOUND.len(),
            "the fanfare should outlast the confirmation"
        );
    }
}
