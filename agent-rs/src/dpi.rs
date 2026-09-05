// What Windows thinks this process knows about scaling.
//
// It matters more here than in most programs: the overlay covers the whole
// desktop in physical pixels and cuts out of a still of the same size. A process
// Windows considers unaware is handed scaled coordinates for one and unscaled
// pixels for the other, and the piece comes out of the wrong place - which is
// exactly the bug this agent was written to fix.
//
// The C# agent declared it in a manifest, so it was aware from the moment the
// process started. This one cannot: the gnu toolchain ships no resource
// compiler, so there is no manifest to embed and the awareness is set on the
// first line of main instead. That works, but only if nothing reads a screen
// metric first - so the answer is written into the log, where it can be checked
// after the fact rather than assumed.

use std::cell::RefCell;

use windows_sys::Win32::Foundation::{BOOL, LPARAM, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use windows_sys::Win32::UI::HiDpi::{
    GetAwarenessFromDpiAwarenessContext, GetDpiForMonitor, GetThreadDpiAwarenessContext,
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    DPI_AWARENESS_PER_MONITOR_AWARE, DPI_AWARENESS_SYSTEM_AWARE, DPI_AWARENESS_UNAWARE,
    MDT_EFFECTIVE_DPI,
};

/// Declares per-monitor awareness. Must be the first thing main does.
pub fn declare() {
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
}

/// How the current thread's awareness reads back, for the log.
pub fn describe() -> &'static str {
    unsafe {
        match GetAwarenessFromDpiAwarenessContext(GetThreadDpiAwarenessContext()) {
            DPI_AWARENESS_PER_MONITOR_AWARE => "per-monitor",
            DPI_AWARENESS_SYSTEM_AWARE => "system-aware",
            DPI_AWARENESS_UNAWARE => "UNAWARE",
            _ => "unknown",
        }
    }
}

/// Every monitor, where it sits and at what scaling, on one line.
///
/// This is the condition the capture depends on and the one a single-monitor
/// desk cannot reproduce: monitors at different scaling are what makes the
/// coordinates of a mouse message and the pixels of the still stop agreeing. Written at every capture, so a piece that comes out
/// wrong somewhere else can be read back rather than guessed at.
///
/// A negative left is a monitor placed left of the primary one - the arrangement
/// where a mismatch eats the left edge of the capture.
pub fn monitors() -> String {
    thread_local! {
        static FOUND: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }

    unsafe extern "system" fn visit(
        monitor: HMONITOR,
        _dc: HDC,
        _clip: *mut RECT,
        _data: LPARAM,
    ) -> BOOL {
        let mut info: MONITORINFOEXW = std::mem::zeroed();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(monitor, &mut info as *mut _ as *mut MONITORINFO) == 0 {
            return 1;
        }
        let mut dpi_x = 0u32;
        let mut dpi_y = 0u32;
        // Effective DPI: the scaling actually in force on that monitor, which is
        // what the person set, not what the panel can do.
        let dpi = if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) == 0 {
            dpi_x
        } else {
            0
        };
        let r = info.monitorInfo.rcMonitor;
        // MONITORINFOF_PRIMARY
        let primary = if info.monitorInfo.dwFlags & 1 == 1 { "*" } else { "" };
        FOUND.with(|f| {
            f.borrow_mut().push(format!(
                "{}{}x{} at {},{} @{}",
                primary,
                r.right - r.left,
                r.bottom - r.top,
                r.left,
                r.top,
                dpi
            ))
        });
        1
    }

    FOUND.with(|f| f.borrow_mut().clear());
    unsafe {
        EnumDisplayMonitors(std::ptr::null_mut(), std::ptr::null(), Some(visit), 0);
    }
    FOUND.with(|f| {
        let found = f.borrow();
        let mixed = found
            .iter()
            .filter_map(|m| m.rsplit_once('@').map(|(_, dpi)| dpi.to_string()))
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            > 1;
        format!(
            "{} monitor [{}]{}",
            found.len(),
            found.join(" | "),
            if mixed { " MIXED SCALING" } else { "" }
        )
    })
}
