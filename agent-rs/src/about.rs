// What the agent is, said from its own menu.
//
// The agent is the half of Cutaway that a person never opens: it sits in the
// notification area, watches for a key, and hands a rectangle to the editor.
// Somebody who finds it there and does not remember installing it deserves an
// answer that is not a search of the running processes - and the answer has to
// come from the agent itself, because the editor may not be running and, on the
// portable build, may not be installed anywhere findable.
//
// A task dialog rather than a message box, for one reason: a message box cannot
// carry a link, and an address a person has to retype is not one. The two that
// matter - where the source is and who wrote it - are clickable here.

use windows_sys::core::{HRESULT, PCWSTR};
use windows_sys::Win32::Foundation::{HWND, LPARAM, S_OK, WPARAM};
use windows_sys::Win32::UI::Controls::{
    TaskDialogIndirect, TASKDIALOGCONFIG, TDCBF_OK_BUTTON, TDF_ALLOW_DIALOG_CANCELLATION,
    TDF_ENABLE_HYPERLINKS, TDF_USE_HICON_MAIN, TDN_HYPERLINK_CLICKED,
};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::strings::t;
use crate::wide::wide;

const AUTHOR: &str = "Giovanni J. Costantini";
const AUTHOR_URL: &str = "https://costantini.pw";
const PROJECT_URL: &str = "https://github.com/GiovanniCst/Cutaway";
const LICENCE: &str = "Apache 2.0";
const LICENCE_URL: &str = "https://www.apache.org/licenses/LICENSE-2.0";

/// Opens whatever the person clicked, in whatever they use for it.
///
/// The address arrives as the wide string written into the markup below, and is
/// handed to the shell unread: these are three addresses compiled into this
/// binary, not anything a document supplied.
unsafe extern "system" fn clicked(
    _hwnd: HWND,
    notification: i32,
    _w: WPARAM,
    href: LPARAM,
    _data: isize,
) -> HRESULT {
    if notification == TDN_HYPERLINK_CLICKED && href != 0 {
        ShellExecuteW(
            std::ptr::null_mut(),
            wide("open").as_ptr(),
            href as PCWSTR,
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        );
    }
    S_OK
}

/// The dialog: what this is, what the key does, and where it came from.
pub fn show() {
    let title = wide(&t("About Cutaway"));
    let heading = wide(&t("Keeps the Cutaway shortcut working."));
    let body = format!(
        "{}\n\n{}\n{}\n\n{}\n{}",
        crate::strings::f1(
            "Press {0} anywhere to freeze the screen and cut a piece out of it. \
             The piece goes to the clipboard and opens in Cutaway.",
            crate::strings::ctrl(),
        ),
        crate::strings::f1("Version {0}", env!("CARGO_PKG_VERSION")),
        crate::strings::f1("Created by {0}", AUTHOR),
        // Written as markup because the dialog is told to read it as markup:
        // the address is the target, the words are what is shown.
        format!(
            "<a href=\"{}\">{}</a>
<a href=\"{}\">{}</a>",
            PROJECT_URL,
            t("Source code"),
            AUTHOR_URL,
            t("The author's site"),
        ),
        format!(
            "<a href=\"{}\">{}</a>",
            LICENCE_URL,
            crate::strings::f1("Licensed under the {0}", LICENCE),
        ),
    );
    let body = wide(&body);

    let mut config: TASKDIALOGCONFIG = unsafe { std::mem::zeroed() };
    config.cbSize = std::mem::size_of::<TASKDIALOGCONFIG>() as u32;
    config.dwFlags = TDF_ENABLE_HYPERLINKS | TDF_ALLOW_DIALOG_CANCELLATION | TDF_USE_HICON_MAIN;
    config.dwCommonButtons = TDCBF_OK_BUTTON;
    config.pszWindowTitle = title.as_ptr();
    config.pszMainInstruction = heading.as_ptr();
    config.pszContent = body.as_ptr();
    config.Anonymous1.hMainIcon = crate::tray::app_icon();
    config.pfCallback = Some(clicked);

    // Nothing is read back: there is one button and it closes the dialog. A
    // failure here would be a machine without common controls version 6, which
    // the manifest asks for; the answer to that is to say nothing rather than
    // to stop an agent that was only being asked to describe itself.
    unsafe {
        TaskDialogIndirect(
            &config,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
    }
}
