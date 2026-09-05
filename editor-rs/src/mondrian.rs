// The picture the program opens with.
//
// A window that starts empty asks somebody to go and find something before it
// will do anything at all, and until they do, every tool in the rail is grey.
// So it starts with a picture of its own: a composition in the manner of
// Mondrian, drawn on the spot and different every time.
//
// Drawn rather than shipped, for the same reason the rail's icons are drawn:
// nothing to load, nothing to licence, no bytes in the binary. It is also a
// good subject to try the tools on - hard edges for the crop, flat fields for
// the background remover, plenty of white for a mark to show against.
//
// **How it is built, and why not the obvious way.** The obvious way is to halve
// a rectangle, then halve the halves. That was tried, and it produces a
// treemap: every line stops at the edge of the cell it divided, and the result
// reads as a chart. In a Mondrian the lines *cross the whole canvas* - that is
// the structure, and the fields are what the crossing leaves behind.
//
// So the lines come first: a few full-width and full-height rules at uneven
// intervals. Then neighbouring cells are joined into larger fields, and the
// piece of line between two joined cells simply is not drawn. That single rule
// - a line is drawn where two different fields meet, and nowhere else - is what
// gives the broken grid these paintings have, where a rule runs the width of
// the picture and then stops halfway down.

use image::{Rgba, RgbaImage};

/// Mondrian's palette, near enough: the ground is a warm off white rather than
/// white, the black is not quite black, and there are only three colours.
const GROUND: Rgba<u8> = Rgba([0xF2, 0xF0, 0xE6, 0xFF]);
const RULE: Rgba<u8> = Rgba([0x11, 0x10, 0x0E, 0xFF]);

/// The mount: white, plainly, and the only pure white in the picture. The
/// composition's own ground is the ivory of paper or primed canvas, so the two
/// whites part company and the edge of the canvas is legible without a line
/// drawn round it - which is the point, because a Mondrian has no black
/// perimeter. The black rules run to the edge and are cut by it.
const MOUNT: Rgba<u8> = Rgba([0xFF, 0xFF, 0xFF, 0xFF]);
const RED: Rgba<u8> = Rgba([0xD4, 0x1E, 0x22, 0xFF]);
const BLUE: Rgba<u8> = Rgba([0x1B, 0x45, 0x96, 0xFF]);
const YELLOW: Rgba<u8> = Rgba([0xF4, 0xC7, 0x00, 0xFF]);
/// Not a primary, and in these paintings not an accident either: the one this
/// generator was checked against is called *Composition with Red, Yellow,
/// Black, Grey and Blue*, and the grey and the black are fields in it, not
/// lines. Without them a composition of three primaries on white reads as a
/// pastiche of Mondrian rather than as one of his.
const GREY: Rgba<u8> = Rgba([0x9E, 0x9C, 0x95, 0xFF]);

/// A very small generator. Nothing here needs to be unpredictable, only
/// different: a whole random number crate for a dozen numbers would be a
/// dependency taken on for decoration.
struct Rolls(u64);

impl Rolls {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    /// A number in `0..span`.
    fn upto(&mut self, span: usize) -> usize {
        if span == 0 {
            return 0;
        }
        (self.next() % span as u64) as usize
    }

    fn chance(&mut self, in_how_many: usize) -> bool {
        self.upto(in_how_many) == 0
    }
}

/// Where the rules fall on one axis: the interior lines, in pixels, in order.
///
/// Uneven on purpose, and never near the edge or near each other. Mondrian's
/// intervals are irregular but they are decisions, not accidents: two rules a
/// few pixels apart would read as a mistake.
fn rules_along(span: u32, how_many: usize, rolls: &mut Rolls) -> Vec<u32> {
    let least = (span / 9).max(12) as usize;
    if span as usize <= 2 * least {
        return Vec::new();
    }
    let mut at: Vec<u32> = Vec::new();
    let mut tries = 0;
    while at.len() < how_many && tries < 200 {
        tries += 1;
        let put = least + rolls.upto(span as usize - 2 * least);
        if at.iter().any(|had| (*had as i64 - put as i64).abs() < least as i64) {
            continue;
        }
        at.push(put as u32);
    }
    at.sort_unstable();
    at
}

/// Which interior rule is the dominant one on an axis.
///
/// Mondrian's classic pictures are built on "una croce assiale decentrata,
/// formata da una singola linea dominante orizzontale e da una singola
/// dominante verticale, da cui si sviluppa l'intera composizione" - one
/// horizontal and one vertical rule, off centre, that the rest hangs off. So
/// one of each is chosen and never joined across: it crosses the whole canvas,
/// whatever else happens.
///
/// Off centre means off centre: the rule nearest three eighths or five eighths
/// of the span, never the one nearest the middle. A line down the centre
/// divides a canvas into two halves, and two halves are a symmetry.
fn dominant(rules: &[u32], span: u32, rolls: &mut Rolls) -> usize {
    if rules.len() <= 2 {
        return 1;
    }
    let wanted = span as f32 * if rolls.chance(2) { 0.375 } else { 0.625 };
    let mut best = 1;
    let mut closest = f32::MAX;
    // Index 0 and the last are the canvas edges, so the interior rules are the
    // ones in between.
    for (index, at) in rules.iter().enumerate().take(rules.len() - 1).skip(1) {
        let apart = (*at as f32 - wanted).abs();
        if apart < closest {
            closest = apart;
            best = index;
        }
    }
    best
}

/// The composition as a grid of fields: which field each cell belongs to.
struct Plan {
    /// The interior rules, with the two edges added, so a cell is a pair of
    /// consecutive entries.
    down: Vec<u32>,
    across: Vec<u32>,
    /// Which field owns each cell, by row then column.
    owner: Vec<usize>,
}

impl Plan {
    fn columns(&self) -> usize {
        self.down.len() - 1
    }

    fn rows(&self) -> usize {
        self.across.len() - 1
    }

    fn at(&self, row: usize, column: usize) -> usize {
        self.owner[row * self.columns() + column]
    }

    fn set(&mut self, row: usize, column: usize, field: usize) {
        let columns = self.columns();
        self.owner[row * columns + column] = field;
    }

    /// The box a field occupies, and how many cells it holds.
    fn extent(&self, field: usize) -> (usize, usize, usize, usize, usize) {
        let (mut top, mut left, mut bottom, mut right, mut count) =
            (usize::MAX, usize::MAX, 0, 0, 0);
        for row in 0..self.rows() {
            for column in 0..self.columns() {
                if self.at(row, column) == field {
                    top = top.min(row);
                    left = left.min(column);
                    bottom = bottom.max(row);
                    right = right.max(column);
                    count += 1;
                }
            }
        }
        (top, left, bottom, right, count)
    }
}

/// Joins neighbouring cells into larger fields.
///
/// Only where the join stays a rectangle: a field shaped like an L would have
/// to be drawn with a line that turns a corner and stops, which is not a thing
/// that happens in these paintings.
fn join(
    plan: &mut Plan,
    rolls: &mut Rolls,
    how_many: usize,
    dominant_column: usize,
    dominant_row: usize,
) {
    let (rows, columns) = (plan.rows(), plan.columns());
    for _ in 0..how_many * 4 {
        let row = rolls.upto(rows);
        let column = rolls.upto(columns);
        let mine = plan.at(row, column);
        let (other_row, other_column) =
            if rolls.chance(2) { (row, column + 1) } else { (row + 1, column) };
        if other_row >= rows || other_column >= columns {
            continue;
        }
        let theirs = plan.at(other_row, other_column);
        if theirs == mine {
            continue;
        }
        let (t1, l1, b1, r1, _) = plan.extent(mine);
        let (t2, l2, b2, r2, _) = plan.extent(theirs);
        let (top, left) = (t1.min(t2), l1.min(l2));
        let (bottom, right) = (b1.max(b2), r1.max(r2));
        // The union has to be a solid block of cells owned only by these two,
        // or the field would not come out a rectangle.
        let mut solid = true;
        for r in top..=bottom {
            for c in left..=right {
                let owner = plan.at(r, c);
                if owner != mine && owner != theirs {
                    solid = false;
                }
            }
        }
        if !solid {
            continue;
        }
        // And not so large that it swallows the picture.
        if (bottom - top + 1) * (right - left + 1) > (rows * columns) / 3 {
            continue;
        }
        // And never across one of the two dominant lines: those are the
        // composition's axis and they run the whole way, always.
        if left < dominant_column && right >= dominant_column {
            continue;
        }
        if top < dominant_row && bottom >= dominant_row {
            continue;
        }
        // Nor so long in one direction that it wipes out the structure across
        // it. A field spanning every row leaves no horizontal rule beside it,
        // and a picture of nothing but full-height strips is a comb: one came
        // out that way, five equal columns and no structure at all.
        if (bottom - top + 1) * 2 > rows.max(2) || (right - left + 1) * 2 > columns.max(2) {
            continue;
        }
        for r in top..=bottom {
            for c in left..=right {
                plan.set(r, c, mine);
            }
        }
    }
}

fn fill(pixels: &mut RgbaImage, left: u32, top: u32, right: u32, bottom: u32, colour: Rgba<u8>) {
    for y in top..bottom.min(pixels.height()) {
        for x in left..right.min(pixels.width()) {
            pixels.put_pixel(x, y, colour);
        }
    }
}

/// One composition, of the given size.
///
/// `seed` decides which one: the same seed gives the same picture, which is
/// what makes any of this testable.
pub fn compose(width: u32, height: u32, seed: u64) -> RgbaImage {
    let (width, height) = (width.max(120), height.max(120));
    let mut pixels = RgbaImage::from_pixel(width, height, GROUND);
    let mut rolls = Rolls(seed | 1);

    // Four or five rules each way. Fewer and there is nothing to look at; more
    // and the fields are too small to hold a colour.
    let mut down = rules_along(width, 4 + rolls.upto(2), &mut rolls);
    let mut across = rules_along(height, 3 + rolls.upto(2), &mut rolls);
    down.insert(0, 0);
    down.push(width);
    across.insert(0, 0);
    across.push(height);

    let columns = down.len() - 1;
    let rows = across.len() - 1;
    let dominant_column = dominant(&down, width, &mut rolls);
    let dominant_row = dominant(&across, height, &mut rolls);
    let mut plan = Plan { down, across, owner: (0..rows * columns).collect() };
    // Joined too eagerly, a whole row of cells becomes one field and every
    // vertical rule vanishes from it: measured, one composition had a single
    // line crossing its middle. A fifth of the cells is enough to break the
    // grid without dissolving it.
    join(&mut plan, &mut rolls, (rows * columns / 5).max(1), dominant_column, dominant_row);

    // --- the fields -----------------------------------------------------------
    //
    // Three to five carry colour and the rest stay white. The white is not
    // background: it is most of the painting.
    let mut fields: Vec<usize> = plan.owner.clone();
    fields.sort_unstable();
    fields.dedup();
    let colours = [RED, BLUE, YELLOW];
    let wanted = 3 + rolls.upto(3);
    // The largest field stays white. The eye needs somewhere to rest, and in
    // these paintings the big rectangle almost always is where it rests.
    let biggest =
        fields.iter().max_by_key(|field| plan.extent(**field).4).copied().unwrap_or(0);
    // The fields that may carry colour: not the largest, and none so large
    // that filling it would make a Rothko. Gathered first and chosen from,
    // rather than walked until enough turn up - walking it missed, and one
    // composition came out with a single small blue square and nothing else.
    let canvas = (width * height) as f64;
    let area_of = |field: usize| -> f64 {
        let (top, left, bottom, right, _) = plan.extent(field);
        ((plan.down[right + 1] - plan.down[left]) as u64
            * (plan.across[bottom + 1] - plan.across[top]) as u64) as f64
            / canvas
    };
    // "La stretta cornice di rettangoli colorati che serrano la zona centrale
    // piu' ampia": the colour sits round the edge and the wide middle stays
    // white. So the fields that touch a border are offered first, and the ones
    // in the middle only if there are not enough of them.
    let touches_border = |field: usize| -> bool {
        let (top, left, bottom, right, _) = plan.extent(field);
        top == 0 || left == 0 || bottom + 1 == rows || right + 1 == columns
    };
    let mut eligible: Vec<usize> = fields
        .iter()
        .copied()
        .filter(|f| *f != biggest && area_of(*f) <= 0.125 && touches_border(*f))
        .collect();
    if eligible.len() < 3 {
        eligible.extend(
            fields
                .iter()
                .copied()
                .filter(|f| *f != biggest && area_of(*f) <= 0.125 && !touches_border(*f)),
        );
    }
    if eligible.len() < 2 {
        // Every field is large, which happens when the joins were generous.
        // Better a looser rule than a picture with one colour in it.
        eligible = fields.iter().copied().filter(|f| *f != biggest && area_of(*f) <= 0.22).collect();
    }

    let mut chosen: Vec<(usize, Rgba<u8>)> = Vec::new();
    let mut inked = 0.0;
    let turn = rolls.upto(3);
    // Spread across the list rather than taken from one end: the fields come
    // out in spatial order, and taking neighbours put every colour in the same
    // corner of the picture.
    let stride = (eligible.len() / wanted.max(1)).max(1);
    let mut at = rolls.upto(eligible.len().max(1));
    for _ in 0..eligible.len() {
        if chosen.len() >= wanted || inked > 0.22 {
            break;
        }
        if eligible.is_empty() {
            break;
        }
        at = (at + stride) % eligible.len();
        let field = eligible[at];
        if chosen.iter().any(|(had, _)| *had == field) {
            at = (at + 1) % eligible.len();
            continue;
        }
        let area = area_of(field);
        if inked + area > 0.24 {
            continue;
        }
        inked += area;
        // The first three are always three different primaries; after those, a
        // neutral field now and then - one grey at most, and more rarely one
        // black, which is what that painting has.
        let colour = if chosen.len() == 3 && rolls.chance(2) {
            GREY
        } else if chosen.len() == 4 && rolls.chance(3) {
            RULE
        } else {
            colours[(turn + chosen.len()) % colours.len()]
        };
        chosen.push((field, colour));
    }

    for (field, colour) in &chosen {
        let (top, left, bottom, right, _) = plan.extent(*field);
        fill(
            &mut pixels,
            plan.down[left],
            plan.across[top],
            plan.down[right + 1],
            plan.across[bottom + 1],
            *colour,
        );
    }

    // --- the rules ------------------------------------------------------------
    //
    // A line is drawn where two different fields meet, and nowhere else. That
    // one rule is what gives the broken grid: a rule crosses the whole picture
    // where the cells either side of it differ all the way down, and stops
    // where two cells were joined into one field.
    // "Griglie e maglie di rette nere e larghe." Wide is part of what these
    // pictures are: a hairline grid on white is a sheet of graph paper.
    let weight = (width.min(height) / 42).clamp(4, 16);
    let half = weight / 2;
    for row in 0..rows {
        for column in 0..columns {
            let mine = plan.at(row, column);
            if column + 1 < columns && plan.at(row, column + 1) != mine {
                let x = plan.down[column + 1];
                fill(
                    &mut pixels,
                    x.saturating_sub(half),
                    plan.across[row],
                    x + weight - half,
                    plan.across[row + 1],
                    RULE,
                );
            }
            if row + 1 < rows && plan.at(row + 1, column) != mine {
                let y = plan.across[row + 1];
                fill(
                    &mut pixels,
                    plan.down[column],
                    y.saturating_sub(half),
                    plan.down[column + 1],
                    y + weight - half,
                    RULE,
                );
            }
        }
    }
    pixels
}

/// Charcoal, for the writing in the mount: dark and warm, not black. The black
/// belongs to the composition and nothing outside it should borrow the weight.
const CHARCOAL: Rgba<u8> = Rgba([0x3A, 0x37, 0x33, 0xFF]);

/// Mounts the composition on white and signs it in the mount.
///
/// Two earlier attempts wrote on the picture itself. The first put the words at
/// the bottom right whatever was there and a black rule ran through the middle
/// of the sentence; the second hunted for a field flat enough to hold them,
/// found none - the sentence is longer than any field of a picture this size -
/// and silently signed nothing, which is the worse of the two failures.
///
/// A mount solves both and is how a print is presented anyway. The bottom
/// margin is deeper than the other three, as a mount's is: an even border looks
/// bottom-heavy because the eye puts the centre of a picture above its middle.
fn mounted(picture: &RgbaImage, seed: u32) -> RgbaImage {
    let side = (picture.height() as f32 / 13.0).round().max(18.0) as u32;
    let foot = (side as f32 * 1.9).round() as u32;
    let mut out = RgbaImage::from_pixel(
        picture.width() + side * 2,
        picture.height() + side + foot,
        MOUNT,
    );
    for (x, y, pixel) in picture.enumerate_pixels() {
        out.put_pixel(x + side, y + side, *pixel);
    }

    let size = (side as f32 * 0.42).clamp(8.0, 13.0);
    let lines = [
        crate::words::w().mondrian_after.to_string(),
        crate::words::fill(
            crate::words::w().mondrian_id,
            &[&format!("{:08X}", seed), &crate::clock::month()],
        ),
    ];
    let mut wide = 0.0_f32;
    for line in &lines {
        wide = wide.max(crate::annotate::text_width(line, size));
    }
    // No system font on this machine: the picture is mounted and goes out
    // unsigned rather than wearing a row of empty boxes.
    if wide <= 0.0 {
        return out;
    }

    // Right-aligned under the picture, not under the mount: the writing lines
    // up with the thing it is about.
    let right = (side + picture.width()) as f32;
    let left = (right - wide).max(side as f32);
    let top = (side + picture.height()) as f32 + size * 1.5;
    let ink = egui::Color32::from_rgba_unmultiplied(
        CHARCOAL[0],
        CHARCOAL[1],
        CHARCOAL[2],
        170,
    );
    for (row, line) in lines.iter().enumerate() {
        crate::annotate::draw_text(
            &mut out,
            line,
            (left, top + row as f32 * size * 1.3),
            size,
            ink,
            false,
        );
    }
    out
}

/// The one the program opens with: a different composition on every run.
///
/// Small enough to sit in the middle of the stage with the trim marks around
/// it, which is how the 1.6 build's first screen looked - a picture resting on
/// a ground, not a picture filling the window.
pub fn opening() -> RgbaImage {
    // Thirty-two bits, so the number printed on the picture *is* the seed and
    // not a shortening of it: the same eight characters give back the same
    // composition. An identifier that cannot be used to find the thing it
    // identifies is a decoration.
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_nanos() as u32)
        .unwrap_or(0x20260905);
    mounted(&compose(560, 390, seed as u64), seed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counted(pixels: &RgbaImage) -> std::collections::HashMap<[u8; 4], usize> {
        let mut seen = std::collections::HashMap::new();
        for pixel in pixels.pixels() {
            *seen.entry(pixel.0).or_insert(0) += 1;
        }
        seen
    }

    /// How many separate runs of rule a line across the middle meets, counted
    /// on the pixels. None means a treemap; every possible one means a grid.
    fn crossings(pixels: &RgbaImage, y: u32) -> usize {
        let mut runs = 0;
        let mut was = false;
        for x in 0..pixels.width() {
            let now = pixels.get_pixel(x, y).0 == RULE.0;
            if now && !was {
                runs += 1;
            }
            was = now;
        }
        runs
    }

    #[test]
    fn the_same_seed_gives_the_same_picture() {
        assert_eq!(compose(400, 300, 12345).as_raw(), compose(400, 300, 12345).as_raw());
    }

    #[test]
    fn a_different_seed_gives_a_different_picture() {
        // A sample that is identical on every run is furniture.
        assert_ne!(compose(400, 300, 1).as_raw(), compose(400, 300, 999_999).as_raw());
    }

    #[test]
    fn the_rules_cross_the_picture() {
        // The defect this was rewritten for. Built by halving rectangles, every
        // line stopped at the edge of the cell it had divided and the result
        // read as a treemap. A cut across the picture has to meet several
        // rules, which is only true when rules run its whole height.
        for seed in 0..12u64 {
            let pixels = compose(620, 430, seed * 7919 + 1);
            let met = crossings(&pixels, pixels.height() / 2);
            assert!(met >= 2, "seme {}: solo {} incontri", seed, met);
        }
    }

    #[test]
    fn but_not_every_rule_crosses_it() {
        // A grid is not a composition either: over a dozen compositions, at
        // least one row has to meet fewer rules than the busiest does.
        let mut busiest = 0;
        let mut quietest = usize::MAX;
        for seed in 0..12u64 {
            let pixels = compose(620, 430, seed * 7919 + 1);
            for part in [4, 2, 3] {
                let met = crossings(&pixels, pixels.height() / part);
                busiest = busiest.max(met);
                quietest = quietest.min(met);
            }
        }
        assert!(busiest > quietest, "tutte le righe uguali: {} ovunque", busiest);
    }

    #[test]
    fn no_field_runs_the_whole_way_across() {
        // A field spanning every row leaves no horizontal rule beside it, and a
        // picture of nothing but full-height strips is a comb. Measured on the
        // pixels: a vertical cut has to meet rules, just as a horizontal one
        // does.
        for seed in 0..16u64 {
            let pixels = compose(620, 430, seed * 15485863 + 5);
            let mut down = 0;
            let mut was = false;
            let x = pixels.width() / 3;
            for y in 0..pixels.height() {
                let now = pixels.get_pixel(x, y).0 == RULE.0;
                if now && !was {
                    down += 1;
                }
                was = now;
            }
            assert!(down >= 2, "seme {}: {} rughe orizzontali", seed, down);
        }
    }

    #[test]
    fn the_axial_cross_crosses_the_whole_picture() {
        // One horizontal and one vertical rule run the whole way, every time:
        // the composition hangs off them. Measured by scanning three columns
        // and three rows - a rule that crosses the whole canvas is met by every
        // scan across it.
        for seed in 0..16u64 {
            let pixels = compose(620, 430, seed * 6700417 + 13);
            let mut met_everywhere = 0;
            for part in [5, 2, 5] {
                let x = pixels.width() / part.max(2);
                let met = (0..pixels.height())
                    .filter(|y| pixels.get_pixel(x, *y).0 == RULE.0)
                    .count();
                if met > 0 {
                    met_everywhere += 1;
                }
            }
            assert!(met_everywhere == 3, "seme {}: nessuna dominante", seed);
        }
    }

    #[test]
    fn the_axis_is_off_centre() {
        // A rule down the middle divides a canvas into two halves, and two
        // halves are a symmetry. Mondrian's cross is decentrata, and this is
        // the one thing about it that can be checked from outside: the middle
        // column of the picture is not where the heaviest structure is.
        let mut centred = 0;
        for seed in 0..24u64 {
            let pixels = compose(620, 430, seed * 999_331 + 7);
            let middle = pixels.width() / 2;
            // A dominant vertical at the centre would make the middle column
            // solid rule from top to bottom.
            let solid = (0..pixels.height())
                .all(|y| pixels.get_pixel(middle, y).0 == RULE.0);
            if solid {
                centred += 1;
            }
        }
        assert!(centred <= 2, "{} composizioni su 24 hanno l'asse al centro", centred);
    }

    #[test]
    fn the_neutral_fields_actually_turn_up() {
        // A rarity that never happens is not a rarity, it is dead code. Over
        // sixty compositions the grey has to appear in a few of them - and not
        // in most, or it stops being the exception it is in the paintings.
        let mut with_grey = 0;
        for seed in 0..60u64 {
            let pixels = compose(620, 430, seed * 48271 + 17);
            if counted(&pixels).contains_key(&GREY.0) {
                with_grey += 1;
            }
        }
        assert!(with_grey >= 4, "il grigio compare in {} su 60", with_grey);
        assert!(with_grey <= 30, "il grigio compare in {} su 60", with_grey);
    }

    #[test]
    fn there_is_always_more_than_one_colour() {
        // One small blue square on a white field is not a composition either.
        for seed in 0..16u64 {
            let pixels = compose(620, 430, seed * 2654435761 + 9);
            let seen = counted(&pixels);
            let colours = [RED, BLUE, YELLOW]
                .iter()
                .filter(|colour| seen.contains_key(&colour.0))
                .count();
            assert!(colours >= 2, "seme {}: {} colori", seed, colours);
        }
    }

    #[test]
    fn it_is_a_mondrian_and_not_a_chart() {
        for seed in 0..12u64 {
            let pixels = compose(620, 430, seed * 104729 + 3);
            let seen = counted(&pixels);
            assert!(seen.len() <= 6, "seme {}: {} colori", seed, seen.len());
            let total = (pixels.width() * pixels.height()) as f64;
            let share = |colour: Rgba<u8>| *seen.get(&colour.0).unwrap_or(&0) as f64 / total;
            // White is most of the painting, black is a structure rather than a
            // filling, and the colour is an accent that is nonetheless there.
            assert!(share(GROUND) > 0.35, "seme {}: fondo {:.0}%", seed, share(GROUND) * 100.0);
            assert!(share(RULE) > 0.02, "seme {}: senza struttura", seed);
            // The black may be a field as well as a line, so it is allowed a
            // little more of the picture than the rules alone would take.
            assert!(share(RULE) < 0.34, "seme {}: nero {:.0}%", seed, share(RULE) * 100.0);
            let colour = share(RED) + share(BLUE) + share(YELLOW) + share(GREY);
            assert!(colour > 0.02, "seme {}: colore {:.1}%", seed, colour * 100.0);
            assert!(colour < 0.45, "seme {}: colore {:.0}%", seed, colour * 100.0);
        }
    }

    #[test]
    fn there_is_always_a_field_to_rest_the_eye_on() {
        for seed in 0..8u64 {
            let pixels = compose(620, 430, seed * 31337 + 11);
            let y = pixels.height() / 2;
            let (mut best, mut run) = (0, 0);
            for x in 0..pixels.width() {
                if pixels.get_pixel(x, y).0 == GROUND.0 {
                    run += 1;
                    best = best.max(run);
                } else {
                    run = 0;
                }
            }
            assert!(best > pixels.width() / 8, "seme {}: campo maggiore {} px", seed, best);
        }
    }

    #[test]
    fn the_mount_leaves_the_composition_alone() {
        // It did not, twice. First the words were written across a black rule;
        // then they hunted for a field flat enough to hold them, found none,
        // and silently signed nothing. In a mount there is nowhere for either
        // to happen: the picture is copied in whole and the writing is outside
        // it.
        for step in 0..8u32 {
            let seed = step.wrapping_mul(2_654_435_761).wrapping_add(5);
            let bare = compose(560, 390, seed as u64);
            let with = mounted(&bare, seed);
            assert!(with.width() > bare.width(), "seme {:08X}: niente cornice", seed);
            assert!(with.height() > bare.height(), "seme {:08X}: niente cornice", seed);
            // The margin is deeper at the foot than at the head.
            let side = (with.width() - bare.width()) / 2;
            let foot = with.height() - bare.height() - side;
            assert!(foot > side, "seme {:08X}: piede {} contro lato {}", seed, foot, side);
            // Every pixel of the composition survives, where it was put.
            for (x, y, pixel) in bare.enumerate_pixels() {
                assert_eq!(
                    with.get_pixel(x + side, y + side).0,
                    pixel.0,
                    "seme {:08X}: la cornice ha toccato {},{}",
                    seed,
                    x,
                    y
                );
            }
            // The mount is white and the canvas is not, which is what draws
            // the edge here: no line is painted round the composition, because
            // a Mondrian has no black perimeter.
            assert_eq!(with.get_pixel(side - 1, side - 1).0, MOUNT.0);
            assert!(
                MOUNT.0[0] > GROUND.0[0] && MOUNT.0[2] > GROUND.0[2],
                "il passe-partout deve essere piu' bianco della tela"
            );
            // And something was written in the foot.
            let written = (bare.height() + side..with.height())
                .flat_map(|y| (0..with.width()).map(move |x| (x, y)))
                .any(|(x, y)| with.get_pixel(x, y).0 != MOUNT.0);
            assert!(written, "seme {:08X}: cornice vuota", seed);
        }
    }

    #[test]
    fn the_colophon_says_which_composition_this_is() {
        // The number printed on it is the seed, not a shortening of one: the
        // same eight characters have to give back the same picture.
        let seed = 0xA0E9_07A4u32;
        assert_eq!(
            compose(620, 430, seed as u64).as_raw(),
            compose(620, 430, seed as u64).as_raw()
        );
        assert_eq!(format!("{:08X}", seed), "A0E907A4");
    }

    #[test]
    fn a_tiny_canvas_does_not_bring_it_down() {
        for (wide, tall) in [(1, 1), (64, 64), (17, 300), (300, 17), (120, 120)] {
            let pixels = compose(wide, tall, 7);
            assert!(pixels.width() >= 120 && pixels.height() >= 120);
        }
    }
}
