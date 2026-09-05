// Printing, by way of a PDF.
//
// There is no dialogue with a printer here and deliberately so: the picture is
// composed onto a page at 300 DPI, written as a PDF, and handed to whatever
// opens PDFs on this machine. That program already knows about the printers,
// the paper trays and the duplexers, and it is the one the person has already
// learned. The same choice the Python build made.
//
// The PDF is written by hand rather than through a library. A page holding one
// JPEG needs five objects and a cross-reference table, which is less code than
// the dependency would be - and this way nothing decides on its own to embed a
// font or a colour profile.

use std::io::Write;
use std::path::{Path, PathBuf};

use image::RgbaImage;

/// What the page is composed at. High enough that a screenshot placed on A4
/// still has its text readable rather than resampled into porridge.
pub const PAGE_DPI: f32 = 300.0;

const CM_PER_INCH: f32 = 2.54;

#[derive(Clone, Copy, PartialEq)]
pub enum Paper {
    A3,
    A4,
    A5,
    Letter,
    Legal,
}

impl Paper {
    pub const ALL: &'static [Paper] =
        &[Paper::A3, Paper::A4, Paper::A5, Paper::Letter, Paper::Legal];

    pub fn label(&self) -> &'static str {
        match self {
            Paper::A3 => "A3",
            Paper::A4 => "A4",
            Paper::A5 => "A5",
            Paper::Letter => "Letter",
            Paper::Legal => "Legal",
        }
    }

    /// Width and height in centimetres, upright.
    fn size_cm(&self) -> (f32, f32) {
        match self {
            Paper::A3 => (29.7, 42.0),
            Paper::A4 => (21.0, 29.7),
            Paper::A5 => (14.8, 21.0),
            Paper::Letter => (21.59, 27.94),
            Paper::Legal => (21.59, 35.56),
        }
    }

    pub fn pixels(&self, landscape: bool) -> (u32, u32) {
        let (width, height) = self.size_cm();
        let (width, height) = if landscape { (height, width) } else { (width, height) };
        (
            (width / CM_PER_INCH * PAGE_DPI).round() as u32,
            (height / CM_PER_INCH * PAGE_DPI).round() as u32,
        )
    }
}

/// Places the picture on a white page, centred, inside a margin.
///
/// Scaled down to fit but never up: enlarging a small picture to fill a sheet
/// prints a blurry version of something that was sharp.
pub fn compose(picture: &RgbaImage, paper: Paper, landscape: bool, margin_mm: f32) -> RgbaImage {
    let (page_w, page_h) = paper.pixels(landscape);
    let margin = (margin_mm.clamp(0.0, 50.0) / 10.0 / CM_PER_INCH * PAGE_DPI).round() as u32;
    let room_w = page_w.saturating_sub(margin * 2).max(1);
    let room_h = page_h.saturating_sub(margin * 2).max(1);

    let scale = (room_w as f32 / picture.width() as f32)
        .min(room_h as f32 / picture.height() as f32)
        .min(1.0);
    let width = ((picture.width() as f32 * scale).round() as u32).max(1);
    let height = ((picture.height() as f32 * scale).round() as u32).max(1);
    let placed = image::imageops::resize(
        picture,
        width,
        height,
        image::imageops::FilterType::Lanczos3,
    );

    let mut sheet = RgbaImage::from_pixel(page_w, page_h, image::Rgba([255, 255, 255, 255]));
    let left = (page_w - width) / 2;
    let top = (page_h - height) / 2;
    // Composited rather than copied, so a picture with transparency lands on the
    // paper's white instead of carrying a black rectangle onto it.
    image::imageops::overlay(&mut sheet, &placed, left as i64, top as i64);
    sheet
}

/// Writes a one-page PDF holding the sheet, and gives back where it went.
pub fn write_pdf(sheet: &RgbaImage, into: Option<&Path>) -> Result<PathBuf, String> {
    // The image goes in as JPEG: a full-page 300 DPI bitmap uncompressed is
    // thirty megabytes, and no viewer thanks you for it.
    let jpeg = crate::save::encode(sheet, crate::save::Format::Jpeg, 92)?;

    let folder = into.map(Path::to_path_buf).unwrap_or_else(std::env::temp_dir);
    let path = folder.join(format!("Cutaway {}.pdf", crate::clock::stamp_file()));

    // A page is measured in points, 72 to the inch, whatever the image's own
    // resolution is.
    let width_pt = sheet.width() as f32 / PAGE_DPI * 72.0;
    let height_pt = sheet.height() as f32 / PAGE_DPI * 72.0;

    let mut out: Vec<u8> = Vec::with_capacity(jpeg.len() + 2048);
    let mut offsets: Vec<usize> = Vec::new();
    out.extend_from_slice(b"%PDF-1.4\n");
    // A comment with high bytes, which is how a PDF says it is not plain text.
    out.extend_from_slice(b"%\xE2\xE3\xCF\xD3\n");

    let mut object = |out: &mut Vec<u8>, offsets: &mut Vec<usize>, body: &[u8]| {
        offsets.push(out.len());
        let number = offsets.len();
        let _ = write!(out, "{} 0 obj\n", number);
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    };

    object(&mut out, &mut offsets, b"<< /Type /Catalog /Pages 2 0 R >>");
    object(&mut out, &mut offsets, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
    let page = format!(
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.2} {:.2}] \
         /Resources << /XObject << /Im0 4 0 R >> >> /Contents 5 0 R >>",
        width_pt, height_pt
    );
    object(&mut out, &mut offsets, page.as_bytes());

    // The image itself, as a stream the page draws once.
    offsets.push(out.len());
    let _ = write!(
        out,
        "4 0 obj\n<< /Type /XObject /Subtype /Image /Width {} /Height {} \
         /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
        sheet.width(),
        sheet.height(),
        jpeg.len()
    );
    out.extend_from_slice(&jpeg);
    out.extend_from_slice(b"\nendstream\nendobj\n");

    // Draw it over the whole page: the margins are already in the bitmap.
    let content = format!("q\n{:.2} 0 0 {:.2} 0 0 cm\n/Im0 Do\nQ\n", width_pt, height_pt);
    let stream = format!("<< /Length {} >>\nstream\n{}endstream", content.len(), content);
    object(&mut out, &mut offsets, stream.as_bytes());

    // The cross-reference table, which is what makes the file navigable, and the
    // one part where a byte out of place makes a viewer refuse the whole thing.
    let xref_at = out.len();
    let _ = write!(out, "xref\n0 {}\n", offsets.len() + 1);
    out.extend_from_slice(b"0000000000 65535 f \n");
    for offset in &offsets {
        let _ = write!(out, "{:010} 00000 n \n", offset);
    }
    let _ = write!(
        out,
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
        offsets.len() + 1,
        xref_at
    );

    std::fs::write(&path, &out).map_err(|exc| exc.to_string())?;
    Ok(path)
}

/// Hands the file to whatever opens it on this machine.
pub fn open_externally(path: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // Through the shell, because the association is the shell's to resolve; the
    // empty title is what start needs when the path is quoted.
    std::process::Command::new("cmd")
        .args(["/c", "start", "", &path.to_string_lossy()])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|exc| exc.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_is_the_size_the_paper_says() {
        // A4 at 300 DPI: 21 x 29.7 cm is 2480 x 3508, the number every print
        // shop quotes.
        assert_eq!(Paper::A4.pixels(false), (2480, 3508));
        // Landscape swaps them and nothing else.
        assert_eq!(Paper::A4.pixels(true), (3508, 2480));
    }

    #[test]
    fn a_small_picture_is_not_blown_up_to_fill_the_sheet() {
        let small = RgbaImage::from_pixel(100, 50, image::Rgba([1, 2, 3, 255]));
        let sheet = compose(&small, Paper::A4, false, 10.0);
        assert_eq!(sheet.dimensions(), Paper::A4.pixels(false));
        // Still 100 x 50 somewhere in the middle: the corners are paper.
        assert_eq!(*sheet.get_pixel(5, 5), image::Rgba([255, 255, 255, 255]));
    }

    #[test]
    fn a_large_picture_fits_inside_the_margins() {
        let large = RgbaImage::from_pixel(6000, 3000, image::Rgba([1, 2, 3, 255]));
        let sheet = compose(&large, Paper::A4, false, 10.0);
        let margin = (10.0_f32 / 10.0 / CM_PER_INCH * PAGE_DPI).round() as u32;
        // The margin is paper, all the way across.
        for x in (0..sheet.width()).step_by(97) {
            assert_eq!(sheet.get_pixel(x, margin / 2)[0], 255);
        }
    }

    #[test]
    fn transparency_lands_on_paper_rather_than_on_black() {
        let clear = RgbaImage::from_pixel(200, 200, image::Rgba([0, 0, 0, 0]));
        let sheet = compose(&clear, Paper::A5, false, 5.0);
        assert_eq!(sheet.get_pixel(sheet.width() / 2, sheet.height() / 2)[0], 255);
    }

    #[test]
    fn the_pdf_is_one_a_reader_will_take() {
        let sheet = RgbaImage::from_pixel(400, 300, image::Rgba([200, 100, 50, 255]));
        let where_at = std::env::temp_dir().join("cutaway_pdf_test");
        std::fs::create_dir_all(&where_at).expect("cartella");
        let path = write_pdf(&sheet, Some(&where_at)).expect("pdf scritto");
        let bytes = std::fs::read(&path).expect("pdf rileggibile");
        assert!(bytes.starts_with(b"%PDF-1.4"), "intestazione mancante");
        assert!(bytes.ends_with(b"%%EOF\n"), "coda mancante");
        // The cross-reference offset has to point at the table itself, or a
        // viewer refuses the file with no clue as to why.
        let tail = String::from_utf8_lossy(&bytes[bytes.len().saturating_sub(60)..]).to_string();
        let at: usize = tail
            .lines()
            .rev()
            .find_map(|line| line.trim().parse().ok())
            .expect("startxref");
        assert_eq!(&bytes[at..at + 4], b"xref");
        let _ = std::fs::remove_file(path);
    }
}
