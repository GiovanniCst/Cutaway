// Removing the portable from a machine.
//
// Deleting Cutaway.exe is what a person does to get rid of a portable program,
// and for this one it is not enough: the agent has copied itself into
// %LOCALAPPDATA% and written a Run key, so at every logon an icon comes back
// whose editor no longer exists. The tray menu offers the way out, and it is
// offered only when the agent is running from %LOCALAPPDATA% - the installed
// copy is removed by its own uninstaller.

use std::os::windows::process::CommandExt;
use std::process::Command;

use windows_sys::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, IDOK, MB_DEFBUTTON2, MB_ICONINFORMATION, MB_ICONWARNING, MB_OK, MB_OKCANCEL,
};

use crate::autostart;
use crate::paths;
use crate::strings::t;
use crate::wide::wide;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// True for any copy that has no uninstaller of its own.
///
/// It used to mean "runs from %LOCALAPPDATA%", which was the only shape a
/// portable took when the editor was a single self-extracting file. A portable
/// is a folder now, unzipped wherever the person likes, and that test said no
/// there: the agent wrote a Run key at every logon and the tray offered no way
/// to take it back, so deleting the folder left an entry pointing at nothing
/// and an icon that could never come back.
///
/// The question is not where the agent lives but whether something else already
/// removes it. An installed copy has its uninstaller in the same folder; a
/// portable has two executables and a licence.
pub fn offered() -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    let Some(here) = exe.parent() else { return false };
    let Ok(entries) = std::fs::read_dir(here) else { return false };
    !entries.filter_map(Result::ok).any(|entry| {
        entry
            .file_name()
            .to_string_lossy()
            .to_ascii_lowercase()
            .starts_with("unins")
    })
}

/// The folder the portable was unpacked into, when that is what this is.
///
/// Nothing here can remove it: the executable running this code is inside it.
/// So it is named instead, in the last thing the person is told.
fn folder_left_behind() -> Option<String> {
    let here = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let profile = paths::agent_dir().to_string_lossy().to_ascii_lowercase();
    let mine = here.to_string_lossy().to_ascii_lowercase();
    (!mine.starts_with(&profile)).then(|| here.to_string_lossy().to_string())
}

/// Asks, then takes everything out and leaves. False when the person said no.
pub fn run() -> bool {
    let answer = unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            wide(&t("This removes the shortcut, the background agent and the folder Cutaway keeps in your user profile. The picture files you saved are not touched.")).as_ptr(),
            wide(&t("Remove Cutaway from this computer")).as_ptr(),
            MB_OKCANCEL | MB_ICONWARNING | MB_DEFBUTTON2,
        )
    };
    if answer != IDOK {
        return false;
    }

    autostart::forget();
    let _ = std::fs::remove_dir_all(paths::captures_dir());
    paths::append("uninstalled by the person");

    // Said before the folder goes, and before the icon disappears: clicking OK on
    // a removal and then seeing nothing at all leaves you wondering.
    let said = match folder_left_behind() {
        Some(folder) => format!(
            "{}

{}",
            t("Cutaway has been removed from this computer."),
            crate::strings::f1(
                "The folder you unpacked is still where you put it: delete {0} when you like.",
                &folder,
            ),
        ),
        None => t("Cutaway has been removed from this computer."),
    };
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            wide(&said).as_ptr(),
            wide(&t("Remove Cutaway from this computer")).as_ptr(),
            MB_OK | MB_ICONINFORMATION,
        )
    };

    // The agent's own folder holds the executable running this code, so it cannot
    // go now: a detached command waits for this process to be gone. Started only
    // here, once the dialog is closed, or its wait would run out while the dialog
    // was still up and the executable still mapped.
    erase_after_exit(&paths::agent_dir().to_string_lossy());
    true
}

fn erase_after_exit(dir: &str) {
    // ping rather than timeout: timeout wants a console input handle and fails
    // outright when there is none. Twice, because the first attempt can still
    // find the executable mapped.
    //
    // raw_arg and not arg, and this is the whole reason the folder used to
    // survive being removed. Rust quotes an argument that contains spaces and
    // escapes the quotes inside it as \" - which is C's convention, not cmd's.
    // cmd passed the backslash through, rd was handed a path beginning with one,
    // and 2>nul swallowed the complaint: the sweep reported nothing and did
    // nothing. Windows Sandbox found it, by looking at what was left.
    let script = format!(
        "/c ping -n 4 127.0.0.1 >nul & rd /s /q \"{dir}\" & ping -n 5 127.0.0.1 >nul & rd /s /q \"{dir}\""
    );
    let started = Command::new("cmd.exe")
        .raw_arg(script)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
    if let Err(exc) = started {
        paths::append(&format!("deferred removal failed: {}", exc));
    }
}
