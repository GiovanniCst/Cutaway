// Telling the agent where Cutaway is.
//
// Normally nothing has to: the editor sits beside the agent, and the agent
// looks there first. That covers the installed copy and a portable folder left
// as it was unpacked.
//
// It stops covering the case the portable is actually used in. A person unzips
// it, runs it, keeps the shortcut - and then moves the folder, or puts the
// editor on a second disk, or copies the agent out on its own. From then on the
// key is pressed, the screen freezes, a rectangle is cut, and nothing opens: the
// agent has a piece and nowhere to send it. The failure is silent and there is
// nothing on screen that would let anybody fix it.
//
// So: one line in the menu, a file dialog, and a path written down. The file it
// writes is the one the handoff has always read.

use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

use windows_sys::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, MB_ICONINFORMATION, MB_ICONWARNING, MB_OK,
};

use crate::paths;
use crate::strings::{f1, t};
use crate::wide::wide;

/// Asks for the file, and remembers it. False when the person said no.
pub fn ask() -> bool {
    // The filter is two pairs of null-terminated strings ending in a second
    // null: the shown name, then the pattern, for each entry.
    let mut filter: Vec<u16> = Vec::new();
    for part in [t("Cutaway"), "Cutaway.exe".to_string(), t("Programs"), "*.exe".to_string()] {
        filter.extend(part.encode_utf16());
        filter.push(0);
    }
    filter.push(0);

    let title = wide(&t("Where Cutaway is"));
    // MAX_PATH is not the limit any more, but the dialog still writes into what
    // it is given: room for a long path, and the length is told in characters.
    let mut chosen = vec![0u16; 4096];
    if let Some(known) = crate::handoff::resolve_editor() {
        let text: Vec<u16> = known.as_os_str().encode_wide().collect();
        if text.len() < chosen.len() {
            chosen[..text.len()].copy_from_slice(&text);
        }
    }

    let mut form: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    form.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    form.lpstrFilter = filter.as_ptr();
    form.lpstrFile = chosen.as_mut_ptr();
    form.nMaxFile = chosen.len() as u32;
    form.lpstrTitle = title.as_ptr();
    form.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST;

    if unsafe { GetOpenFileNameW(&mut form) } == 0 {
        return false; // cancelled, or the dialog could not open
    }

    let end = chosen.iter().position(|c| *c == 0).unwrap_or(chosen.len());
    let picked = PathBuf::from(String::from_utf16_lossy(&chosen[..end]));
    if !picked.exists() {
        return false;
    }

    // Said out loud when it is not the file the agent expects. Not refused: a
    // renamed copy is still the editor, and a person who has gone looking for
    // it knows better than this check does.
    let looks_right = picked
        .file_name()
        .map(|name| name.eq_ignore_ascii_case("Cutaway.exe"))
        .unwrap_or(false);

    paths::write_line(&paths::editor_hint(), &picked.to_string_lossy());

    let shown = picked.to_string_lossy().to_string();
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            wide(&f1("Cutaway will be opened from {0}.", &shown)).as_ptr(),
            wide(&t("Where Cutaway is")).as_ptr(),
            MB_OK | if looks_right { MB_ICONINFORMATION } else { MB_ICONWARNING },
        );
    }
    true
}
