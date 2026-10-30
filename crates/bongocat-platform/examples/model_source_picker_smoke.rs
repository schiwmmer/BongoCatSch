//! Manual smoke harness for the native model folder picker.
//!
//! The picker asks for one thing — the folder a user exported — and macOS
//! answers it with its own folder panel.
//!
//! ```text
//! cargo run -p bongocat-platform --example model_source_picker_smoke -- --expect-cancel
//! cargo run -p bongocat-platform --example model_source_picker_smoke -- --expect-selected-any
//! cargo run -p bongocat-platform --example model_source_picker_smoke -- --expect-selected <path>
//! ```

use bongocat_platform::{ModelSourcePickerOutcome, pick_model_folder};
use std::{
    env,
    error::Error,
    io,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool, mpsc},
};

struct NativeApplication {
    _window: objc2::rc::Retained<objc2_app_kit::NSWindow>,
}

fn prepare_native_application() -> NativeApplication {
    use objc2::{MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{
        NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSWindow,
        NSWindowStyleMask,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    let mtm = MainThreadMarker::new().expect("picker smoke must run on the AppKit main thread");
    let application = NSApplication::sharedApplication(mtm);
    let _ = application.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    // SAFETY: the caller runs on the AppKit main thread, the allocated window is retained for the
    // smoke process lifetime, and `releasedWhenClosed(false)` satisfies objc2's ownership contract.
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(320.0, 200.0)),
            NSWindowStyleMask::Titled,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    // SAFETY: this retained NSWindow instance is never closed by the smoke process.
    unsafe { window.setReleasedWhenClosed(false) };
    window.center();
    window.makeKeyAndOrderFront(None);
    application.activate();
    NativeApplication { _window: window }
}

fn start_native_picker(
    sender: mpsc::SyncSender<
        Result<ModelSourcePickerOutcome, bongocat_platform::ModelSourcePickerError>,
    >,
) {
    use dispatch2::DispatchQueue;

    DispatchQueue::main().exec_async(move || {
        let callback_sender = sender;
        let callback_sender_for_picker = callback_sender.clone();
        let result = pick_model_folder(move |result| {
            let _ = callback_sender_for_picker.try_send(result);
            // Project validation runs on a worker thread, so dispatch shutdown unconditionally
            // rather than assuming the callback is already on AppKit's main thread.
            DispatchQueue::main().exec_async(|| {
                let mtm = objc2::MainThreadMarker::new()
                    .expect("NSApplication stop must run on the AppKit main thread");
                objc2_app_kit::NSApplication::sharedApplication(mtm).stop(None);
            });
        });
        if let Err(error) = result {
            let _ = callback_sender.try_send(Err(error));
            DispatchQueue::main().exec_async(|| {
                let mtm = objc2::MainThreadMarker::new()
                    .expect("NSApplication stop must run on the AppKit main thread");
                objc2_app_kit::NSApplication::sharedApplication(mtm).stop(None);
            });
        }
    });
}

fn run_native_application(_application: &NativeApplication) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;

    let mtm = MainThreadMarker::new().expect("picker smoke must run on the AppKit main thread");
    NSApplication::sharedApplication(mtm).run();
}

enum ExpectedOutcome {
    Cancelled,
    Selected(PathBuf),
    SelectedAny,
}

struct SmokeOptions {
    expected: ExpectedOutcome,
    automated: bool,
}

fn smoke_options() -> Result<SmokeOptions, io::Error> {
    let mut arguments = env::args().skip(1);
    let expected = match arguments.next().as_deref() {
        Some("--expect-cancel") => ExpectedOutcome::Cancelled,
        Some("--expect-selected") => {
            let path = arguments
                .next()
                .ok_or_else(|| io::Error::other("--expect-selected requires a path"))?;
            ExpectedOutcome::Selected(PathBuf::from(path).canonicalize()?)
        }
        Some("--expect-selected-any") => ExpectedOutcome::SelectedAny,
        _ => {
            return Err(io::Error::other(
                "expected --expect-cancel, --expect-selected <path>, or --expect-selected-any",
            ));
        }
    };
    let mut automated = false;
    for argument in arguments {
        match argument.as_str() {
            "--auto" => automated = true,
            _ => return Err(io::Error::other("unexpected picker smoke argument")),
        }
    }
    Ok(SmokeOptions {
        expected,
        automated,
    })
}

fn start_automation(
    _expected: &ExpectedOutcome,
    automated: bool,
    _completed: Arc<AtomicBool>,
) -> Result<Option<std::thread::JoinHandle<Result<(), io::Error>>>, io::Error> {
    if automated {
        return Err(io::Error::other(
            "--auto is not supported by the macOS picker smoke",
        ));
    }
    Ok(None)
}

fn run_native_picker_smoke() -> Result<(), Box<dyn Error>> {
    let options = smoke_options()?;
    let native_application = prepare_native_application();
    let (sender, receiver) = mpsc::sync_channel(1);
    let completed = Arc::new(AtomicBool::new(false));
    start_native_picker(sender.clone());
    let automation = start_automation(&options.expected, options.automated, completed)?;
    run_native_application(&native_application);
    let actual = receiver
        .recv()
        .map_err(|_| io::Error::other("model source picker callback was dropped"))??;
    if let Some(automation) = automation {
        automation
            .join()
            .map_err(|_| io::Error::other("model source picker automation panicked"))??;
    }
    match (options.expected, actual) {
        (ExpectedOutcome::Cancelled, ModelSourcePickerOutcome::Cancelled) => Ok(()),
        (ExpectedOutcome::Selected(expected), ModelSourcePickerOutcome::Selected(actual))
            if actual == expected =>
        {
            Ok(())
        }
        // The picker promises one shape: a real directory. What that directory
        // contains is the model store's decision, not the dialog's.
        (ExpectedOutcome::SelectedAny, ModelSourcePickerOutcome::Selected(actual))
            if actual.is_absolute() && actual.is_dir() =>
        {
            Ok(())
        }
        _ => Err(io::Error::other("model source picker returned an unexpected outcome").into()),
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    run_native_picker_smoke()
}
