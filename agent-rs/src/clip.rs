// The piece, on the clipboard, the moment it is cut.
//
// The editor takes seconds to come up, and for a good part of what people do
// with a screenshot - paste it into a mail, a chat, a document - the editor is
// not wanted at all. So the agent puts the piece on the clipboard itself,
// before the handoff even starts: by the time the mouse button is back up, it
// can already be pasted somewhere else.
//
// Two formats, the same pair the editor writes when you copy from inside it:
// PNG for programs that understand it, and a device-independent bitmap for the
// ones that do not. A screen capture has no transparency to lose, so nothing is
// given up by the older format here.

// GlobalFree lives in Foundation while the rest of the Global* family is in
// System::Memory - one of several places this crate files a function where
// nobody would look for it.
use windows_sys::Win32::Foundation::{GlobalFree, HANDLE};
use windows_sys::Win32::Graphics::Gdi::{BITMAPINFOHEADER, BI_RGB};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

use crate::paths;
use crate::wide::wide;

const CF_DIB: u32 = 8;

/// Puts the piece on the clipboard. Never fails loudly: a clipboard held by
/// another program at that instant must not cost the capture.
pub fn put(width: u32, height: u32, rgba: &[u8]) -> bool {
    let dib = as_dib(width, height, rgba);
    let png = as_png(width, height, rgba);

    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            paths::append("clipboard refused: another program is holding it");
            return false;
        }
        EmptyClipboard();
        let mut placed = hand_over(CF_DIB, &dib);
        if let Some(bytes) = png {
            let format = RegisterClipboardFormatW(wide("PNG").as_ptr());
            if format != 0 {
                placed |= hand_over(format, &bytes);
            }
        }
        CloseClipboard();
        placed
    }
}

/// Copies one format's bytes into moveable memory and hands it over. The
/// clipboard owns that memory afterwards, so it is only freed here when the
/// handover itself failed.
unsafe fn hand_over(format: u32, data: &[u8]) -> bool {
    let handle = GlobalAlloc(GMEM_MOVEABLE, data.len());
    if handle.is_null() {
        return false;
    }
    let target = GlobalLock(handle);
    if target.is_null() {
        GlobalFree(handle);
        return false;
    }
    std::ptr::copy_nonoverlapping(data.as_ptr(), target as *mut u8, data.len());
    GlobalUnlock(handle);
    if SetClipboardData(format, handle as HANDLE).is_null() {
        GlobalFree(handle);
        return false;
    }
    true
}

/// A CF_DIB: the header followed by the pixels, bottom-up, as that format wants
/// them. No alpha channel, which costs nothing for a picture of a screen.
fn as_dib(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let header_size = std::mem::size_of::<BITMAPINFOHEADER>();
    // Each row of a DIB is padded to four bytes.
    let stride = ((width * 3 + 3) / 4 * 4) as usize;
    let mut out = vec![0u8; header_size + stride * height as usize];

    let header = BITMAPINFOHEADER {
        biSize: header_size as u32,
        biWidth: width as i32,
        biHeight: height as i32, // positive: bottom-up, the way CF_DIB is read
        biPlanes: 1,
        biBitCount: 24,
        biCompression: BI_RGB,
        biSizeImage: (stride * height as usize) as u32,
        biXPelsPerMeter: 0,
        biYPelsPerMeter: 0,
        biClrUsed: 0,
        biClrImportant: 0,
    };
    unsafe {
        std::ptr::copy_nonoverlapping(
            &header as *const BITMAPINFOHEADER as *const u8,
            out.as_mut_ptr(),
            header_size,
        );
    }

    for y in 0..height as usize {
        // Bottom-up: the last row of the picture is the first row of the DIB.
        let source = (height as usize - 1 - y) * width as usize * 4;
        let target = header_size + y * stride;
        for x in 0..width as usize {
            let at = source + x * 4;
            out[target + x * 3] = rgba[at + 2]; // B
            out[target + x * 3 + 1] = rgba[at + 1]; // G
            out[target + x * 3 + 2] = rgba[at]; // R
        }
    }
    out
}

fn as_png(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
    }
    Some(bytes)
}
