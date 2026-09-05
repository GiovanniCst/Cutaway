// Marks put on top of a picture: rectangles, ellipses, arrows, lines, text,
// highlighter, eraser.
//
// Unlike the tone tools these create pixels rather than transforming them, so
// there is no per-pixel formula shared between preview and save. What is shared
// is the geometry: where an arrow's head sits, which points a stroke passes
// through, how a rectangle is ordered. That lives here once and both the live
// view and the rasteriser read it, which is the part that used to drift - in the
// WebView2 build these shapes are drawn as SVG for the preview and again with
// ImageDraw for the save, in two languages.
//
// Coordinates are pixels of the full-resolution picture, not fractions. A stroke
// width has to mean something absolute, and mixing one coordinate system with
// another is exactly the mismatch these tools keep producing.

use egui::Color32;
use image::{Rgba, RgbaImage};

pub const MIN_STROKE: f32 = 1.0;
pub const MAX_STROKE: f32 = 400.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Rect,
    Ellipse,
    Arrow,
    Line,
    Text,
    /// A numbered badge, for documenting a procedure step by step. It counts
    /// itself: the whole point is putting them down one after another without
    /// stopping to type a number each time.
    Number,
    Highlight,
    Erase,
}

impl Kind {
    pub fn label(&self) -> &'static str {
        match self {
            Kind::Rect => crate::words::w().rect,
            Kind::Ellipse => crate::words::w().ellipse,
            Kind::Arrow => crate::words::w().arrow,
            Kind::Line => crate::words::w().line,
            Kind::Text => crate::words::w().text,
            Kind::Number => crate::words::w().number,
            Kind::Highlight => crate::words::w().highlight,
            Kind::Erase => crate::words::w().eraser,
        }
    }

    /// Freehand kinds carry a trail of points rather than two corners, and
    /// compose differently: one is painted through, the other rubs out.
    pub fn freehand(&self) -> bool {
        matches!(self, Kind::Highlight | Kind::Erase)
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Shape {
    pub kind: Kind,
    /// The two corners, or the ends of a line. Unused by freehand kinds.
    pub from: (f32, f32),
    pub to: (f32, f32),
    /// Where a freehand stroke went.
    pub trail: Vec<(f32, f32)>,
    pub colour: Color32,
    pub width: f32,
    pub text: String,
    pub size: f32,
    /// Which step this badge is, when it is one.
    pub number: u32,
}

impl Shape {
    pub fn new(kind: Kind, at: (f32, f32), colour: Color32, width: f32) -> Shape {
        Shape {
            kind,
            from: at,
            to: at,
            trail: if kind.freehand() { vec![at] } else { Vec::new() },
            colour,
            width,
            text: String::new(),
            size: 48.0,
            number: 1,
        }
    }

    /// The corners in order, whichever way the drag went.
    pub fn ordered(&self) -> (f32, f32, f32, f32) {
        (
            self.from.0.min(self.to.0),
            self.from.1.min(self.to.1),
            self.from.0.max(self.to.0),
            self.from.1.max(self.to.1),
        )
    }

    /// Big enough to have been meant. A click that placed nothing should leave
    /// nothing behind.
    pub fn worth_keeping(&self) -> bool {
        match self.kind {
            Kind::Text => !self.text.trim().is_empty(),
            // A badge is placed by a click and has a size of its own: there is
            // nothing to drag out, so there is nothing to be too small.
            Kind::Number => true,
            _ if self.kind.freehand() => self.trail.len() > 1,
            Kind::Line | Kind::Arrow => {
                let (dx, dy) = (self.to.0 - self.from.0, self.to.1 - self.from.1);
                (dx * dx + dy * dy).sqrt() > 3.0
            }
            _ => {
                let (left, top, right, bottom) = self.ordered();
                right - left > 3.0 && bottom - top > 3.0
            }
        }
    }

    /// Moves the whole mark by a delta, which is what dragging a placed one does.
    pub fn moved_by(&self, dx: f32, dy: f32) -> Shape {
        let mut moved = self.clone();
        moved.from = (self.from.0 + dx, self.from.1 + dy);
        moved.to = (self.to.0 + dx, self.to.1 + dy);
        moved.trail = self.trail.iter().map(|(x, y)| (x + dx, y + dy)).collect();
        moved
    }

    /// The box a mark occupies, for hit testing and for the selection outline.
    pub fn bounds(&self) -> (f32, f32, f32, f32) {
        if self.kind.freehand() && !self.trail.is_empty() {
            let mut left = f32::MAX;
            let mut top = f32::MAX;
            let mut right = f32::MIN;
            let mut bottom = f32::MIN;
            for (x, y) in &self.trail {
                left = left.min(*x);
                top = top.min(*y);
                right = right.max(*x);
                bottom = bottom.max(*y);
            }
            let half = self.width / 2.0;
            return (left - half, top - half, right + half, bottom + half);
        }
        if self.kind == Kind::Number {
            let radius = self.badge_radius();
            return (
                self.from.0 - radius,
                self.from.1 - radius,
                self.from.0 + radius,
                self.from.1 + radius,
            );
        }
        if self.kind == Kind::Text {
            // Roughly what the glyphs will occupy: enough to grab, and the real
            // extent is not known until it is laid out.
            let width = self.size * 0.6 * self.text.chars().count().max(1) as f32;
            return (self.from.0, self.from.1, self.from.0 + width, self.from.1 + self.size);
        }
        let (left, top, right, bottom) = self.ordered();
        let half = self.width / 2.0;
        (left - half, top - half, right + half, bottom + half)
    }

    /// How big the badge is drawn. Tied to the stroke width, so the one slider
    /// that governs how heavy a mark is governs this too.
    pub fn badge_radius(&self) -> f32 {
        (self.width * 3.5).max(12.0)
    }

    pub fn contains(&self, x: f32, y: f32, reach: f32) -> bool {
        let (left, top, right, bottom) = self.bounds();
        x >= left - reach && x <= right + reach && y >= top - reach && y <= bottom + reach
    }
}

/// Where an arrow's head goes: the two barbs, computed from the shaft.
///
/// Shared by the preview and the rasteriser, because an arrow whose head moves
/// when it is applied is worse than one with no head at all.
pub fn arrow_head(from: (f32, f32), to: (f32, f32), width: f32) -> [(f32, f32); 2] {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let length = (dx * dx + dy * dy).sqrt().max(1e-3);
    let angle = dy.atan2(dx);
    // The head grows with the stroke but stops growing past a point, or a thick
    // arrow becomes a triangle with a tail. Never longer than half the shaft,
    // and on a very short arrow that half is what decides.
    //
    // Written as a clamp, this crashed the program: `f32::clamp` panics when
    // the low bound is above the high one, and any arrow shorter than 20 px
    // puts it there. Short arrows are what a small screenshot is made of, so
    // this was not a corner case - it was most of the arrows anybody would draw
    // on a captured button.
    let arm = (width * 4.0).max(10.0).min(length * 0.5).max(1.0);
    let spread = 0.45_f32;
    [
        (
            to.0 - arm * (angle - spread).cos(),
            to.1 - arm * (angle - spread).sin(),
        ),
        (
            to.0 - arm * (angle + spread).cos(),
            to.1 - arm * (angle + spread).sin(),
        ),
    ]
}

/// Fills in the gaps in a freehand trail.
///
/// A pointer reports positions, not a continuous line: at speed the gaps between
/// them are wider than the brush, and a highlighter drawn quickly comes out as a
/// row of dots. The points between are put back before anything is painted.
pub fn densify(trail: &[(f32, f32)], step: f32) -> Vec<(f32, f32)> {
    if trail.len() < 2 {
        return trail.to_vec();
    }
    let step = step.max(1.0);
    let mut out = vec![trail[0]];
    for pair in trail.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let length = (dx * dx + dy * dy).sqrt();
        let steps = (length / step).ceil() as usize;
        for i in 1..=steps {
            let part = i as f32 / steps as f32;
            out.push((from.0 + dx * part, from.1 + dy * part));
        }
    }
    out
}

// --- putting marks into pixels ---------------------------------------------

fn blend(under: Rgba<u8>, over: Color32) -> Rgba<u8> {
    let alpha = over.a() as f32 / 255.0;
    if alpha >= 1.0 {
        return Rgba([over.r(), over.g(), over.b(), 255.max(under[3])]);
    }
    let mix = |a: u8, b: u8| (b as f32 * alpha + a as f32 * (1.0 - alpha)).round() as u8;
    Rgba([
        mix(under[0], over.r()),
        mix(under[1], over.g()),
        mix(under[2], over.b()),
        under[3].max((alpha * 255.0) as u8),
    ])
}

fn dot(pixels: &mut RgbaImage, x: f32, y: f32, radius: f32, colour: Color32) {
    let radius = radius.max(0.5);
    let (left, top) = ((x - radius).floor() as i64, (y - radius).floor() as i64);
    let (right, bottom) = ((x + radius).ceil() as i64, (y + radius).ceil() as i64);
    for py in top..=bottom {
        for px in left..=right {
            if px < 0 || py < 0 || px >= pixels.width() as i64 || py >= pixels.height() as i64 {
                continue;
            }
            let (dx, dy) = (px as f32 + 0.5 - x, py as f32 + 0.5 - y);
            if dx * dx + dy * dy <= radius * radius {
                let under = *pixels.get_pixel(px as u32, py as u32);
                pixels.put_pixel(px as u32, py as u32, blend(under, colour));
            }
        }
    }
}

/// A line of a given thickness, drawn as a run of discs.
///
/// Not the fastest way, and deliberately so: a disc per step gives round ends
/// and round joins for free, which is what a drawn stroke looks like. The cost
/// is paid once, when the marks are applied.
fn thick_line(
    pixels: &mut RgbaImage,
    from: (f32, f32),
    to: (f32, f32),
    width: f32,
    colour: Color32,
) {
    let radius = (width / 2.0).max(0.5);
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let length = (dx * dx + dy * dy).sqrt();
    let steps = (length / (radius * 0.5).max(0.5)).ceil().max(1.0) as usize;
    for i in 0..=steps {
        let part = i as f32 / steps as f32;
        dot(pixels, from.0 + dx * part, from.1 + dy * part, radius, colour);
    }
}

fn rect_outline(pixels: &mut RgbaImage, shape: &Shape) {
    let (left, top, right, bottom) = shape.ordered();
    for (from, to) in [
        ((left, top), (right, top)),
        ((right, top), (right, bottom)),
        ((right, bottom), (left, bottom)),
        ((left, bottom), (left, top)),
    ] {
        thick_line(pixels, from, to, shape.width, shape.colour);
    }
}

fn ellipse_outline(pixels: &mut RgbaImage, shape: &Shape) {
    let (left, top, right, bottom) = shape.ordered();
    let (cx, cy) = ((left + right) / 2.0, (top + bottom) / 2.0);
    let (rx, ry) = ((right - left) / 2.0, (bottom - top) / 2.0);
    if rx <= 0.0 || ry <= 0.0 {
        return;
    }
    // Enough steps that the ellipse is smooth at its own size rather than at
    // some fixed count that looks like a polygon when it is large.
    let steps = ((rx + ry) * 2.0).clamp(32.0, 2048.0) as usize;
    let mut previous = (cx + rx, cy);
    for i in 1..=steps {
        let angle = i as f32 / steps as f32 * std::f32::consts::TAU;
        let at = (cx + rx * angle.cos(), cy + ry * angle.sin());
        thick_line(pixels, previous, at, shape.width, shape.colour);
        previous = at;
    }
}

/// The eraser: puts back what was underneath, which for a picture with no
/// history means clearing to transparent.
fn erase(pixels: &mut RgbaImage, shape: &Shape) {
    let radius = (shape.width / 2.0).max(0.5);
    for (x, y) in densify(&shape.trail, radius * 0.5) {
        let (left, top) = ((x - radius).floor() as i64, (y - radius).floor() as i64);
        let (right, bottom) = ((x + radius).ceil() as i64, (y + radius).ceil() as i64);
        for py in top..=bottom {
            for px in left..=right {
                if px < 0 || py < 0 || px >= pixels.width() as i64 || py >= pixels.height() as i64 {
                    continue;
                }
                let (dx, dy) = (px as f32 + 0.5 - x, py as f32 + 0.5 - y);
                if dx * dx + dy * dy <= radius * radius {
                    pixels.put_pixel(px as u32, py as u32, Rgba([0, 0, 0, 0]));
                }
            }
        }
    }
}

/// What each instrument is as wide as when it is first picked up.
///
/// Three numbers and not one. They were one, and the eraser inherited whatever
/// the pen was set to: a three-pixel eraser dragged across a four-megapixel
/// photograph removes a hair and reads, correctly, as an eraser that does not
/// erase. The WebView2 build keeps the same three, at the same sizes.
pub const STROKE_WIDTH: f32 = 6.0;
pub const HIGHLIGHT_WIDTH: f32 = 28.0;
pub const ERASER_WIDTH: f32 = 40.0;

/// Draws one mark into the pixels.
pub fn draw(pixels: &mut RgbaImage, shape: &Shape) {
    match shape.kind {
        Kind::Rect => rect_outline(pixels, shape),
        Kind::Ellipse => ellipse_outline(pixels, shape),
        Kind::Line => thick_line(pixels, shape.from, shape.to, shape.width, shape.colour),
        Kind::Arrow => {
            thick_line(pixels, shape.from, shape.to, shape.width, shape.colour);
            for barb in arrow_head(shape.from, shape.to, shape.width) {
                thick_line(pixels, shape.to, barb, shape.width, shape.colour);
            }
        }
        Kind::Highlight => {
            // Painted through in one pass over a densified trail: going over the
            // same spot twice with a translucent colour would darken it, and a
            // highlighter that darkens where the hand slowed is not one.
            let radius = (shape.width / 2.0).max(0.5);
            let mut painted = std::collections::HashSet::new();
            for (x, y) in densify(&shape.trail, radius * 0.5) {
                let (left, top) = ((x - radius).floor() as i64, (y - radius).floor() as i64);
                let (right, bottom) = ((x + radius).ceil() as i64, (y + radius).ceil() as i64);
                for py in top..=bottom {
                    for px in left..=right {
                        if px < 0
                            || py < 0
                            || px >= pixels.width() as i64
                            || py >= pixels.height() as i64
                        {
                            continue;
                        }
                        let (dx, dy) = (px as f32 + 0.5 - x, py as f32 + 0.5 - y);
                        if dx * dx + dy * dy <= radius * radius && painted.insert((px, py)) {
                            let under = *pixels.get_pixel(px as u32, py as u32);
                            pixels.put_pixel(px as u32, py as u32, blend(under, shape.colour));
                        }
                    }
                }
            }
        }
        Kind::Number => {
            let radius = shape.badge_radius();
            dot(pixels, shape.from.0, shape.from.1, radius, shape.colour);
            let ink = crate::skin::over(shape.colour);
            draw_text(
                pixels,
                &shape.number.to_string(),
                shape.from,
                (radius * 1.2).max(8.0),
                ink,
                true,
            );
        }
        Kind::Erase => erase(pixels, shape),
        Kind::Text => {
            draw_text(pixels, &shape.text, shape.from, shape.size, shape.colour, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank() -> RgbaImage {
        RgbaImage::from_pixel(100, 100, Rgba([255, 255, 255, 255]))
    }

    #[test]
    fn a_click_leaves_nothing() {
        let dot = Shape::new(Kind::Rect, (10.0, 10.0), Color32::RED, 4.0);
        assert!(!dot.worth_keeping());
    }

    #[test]
    fn a_backwards_rectangle_is_still_a_rectangle() {
        let mut shape = Shape::new(Kind::Rect, (80.0, 90.0), Color32::RED, 2.0);
        shape.to = (20.0, 30.0);
        let (left, top, right, bottom) = shape.ordered();
        assert!(left < right && top < bottom);
        assert_eq!((left, top), (20.0, 30.0));
    }

    #[test]
    fn a_line_puts_ink_on_the_picture() {
        let mut pixels = blank();
        let mut shape = Shape::new(Kind::Line, (10.0, 50.0), Color32::RED, 5.0);
        shape.to = (90.0, 50.0);
        draw(&mut pixels, &shape);
        assert_eq!(pixels.get_pixel(50, 50)[0], 255);
        assert_eq!(pixels.get_pixel(50, 50)[1], 0);
        // And not everywhere: a mark that covers the picture is not a mark.
        assert_eq!(pixels.get_pixel(50, 10)[1], 255);
    }

    #[test]
    fn an_arrow_has_a_head_on_the_far_end() {
        let barbs = arrow_head((0.0, 0.0), (100.0, 0.0), 4.0);
        // Both barbs sit behind the point, not beyond it.
        for (x, _) in barbs {
            assert!(x < 100.0 && x > 50.0);
        }
        // And on opposite sides of the shaft.
        assert!(barbs[0].1 * barbs[1].1 < 0.0);
    }

    #[test]
    fn a_fast_stroke_is_not_a_row_of_dots() {
        let sparse = vec![(0.0, 0.0), (100.0, 0.0)];
        let filled = densify(&sparse, 2.0);
        assert!(filled.len() > 40, "solo {} punti", filled.len());
    }

    #[test]
    fn the_highlighter_does_not_darken_where_it_overlaps() {
        let mut pixels = blank();
        let mut shape = Shape::new(
            Kind::Highlight,
            (20.0, 50.0),
            Color32::from_rgba_unmultiplied(255, 240, 0, 100),
            20.0,
        );
        // A trail that doubles back over itself, which is what a hand does.
        shape.trail = vec![(20.0, 50.0), (80.0, 50.0), (20.0, 50.0)];
        draw(&mut pixels, &shape);
        let once = *pixels.get_pixel(78, 50);
        let twice = *pixels.get_pixel(50, 50);
        assert_eq!(once, twice, "il doppio passaggio ha scurito");
    }

    #[test]
    fn the_eraser_leaves_holes() {
        let mut pixels = blank();
        let mut shape = Shape::new(Kind::Erase, (50.0, 50.0), Color32::WHITE, 10.0);
        shape.trail = vec![(50.0, 50.0), (60.0, 50.0)];
        draw(&mut pixels, &shape);
        assert_eq!(pixels.get_pixel(55, 50)[3], 0);
        assert_eq!(pixels.get_pixel(10, 10)[3], 255);
    }
}

// --- text, which needs a font ----------------------------------------------

/// The fonts to try, in order. All four ship with Windows, so nothing has to be
/// bundled and no font licence enters the project.
const FONT_CANDIDATES: &[&str] = &["segoeui.ttf", "arial.ttf", "calibri.ttf", "tahoma.ttf"];

fn system_font() -> Option<ab_glyph::FontVec> {
    let root = std::env::var_os("WINDIR").map(std::path::PathBuf::from)?;
    for name in FONT_CANDIDATES {
        let path = root.join("Fonts").join(name);
        if let Ok(bytes) = std::fs::read(&path) {
            if let Ok(font) = ab_glyph::FontVec::try_from_vec(bytes) {
                return Some(font);
            }
        }
    }
    None
}

/// Draws a string into the pixels, left-aligned from a baseline-less top-left
/// corner - the same anchor the live view uses, so applying does not shift it.
///
/// Returns false when no font could be loaded, which is the one case where a
/// mark cannot be honoured and the caller has to say so rather than silently
/// dropping it.
pub fn draw_text(
    pixels: &mut RgbaImage,
    text: &str,
    at: (f32, f32),
    size: f32,
    colour: Color32,
    centred: bool,
) -> bool {
    use ab_glyph::{Font, ScaleFont};
    let Some(font) = system_font() else { return false };
    let scaled = font.as_scaled(ab_glyph::PxScale::from(size));

    // Laid out first, so a centred string can be moved by half its width.
    let mut advance = 0.0_f32;
    for ch in text.chars() {
        advance += scaled.h_advance(scaled.glyph_id(ch));
    }
    let (mut pen_x, top) = if centred {
        (at.0 - advance / 2.0, at.1 - scaled.height() / 2.0)
    } else {
        at
    };
    let baseline = top + scaled.ascent();

    for ch in text.chars() {
        // scaled_glyph already carries the scale; only the position is missing.
        let mut glyph = scaled.scaled_glyph(ch);
        glyph.position = ab_glyph::point(pen_x, baseline);
        if let Some(outline) = font.outline_glyph(glyph) {
            let bounds = outline.px_bounds();
            outline.draw(|gx, gy, coverage| {
                if coverage <= 0.0 {
                    return;
                }
                let px = bounds.min.x as i64 + gx as i64;
                let py = bounds.min.y as i64 + gy as i64;
                if px < 0 || py < 0 || px >= pixels.width() as i64 || py >= pixels.height() as i64 {
                    return;
                }
                // Coverage is the antialiasing: the glyph's own alpha, multiplied
                // into the colour's, which is what keeps edges smooth instead of
                // stepped.
                let ink = Color32::from_rgba_unmultiplied(
                    colour.r(),
                    colour.g(),
                    colour.b(),
                    (coverage * colour.a() as f32).round() as u8,
                );
                let under = *pixels.get_pixel(px as u32, py as u32);
                pixels.put_pixel(px as u32, py as u32, blend(under, ink));
            });
        }
        pen_x += scaled.h_advance(scaled.glyph_id(ch));
    }
    true
}

#[cfg(test)]
mod text_tests {
    use super::*;

    #[test]
    fn text_lands_in_the_pixels() {
        let mut pixels = RgbaImage::from_pixel(300, 100, Rgba([255, 255, 255, 255]));
        let drawn = draw_text(&mut pixels, "Cutaway", (10.0, 20.0), 48.0, Color32::BLACK, false);
        assert!(drawn, "nessun font di sistema trovato");
        // Something dark somewhere in the first half, and paper at the far end.
        let inked = pixels
            .enumerate_pixels()
            .filter(|(x, _, p)| *x < 200 && p[0] < 128)
            .count();
        assert!(inked > 50, "solo {} pixel di inchiostro", inked);
        assert_eq!(pixels.get_pixel(295, 95)[0], 255);
    }

    #[test]
    fn a_badge_carries_its_number() {
        let mut pixels = RgbaImage::from_pixel(120, 120, Rgba([255, 255, 255, 255]));
        let mut badge = Shape::new(Kind::Number, (60.0, 60.0), Color32::from_rgb(0xE5, 0x3E, 0x3E), 8.0);
        badge.number = 7;
        draw(&mut pixels, &badge);
        // The disc is there.
        assert_eq!(pixels.get_pixel(60, 32)[0], 0xE5);
        // And something light inside it, which is the digit on the red.
        let light = (45..75)
            .flat_map(|y| (45..75).map(move |x| (x, y)))
            .filter(|(x, y)| pixels.get_pixel(*x, *y)[1] > 200)
            .count();
        assert!(light > 10, "la cifra non si vede: {} pixel", light);
    }
}

#[cfg(test)]
mod short_marks {
    use super::*;

    /// A mark drawn on a small screenshot, which is what most screenshots are.
    fn arrow(from: (f32, f32), to: (f32, f32), width: f32) -> Shape {
        let mut shape = Shape::new(Kind::Arrow, from, Color32::RED, width);
        shape.to = to;
        shape
    }

    #[test]
    fn a_short_arrow_does_not_bring_the_program_down() {
        // This crashed: the head was sized with `clamp(10.0, length / 2.0)`,
        // and `f32::clamp` panics when the low bound is above the high one -
        // which every arrow shorter than 20 px does. Drawing one on a captured
        // button killed the window.
        for length in [1.0_f32, 4.0, 9.0, 19.0, 20.0, 21.0, 200.0] {
            let head = arrow_head((0.0, 0.0), (length, 0.0), 4.0);
            for (x, y) in head {
                assert!(x.is_finite() && y.is_finite(), "lunghezza {}", length);
            }
        }
    }

    #[test]
    fn the_head_never_outgrows_the_shaft() {
        // A head longer than the arrow is a triangle with the tail on the wrong
        // side of it.
        for length in [6.0_f32, 12.0, 40.0] {
            for width in [1.0_f32, 8.0, 64.0] {
                let head = arrow_head((0.0, 0.0), (length, 0.0), width);
                for (x, _) in head {
                    // The barbs are measured back from the tip.
                    assert!(
                        x >= -0.001,
                        "lunghezza {} spessore {}: barba a {}",
                        length,
                        width,
                        x
                    );
                }
            }
        }
    }

    #[test]
    fn a_short_arrow_can_be_rasterised_on_a_small_picture() {
        // The whole path, not only the arithmetic: a tiny picture with a tiny
        // arrow, which is the case that used to bring the program down.
        let mut pixels = RgbaImage::from_pixel(40, 24, Rgba([255, 255, 255, 255]));
        draw(&mut pixels, &arrow((4.0, 4.0), (12.0, 9.0), 4.0));
        // And one drawn off the edge, which is what a dragged arrow does.
        draw(&mut pixels, &arrow((30.0, 20.0), (120.0, 90.0), 6.0));
        draw(&mut pixels, &arrow((-40.0, -30.0), (5.0, 5.0), 2.0));
        // A zero-length one, which a click without a drag produces.
        draw(&mut pixels, &arrow((10.0, 10.0), (10.0, 10.0), 4.0));
    }
}

/// How wide a string will be at this size, before drawing it.
///
/// The rasteriser lays the text out to place it; this asks the same question
/// without putting anything down, which is what a caption needs in order to sit
/// against a right-hand edge.
pub fn text_width(text: &str, size: f32) -> f32 {
    use ab_glyph::{Font, ScaleFont};
    let Some(font) = system_font() else { return 0.0 };
    let scaled = font.as_scaled(ab_glyph::PxScale::from(size));
    text.chars().map(|ch| scaled.h_advance(scaled.glyph_id(ch))).sum()
}
