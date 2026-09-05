// Taking a background out.
//
// Pick a colour with the dropper, and everything near it becomes transparent.
// Near is a distance in RGB space: the tolerance is the radius that disappears
// completely, the softness the width of the fade past it. Without that fade a
// hard threshold leaves a fringe of old background around anything antialiased -
// which is most artwork, and all text.
//
// The formula is the one the Python build uses, and for the same reason as the
// tone tools it now exists once rather than twice: the WebView2 build repeats it
// on a canvas in JavaScript so the preview can follow the sliders.

use image::{Rgba, RgbaImage};

/// The longest distance possible between two colours, corner to corner of the
/// cube: sqrt(3) * 255.
pub const MAX_DISTANCE: f32 = 441.672_96;

#[derive(Clone, Copy, PartialEq)]
pub struct Keying {
    pub colour: [u8; 3],
    /// Both 0-1, as fractions of MAX_DISTANCE.
    pub tolerance: f32,
    pub softness: f32,
}

impl Default for Keying {
    fn default() -> Keying {
        Keying { colour: [255, 255, 255], tolerance: 0.12, softness: 0.05 }
    }
}

/// The colour under a point given in fractions, read from the full-resolution
/// picture.
///
/// Sampling a downscaled preview instead would return an interpolated value:
/// the reduction blends neighbouring pixels, and near an edge that is a colour
/// present nowhere in the original - so the dropper would pick something that
/// is not there and key out the wrong thing.
pub fn sample(pixels: &RgbaImage, x: f32, y: f32) -> [u8; 3] {
    let px = ((x.clamp(0.0, 1.0) * (pixels.width().max(1) - 1) as f32).round() as u32)
        .min(pixels.width().saturating_sub(1));
    let py = ((y.clamp(0.0, 1.0) * (pixels.height().max(1) - 1) as f32).round() as u32)
        .min(pixels.height().saturating_sub(1));
    let pixel = pixels.get_pixel(px, py);
    [pixel[0], pixel[1], pixel[2]]
}

/// How opaque a pixel should remain, 0 to 255.
fn opacity(pixel: &Rgba<u8>, how: &Keying, inner: f32, scale: f32) -> u8 {
    let dr = pixel[0] as f32 - how.colour[0] as f32;
    let dg = pixel[1] as f32 - how.colour[1] as f32;
    let db = pixel[2] as f32 - how.colour[2] as f32;
    let distance = (dr * dr + dg * dg + db * db).sqrt();
    if scale <= 0.0 {
        // The limit of the ramp as its width reaches zero: keep what is far
        // enough away, drop the rest.
        return if distance > inner { 255 } else { 0 };
    }
    ((distance - inner) * scale).clamp(0.0, 255.0) as u8
}

/// Makes pixels near the chosen colour transparent.
pub fn key_out(pixels: &mut RgbaImage, how: &Keying) {
    let inner = how.tolerance.clamp(0.0, 1.0) * MAX_DISTANCE;
    let width = how.softness.clamp(0.0, 1.0) * MAX_DISTANCE;
    let scale = if width > 0.0 { 255.0 / width } else { 0.0 };
    for pixel in pixels.pixels_mut() {
        let keep = opacity(pixel, how, inner, scale);
        // Multiplied rather than replaced: a pixel already transparent from an
        // earlier pass has to stay transparent, or keying twice would bring
        // back what the first pass removed.
        pixel[3] = ((pixel[3] as u16 * keep as u16) / 255) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture() -> RgbaImage {
        let mut pixels = RgbaImage::new(3, 1);
        pixels.put_pixel(0, 0, Rgba([255, 255, 255, 255])); // the background
        pixels.put_pixel(1, 0, Rgba([250, 250, 250, 255])); // nearly it
        pixels.put_pixel(2, 0, Rgba([10, 20, 30, 255])); // the subject
        pixels
    }

    #[test]
    fn the_background_goes_and_the_subject_stays() {
        let mut pixels = picture();
        key_out(
            &mut pixels,
            &Keying { colour: [255, 255, 255], tolerance: 0.05, softness: 0.0 },
        );
        assert_eq!(pixels.get_pixel(0, 0)[3], 0);
        assert_eq!(pixels.get_pixel(2, 0)[3], 255);
    }

    #[test]
    fn softness_makes_an_edge_rather_than_a_cliff() {
        let mut hard = picture();
        key_out(&mut hard, &Keying { colour: [255, 255, 255], tolerance: 0.0, softness: 0.0 });
        let mut soft = picture();
        key_out(&mut soft, &Keying { colour: [255, 255, 255], tolerance: 0.0, softness: 0.1 });
        // The nearly-white pixel is all or nothing without softness, and part
        // way with it: that partial value is the whole point.
        let edge = soft.get_pixel(1, 0)[3];
        assert!(edge > 0 && edge < 255, "bordo netto invece che sfumato: {}", edge);
        assert_eq!(hard.get_pixel(1, 0)[3], 255);
    }

    #[test]
    fn keying_twice_does_not_bring_anything_back() {
        let mut pixels = picture();
        let how = Keying { colour: [255, 255, 255], tolerance: 0.05, softness: 0.0 };
        key_out(&mut pixels, &how);
        // A second pass for a colour that is nowhere near: what went must stay
        // gone.
        key_out(&mut pixels, &Keying { colour: [0, 255, 0], tolerance: 0.01, softness: 0.0 });
        assert_eq!(pixels.get_pixel(0, 0)[3], 0);
    }

    #[test]
    fn the_dropper_reads_the_pixel_it_points_at() {
        let pixels = picture();
        assert_eq!(sample(&pixels, 0.0, 0.0), [255, 255, 255]);
        assert_eq!(sample(&pixels, 1.0, 0.0), [10, 20, 30]);
    }
}
