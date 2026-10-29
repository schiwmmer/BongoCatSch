//! The headless previews, which run a model without a window.
//!
//! A preview answers "would this model work" without asking the user to install
//! it: it loads the model, drives it, and renders frames off-screen. The
//! generation counter exists because a preview can be superseded while it is
//! running, and a slow one must not overwrite the result of a newer one.

use super::*;

pub fn run_model_preview(
    model_id: &str,
    model_root: &Path,
    duration: Duration,
) -> Result<PreviewReport, OverlayError> {
    {
        macos::run_model_preview(model_id, model_root, duration, false, None)
    }

}

pub fn run_interactive_model_preview(
    model_id: &str,
    model_root: &Path,
    duration: Duration,
) -> Result<PreviewReport, OverlayError> {
    {
        macos::run_model_preview(model_id, model_root, duration, true, None)
    }

}

pub fn run_model_switch_preview(
    model_id: &str,
    model_root: &Path,
    switch_cycles: u32,
) -> Result<PreviewReport, OverlayError> {
    {
        macos::run_model_preview(
            model_id,
            model_root,
            Duration::ZERO,
            false,
            Some(switch_cycles),
        )
    }

}

pub(crate) fn validate_model_generation_advance(
    active_generation: u64,
    candidate_generation: u64,
) -> Result<(), OverlayError> {
    if candidate_generation <= active_generation {
        return Err(OverlayError::new(format!(
            "GPU model generation did not advance from {active_generation} to {candidate_generation}"
        )));
    }
    Ok(())
}
