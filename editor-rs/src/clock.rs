// Dates, without a date library for them.
//
// A capture is nameless, so the moment it arrived is the whole name; a printed
// sheet is the same. Both become file names, so the colons a clock would use are
// dashes here - Windows refuses them in a name, and the save dialog refuses them
// louder.

use windows_sys::Win32::Foundation::SYSTEMTIME;
use windows_sys::Win32::System::SystemInformation::GetLocalTime;

fn local() -> SYSTEMTIME {
    unsafe {
        let mut t: SYSTEMTIME = std::mem::zeroed();
        GetLocalTime(&mut t);
        t
    }
}

/// `yyyy-MM-dd HH-mm-ss`, for a file name.
pub fn stamp_file() -> String {
    let t = local();
    format!(
        "{:04}-{:02}-{:02} {:02}-{:02}-{:02}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond
    )
}

/// `yyyy-MM-dd`, for the day a shortlist or a measurement was true.
pub fn today() -> String {
    let t = local();
    format!("{:04}-{:02}-{:02}", t.wYear, t.wMonth, t.wDay)
}

/// `yyyy-MM`, which is as precise as a signature on a picture needs to be.
pub fn month() -> String {
    let t = local();
    format!("{:04}-{:02}", t.wYear, t.wMonth)
}
