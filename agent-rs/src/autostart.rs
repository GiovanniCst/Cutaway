// Starting with Windows: on unless the person turned it off.
//
// The choice is a one-line file, not the Run value itself, because "absent" has
// to mean two different things - never asked (on) and switched off - and the
// editor reads it too, to know whether to bring the agent back. Task Manager
// disables a Run entry without touching it: it writes StartupApproved instead,
// so that is read as well and respected.

use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_BINARY, REG_OPTION_NON_VOLATILE, REG_SZ,
};

use crate::paths;
use crate::wide::wide;

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const APPROVED_KEY: &str =
    "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run";
const VALUE_NAME: &str = "Cutaway";

/// The choice on file (absent = on), and not vetoed from Task Manager.
pub fn enabled() -> bool {
    paths::read_line(&paths::autostart_file()).as_deref() != Some("0") && !disabled_by_windows()
}

/// The person clicked the menu: this is the one moment a Task Manager veto is
/// overridden.
pub fn set(on: bool) {
    paths::write_line(&paths::autostart_file(), if on { "1" } else { "0" });
    if let Some(key) = create_key(APPROVED_KEY) {
        unsafe {
            if on {
                // The twelve bytes Explorer writes for "enabled"; only the first
                // one is read, and its low bit is the veto.
                let value = [2u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
                RegSetValueExW(
                    key.0,
                    wide(VALUE_NAME).as_ptr(),
                    0,
                    REG_BINARY,
                    value.as_ptr(),
                    value.len() as u32,
                );
            } else {
                RegDeleteValueW(key.0, wide(VALUE_NAME).as_ptr());
            }
        }
    }
    apply();
}

/// Makes the Run value match the choice. Called at every start, so an installed
/// agent takes over from a portable one without anybody noticing.
pub fn apply() {
    let Some(key) = create_key(RUN_KEY) else { return };
    let wanted = paths::read_line(&paths::autostart_file()).as_deref() != Some("0");
    unsafe {
        if wanted {
            let Ok(exe) = std::env::current_exe() else { return };
            let command = format!("\"{}\" --background", exe.display());
            let value = wide(&command);
            RegSetValueExW(
                key.0,
                wide(VALUE_NAME).as_ptr(),
                0,
                REG_SZ,
                value.as_ptr() as *const u8,
                // Bytes, terminator included, which is what REG_SZ counts.
                (value.len() * 2) as u32,
            );
        } else {
            RegDeleteValueW(key.0, wide(VALUE_NAME).as_ptr());
        }
    }
}

/// Takes the Run and StartupApproved values away: the portable being removed.
pub fn forget() {
    for path in [RUN_KEY, APPROVED_KEY] {
        if let Some(key) = open_key(path, KEY_WRITE) {
            unsafe { RegDeleteValueW(key.0, wide(VALUE_NAME).as_ptr()) };
        }
    }
}

fn disabled_by_windows() -> bool {
    let Some(key) = open_key(APPROVED_KEY, KEY_READ) else { return false };
    unsafe {
        let mut kind = 0u32;
        let mut buffer = [0u8; 16];
        let mut size = buffer.len() as u32;
        let status = RegQueryValueExW(
            key.0,
            wide(VALUE_NAME).as_ptr(),
            std::ptr::null_mut(),
            &mut kind,
            buffer.as_mut_ptr(),
            &mut size,
        );
        // 0x03 is disabled, 0x02 enabled: the low bit carries the veto.
        status == ERROR_SUCCESS && size > 0 && (buffer[0] & 1) == 1
    }
}

struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        unsafe { RegCloseKey(self.0) };
    }
}

fn create_key(path: &str) -> Option<Key> {
    unsafe {
        let mut key: HKEY = std::ptr::null_mut();
        let status = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            wide(path).as_ptr(),
            0,
            std::ptr::null_mut(),
            REG_OPTION_NON_VOLATILE,
            KEY_READ | KEY_WRITE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        );
        // A Run value that cannot be written is not worth a crash.
        if status == ERROR_SUCCESS { Some(Key(key)) } else { None }
    }
}

fn open_key(path: &str, access: u32) -> Option<Key> {
    unsafe {
        let mut key: HKEY = std::ptr::null_mut();
        let status = RegOpenKeyExW(HKEY_CURRENT_USER, wide(path).as_ptr(), 0, access, &mut key);
        if status == ERROR_SUCCESS { Some(Key(key)) } else { None }
    }
}
