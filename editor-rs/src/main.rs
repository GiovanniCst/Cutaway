// Cutaway, natively.
//
//   Cutaway.exe                       open empty
//   Cutaway.exe <file>                open a picture
//   Cutaway.exe --capture-from <dir>  wait for the agent to drop a piece there
//   Cutaway.exe --agent-pid <pid>     the agent to watch while waiting
//
// The interface is drawn rather than rendered: no browser, no page, no bridge.
// Measured, a drawn window is on screen in about 120 ms against
// the 2576 ms WebView2 takes to reach the same point - and the editor is started
// afresh on every capture, so that difference is paid every single time.
//
// The other half of the argument is not speed. In the WebView2 build the tone
// adjustments exist twice, once in JavaScript for the preview and once in Python
// for the save, with a test whose only job is to check that the two agree. Here
// the preview and the save are the same code, and that entire class of bug has
// nowhere to live.
#![windows_subsystem = "windows"]

mod about;
mod adjust;
mod ai;
mod annotate;
mod capture;
mod clip;
mod clock;
mod crop;
mod cutout;
mod empty;
mod mail;
mod markup;
mod models;
mod mondrian;
mod ocr;
mod picture;
mod print;
mod save;
mod scroll;
mod secrets;
mod settings;
mod skin;
mod studio;
mod ui;
mod widgets;
mod words;

use std::path::PathBuf;
use std::time::Instant;

use windows_sys::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};

/// The picture Windows puts on the taskbar and in the corner of the window.
///
/// Nine kilobytes in the binary, and the only thing in it that is not code.
/// Without it Windows draws a placeholder of its own - a letter in a rounded
/// square - which is what a program with no icon looks like: an unfinished one.
///
/// The same icon the 1.6 build uses. A new one belongs to the identity work,
/// not to the fixing of a missing one.
fn icon() -> egui::IconData {
    const BYTES: &[u8] = include_bytes!("../../assets/cutaway.png");
    match image::load_from_memory(BYTES) {
        Ok(loaded) => {
            let rgba = loaded.to_rgba8();
            let (width, height) = rgba.dimensions();
            egui::IconData { rgba: rgba.into_raw(), width, height }
        }
        // A window with the wrong icon is better than no window.
        Err(_) => egui::IconData { rgba: Vec::new(), width: 0, height: 0 },
    }
}

/// What the command line asked for.
pub struct Started {
    pub open: Option<PathBuf>,
    pub capture_from: Option<PathBuf>,
    pub agent_pid: Option<u32>,
    /// When the process began, so the window can say how long it took.
    pub clock: Instant,
}

fn main() -> eframe::Result<()> {
    let clock = Instant::now();
    // Before any window exists. Without a manifest to declare it - the gnu
    // toolchain has no resource compiler, and this one is built with msvc but
    // keeps the same habit - this call is what stands between the window and a
    // blurry upscaled one on a display that is not at 100%.
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };

    let mut started = Started { open: None, capture_from: None, agent_pid: None, clock };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--capture-from" => started.capture_from = args.next().map(PathBuf::from),
            "--agent-pid" => started.agent_pid = args.next().and_then(|p| p.parse().ok()),
            other if !other.starts_with("--") => started.open = Some(PathBuf::from(other)),
            _ => {}
        }
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Cutaway")
            .with_icon(icon())
            .with_inner_size([1280.0, 860.0])
            .with_min_inner_size([960.0, 640.0])
            // Started for a capture: loaded and ready behind the agent's overlay,
            // shown only once there is a picture in it.
            .with_visible(started.capture_from.is_none()),
        ..Default::default()
    };

    eframe::run_native(
        "Cutaway",
        options,
        Box::new(|cc| Ok(Box::new(ui::Editor::new(cc, started)))),
    )
}
