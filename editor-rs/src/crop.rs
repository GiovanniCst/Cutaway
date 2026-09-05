// The blade.
//
// The selection is kept in fractions of the picture rather than in pixels, which
// is what the WebView2 build does too and for the same reason: the rectangle
// survives a zoom, a resize of the window, and being drawn at a size that has
// nothing to do with the picture's own. Pixels come back only at the moment of
// cutting.

use egui::{Pos2, Rect, Vec2};
use image::RgbaImage;

/// Below this the drag is a slip, not a selection - in fractions, so it means
/// the same thing on a thumbnail and on a photograph.
const MINIMUM: f32 = 0.005;

/// How close to an edge counts as grabbing it, in points on screen.
pub const REACH: f32 = 10.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Selection {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Selection {
    /// The whole picture, which is where a fresh selection starts.
    pub fn whole() -> Selection {
        Selection { left: 0.0, top: 0.0, right: 1.0, bottom: 1.0 }
    }

    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    pub fn empty(&self) -> bool {
        self.width() < MINIMUM || self.height() < MINIMUM
    }

    /// Normalised and held inside the picture: a drag can go both backwards and
    /// off the edge, and neither should produce a rectangle that cannot exist.
    pub fn tidy(self) -> Selection {
        Selection {
            left: self.left.min(self.right).clamp(0.0, 1.0),
            top: self.top.min(self.bottom).clamp(0.0, 1.0),
            right: self.left.max(self.right).clamp(0.0, 1.0),
            bottom: self.top.max(self.bottom).clamp(0.0, 1.0),
        }
    }

    /// Where the selection falls on screen, given where the picture is drawn.
    pub fn on_screen(&self, shown: Rect) -> Rect {
        Rect::from_min_max(
            Pos2::new(
                shown.left() + self.left * shown.width(),
                shown.top() + self.top * shown.height(),
            ),
            Pos2::new(
                shown.left() + self.right * shown.width(),
                shown.top() + self.bottom * shown.height(),
            ),
        )
    }

    /// A point on screen, as a fraction of the picture.
    pub fn fraction_of(at: Pos2, shown: Rect) -> Vec2 {
        Vec2::new(
            ((at.x - shown.left()) / shown.width().max(1.0)).clamp(0.0, 1.0),
            ((at.y - shown.top()) / shown.height().max(1.0)).clamp(0.0, 1.0),
        )
    }

    /// The pixels this selection covers, as whole pixels.
    ///
    /// Rounded rather than truncated, and never smaller than one pixel: a
    /// selection that rounds to nothing would produce an image with no area,
    /// which every encoder refuses and no message explains.
    pub fn in_pixels(&self, width: u32, height: u32) -> (u32, u32, u32, u32) {
        let left = (self.left * width as f32).round().clamp(0.0, width as f32 - 1.0) as u32;
        let top = (self.top * height as f32).round().clamp(0.0, height as f32 - 1.0) as u32;
        let right = (self.right * width as f32).round().clamp(0.0, width as f32) as u32;
        let bottom = (self.bottom * height as f32).round().clamp(0.0, height as f32) as u32;
        (left, top, (right - left).max(1), (bottom - top).max(1))
    }
}

/// Which part of the selection a pointer is over, and therefore what dragging
/// it would do.
#[derive(Clone, Copy, PartialEq)]
pub enum Grip {
    None,
    Inside,
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Grip {
    pub fn at(pointer: Pos2, box_on_screen: Rect) -> Grip {
        let near_left = (pointer.x - box_on_screen.left()).abs() <= REACH;
        let near_right = (pointer.x - box_on_screen.right()).abs() <= REACH;
        let near_top = (pointer.y - box_on_screen.top()).abs() <= REACH;
        let near_bottom = (pointer.y - box_on_screen.bottom()).abs() <= REACH;
        let within_x = pointer.x >= box_on_screen.left() - REACH
            && pointer.x <= box_on_screen.right() + REACH;
        let within_y = pointer.y >= box_on_screen.top() - REACH
            && pointer.y <= box_on_screen.bottom() + REACH;

        // Corners first: near two edges at once is a corner, and testing edges
        // first would make the corners unreachable.
        match (near_left, near_right, near_top, near_bottom) {
            (true, _, true, _) => Grip::TopLeft,
            (_, true, true, _) => Grip::TopRight,
            (true, _, _, true) => Grip::BottomLeft,
            (_, true, _, true) => Grip::BottomRight,
            (true, _, _, _) if within_y => Grip::Left,
            (_, true, _, _) if within_y => Grip::Right,
            (_, _, true, _) if within_x => Grip::Top,
            (_, _, _, true) if within_x => Grip::Bottom,
            _ if box_on_screen.contains(pointer) => Grip::Inside,
            _ => Grip::None,
        }
    }

    pub fn cursor(&self) -> egui::CursorIcon {
        match self {
            Grip::None => egui::CursorIcon::Crosshair,
            Grip::Inside => egui::CursorIcon::Move,
            Grip::Left | Grip::Right => egui::CursorIcon::ResizeHorizontal,
            Grip::Top | Grip::Bottom => egui::CursorIcon::ResizeVertical,
            Grip::TopLeft | Grip::BottomRight => egui::CursorIcon::ResizeNwSe,
            Grip::TopRight | Grip::BottomLeft => egui::CursorIcon::ResizeNeSw,
        }
    }

    /// Moves the selection by a drag, according to what was grabbed.
    pub fn drag(&self, from: Selection, by: Vec2) -> Selection {
        let mut to = from;
        match self {
            Grip::Inside => {
                // Moved whole, and held inside the picture rather than clamped
                // edge by edge, which would deform it against a border.
                // `min` and `max` rather than `clamp`, which panics when the low
                // bound is above the high one. A tidy selection cannot put it
                // there - but relying on that is exactly how the arrow head
                // crashed the program, so the same shape of bug is closed here
                // before it has to be found.
                let dx = by.x.max(-from.left).min((1.0 - from.right).max(-from.left));
                let dy = by.y.max(-from.top).min((1.0 - from.bottom).max(-from.top));
                to.left += dx;
                to.right += dx;
                to.top += dy;
                to.bottom += dy;
            }
            Grip::Left => to.left += by.x,
            Grip::Right => to.right += by.x,
            Grip::Top => to.top += by.y,
            Grip::Bottom => to.bottom += by.y,
            Grip::TopLeft => {
                to.left += by.x;
                to.top += by.y;
            }
            Grip::TopRight => {
                to.right += by.x;
                to.top += by.y;
            }
            Grip::BottomLeft => {
                to.left += by.x;
                to.bottom += by.y;
            }
            Grip::BottomRight => {
                to.right += by.x;
                to.bottom += by.y;
            }
            Grip::None => {}
        }
        to.tidy()
    }
}

/// Cuts the picture down to the selection.
pub fn cut(pixels: &RgbaImage, to: Selection) -> RgbaImage {
    let (x, y, width, height) = to.in_pixels(pixels.width(), pixels.height());
    image::imageops::crop_imm(pixels, x, y, width, height).to_image()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_backwards_drag_still_makes_a_rectangle() {
        let drawn = Selection { left: 0.8, top: 0.9, right: 0.2, bottom: 0.1 }.tidy();
        assert!(drawn.left < drawn.right && drawn.top < drawn.bottom);
        assert_eq!(drawn.left, 0.2);
        assert_eq!(drawn.bottom, 0.9);
    }

    #[test]
    fn nothing_ever_leaves_the_picture() {
        let pushed = Selection { left: -0.5, top: -0.5, right: 1.5, bottom: 1.5 }.tidy();
        assert_eq!(pushed, Selection::whole());
    }

    #[test]
    fn moving_the_whole_box_keeps_its_size() {
        let from = Selection { left: 0.1, top: 0.1, right: 0.4, bottom: 0.5 };
        let to = Grip::Inside.drag(from, Vec2::new(0.9, 0.9));
        assert!((to.width() - from.width()).abs() < 1e-6);
        assert!((to.height() - from.height()).abs() < 1e-6);
        // And it stopped at the edge rather than going through it.
        assert!(to.right <= 1.0 && to.bottom <= 1.0);
    }

    #[test]
    fn a_cut_is_never_empty() {
        let sliver = Selection { left: 0.5, top: 0.5, right: 0.5001, bottom: 0.5001 };
        let (_, _, width, height) = sliver.in_pixels(100, 100);
        assert!(width >= 1 && height >= 1);
    }

    #[test]
    fn the_cut_is_the_size_it_claims() {
        let picture = RgbaImage::from_pixel(200, 100, image::Rgba([1, 2, 3, 4]));
        let half = Selection { left: 0.25, top: 0.0, right: 0.75, bottom: 1.0 };
        let cut = cut(&picture, half);
        assert_eq!((cut.width(), cut.height()), (100, 100));
    }
}

// --- the ratios worth having a button for -----------------------------------

/// The shapes people ask a screenshot to be, and what each is called.
///
/// The 1.6 build had these and this one lost them in the rewrite; they are the
/// difference between cropping to 16:9 and cropping to nearly 16:9 by hand and
/// finding out later.
pub const RATIOS: &[(&str, f32)] =
    &[("1:1", 1.0), ("4:3", 4.0 / 3.0), ("3:2", 1.5), ("16:9", 16.0 / 9.0)];

impl Selection {
    /// The largest rectangle of this ratio that fits in the picture, centred on
    /// where the selection is now.
    ///
    /// Centred on the current selection rather than on the picture: somebody who
    /// has already framed roughly what they want and then asks for 16:9 wants
    /// that thing in 16:9, not the middle of the photograph.
    ///
    /// `ratio` is width over height in *pixels*, so the picture's own dimensions
    /// have to come in: a selection is held in fractions, and a fraction that is
    /// square is only a square picture on a square photograph.
    pub fn at_ratio(self, ratio: f32, width: u32, height: u32) -> Selection {
        if ratio <= 0.0 || width == 0 || height == 0 {
            return self;
        }
        let (picture_w, picture_h) = (width as f32, height as f32);
        // In fractions of the picture, the wanted ratio is this.
        let wanted = ratio * picture_h / picture_w;

        let (middle_x, middle_y) = (
            (self.left + self.right) / 2.0,
            (self.top + self.bottom) / 2.0,
        );
        // Start from the current size and grow or shrink one side to suit.
        let (mut w, mut h) = (self.width(), self.height());
        if w / h > wanted {
            w = h * wanted;
        } else {
            h = w / wanted;
        }
        // Then hold it inside the picture, keeping the ratio: shrinking both
        // sides by the same factor is what keeps it.
        let room = (1.0 / w).min(1.0 / h).min(1.0);
        let (w, h) = (w * room, h * room);

        let left = (middle_x - w / 2.0).clamp(0.0, 1.0 - w);
        let top = (middle_y - h / 2.0).clamp(0.0, 1.0 - h);
        Selection { left, top, right: left + w, bottom: top + h }
    }
}

#[cfg(test)]
mod ratio_tests {
    use super::*;

    /// The ratio that matters is the one in pixels, so it is checked there.
    fn measured(selection: Selection, width: u32, height: u32) -> f32 {
        let (_, _, w, h) = selection.in_pixels(width, height);
        w as f32 / h as f32
    }

    #[test]
    fn a_preset_gives_that_ratio_in_pixels() {
        // A landscape photograph: a square selection is not a square fraction.
        for (name, ratio) in RATIOS {
            let cut = Selection::whole().at_ratio(*ratio, 1920, 1080);
            let got = measured(cut, 1920, 1080);
            assert!(
                (got - ratio).abs() < 0.02,
                "{}: chiesto {:.3}, ottenuto {:.3}",
                name,
                ratio,
                got
            );
        }
    }

    #[test]
    fn it_works_on_a_tall_picture_too() {
        for (name, ratio) in RATIOS {
            let cut = Selection::whole().at_ratio(*ratio, 600, 1600);
            let got = measured(cut, 600, 1600);
            assert!((got - ratio).abs() < 0.02, "{}: ottenuto {:.3}", name, got);
        }
    }

    #[test]
    fn it_stays_inside_the_picture() {
        // Asked from a corner, a wide ratio must not run off the edge.
        let corner = Selection { left: 0.8, top: 0.8, right: 1.0, bottom: 1.0 };
        let cut = corner.at_ratio(16.0 / 9.0, 1920, 1080);
        assert!(cut.left >= 0.0 && cut.top >= 0.0, "{:?}", cut);
        assert!(cut.right <= 1.0001 && cut.bottom <= 1.0001, "{:?}", cut);
    }

    #[test]
    fn it_keeps_the_middle_where_it_was() {
        // Somebody who framed something and then asks for a ratio wants that
        // thing in that ratio, not the middle of the photograph.
        let framed = Selection { left: 0.1, top: 0.1, right: 0.3, bottom: 0.3 };
        let cut = framed.at_ratio(1.0, 1000, 1000);
        let middle = ((cut.left + cut.right) / 2.0, (cut.top + cut.bottom) / 2.0);
        assert!((middle.0 - 0.2).abs() < 0.001, "{:?}", cut);
        assert!((middle.1 - 0.2).abs() < 0.001, "{:?}", cut);
    }

    #[test]
    fn a_nonsense_ratio_leaves_the_selection_alone() {
        let was = Selection { left: 0.1, top: 0.2, right: 0.4, bottom: 0.5 };
        assert_eq!(was.at_ratio(0.0, 800, 600), was);
        assert_eq!(was.at_ratio(1.0, 0, 600), was);
    }
}
