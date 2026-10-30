//! Regression coverage for the behaviour-palette bug: preset expressions that
//! drive the eye-open parameters and preset motions that drive the angle
//! parameters appeared to do nothing because two later steps in the same frame
//! overwrote them.
//!
//! 1. The automatic eye blink used to `set_normalized_parameter` the eye-open
//!    parameters on every frame, cancelling a squint / short-eye expression.
//! 2. The model input overwrote the angle parameters with the pointer position
//!    on every frame, cancelling a nod / headshake motion. A guard that only
//!    fired when the pointer was exactly at zero never fired in practice, so the
//!    motion has to be protected by what it declares, not by where the cursor is.

use super::*;
use std::path::PathBuf;

fn resource(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../resources/models/standard")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn squint_clip() -> ExpressionClip {
    ExpressionClip::from_slice(
        br#"{"Type":"Live2D Expression","FadeInTime":0.0,"FadeOutTime":0.0,"Parameters":[
          {"Id":"ParamEyeLOpen","Value":0.321,"Blend":"Multiply"},
          {"Id":"ParamEyeROpen","Value":0.313,"Blend":"Multiply"}]}"#,
    )
    .expect("squint")
}

#[test]
fn expression_survives_automatic_blink_in_both_windows() {
    // t=0 sits in the blink closed window (blink=-1.0), t=1s in the open
    // window (blink=0.0). The expression's Multiply 0.321 must survive BOTH,
    // otherwise the squint is cancelled by the blink on every frame.
    for now in [Duration::ZERO, Duration::from_secs(1)] {
        let (bootstrap, _consumer) = RuntimeRenderer::channel();
        let mut renderer = RuntimeRenderer::start(bootstrap);
        let token = renderer
            .prepare(1, &preset_model("standard"), ModelInputSnapshot::default())
            .expect("prepare model");
        assert!(renderer.commit(token));
        renderer
            .active
            .as_mut()
            .expect("active model")
            .expressions
            .push(ExpressionPlayback {
                clip: squint_clip(),
                started_at: Duration::ZERO,
                fade_in_completed: false,
                fade_out_started_at: None,
            });
        renderer
            .evaluate(ModelInputSnapshot::default(), now)
            .expect("evaluate frame");
        let open = renderer
            .active
            .as_ref()
            .expect("active model")
            .model
            .parameter_value_by_id("ParamEyeLOpen")
            .expect("value")
            .expect("supported");
        assert!(
            (open - 0.321).abs() < 0.01,
            "squint 0.321 must survive the blink at {now:?}, got {open}"
        );
    }
}

#[test]
fn preset_motion_survives_neutral_input() {
    // The nod motion drives ParamAngleY (the vertical axis). With a neutral
    // (centre) input the model input must NOT reset it to 0; the motion's value
    // plus the breath contribution is what survives.
    let angle_y = evaluate_nod(None, Duration::from_millis(500));
    // The nod curve is 15.0 at 0.5s; the breath adds ~3.11 at the same time.
    // The key assertion: the motion contribution is retained (not reset to the
    // neutral 0 the input used to force).
    assert!(
        (angle_y - 18.105).abs() < 0.1,
        "nod motion must retain its angle under a neutral input, got {angle_y}"
    );
}

/// The regression that actually reaches a user.
///
/// The pointer is only exactly zero at the centre of the screen, so a guard
/// keyed on "pointer is at rest" never fires in practice and the cursor reset
/// the angle on every frame. The motion has to win over any pointer position.
#[test]
fn preset_motion_survives_a_moving_pointer() {
    for (pointer_x, pointer_y) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.63, -0.41)] {
        let angle_y = evaluate_nod(Some((pointer_x, pointer_y)), Duration::from_millis(500));
        assert!(
            (angle_y - 18.105).abs() < 0.1,
            "nod must keep its angle at pointer ({pointer_x}, {pointer_y}), got {angle_y}"
        );
    }
}

/// Plays the shipped `nod` clip and returns `ParamAngleY` after one frame.
fn evaluate_nod(pointer: Option<(f32, f32)>, now: Duration) -> f32 {
    let json = resource("nod.motion3.json");
    let clip = MotionClip::from_slice(json.as_bytes(), 0.0, 0.0).expect("nod");
    let (bootstrap, _consumer) = RuntimeRenderer::channel();
    let mut renderer = RuntimeRenderer::start(bootstrap);
    let token = renderer
        .prepare(1, &preset_model("standard"), ModelInputSnapshot::default())
        .expect("prepare model");
    assert!(renderer.commit(token));
    renderer
        .active
        .as_mut()
        .expect("active")
        .motions
        .push(MotionPlayback {
            motion: MotionId::new("CAT_motion", 0).expect("motion id"),
            clip,
            looping: false,
            started_at: Duration::ZERO,
            completed: false,
            fade_out_started_at: None,
            last_event_elapsed: None,
        });
    let input = ModelInputSnapshot {
        pointer_x: pointer.map_or(0.0, |(x, _)| x),
        pointer_y: pointer.map_or(0.0, |(_, y)| y),
        ..ModelInputSnapshot::default()
    };
    renderer.evaluate(input, now).expect("motion frame");
    renderer
        .active
        .as_ref()
        .expect("active")
        .model
        .parameter_value_by_id("ParamAngleY")
        .expect("value")
        .expect("supported")
}
