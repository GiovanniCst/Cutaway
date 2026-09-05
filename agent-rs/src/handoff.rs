// Getting the piece to the editor: a folder per capture, the file inside it, and
// the editor started early enough that it is usually ready when the file is.

use std::fs;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow;

use crate::paths;

/// Lends the right to whichever process asks next, rather than to one pid.
const ASFW_ANY: u32 = 0xFFFF_FFFF;

/// No console window for a child started from a windowed process.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Cutaway.exe beside the agent when installed; the portable one writes where it is.
pub fn resolve_editor() -> Option<PathBuf> {
    // What somebody said, first. The file beside the agent is a good guess and
    // is right for an installed copy, but a guess is what it is: a person who
    // has been through the menu and pointed at the editor has answered the
    // question, and their answer should not be overruled by a sibling.
    if let Some(hinted) = paths::read_line(&paths::editor_hint()) {
        let hinted = PathBuf::from(hinted);
        if hinted.exists() {
            return Some(hinted);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sibling = dir.join("Cutaway.exe");
            if sibling.exists() {
                return Some(sibling);
            }
        }
    }
    None
}

/// Starts the editor for a capture that is about to be drawn.
pub fn start_editor(editor: &Path, dir: &Path) -> std::io::Result<Child> {
    let started = command(editor)
        .arg("--capture-from")
        .arg(dir)
        .arg("--agent-pid")
        .arg(std::process::id().to_string())
        .spawn()?;
    unsafe { AllowSetForegroundWindow(started.id()) };
    Ok(started)
}

/// Opens the editor with nothing in it: the tray menu, or a double click.
pub fn open_editor(editor: &Path) -> std::io::Result<Child> {
    let started = command(editor).spawn()?;
    unsafe { AllowSetForegroundWindow(started.id()) };
    Ok(started)
}

/// Drops PyInstaller's private variables from the child's environment.
///
/// A onefile editor unpacks itself and relaunches as its own child, and says so
/// to that child through _PYI_PARENT_PROCESS_LEVEL. If this agent was started by
/// such an editor it inherited them, and handing them back to the editor it
/// launches makes that editor believe it is somebody's unpacked child. It then
/// checks that its parent is the same executable, finds CutawayAgent.exe
/// instead, and refuses to start with "parent process has different executable".
/// The shortcut simply stops working, for the portable only.
///
/// The editor clears them on its side too. This is the same fix on the other
/// end, for an agent already installed by an older copy and still carrying them.
fn command(editor: &Path) -> Command {
    let mut command = Command::new(editor);
    if let Some(dir) = editor.parent() {
        command.current_dir(dir);
    }
    for (name, _) in std::env::vars_os().filter_map(|(k, v)| {
        k.to_str()
            .filter(|k| k.to_ascii_uppercase().starts_with("_PYI"))
            .map(|k| (k.to_string(), v))
    }) {
        command.env_remove(name);
    }
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

/// Called again right before the overlay goes down, when the editor's window is
/// about to show.
///
/// ASFW_ANY rather than the editor's pid, because with the portable that pid is
/// the wrong one: the PyInstaller bootloader unpacks itself and relaunches as a
/// child, and the window belongs to the child, which the grant to the parent
/// does not cover. The permission lasts until the next foreground change, and
/// the only process about to ask for it is the one we just started.
pub fn lend_foreground() {
    unsafe { AllowSetForegroundWindow(ASFW_ANY) };
}

/// Written to a temporary name and renamed: the editor never sees a half file.
pub fn write_piece(dir: &Path, width: u32, height: u32, rgba: &[u8]) -> std::io::Result<()> {
    let temp = dir.join("piece.png.tmp");
    {
        let file = fs::File::create(&temp)?;
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(rgba)?;
    }
    fs::rename(temp, dir.join("piece.png"))
}

pub fn write_cancel(dir: &Path) {
    let _ = fs::write(dir.join("cancel"), b"");
}

pub fn remove(dir: &Path) {
    let _ = fs::remove_dir_all(dir);
}
