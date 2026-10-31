//! Turning an expression off, as opposed to replacing it.
//!
//! The whole point of clearing is the face the model returns to, so these tests
//! read a model parameter rather than a render snapshot: breathing and blinking
//! move most of the model on every frame, so only a parameter an expression is
//! the sole writer of can say whether the expression is still in effect.

use super::*;

fn mouth_opening(renderer: &RuntimeRenderer) -> f32 {
    renderer
        .active
        .as_ref()
        .expect("active model")
        .model
        .parameter_value_by_id("ParamMouthOpenY")
        .expect("parameter value")
        .expect("supported parameter")
}

fn expression_clip() -> ExpressionClip {
    ExpressionClip::from_slice(
        br#"{
          "Type":"Live2D Expression","FadeInTime":0.0,"FadeOutTime":0.5,
          "Parameters":[
            {"Id":"ParamMouthOpenY","Value":0.8,"Blend":"Overwrite"}
          ]
        }"#,
    )
    .expect("expression")
}

/// Clearing starts the same fade a replacement does and adds nothing in its
/// place, so the model lands back on the face it wears with no expression applied.
#[test]
fn clearing_the_expression_stack_returns_the_model_to_its_default_face() {
    let (bootstrap, _consumer) = RuntimeRenderer::channel();
    let mut renderer = RuntimeRenderer::start(bootstrap);
    let token = renderer
        .prepare(1, &preset_model("standard"), ModelInputSnapshot::default())
        .expect("prepare model");
    assert!(renderer.commit(token));

    renderer
        .evaluate(ModelInputSnapshot::default(), Duration::ZERO)
        .expect("baseline frame");
    let baseline = mouth_opening(&renderer);

    renderer
        .active
        .as_mut()
        .expect("active model")
        .expressions
        .push(ExpressionPlayback {
            clip: expression_clip(),
            started_at: Duration::ZERO,
            fade_in_completed: false,
            auto_recover_at: None,
            fade_out_started_at: None,
        });
    renderer
        .evaluate(ModelInputSnapshot::default(), Duration::from_secs(1))
        .expect("expression frame");
    assert!(
        (mouth_opening(&renderer) - 0.8).abs() < 0.0001,
        "the expression is in effect: {}",
        mouth_opening(&renderer)
    );

    assert!(
        renderer.clear_expression(Duration::from_secs(1)),
        "there was an expression to turn off"
    );
    // Halfway through the clip's own fade the face is on its way back rather than
    // snapping, which is what makes the behaviour read as the model relaxing
    // instead of the picture glitching.
    renderer
        .evaluate(ModelInputSnapshot::default(), Duration::from_millis(1250))
        .expect("mid-fade frame");
    let mid_fade = mouth_opening(&renderer);
    assert!(
        mid_fade > baseline && mid_fade < 0.8,
        "the cleared expression is still fading out: {mid_fade}"
    );

    renderer
        .evaluate(ModelInputSnapshot::default(), Duration::from_secs(3))
        .expect("settled frame");
    assert!(
        renderer
            .active
            .as_ref()
            .expect("active model")
            .expressions
            .is_empty(),
        "a finished fade leaves nothing to apply"
    );
    assert!(
        (mouth_opening(&renderer) - baseline).abs() < 0.0001,
        "the model wears its own default face again: {} vs {baseline}",
        mouth_opening(&renderer)
    );
}

/// Clearing is idempotent, because a repeat trigger can arrive while the previous
/// fade is still running and there is nothing left for the second one to close.
#[test]
fn clearing_twice_is_the_same_as_clearing_once() {
    let (bootstrap, _consumer) = RuntimeRenderer::channel();
    let mut renderer = RuntimeRenderer::start(bootstrap);
    let token = renderer
        .prepare(1, &preset_model("standard"), ModelInputSnapshot::default())
        .expect("prepare model");
    assert!(renderer.commit(token));
    renderer
        .evaluate(ModelInputSnapshot::default(), Duration::ZERO)
        .expect("baseline frame");
    let baseline = mouth_opening(&renderer);

    renderer
        .active
        .as_mut()
        .expect("active model")
        .expressions
        .push(ExpressionPlayback {
            clip: expression_clip(),
            started_at: Duration::ZERO,
            fade_in_completed: false,
            auto_recover_at: None,
            fade_out_started_at: None,
        });
    assert!(renderer.clear_expression(Duration::ZERO));
    assert!(
        renderer.clear_expression(Duration::from_millis(100)),
        "the layer is still fading, so it is still there to close"
    );
    renderer
        .evaluate(ModelInputSnapshot::default(), Duration::from_secs(2))
        .expect("settled frame");
    assert!(
        !renderer.clear_expression(Duration::from_secs(2)),
        "nothing is left to turn off"
    );
    assert!((mouth_opening(&renderer) - baseline).abs() < 0.0001);
}

/// A renderer with no model has no expression to turn off, and says so rather
/// than reporting a clear that changed nothing.
#[test]
fn clearing_without_an_active_model_reports_nothing_to_close() {
    let (bootstrap, _consumer) = RuntimeRenderer::channel();
    let mut renderer = RuntimeRenderer::start(bootstrap);
    assert!(!renderer.clear_expression(Duration::ZERO));
}

fn right_eye_opening(renderer: &RuntimeRenderer) -> f32 {
    renderer
        .active
        .as_ref()
        .expect("active model")
        .model
        .parameter_value_by_id("ParamEyeROpen")
        .expect("parameter value")
        .expect("supported parameter")
}

/// A one-shot expression (the shipped `wink`) sets its own recovery time: after
/// its fade-in and hold it fades out on its own and the model wears its own
/// face again. This is the report: a wink must not stay frozen on screen.
#[test]
fn one_shot_expression_recovers_to_default_after_its_hold() {
    let (bootstrap, _consumer) = RuntimeRenderer::channel();
    let mut renderer = RuntimeRenderer::start(bootstrap);
    let token = renderer
        .prepare(1, &preset_model("standard"), ModelInputSnapshot::default())
        .expect("prepare model");
    assert!(renderer.commit(token));
    // The baseline is read inside the blink-open window (past the 180ms closed
    // phase), so the automatic blink is not the writer of the eye parameter.
    let baseline_at = Duration::from_millis(500);
    renderer
        .evaluate(ModelInputSnapshot::default(), baseline_at)
        .expect("baseline frame");
    let baseline = right_eye_opening(&renderer);

    let expression = crate::ExpressionId::new("wink.exp3.json").expect("expression id");
    assert_eq!(
        renderer
            .set_expression(&expression, baseline_at)
            .expect("set wink"),
        ExpressionSetOutcome::AutoRecover,
        "the wink declares an auto recovery time"
    );
    // Still inside the fade-in + hold (0.3 + 1.5s after `baseline_at`): the
    // eye is closed by the expression's Add -1.0 over the open baseline.
    renderer
        .evaluate(ModelInputSnapshot::default(), Duration::from_millis(1300))
        .expect("held frame");
    let held = right_eye_opening(&renderer);
    assert!(
        (held - (baseline - 1.0)).abs() < 0.0001,
        "the wink holds the eye closed: {held} vs {baseline}"
    );

    // Past the recovery point (2.3s) but mid fade-out: the eye is on its way
    // back rather than snapping, which is what makes it read as a blink.
    renderer
        .evaluate(ModelInputSnapshot::default(), Duration::from_millis(2600))
        .expect("recovering frame");
    let mid = right_eye_opening(&renderer);
    assert!(
        mid > baseline - 1.0 && mid < baseline,
        "the wink is fading back out: {mid} vs {baseline}"
    );

    // The fade (0.5s) is done: nothing is left to apply and the model is back.
    renderer
        .evaluate(ModelInputSnapshot::default(), Duration::from_millis(3000))
        .expect("settled frame");
    assert!(
        renderer
            .active
            .as_ref()
            .expect("active model")
            .expressions
            .is_empty(),
        "a finished auto recovery leaves nothing to apply"
    );
    assert!(
        (right_eye_opening(&renderer) - baseline).abs() < 0.0001,
        "the model wears its own face again: {} vs {baseline}",
        right_eye_opening(&renderer)
    );
}

/// A persistent expression (no `AutoRecoverTime`) does not recover on its own:
/// it stays full weight until it is replaced or cleared.
#[test]
fn persistent_expression_does_not_recover_on_its_own() {
    let (bootstrap, _consumer) = RuntimeRenderer::channel();
    let mut renderer = RuntimeRenderer::start(bootstrap);
    let token = renderer
        .prepare(1, &preset_model("standard"), ModelInputSnapshot::default())
        .expect("prepare model");
    assert!(renderer.commit(token));
    renderer
        .evaluate(ModelInputSnapshot::default(), Duration::ZERO)
        .expect("baseline frame");

    renderer
        .active
        .as_mut()
        .expect("active model")
        .expressions
        .push(ExpressionPlayback {
            clip: expression_clip(),
            started_at: Duration::ZERO,
            fade_in_completed: false,
            fade_out_started_at: None,
            auto_recover_at: None,
        });
    // Long after any plausible hold: the expression is still full weight.
    renderer
        .evaluate(ModelInputSnapshot::default(), Duration::from_secs(60))
        .expect("held frame");
    assert!(
        (mouth_opening(&renderer) - 0.8).abs() < 0.0001,
        "a persistent expression holds: {}",
        mouth_opening(&renderer)
    );
    assert_eq!(
        renderer
            .active
            .as_ref()
            .expect("active model")
            .expressions
            .len(),
        1,
        "the persistent layer is still there"
    );
}
