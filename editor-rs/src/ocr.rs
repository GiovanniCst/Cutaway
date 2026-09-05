// Reading the text out of a picture.
//
// Windows has an OCR engine built in - the one the Snipping Tool and PowerToys
// use - and it is reachable from here, so nothing is downloaded, no service is
// called, and the picture never leaves the machine. That last point is the
// reason to prefer it over anything cloud-based even where the cloud would be
// more accurate: a screenshot is exactly the kind of thing that should not be
// uploaded by an app that was only asked to read it.
//
// The engine speaks whichever languages are installed. It is asked for the
// user's own first, which is what makes it recognise accented text correctly on
// an Italian machine, and falls back to any engine at all rather than refusing.

use image::RgbaImage;
use windows::Graphics::Imaging::BitmapDecoder;
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

/// What was read, and where each line sits, so the caller can show it.
pub struct Reading {
    pub text: String,
    pub lines: usize,
}

/// Reads the text in a picture. The error is meant to be shown, so it says what
/// went wrong in a sentence rather than a code.
pub fn read(picture: &RgbaImage) -> Result<Reading, String> {
    // The engine wants a SoftwareBitmap. Going through PNG in memory is a few
    // more steps than filling a pixel buffer by hand, but every one of them is
    // an API that either works or says why - where the buffer route needs a COM
    // interface cast that fails silently when it is wrong.
    let png = crate::save::encode(picture, crate::save::Format::Png, 100)?;

    let stream = InMemoryRandomAccessStream::new().map_err(say)?;
    let writer = DataWriter::CreateDataWriter(&stream).map_err(say)?;
    writer.WriteBytes(&png).map_err(say)?;
    writer.StoreAsync().map_err(say)?.get().map_err(say)?;
    writer.FlushAsync().map_err(say)?.get().map_err(say)?;
    stream.Seek(0).map_err(say)?;

    let decoder = BitmapDecoder::CreateAsync(&stream).map_err(say)?.get().map_err(say)?;
    let bitmap = decoder.GetSoftwareBitmapAsync().map_err(say)?.get().map_err(say)?;

    // The user's own languages first: on an Italian machine that is what makes
    // accented words come back as words rather than as approximations.
    let engine = OcrEngine::TryCreateFromUserProfileLanguages()
        .map_err(say)
        .and_then(|engine| {
            if engine.is_ok() {
                Ok(engine)
            } else {
                Err(crate::words::w().no_ocr_engine.to_string())
            }
        })?;

    let result = engine.RecognizeAsync(&bitmap).map_err(say)?.get().map_err(say)?;
    let text = result.Text().map_err(say)?.to_string_lossy();
    let lines = result.Lines().map_err(say)?.Size().map_err(say)? as usize;
    Ok(Reading { text, lines })
}

/// Whether this machine can do it at all, so the button can say so before it is
/// pressed rather than after.
pub fn available() -> bool {
    OcrEngine::TryCreateFromUserProfileLanguages().map(|e| e.is_ok()).unwrap_or(false)
}

fn say(error: windows::core::Error) -> String {
    let said = error.message().to_string();
    if said.is_empty() {
        crate::words::fill(crate::words::w().windows_error, &[&format!("{:08X}", error.code().0)])
    } else {
        said
    }
}

trait Ok_ {
    fn is_ok(&self) -> bool;
}

impl Ok_ for OcrEngine {
    /// A WinRT reference can come back null where a Rust Option would be None:
    /// TryCreate* returns success with nothing in it when no engine matches.
    fn is_ok(&self) -> bool {
        self.RecognizerLanguage().is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Color32;

    /// Writes a sentence and asks Windows to read it back.
    ///
    /// End to end on purpose: a unit test of the plumbing would pass with an
    /// engine that recognises nothing, and the only thing worth knowing here is
    /// whether the text comes back.
    #[test]
    fn what_is_written_can_be_read_back() {
        if !available() {
            eprintln!("nessun motore OCR su questa macchina: prova saltata");
            return;
        }
        let mut picture = RgbaImage::from_pixel(900, 200, image::Rgba([255, 255, 255, 255]));
        let written = "Cutaway legge il testo";
        assert!(crate::annotate::draw_text(
            &mut picture,
            written,
            (30.0, 60.0),
            72.0,
            Color32::BLACK,
            false,
        ));

        let reading = read(&picture).expect("lettura riuscita");
        let got = reading.text.to_lowercase();
        // Not an exact match: OCR is allowed to disagree about spacing and
        // punctuation. What it must not do is come back empty or with nothing
        // recognisable in it.
        for word in ["cutaway", "legge", "testo"] {
            assert!(got.contains(word), "manca '{}' in '{}'", word, reading.text);
        }
    }

    #[test]
    fn a_blank_picture_reads_as_nothing_rather_than_failing() {
        if !available() {
            return;
        }
        let blank = RgbaImage::from_pixel(200, 100, image::Rgba([255, 255, 255, 255]));
        let reading = read(&blank).expect("una pagina bianca non e un errore");
        assert!(reading.text.trim().is_empty(), "letto '{}' dal nulla", reading.text);
    }
}
