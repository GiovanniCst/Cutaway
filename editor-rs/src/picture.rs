// The picture being edited, and the texture the screen shows it through.
//
// Two representations of the same thing, deliberately: the pixels, which every
// edit and every save works on, and a texture uploaded to the GPU, which is what
// gets drawn. The texture is rebuilt only when the pixels change - a pan or a
// zoom does not touch it, so moving around a large photograph costs nothing.

use std::path::{Path, PathBuf};

use egui::{ColorImage, TextureHandle, TextureOptions};
use image::RgbaImage;

use crate::adjust::{self, Adjustments};

pub struct Picture {
    /// The picture as it arrived, kept whole. Adjustments are a lens rather than
    /// a change: moving a slider back to the middle has to give back exactly
    /// what was there, and it cannot if the original was overwritten on the way.
    original: RgbaImage,
    /// The picture as it looks now, which is what gets drawn and saved.
    pub pixels: RgbaImage,
    /// What is being applied to the original to arrive at the above.
    pub adjustments: Adjustments,
    /// Where it came from, when it came from anywhere. A capture has no path,
    /// and no path through this program overwrites the file it opened.
    pub path: Option<PathBuf>,
    /// What to call it when saving: the file's name, or the moment it was cut.
    pub name: String,
    /// States to go back to. Each holds decoded pixels, so this is a memory
    /// budget as much as a decision about how forgiving to be - twelve is what
    /// the Python build settled on and there is no reason to differ.
    history: Vec<RgbaImage>,
    texture: Option<TextureHandle>,
    /// Whether any pixel is less than fully opaque. Answered once when the
    /// pixels change rather than on every frame: the chequer under the picture
    /// is only drawn when there is something to see through, and scanning four
    /// million pixels sixty times a second to decide that would cost more than
    /// everything else the window does.
    see_through: Option<bool>,
    /// Which filter the current texture was uploaded with. Past 100% the pixels
    /// are the subject and are shown sharp; below it they are smoothed, or a
    /// picture shown smaller comes out speckled.
    magnified: bool,
}

impl Picture {
    pub fn open(path: &Path) -> Result<Picture, String> {
        // The orientation tag is applied on load, so a photograph from a phone
        // is the right way up rather than the way the sensor happened to sit.
        let decoded = image::ImageReader::open(path)
            .map_err(|exc| format!("{}", exc))?
            .with_guessed_format()
            .map_err(|exc| format!("{}", exc))?
            .decode()
            .map_err(|exc| format!("{}", exc))?;
        let pixels = decoded.to_rgba8();
        Ok(Picture {
            original: pixels.clone(),
            pixels,
            adjustments: Adjustments::default(),
            path: Some(path.to_path_buf()),
            name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            history: Vec::new(),
            texture: None,
            see_through: None,
            magnified: false,
        })
    }

    /// A picture that came from no file: a capture, or a paste.
    pub fn adopt(pixels: RgbaImage, name: String) -> Picture {
        Picture {
            original: pixels.clone(),
            pixels,
            adjustments: Adjustments::default(),
            path: None,
            name,
            history: Vec::new(),
            texture: None,
            see_through: None,
            magnified: false,
        }
    }

    /// Recomputes the picture from the original under the current adjustments.
    ///
    /// From the original every time, not from the last result: applying a curve
    /// to an already-curved picture compounds it, and the slider would then
    /// behave differently depending on how it got where it is.
    pub fn readjust(&mut self) {
        self.pixels = self.original.clone();
        adjust::apply(&mut self.pixels, &self.adjustments);
        self.touched();
    }

    const REMEMBERED: usize = 12;

    /// Puts the current state aside before something changes it.
    ///
    /// Called by every operation that alters the pixels, and only by those: an
    /// adjustment being dragged is not a step, or a slider would fill the
    /// history with a state per frame.
    fn remember(&mut self) {
        self.history.push(self.original.clone());
        if self.history.len() > Self::REMEMBERED {
            self.history.remove(0);
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    /// Steps back one operation.
    pub fn undo(&mut self) {
        if let Some(previous) = self.history.pop() {
            self.original = previous;
            self.pixels = self.original.clone();
            self.adjustments = Adjustments::default();
            self.touched();
        }
    }

    /// Resamples the picture to a new size.
    ///
    /// Lanczos3, which is what the Python build uses: on a screenshot it keeps
    /// text legible at sizes where a bilinear filter turns it to mush.
    pub fn resize_to(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.remember();
        self.pixels = image::imageops::resize(
            &self.pixels,
            width,
            height,
            image::imageops::FilterType::Lanczos3,
        );
        self.original = self.pixels.clone();
        self.adjustments = Adjustments::default();
        self.touched();
    }

    /// Takes a wholly different set of pixels as the picture.
    ///
    /// What comes back from a model is a new picture rather than a change to
    /// this one - a different size, often a different everything - so there is
    /// nothing to blend and nothing to recompute. It goes through the history
    /// like every other operation, which is what makes "Annulla" the way back
    /// from an edit somebody did not like.
    pub fn replace_with(&mut self, pixels: RgbaImage) {
        self.remember();
        self.pixels = pixels;
        self.original = self.pixels.clone();
        self.adjustments = Adjustments::default();
        self.touched();
    }

    /// The colour at a point, read from the picture before any preview.
    pub fn sample_original(&self, x: f32, y: f32) -> [u8; 3] {
        crate::cutout::sample(&self.original, x, y)
    }

    /// Shows what a keying would do, without committing to it.
    ///
    /// Recomputed from the original each time rather than applied on top of the
    /// last preview, or dragging a slider would eat the picture a pass at a
    /// time - the same reason the adjustments recompute from the original.
    pub fn preview_cutout(&mut self, how: &crate::cutout::Keying) {
        self.pixels = self.original.clone();
        crate::adjust::apply(&mut self.pixels, &self.adjustments);
        crate::cutout::key_out(&mut self.pixels, how);
        self.touched();
    }

    /// Commits the keying: what is transparent now stays transparent.
    pub fn apply_cutout(&mut self, how: &crate::cutout::Keying) {
        self.remember();
        self.pixels = self.original.clone();
        crate::adjust::apply(&mut self.pixels, &self.adjustments);
        crate::cutout::key_out(&mut self.pixels, how);
        self.original = self.pixels.clone();
        self.adjustments = Adjustments::default();
        self.touched();
    }

    /// Puts the preview away without keeping it.
    pub fn forget_preview(&mut self) {
        self.readjust();
    }

    /// Cuts the picture down to a selection, and forgets what was outside it.
    ///
    /// The original goes too: after a cut, "back to how it was" means the picture
    /// as cut, not the one that was opened. Undo is a separate idea and belongs
    /// to a history that does not exist yet.
    pub fn cut_to(&mut self, to: crate::crop::Selection) {
        self.remember();
        self.pixels = crate::crop::cut(&self.pixels, to);
        self.original = self.pixels.clone();
        self.adjustments = Adjustments::default();
        self.touched();
    }

    /// Writes marks into the pixels, where they stop being marks.
    ///
    /// Applied to the original as well, so a later adjustment does not undo
    /// them: after this the marks are part of the picture, which is what
    /// applying means.
    pub fn stamp(&mut self, shapes: &[crate::annotate::Shape]) {
        self.remember();
        for shape in shapes {
            crate::annotate::draw(&mut self.pixels, shape);
        }
        self.original = self.pixels.clone();
        self.adjustments = Adjustments::default();
        self.touched();
    }

    /// How long the last readjust took, for the status line - the whole point of
    /// doing this natively is that the answer is small.
    pub fn adjust_cost_ms(&mut self) -> u128 {
        let clock = std::time::Instant::now();
        self.readjust();
        clock.elapsed().as_millis()
    }

    pub fn width(&self) -> u32 {
        self.pixels.width()
    }

    pub fn height(&self) -> u32 {
        self.pixels.height()
    }

    /// The texture to draw, uploaded on first use, after every edit, and when
    /// the picture crosses 100% in either direction.
    pub fn texture(&mut self, ctx: &egui::Context, magnified: bool) -> TextureHandle {
        if let Some(handle) = &self.texture {
            if self.magnified == magnified {
                return handle.clone();
            }
        }
        let size = [self.pixels.width() as usize, self.pixels.height() as usize];
        let image = ColorImage::from_rgba_unmultiplied(size, self.pixels.as_raw());
        let filter = if magnified { TextureOptions::NEAREST } else { TextureOptions::LINEAR };
        let handle = ctx.load_texture("picture", image, filter);
        self.texture = Some(handle.clone());
        self.magnified = magnified;
        handle
    }

    /// Called after anything changes the pixels: the texture is now a lie.
    pub fn touched(&mut self) {
        self.texture = None;
        self.see_through = None;
    }

    /// True when part of the picture is transparent, and the ground under it
    /// should therefore be a chequer rather than a flat grey - which is the
    /// same grey as a background somebody has just removed.
    pub fn see_through(&mut self) -> bool {
        match self.see_through {
            Some(known) => known,
            None => {
                let found = self.pixels.pixels().any(|pixel| pixel[3] < 255);
                self.see_through = Some(found);
                found
            }
        }
    }
}
