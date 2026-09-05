// Who made this, and under what terms.
//
// The 1.6 build opened this by clicking the name in the top left corner, and
// the 2.0 lost it in the rewrite along with the name itself. The words are the
// 1.6's, taken as they were: they were written for this and nothing about the
// program they describe has changed.
//
// The licence line is not decoration. This program is Apache 2.0 and says so
// where somebody can read it, which is the half of a licence that a LICENSE
// file in a repository does not do for the person running the binary.

use eframe::egui::{self, Context};

pub const AUTHOR: &str = "Giovanni J. Costantini";
pub const AUTHOR_URL: &str = "https://costantini.pw";
pub const PROJECT_URL: &str = "https://github.com/GiovanniCst/Cutaway";
pub const LICENCE: &str = "Apache License 2.0";
pub const LICENCE_URL: &str = "https://www.apache.org/licenses/LICENSE-2.0";

/// The version this binary was built as, from the manifest rather than from a
/// constant somebody has to remember to change.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Draws the window when it is open. Returns false when it asked to close.
pub fn window(ctx: &Context, open: &mut bool) {
    let tokens = crate::skin::tokens(ctx);
    let w = crate::words::w();
    egui::Window::new(w.about_title)
        .open(open)
        .collapsible(false)
        .resizable(false)
        .default_width(420.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            ui.add_space(crate::skin::S2);
            // The wordmark again, at the size a title wants: the same two marks
            // the toolbar carries, so the window that says the name shows the
            // sign that goes with it.
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(
                    egui::Vec2::new(28.0, 28.0),
                    egui::Sense::hover(),
                );
                crate::skin::trim_mark(
                    ui.painter(),
                    rect.left_top() + egui::Vec2::splat(6.0),
                    (-1.0, -1.0),
                    18.0,
                    tokens.accent,
                    false,
                );
                ui.label(
                    egui::RichText::new("CUTAWAY")
                        .text_style(crate::skin::stage_title_style())
                        .color(tokens.ink),
                );
                ui.label(
                    egui::RichText::new(VERSION)
                        .text_style(crate::skin::numeric_style())
                        .color(tokens.ink_faint),
                );
            });

            ui.add_space(crate::skin::S3);
            ui.label(egui::RichText::new(w.about_tagline).color(tokens.ink));
            ui.add_space(crate::skin::S2);
            crate::widgets::caption(ui, w.about_summary);

            ui.add_space(crate::skin::S4);
            ui.horizontal(|ui| {
                crate::widgets::caption(ui, &crate::words::fill(w.about_created_by, &[AUTHOR]));
            });
            ui.horizontal(|ui| {
                ui.hyperlink_to(w.about_author_site, AUTHOR_URL);
                ui.add_space(crate::skin::S2);
                ui.hyperlink_to(w.about_project, PROJECT_URL);
            });

            ui.add_space(crate::skin::S3);
            // No spacing between the pieces: the tail begins with a full stop,
            // and a gap in front of a full stop is a typographic mistake the
            // layout would otherwise insert for us.
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                crate::widgets::caption(ui, w.about_licence_before);
                ui.hyperlink_to(LICENCE, LICENCE_URL);
                crate::widgets::caption(ui, w.about_licence_after);
            });
            ui.add_space(crate::skin::S2);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_comes_from_the_manifest() {
        // Not a constant somebody has to remember to change: this is the number
        // cargo built the binary with.
        assert!(VERSION.starts_with('2'), "{}", VERSION);
        assert!(VERSION.split('.').count() >= 2, "{}", VERSION);
    }

    #[test]
    fn the_licence_is_named_and_reachable() {
        // A licence a person cannot read is a licence nobody read.
        assert!(LICENCE_URL.starts_with("https://"));
        assert!(PROJECT_URL.starts_with("https://"));
        assert!(AUTHOR_URL.starts_with("https://"));
        assert!(LICENCE.contains("Apache"));
    }
}
