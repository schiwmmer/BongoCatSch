//! Every behaviour a shipped model advertises has to draw something.
//!
//! A parameter can be written successfully and still draw nothing: the value
//! lands in the model, but no vertex references it, so a palette chip that plays
//! that behaviour looks like a dead button. Those two facts are independent, so
//! "the click did nothing" has to be told apart from "the click never ran" by
//! measuring the model rather than the click.
//!
//! The parameters each behaviour drives are read from the model package itself
//! rather than listed here, so a model that gains a behaviour is covered the day
//! it is added. An expression with no parameter list at all is skipped: it is a
//! neutral reset, and having nothing to draw is the point of it.

use super::*;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// The models the repository ships. Each one is a separate moc3, so each one
/// can bind a different set of parameters.
const PRESET_MODELS: &[&str] = &["standard", "gamepad", "keyboard"];

const MODEL3: &str = "cat.model3.json";

fn resources_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repository root")
        .join("resources/models")
}

fn load_model(model_id: &str) -> Live2dModel {
    use bongocat_model::{ModelId, ModelPackageLimits, PresetModelCatalog};

    let committed = PresetModelCatalog::open(resources_root(), ModelPackageLimits::default())
        .expect("preset catalog")
        .load(&ModelId::parse(model_id).expect("model id"))
        .expect("preset model");
    Live2dModel::load(&committed).expect("Live2D model")
}

fn read_json(path: &Path) -> Value {
    let bytes =
        std::fs::read(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()))
}

/// The parameter curves of a motion clip, in declaration order.
fn motion_parameters(directory: &Path, file: &str) -> Vec<String> {
    read_json(&directory.join(file))["Curves"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter(|curve| curve["Target"].as_str() == Some("Parameter"))
        .filter_map(|curve| curve["Id"].as_str().map(str::to_owned))
        .collect()
}

/// The parameter targets of an expression.
fn expression_parameters(directory: &Path, file: &str) -> Vec<String> {
    read_json(&directory.join(file))["Parameters"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(|parameter| parameter["Id"].as_str().map(str::to_owned))
        .collect()
}

/// Whether one parameter changes what the model draws, either by moving a vertex
/// or by fading / hiding a drawable.
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

/// Returns the complaint when none of `parameters` changes what the model
/// draws, and `None` when the behaviour is drawable.
fn complaint(
    model: &mut Live2dModel,
    baseline: &RenderSnapshot,
    what: &str,
    parameters: &[String],
) -> Option<String> {
    let mut driven = Vec::new();
    for id in parameters {
        model
            .restore_parameter_defaults()
            .expect("restore parameter defaults");
        if drives_geometry(model, baseline, id) {
            driven.push(id.as_str());
        }
    }
    // A parameter the model does not bind is allowed next to one it does: a hair
    // curve beside a head angle is decoration, not the behaviour. All of them
    // being inert is the failure.
    if driven.is_empty() {
        return Some(format!(
            "{what} declares {parameters:?} but none of them draws anything"
        ));
    }
    None
}

/// Collects every inert behaviour across every shipped model in one run, so a
/// model that needs artwork or a declaration change is reported whole rather
/// than one name per test run.
#[test]
fn every_preset_model_declares_only_behaviours_it_can_draw() {
    let mut dead = Vec::new();
    for model_id in PRESET_MODELS {
        let directory = resources_root().join(model_id);
        let model3 = read_json(&directory.join(MODEL3));
        let mut model = load_model(model_id);
        model
            .restore_parameter_defaults()
            .expect("restore parameter defaults");
        let baseline = model.core.update_and_snapshot().expect("baseline frame");

        let motions = &model3["FileReferences"]["Motions"];
        for (group, entries) in motions.as_object().expect("motion groups") {
            for (index, entry) in entries.as_array().expect("motion list").iter().enumerate() {
                let file = entry["File"].as_str().expect("motion file");
                let parameters = motion_parameters(&directory, file);
                assert!(
                    !parameters.is_empty(),
                    "{model_id} motion {group}[{index}] ({file}) declares no parameter curve"
                );
                dead.extend(complaint(
                    &mut model,
                    &baseline,
                    &format!("{model_id} motion {group}[{index}] ({file})"),
                    &parameters,
                ));
            }
        }

        for entry in model3["FileReferences"]["Expressions"]
            .as_array()
            .expect("expression list")
        {
            let file = entry["File"].as_str().expect("expression file");
            let parameters = expression_parameters(&directory, file);
            if parameters.is_empty() {
                // A neutral reset: it exists to clear another expression, so it
                // has nothing of its own to draw.
                continue;
            }
            dead.extend(complaint(
                &mut model,
                &baseline,
                &format!("{model_id} expression {file}"),
                &parameters,
            ));
        }
    }
    assert!(
        dead.is_empty(),
        "a declared behaviour that cannot draw is a dead palette chip:\n  {}",
        dead.join("\n  ")
    );
}

/// The head angles the standard model's face rig actually has, pinned because
/// two later passes in the same frame used to reset them: the automatic blink,
/// and pointer tracking.
#[test]
fn the_standard_model_rigs_both_head_angles_but_not_roll() {
    let mut model = load_model("standard");
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
