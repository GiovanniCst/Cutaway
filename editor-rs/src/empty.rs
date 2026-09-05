// What the window shows before there is anything in it.
//
// It used to be one grey sentence in the middle of a grey field. Measured, the
// stage was inked at 0.04 % against the 1.6 build's 0.98 %, and that one
// sentence sat at a contrast of 2.76 in the dark theme and 1.76 in the light
// one - because it was themed text written on the unthemed photometric ground.
// A first screen that says nothing and can barely be read is a program with
// nothing to say for itself.
//
// So it is an empty crop: the very thing this program makes, shown before there
// is anything inside it, drawn with the same marks a live selection is drawn
// with. The name of the program is the name of the operation - a cutaway is the
// cut, the section, what is left when a piece is taken away - and the trim mark
// is that operation's sign. It was already drawn twice in this codebase and
// never anywhere a person looks first.

use eframe::egui::{self, Pos2, Rect, Sense, Ui, Vec2};

use crate::skin;
use crate::words::w;

/// What the box is, before it grows to fit its longest line.
const WIDE: f32 = 440.0;
const TALL: f32 = 300.0;
/// The arm of the corner marks.
const ARM: f32 = 18.0;

/// What a click on the empty stage asked for.
pub enum Wanted {
    Nothing,
    Open,
}

/// Draws the empty crop and says whether it was clicked.
///
/// `dragging` is true while a file is being held over the window: the box is
/// the target, so it is the thing that should react.
pub fn stage(ui: &mut Ui, dragging: bool) -> Wanted {
    let room = ui.available_rect_before_wrap();
    // Painted, not laid out: nothing here may take part in deciding the size of
    // anything, or opening a panel would move it and the stage would shiver.
    let wide = WIDE.min(room.width() - 2.0 * skin::S8);
    let tall = TALL.min(room.height() - 2.0 * skin::S8);
    let box_ = Rect::from_center_size(room.center(), Vec2::new(wide, tall));

    let response = ui.allocate_rect(box_, Sense::click());
    let lit = ui.ctx().animate_bool_with_time_and_easing(
        egui::Id::new("vuoto"),
        response.hovered() || dragging,
        0.12,
        egui::emath::easing::cubic_out,
    );
    let painter = ui.painter();

    // Three states, and only colour changes between them. The rectangle never
    // moves, so the stage never relayouts.
    let veil = 20.0 + lit * (if dragging { 26.0 } else { 14.0 });
    let dash_alpha = 128.0 + lit * (if dragging { 127.0 } else { 50.0 });
    painter.rect_filled(box_, 0.0, skin::MARK.gamma_multiply(veil / 255.0));

    // The sides whisper and the corners hold the rectangle: the dashes alone
    // would be illegible, which is exactly the balance the 1.6 build struck.
    let dashed = skin::MARK.gamma_multiply(dash_alpha / 255.0);
    skin::dashes(painter, box_.left_top(), box_.right_top(), dashed);
    skin::dashes(painter, box_.left_bottom(), box_.right_bottom(), dashed);
    skin::dashes(painter, box_.left_top(), box_.left_bottom(), dashed);
    skin::dashes(painter, box_.right_top(), box_.right_bottom(), dashed);
    for (corner, which) in skin::corners(box_) {
        skin::trim_mark(painter, corner, which, ARM, skin::MARK, true);
    }

    // --- what it says ---------------------------------------------------------

    let style = ui.style();
    let title_font = egui::TextStyle::resolve(&skin::stage_title_style(), style);
    let caption_font = egui::TextStyle::resolve(&skin::caption_style(), style);
    let key_font = egui::TextStyle::resolve(&skin::numeric_style(), style);

    let headline = if dragging { w().release_to_open } else { w().drop_here };
    let title_at = Pos2::new(box_.center().x, box_.top() + tall * 0.32);
    painter.text(
        title_at,
        egui::Align2::CENTER_CENTER,
        headline,
        title_font,
        skin::STAGE_INK,
    );

    // Three ways in, each with the keys that take it. The screenshot key is
    // called what it is called on this keyboard: an Italian one says Stamp and
    // has no key named PrtSc.
    let ctrl = "Ctrl";
    let ways: [(&[&str], &str); 3] = [
        (&[ctrl, "O"], w().to_pick_one),
        (&[ctrl, "V"], w().to_paste_one),
        (&[ctrl, w().print_screen_key], w().to_cut_one),
    ];
    let line_height = 26.0;
    let first = box_.center().y + skin::S4;
    for (row, (keys, what)) in ways.iter().enumerate() {
        let y = first + row as f32 * line_height;
        // Measured and then placed, so a longer translation cannot be clipped
        // by a guess about how wide its glyphs are.
        let or_wide = painter
            .layout_no_wrap(w().or_else.to_string(), caption_font.clone(), skin::STAGE_INK_DIM)
            .size()
            .x;
        let key_widths: Vec<f32> = keys
            .iter()
            .map(|key| {
                painter
                    .layout_no_wrap(key.to_string(), key_font.clone(), skin::STAGE_INK)
                    .size()
                    .x
                    + 2.0 * skin::S2
            })
            .collect();
        let what_wide = painter
            .layout_no_wrap(what.to_string(), caption_font.clone(), skin::STAGE_INK_DIM)
            .size()
            .x;
        let total = or_wide
            + skin::S2
            + key_widths.iter().sum::<f32>()
            + skin::S1 * (keys.len() as f32 - 1.0)
            + skin::S2
            + what_wide;

        let mut x = box_.center().x - total / 2.0;
        painter.text(
            Pos2::new(x, y),
            egui::Align2::LEFT_CENTER,
            w().or_else,
            caption_font.clone(),
            skin::STAGE_INK_DIM,
        );
        x += or_wide + skin::S2;
        for (key, wide) in keys.iter().zip(key_widths.iter()) {
            let cap = Rect::from_min_size(Pos2::new(x, y - 10.0), Vec2::new(*wide, 20.0));
            painter.rect_stroke(
                cap,
                3.0,
                egui::Stroke::new(1.0, skin::STAGE_INK_DIM.gamma_multiply(0.6)),
            );
            painter.text(
                cap.center(),
                egui::Align2::CENTER_CENTER,
                *key,
                key_font.clone(),
                skin::STAGE_INK,
            );
            x += wide + skin::S1;
        }
        x += skin::S2 - skin::S1;
        painter.text(
            Pos2::new(x, y),
            egui::Align2::LEFT_CENTER,
            *what,
            caption_font.clone(),
            skin::STAGE_INK_DIM,
        );
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let response = response.on_hover_text(w().hint_open);
    // A target this size that looks like a target and is not one would be worse
    // than no target at all.
    if response.clicked() {
        Wanted::Open
    } else {
        Wanted::Nothing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Relative luminance, as WCAG defines it.
    fn luminance(colour: egui::Color32) -> f64 {
        let channel = |value: u8| {
            let value = value as f64 / 255.0;
            if value <= 0.03928 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(colour.r()) + 0.7152 * channel(colour.g()) + 0.0722 * channel(colour.b())
    }

    fn contrast(a: egui::Color32, b: egui::Color32) -> f64 {
        let (a, b) = (luminance(a), luminance(b));
        let (light, dark) = if a > b { (a, b) } else { (b, a) };
        (light + 0.05) / (dark + 0.05)
    }

    #[test]
    fn what_is_written_on_the_ground_can_be_read_on_it() {
        // The whole reason these two are their own tokens: the themed greys put
        // this text at 2.76 in the dark theme and 1.76 in the light one, and
        // the ground does not follow the theme so the ink cannot either.
        let title = contrast(skin::STAGE_INK, skin::STAGE);
        let line = contrast(skin::STAGE_INK_DIM, skin::STAGE);
        assert!(title >= 4.5, "titolo a {:.2}", title);
        assert!(line >= 4.5, "righe a {:.2}", line);
        // The corners have to hold the rectangle on their own.
        assert!(contrast(skin::MARK, skin::STAGE) >= 3.0);
    }

    #[test]
    fn the_hierarchy_is_the_one_that_was_measured() {
        // The corners shout, the words speak, the dashes whisper. If the dashes
        // ever came up to the corners the rectangle would read as a border, and
        // a border is not a crop.
        let corners = contrast(skin::MARK, skin::STAGE);
        let dashes = contrast(skin::MARK.gamma_multiply(128.0 / 255.0), skin::STAGE);
        assert!(dashes < corners, "{:.2} contro {:.2}", dashes, corners);
    }

    #[test]
    fn the_box_fits_the_smallest_window() {
        // main.rs allows 960 x 640; the rail and the two bars take their share.
        let stage_wide = 960.0 - crate::widgets::RAIL_WIDTH;
        let stage_tall = 640.0 - 46.0 - 45.0;
        assert!(WIDE + 2.0 * skin::S8 <= stage_wide, "largo {}", stage_wide);
        assert!(TALL + 2.0 * skin::S8 <= stage_tall, "alto {}", stage_tall);
    }
}
