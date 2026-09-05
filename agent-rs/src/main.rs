// Entry point and the modes the agent runs in.
//
//   CutawayAgent.exe --background   stay resident (what the Run key and the editor start)
//   CutawayAgent.exe --once <dir>   one capture into <dir>, then exit (the editor, when no agent runs)
//   CutawayAgent.exe --quit         ask the resident instance to leave, wait until it has
//
// No console window: this is a desktop program, and a stray line of output
// should not open a black rectangle behind it.
#![windows_subsystem = "windows"]

mod about;
mod autostart;
mod clip;
mod clock;
mod dpi;
mod handoff;
mod locate;
mod overlay;
mod paths;
mod quit;
mod resident;
mod strings;
mod tray;
mod uninstall;
mod wide;

use std::path::PathBuf;

fn main() {
    // Before any window exists and before any screen metric is read: with no
    // manifest to declare it, this call is the only thing standing between the
    // overlay and a capture cut from the wrong place. See dpi.rs.
    dpi::declare();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args.first().map(String::as_str).unwrap_or("--background");

    match mode {
        "--quit" => {
            std::process::exit(if quit::request(5000) { 0 } else { 1 });
        }
        "--once" => match args.get(1) {
            Some(dir) => resident::run_once(PathBuf::from(dir)),
            None => std::process::exit(1),
        },
        _ => {
            // One resident agent per session. The handle is held for the whole
            // run: dropping it early would let a second one in.
            let Some(_single) = quit::claim_single_instance() else {
                return; // already resident
            };
            resident::run();
        }
    }
}
