//! Shutdown aggregation across the runtime and the worker services.

use super::*;

#[test]
fn a_clean_shutdown_returns_the_stopped_runtime() {
    combine_shutdown_results::<()>(Ok(()), Ok(()), Ok(())).expect("a clean shutdown");
}

#[test]
fn a_single_service_failure_keeps_its_own_error_code() {
    let runtime = combine_shutdown_results::<()>(Err(ShutdownError::TimedOut), Ok(()), Ok(()))
        .expect_err("a runtime failure must be returned");
    assert!(matches!(
        runtime,
        ApplicationError::Shutdown(ShutdownError::TimedOut)
    ));
    assert_eq!(runtime.stable_code(), "runtime_shutdown_failed");

    let audio = combine_shutdown_results(Ok(()), Err(MotionAudioShutdownError::TimedOut), Ok(()))
        .expect_err("an audio failure must be returned");
    assert!(matches!(audio, ApplicationError::MotionAudioShutdown(_)));
    assert_eq!(audio.stable_code(), "audio_shutdown_failed");

    let pomodoro = combine_shutdown_results(Ok(()), Ok(()), Err(PomodoroShutdownError::TimedOut))
        .expect_err("a pomodoro failure must be returned");
    assert!(matches!(
        pomodoro,
        ApplicationError::PomodoroShutdown(PomodoroShutdownError::TimedOut)
    ));
    assert_eq!(pomodoro.stable_code(), "pomodoro_shutdown_failed");
}

#[test]
fn two_failures_are_reported_together_and_named() {
    let dual = combine_shutdown_results::<()>(
        Err(ShutdownError::WorkerPanicked),
        Err(MotionAudioShutdownError::TimedOut),
        Ok(()),
    )
    .expect_err("both service failures must be retained");
    match dual {
        ApplicationError::ShutdownAggregate(error) => {
            assert_eq!(
                error
                    .failures
                    .iter()
                    .map(|failure| failure.service)
                    .collect::<Vec<_>>(),
                vec!["runtime", "audio"]
            );
            assert_eq!(
                error.to_string(),
                "runtime: runtime worker panicked; audio: motion audio shutdown timed out"
            );
        }
        other => panic!("unexpected shutdown error: {other}"),
    }

    // Two workers failing with the runtime healthy is the case the single-error
    // variants cannot express.
    let workers = combine_shutdown_results::<()>(
        Ok(()),
        Err(MotionAudioShutdownError::WorkerPanicked),
        Err(PomodoroShutdownError::TimedOut),
    )
    .expect_err("both worker failures must be retained");
    match workers {
        ApplicationError::ShutdownAggregate(error) => {
            assert_eq!(
                error
                    .failures
                    .iter()
                    .map(|failure| failure.service)
                    .collect::<Vec<_>>(),
                vec!["audio", "pomodoro"]
            );
        }
        other => panic!("unexpected shutdown error: {other}"),
    }
}

#[test]
fn every_failing_service_is_named() {
    let all = combine_shutdown_results::<()>(
        Err(ShutdownError::WorkerPanicked),
        Err(MotionAudioShutdownError::TimedOut),
        Err(PomodoroShutdownError::WorkerPanicked),
    )
    .expect_err("every failure must be retained");
    match all {
        ApplicationError::ShutdownAggregate(error) => {
            assert_eq!(
                error
                    .failures
                    .iter()
                    .map(|failure| failure.service)
                    .collect::<Vec<_>>(),
                vec!["runtime", "audio", "pomodoro"]
            );
        }
        other => panic!("unexpected shutdown error: {other}"),
    }
}
