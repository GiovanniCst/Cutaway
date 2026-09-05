// Writing the picture out.
//
// The size of the resulting file is measured rather than estimated: the picture
// is encoded, the bytes are counted, and that is what the panel shows. It costs
// an encode per change of the quality slider, which on the pictures this program
// handles is a few milliseconds - and it means the number on screen is the
// number that will be on disk, not a guess that turns out wrong after the fact.

use std::io::Cursor;
use std::path::Path;

use image::{ImageFormat, RgbaImage};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Format {
    Png,
    Jpeg,
    WebP,
    Tiff,
    Bmp,
}

impl Format {
    pub const ALL: &'static [Format] =
        &[Format::Png, Format::Jpeg, Format::WebP, Format::Tiff, Format::Bmp];

    pub fn label(&self) -> &'static str {
        match self {
            Format::Png => "PNG",
            Format::Jpeg => "JPEG",
            Format::WebP => "WebP",
            Format::Tiff => "TIFF",
            Format::Bmp => "BMP",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Jpeg => "jpg",
            Format::WebP => "webp",
            Format::Tiff => "tif",
            Format::Bmp => "bmp",
        }
    }

    /// Whether a quality setting means anything here.
    pub fn lossy(&self) -> bool {
        matches!(self, Format::Jpeg)
    }

    /// Whether transparency survives this format.
    pub fn keeps_transparency(&self) -> bool {
        matches!(self, Format::Png | Format::WebP | Format::Tiff)
    }

    pub fn from_path(path: &Path) -> Format {
        match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
            Some("jpg") | Some("jpeg") => Format::Jpeg,
            Some("webp") => Format::WebP,
            Some("tif") | Some("tiff") => Format::Tiff,
            Some("bmp") => Format::Bmp,
            _ => Format::Png,
        }
    }
}

/// Encodes the picture, giving back exactly the bytes that would be written.
pub fn encode(pixels: &RgbaImage, format: Format, quality: u8) -> Result<Vec<u8>, String> {
    let mut out = Cursor::new(Vec::new());
    match format {
        Format::Jpeg => {
            // JPEG has no alpha, so transparency is composited onto white
            // rather than left to fall on black - the same choice the clipboard
            // makes, and for the same reason.
            let flat = flatten(pixels);
            let mut encoder =
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality.max(1));
            encoder
                .encode(flat.as_raw(), flat.width(), flat.height(), image::ExtendedColorType::Rgb8)
                .map_err(|exc| exc.to_string())?;
        }
        Format::Bmp => {
            let flat = flatten(pixels);
            flat.write_to(&mut out, ImageFormat::Bmp).map_err(|exc| exc.to_string())?;
        }
        Format::Png => {
            pixels.write_to(&mut out, ImageFormat::Png).map_err(|exc| exc.to_string())?
        }
        Format::WebP => {
            pixels.write_to(&mut out, ImageFormat::WebP).map_err(|exc| exc.to_string())?
        }
        Format::Tiff => {
            pixels.write_to(&mut out, ImageFormat::Tiff).map_err(|exc| exc.to_string())?
        }
    }
    Ok(out.into_inner())
}

/// Composites transparency onto white, for the formats that have nowhere to put
/// an alpha channel.
fn flatten(pixels: &RgbaImage) -> image::RgbImage {
    let mut out = image::RgbImage::new(pixels.width(), pixels.height());
    for (x, y, pixel) in pixels.enumerate_pixels() {
        let alpha = pixel[3] as f32 / 255.0;
        let over =
            |channel: u8| (channel as f32 * alpha + 255.0 * (1.0 - alpha)).round() as u8;
        out.put_pixel(x, y, image::Rgb([over(pixel[0]), over(pixel[1]), over(pixel[2])]));
    }
    out
}

pub fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|exc| exc.to_string())
}

/// A size a person can read, which is what the panel wants.
pub fn readable(bytes: usize) -> String {
    if bytes < 1024 {
        return format!("{} byte", bytes);
    }
    let kb = bytes as f64 / 1024.0;
    if kb < 1024.0 {
        return format!("{:.0} KB", kb);
    }
    format!("{:.1} MB", kb / 1024.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture() -> RgbaImage {
        let mut pixels = RgbaImage::new(64, 64);
        for (x, y, pixel) in pixels.enumerate_pixels_mut() {
            *pixel = image::Rgba([(x * 4) as u8, (y * 4) as u8, 128, if x < 32 { 255 } else { 0 }]);
        }
        pixels
    }

    #[test]
    fn every_format_writes_something_readable_back() {
        for format in Format::ALL {
            let bytes = encode(&picture(), *format, 90).expect(format.label());
            assert!(!bytes.is_empty(), "{} vuoto", format.label());
            let back = image::load_from_memory(&bytes)
                .unwrap_or_else(|exc| panic!("{} non rileggibile: {}", format.label(), exc));
            assert_eq!((back.width(), back.height()), (64, 64), "{}", format.label());
        }
    }

    #[test]
    fn quality_actually_changes_the_size() {
        let small = encode(&picture(), Format::Jpeg, 20).expect("jpeg");
        let large = encode(&picture(), Format::Jpeg, 95).expect("jpeg");
        assert!(large.len() > small.len(), "{} contro {}", large.len(), small.len());
    }

    #[test]
    fn transparency_goes_to_white_where_it_cannot_survive() {
        let bytes = encode(&picture(), Format::Jpeg, 95).expect("jpeg");
        let back = image::load_from_memory(&bytes).expect("rileggibile").to_rgb8();
        // The right half was transparent; over white it comes back light, not
        // black, which is what happens if the alpha is simply dropped.
        let pixel = back.get_pixel(60, 10);
        assert!(pixel[0] > 200 && pixel[1] > 200, "sfondo scuro: {:?}", pixel);
    }

    #[test]
    fn png_keeps_what_jpeg_cannot() {
        let bytes = encode(&picture(), Format::Png, 90).expect("png");
        let back = image::load_from_memory(&bytes).expect("rileggibile").to_rgba8();
        assert_eq!(back.get_pixel(60, 10)[3], 0);
    }

    #[test]
    fn sizes_read_the_way_people_say_them() {
        assert_eq!(readable(512), "512 byte");
        assert_eq!(readable(2048), "2 KB");
        assert_eq!(readable(3 * 1024 * 1024), "3.0 MB");
    }
}
