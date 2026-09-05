// Capturing something taller than the screen.
//
// A window is photographed, scrolled, photographed again, and the pieces are
// sewn together. The photographing is the easy half; the difficult half is
// knowing by how much the content actually moved, because it is never the amount
// that was asked for: a wheel notch scrolls three lines and a line is not a fixed
// number of pixels, a page may scroll smoothly and land between two positions,
// and anything that animates arrives late.
//
// So the overlap is measured rather than assumed. The bottom band of what is
// already collected is searched for in the new frame, and the offset that
// matches best is where the content went. That also detects the end: when the
// best match is at zero, nothing moved, and the page is as far down as it goes.

use image::RgbaImage;

/// How tall a band to match on. Enough to be distinctive - a few lines of text
/// or part of a picture - and small enough that the search stays cheap.
const BAND: u32 = 80;

/// Below this the two frames are considered the same picture, which means the
/// scroll did nothing and the page has ended.
const SAME: f64 = 2.0;

/// How well two bands agree, as a mean absolute difference per channel. Zero is
/// identical.
fn difference(a: &RgbaImage, a_top: u32, b: &RgbaImage, b_top: u32, height: u32) -> f64 {
    let width = a.width().min(b.width());
    if width == 0 || height == 0 {
        return f64::MAX;
    }
    let mut total = 0u64;
    let mut counted = 0u64;
    // Every fourth row and every third column: the answer is the same to within
    // a rounding error and the search is twelve times cheaper, which matters
    // because this runs for every candidate offset.
    let mut y = 0;
    while y < height {
        let mut x = 0;
        while x < width {
            let one = a.get_pixel(x, a_top + y);
            let two = b.get_pixel(x, b_top + y);
            total += (one[0] as i32 - two[0] as i32).unsigned_abs() as u64;
            total += (one[1] as i32 - two[1] as i32).unsigned_abs() as u64;
            total += (one[2] as i32 - two[2] as i32).unsigned_abs() as u64;
            counted += 3;
            x += 3;
        }
        y += 4;
    }
    if counted == 0 {
        return f64::MAX;
    }
    total as f64 / counted as f64
}

/// By how much the content moved between two frames, in pixels.
///
/// None when nothing moved, which is how the end of a page is recognised.
pub fn shift_between(previous: &RgbaImage, next: &RgbaImage) -> Option<u32> {
    let height = previous.height().min(next.height());
    if height <= BAND {
        return None;
    }
    // The band taken from the bottom of the old frame is looked for in the new
    // one: content moving up is what scrolling down looks like.
    let band_top = height - BAND;
    let mut best = (0u32, f64::MAX);
    // A scroll of less than a few pixels is noise; one of more than most of the
    // screen means the page jumped rather than scrolled, and stitching that
    // would join two unrelated things.
    for offset in 1..(height - BAND) {
        let score = difference(previous, band_top, next, band_top - offset, BAND);
        if score < best.1 {
            best = (offset, score);
        }
    }

    // Did it move at all? Comparing the two frames as they are answers that
    // without trusting the search: if they are the same picture, the best
    // "match" above is meaningless.
    let unmoved = difference(previous, 0, next, 0, height);
    if unmoved < SAME {
        return None;
    }
    // A match that is no better than the frames being identical is not a match.
    if best.1 >= unmoved || best.0 == 0 {
        return None;
    }
    Some(best.0)
}

/// Sews a new frame onto what has been collected.
///
/// Only the part that is new is added: the overlap is already there, and adding
/// it twice is how a stitched screenshot ends up with a repeated line of text.
pub fn stitch(collected: &RgbaImage, next: &RgbaImage, shift: u32) -> RgbaImage {
    let width = collected.width().min(next.width());
    let fresh = shift.min(next.height());
    let mut out = RgbaImage::new(width, collected.height() + fresh);
    for y in 0..collected.height() {
        for x in 0..width {
            out.put_pixel(x, y, *collected.get_pixel(x, y));
        }
    }
    // The new rows are the bottom `shift` rows of the new frame.
    let from = next.height() - fresh;
    for y in 0..fresh {
        for x in 0..width {
            out.put_pixel(x, collected.height() + y, *next.get_pixel(x, from + y));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A picture with distinguishable rows, so a shift is detectable at all.
    fn striped(height: u32, from: u32) -> RgbaImage {
        let mut pixels = RgbaImage::new(200, height);
        for y in 0..height {
            let line = from + y;
            for x in 0..200 {
                // A pattern that varies with the row and along it: a flat colour
                // would match at every offset and prove nothing.
                let value = ((line * 7 + x * 3) % 256) as u8;
                pixels.put_pixel(x, y, image::Rgba([value, (line % 256) as u8, 40, 255]));
            }
        }
        pixels
    }

    #[test]
    fn the_shift_is_found_rather_than_assumed() {
        let first = striped(400, 0);
        // The same content, 120 pixels further down.
        let second = striped(400, 120);
        assert_eq!(shift_between(&first, &second), Some(120));
    }

    #[test]
    fn a_page_that_did_not_move_says_so() {
        let still = striped(400, 0);
        assert_eq!(shift_between(&still, &still.clone()), None);
    }

    #[test]
    fn stitching_does_not_repeat_the_overlap() {
        let first = striped(400, 0);
        let second = striped(400, 120);
        let joined = stitch(&first, &second, 120);
        assert_eq!(joined.height(), 520);
        // Row 400 of the result must be row 400 of the original stream, not a
        // repeat of something already there.
        let whole = striped(520, 0);
        for x in (0..200).step_by(17) {
            assert_eq!(
                joined.get_pixel(x, 450),
                whole.get_pixel(x, 450),
                "colonna {} riga 450",
                x
            );
        }
    }

    #[test]
    fn a_small_scroll_is_still_found() {
        let first = striped(400, 0);
        let second = striped(400, 7);
        assert_eq!(shift_between(&first, &second), Some(7));
    }
}

// --- doing the scrolling ----------------------------------------------------

use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC,
    SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, SRCCOPY,
};
use std::time::Instant;

use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_MOUSE, MOUSEEVENTF_WHEEL, VK_ESCAPE, VK_LBUTTON,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetAncestor, GetClientRect, GetCursorPos, SetCursorPos, SetForegroundWindow, WindowFromPoint,
};
use windows_sys::Win32::Graphics::Gdi::ClientToScreen;

/// One wheel notch, as Windows counts them.
const WHEEL_DELTA: i32 = 120;

/// A photograph of one rectangle of the screen, in physical pixels.
pub fn grab(area: RECT) -> Option<RgbaImage> {
    let width = area.right - area.left;
    let height = area.bottom - area.top;
    if width <= 0 || height <= 0 {
        return None;
    }
    unsafe {
        let screen = GetDC(std::ptr::null_mut());
        let memory = CreateCompatibleDC(screen);
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = width;
        info.bmiHeader.biHeight = -height; // top-down
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let bitmap =
            CreateDIBSection(screen, &info, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
        if bitmap.is_null() || bits.is_null() {
            DeleteDC(memory);
            ReleaseDC(std::ptr::null_mut(), screen);
            return None;
        }
        let previous = SelectObject(memory, bitmap as _);
        let ok = BitBlt(memory, 0, 0, width, height, screen, area.left, area.top, SRCCOPY);
        SelectObject(memory, previous);

        let mut out = None;
        if ok != 0 {
            let count = (width * height) as usize;
            let source = std::slice::from_raw_parts(bits as *const u32, count);
            let mut pixels = RgbaImage::new(width as u32, height as u32);
            for (i, pixel) in source.iter().enumerate() {
                let x = (i % width as usize) as u32;
                let y = (i / width as usize) as u32;
                pixels.put_pixel(
                    x,
                    y,
                    image::Rgba([
                        ((pixel >> 16) & 0xFF) as u8,
                        ((pixel >> 8) & 0xFF) as u8,
                        (pixel & 0xFF) as u8,
                        255,
                    ]),
                );
            }
            out = Some(pixels);
        }
        DeleteObject(bitmap as _);
        DeleteDC(memory);
        ReleaseDC(std::ptr::null_mut(), screen);
        out
    }
}

/// The window under a point, and the rectangle it occupies.
/// What a scrolling capture was aimed at.
pub struct Target {
    /// The top-level window, whose contents are photographed.
    pub window: HWND,
    /// The control the pointer is actually over, which is the thing that
    /// scrolls. In a browser it is an inner surface several levels down, and
    /// posting the wheel to the top-level window instead does nothing.
    pub scrolls: HWND,
    /// Where the contents are on screen.
    pub area: RECT,
}

/// The window under a point, and the rectangle of its *contents*.
///
/// Three things, each of which was wrong before:
///
/// The top-level window, not the control under the pointer. `WindowFromPoint`
/// returns the deepest child, which in a browser is some inner surface whose
/// rectangle is not the page.
///
/// Brought to the front, and given a moment to arrive. The capture reads the
/// screen, so anything overlapping the target ends up inside the picture - a
/// notification, another program's window, a tooltip. Raising it first is what
/// makes the photograph be of the thing that was aimed at.
///
/// And the client area, not the window frame: the border and the drop shadow
/// belong to Windows, not to the page being read.
pub fn window_at(x: i32, y: i32) -> Option<Target> {
    unsafe {
        let deep = WindowFromPoint(POINT { x, y });
        if deep.is_null() {
            return None;
        }
        // GA_ROOT: the top-level window this control belongs to.
        let window = GetAncestor(deep, 2);
        let window = if window.is_null() { deep } else { window };

        SetForegroundWindow(window);
        std::thread::sleep(std::time::Duration::from_millis(250));

        let mut client: RECT = std::mem::zeroed();
        if GetClientRect(window, &mut client) == 0 {
            return None;
        }
        // The client rectangle is in the window's own coordinates; the capture
        // reads the screen, so it has to be put there.
        let mut origin = POINT { x: 0, y: 0 };
        if ClientToScreen(window, &mut origin) == 0 {
            return None;
        }
        Some(Target {
            window,
            scrolls: deep,
            area: RECT {
                left: client.left + origin.x,
                top: client.top + origin.y,
                right: client.right + origin.x,
                bottom: client.bottom + origin.y,
            },
        })
    }
}

pub fn cursor() -> POINT {
    unsafe {
        let mut at = POINT { x: 0, y: 0 };
        GetCursorPos(&mut at);
        at
    }
}

/// Turns the wheel where the pointer is.
///
/// Injected rather than posted. Posting WM_MOUSEWHEEL straight to the window
/// looks like the targeted thing to do and was tried: a browser ignores it -
/// measured on Firefox, which renders into a single window and takes its input
/// another way, the capture still came back holding one screen.
///
/// Injected input goes to whatever holds the focus, which is why 
/// brings the target to the front first. That was the whole defect: the window
/// being photographed was never in front, so the wheel turned somewhere else
/// and the page never moved.
fn turn_wheel(notches: i32) {
    unsafe {
        let mut input: INPUT = std::mem::zeroed();
        input.r#type = INPUT_MOUSE;
        input.Anonymous.mi.dwFlags = MOUSEEVENTF_WHEEL;
        // Negative scrolls down, which is the direction that reveals what is
        // below - the direction a tall page is read in.
        input.Anonymous.mi.mouseData = (-notches * WHEEL_DELTA) as u32;
        SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Why a capture stopped, which is not always because it finished.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stop {
    /// The page would not move any further: the bottom.
    Ended,
    /// Escape was pressed.
    Cancelled,
    /// It went on longer than any page has a right to. An infinitely scrolling
    /// feed has no bottom to find, and without this the capture ran until the
    /// machine ran out of memory.
    TooTall,
}

/// True while Escape is held. Read straight from the keyboard rather than from
/// a window's events: this program has put itself away, so it has no window to
/// receive a key press in - which is why pressing Escape did nothing at all.
fn escape_pressed() -> bool {
    // The high bit is "down now", as distinct from "was pressed since last
    // asked", which is the low one.
    unsafe { (GetAsyncKeyState(VK_ESCAPE as i32) as u16 & 0x8000) != 0 }
}

/// Waits for a click, and says where it landed. None when Escape came first.
///
/// A click rather than a countdown: a countdown makes somebody hurry, and the
/// thing they are hurrying to do - put the pointer on the right window - is the
/// one thing the capture cannot recover from getting wrong.
pub fn wait_for_click(patience: std::time::Duration) -> Option<POINT> {
    let until = Instant::now() + patience;
    // Let go of the button that opened this, or the click that pressed the
    // toolbar button would be taken as the choice of window.
    while Instant::now() < until {
        if unsafe { (GetAsyncKeyState(VK_LBUTTON as i32) as u16 & 0x8000) == 0 } {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    while Instant::now() < until {
        if escape_pressed() {
            return None;
        }
        if unsafe { (GetAsyncKeyState(VK_LBUTTON as i32) as u16 & 0x8000) != 0 } {
            // Down. Wait for it to come up, so the window has had the click too.
            while unsafe { (GetAsyncKeyState(VK_LBUTTON as i32) as u16 & 0x8000) != 0 } {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            return Some(cursor());
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    None
}

/// What a scrolling capture is allowed to do, so it cannot run away.
pub struct Limits {
    /// How many times to scroll before giving up on reaching the end.
    pub frames: usize,
    /// How tall the result may get. A feed that loads more as it scrolls has no
    /// bottom, and something has to say when enough is enough.
    pub tallest: u32,
    /// How many notches per step. Smaller means more overlap to match on and a
    /// slower capture; three is what a wheel click does.
    pub notches: i32,
    /// How long to wait for the content to settle after each scroll. Anything
    /// that animates its scrolling arrives late, and photographing mid-animation
    /// gives a frame that matches nothing.
    pub settle: std::time::Duration,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            // A long article is longer than forty screens' worth of three
            // notches. The end is found by the content not moving, so this is
            // only the guard against a page that scrolls for ever - and a guard
            // that stops halfway down an ordinary page is not a guard, it is a
            // bug wearing one.
            frames: 200,
            // Twenty thousand pixels is about ten screens of a long article,
            // and about 100 MB of picture. Past that a capture is not a
            // screenshot any more.
            tallest: 20_000,
            notches: 3,
            settle: std::time::Duration::from_millis(320),
        }
    }
}

/// Photographs an area, scrolling between shots, and sews the results together.
///
/// The pointer is moved into the area first, because the wheel goes to whatever
/// is under it - and put back afterwards, because moving somebody's pointer and
/// leaving it there is rude.
/// Returns the sewn picture and how many frames went into it. The count is
/// not a statistic: a capture that stopped after one frame looks like a plain
/// screenshot, and without the number nobody can tell that from a page that
/// simply did not scroll.
pub fn capture(
    target: &Target,
    limits: &Limits,
) -> Result<(RgbaImage, usize, Stop), String> {
    let area = target.area;
    let middle = POINT {
        x: (area.left + area.right) / 2,
        y: (area.top + area.bottom) / 2,
    };
    let was = cursor();
    unsafe {
        SetCursorPos(middle.x, middle.y);
    }
    std::thread::sleep(std::time::Duration::from_millis(120));

    let first = grab(area).ok_or(crate::words::w().could_not_photograph)?;
    let mut previous = first.clone();
    // The strips are kept and sewn once at the end. Sewing as it went meant
    // allocating and copying the whole accumulated picture on every single
    // frame - the cost grows with the square of the number of frames, and on a
    // feed that scrolls for ever that is what ran the machine out of memory
    // rather than any one picture being too large.
    let mut tall = first.height();
    let mut strips: Vec<(RgbaImage, u32)> = Vec::new();
    let mut still = 0;
    let mut why = Stop::Ended;

    for _ in 0..limits.frames {
        if escape_pressed() {
            why = Stop::Cancelled;
            break;
        }
        turn_wheel(limits.notches);
        std::thread::sleep(limits.settle);
        let Some(next) = grab(area) else { break };
        match shift_between(&previous, &next) {
            Some(shift) => {
                if tall + shift > limits.tallest {
                    why = Stop::TooTall;
                    break;
                }
                tall += shift;
                strips.push((next.clone(), shift));
                previous = next;
                still = 0;
            }
            None => {
                // Nothing moved. Once can be a frame caught mid-animation or a
                // page that had not finished loading; twice in a row is the
                // bottom. Believing the first one is how a capture came back
                // holding the top of the page and nothing else.
                still += 1;
                if still >= 2 {
                    break;
                }
                std::thread::sleep(limits.settle);
            }
        }
    }

    unsafe {
        SetCursorPos(was.x, was.y);
    }

    let frames = strips.len() + 1;
    let mut collected = RgbaImage::new(first.width(), tall);
    for y in 0..first.height() {
        for x in 0..first.width() {
            collected.put_pixel(x, y, *first.get_pixel(x, y));
        }
    }
    let mut at = first.height();
    for (strip, shift) in strips {
        let from = strip.height() - shift.min(strip.height());
        for y in 0..shift.min(strip.height()) {
            for x in 0..collected.width().min(strip.width()) {
                collected.put_pixel(x, at + y, *strip.get_pixel(x, from + y));
            }
        }
        at += shift;
    }
    Ok((collected, frames, why))
}
