// The resident agent: one instance, two shortcuts, a tray icon, and the handoff
// of a captured piece to the editor.
//
// Everything runs on one thread with one message pump. The two named events are
// waited on by threads of their own, which do nothing but post a message here:
// that keeps every piece of state below single-threaded, and the state machine
// honest.

use std::cell::RefCell;
use std::path::PathBuf;
use std::process::Child;
use std::time::Instant;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::RemoteDesktop::{
    WTSRegisterSessionNotification, WTSUnRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, VK_SNAPSHOT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use windows_sys::Win32::UI::WindowsAndMessaging::{PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND};

use crate::handoff;
use crate::overlay::{self, Outcome, WM_OVERLAY_DONE, WM_OVERLAY_UP};
use crate::paths;
use crate::quit;
use crate::strings::t;
use crate::tray::{
    self, Tray, ID_ABOUT, ID_AUTOSTART, ID_CAPTURE, ID_LOCATE, ID_OPEN, ID_QUIT,
    ID_UNINSTALL, WM_TRAY,
};
use crate::wide::wide;
use crate::{autostart, uninstall};

/// Posted by the thread waiting on the quit event.
const WM_QUIT_REQUESTED: u32 = WM_APP + 4;
/// Posted by the thread waiting on the capture event.
const WM_CAPTURE_REQUESTED: u32 = WM_APP + 5;

const TIMER_CONSUME: usize = 1;

/// How long the selection stays on screen for an editor that never shows up.
///
/// Only ever reached by an editor that is hung: one that died is noticed at once,
/// in well under a second. So this has to clear the slowest honest start rather
/// than the usual one - measured, the portable takes 3.4 to 4.5 seconds to open,
/// and it is launched when the overlay appears, so a quick drag leaves almost all
/// of that still to run after the mouse comes up. Five seconds would have thrown
/// away good captures on a slower machine, and blamed the editor for closing.
const CONSUME_TIMEOUT_MS: u128 = 15_000;

/// Between two notes about being busy: pressing four times should not produce
/// four balloons.
const BUSY_NOTE_SECONDS: u64 = 3;

/// What the agent is doing. A new capture starts only from Idle: while a piece is
/// waiting to be taken the overlay is already down, and without this a second
/// shortcut would start a capture that the previous one's timer then tears down.
#[derive(PartialEq, Clone, Copy)]
enum Stage {
    Idle,
    Picking,
    Handing,
}

struct Agent {
    hwnd: HWND,
    tray: Option<Tray>,
    stage: Stage,
    /// The capture in progress: its folder, the editor started for it, and
    /// whether the folder belongs to a caller rather than to us.
    dir: Option<PathBuf>,
    editor: Option<Child>,
    from_request: bool,
    picked_at: Option<Instant>,
    last_busy_note: Option<Instant>,
}

thread_local! {
    static AGENT: RefCell<Agent> = RefCell::new(Agent {
        hwnd: std::ptr::null_mut(),
        tray: None,
        stage: Stage::Idle,
        dir: None,
        editor: None,
        from_request: false,
        picked_at: None,
        last_busy_note: None,
    });
}

const CLASS_NAME: &str = "CutawayAgentWindow";

/// Runs the agent until it is asked to leave.
pub fn run() {
    paths::trim_log();
    quit::record_self();
    paths::sweep_captures();
    autostart::apply();
    overlay::register_class();

    let hwnd = create_window();
    if hwnd.is_null() {
        paths::append("could not create the agent window");
        return;
    }

    let tray = Tray::new(hwnd);
    AGENT.with(|a| {
        let mut a = a.borrow_mut();
        a.hwnd = hwnd;
        a.tray = Some(tray);
    });

    let ctrl_ok = unsafe {
        RegisterHotKey(hwnd, 1, MOD_CONTROL | MOD_NOREPEAT, VK_SNAPSHOT as u32) != 0
    };
    let altgr_ok = unsafe {
        RegisterHotKey(hwnd, 2, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, VK_SNAPSHOT as u32) != 0
    };

    // Held for as long as the agent runs: dropping either handle would close the
    // named event and let a second agent create a fresh one.
    let raw = hwnd as usize;
    let _quit_watch = quit::watch(move || unsafe {
        PostMessageW(raw as HWND, WM_QUIT_REQUESTED, 0, 0);
    });
    let _capture_watch = quit::watch_captures(move || unsafe {
        PostMessageW(raw as HWND, WM_CAPTURE_REQUESTED, 0, 0);
    });

    unsafe { WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) };

    AGENT.with(|a| {
        let mut a = a.borrow_mut();
        if let Some(tray) = a.tray.as_mut() {
            if !paths::introduced().exists() {
                paths::write_line(&paths::introduced(), "1");
                tray.balloon(
                    &t("Cutaway is here"),
                    &t("Press Ctrl+PrtSc or AltGr+PrtSc to cut a piece of the screen. Quit from this icon's menu."),
                );
            }
            tray::report_shortcuts(tray, ctrl_ok, altgr_ok);
        }
    });
    paths::append(&format!(
        "started; ctrl={} altgr={} session={} dpi={}",
        ctrl_ok,
        altgr_ok,
        paths::session(),
        crate::dpi::describe()
    ));

    unsafe {
        let mut message: MSG = std::mem::zeroed();
        while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        UnregisterHotKey(hwnd, 1);
        UnregisterHotKey(hwnd, 2);
        WTSUnRegisterSessionNotification(hwnd);
    }

    // The tray icon goes before the process does, or its ghost stays in the
    // notification area until something makes Explorer look again.
    AGENT.with(|a| a.borrow_mut().tray = None);
    let _ = std::fs::remove_file(paths::pid_file());
}

fn create_window() -> HWND {
    unsafe {
        // Bound to a name: RegisterClassW keeps this pointer, so the buffer has
        // to outlive the call rather than the statement.
        let class_name = wide(CLASS_NAME);
        let class = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: GetModuleHandleW(std::ptr::null()),
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };
        RegisterClassW(&class);
        // A message-only window would not receive WM_WTSSESSION_CHANGE or the
        // power broadcasts, so this is an ordinary window that is never shown.
        CreateWindowExW(
            0,
            wide(CLASS_NAME).as_ptr(),
            wide("Cutaway").as_ptr(),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match msg {
        WM_HOTKEY => {
            start_capture(None);
            0
        }
        WM_CAPTURE_REQUESTED => {
            // One event can stand for more than one request: they are read together.
            let wanted = paths::take_requests();
            for (i, dir) in wanted.iter().enumerate() {
                if i == 0 {
                    start_capture(Some(PathBuf::from(dir)));
                } else {
                    // One at a time; the others hear back at once.
                    handoff::write_cancel(std::path::Path::new(dir));
                }
            }
            0
        }
        WM_OVERLAY_UP => {
            on_overlay_up();
            0
        }
        WM_OVERLAY_DONE => {
            on_overlay_done();
            0
        }
        WM_TIMER if w == TIMER_CONSUME => {
            on_consume_tick();
            0
        }
        WM_TRAY => {
            match tray::tray_event(l) {
                WM_RBUTTONUP | WM_CONTEXTMENU => {
                    AGENT.with(|a| {
                        if let Some(tray) = a.borrow().tray.as_ref() {
                            tray.show_menu();
                        }
                    });
                }
                WM_LBUTTONDBLCLK => open_editor(),
                _ => {}
            }
            0
        }
        WM_COMMAND => {
            match tray::menu_id(w) {
                ID_CAPTURE => start_capture(None),
                ID_LOCATE => {
                    crate::locate::ask();
                }
                ID_ABOUT => crate::about::show(),
                ID_OPEN => open_editor(),
                ID_AUTOSTART => autostart::set(!autostart::enabled()),
                ID_UNINSTALL => {
                    if uninstall::run() {
                        PostMessageW(hwnd, WM_CLOSE, 0, 0);
                    }
                }
                ID_QUIT => {
                    PostMessageW(hwnd, WM_CLOSE, 0, 0);
                }
                _ => {}
            }
            0
        }
        // The screen under the overlay is no longer the screen that was frozen:
        // the desktop changed shape, the session locked or was disconnected, or
        // the machine woke up with a still that is hours old.
        WM_DISPLAYCHANGE => {
            overlay::cancel_from_outside();
            0
        }
        WM_WTSSESSION_CHANGE => {
            const WTS_SESSION_LOCK: usize = 0x7;
            const WTS_REMOTE_DISCONNECT: usize = 0x4;
            if w == WTS_SESSION_LOCK || w == WTS_REMOTE_DISCONNECT {
                overlay::cancel_from_outside();
            }
            0
        }
        WM_POWERBROADCAST => {
            if w as u32 == PBT_APMRESUMESUSPEND || w as u32 == PBT_APMRESUMEAUTOMATIC {
                overlay::cancel_from_outside();
            }
            1
        }
        WM_QUIT_REQUESTED | WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}

fn start_capture(request_dir: Option<PathBuf>) {
    let busy = AGENT.with(|a| a.borrow().stage != Stage::Idle);
    if busy {
        // Busy: either the overlay is up, or a piece is still waiting to be
        // taken. A caller waiting on its folder gets an answer straight away.
        match request_dir {
            Some(dir) => handoff::write_cancel(&dir),
            None => note_busy(),
        }
        return;
    }
    let hwnd = AGENT.with(|a| {
        let mut a = a.borrow_mut();
        a.stage = Stage::Picking;
        a.from_request = request_dir.is_some();
        a.dir = request_dir;
        a.editor = None;
        a.hwnd
    });
    if !overlay::begin(hwnd) {
        // begin has already reported through WM_OVERLAY_DONE; nothing to add.
    }
}

/// The overlay is already down when this happens, so nothing on screen explains
/// why the shortcut did nothing. While it is still up, the screen says it itself.
fn note_busy() {
    AGENT.with(|a| {
        let mut a = a.borrow_mut();
        if a.stage != Stage::Handing {
            return;
        }
        let recent = a
            .last_busy_note
            .map(|at| at.elapsed().as_secs() < BUSY_NOTE_SECONDS)
            .unwrap_or(false);
        if recent {
            return;
        }
        a.last_busy_note = Some(Instant::now());
        if let Some(tray) = a.tray.as_mut() {
            tray.balloon(
                "Cutaway",
                &t("The last capture is still on its way to the editor. Try again in a moment."),
            );
        }
    });
}

/// The overlay is on the glass: now, and not before, the editor gets started.
fn on_overlay_up() {
    let from_request = AGENT.with(|a| a.borrow().from_request);
    if from_request {
        return;
    }
    let Some(editor) = handoff::resolve_editor() else {
        AGENT.with(|a| {
            if let Some(tray) = a.borrow_mut().tray.as_mut() {
                tray.balloon(
                    &t("Cutaway not found"),
                    &t("The editor is not where the agent expected it. Start Cutaway once and try again."),
                );
            }
        });
        overlay::cancel_from_outside();
        return;
    };
    let dir = match paths::new_capture_dir() {
        Ok(dir) => dir,
        Err(exc) => {
            paths::append(&format!("could not make a capture folder: {}", exc));
            overlay::cancel_from_outside();
            return;
        }
    };
    match handoff::start_editor(&editor, &dir) {
        Ok(child) => AGENT.with(|a| {
            let mut a = a.borrow_mut();
            a.dir = Some(dir);
            a.editor = Some(child);
        }),
        Err(exc) => {
            paths::append(&format!("editor start failed: {}", exc));
            AGENT.with(|a| {
                if let Some(tray) = a.borrow_mut().tray.as_mut() {
                    tray.balloon(&t("Cutaway not found"), &exc.to_string());
                }
            });
            overlay::cancel_from_outside();
        }
    }
}

fn on_overlay_done() {
    let Some(outcome) = overlay::take_outcome() else { return };
    match outcome {
        Outcome::Picked { width, height, rgba, rect } => on_picked(width, height, &rgba, rect),
        Outcome::Cancelled => on_cancelled(),
        Outcome::Blank => on_blank(),
        Outcome::Unreadable => on_unreadable(),
    }
}

fn on_picked(
    width: u32,
    height: u32,
    rgba: &[u8],
    rect: windows_sys::Win32::Foundation::RECT,
) {
    let (dir, from_request, hwnd) =
        AGENT.with(|a| {
            let a = a.borrow();
            (a.dir.clone(), a.from_request, a.hwnd)
        });
    let Some(dir) = dir else {
        overlay::finish();
        AGENT.with(|a| a.borrow_mut().stage = Stage::Idle);
        return;
    };
    // First, and before the handoff: this is what makes the piece usable in the
    // seconds the editor is still starting.
    let copied = crate::clip::put(width, height, rgba);
    if let Err(exc) = handoff::write_piece(&dir, width, height, rgba) {
        paths::append(&format!("could not write the piece: {}", exc));
    }
    // The rectangle is logged in screen coordinates, negatives included: a
    // capture that came out wrong on a second monitor can be read back from here.
    paths::append(&format!(
        "picked {}x{} at {},{}, clipboard {}",
        width,
        height,
        rect.left,
        rect.top,
        if copied { "ok" } else { "NO" }
    ));
    if from_request {
        overlay::finish();
        AGENT.with(|a| {
            let mut a = a.borrow_mut();
            a.dir = None;
            a.stage = Stage::Idle;
        });
        return;
    }
    AGENT.with(|a| {
        let mut a = a.borrow_mut();
        a.picked_at = Some(Instant::now());
        a.stage = Stage::Handing;
    });
    overlay::show_waiting();
    unsafe { SetTimer(hwnd, TIMER_CONSUME, 50, None) };
}

fn on_cancelled() {
    let (dir, from_request, hwnd) = AGENT.with(|a| {
        let a = a.borrow();
        (a.dir.clone(), a.from_request, a.hwnd)
    });
    if let Some(dir) = dir.as_ref() {
        handoff::write_cancel(dir);
    }
    overlay::finish();
    if dir.is_some() && !from_request {
        // The editor, if it got started, sees the file and leaves; the folder
        // goes after it. Until then this counts as busy: the overlay is down, but
        // the capture is not over.
        AGENT.with(|a| {
            let mut a = a.borrow_mut();
            a.picked_at = Some(Instant::now());
            a.stage = Stage::Handing;
        });
        unsafe { SetTimer(hwnd, TIMER_CONSUME, 50, None) };
    } else {
        AGENT.with(|a| {
            let mut a = a.borrow_mut();
            a.dir = None;
            a.stage = Stage::Idle;
        });
    }
    paths::append("cancelled");
}

/// The desktop could not be read: not a black screen, and not something the
/// person did. Saying the wrong one sends them looking for a cause that is not
/// there.
fn on_unreadable() {
    AGENT.with(|a| {
        let mut a = a.borrow_mut();
        if let (Some(dir), true) = (a.dir.as_ref(), a.from_request) {
            handoff::write_cancel(dir);
        }
        if let Some(tray) = a.tray.as_mut() {
            tray.balloon("Cutaway", &t("The screen could not be read just now. Try again."));
        }
        a.dir = None;
        a.editor = None;
        a.stage = Stage::Idle;
    });
    overlay::finish();
}

/// The grab came back black: there is nothing to cut, and saying so beats handing
/// over a black rectangle.
fn on_blank() {
    paths::append("blank screen: nothing to capture");
    AGENT.with(|a| {
        let mut a = a.borrow_mut();
        if let (Some(dir), true) = (a.dir.as_ref(), a.from_request) {
            handoff::write_cancel(dir);
        }
        if let Some(tray) = a.tray.as_mut() {
            tray.balloon(
                "Cutaway",
                &t("There is nothing on the screen to cut: it came back black."),
            );
        }
        a.dir = None;
        a.editor = None;
        a.stage = Stage::Idle;
    });
    overlay::finish();
}

/// Waits for the editor to take the piece (it removes the folder) or to die.
fn on_consume_tick() {
    enum Step {
        Wait,
        Stop { consumed: bool, dead: bool, late: bool, remove: bool, complain: bool },
    }

    let (step, dir, hwnd) = AGENT.with(|a| {
        let mut a = a.borrow_mut();
        let hwnd = a.hwnd;
        if a.stage != Stage::Handing || a.dir.is_none() {
            // Nothing of ours to watch any more: stopping here is what keeps this
            // timer from tearing down a capture that came after.
            return (Step::Stop { consumed: false, dead: false, late: false, remove: false, complain: false }, None, hwnd);
        }
        let dir = a.dir.clone().unwrap();
        let consumed = !dir.exists();
        let cancelled = dir.join("cancel").exists();
        let dead = a
            .editor
            .as_mut()
            .map(|child| matches!(child.try_wait(), Ok(Some(_))))
            .unwrap_or(false);
        let late = a
            .picked_at
            .map(|at| at.elapsed().as_millis() > CONSUME_TIMEOUT_MS)
            .unwrap_or(false);
        if !consumed && !dead && !late && !cancelled {
            return (Step::Wait, Some(dir), hwnd);
        }
        // The editor is still on its way out: give it the moment it needs.
        if cancelled && !dead && !late {
            return (Step::Wait, Some(dir), hwnd);
        }
        let complain = !consumed && (dead || late) && !cancelled;
        let remove = (cancelled && dead) || (!consumed && !cancelled) || (cancelled && late);
        (Step::Stop { consumed, dead, late, remove, complain }, Some(dir), hwnd)
    });

    let Step::Stop { consumed, dead, late, remove, complain } = step else { return };
    unsafe { KillTimer(hwnd, TIMER_CONSUME) };
    let Some(dir) = dir else { return };

    if complain {
        // Two different things happened, and saying the wrong one sends the
        // person looking in the wrong place: a process that started and died is
        // not a process that never showed up.
        let said = if dead {
            t("The editor closed before showing the capture.")
        } else {
            t("The editor did not open the capture.")
        };
        AGENT.with(|a| {
            if let Some(tray) = a.borrow_mut().tray.as_mut() {
                tray.balloon("Cutaway", &said);
            }
        });
    }
    if remove {
        handoff::remove(&dir);
    }
    let waited = AGENT.with(|a| {
        a.borrow().picked_at.map(|at| at.elapsed().as_millis()).unwrap_or(0)
    });
    paths::append(&format!(
        "handoff: consumed={} dead={} late={} after {} ms",
        consumed, dead, late, waited
    ));
    // The editor is about to show its window: it needs the foreground we still hold.
    if consumed {
        handoff::lend_foreground();
    }
    overlay::finish();
    AGENT.with(|a| {
        let mut a = a.borrow_mut();
        a.dir = None;
        a.editor = None;
        a.stage = Stage::Idle;
    });
}

fn open_editor() {
    let Some(editor) = handoff::resolve_editor() else {
        AGENT.with(|a| {
            if let Some(tray) = a.borrow_mut().tray.as_mut() {
                tray.balloon(
                    &t("Cutaway not found"),
                    &t("The editor is not where the agent expected it. Start Cutaway once and try again."),
                );
            }
        });
        return;
    };
    if let Err(exc) = handoff::open_editor(&editor) {
        AGENT.with(|a| {
            if let Some(tray) = a.borrow_mut().tray.as_mut() {
                tray.balloon("Cutaway", &exc.to_string());
            }
        });
    }
}

/// One capture for a caller that owns the folder, then out: what the editor gets
/// when no agent is resident.
pub fn run_once(dir: PathBuf) {
    overlay::register_class();
    let hwnd = create_window();
    if hwnd.is_null() {
        return;
    }
    AGENT.with(|a| {
        let mut a = a.borrow_mut();
        a.hwnd = hwnd;
        a.stage = Stage::Picking;
        a.from_request = true;
        a.dir = Some(dir);
    });
    if !overlay::begin(hwnd) {
        // Nothing to cut: the caller is waiting on an answer, and gets one.
        let dir = AGENT.with(|a| a.borrow().dir.clone());
        if let Some(dir) = dir {
            handoff::write_cancel(&dir);
        }
        return;
    }
    unsafe {
        let mut message: MSG = std::mem::zeroed();
        while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
            if AGENT.with(|a| a.borrow().stage == Stage::Idle) {
                break;
            }
        }
    }
    overlay::finish();
}
