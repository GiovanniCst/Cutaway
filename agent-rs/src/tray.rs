// The icon in the notification area: the only visible sign the agent exists, and
// the only place to switch it off.

use windows_sys::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::autostart;
use crate::strings::{altgr, ctrl, t};
use crate::uninstall;
use crate::wide::{fill, wide};

/// The message the icon sends back to the agent's window.
pub const WM_TRAY: u32 = WM_APP + 1;

pub const ID_CAPTURE: u32 = 1;
pub const ID_OPEN: u32 = 2;
pub const ID_AUTOSTART: u32 = 3;
pub const ID_UNINSTALL: u32 = 4;
pub const ID_QUIT: u32 = 5;
pub const ID_ABOUT: u32 = 6;
pub const ID_LOCATE: u32 = 7;

pub struct Tray {
    hwnd: HWND,
    data: NOTIFYICONDATAW,
    icon: HICON,
    /// A shortcut another program owns, shown where it cannot be missed.
    taken: Vec<String>,
}

impl Tray {
    pub fn new(hwnd: HWND) -> Tray {
        let icon = load_icon();
        let mut data: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = hwnd;
        data.uID = 1;
        data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        data.uCallbackMessage = WM_TRAY;
        data.hIcon = icon;
        fill(&mut data.szTip, "Cutaway");
        unsafe { Shell_NotifyIconW(NIM_ADD, &data) };
        Tray { hwnd, data, icon, taken: Vec::new() }
    }

    /// Best effort: Windows may turn it into a toast, or swallow it under Do Not
    /// Disturb.
    pub fn balloon(&mut self, title: &str, text: &str) {
        self.data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_INFO;
        fill(&mut self.data.szInfoTitle, title);
        fill(&mut self.data.szInfo, text);
        self.data.Anonymous.uTimeout = 8000;
        unsafe { Shell_NotifyIconW(NIM_MODIFY, &self.data) };
        self.data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    }

    /// A shortcut another program owns: said in the menu and in the tooltip.
    pub fn mark_taken(&mut self, shortcut: &str) {
        let note = crate::strings::f1("{0}: taken by another program", shortcut);
        let tip = format!("Cutaway \u{2014} {}", note);
        self.taken.push(note);
        fill(&mut self.data.szTip, &tip);
        unsafe { Shell_NotifyIconW(NIM_MODIFY, &self.data) };
    }

    /// Opens the menu where the pointer is. The two calls around TrackPopupMenu
    /// are what stops the menu from staying up after a click elsewhere.
    pub fn show_menu(&self) {
        unsafe {
            let menu = CreatePopupMenu();
            for note in &self.taken {
                AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, wide(note).as_ptr());
            }
            if !self.taken.is_empty() {
                AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
            }
            AppendMenuW(
                menu,
                MF_STRING,
                ID_CAPTURE as usize,
                wide(&t("Cut a piece of the screen")).as_ptr(),
            );
            AppendMenuW(menu, MF_STRING, ID_OPEN as usize, wide(&t("Open Cutaway")).as_ptr());
            AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
            AppendMenuW(
                menu,
                MF_STRING | if autostart::enabled() { MF_CHECKED } else { MF_UNCHECKED },
                ID_AUTOSTART as usize,
                wide(&t("Start with Windows")).as_ptr(),
            );
            AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
            // Only the portable offers this: the installed copy has an uninstaller.
            if uninstall::offered() {
                AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_UNINSTALL as usize,
                    wide(&t("Remove Cutaway from this computer")).as_ptr(),
                );
            }
            AppendMenuW(
                menu,
                MF_STRING,
                ID_LOCATE as usize,
                wide(&t("Where Cutaway is...")).as_ptr(),
            );
            AppendMenuW(
                menu,
                MF_STRING,
                ID_ABOUT as usize,
                wide(&t("About Cutaway")).as_ptr(),
            );
            AppendMenuW(menu, MF_STRING, ID_QUIT as usize, wide(&t("Quit")).as_ptr());

            let mut where_at = POINT { x: 0, y: 0 };
            GetCursorPos(&mut where_at);
            // Without this the menu will not close when the person clicks away.
            SetForegroundWindow(self.hwnd);
            TrackPopupMenu(
                menu,
                TPM_RIGHTBUTTON,
                where_at.x,
                where_at.y,
                0,
                self.hwnd,
                std::ptr::null(),
            );
            PostMessageW(self.hwnd, WM_NULL, 0, 0);
            DestroyMenu(menu);
        }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        unsafe {
            Shell_NotifyIconW(NIM_DELETE, &self.data);
            if !self.icon.is_null() {
                DestroyIcon(self.icon);
            }
        }
    }
}

/// The application icon, for anything else that wants one - the About dialog.
///
/// A fresh handle each time and not a shared one: the caller owns what it gets
/// and the tray's own icon has an owner already.
pub fn app_icon() -> HICON {
    load_icon()
}

/// The application icon, carried in the executable rather than looked for on
/// disk: the agent has to have a face even when it is the only file left.
fn load_icon() -> HICON {
    const ICO: &[u8] = include_bytes!("../../assets/cutaway.ico");
    match icon_from_ico(ICO) {
        Some(icon) => icon,
        None => unsafe { LoadIconW(std::ptr::null_mut(), IDI_APPLICATION) },
    }
}

/// Picks an image out of an .ico file and makes an HICON of it.
///
/// CreateIconFromResourceEx wants the image on its own, not the file: the six
/// byte directory header and the sixteen byte entries in front of it have to be
/// read and stepped over first.
fn icon_from_ico(bytes: &[u8]) -> Option<HICON> {
    if bytes.len() < 6 {
        return None;
    }
    let count = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
    let mut best: Option<(u32, u32)> = None; // (offset, length)
    let mut best_width = 0u32;
    for i in 0..count {
        let at = 6 + i * 16;
        if at + 16 > bytes.len() {
            break;
        }
        // A zero in the width byte means 256.
        let width = if bytes[at] == 0 { 256 } else { bytes[at] as u32 };
        let length = u32::from_le_bytes([bytes[at + 8], bytes[at + 9], bytes[at + 10], bytes[at + 11]]);
        let offset = u32::from_le_bytes([bytes[at + 12], bytes[at + 13], bytes[at + 14], bytes[at + 15]]);
        // The notification area asks for a small icon; 32 is what it scales from
        // best, and anything larger is a waste of the decode.
        let better = match best_width {
            0 => true,
            current => (width <= 32 && width > current) || (current > 32 && width < current && width >= 32),
        };
        if better {
            best_width = width;
            best = Some((offset, length));
        }
    }
    let (offset, length) = best?;
    let start = offset as usize;
    let end = start.checked_add(length as usize)?;
    if end > bytes.len() {
        return None;
    }
    unsafe {
        let icon = CreateIconFromResourceEx(
            bytes[start..end].as_ptr(),
            length,
            1, // an icon, not a cursor
            0x0003_0000,
            0,
            0,
            LR_DEFAULTCOLOR,
        );
        (!icon.is_null()).then_some(icon)
    }
}

/// Which shortcuts were not available, said once at start.
pub fn report_shortcuts(tray: &mut Tray, ctrl_ok: bool, altgr_ok: bool) {
    if !ctrl_ok {
        tray.mark_taken(ctrl());
    }
    if !altgr_ok {
        tray.mark_taken(altgr());
    }
    if !ctrl_ok && !altgr_ok {
        tray.balloon(
            &t("Shortcut taken"),
            &t("Both shortcuts are already used by other programs."),
        );
    } else if !ctrl_ok {
        tray.balloon(
            &t("Shortcut taken"),
            &crate::strings::f2(
                "{0} is already used by another program; {1} still works.",
                ctrl(),
                altgr(),
            ),
        );
    } else if !altgr_ok {
        tray.balloon(
            &t("Shortcut taken"),
            &crate::strings::f2(
                "{0} is already used by another program; {1} still works.",
                altgr(),
                ctrl(),
            ),
        );
    }
}

/// The two halves of the lParam the notification icon sends back.
pub fn tray_event(l: LPARAM) -> u32 {
    (l as u32) & 0xFFFF
}

pub fn menu_id(w: WPARAM) -> u32 {
    (w as u32) & 0xFFFF
}
