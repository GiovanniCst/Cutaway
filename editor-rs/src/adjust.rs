// Tone and colour, in one place.
//
// This module is the argument for the whole native build, more than the
// milliseconds are. In the WebView2 editor these adjustments exist twice: once
// in JavaScript, so the preview can follow a slider, and once in Python, so the
// saved file is right. Two implementations of the same arithmetic, in two
// languages, with a test - 68 chroma cases and 12 adjustment cases over 12,000
// pixels - whose entire job is to check that they still agree.
//
// Here the preview and the save call this function. There is nothing to keep in
// step, so there is nothing to drift, and the test that watched for drift has
// nothing left to watch.
//
// The order is brightness, contrast, gamma, saturation, and it is not arbitrary:
// changing it changes the result. Contrast pivots on a fixed mid grey rather
// than the picture's own average, which keeps every step a pure per-pixel
// function - so a preview computed on a smaller copy gives the same answer as
// the full-resolution save, rather than one that quietly differs because the
// average did.

use image::RgbaImage;

#[derive(Clone, Copy, PartialEq)]
pub struct Adjustments {
    pub brightness: f32,
    pub contrast: f32,
    pub gamma: f32,
    pub saturation: f32,
    pub monochrome: bool,
}

impl Default for Adjustments {
    fn default() -> Adjustments {
        Adjustments {
            brightness: 1.0,
            contrast: 1.0,
            gamma: 1.0,
            saturation: 1.0,
            monochrome: false,
        }
    }
}

impl Adjustments {
    /// True when applying these would change nothing, so the work can be skipped.
    pub fn neutral(&self) -> bool {
        *self == Adjustments::default()
    }
}

/// The 256-entry table for the per-channel part: brightness, contrast, gamma.
///
/// A table rather than the arithmetic per pixel, because there are only 256
/// possible inputs per channel and several million pixels.
fn tone_curve(brightness: f32, contrast: f32, gamma: f32) -> [u8; 256] {
    let mut table = [0u8; 256];
    let inverse_gamma = 1.0 / gamma.max(1e-6);
    for (value, slot) in table.iter_mut().enumerate() {
        let mut v = value as f32 * brightness;
        v = (v - 128.0) * contrast + 128.0;
        // Clamped before the power: a negative base with a fractional exponent
        // is not a real number.
        v = v.clamp(0.0, 255.0);
        v = 255.0 * (v / 255.0).powf(inverse_gamma);
        // floor(x + 0.5), matching the table the Python build produces, so a
        // picture adjusted by either comes out identical.
        *slot = (v + 0.5).clamp(0.0, 255.0) as u8;
    }
    table
}

/// The weights Rec.601 gives the channels, which is what PIL's L conversion uses.
fn grey_of(r: u8, g: u8, b: u8) -> f32 {
    // PIL rounds this to an integer before using it; done the same way here so
    // the two builds agree pixel for pixel.
    ((r as f32 * 299.0 + g as f32 * 587.0 + b as f32 * 114.0) / 1000.0).floor()
}

/// Applies the adjustments in place, leaving any alpha channel untouched.
pub fn apply(pixels: &mut RgbaImage, how: &Adjustments) {
    if how.neutral() {
        return;
    }
    let curve = tone_curve(how.brightness, how.contrast, how.gamma);
    for pixel in pixels.pixels_mut() {
        let r = curve[pixel[0] as usize];
        let g = curve[pixel[1] as usize];
        let b = curve[pixel[2] as usize];

        let (r, g, b) = if how.monochrome {
            let grey = grey_of(r, g, b) as u8;
            (grey, grey, grey)
        } else if how.saturation != 1.0 {
            let grey = grey_of(r, g, b);
            // blend(grey, colour, t) is grey + t*(colour - grey): above 1 it
            // extrapolates, which is what makes saturation over 100% mean
            // something rather than clamping at the original.
            let mix = |channel: u8| {
                (grey + how.saturation * (channel as f32 - grey)).clamp(0.0, 255.0) as u8
            };
            (mix(r), mix(g), mix(b))
        } else {
            (r, g, b)
        };

        pixel[0] = r;
        pixel[1] = g;
        pixel[2] = b;
        // pixel[3], the alpha, is deliberately not touched: transparency is not
        // a tone.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_changes_nothing() {
        let mut pixels = RgbaImage::from_pixel(2, 2, image::Rgba([10, 120, 250, 128]));
        let before = pixels.clone();
        apply(&mut pixels, &Adjustments::default());
        assert_eq!(before, pixels);
    }

    #[test]
    fn alpha_survives_everything() {
        let mut pixels = RgbaImage::from_pixel(1, 1, image::Rgba([10, 120, 250, 77]));
        apply(
            &mut pixels,
            &Adjustments { brightness: 1.4, contrast: 0.6, gamma: 2.2, saturation: 0.0, monochrome: true },
        );
        assert_eq!(pixels.get_pixel(0, 0)[3], 77);
    }

    #[test]
    fn the_curve_holds_its_ends() {
        let curve = tone_curve(1.0, 1.0, 1.0);
        assert_eq!(curve[0], 0);
        assert_eq!(curve[255], 255);
        // And is monotonic, or a slider would make a picture darker in places
        // while making it lighter overall.
        assert!(curve.windows(2).all(|pair| pair[0] <= pair[1]));
    }

    /// What a slider costs, measured as best of three.
    ///
    /// The best run rather than the average: a run that was interrupted by
    /// something else on the machine measured the machine, not this code, and
    /// the minimum is the run least contaminated by it. This test used to
    /// measure a single run against a wall clock and failed on a busy
    /// afternoon - which is a test crying wolf, and a test crying wolf is worse
    /// than no test.
    fn cost_of(width: u32, height: u32) -> u128 {
        let how = Adjustments {
            brightness: 1.2,
            contrast: 1.1,
            gamma: 0.9,
            saturation: 1.3,
            monochrome: false,
        };
        (0..3)
            .map(|_| {
                let mut pixels =
                    RgbaImage::from_pixel(width, height, image::Rgba([120, 90, 200, 255]));
                let clock = std::time::Instant::now();
                apply(&mut pixels, &how);
                clock.elapsed().as_micros()
            })
            .min()
            .unwrap_or(u128::MAX)
    }

    /// What a slider costs on a full screen's worth of pixels.
    ///
    /// The WebView2 build never asks this question: it cannot afford the full
    /// resolution in JavaScript, so it previews on a copy capped at 2000 px and
    /// recomputes properly only when saving - which is precisely why the same
    /// arithmetic had to exist twice. If this number stays small, one
    /// implementation is enough.
    #[test]
    fn a_slider_can_afford_the_whole_picture() {
        // A full screen's worth of pixels, at a common desktop resolution.
        let micros = cost_of(2560, 1600);
        println!("4,1 milioni di pixel regolati in {} ms", micros / 1000);
        // The figure that matters is the release one - 38 ms measured, which is
        // what decides that a slider can drive the whole picture rather than a
        // downscaled stand-in. A debug build runs this an order of magnitude
        // slower for reasons that belong to the build and not to the code, so
        // the ceiling is scaled to match. At 500 000 flat this test had ten per
        // cent of headroom in debug and cried wolf whenever the machine was
        // busy, which is worse than not testing it.
        let ceiling = if cfg!(debug_assertions) { 3_000_000 } else { 500_000 };
        assert!(micros < ceiling, "troppo lento per uno slider: {} ms", micros / 1000);
    }

    #[test]
    fn the_cost_is_linear_in_the_pixels() {
        // The property that actually matters, and the one a wall clock was
        // standing in for: four times the pixels must cost about four times as
        // much. An accidentally quadratic pass would cost sixteen.
        let small = cost_of(640, 400).max(1);
        let large = cost_of(1280, 800);
        let ratio = large as f64 / small as f64;
        println!("4x i pixel costano {:.2}x", ratio);
        assert!(ratio < 8.0, "il costo non e lineare nei pixel: {:.2}x", ratio);
    }

    #[test]
    fn monochrome_flattens_to_one_value() {
        let mut pixels = RgbaImage::from_pixel(1, 1, image::Rgba([200, 100, 50, 255]));
        apply(&mut pixels, &Adjustments { monochrome: true, ..Default::default() });
        let out = pixels.get_pixel(0, 0);
        assert_eq!(out[0], out[1]);
        assert_eq!(out[1], out[2]);
    }
}
