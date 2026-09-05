// Dates in the two shapes this agent has to produce, without pulling in a date
// library for them.
//
// The capture folder is named by local time to the millisecond, and the log
// carries the same clock. Both are read by people, and the folder name is also
// what sorts the captures, so the format is fixed by the editor's side.

use windows_sys::Win32::Foundation::{FILETIME, SYSTEMTIME};
use windows_sys::Win32::System::SystemInformation::GetLocalTime;

/// `yyyyMMdd-HHmmss-fff`, the capture folder's name.
pub fn stamp_ms() -> String {
    let t = local();
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}-{:03}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds
    )
}

/// `yyyy-MM-dd HH:mm:ss.fff`, what every log line starts with.
pub fn stamp_log() -> String {
    let t = local();
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds
    )
}

fn local() -> SYSTEMTIME {
    unsafe {
        let mut t: SYSTEMTIME = std::mem::zeroed();
        GetLocalTime(&mut t);
        t
    }
}

/// A FILETIME as .NET counts ticks: 100 ns units from 0001-01-01, where a
/// FILETIME counts them from 1601-01-01.
///
/// This conversion is not cosmetic. The `pid` file pairs the process id with its
/// start time so that a reused pid cannot be mistaken for the agent, and the
/// editor reads that file with .NET's arithmetic. Writing a raw FILETIME here
/// would make every check fail, and the editor would decide the agent had gone.
pub const TICKS_1601: i64 = 504_911_232_000_000_000;

pub fn filetime_to_net_ticks(ft: FILETIME) -> i64 {
    let raw = ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64;
    raw as i64 + TICKS_1601
}
