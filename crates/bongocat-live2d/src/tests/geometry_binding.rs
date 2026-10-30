//! Every behaviour a shipped model advertises has to draw something.
//!
//! A parameter can be written successfully and still draw nothing: the value
//! lands in the model, but no vertex references it, so a palette chip that plays
//! that behaviour looks like a dead button. Those two facts are independent, so
//! "the click did nothing" has to be told apart from "the click never ran" by
//! measuring the model rather than the click.
//!
//! The standard model's moc3 rigs exactly two things: the eye-open parameters,
//! and the head angles around the horizontal and vertical axes. It has no mouth,
//! brow, cheek or roll rig at all, and it swaps facial artwork through the
//! `Param4` / `Param5` texture parameters instead of deforming a face. That is
//! why the behaviours built on mouth and brow parameters are not declared by
//! this model — see `cat.model3.json` — rather than declared and inert.

use super::*;

fn standard_model() -> Live2dModel {
    use bongocat_model::{ModelId, ModelPackageLimits, PresetModelCatalog};
    use std::path::Path;

    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repository root");
    let committed = PresetModelCatalog::open(
        repository_root.join("resources/models"),
        ModelPackageLimits::default(),
    )
    .expect("preset catalog")
    .load(&ModelId::parse("standard").expect("model id"))
    .expect("preset model");
    Live2dModel::load(&committed).expect("Live2D model")
}

/// The behaviours the standard model declares, with the parameters each writes.
const DECLARED_BEHAVIOURS: &[(&str, &[&str])] = &[
    ("exp_1", &["ParamEyeLOpen", "ParamEyeROpen"]),
    ("exp_2", &["ParamEyeLOpen"]),
    ("wink", &["ParamEyeROpen"]),
    (
        "focus",
        &[
            "ParamEyeLOpen",
            "ParamEyeROpen",
            "ParamBrowLY",
            "ParamBrowRY",
            "ParamBrowLAngle",
            "ParamBrowRAngle",
            "ParamMouthForm",
            "ParamMouthOpenY",
        ],
    ),
    ("live2d_expression1", &["Param4"]),
    ("live2d_expression2", &["Param5"]),
    ("nod", &["ParamAngleX", "ParamHairFront"]),
    ("headshake", &["ParamAngleY", "ParamHairSide"]),
];

/// Whether one parameter changes what the model draws, either by moving a
/// vertex or by fading / hiding a drawable.
fn drives_geometry(model: &mut Live2dModel, baseline: &RenderSnapshot, id: &str) -> bool {
    let Some(range) = model.core.parameter_range_by_id(id) else {
        return false;
    };
    let target = if (range.maximum - range.default).abs() >= (range.minimum - range.default).abs() {
        range.maximum
    } else {
        range.minimum
    };
    model
        .core
        .set_parameter_by_id(id, target, 1.0)
        .expect("set parameter");
    let Ok(probe) = model.core.update_and_snapshot() else {
        return false;
    };
    probe
        .drawables
        .iter()
        .zip(baseline.drawables.iter())
        .any(|(probe, baseline)| {
            if (probe.opacity - baseline.opacity).abs() > 1e-4 || probe.visible != baseline.visible
            {
                return true;
            }
            probe
                .vertices
                .iter()
                .zip(baseline.vertices.iter())
                .any(|(probe, baseline)| {
                    let dx = probe.position[0] - baseline.position[0];
                    let dy = probe.position[1] - baseline.position[1];
                    (dx * dx + dy * dy).sqrt() > 1e-4
                })
        })
}

#[test]
fn every_declared_behaviour_draws_something() {
    let mut model = standard_model();
    model
        .restore_parameter_defaults()
        .expect("restore parameter defaults");
    let baseline = model.core.update_and_snapshot().expect("baseline frame");

    for (behaviour, parameters) in DECLARED_BEHAVIOURS {
        let mut driven = Vec::new();
        for id in *parameters {
            model
                .restore_parameter_defaults()
                .expect("restore parameter defaults");
            if drives_geometry(&mut model, &baseline, id) {
                driven.push(*id);
            }
        }
        // Parameters the model does not drive are allowed as long as one other
        // parameter of the same behaviour does; a hair curve next to a head
        // angle is decoration, not the behaviour.
        assert!(
            !driven.is_empty(),
            "the standard model declares {behaviour} but none of {parameters:?} draws anything; \
             a declared behaviour that cannot draw is a dead palette chip"
        );
    }
}

/// The head angles the face rig actually has, pinned because two later passes in
/// the same frame used to reset them: the automatic blink, and pointer tracking.
#[test]
fn the_standard_model_rigs_both_head_angles_but_not_roll() {
    let mut model = standard_model();
    model
        .restore_parameter_defaults()
        .expect("restore parameter defaults");
    let baseline = model.core.update_and_snapshot().expect("baseline frame");

    for id in ["ParamAngleX", "ParamAngleY"] {
        model
            .restore_parameter_defaults()
            .expect("restore parameter defaults");
        assert!(
            drives_geometry(&mut model, &baseline, id),
            "{id} drives no drawable, so a nod or a headshake cannot be seen"
        );
    }
    model
        .restore_parameter_defaults()
        .expect("restore parameter defaults");
    assert!(
        !drives_geometry(&mut model, &baseline, "ParamAngleZ"),
        "the standard model gained a roll rig; tilt.motion3.json can be declared again"
    );
}
