// What the window is made of: the colours, the type, the spacing.
//
// One place, because the alternative is what this file replaced - a literal
// grey written once in `ui.rs` that turned out to be the toolbar, the panels,
// the status bar and the ground under the photograph all at the same time.
// Measured, the toolbar and the ground had a contrast ratio of 1.00: they were
// the same colour, and the window had no frame at all.
//
// The rule that keeps it one place: no other file in this program writes a
// literal colour. A colour that is needed and not here is a token that is
// missing.
//
// The ground under the photograph is deliberately outside the theme. It is a
// photometric reference, and a reference that changes with the theme makes the
// same picture look different in the two of them - so a judgement about
// brightness or gamma made in one would not carry to the other. It follows that
// nothing drawn *on* that ground can follow the theme either, which is why the
// crop marks are the same pale cyan in both.

use eframe::egui::{
    self, Color32, FontData, FontDefinitions, FontFamily, FontId, Rounding, Shadow, Stroke,
    TextStyle, Vec2, Visuals,
};

// --- what does not belong to a theme -----------------------------------------

/// The ground the photograph sits on. Neutral: R = G = B.
///
/// The previous value was #4A4C4F - the same lightness, but with five points of
/// blue between R and B. A reference ground with a cast moves the judgement made
/// against it, so this is a de-casting rather than a change of density: L* moves
/// by 0.2, which nobody can see.
pub const STAGE: Color32 = Color32::from_rgb(0x4C, 0x4C, 0x4C);

/// Everything drawn *over* the photograph: the crop frame, the guides, the
/// handles, the outline of a held shape.
///
/// Fixed in both themes because the ground is. The light theme's accent on this
/// ground gives a contrast of 1.50 - invisible; this gives 5.11.
pub const MARK: Color32 = Color32::from_rgb(0x8C, 0xD2, 0xE8);

/// The veil over what a crop would throw away.
pub const VEIL: Color32 = Color32::from_black_alpha(120);

/// Words written on the ground, which like the ground stay outside the theme.
///
/// Chosen against STAGE rather than taken from a palette: the empty stage used
/// to write themed text on the unthemed ground, which measured 2.76 in the dark
/// theme and 1.76 in the light one - a sentence nobody could read in either.
pub const STAGE_INK: Color32 = Color32::from_rgb(0xE8, 0xEA, 0xEC);
pub const STAGE_INK_DIM: Color32 = Color32::from_rgb(0xC2, 0xC7, 0xCB);

/// The halo under a trim mark, so the half that falls on the photograph holds
/// against any subject.
pub const HALO: Color32 = Color32::from_black_alpha(160);

/// The rule-of-thirds guides inside a crop: fainter than the frame, because
/// they are a hint and the frame is the decision - but the same colour as it,
/// so they belong to the same object.
///
/// White at a quarter alpha is not faint on a light photograph, it is absent:
/// the guides read as missing while being drawn every frame. 40% of the
/// selection colour is visible on any subject and still subordinate to it.
/// Premultiplied because a const cannot call the unmultiplied constructor:
/// this is MARK (0x8CD2E8) at alpha 110, worked out once here.
pub const GUIDE: Color32 = Color32::from_rgba_premultiplied(60, 91, 100, 110);

/// What a person draws with. On the photograph, so outside the theme like
/// everything else there: a marker that changed colour with the window would
/// stamp a different colour into the pixels depending on the time of day.
/// The names are in `words`, in the same order: a colour is a colour in both
/// languages, but what it is called is not.
pub const PALETTE: &[Color32] = &[
    Color32::from_rgb(0xE5, 0x3E, 0x3E),
    Color32::from_rgb(0xF5, 0x9E, 0x0B),
    Color32::from_rgb(0xEA, 0xB3, 0x08),
    Color32::from_rgb(0x22, 0xC5, 0x5E),
    Color32::from_rgb(0x3B, 0x82, 0xF6),
    Color32::from_rgb(0xA8, 0x55, 0xF7),
    Color32::from_rgb(0x11, 0x11, 0x11),
    Color32::from_rgb(0xF5, 0xF5, 0xF5),
];

/// How opaque a highlighter is. Below this it is a marker, not a highlighter.
pub const HIGHLIGHT_ALPHA: u8 = 110;

/// The highlighter's own colours, which are not the pen's.
///
/// A pen writes in red, black, blue: colours chosen to be read *as* the mark.
/// A highlighter tints what is underneath and has to stay lighter than it, so
/// the set is the fluorescent one every real highlighter comes in. Laying a
/// dark red down at half strength does not make a highlighter, it makes a
/// stain.
pub const HIGHLIGHTERS: [Color32; 5] = [
    Color32::from_rgb(0xFF, 0xEE, 0x00),
    Color32::from_rgb(0x7C, 0xFF, 0x5A),
    Color32::from_rgb(0x5A, 0xD6, 0xFF),
    Color32::from_rgb(0xFF, 0x8A, 0xD6),
    Color32::from_rgb(0xFF, 0xAB, 0x40),
];

/// The eraser's own colour. It cannot be shown as what it does - it takes
/// pixels away - so it is shown as where it is going.
pub const ERASER: Color32 = Color32::from_rgba_premultiplied(90, 90, 90, 90);

/// The transparency chequer, 8 px squares.
pub const CHECKER_A: Color32 = Color32::from_rgb(0x5A, 0x5A, 0x5A);
pub const CHECKER_B: Color32 = Color32::from_rgb(0x43, 0x43, 0x43);

/// The hairline round the photograph, and the shadow under it.
pub const PIC_EDGE: Color32 = Color32::from_black_alpha(90);
pub const PIC_SHADOW: Color32 = Color32::from_black_alpha(90);

// --- the spacing scale -------------------------------------------------------
//
// Base four, six values, nothing off the scale. Distance says kinship: siblings
// sit at S2, groups at S6. A label sits at S1 from its control and S4 from
// whatever came before.

pub const S0: f32 = 2.0;
pub const S1: f32 = 4.0;
pub const S2: f32 = 8.0;
pub const S3: f32 = 12.0;
pub const S4: f32 = 16.0;
pub const S6: f32 = 24.0;
pub const S8: f32 = 32.0;

// --- the two themes ----------------------------------------------------------

#[derive(Clone, Copy)]
pub struct Tokens {
    /// Toolbar, rail, status bar: the frame around the work.
    pub chrome: Color32,
    /// Side panels, popups, dialogs.
    pub surface: Color32,
    /// Hover fill, the ground of text fields and slider rails.
    pub raised: Color32,
    /// Hover on something that already sits raised.
    pub raised_hi: Color32,
    /// Pressed.
    pub raised_lo: Color32,
    /// Hairlines inside the chrome.
    pub line: Color32,
    /// The chrome-to-stage boundary: the one line meant to cut.
    pub edge: Color32,
    pub ink: Color32,
    /// Section labels, units of measure.
    pub ink_dim: Color32,
    /// Captions, counters, explanations.
    pub ink_faint: Color32,
    pub ink_off: Color32,
    /// Active state, primary action, focus.
    pub accent: Color32,
    /// The filled part of a slider, marks at rest.
    pub accent_lo: Color32,
    /// Text over a filled accent.
    pub on_accent: Color32,
    pub danger: Color32,
}

pub const DARK: Tokens = Tokens {
    chrome: Color32::from_rgb(0x1C, 0x1F, 0x22),
    surface: Color32::from_rgb(0x24, 0x27, 0x2B),
    raised: Color32::from_rgb(0x31, 0x35, 0x3B),
    raised_hi: Color32::from_rgb(0x3B, 0x40, 0x46),
    raised_lo: Color32::from_rgb(0x2A, 0x2E, 0x33),
    line: Color32::from_rgb(0x38, 0x3D, 0x43),
    edge: Color32::from_rgb(0x10, 0x12, 0x14),
    ink: Color32::from_rgb(0xE8, 0xEA, 0xEC),
    ink_dim: Color32::from_rgb(0xA8, 0xAF, 0xB6),
    ink_faint: Color32::from_rgb(0x8B, 0x93, 0x9A),
    ink_off: Color32::from_rgb(0x64, 0x6B, 0x72),
    accent: Color32::from_rgb(0x8C, 0xD2, 0xE8),
    accent_lo: Color32::from_rgb(0x5F, 0xA9, 0xC4),
    on_accent: Color32::from_rgb(0x0A, 0x1D, 0x25),
    danger: Color32::from_rgb(0xE8, 0x8A, 0x8A),
};

pub const LIGHT: Tokens = Tokens {
    chrome: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    surface: Color32::from_rgb(0xF5, 0xF6, 0xF8),
    raised: Color32::from_rgb(0xE1, 0xE5, 0xE9),
    raised_hi: Color32::from_rgb(0xD4, 0xD9, 0xDE),
    raised_lo: Color32::from_rgb(0xC6, 0xCC, 0xD2),
    line: Color32::from_rgb(0xDD, 0xE1, 0xE5),
    edge: Color32::from_rgb(0xAE, 0xB5, 0xBB),
    ink: Color32::from_rgb(0x16, 0x19, 0x1C),
    ink_dim: Color32::from_rgb(0x4B, 0x52, 0x58),
    ink_faint: Color32::from_rgb(0x6B, 0x72, 0x78),
    ink_off: Color32::from_rgb(0x8E, 0x95, 0x9B),
    // The 1.6 build's light accent, sampled from it rather than invented.
    accent: Color32::from_rgb(0x0E, 0x6E, 0x93),
    accent_lo: Color32::from_rgb(0x2E, 0x88, 0xAB),
    on_accent: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    danger: Color32::from_rgb(0xB3, 0x26, 0x1E),
};

/// Black or white, whichever can be seen on top of `under`.
///
/// The green channel counts double because the eye is built that way, and 500
/// is the middle of the range that produces. The same rule was written out
/// twice for the number badges - a yellow badge with white digits says nothing -
/// and it turns out to be the rule the swatch ring needs too.
pub fn over(under: Color32) -> Color32 {
    let bright = under.r() as u32 + under.g() as u32 * 2 + under.b() as u32;
    if bright > 500 {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

/// The tokens for whichever theme is showing.
pub fn tokens(ctx: &egui::Context) -> Tokens {
    if ctx.style().visuals.dark_mode {
        DARK
    } else {
        LIGHT
    }
}

// --- type --------------------------------------------------------------------

/// The program saying its own name.
pub fn wordmark_style() -> TextStyle {
    TextStyle::Name("Wordmark".into())
}

/// The heading of the empty stage, in the same voice as the wordmark.
pub fn stage_title_style() -> TextStyle {
    TextStyle::Name("StageTitle".into())
}

/// Section labels inside a panel: Semibold 12, `ink_dim`.
pub fn label_style() -> TextStyle {
    TextStyle::Name("Label".into())
}

/// Captions, counters, explanations: Regular 11, `ink_faint`.
pub fn caption_style() -> TextStyle {
    TextStyle::Name("Caption".into())
}

/// Every number the window shows. Consolas, so the digits are all one width and
/// `38 ms` becoming `140 ms` moves nothing - which matters because that figure
/// is written again on every frame while a slider is being dragged.
pub fn numeric_style() -> TextStyle {
    TextStyle::Name("Numeric".into())
}

/// The label under a rail icon: Regular 10, `ink_dim`.
pub fn rail_style() -> TextStyle {
    TextStyle::Name("RailLabel".into())
}

/// Reads the three Windows faces. None if any is missing, in which case egui's
/// own fonts stay - a reduced Windows image is not a reason to fail to start.
///
/// Nothing is embedded in the binary: these are read at run time from the
/// system, so nothing is redistributed and there is no licence question.
///
/// Segoe UI Variable is deliberately not used. `ab_glyph`, the rasteriser egui
/// draws with, exposes no way to select an instance of a variable font, so it
/// would render only at the default axis position. The two static files are
/// what work.
fn windows_fonts() -> Option<FontDefinitions> {
    let dir = std::path::PathBuf::from(std::env::var_os("WINDIR")?).join("Fonts");
    let read = |name: &str| std::fs::read(dir.join(name)).ok();
    let (regular, semibold, mono) =
        (read("segoeui.ttf")?, read("seguisb.ttf")?, read("consola.ttf")?);
    // The bold monospace is wanted, not needed: without it the wordmark is set
    // in the regular one. Read with a fallback rather than with `?`, or a
    // machine missing this one file would lose all four faces.
    let mono_bold = read("consolab.ttf").unwrap_or_else(|| mono.clone());

    // The defaults are kept underneath as the fallback for anything these three
    // do not cover - an emoji in a file name, say.
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("ui".into(), FontData::from_owned(regular));
    fonts.font_data.insert("ui-strong".into(), FontData::from_owned(semibold));
    fonts.font_data.insert("num".into(), FontData::from_owned(mono));
    fonts.font_data.insert("num-strong".into(), FontData::from_owned(mono_bold));

    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "ui".to_owned());
    fonts.families.insert(FontFamily::Name("strong".into()), vec!["ui-strong".to_owned()]);
    fonts.families.insert(FontFamily::Name("num".into()), vec!["num".to_owned()]);
    fonts
        .families
        .insert(FontFamily::Name("num-strong".into()), vec!["num-strong".to_owned()]);
    Some(fonts)
}

/// The scale: 15 / 13 / 12 / 11 / 10.
///
/// Narrow on purpose. A wide scale makes headings shout in a window where the
/// only thing that should speak is the photograph; the hierarchy here is made
/// of weight and colour instead.
///
/// `windows` says whether the two extra families exist. When they do not - a
/// reduced Windows image, a machine without Segoe - every style falls back to a
/// family egui always has. Naming a family that is not bound is not a
/// degradation but a panic on the first frame, in epaint, with the window
/// already up: measured, by pointing WINDIR at nothing.
fn text_styles(windows: bool) -> std::collections::BTreeMap<TextStyle, FontId> {
    let strong =
        if windows { FontFamily::Name("strong".into()) } else { FontFamily::Proportional };
    let num = if windows { FontFamily::Name("num".into()) } else { FontFamily::Monospace };
    let wordmark =
        if windows { FontFamily::Name("num-strong".into()) } else { FontFamily::Monospace };
    [
        (TextStyle::Heading, FontId::new(15.0, strong.clone())),
        (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(12.0, num.clone())),
        (TextStyle::Name("Label".into()), FontId::new(12.0, strong)),
        (TextStyle::Name("Caption".into()), FontId::new(11.0, FontFamily::Proportional)),
        (TextStyle::Name("Numeric".into()), FontId::new(12.0, num)),
        (TextStyle::Name("RailLabel".into()), FontId::new(10.0, FontFamily::Proportional)),
        // The two places the program speaks of itself rather than reporting:
        // the wordmark, and the heading of the empty stage. The first plan gave
        // the monospace to numbers and that rule holds - this is the same
        // typeface at a weight no figure ever uses, which is an extension of
        // the rule rather than an exception to it.
        (TextStyle::Name("Wordmark".into()), FontId::new(12.0, wordmark.clone())),
        (TextStyle::Name("StageTitle".into()), FontId::new(15.0, wordmark)),
    ]
    .into_iter()
    .collect()
}

// --- putting it on ------------------------------------------------------------

fn visuals_from(tokens: &Tokens, dark: bool) -> Visuals {
    let mut visuals = if dark { Visuals::dark() } else { Visuals::light() };

    // Not the ground under the photograph. That is the whole point: the stage
    // paints its own, and this is the frame around it.
    visuals.panel_fill = tokens.surface;
    visuals.window_fill = tokens.surface;
    visuals.extreme_bg_color = tokens.raised;
    visuals.faint_bg_color = tokens.raised;
    // The colour of a piece of text is decided by what it is, not by a default
    // that overrides every role at once.
    visuals.override_text_color = None;

    visuals.window_rounding = Rounding::same(10.0);
    visuals.menu_rounding = Rounding::same(8.0);
    visuals.window_stroke = Stroke::new(1.0, tokens.line);
    let shade = Shadow {
        offset: Vec2::new(0.0, 8.0),
        blur: 24.0,
        spread: 0.0,
        color: Color32::from_black_alpha(if dark { 96 } else { 40 }),
    };
    visuals.window_shadow = shade;
    visuals.popup_shadow = shade;

    visuals.slider_trailing_fill = true;
    visuals.handle_shape = egui::style::HandleShape::Circle;
    visuals.indent_has_left_vline = false;
    visuals.striped = false;
    visuals.selection = egui::style::Selection {
        bg_fill: tokens.accent.linear_multiply(0.18),
        stroke: Stroke::new(1.0, tokens.accent),
    };
    visuals.hyperlink_color = tokens.accent;
    visuals.warn_fg_color = tokens.danger;
    visuals.error_fg_color = tokens.danger;

    let widgets = &mut visuals.widgets;
    widgets.noninteractive.weak_bg_fill = tokens.surface;
    widgets.noninteractive.bg_fill = tokens.surface;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, tokens.line);
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, tokens.ink_dim);
    widgets.noninteractive.rounding = Rounding::same(6.0);
    widgets.noninteractive.expansion = 0.0;

    // A button at rest is a word, not a lozenge. Fourteen filled pills in a row
    // is what made the toolbar read as a toolkit demo rather than a program.
    widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    widgets.inactive.bg_fill = tokens.raised;
    widgets.inactive.bg_stroke = Stroke::NONE;
    widgets.inactive.fg_stroke = Stroke::new(1.0, tokens.ink);
    widgets.inactive.rounding = Rounding::same(6.0);
    // Zero, so a button does not grow when the pointer crosses it. The jump is
    // what makes a row of them feel loose.
    widgets.inactive.expansion = 0.0;

    widgets.hovered.weak_bg_fill = tokens.raised;
    widgets.hovered.bg_fill = tokens.raised_hi;
    widgets.hovered.bg_stroke = Stroke::NONE;
    widgets.hovered.fg_stroke = Stroke::new(1.0, tokens.ink);
    widgets.hovered.rounding = Rounding::same(6.0);
    widgets.hovered.expansion = 0.0;

    widgets.active.weak_bg_fill = tokens.raised_lo;
    widgets.active.bg_fill = tokens.raised_lo;
    widgets.active.bg_stroke = Stroke::NONE;
    widgets.active.fg_stroke = Stroke::new(1.0, tokens.ink);
    widgets.active.rounding = Rounding::same(6.0);
    widgets.active.expansion = 0.0;

    widgets.open.weak_bg_fill = tokens.raised;
    widgets.open.bg_fill = tokens.raised;
    widgets.open.bg_stroke = Stroke::new(1.0, tokens.line);
    widgets.open.fg_stroke = Stroke::new(1.0, tokens.ink);
    widgets.open.rounding = Rounding::same(6.0);
    widgets.open.expansion = 0.0;

    visuals
}

/// The frame for the toolbar and the status bar.
///
/// They are the chrome, not a panel: a `TopBottomPanel` left to itself takes
/// `panel_fill`, which is the panels' colour, and then nothing in the window is
/// a different colour from anything else. The hairline is the one line meant to
/// cut - the boundary between the frame and the work.
pub fn chrome_frame(ctx: &egui::Context, at_top: bool) -> egui::Frame {
    let tokens = tokens(ctx);
    egui::Frame::none()
        .fill(tokens.chrome)
        .inner_margin(egui::Margin::symmetric(S3, S2))
        .stroke(Stroke::NONE)
        .outer_margin(egui::Margin {
            top: if at_top { 0.0 } else { S0 / 2.0 },
            bottom: if at_top { S0 / 2.0 } else { 0.0 },
            ..Default::default()
        })
}

/// The frame for a tool panel: the surface colour, and room to breathe.
pub fn panel_frame(ctx: &egui::Context) -> egui::Frame {
    egui::Frame::none()
        .fill(tokens(ctx).surface)
        .inner_margin(egui::Margin::symmetric(S4, S2))
}

/// The nine things a picture can be done to. One per tool, in the order a photo
/// is worked rather than the order the enum happens to be written in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Crop,
    Adjust,
    Cutout,
    Ai,
    Resize,
    Markup,
    Text,
    Print,
    Save,
    /// The three the theme button cycles through.
    Sun,
    Moon,
    Follow,
}

/// The trim mark: two arms meeting at a right angle, standing at a corner and
/// saying where the blade would pass without touching the subject.
///
/// It is the sign of the trade this program is in - a cutaway is the cut, the
/// section, what is left when a piece is taken away - and it was already drawn
/// twice in here, in the crop icon and around a live selection, but never
/// anywhere a person looks first.
///
/// `which` is the corner, as a pair of signs: (-1,-1) is top left. The arm
/// straddles the corner, mostly outside it, so the mark frames what it points
/// at instead of sitting on it.
pub fn trim_mark(
    painter: &egui::Painter,
    corner: egui::Pos2,
    which: (f32, f32),
    arm: f32,
    colour: Color32,
    halo: bool,
) {
    let (sx, sy) = which;
    // Eight out and ten in, which is the proportion the 1.6 build used.
    let out = arm * 8.0 / 18.0;
    let start = egui::Pos2::new(corner.x - sx * out, corner.y - sy * out);
    let along_x = egui::Pos2::new(start.x + sx * arm, start.y);
    let along_y = egui::Pos2::new(start.x, start.y + sy * arm);
    if halo {
        // Under it, wider and black: half of every arm falls on the photograph,
        // and a photograph can be any colour.
        let under = Stroke::new(3.0, HALO);
        painter.line_segment([start, along_x], under);
        painter.line_segment([start, along_y], under);
    }
    let stroke = Stroke::new(1.5, colour);
    painter.line_segment([start, along_x], stroke);
    painter.line_segment([start, along_y], stroke);
}

/// The four corners of a rectangle, each with the signs that point outwards.
pub fn corners(at: egui::Rect) -> [(egui::Pos2, (f32, f32)); 4] {
    [
        (at.left_top(), (-1.0, -1.0)),
        (at.right_top(), (1.0, -1.0)),
        (at.left_bottom(), (-1.0, 1.0)),
        (at.right_bottom(), (1.0, 1.0)),
    ]
}

/// A dashed line, which egui has no primitive for.
pub fn dashes(painter: &egui::Painter, from: egui::Pos2, to: egui::Pos2, colour: Color32) {
    const DASH: f32 = 6.0;
    const GAP: f32 = 5.0;
    let span = to - from;
    let length = span.length();
    if length <= 0.0 {
        return;
    }
    let step = span / length;
    let stroke = Stroke::new(1.5, colour);
    let mut along = 0.0;
    while along < length {
        let end = (along + DASH).min(length);
        painter.line_segment([from + step * along, from + step * end], stroke);
        along = end + GAP;
    }
}

/// Draws one icon inside `at`, which is expected to be square.
///
/// Drawn rather than set in an icon font: no file to load, no codepoint to
/// guess, sharp at any DPI, and consistent with a program that already draws
/// its own crop marks. Every figure is a 1.5 stroke inside the box.
pub fn icon(painter: &egui::Painter, at: egui::Rect, which: Icon, colour: Color32) {
    let stroke = Stroke::new(1.5, colour);
    // A little air, so a figure that reaches its corners does not touch the
    // next thing along.
    let box_ = at.shrink(at.width() * 0.12);
    let (left, top) = (box_.left(), box_.top());
    let (w, h) = (box_.width(), box_.height());
    let line = |a: egui::Pos2, b: egui::Pos2| painter.line_segment([a, b], stroke);
    let at_xy = |fx: f32, fy: f32| egui::Pos2::new(left + w * fx, top + h * fy);

    match which {
        // Two opposed L-squares, the same corners the crop already draws.
        Icon::Crop => {
            line(at_xy(0.0, 0.28), at_xy(0.72, 0.28));
            line(at_xy(0.72, 0.28), at_xy(0.72, 1.0));
            line(at_xy(0.28, 0.0), at_xy(0.28, 0.72));
            line(at_xy(0.28, 0.72), at_xy(1.0, 0.72));
        }
        // Three rails with a knob on each, at different settings.
        Icon::Adjust => {
            for (row, knob) in [(0.2_f32, 0.65_f32), (0.5, 0.35), (0.8, 0.75)] {
                line(at_xy(0.0, row), at_xy(1.0, row));
                painter.circle_filled(at_xy(knob, row), 2.0, colour);
            }
        }
        // A droplet: an arc closed by two sides meeting at the point.
        Icon::Cutout => {
            let tip = at_xy(0.5, 0.06);
            let belly = at_xy(0.5, 0.62);
            painter.circle_stroke(belly, w * 0.32, stroke);
            line(tip, at_xy(0.5 - 0.30, 0.62));
            line(tip, at_xy(0.5 + 0.30, 0.62));
        }
        // A sparkle: the mark this kind of thing wears everywhere. Four long
        // arms and four short ones between them - four alone is a plus sign,
        // which is what it looked like before somebody looked at it.
        Icon::Ai => {
            let middle = box_.center();
            let arm = |dx: f32, dy: f32, length: f32| {
                line(middle, middle + egui::Vec2::new(dx * w * length, dy * h * length))
            };
            for (dx, dy) in [(0.0_f32, -1.0_f32), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)] {
                arm(dx, dy, 0.5);
            }
            let corner = std::f32::consts::FRAC_1_SQRT_2;
            for (dx, dy) in [
                (corner, -corner),
                (corner, corner),
                (-corner, corner),
                (-corner, -corner),
            ] {
                arm(dx, dy, 0.22);
            }
        }
        // A rectangle with a diagonal pulling its far corner out.
        //
        // The arrow starts clear of the rectangle, with a gap. Drawn from
        // inside it, as it was, the two together read as a magnifying glass:
        // a round-cornered lens with a handle coming off it.
        Icon::Resize => {
            painter.rect_stroke(
                egui::Rect::from_min_max(at_xy(0.0, 0.0), at_xy(0.58, 0.58)),
                0.0,
                stroke,
            );
            line(at_xy(0.72, 0.72), at_xy(1.0, 1.0));
            line(at_xy(1.0, 1.0), at_xy(0.7, 1.0));
            line(at_xy(1.0, 1.0), at_xy(1.0, 0.7));
        }
        // A stroke with a wedge nib at the end of it.
        Icon::Markup => {
            line(at_xy(0.1, 0.9), at_xy(0.85, 0.15));
            line(at_xy(0.1, 0.9), at_xy(0.34, 0.82));
            line(at_xy(0.34, 0.82), at_xy(0.24, 0.6));
            line(at_xy(0.24, 0.6), at_xy(0.1, 0.9));
        }
        // An A, in three strokes.
        Icon::Text => {
            line(at_xy(0.12, 1.0), at_xy(0.5, 0.0));
            line(at_xy(0.5, 0.0), at_xy(0.88, 1.0));
            line(at_xy(0.28, 0.66), at_xy(0.72, 0.66));
        }
        // A low box with a sheet coming out of the top of it.
        Icon::Print => {
            painter.rect_stroke(
                egui::Rect::from_min_max(at_xy(0.0, 0.38), at_xy(1.0, 0.82)),
                2.0,
                stroke,
            );
            painter.rect_stroke(
                egui::Rect::from_min_max(at_xy(0.22, 0.0), at_xy(0.78, 0.38)),
                1.0,
                stroke,
            );
        }
        // An arrow going down into an open tray.
        Icon::Save => {
            line(at_xy(0.5, 0.0), at_xy(0.5, 0.62));
            line(at_xy(0.5, 0.62), at_xy(0.26, 0.38));
            line(at_xy(0.5, 0.62), at_xy(0.74, 0.38));
            line(at_xy(0.06, 0.72), at_xy(0.06, 1.0));
            line(at_xy(0.06, 1.0), at_xy(0.94, 1.0));
            line(at_xy(0.94, 1.0), at_xy(0.94, 0.72));
        }
        // A disc with eight short rays.
        //
        // Filled, and the rays kept short. Drawn as an outlined circle with long
        // thin arms it came out at eighteen pixels looking like a cog, which in
        // the corner of a status bar is not a near miss: a cog means settings.
        Icon::Sun => {
            painter.circle_filled(box_.center(), w * 0.26, colour);
            for step in 0..8 {
                let angle = std::f32::consts::TAU * step as f32 / 8.0;
                let arm = Vec2::angled(angle);
                line(box_.center() + arm * w * 0.38, box_.center() + arm * w * 0.5);
            }
        }
        // A crescent, outlined: the far edge of one disc and the near edge of
        // a second one offset into it. Drawn as one closed path rather than a
        // circle with a filled bite, because a bite the colour of the panel
        // stops being invisible the moment the panel changes colour - and this
        // figure's whole job is to be the button that changes it.
        Icon::Moon => {
            let centre = box_.center();
            let radius = w * 0.44;
            let mut outline: Vec<egui::Pos2> = Vec::with_capacity(40);
            let arc = |points: &mut Vec<egui::Pos2>, at: egui::Pos2, r: f32, from: f32, to: f32| {
                for step in 0..=16 {
                    let angle = from + (to - from) * step as f32 / 16.0;
                    points.push(at + Vec2::angled(angle) * r);
                }
            };
            // The lit edge: two thirds of the way round, leaving the horns.
            arc(&mut outline, centre, radius, 1.047, 5.236);
            // And back along the shadow, on a disc pushed into the first.
            let bite = centre + Vec2::new(radius * 0.55, 0.0);
            arc(&mut outline, bite, radius * 0.87, 4.655, 1.628);
            painter.add(egui::Shape::closed_line(outline, stroke));
        }
        // A screen on a stand: what "follow Windows" looks like.
        Icon::Follow => {
            line(at_xy(0.04, 0.14), at_xy(0.96, 0.14));
            line(at_xy(0.96, 0.14), at_xy(0.96, 0.68));
            line(at_xy(0.96, 0.68), at_xy(0.04, 0.68));
            line(at_xy(0.04, 0.68), at_xy(0.04, 0.14));
            line(at_xy(0.5, 0.68), at_xy(0.5, 0.88));
            line(at_xy(0.26, 0.88), at_xy(0.74, 0.88));
        }
    }
}

/// Changes which theme is showing, without touching anything else.
///
/// The fonts and the two palettes are already installed; only the choice
/// between them moves. Separate from `install` so the button in the status bar
/// cannot accidentally re-read three font files on every click.
pub fn wear(ctx: &egui::Context, wanted: crate::settings::Theme) {
    ctx.set_theme(match wanted {
        crate::settings::Theme::System => egui::ThemePreference::System,
        crate::settings::Theme::Light => egui::ThemePreference::Light,
        crate::settings::Theme::Dark => egui::ThemePreference::Dark,
    });
}

/// Puts the skin on, and says how many milliseconds it took.
///
/// The figure is not decoration: this runs on the way to the first frame, and
/// the whole argument for a native window is a number in the tens of
/// milliseconds. Reading three font files is the only part of it that touches
/// the disk, so it is the part worth watching.
pub fn install(ctx: &egui::Context, wanted: crate::settings::Theme) -> u128 {
    let clock = std::time::Instant::now();

    let windows = match windows_fonts() {
        Some(fonts) => {
            ctx.set_fonts(fonts);
            true
        }
        None => false,
    };

    // Both themes, not the current one. `set_style` writes only the style of
    // whichever theme is showing, so the named text styles landed on one theme
    // and the other kept egui's five defaults - and the first `TextStyle::Name`
    // drawn there panicked epaint with the window already up: a machine set to
    // the light theme could not start this program at all, and a machine set to
    // the dark one never met it.
    ctx.all_styles_mut(|style| {
        style.text_styles = text_styles(windows);
        style.spacing.item_spacing = Vec2::new(S2, S2);
        style.spacing.button_padding = Vec2::new(10.0, 6.0);
        // The x at zero leaves the width to the content; only the height is
        // set, because a row of controls that are not the same height reads as
        // an accident.
        style.spacing.interact_size = Vec2::new(0.0, 28.0);
        style.spacing.slider_width = 232.0;
        style.spacing.icon_width = 16.0;
        style.spacing.icon_spacing = S2;
    });

    // Both themes are handed over, and then whoever was asked to choose does.
    // Following Windows is the default: egui-winit feeds the system theme in at
    // startup and again on WindowEvent::ThemeChanged, so a person switching
    // Windows to light while this window is open sees it follow - without
    // reading the registry and without a new dependency. A person who picked a
    // side in the settings gets that side instead, in both builds.
    ctx.set_visuals_of(egui::Theme::Dark, visuals_from(&DARK, true));
    ctx.set_visuals_of(egui::Theme::Light, visuals_from(&LIGHT, false));
    wear(ctx, wanted);

    clock.elapsed().as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Relative luminance, as WCAG defines it.
    fn luminance(colour: Color32) -> f64 {
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

    fn contrast(a: Color32, b: Color32) -> f64 {
        let (a, b) = (luminance(a), luminance(b));
        let (light, dark) = if a > b { (a, b) } else { (b, a) };
        (light + 0.05) / (dark + 0.05)
    }

    /// The numbers the palette was chosen to hit. Written down as assertions
    /// because a palette is easy to adjust and easy to break while adjusting:
    /// the previous one had chrome and stage at 1.00 and nobody noticed until
    /// the pixels were sampled.
    #[test]
    fn nothing_is_below_its_threshold_in_either_theme() {
        for (name, tokens) in [("scuro", DARK), ("chiaro", LIGHT)] {
            let check = |what: &str, pair: f64, floor: f64| {
                assert!(pair >= floor, "{} / {}: {:.2} sotto {:.2}", name, what, pair, floor);
            };
            check("testo su barra", contrast(tokens.ink, tokens.chrome), 4.5);
            check("testo su pannello", contrast(tokens.ink, tokens.surface), 4.5);
            check("testo su rilievo", contrast(tokens.ink, tokens.raised), 4.5);
            check("etichetta su pannello", contrast(tokens.ink_dim, tokens.surface), 4.5);
            // The one that was at 1.10 before: every secondary word in the
            // program is this colour.
            check("didascalia su pannello", contrast(tokens.ink_faint, tokens.surface), 4.5);
            check("didascalia su barra", contrast(tokens.ink_faint, tokens.chrome), 4.5);
            check("disabilitato", contrast(tokens.ink_off, tokens.surface), 2.5);
            check("accento su barra", contrast(tokens.accent, tokens.chrome), 4.5);
            check("accento su pannello", contrast(tokens.accent, tokens.surface), 4.5);
            check("testo su accento", contrast(tokens.on_accent, tokens.accent), 4.5);
            check("errore su pannello", contrast(tokens.danger, tokens.surface), 4.5);
            // The window has a frame again: this pair was 1.00.
            check("barra contro stage", contrast(tokens.chrome, STAGE), 1.60);
            check("pannello contro stage", contrast(tokens.surface, STAGE), 1.60);
            check("bordo contro stage", contrast(tokens.edge, STAGE), 1.60);
            check("rilievo contro pannello", contrast(tokens.raised, tokens.surface), 1.15);
            check("hover contro rilievo", contrast(tokens.raised_hi, tokens.raised), 1.10);
            check("slider riempito", contrast(tokens.accent_lo, tokens.raised), 3.0);
        }
    }

    #[test]
    fn what_is_drawn_on_the_photograph_does_not_follow_the_theme() {
        // The ground is a photometric reference, so the marks over it are fixed
        // too. The light accent on this ground would be 1.50 - invisible.
        assert!(contrast(MARK, STAGE) >= 3.0, "{:.2}", contrast(MARK, STAGE));
        assert!(contrast(LIGHT.accent, STAGE) < 3.0, "il presupposto e cambiato");
    }

    #[test]
    fn every_style_names_a_family_that_exists_without_windows() {
        // Naming an unbound family is not a quiet degradation: epaint panics on
        // the first frame, with the window already on screen. Found by pointing
        // WINDIR at a directory that is not there.
        for (style, font) in text_styles(false) {
            assert!(
                matches!(
                    font.family,
                    FontFamily::Proportional | FontFamily::Monospace
                ),
                "{:?} chiede {:?}, che senza i font di sistema non esiste",
                style,
                font.family
            );
        }
        // And with them, the two extra families are the point of reading them.
        let named: Vec<_> = text_styles(true)
            .into_values()
            .filter(|font| matches!(font.family, FontFamily::Name(_)))
            .collect();
        assert!(!named.is_empty());
    }

    #[test]
    fn the_chosen_swatch_shows_at_both_ends_of_the_palette() {
        // A single white ring, which is what this was, gives 1.09 against the
        // white swatch: choosing white produced no visible answer at all. The
        // ring is double, and each half has its own job: the inner one has to
        // separate from the swatch, the outer one from the panel.
        //
        // The first attempt made the inner ring the panel's colour, on the
        // reasoning that it would always differ from the swatch. It does not:
        // the black swatch against the dark panel came to 1.26, which this test
        // caught and the eye would not have.
        for (name, tokens) in [("scuro", DARK), ("chiaro", LIGHT)] {
            for swatch in PALETTE {
                let inner = contrast(over(*swatch), *swatch);
                assert!(inner >= 3.0, "{} / {:?}: anello interno a {:.2}", name, swatch, inner);
            }
            let outer = contrast(tokens.ink, tokens.surface);
            assert!(outer >= 4.5, "{}: anello esterno a {:.2}", name, outer);
        }
    }

    #[test]
    fn the_ground_is_neutral() {
        // A reference ground with a colour cast moves the judgement made
        // against it. The previous one had five points of blue.
        assert_eq!(STAGE.r(), STAGE.g());
        assert_eq!(STAGE.g(), STAGE.b());
    }

    #[test]
    fn the_spacing_scale_has_no_stray_values() {
        // Base four, and each step distinguishable from the last: 6 and 8 used
        // alternately, as they were, are too close to read as different. S0 is
        // the deliberate half step, and it is only ever a hairline or an offset.
        assert_eq!(S0, S1 / 2.0);
        for value in [S1, S2, S3, S4, S6, S8] {
            assert_eq!(value % 4.0, 0.0, "{} non e sulla scala", value);
        }
        assert!(S6 / S2 >= 3.0, "un gruppo deve staccarsi da un fratello");
    }
}
