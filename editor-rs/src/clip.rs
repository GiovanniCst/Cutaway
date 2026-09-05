// The clipboard, both directions.
//
// Out, in the two formats the rest of this project writes: PNG for programs that
// read it, and a device-independent bitmap for the ones that do not. In, taking
// whichever of the two is on offer, PNG first because it is the one that can
// carry transparency.

use image::RgbaImage;
use windows_sys::Win32::Foundation::{GlobalFree, HANDLE};
use windows_sys::Win32::Graphics::Gdi::{BITMAPINFOHEADER, BI_RGB};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
};

const CF_DIB: u32 = 8;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Puts a picture on the clipboard. False when another program was holding it.
pub fn put(picture: &RgbaImage) -> bool {
    let dib = as_dib(picture);
    let png = as_png(picture);
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
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

/// Puts text on the clipboard, which is what the OCR wants.
pub fn put_text(text: &str) -> bool {
    // CF_UNICODETEXT, with the terminator the format requires: a string handed
    // over without it is read until something else says stop.
    const CF_UNICODETEXT: u32 = 13;
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = unsafe {
        std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2)
    };
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return false;
        }
        EmptyClipboard();
        let placed = hand_over(CF_UNICODETEXT, bytes);
        CloseClipboard();
        placed
    }
}

/// Takes a picture off the clipboard, when there is one to take.
pub fn take() -> Option<RgbaImage> {
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return None;
        }
        let png_format = RegisterClipboardFormatW(wide("PNG").as_ptr());
        // PNG first: it is the only one of the two that carries transparency, so
        // taking the bitmap when both are present would quietly flatten it.
        let taken = if png_format != 0 && IsClipboardFormatAvailable(png_format) != 0 {
            read(png_format).and_then(|bytes| image::load_from_memory(&bytes).ok())
                .map(|decoded| decoded.to_rgba8())
        } else if IsClipboardFormatAvailable(CF_DIB) != 0 {
            read(CF_DIB).and_then(|bytes| from_dib(&bytes))
        } else {
            None
        };
        CloseClipboard();
        taken
    }
}

unsafe fn read(format: u32) -> Option<Vec<u8>> {
    let handle = GetClipboardData(format);
    if handle.is_null() {
        return None;
    }
    let size = GlobalSize(handle as _);
    if size == 0 {
        return None;
    }
    let source = GlobalLock(handle as _);
    if source.is_null() {
        return None;
    }
    let mut bytes = vec![0u8; size];
    std::ptr::copy_nonoverlapping(source as *const u8, bytes.as_mut_ptr(), size);
    GlobalUnlock(handle as _);
    Some(bytes)
}

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

fn as_dib(picture: &RgbaImage) -> Vec<u8> {
    let header_size = std::mem::size_of::<BITMAPINFOHEADER>();
    let (width, height) = (picture.width(), picture.height());
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
    for y in 0..height {
        let source_row = height - 1 - y;
        let target = header_size + y as usize * stride;
        for x in 0..width {
            let pixel = picture.get_pixel(x, source_row);
            let at = target + x as usize * 3;
            // Transparency cannot survive here, so it is composited onto white -
            // the same choice the Python build makes, and for the same reason:
            // the alternative is a black fringe nobody asked for.
            let alpha = pixel[3] as f32 / 255.0;
            let over = |channel: u8| {
                (channel as f32 * alpha + 255.0 * (1.0 - alpha)).round() as u8
            };
            out[at] = over(pixel[2]);
            out[at + 1] = over(pixel[1]);
            out[at + 2] = over(pixel[0]);
        }
    }
    out
}

/// Reads a device-independent bitmap back into pixels.
///
/// Only the shapes that actually turn up are handled - 24 and 32 bits, no
/// palette, no compression - because a clipboard bitmap comes from a program
/// that just put it there, not from a file of unknown age.
fn from_dib(bytes: &[u8]) -> Option<RgbaImage> {
    let header_size = std::mem::size_of::<BITMAPINFOHEADER>();
    if bytes.len() < header_size {
        return None;
    }
    let header: BITMAPINFOHEADER = unsafe { std::ptr::read(bytes.as_ptr() as *const _) };
    if header.biCompression != BI_RGB || (header.biBitCount != 24 && header.biBitCount != 32) {
        return None;
    }
    let width = header.biWidth.unsigned_abs();
    // A negative height means the rows are stored top-down.
    let top_down = header.biHeight < 0;
    let height = header.biHeight.unsigned_abs();
    if width == 0 || height == 0 {
        return None;
    }
    let depth = header.biBitCount as usize / 8;
    let stride = (width as usize * depth + 3) / 4 * 4;
    let start = header.biSize as usize;
    if bytes.len() < start + stride * height as usize {
        return None;
    }
    let mut out = RgbaImage::new(width, height);
    for y in 0..height {
        let row = if top_down { y } else { height - 1 - y };
        let at = start + row as usize * stride;
        for x in 0..width {
            let p = at + x as usize * depth;
            // 32-bit clipboard bitmaps carry a fourth byte that is usually
            // meaningless rather than an alpha channel, so it is not trusted.
            out.put_pixel(x, y, image::Rgba([bytes[p + 2], bytes[p + 1], bytes[p], 255]));
        }
    }
    Some(out)
}

fn as_png(picture: &RgbaImage) -> Option<Vec<u8>> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    picture.write_to(&mut bytes, image::ImageFormat::Png).ok()?;
    Some(bytes.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bitmap_survives_the_round_trip() {
        let mut original = RgbaImage::new(3, 2);
        original.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        original.put_pixel(1, 0, image::Rgba([0, 255, 0, 255]));
        original.put_pixel(2, 0, image::Rgba([0, 0, 255, 255]));
        original.put_pixel(0, 1, image::Rgba([1, 2, 3, 255]));
        original.put_pixel(1, 1, image::Rgba([250, 251, 252, 255]));
        original.put_pixel(2, 1, image::Rgba([128, 128, 128, 255]));

        let back = from_dib(&as_dib(&original)).expect("un DIB che si rilegge");
        assert_eq!(back.dimensions(), original.dimensions());
        // Rows are stored upside down in a DIB; getting that wrong is the
        // classic mistake and this is what catches it.
        assert_eq!(back.get_pixel(0, 0), original.get_pixel(0, 0));
        assert_eq!(back.get_pixel(2, 1), original.get_pixel(2, 1));
    }

    #[test]
    fn transparency_lands_on_white_rather_than_black() {
        let clear = RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 0]));
        let back = from_dib(&as_dib(&clear)).expect("un DIB");
        assert_eq!(back.get_pixel(0, 0)[0], 255);
    }
}
