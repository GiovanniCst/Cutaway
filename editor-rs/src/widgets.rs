// The pieces the standard ones do not cover.
//
// A rail entry is not a button with a picture on it: it is the visible form of
// a mode the program is in, and the difference shows in how it says so - an
// accent bar flush with the edge rather than a filled lozenge, because a mode
// is a place you are, not a thing you pressed.

use eframe::egui::{self, Response, Sense, Ui, Vec2};

use crate::skin::{self, Icon};

/// How tall one rail entry is, and how wide the rail.
///
/// 72 rather than the 64 the layout was drawn at: measured on the rendered
/// window, "Ridimensiona" at 10 px comes to about 60 logical pixels and was
/// being clipped by the rail's own edge. A label that does not fit is a label
/// that has to be abbreviated, and an abbreviation in a rail is a word nobody
/// reads - eight pixels of chrome is the cheaper of the two.
pub const RAIL_WIDTH: f32 = 72.0;
const ENTRY_HEIGHT: f32 = 56.0;

/// One mode in the left rail: icon over label, and an accent bar when it is the
/// one the program is in.
pub fn rail_entry(
    ui: &mut Ui,
    which: Icon,
    label: &str,
    active: bool,
    enabled: bool,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(RAIL_WIDTH - skin::S2, ENTRY_HEIGHT),
        if enabled { Sense::click() } else { Sense::hover() },
    );
    let tokens = skin::tokens(ui.ctx());

    // egui runs the interpolation and asks for a repaint only while it is in
    // flight, so a still window still draws nothing. Colour and opacity only:
    // animating anything that decides layout would relayout the stage on every
    // intermediate frame, and the photograph would shiver.
    let lit = ui.ctx().animate_bool_with_time_and_easing(
        response.id.with("lit"),
        enabled && (active || response.hovered()),
        0.12,
        egui::emath::easing::cubic_out,
    );
    let on = ui.ctx().animate_bool_with_time_and_easing(
        response.id.with("on"),
        active,
        0.16,
        egui::emath::easing::cubic_out,
    );
    let painter = ui.painter();

    if lit > 0.0 {
        painter.rect_filled(rect, 6.0, tokens.chrome.lerp_to_gamma(tokens.raised, lit));
    }
    if on > 0.0 {
        // Flush with the left edge, and short of the entry's own height top and
        // bottom, so a run of them reads as a column of separate places. It
        // grows from the middle outwards, which is the one size animated here -
        // it floats inside a rectangle that was allocated whole.
        let half = (rect.height() / 2.0 - skin::S1) * on;
        let bar = egui::Rect::from_min_max(
            egui::Pos2::new(rect.left(), rect.center().y - half),
            egui::Pos2::new(rect.left() + 3.0, rect.center().y + half),
        );
        painter.rect_filled(bar, 1.5, tokens.accent);
    }

    let colour = if !enabled {
        tokens.ink_off
    } else {
        tokens.ink_dim.lerp_to_gamma(tokens.accent, on)
    };

    let icon_at = egui::Rect::from_center_size(
        egui::Pos2::new(rect.center().x, rect.top() + 18.0),
        Vec2::splat(20.0),
    );
    skin::icon(painter, icon_at, which, colour);
    painter.text(
        egui::Pos2::new(rect.center().x, rect.bottom() - 12.0),
        egui::Align2::CENTER_CENTER,
        label,
        egui::TextStyle::resolve(&skin::rail_style(), ui.style()),
        colour,
    );

    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

/// How wide a tool panel is: fixed, never resized.
///
/// 232 px of content inside the S4 margins, which is exactly two columns of 112
/// with an S2 gutter - so the tool grid stops being crooked by construction
/// rather than by attention. It never shrinks: below 264 that grid goes ragged
/// again, which is the thing being corrected.
pub const PANEL_WIDTH: f32 = 264.0;

/// The AI panel is the one exception, and only because of what it holds: a list
/// of models, each carrying a name, a vendor, a price and a measured time on
/// one line. At 264 every row wraps, and a wrapped row in a list of rows is
/// unreadable.
pub const WIDE_PANEL_WIDTH: f32 = 340.0;

/// The air above a panel's footer, which is what separates the way out from
/// the settings above it.
pub const FOOT: f32 = skin::S6;

/// The action a panel exists for: filled, full width, unmistakable.
///
/// Full width rather than side by side with its Cancel. Two buttons of 112 px
/// in a row are two buttons that look the same, and looking the same is exactly
/// what an Apply and a Cancel must not do.
pub fn primary(ui: &mut Ui, text: &str, enabled: bool) -> Response {
    let tokens = skin::tokens(ui.ctx());
    let (fill, ink) = if enabled {
        (tokens.accent, tokens.on_accent)
    } else {
        // Still a block when it is disabled, so its position does not move when
        // it becomes available.
        (tokens.raised, tokens.ink_off)
    };
    ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(text).color(ink))
            .fill(fill)
            .min_size(Vec2::new(ui.available_width(), 30.0)),
    )
}

/// The way back out: outlined, full width, under the primary.
pub fn secondary(ui: &mut Ui, text: &str, enabled: bool) -> Response {
    let tokens = skin::tokens(ui.ctx());
    ui.add_enabled(
        enabled,
        egui::Button::new(text)
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::new(1.0, tokens.line))
            .min_size(Vec2::new(ui.available_width(), 30.0)),
    )
}

/// Something that throws work away.
///
/// No outline, ever: a destructive button should not *invite*, it should be
/// recognisable when somebody goes looking for it.
pub fn destructive(ui: &mut Ui, text: &str, enabled: bool) -> Response {
    let tokens = skin::tokens(ui.ctx());
    let ink = if enabled { tokens.danger } else { tokens.ink_off };
    ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(text).color(ink))
            .fill(egui::Color32::TRANSPARENT),
    )
}

/// A panel's title row: the name, and the way to close it.
///
/// Every panel had a heading and none had a close, so the only ways out were
/// Escape and clicking the entry that opened it again.
pub fn panel_title(ui: &mut Ui, title: &str) -> bool {
    let mut close = false;
    ui.horizontal(|ui| {
        ui.heading(title);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            close = ui
                .add(egui::Button::new("\u{00D7}").fill(egui::Color32::TRANSPARENT))
                .on_hover_text("Chiudi")
                .clicked();
        });
    });
    ui.add_space(skin::S4);
    close
}

/// A section label inside a panel: what the controls under it are for.
///
/// Between two sections there is air and a word, not a rule across the panel.
pub fn section(ui: &mut Ui, label: &str) {
    ui.add_space(skin::S4);
    ui.label(
        egui::RichText::new(label)
            .text_style(skin::label_style())
            .color(skin::tokens(ui.ctx()).ink_dim),
    );
    ui.add_space(skin::S1);
}

/// A caption: an explanation, a count, a measurement.
pub fn caption(ui: &mut Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .text_style(skin::caption_style())
            .color(skin::tokens(ui.ctx()).ink_faint),
    );
}

/// A caption on its way out.
pub fn fading_caption(ui: &mut Ui, text: &str, alpha: f32) {
    ui.label(
        egui::RichText::new(text)
            .text_style(skin::caption_style())
            .color(skin::tokens(ui.ctx()).ink_faint.gamma_multiply(alpha)),
    );
}

/// A number the window shows. Fixed width digits, so it does not dance.
pub fn number(ui: &mut Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .text_style(skin::numeric_style())
            .color(skin::tokens(ui.ctx()).ink),
    );
}

/// A slider with its name above it and its value on the right.
///
/// egui's own slider puts the label *after* the control and the number in a
/// drag field that looks like a different widget again. Worse here, it sizes
/// itself to `slider_width` plus that trailing label, which is how the panels
/// came to be wider than the panel: measured on the rendered window, a panel
/// with a slider laid out 134 px past its own frame and clipped everything in
/// it, while the panels with no slider were exact.
///
/// The number is set in Numeric, so it does not shift the label as it counts.
pub fn slider_row<Num: egui::emath::Numeric>(
    ui: &mut Ui,
    label: &str,
    value: &mut Num,
    range: std::ops::RangeInclusive<Num>,
    shown: &str,
) -> Response {
    let tokens = skin::tokens(ui.ctx());
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(label).text_style(skin::label_style()).color(tokens.ink_dim),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(shown)
                    .text_style(skin::numeric_style())
                    .color(tokens.ink),
            );
        });
    });
    ui.spacing_mut().slider_width = ui.available_width();
    let response = ui.add(egui::Slider::new(value, range).show_value(false));
    ui.add_space(skin::S2);
    response
}

/// One cell of a two-column grid inside a panel.
///
/// 112, which is what half of a panel's 232 of content comes to once the S2
/// gutter is taken out. Fixed, because a cell that sizes itself to its own word
/// puts the second column wherever the first word happens to end: measured on
/// the old window, the second column of the tool grid started at 92, 68, 60 and
/// 105 on its four rows - a 45 px excursion on a panel 221 wide.
pub const CELL: f32 = 112.0;

/// One entry in that grid: a mode among several, exactly one of them chosen.
pub fn cell(ui: &mut Ui, label: &str, chosen: bool) -> Response {
    ui.add_sized(
        Vec2::new(CELL, 28.0),
        egui::SelectableLabel::new(chosen, label),
    )
}

/// A colour to choose, and a ring saying whether it is the chosen one.
///
/// The ring is double, and each half has one job: the inner one has to
/// separate from the swatch, so it is black or white depending on the swatch;
/// the outer one has to separate from the panel, so it is the text colour. A
/// single white ring - which is what this was - gives 1.09 against the white
/// swatch, so choosing white produced no visible answer at all.
pub fn swatch(ui: &mut Ui, name: &str, colour: egui::Color32, chosen: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(26.0), egui::Sense::click());
    let tokens = skin::tokens(ui.ctx());
    let painter = ui.painter();
    painter.rect_filled(rect, 4.0, colour);
    // A hairline on every swatch, chosen or not. Without it the white swatch
    // is invisible on the light theme panel, which is the same defect the
    // chosen-ring already had at the other end of the palette - only this half
    // showed up when the light theme finally started.
    painter.rect_stroke(rect, 4.0, egui::Stroke::new(1.0, tokens.line));
    if chosen {
        painter.rect_stroke(rect.shrink(1.0), 3.0, egui::Stroke::new(2.0, skin::over(colour)));
        painter.rect_stroke(rect.expand(2.0), 6.0, egui::Stroke::new(2.0, tokens.ink));
    } else if response.hovered() {
        painter.rect_stroke(rect.expand(2.0), 6.0, egui::Stroke::new(1.0, tokens.ink_dim));
    }
    response.on_hover_text(name)
}

/// The chequer that says "there is nothing here", under a see-through picture.
///
/// Without it, a picture whose background has just been removed shows the
/// stage - which is a grey very like the background that was removed, so the
/// tool appears to have done nothing.
pub fn chequer(painter: &egui::Painter, at: egui::Rect) {
    const SQUARE: f32 = 8.0;
    painter.rect_filled(at, 0.0, skin::CHECKER_A);
    let mut y = at.top();
    let mut row = 0;
    while y < at.bottom() {
        let mut x = at.left() + if row % 2 == 0 { 0.0 } else { SQUARE };
        while x < at.right() {
            let square = egui::Rect::from_min_size(
                egui::Pos2::new(x, y),
                Vec2::new(SQUARE.min(at.right() - x), SQUARE.min(at.bottom() - y)),
            );
            painter.rect_filled(square, 0.0, skin::CHECKER_B);
            x += SQUARE * 2.0;
        }
        y += SQUARE;
        row += 1;
    }
}

/// The shadow and the hairline that make the picture an object resting on the
/// ground rather than a stain in it.
///
/// A photograph with pale edges used to dissolve into the grey; one with dark
/// edges did the same at the other end.
pub fn picture_shadow(painter: &egui::Painter, at: egui::Rect) {
    // Four rings of decreasing alpha rather than a real blur: the painter has
    // no blur, and at this size the difference is not visible while the cost is
    // four rectangles.
    for step in (1..=4).rev() {
        let spread = step as f32 * 3.0;
        let alpha = (18 / step) as u8;
        painter.rect_filled(
            at.translate(Vec2::new(0.0, skin::S0)).expand(spread),
            spread,
            egui::Color32::from_black_alpha(alpha),
        );
    }
}

pub fn picture_edge(painter: &egui::Painter, at: egui::Rect) {
    painter.rect_stroke(at, 0.0, egui::Stroke::new(1.0, skin::PIC_EDGE));
}

/// The program saying its own name, at the left end of the toolbar.
///
/// A trim mark and the word, which is the same pairing at a third scale: the
/// icon in the rail, the marks round the photograph, and this. Without it the
/// window never writes the name of the program anywhere inside itself.
///
/// Clicking it opens the credits, as it did in the 1.6 build.
pub fn wordmark(ui: &mut Ui) -> bool {
    let tokens = skin::tokens(ui.ctx());
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(96.0, 20.0), egui::Sense::click());
    let painter = ui.painter();
    // Inset by the arm that reaches outside the corner, or the mark is drawn
    // past the left edge of what was allocated and the panel clips it off.
    let outside = 12.0 * 8.0 / 18.0;
    let mark = egui::Rect::from_min_size(
        egui::Pos2::new(rect.left() + outside, rect.center().y - 6.0),
        Vec2::splat(12.0),
    );
    skin::trim_mark(painter, mark.left_top(), (-1.0, -1.0), 12.0, tokens.accent, false);
    painter.text(
        egui::Pos2::new(mark.right() + skin::S2, rect.center().y),
        egui::Align2::LEFT_CENTER,
        "CUTAWAY",
        egui::TextStyle::resolve(&skin::wordmark_style(), ui.style()),
        tokens.ink,
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.on_hover_text(crate::words::w().hint_wordmark).clicked()
}

/// The four trim marks round the photograph.
///
/// Not decoration: the mark *indicates*. It says where the edges of the picture
/// are, which is real information when a photograph has pale or dark borders
/// that dissolve into the ground - the defect the first plan listed and a
/// one-pixel hairline only half solved. With the crop tool in hand they light
/// up, and then they are also the hint.
pub fn picture_marks(painter: &egui::Painter, at: egui::Rect, cropping: bool) {
    let colour =
        if cropping { skin::MARK } else { skin::STAGE_INK.gamma_multiply(148.0 / 255.0) };
    for (corner, which) in skin::corners(at) {
        skin::trim_mark(painter, corner, which, 18.0, colour, true);
    }
}

/// A vertical hairline, for separating groups in the toolbar.
///
/// egui's own separator, laid horizontally, is a line the width of the
/// remaining space in a colour that does not read against the chrome: measured,
/// the groups in the old toolbar were already spaced correctly and nobody could
/// see it.
pub fn divider(ui: &mut Ui) {
    ui.add_space(skin::S6 / 2.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 16.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, skin::tokens(ui.ctx()).line);
    ui.add_space(skin::S6 / 2.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panel_holds_two_even_columns() {
        // The tool grid is two columns of 112 with an 8 gutter. If this stops
        // being exact the grid goes ragged, which is what it was.
        let content = PANEL_WIDTH - 2.0 * skin::S4;
        assert_eq!(content, 232.0);
        assert_eq!((content - skin::S2) / 2.0, 112.0);
    }

    #[test]
    fn two_cells_and_a_gutter_are_exactly_the_content() {
        assert_eq!(2.0 * CELL + skin::S2, PANEL_WIDTH - 2.0 * skin::S4);
    }

    #[test]
    fn four_swatches_and_their_gutters_fit_the_content() {
        let content = PANEL_WIDTH - 2.0 * skin::S4;
        // 26 wide, four across, three gutters of S2.
        assert!(4.0 * 26.0 + 3.0 * skin::S2 <= content);
    }

    #[test]
    fn the_chrome_leaves_room_for_the_picture() {
        // Rail plus the widest panel, on the narrowest window allowed.
        let chrome = RAIL_WIDTH + WIDE_PANEL_WIDTH;
        assert!(960.0 - chrome > 500.0, "allo stage restano solo {} px", 960.0 - chrome);
    }

    #[test]
    fn the_rail_fits_the_smallest_window_it_allows() {
        // main.rs sets a minimum inner height of 640. The toolbar and the status
        // bar take their share, and what is left has to hold every mode - a rail
        // that scrolls is a rail that hides one.
        let toolbar = 40.0;
        let status = 26.0;
        let entries = 9.0;
        let needed = skin::S3 + entries * (ENTRY_HEIGHT + skin::S1);
        let room = 640.0 - toolbar - status;
        assert!(needed <= room, "il rail chiede {} px e ne ha {}", needed, room);
    }
}
