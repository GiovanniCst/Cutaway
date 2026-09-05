// The marking-up tool: the panel, the live view, and putting a mark down.
//
// The geometry lives in annotate.rs and is read from both here and the
// rasteriser, so a shape cannot look one way while being drawn and another once
// applied - which is the failure the two-implementation arrangement kept
// producing.

use egui::{Color32, Painter, Pos2, Rect, Response, Stroke, Ui};

use crate::annotate::{self, Kind, Shape};

/// The colours offered, which are the ones that show up on a screenshot.
pub use crate::skin::PALETTE;

pub struct Markup {
    pub shapes: Vec<Shape>,
    /// Which mark is in hand. A new one is held as soon as it is put down, so
    /// the colour and width controls point at it without it having to be picked
    /// up again.
    pub held: Option<usize>,
    pub kind: Kind,
    pub colour: Color32,
    /// The highlighter's colour, kept apart from the pen's: they come from two
    /// different sets and swapping instrument should not lose either choice.
    /// Held opaque, with the transparency next to it rather than folded in:
    /// Color32 stores its channels premultiplied, so a colour that already
    /// carries an alpha and is asked for one again comes back darker every
    /// time.
    pub highlighter: Color32,
    /// How much of the highlighter goes down, 0 to 255. A highlighter with no
    /// transparency is a marker.
    pub highlight_alpha: u8,
    /// One width per instrument. Sharing one is what made the eraser useless.
    pub width: f32,
    pub highlight_width: f32,
    pub eraser_width: f32,
    pub size: f32,
    pub text: String,
    /// What the next badge will say. Advances on its own after each one, which
    /// is the entire reason the tool exists: documenting a procedure means
    /// putting down 1, 2, 3 without stopping to type them.
    pub next_number: u32,
    /// The mark being drawn right now, before the button comes up.
    drawing: Option<Shape>,
    /// Where a held mark was grabbed, to move it by the difference.
    grabbed_at: Option<Pos2>,
}

impl Default for Markup {
    fn default() -> Markup {
        Markup {
            shapes: Vec::new(),
            held: None,
            kind: Kind::Rect,
            colour: PALETTE[0],
            highlighter: crate::skin::HIGHLIGHTERS[0],
            highlight_alpha: crate::skin::HIGHLIGHT_ALPHA,
            width: annotate::STROKE_WIDTH,
            highlight_width: annotate::HIGHLIGHT_WIDTH,
            eraser_width: annotate::ERASER_WIDTH,
            size: 48.0,
            text: String::new(),
            next_number: 1,
            drawing: None,
            grabbed_at: None,
        }
    }
}

impl Markup {
    pub fn clear(&mut self) {
        self.shapes.clear();
        self.held = None;
        self.drawing = None;
        self.next_number = 1;
    }

    /// What a fresh mark of this kind is made of: its ink and its width.
    ///
    /// One place, because the panel and the stage both need the answer and they
    /// disagreed: the panel showed the pen's width while the stage drew the
    /// eraser with it. The alpha is folded into the colour here, so a mark is
    /// never handed an opaque highlighter.
    pub fn instrument(&self, kind: Kind) -> (Color32, f32) {
        match kind {
            Kind::Highlight => {
                let straight = self.highlighter.to_srgba_unmultiplied();
                (
                    Color32::from_rgba_unmultiplied(
                        straight[0],
                        straight[1],
                        straight[2],
                        self.highlight_alpha,
                    ),
                    self.highlight_width,
                )
            }
            Kind::Erase => (self.colour, self.eraser_width),
            _ => (self.colour, self.width),
        }
    }

    /// The mark in hand, if there is one.
    fn holding(&mut self) -> Option<&mut Shape> {
        self.held.and_then(move |at| self.shapes.get_mut(at))
    }

    /// The panel: what to draw with, and what the held mark looks like.
    ///
    /// The same controls serve both, which is why they read from the held mark
    /// when there is one: a colour picker that shows the tool's colour while a
    /// mark of another colour is selected is telling you about the wrong thing.
    /// Returns true when the panel asked to be closed. What changed inside it
    /// is the panel's own business: the marks are held here.
    pub fn panel(&mut self, ui: &mut Ui) -> bool {
        let shut = crate::widgets::panel_title(ui, crate::words::w().markup);
        let mut changed = false;

        ui.label(
            egui::RichText::new(crate::words::w().tool)
                .text_style(crate::skin::label_style())
                .color(crate::skin::tokens(ui.ctx()).ink_dim),
        );
        for row in [
            [Kind::Rect, Kind::Ellipse],
            [Kind::Arrow, Kind::Line],
            [Kind::Text, Kind::Number],
            [Kind::Highlight, Kind::Erase],
        ] {
            ui.horizontal(|ui| {
                for kind in row {
                    if crate::widgets::cell(ui, kind.label(), self.kind == kind)
                        .on_hover_text(crate::words::w().hint_kind)
                        .clicked()
                    {
                        self.kind = kind;
                        // Choosing a tool lets go of what was held: the controls
                        // now describe the next mark rather than the last one.
                        self.held = None;
                    }
                }
            });
        }

        let editing_kind = self
            .held
            .and_then(|at| self.shapes.get(at))
            .map(|s| s.kind)
            .unwrap_or(self.kind);

        // What the controls are pointed at: the held mark, or the next one -
        // and for the next one, the settings belonging to the instrument
        // chosen rather than one set shared by all of them.
        let mine = self.instrument(editing_kind);
        let (mut colour, mut width, mut size) = match self.held.and_then(|at| self.shapes.get(at)) {
            Some(held) => (held.colour, held.width, held.size),
            None => (mine.0, mine.1, self.size),
        };
        // The alpha travels in the colour, so the slider reads it back out.
        let mut alpha = if editing_kind == Kind::Highlight {
            if self.held.is_some() { colour.a() } else { self.highlight_alpha }
        } else {
            255
        };

        if editing_kind != Kind::Erase {
            crate::widgets::section(ui, crate::words::w().colour);
            if editing_kind == Kind::Highlight {
                let names = crate::words::w().highlighter_names;
                ui.horizontal(|ui| {
                    for (column, swatch) in crate::skin::HIGHLIGHTERS.iter().enumerate() {
                        let lit = colour.to_srgba_unmultiplied()[..3] == swatch.to_srgba_unmultiplied()[..3];
                        if crate::widgets::swatch(ui, names[column], *swatch, lit).clicked() {
                            colour = *swatch;
                            changed = true;
                        }
                    }
                });
            } else {
                let names = crate::words::w().colour_names;
                for (row, chunk) in PALETTE.chunks(4).enumerate() {
                    ui.horizontal(|ui| {
                        for (column, swatch) in chunk.iter().enumerate() {
                            let name = names[row * 4 + column];
                            if crate::widgets::swatch(ui, name, *swatch, colour == *swatch).clicked() {
                                colour = *swatch;
                                changed = true;
                            }
                        }
                    });
                }
            }
            ui.add_space(crate::skin::S2);
        }

        let stroke_label =
            if editing_kind == Kind::Erase { crate::words::w().eraser } else { crate::words::w().stroke };
        let shown = format!("{:.0}", width);
        if crate::widgets::slider_row(ui, stroke_label, &mut width, annotate::MIN_STROKE..=64.0, &shown)
            .changed()
        {
            changed = true;
        }

        if editing_kind == Kind::Highlight {
            let mut percent = alpha as f32 / 255.0 * 100.0;
            let shown = format!("{:.0}%", percent);
            if crate::widgets::slider_row(
                ui,
                crate::words::w().how_strong,
                &mut percent,
                10.0..=100.0,
                &shown,
            )
            .changed()
            {
                alpha = (percent / 100.0 * 255.0).round().clamp(1.0, 255.0) as u8;
                changed = true;
            }
        }

        if editing_kind == Kind::Number {
            // This row used to read "Prossimo [1] [Da 1]" and explain neither
            // half: what the number was for, nor that the button resets a
            // counter which, having never advanced, was already at 1 - so it
            // looked like a button that did nothing.
            crate::widgets::section(ui, crate::words::w().numbering);
            crate::widgets::caption(ui, crate::words::w().numbering_hint);
            ui.add_space(crate::skin::S2);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(crate::words::w().next_number)
                        .text_style(crate::skin::label_style())
                        .color(crate::skin::tokens(ui.ctx()).ink_dim),
                );
                ui.add(egui::DragValue::new(&mut self.next_number).range(1..=999));
            });
            ui.add_space(crate::skin::S1);
            // Only offered when it would change something.
            if crate::widgets::secondary(ui, crate::words::w().from_one, self.next_number != 1)
                .on_hover_text(crate::words::w().hint_from_one)
                .clicked()
            {
                self.next_number = 1;
            }
        }

        if editing_kind == Kind::Text {
            let shown = format!("{:.0}", size);
            if crate::widgets::slider_row(ui, crate::words::w().body_size, &mut size, 8.0..=200.0, &shown).changed() {
                changed = true;
            }
            crate::widgets::section(ui, crate::words::w().text);
            let mut text = self
                .held
                .and_then(|at| self.shapes.get(at))
                .map(|s| s.text.clone())
                .unwrap_or_else(|| self.text.clone());
            if ui.text_edit_singleline(&mut text).changed() {
                match self.holding() {
                    Some(held) => held.text = text,
                    None => self.text = text,
                }
                changed = true;
            }
        }

        // Written back to whichever subject the controls were describing.
        //
        // The highlighter's transparency is carried in its colour, so it has to
        // be put back on every write and not only when the mark is first drawn:
        // picking a swatch used to hand the shape the opaque colour straight
        // from the palette, and the mark stopped being a highlighter.
        let painted = if editing_kind == Kind::Highlight {
            Color32::from_rgba_unmultiplied(colour.r(), colour.g(), colour.b(), alpha)
        } else {
            colour
        };
        match self.holding() {
            Some(held) => {
                held.colour = painted;
                held.width = width;
                held.size = size;
            }
            None => {
                match editing_kind {
                    Kind::Highlight => {
                        // Opaque, with the alpha beside it: see the field.
                        let straight = colour.to_srgba_unmultiplied();
                        self.highlighter =
                            Color32::from_rgb(straight[0], straight[1], straight[2]);
                        self.highlight_alpha = alpha;
                        self.highlight_width = width;
                    }
                    Kind::Erase => self.eraser_width = width,
                    _ => {
                        self.colour = colour;
                        self.width = width;
                    }
                }
                self.size = size;
            }
        }

        ui.add_space(crate::skin::S4);
        ui.horizontal(|ui| {
            let holding = self.held.is_some();
            if crate::widgets::destructive(ui, crate::words::w().delete, holding)
                .on_hover_text(crate::words::w().hint_delete)
                .clicked()
            {
                if let Some(at) = self.held.take() {
                    self.shapes.remove(at);
                    changed = true;
                }
            }
            if crate::widgets::destructive(ui, crate::words::w().clear_all, !self.shapes.is_empty())
                .on_hover_text(crate::words::w().hint_clear_all)
                .clicked()
            {
                self.clear();
                changed = true;
            }
        });
        ui.add_space(crate::skin::S1);
        crate::widgets::caption(ui, &crate::words::count(
            self.shapes.len(),
            crate::words::w().one_mark,
            crate::words::w().marks_count,
        ));
        let _ = changed;
        shut
    }

    /// Which mark a click at this point would take hold of.
    ///
    /// Its own function so the rule can be checked without a window: the rule
    /// was wrong for as long as it lived inside the event handling, and nothing
    /// could see it.
    pub fn under(&self, x: f32, y: f32, reach: f32) -> Option<usize> {
        // From the top down: the last mark drawn is the one on top, and the one
        // on top is the one a person means.
        self.shapes.iter().rposition(|shape| shape.contains(x, y, reach))
    }

    /// The live view, and the drawing of new marks.
    ///
    /// `shown` is where the picture sits on screen; marks are kept in picture
    /// pixels, so everything converts through it and nothing is stored in
    /// screen coordinates - a window resize would otherwise move the marks.
    pub fn on_stage(
        &mut self,
        painter: &Painter,
        response: &Response,
        shown: Rect,
        picture: (u32, u32),
    ) {
        let to_picture = |at: Pos2| -> (f32, f32) {
            (
                (at.x - shown.left()) / shown.width().max(1.0) * picture.0 as f32,
                (at.y - shown.top()) / shown.height().max(1.0) * picture.1 as f32,
            )
        };
        let scale = shown.width().max(1.0) / picture.0 as f32;

        if response.drag_started() {
            if let Some(at) = response.interact_pointer_pos() {
                let (x, y) = to_picture(at);
                // A mark already down and taken hold of is picked up and moved
                // rather than drawn over. *Any* mark, not only the one already
                // held: requiring it to be held first meant that the moment a
                // second mark went down the first became unreachable, because
                // clicking it drew a third one on top of it.
                //
                // The freehand tools are the exception. A highlighter or an
                // eraser is swept across the picture and across whatever is on
                // it, so there a hit means draw.
                let hit =
                    if self.kind.freehand() { None } else { self.under(x, y, 6.0 / scale) };
                match hit {
                    Some(at_index) => {
                        self.held = Some(at_index);
                        self.grabbed_at = Some(Pos2::new(x, y));
                    }
                    None => {
                        let (ink, wide) = self.instrument(self.kind);
                        let mut fresh = Shape::new(self.kind, (x, y), ink, wide);
                        fresh.text = self.text.clone();
                        fresh.size = self.size;
                        self.drawing = Some(fresh);
                    }
                }
            }
        }

        if response.dragged() {
            if let Some(at) = response.interact_pointer_pos() {
                let (x, y) = to_picture(at);
                if let Some(from) = self.grabbed_at {
                    let (dx, dy) = (x - from.x, y - from.y);
                    if let Some(at_index) = self.held {
                        if let Some(shape) = self.shapes.get(at_index) {
                            self.shapes[at_index] = shape.moved_by(dx, dy);
                        }
                    }
                    self.grabbed_at = Some(Pos2::new(x, y));
                } else if let Some(drawing) = &mut self.drawing {
                    drawing.to = (x, y);
                    if drawing.kind.freehand() {
                        drawing.trail.push((x, y));
                    }
                }
            }
        }

        if response.drag_stopped() {
            self.grabbed_at = None;
            if let Some(finished) = self.drawing.take() {
                // Text lands where it is clicked and needs no drag, so it counts
                // as worth keeping on its own terms.
                if finished.worth_keeping() {
                    self.shapes.push(finished);
                    // Held as soon as it is put down: everything needed to adjust
                    // a mark already works on the held one, and sending the person
                    // back to pick up what they have just drawn buys nothing.
                    self.held = Some(self.shapes.len() - 1);
                }
            }
        }

        // A click without a drag: it takes hold of a mark, and where there is
        // none it is how text and badges are placed. Selecting comes first, or
        // clicking a badge to adjust its colour would stack a second badge on
        // top of it.
        if response.clicked() {
            if let Some(at) = response.interact_pointer_pos() {
                let (x, y) = to_picture(at);
                match self.under(x, y, 6.0 / scale) {
                    Some(at_index) => self.held = Some(at_index),
                    None => match self.kind {
                        Kind::Text if !self.text.trim().is_empty() => {
                            let mut fresh =
                                Shape::new(Kind::Text, (x, y), self.colour, self.width);
                            fresh.text = self.text.clone();
                            fresh.size = self.size;
                            self.shapes.push(fresh);
                            self.held = Some(self.shapes.len() - 1);
                        }
                        Kind::Number => {
                            let mut fresh =
                                Shape::new(Kind::Number, (x, y), self.colour, self.width);
                            fresh.number = self.next_number;
                            self.next_number += 1;
                            self.shapes.push(fresh);
                            self.held = Some(self.shapes.len() - 1);
                        }
                        // Clicking empty ground with a shape tool lets go of
                        // whatever was held: the panel then describes the next
                        // mark rather than the last one.
                        _ => self.held = None,
                    },
                }
            }
        }

        for (at, shape) in self.shapes.iter().enumerate() {
            paint(painter, shape, shown, picture, self.held == Some(at));
        }
        if let Some(drawing) = &self.drawing {
            paint(painter, drawing, shown, picture, false);
        }
    }
}

/// Draws one mark on screen, in the picture's own coordinates scaled to where it
/// is shown. The geometry comes from annotate.rs, so this and the rasteriser
/// cannot disagree about where an arrow's head goes.
fn paint(painter: &Painter, shape: &Shape, shown: Rect, picture: (u32, u32), held: bool) {
    let scale = shown.width().max(1.0) / picture.0 as f32;
    let at = |(x, y): (f32, f32)| -> Pos2 {
        Pos2::new(shown.left() + x * scale, shown.top() + y * scale)
    };
    let width = (shape.width * scale).max(1.0);
    let stroke = Stroke::new(width, shape.colour);

    match shape.kind {
        Kind::Rect => {
            let (left, top, right, bottom) = shape.ordered();
            painter.rect_stroke(Rect::from_min_max(at((left, top)), at((right, bottom))), 0.0, stroke);
        }
        Kind::Ellipse => {
            let (left, top, right, bottom) = shape.ordered();
            let box_on_screen = Rect::from_min_max(at((left, top)), at((right, bottom)));
            let steps = 64;
            let mut points = Vec::with_capacity(steps + 1);
            for i in 0..=steps {
                let angle = i as f32 / steps as f32 * std::f32::consts::TAU;
                points.push(Pos2::new(
                    box_on_screen.center().x + box_on_screen.width() / 2.0 * angle.cos(),
                    box_on_screen.center().y + box_on_screen.height() / 2.0 * angle.sin(),
                ));
            }
            let _ = painter.add(egui::Shape::line(points, stroke));
        }
        Kind::Line => {
            painter.line_segment([at(shape.from), at(shape.to)], stroke);
        }
        Kind::Arrow => {
            painter.line_segment([at(shape.from), at(shape.to)], stroke);
            for barb in annotate::arrow_head(shape.from, shape.to, shape.width) {
                painter.line_segment([at(shape.to), at(barb)], stroke);
            }
        }
        Kind::Highlight | Kind::Erase => {
            let colour = if shape.kind == Kind::Erase {
                // The eraser cannot be shown as what it does - it takes pixels
                // away - so it is shown as where it is going.
                crate::skin::ERASER
            } else {
                shape.colour
            };
            let trail: Vec<Pos2> = shape.trail.iter().map(|p| at(*p)).collect();
            if trail.len() > 1 {
                let _ = painter.add(egui::Shape::line(trail, Stroke::new(width, colour)));
            }
        }
        Kind::Number => {
            let radius = shape.badge_radius() * scale;
            let middle = at(shape.from);
            painter.circle_filled(middle, radius, shape.colour);
            // White or black on top, whichever the badge's own colour leaves
            // readable: a yellow badge with white digits says nothing.
            let ink = crate::skin::over(shape.colour);
            painter.text(
                middle,
                egui::Align2::CENTER_CENTER,
                shape.number.to_string(),
                egui::FontId::proportional((radius * 1.2).max(8.0)),
                ink,
            );
        }
        Kind::Text => {
            painter.text(
                at(shape.from),
                egui::Align2::LEFT_TOP,
                &shape.text,
                egui::FontId::proportional((shape.size * scale).max(6.0)),
                shape.colour,
            );
        }
    }

    if held {
        let (left, top, right, bottom) = shape.bounds();
        painter.rect_stroke(
            Rect::from_min_max(at((left, top)), at((right, bottom))).expand(2.0),
            2.0,
            Stroke::new(1.0_f32, crate::skin::MARK),
        );
    }
}

#[cfg(test)]
mod picking {
    use super::*;
    use crate::annotate::Kind;

    fn with_two() -> Markup {
        let mut markup = Markup::default();
        let mut first = Shape::new(Kind::Rect, (10.0, 10.0), Color32::RED, 4.0);
        first.to = (60.0, 60.0);
        let mut second = Shape::new(Kind::Rect, (200.0, 200.0), Color32::BLUE, 4.0);
        second.to = (260.0, 260.0);
        markup.shapes.push(first);
        markup.shapes.push(second);
        markup
    }

    #[test]
    fn a_second_mark_does_not_bury_the_first() {
        // What the program did before: only the mark already held could be
        // picked up, so the moment a second one went down the first became
        // unreachable - clicking it drew a third one on top of it.
        let mut markup = with_two();
        markup.held = Some(1);
        assert_eq!(markup.under(30.0, 30.0, 6.0), Some(0));
        assert_eq!(markup.under(230.0, 230.0, 6.0), Some(1));
    }

    #[test]
    fn empty_ground_holds_nothing() {
        let markup = with_two();
        assert_eq!(markup.under(500.0, 500.0, 6.0), None);
    }

    #[test]
    fn the_one_on_top_is_the_one_meant() {
        // Two marks over each other: the last drawn is the one on top, and the
        // one on top is what a person is pointing at.
        let mut markup = with_two();
        let mut third = Shape::new(Kind::Rect, (20.0, 20.0), Color32::GREEN, 4.0);
        third.to = (40.0, 40.0);
        markup.shapes.push(third);
        assert_eq!(markup.under(30.0, 30.0, 6.0), Some(2));
    }

    #[test]
    fn the_reach_is_generous_enough_to_click_a_thin_line() {
        let mut markup = Markup::default();
        let mut line = Shape::new(Kind::Line, (100.0, 100.0), Color32::RED, 2.0);
        line.to = (300.0, 100.0);
        markup.shapes.push(line);
        // A pixel or two off a one-pixel line still means that line.
        assert_eq!(markup.under(200.0, 104.0, 6.0), Some(0));
        assert_eq!(markup.under(200.0, 140.0, 6.0), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_instrument_has_its_own_width() {
        // They shared one. Choosing the eraser kept whatever the pen was set to,
        // so an eraser dragged across a photograph at six pixels took away a
        // hair, which is indistinguishable from an eraser that does not work.
        let kit = Markup::default();
        let pen = kit.instrument(Kind::Rect).1;
        let highlighter = kit.instrument(Kind::Highlight).1;
        let eraser = kit.instrument(Kind::Erase).1;
        assert!(pen < highlighter, "penna {} contro evidenziatore {}", pen, highlighter);
        assert!(highlighter < eraser, "evidenziatore {} contro gomma {}", highlighter, eraser);
        assert_eq!(eraser, annotate::ERASER_WIDTH);
    }

    #[test]
    fn the_highlighter_is_never_opaque() {
        let mut kit = Markup::default();
        assert!(kit.instrument(Kind::Highlight).0.a() < 255);
        // And it stays transparent whichever colour is chosen.
        for swatch in crate::skin::HIGHLIGHTERS {
            kit.highlighter = swatch;
            let ink = kit.instrument(Kind::Highlight).0;
            assert!(ink.a() < 255, "{:?} opaco", swatch);
            // Compared unmultiplied, because Color32 keeps its channels
            // premultiplied and the straight colour is the one chosen. Within
            // one step, not exactly: eight bits of premultiplied storage cannot
            // give a colour back unchanged, and the neighbouring test shows the
            // error does not accumulate.
            let (got, want) = (ink.to_srgba_unmultiplied(), swatch.to_srgba_unmultiplied());
            for channel in 0..3 {
                assert!(
                    (got[channel] as i16 - want[channel] as i16).abs() <= 2,
                    "{:?}: {:?} contro {:?}",
                    swatch,
                    got,
                    want
                );
            }
        }
        // The pen, on the other hand, must be solid.
        assert_eq!(kit.instrument(Kind::Rect).0.a(), 255);
    }

    #[test]
    fn choosing_a_colour_twice_does_not_darken_it() {
        // Color32 is premultiplied. Storing the highlighter with its alpha
        // already in it and then asking for the alpha again multiplied it a
        // second time, so the same swatch came out darker on every pass.
        let mut kit = Markup::default();
        let first = kit.instrument(Kind::Highlight).0.to_srgba_unmultiplied();
        for _ in 0..4 {
            let ink = kit.instrument(Kind::Highlight).0;
            let straight = ink.to_srgba_unmultiplied();
            kit.highlighter = Color32::from_rgb(straight[0], straight[1], straight[2]);
        }
        assert_eq!(kit.instrument(Kind::Highlight).0.to_srgba_unmultiplied(), first);
    }

    #[test]
    fn the_highlighter_uses_its_own_colours() {
        // A pen writes to be read; a highlighter tints what is under it and has
        // to stay lighter than it. The two sets do not overlap.
        for swatch in crate::skin::HIGHLIGHTERS {
            assert!(
                !PALETTE.contains(&swatch),
                "{:?} sta in tutti e due i set",
                swatch
            );
        }
        assert_eq!(
            crate::skin::HIGHLIGHTERS.len(),
            crate::words::w().highlighter_names.len()
        );
    }
}
