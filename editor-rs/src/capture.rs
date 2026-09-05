// Taking delivery of a piece the agent cut.
//
// The agent starts this editor the moment its overlay is on the glass, while the
// rectangle is still being drawn, and drops the piece into a folder when the
// mouse comes up. So the wait happens on a thread of its own and the window goes
// on loading meanwhile: by the time there is something to show, there is
// somewhere to show it.
//
// The protocol is the one the Python editor already speaks, unchanged, because
// the agent on the other side is the same program: a folder that contains either
// piece.png or a file called cancel, and taking the folder away is how the agent
// learns the piece arrived.

use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use image::RgbaImage;

/// How long to wait for a piece before giving up on the agent.
///
/// The agent's own timeout is fifteen seconds, and it takes the folder away when
/// it stops waiting; this one only has to outlive that, so a slow disk cannot
/// make the two disagree about who gave up first.
const PATIENCE: Duration = Duration::from_secs(20);

pub enum Delivery {
    /// The piece, and the name it should carry.
    Piece(RgbaImage, String),
    /// The person cancelled, or the agent went away. Nothing was ever shown, and
    /// leaving quietly is the whole point.
    Nothing,
}

/// Watches the folder on a thread and reports once.
pub fn wait_in(folder: &Path, agent_pid: Option<u32>) -> Receiver<Delivery> {
    let (send, receive) = channel();
    let folder = folder.to_path_buf();
    thread::spawn(move || {
        let outcome = watch(&folder, agent_pid);
        // Taking the folder away is how the agent learns the piece arrived and
        // takes the overlay down, so it happens before the window comes up.
        let _ = std::fs::remove_dir_all(&folder);
        let _ = send.send(outcome);
    });
    receive
}

fn watch(folder: &Path, agent_pid: Option<u32>) -> Delivery {
    let piece = folder.join("piece.png");
    let cancelled = folder.join("cancel");
    let until = Instant::now() + PATIENCE;
    while Instant::now() < until {
        if cancelled.exists() {
            return Delivery::Nothing;
        }
        if piece.is_file() {
            // Written to a temporary name and renamed by the agent, so a file
            // that exists is a file that is whole.
            match image::open(&piece) {
                Ok(decoded) => {
                    return Delivery::Piece(decoded.to_rgba8(), stamped_name());
                }
                Err(_) => return Delivery::Nothing,
            }
        }
        if let Some(pid) = agent_pid {
            if !alive(pid) {
                // The agent died mid-capture: there will never be a piece.
                return Delivery::Nothing;
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    Delivery::Nothing
}

/// A capture is nameless, so the moment it arrived is the whole name.
fn stamped_name() -> String {
    format!("Screenshot {}.png", crate::clock::stamp_file())
}

fn alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut code = 0u32;
        let asked = GetExitCodeProcess(handle, &mut code);
        CloseHandle(handle);
        // 259 is STILL_ACTIVE.
        asked != 0 && code == 259
    }
}

/// Where the piece would be, for a folder the agent named.
/// The agent that cuts a rectangle out of the screen, if it can be found.
///
/// Beside this program when installed - the installer puts the two together -
/// and beside its own build folder when this is a build. Nothing is guessed
/// beyond those two: an agent found somewhere else on the machine is not
/// necessarily this agent.
pub fn find_agent() -> Option<PathBuf> {
    let here = std::env::current_exe().ok()?;
    let beside = here.parent()?.join("CutawayAgent.exe");
    if beside.exists() {
        return Some(beside);
    }
    // A build: editor-rs/target/<profile>/ and agent-rs/target/release/ sit
    // under the same project folder.
    let project = here.parent()?.parent()?.parent()?.parent()?;
    let built = project.join("agent-rs").join("target").join("release").join("CutawayAgent.exe");
    built.exists().then_some(built)
}

/// Starts the agent beside this program, unless it was told not to.
///
/// The portable is unzipped and Cutaway.exe is double-clicked, and until this
/// existed that was the end of it: the shortcut the program is named for did
/// not work, because nothing had ever started the half that listens for it. The
/// installer starts the agent; a zip cannot.
///
/// Two things make it safe to do on every start. A second agent cannot exist -
/// the first thing one does is take a named mutex, and the loser returns
/// without a window - so this is at worst a process that begins and ends. And
/// the person's choice is read first: the agent writes "0" into that file when
/// "Start with Windows" is turned off, and its own comment says the editor
/// reads it "to know whether to bring the agent back". This is that.
pub fn wake_agent() {
    let Some(agent) = find_agent() else { return };
    let choice = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Cutaway")
        .join("agent")
        .join("autostart");
    if std::fs::read_to_string(&choice).map(|line| line.trim() == "0").unwrap_or(false) {
        return;
    }
    // On a thread of its own: creating a process is not slow, but the whole
    // argument for this build is a window in a hundred milliseconds and nothing
    // that can be moved off that path stays on it.
    std::thread::spawn(move || {
        let _ = std::process::Command::new(agent)
            .arg("--background")
            // No console window flashes up from a windowed program.
            .creation_flags(0x0800_0000)
            .spawn();
    });
}

/// A folder for one capture, named so two cannot collide.
pub fn fresh_folder() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("Cutaway").join("captures").join(format!(
        "w{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.as_millis())
            .unwrap_or(0)
    ))
}

pub fn piece_in(folder: &Path) -> PathBuf {
    folder.join("piece.png")
}
