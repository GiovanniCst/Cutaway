// The frozen screen, dimmed, with a rectangle drawn on it.
//
// One window over every monitor, in physical pixels, painted from a still taken
// the instant the shortcut was pressed: nothing moves under the hand, and the
// overlay cannot photograph itself. Left button drags, a click without a drag
// clears, Esc or the right button cancels.
//
// The still and its dimmed copy are DIB sections, so a frame is one BitBlt of
// the dimmed copy plus one of the bright rectangle out of the original - no
// alpha fill per frame, and no drawing library on the path that has to be short.
//
// There is no message pump of its own here. The agent has one, and the selection
// has to stay on the glass after it is made, while the editor comes to take it:
// a nested pump would end the moment the mouse came up.

use std::cell::RefCell;
use std::time::Instant;

use windows_sys::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows_sys::Win32::Graphics::Dwm::DwmFlush;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::GetDpiForSystem;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, SetFocus, VK_ESCAPE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::paths;
use crate::strings::t;
use crate::wide::wide;

/// Posted to the agent's window once the overlay is on the glass: the moment the
/// editor should be started, so its loading overlaps the time spent aiming.
pub const WM_OVERLAY_UP: u32 = WM_APP + 2;

/// Posted when the person has chosen, cancelled, or there was nothing to cut.
pub const WM_OVERLAY_DONE: u32 = WM_APP + 3;

/// Below this edge length the drag counts as a slip, not a selection.
const MINIMUM_EDGE: i32 = 4;

/// The veil: what the C# side wrote as alpha 140 over black, which leaves
/// (255-140)/255 of every channel. Applied once, into the dimmed copy.
const VEIL_KEEP: u32 = 255 - 140;

/// The bright colour of the selection's trim, as GDI wants it: 0x00BBGGRR.
const MARK: COLORREF = 0x00E8_D28C; // 140, 210, 232 in RGB

/// Not quite zero: a screen that is black to the eye does not have to grab as
/// 0,0,0. Measured, a window painted pure black comes back as 5,5,5 to 9,9,9:
/// the display's colour handling sits in between. Well under
/// any real content, including the darkest theme this app ships.
const NEARLY_BLACK: u32 = 16;

pub enum Outcome {
    /// The piece, as RGBA, with the rectangle it was cut from in screen coordinates.
    Picked { width: u32, height: u32, rgba: Vec<u8>, rect: RECT },
    Cancelled,
    /// The screen came back black: protected content, or the secure desktop.
    Blank,
    /// The desktop refused to be read at all - a session being locked or
    /// switched, a secure desktop up, a remote session losing its console. A
    /// different thing from a black screen, and worth saying differently.
    Unreadable,
}

/// A DIB section: an HBITMAP whose pixels are also a slice we can write.
struct Surface {
    bitmap: HBITMAP,
    pixels: *mut u32,
    width: i32,
    height: i32,
}

impl Surface {
    fn new(dc: HDC, width: i32, height: i32) -> Option<Surface> {
        unsafe {
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = width;
            // Negative: top-down, so row 0 is the top of the screen and the
            // arithmetic below reads the way the screen looks.
            info.bmiHeader.biHeight = -height;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = BI_RGB;
            let mut pixels: *mut core::ffi::c_void = std::ptr::null_mut();
            let bitmap =
                CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut pixels, std::ptr::null_mut(), 0);
            if bitmap.is_null() || pixels.is_null() {
                return None;
            }
            Some(Surface { bitmap, pixels: pixels as *mut u32, width, height })
        }
    }

    fn as_slice(&self) -> &[u32] {
        unsafe { std::slice::from_raw_parts(self.pixels, (self.width * self.height) as usize) }
    }

    fn as_mut_slice(&mut self) -> &mut [u32] {
        unsafe { std::slice::from_raw_parts_mut(self.pixels, (self.width * self.height) as usize) }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe { DeleteObject(self.bitmap as _) };
    }
}

struct State {
    hwnd: HWND,
    owner: HWND,
    /// The virtual screen in physical pixels. Its origin can be negative, when a
    /// monitor sits left of or above the primary one; every coordinate below is
    /// client-relative, which is that origin moved to 0,0.
    screen: RECT,
    still: Option<Surface>,
    dimmed: Option<Surface>,
    anchor: Option<POINT>,
    rect: RECT,
    taken: bool,
    waiting: bool,
    outcome: Option<Outcome>,
    first_paint: bool,
    clock: Option<Instant>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State {
        hwnd: std::ptr::null_mut(),
        owner: std::ptr::null_mut(),
        screen: RECT { left: 0, top: 0, right: 0, bottom: 0 },
        still: None,
        dimmed: None,
        anchor: None,
        rect: RECT { left: 0, top: 0, right: 0, bottom: 0 },
        taken: false,
        waiting: false,
        outcome: None,
        first_paint: false,
        clock: None,
    });
}

const CLASS_NAME: &str = "CutawayOverlay";

/// Registers the window class once per process.
pub fn register_class() {
    unsafe {
        // Bound to a name: RegisterClassW keeps this pointer, so the buffer has
        // to outlive the call rather than the statement.
        let class_name = wide(CLASS_NAME);
        let class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: GetModuleHandleW(std::ptr::null()),
            hIcon: std::ptr::null_mut(),
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_CROSS),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };
        RegisterClassW(&class);
    }
}

/// Freezes the screen and comes up over it. False when there was nothing to cut,
/// in which case the caller has already been told through WM_OVERLAY_DONE.
pub fn begin(owner: HWND) -> bool {
    let clock = Instant::now();
    let screen = virtual_screen();
    let width = screen.right - screen.left;
    let height = screen.bottom - screen.top;

    let Some((still, mut dimmed)) = grab_screen(&screen) else {
        paths::append("screen grab failed");
        report(owner, Outcome::Unreadable);
        return false;
    };

    // One pass over the pixels does both jobs the C# side did in two: it builds
    // the dimmed copy and answers whether anything on the screen was lit.
    let mut lit = false;
    {
        let source = still.as_slice();
        let target = dimmed.as_mut_slice();
        for (i, &pixel) in source.iter().enumerate() {
            let b = pixel & 0xFF;
            let g = (pixel >> 8) & 0xFF;
            let r = (pixel >> 16) & 0xFF;
            if !lit && (r > NEARLY_BLACK || g > NEARLY_BLACK || b > NEARLY_BLACK) {
                lit = true;
            }
            target[i] =
                ((r * VEIL_KEEP / 255) << 16) | ((g * VEIL_KEEP / 255) << 8) | (b * VEIL_KEEP / 255);
        }
    }
    if !lit {
        // Protected content, or the secure desktop over everything. Cutting a
        // black rectangle and saying nothing is indistinguishable from a broken app.
        report(owner, Outcome::Blank);
        return false;
    }
    let grab_ms = clock.elapsed().as_secs_f64() * 1000.0;

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            wide(CLASS_NAME).as_ptr(),
            wide("Cutaway").as_ptr(),
            WS_POPUP,
            screen.left,
            screen.top,
            width,
            height,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    };
    if hwnd.is_null() {
        report(owner, Outcome::Cancelled);
        return false;
    }

    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.hwnd = hwnd;
        s.owner = owner;
        s.screen = screen;
        s.still = Some(still);
        s.dimmed = Some(dimmed);
        s.anchor = None;
        s.rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        s.taken = false;
        s.waiting = false;
        s.outcome = None;
        s.first_paint = true;
        s.clock = Some(clock);
    });

    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
        SetFocus(hwnd);
        // Painted now rather than at the pump's leisure: the whole point is that
        // the screen freezes before the hand has moved.
        UpdateWindow(hwnd);
    }

    paths::append(&format!(
        "overlay up: screen {}x{} at {},{}, dpi {}, {}, grab+veil {:.0} ms, on glass {:.0} ms",
        width,
        height,
        screen.left,
        screen.top,
        crate::dpi::describe(),
        crate::dpi::monitors(),
        grab_ms,
        clock.elapsed().as_secs_f64() * 1000.0
    ));
    true
}

/// Marks the selection as handed over: it stays on screen, labelled, while the
/// editor comes to take it.
pub fn show_waiting() {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.waiting = true;
        if !s.hwnd.is_null() {
            unsafe { InvalidateRect(s.hwnd, std::ptr::null(), 0) };
        }
    });
}

/// Takes the overlay down and lets the two full-screen surfaces go.
pub fn finish() {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if !s.hwnd.is_null() {
            unsafe { DestroyWindow(s.hwnd) };
            s.hwnd = std::ptr::null_mut();
        }
        s.still = None;
        s.dimmed = None;
        s.outcome = None;
    });
}

/// The outcome of the capture the agent was told about, taken once.
pub fn take_outcome() -> Option<Outcome> {
    STATE.with(|s| s.borrow_mut().outcome.take())
}

/// Cancels a capture in progress: the screen changed under it, or the session
/// went away.
pub fn cancel_from_outside() {
    STATE.with(|s| {
        let hwnd = s.borrow().hwnd;
        if !hwnd.is_null() {
            unsafe { cancel(hwnd) };
        }
    });
}

fn report(owner: HWND, outcome: Outcome) {
    STATE.with(|s| s.borrow_mut().outcome = Some(outcome));
    unsafe { PostMessageW(owner, WM_OVERLAY_DONE, 0, 0) };
}

/// The whole desktop in physical pixels, origin included: with a monitor left of
/// the primary one that origin is negative, and everything else here is relative
/// to it.
pub fn virtual_screen() -> RECT {
    unsafe {
        let left = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let top = GetSystemMetrics(SM_YVIRTUALSCREEN);
        RECT {
            left,
            top,
            right: left + GetSystemMetrics(SM_CXVIRTUALSCREEN),
            bottom: top + GetSystemMetrics(SM_CYVIRTUALSCREEN),
        }
    }
}

/// BitBlt of the whole desktop into a pair of surfaces: the still, and room for
/// its dimmed copy.
fn grab_screen(screen: &RECT) -> Option<(Surface, Surface)> {
    let width = screen.right - screen.left;
    let height = screen.bottom - screen.top;
    if width <= 0 || height <= 0 {
        return None;
    }
    unsafe {
        let desktop = GetDC(std::ptr::null_mut());
        let memory = CreateCompatibleDC(desktop);
        let mut still = Surface::new(desktop, width, height)?;
        let dimmed = Surface::new(desktop, width, height)?;
        let previous = SelectObject(memory, still.bitmap as _);
        let ok = BitBlt(memory, 0, 0, width, height, desktop, screen.left, screen.top, SRCCOPY);
        SelectObject(memory, previous);
        DeleteDC(memory);
        ReleaseDC(std::ptr::null_mut(), desktop);
        if ok == 0 {
            return None;
        }
        // GDI batches: without this the DIB's pixels can still be on their way
        // when the loop below reads them.
        GdiFlush();
        let _ = still.as_mut_slice();
        Some((still, dimmed))
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let dc = BeginPaint(hwnd, &mut ps);
            paint(dc);
            EndPaint(hwnd, &ps);
            0
        }
        // The paint covers every pixel; erasing first only flickers.
        WM_ERASEBKGND => 1,
        WM_LBUTTONDOWN => {
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                if s.taken {
                    return;
                }
                let p = clamp(&s.screen, pointer_in(&s.screen));
                s.anchor = Some(p);
                s.rect = RECT { left: p.x, top: p.y, right: p.x, bottom: p.y };
            });
            SetCapture(hwnd);
            InvalidateRect(hwnd, std::ptr::null(), 0);
            0
        }
        WM_MOUSEMOVE => {
            let redraw = STATE.with(|s| {
                let mut s = s.borrow_mut();
                let (Some(anchor), false) = (s.anchor, s.taken) else {
                    return false;
                };
                let p = clamp(&s.screen, pointer_in(&s.screen));
                s.rect = between(anchor, p);
                true
            });
            if redraw {
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            0
        }
        WM_LBUTTONUP => {
            ReleaseCapture();
            finish_drag(hwnd);
            0
        }
        WM_RBUTTONUP => {
            cancel(hwnd);
            0
        }
        WM_KEYDOWN if w as u32 == VK_ESCAPE as u32 => {
            cancel(hwnd);
            0
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}

/// Where the pointer is, in the still's own coordinates.
///
/// Deliberately not the lParam of the mouse message. A window spanning monitors
/// of different scaling has one DPI at a time, and Windows expresses the mouse
/// coordinates of a message in that DPI - while the still is in physical pixels
/// of the whole desktop. On a mixed-scaling desktop the two stop agreeing, the
/// rectangle is read from the wrong place, and the clamp below trims whatever
/// fell outside: on a monitor left of the primary one, that is the left edge of
/// the capture.
///
/// GetCursorPos has no such ambiguity in a per-monitor-aware process: it answers
/// in physical pixels of the virtual screen, which is exactly what the still is
/// made of. Subtracting the screen origin - negative when a monitor sits left of
/// or above the primary one - gives the offset into it.
fn pointer_in(screen: &RECT) -> POINT {
    unsafe {
        let mut at = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut at) == 0 {
            return POINT { x: 0, y: 0 };
        }
        POINT { x: at.x - screen.left, y: at.y - screen.top }
    }
}

fn clamp(screen: &RECT, p: POINT) -> POINT {
    POINT {
        x: p.x.max(0).min(screen.right - screen.left),
        y: p.y.max(0).min(screen.bottom - screen.top),
    }
}

fn between(a: POINT, b: POINT) -> RECT {
    RECT { left: a.x.min(b.x), top: a.y.min(b.y), right: a.x.max(b.x), bottom: a.y.max(b.y) }
}

unsafe fn cancel(hwnd: HWND) {
    let owner = STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.taken {
            return std::ptr::null_mut();
        }
        s.taken = true;
        s.outcome = Some(Outcome::Cancelled);
        s.owner
    });
    let _ = hwnd;
    if !owner.is_null() {
        PostMessageW(owner, WM_OVERLAY_DONE, 0, 0);
    }
}

unsafe fn finish_drag(hwnd: HWND) {
    let owner = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let (Some(anchor), false) = (s.anchor, s.taken) else {
            return std::ptr::null_mut();
        };
        s.rect = between(anchor, clamp(&s.screen, pointer_in(&s.screen)));
        s.anchor = None;
        if s.rect.right - s.rect.left < MINIMUM_EDGE || s.rect.bottom - s.rect.top < MINIMUM_EDGE {
            // A click without a drag is not a cancel: it clears and leaves the
            // overlay up.
            s.rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            return std::ptr::null_mut();
        }
        s.taken = true;
        s.outcome = Some(cut_out(&s));
        s.owner
    });
    if owner.is_null() {
        InvalidateRect(hwnd, std::ptr::null(), 0);
    } else {
        PostMessageW(owner, WM_OVERLAY_DONE, 0, 0);
    }
}

/// Copies the selected pixels out of the still, as RGBA for the PNG encoder.
///
/// The rectangle is in client coordinates, which are the still's own: both have
/// their origin at the top-left of the virtual screen. The screen rectangle
/// handed back for the log adds that origin again, so a capture on a monitor
/// left of the primary one reads as the negative coordinate it really is.
fn cut_out(s: &State) -> Outcome {
    let still = s.still.as_ref().expect("a still to cut from");
    let width = (s.rect.right - s.rect.left) as u32;
    let height = (s.rect.bottom - s.rect.top) as u32;
    let source = still.as_slice();
    let mut rgba = Vec::with_capacity((width as usize) * (height as usize) * 4);
    for y in 0..height as i32 {
        let row = (s.rect.top + y) * still.width;
        for x in 0..width as i32 {
            let pixel = source[(row + s.rect.left + x) as usize];
            rgba.push(((pixel >> 16) & 0xFF) as u8); // R
            rgba.push(((pixel >> 8) & 0xFF) as u8); // G
            rgba.push((pixel & 0xFF) as u8); // B
            rgba.push(255); // the desktop has no transparency to carry over
        }
    }
    Outcome::Picked {
        width,
        height,
        rgba,
        rect: RECT {
            left: s.screen.left + s.rect.left,
            top: s.screen.top + s.rect.top,
            right: s.screen.left + s.rect.right,
            bottom: s.screen.top + s.rect.bottom,
        },
    }
}

// --- painting ---------------------------------------------------------------

unsafe fn paint(dc: HDC) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let (Some(still), Some(dimmed)) = (s.still.as_ref(), s.dimmed.as_ref()) else {
            return;
        };
        let width = still.width;
        let height = still.height;
        let memory = CreateCompatibleDC(dc);

        // The whole screen, veiled: one blit.
        let previous = SelectObject(memory, dimmed.bitmap as _);
        BitBlt(dc, 0, 0, width, height, memory, 0, 0, SRCCOPY);

        let selected = s.rect.right > s.rect.left && s.rect.bottom > s.rect.top;
        if selected {
            // The inside of the rectangle is the screen as it was, undimmed.
            SelectObject(memory, still.bitmap as _);
            BitBlt(
                dc,
                s.rect.left,
                s.rect.top,
                s.rect.right - s.rect.left,
                s.rect.bottom - s.rect.top,
                memory,
                s.rect.left,
                s.rect.top,
                SRCCOPY,
            );
        }
        SelectObject(memory, previous);
        DeleteDC(memory);

        if selected {
            draw_trim(dc, s.rect);
            draw_badge(dc, &s);
        } else if !s.taken {
            draw_prompt(dc, &s);
        }

        if s.first_paint {
            s.first_paint = false;
            // Waits for the frame to actually reach the glass, so the figure in
            // the log is when the person could see it, not when we asked.
            DwmFlush();
            if !s.owner.is_null() {
                PostMessageW(s.owner, WM_OVERLAY_UP, 0, 0);
            }
        }
    });
}

unsafe fn draw_trim(dc: HDC, r: RECT) {
    let pen = CreatePen(PS_SOLID, 2, MARK);
    let previous = SelectObject(dc, pen as _);
    let previous_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
    Rectangle(dc, r.left, r.top, r.right, r.bottom);
    SelectObject(dc, previous_brush);
    SelectObject(dc, previous);
    DeleteObject(pen as _);

    // The corners, in a heavier stroke: they read as a frame without drawing an
    // edge the photograph has not.
    const ARM: i32 = 14;
    let thick = CreatePen(PS_SOLID, 4, MARK);
    let previous = SelectObject(dc, thick as _);
    for (a, b, c) in [
        ((r.left, r.top + ARM), (r.left, r.top), (r.left + ARM, r.top)),
        ((r.right - ARM, r.top), (r.right, r.top), (r.right, r.top + ARM)),
        ((r.left, r.bottom - ARM), (r.left, r.bottom), (r.left + ARM, r.bottom)),
        ((r.right - ARM, r.bottom), (r.right, r.bottom), (r.right, r.bottom - ARM)),
    ] {
        let points =
            [POINT { x: a.0, y: a.1 }, POINT { x: b.0, y: b.1 }, POINT { x: c.0, y: c.1 }];
        Polyline(dc, points.as_ptr(), 3);
    }
    SelectObject(dc, previous);
    DeleteObject(thick as _);
}

unsafe fn draw_badge(dc: HDC, s: &State) {
    let label = if s.waiting {
        t("Cutaway\u{2026}")
    } else {
        format!("{} \u{d7} {} px", s.rect.right - s.rect.left, s.rect.bottom - s.rect.top)
    };
    let font = mono_font();
    let previous = SelectObject(dc, font as _);
    let text = wide(&label);
    let count = (text.len() - 1) as i32;
    let mut size = SIZE { cx: 0, cy: 0 };
    GetTextExtentPoint32W(dc, text.as_ptr(), count, &mut size);

    let mut box_rect = RECT {
        left: s.rect.left,
        top: s.rect.bottom + 8,
        right: s.rect.left + size.cx + 12,
        bottom: s.rect.bottom + 8 + size.cy + 6,
    };
    // Under the selection, unless that would fall off the bottom of the screen.
    let height = s.screen.bottom - s.screen.top;
    let width = s.screen.right - s.screen.left;
    if box_rect.bottom > height - 4 {
        let tall = box_rect.bottom - box_rect.top;
        box_rect.bottom = s.rect.bottom - 6;
        box_rect.top = box_rect.bottom - tall;
    }
    if box_rect.right > width - 4 {
        let across = box_rect.right - box_rect.left;
        box_rect.right = width - 4;
        box_rect.left = box_rect.right - across;
    }

    let back = CreateSolidBrush(0x0000_0000);
    FillRect(dc, &box_rect, back);
    DeleteObject(back as _);
    SetBkMode(dc, TRANSPARENT as i32);
    SetTextColor(dc, 0x00FF_FFFF);
    TextOutW(dc, box_rect.left + 6, box_rect.top + 3, text.as_ptr(), count);
    SelectObject(dc, previous);
    DeleteObject(font as _);
}

unsafe fn draw_prompt(dc: HDC, s: &State) {
    let hint = t("Drag a rectangle over what you want");
    let opens = format!(
        "{} Esc {}",
        t("it opens in Cutaway when you let go \u{b7}"),
        t("to leave the screen alone")
    );
    let mono = mono_font();
    let text_font = ui_font();

    let previous = SelectObject(dc, mono as _);
    let hint_w = wide(&hint);
    let hint_n = (hint_w.len() - 1) as i32;
    let mut hint_size = SIZE { cx: 0, cy: 0 };
    GetTextExtentPoint32W(dc, hint_w.as_ptr(), hint_n, &mut hint_size);
    SelectObject(dc, text_font as _);
    let opens_w = wide(&opens);
    let opens_n = (opens_w.len() - 1) as i32;
    let mut opens_size = SIZE { cx: 0, cy: 0 };
    GetTextExtentPoint32W(dc, opens_w.as_ptr(), opens_n, &mut opens_size);

    let box_w = hint_size.cx.max(opens_size.cx) + 80;
    let box_h = hint_size.cy + opens_size.cy + 56;
    // Centred on the primary monitor, which is where the eye is. In client
    // coordinates, so the virtual screen's own origin comes off first.
    let left = -s.screen.left + (GetSystemMetrics(SM_CXSCREEN) - box_w) / 2;
    let top = -s.screen.top + (GetSystemMetrics(SM_CYSCREEN) - box_h) / 2;
    let box_rect = RECT { left, top, right: left + box_w, bottom: top + box_h };

    let back = CreateSolidBrush(0x0022_1F1E); // #1E1F22, the app's own dark ground
    FillRect(dc, &box_rect, back);
    DeleteObject(back as _);
    draw_trim(dc, box_rect);

    SetBkMode(dc, TRANSPARENT as i32);
    SetTextColor(dc, 0x00FF_FFFF);
    SelectObject(dc, mono as _);
    TextOutW(dc, left + (box_w - hint_size.cx) / 2, top + 24, hint_w.as_ptr(), hint_n);
    SetTextColor(dc, 0x00AA_AAAA);
    SelectObject(dc, text_font as _);
    TextOutW(
        dc,
        left + (box_w - opens_size.cx) / 2,
        top + 24 + hint_size.cy + 8,
        opens_w.as_ptr(),
        opens_n,
    );

    SelectObject(dc, previous);
    DeleteObject(mono as _);
    DeleteObject(text_font as _);
}

unsafe fn mono_font() -> HFONT {
    font("Consolas", 11)
}

unsafe fn ui_font() -> HFONT {
    font("Segoe UI", 10)
}

/// A font asked for in points, like the C# side did, rather than in pixels.
///
/// WinForms scaled a point size by the process DPI on its own; GDI does not. On
/// a 150% display a size written straight into CreateFontW as pixels comes out
/// two thirds of the size it should be, which is exactly how it looked.
///
/// The system DPI is the primary monitor's, which is where the prompt is centred
/// and where the eye is; a capture is over in a second, so following the pointer
/// across monitors of different scaling would buy nothing.
unsafe fn font(name: &str, points: i32) -> HFONT {
    let dpi = GetDpiForSystem() as i32;
    let height = points * dpi / 72;
    CreateFontW(
        -height,
        0,
        0,
        0,
        FW_NORMAL as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET as u32,
        OUT_DEFAULT_PRECIS as u32,
        CLIP_DEFAULT_PRECIS as u32,
        CLEARTYPE_QUALITY as u32,
        (DEFAULT_PITCH | FF_DONTCARE) as u32,
        wide(name).as_ptr(),
    )
}
