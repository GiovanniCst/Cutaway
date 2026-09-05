// The panel that asks a model to change the picture.
//
// Everything here that touches the network happens on a worker thread and comes
// back through a channel: an image model takes tens of seconds, and a window
// that stops repainting for that long is a window Windows offers to close for
// you. The panel's whole job is to say what is happening while it happens.
//
// Three numbers decide which model a person picks - what it costs, how long it
// takes, and where it sits on the board - so all three are on the row, and a
// blank means nobody measured it rather than nought. See `models`.

use eframe::egui::{self, Ui};
use image::RgbaImage;

use crate::ai;
use crate::models::{self, Catalogue, Model, Tier};

type Waiting<T> = std::sync::mpsc::Receiver<Result<T, String>>;

#[derive(PartialEq, Eq, Clone, Copy)]
enum Job {
    Edit,
    Upscale,
}

pub struct Studio {
    provider: &'static str,
    /// The key on its way in. Held only long enough to be encrypted, and never
    /// written anywhere in the clear; see `secrets`.
    typing_key: String,
    /// True while the key box is open, which it is whenever no key is stored
    /// for the chosen provider.
    asking_key: bool,
    /// Whether a key is stored, read once rather than decrypted every frame.
    /// False until the panel has looked, which is why it looks before it draws.
    stored_key: bool,
    /// False until the first draw, so the panel sets itself up however it was
    /// opened - by the button, by CUTAWAY_TOOL, or by a key that was there all
    /// along. Asking the caller to remember is how the panel came to claim a
    /// key it had never looked for.
    seen: bool,
    catalogue: Option<Catalogue>,
    loading: Option<Waiting<Catalogue>>,
    refreshing: Option<Waiting<models::Refresh>>,
    /// What a refresh found, in one line, until something else happens.
    refreshed: Option<String>,
    chosen: Option<String>,
    /// Filters the rest of the catalogue - the part nobody has weighed.
    search: String,
    prompt: String,
    size: ai::Size,
    running: Option<Waiting<ai::Outcome>>,
    job: Job,
    started: Option<std::time::Instant>,
    /// What the last edit cost and took, and what size came back.
    reported: Option<String>,
    trouble: Option<String>,
}

impl Default for Studio {
    fn default() -> Studio {
        Studio {
            provider: ai::OPENROUTER,
            typing_key: String::new(),
            asking_key: false,
            stored_key: false,
            seen: false,
            catalogue: None,
            loading: None,
            refreshing: None,
            refreshed: None,
            chosen: None,
            search: String::new(),
            prompt: String::new(),
            size: ai::Size::Original,
            running: None,
            job: Job::Edit,
            started: None,
            reported: None,
            trouble: None,
        }
    }
}

impl Studio {
    /// True while a call is in flight, so the rest of the window can say so.
    pub fn busy(&self) -> bool {
        self.running.is_some()
    }

    /// Called when the panel is opened: the catalogue is read once, not on
    /// every frame, and only if there is a key to read it with.
    pub fn opened(&mut self, ctx: &egui::Context) {
        self.seen = true;
        self.stored_key = crate::secrets::has_key(self.provider);
        self.asking_key = !self.stored_key;
        if self.catalogue.is_none() && self.loading.is_none() && self.stored_key {
            self.load(ctx);
        }
    }

    fn load(&mut self, ctx: &egui::Context) {
        let (send, receive) = std::sync::mpsc::channel();
        self.loading = Some(receive);
        let provider = self.provider;
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = send.send(models::list(provider));
            ctx.request_repaint();
        });
    }

    fn switch_to(&mut self, provider: &'static str, ctx: &egui::Context) {
        if self.provider == provider {
            return;
        }
        self.provider = provider;
        self.catalogue = None;
        self.chosen = None;
        self.refreshed = None;
        self.trouble = None;
        self.seen = false;
        self.opened(ctx);
    }

    /// The model that is picked, with the sizes it accepts.
    fn picked(&self) -> Option<&Model> {
        let chosen = self.chosen.as_ref()?;
        let catalogue = self.catalogue.as_ref()?;
        catalogue
            .models
            .iter()
            .chain(catalogue.others.iter())
            .find(|model| &model.id == chosen)
    }

    fn start(&mut self, job: Job, source: &RgbaImage, ctx: &egui::Context) {
        let Some(model) = self.picked().cloned() else { return };
        let (send, receive) = std::sync::mpsc::channel();
        self.running = Some(receive);
        self.job = job;
        self.started = Some(std::time::Instant::now());
        self.trouble = None;
        self.reported = None;

        let provider = self.provider;
        let prompt = self.prompt.clone();
        let size = self.size;
        let picture = source.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let outcome = match job {
                Job::Edit => ai::edit(
                    provider,
                    &model.id,
                    &prompt,
                    &picture,
                    &model.aspect_ratios,
                    &model.resolutions,
                    size,
                ),
                Job::Upscale => ai::upscale(
                    provider,
                    &model.id,
                    &picture,
                    &model.aspect_ratios,
                    &model.resolutions,
                    // An upscale at the original size is not an upscale; 2K is
                    // the smallest tier that means anything here.
                    if size == ai::Size::Original { ai::Size::K2 } else { size },
                ),
            };
            let _ = send.send(outcome);
            ctx.request_repaint();
        });
    }

    /// Collects whatever has finished. Returns the new pixels when an edit
    /// came back.
    fn collect(&mut self, ctx: &egui::Context) -> Option<RgbaImage> {
        if let Some(waiting) = &self.loading {
            match waiting.try_recv() {
                Ok(Ok(catalogue)) => {
                    // The first model in the shortlist is the sane default: it
                    // is the top of the frontier list, which is what the whole
                    // ranking exists to identify.
                    if self.chosen.is_none() {
                        self.chosen = catalogue.models.first().map(|model| model.id.clone());
                    }
                    self.catalogue = Some(catalogue);
                    self.loading = None;
                }
                Ok(Err(exc)) => {
                    self.trouble = Some(exc);
                    self.loading = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.loading = None,
            }
        }

        if let Some(waiting) = &self.refreshing {
            match waiting.try_recv() {
                Ok(Ok(fresh)) => {
                    self.refreshed = Some(crate::words::fill(
                        crate::words::w().refreshed,
                        &[
                            &fresh.board_size.to_string(),
                            &fresh.reachable.to_string(),
                            &fresh.generated_at,
                        ],
                    ));
                    self.refreshing = None;
                    // The list changed underneath, so read the catalogue again.
                    self.catalogue = None;
                    self.load(ctx);
                }
                Ok(Err(exc)) => {
                    // A refresh that did not happen is not an update: the stored
                    // list stays exactly as it was, and the menu keeps showing
                    // it under its own date.
                    self.trouble = Some(exc);
                    self.refreshing = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.refreshing = None,
            }
        }

        let waiting = self.running.as_ref()?;
        match waiting.try_recv() {
            Ok(Ok(outcome)) => {
                let model = self.chosen.clone().unwrap_or_default();
                // Every edit is also a measurement: the response says what it
                // cost, the clock says how long it took, and both are worth more
                // than any number read off a price list. An upscale is not
                // recorded - a 2K render has no business in the economics of a
                // 1024 edit.
                if self.job == Job::Edit {
                    models::record(
                        &model,
                        Some(outcome.seconds),
                        outcome.usd,
                        Some(outcome.returned.0.max(outcome.returned.1)),
                    );
                }
                let mut said = match outcome.usd {
                    Some(usd) => format!("{:.0} s, ${:.4}", outcome.seconds, usd),
                    None => format!("{:.0} s", outcome.seconds),
                };
                said.push_str(" · ");
                said.push_str(&crate::words::fill(
                    crate::words::w().answered_at,
                    &[&outcome.returned.0.to_string(), &outcome.returned.1.to_string()],
                ));
                if outcome.resolution_refused {
                    said.push_str(" · ");
                    said.push_str(crate::words::w().size_refused);
                }
                self.reported = Some(said);
                self.running = None;
                self.started = None;
                // The catalogue holds the measured columns, so it is stale now.
                if let Some(catalogue) = self.catalogue.as_mut() {
                    models::apply_measurements(&mut catalogue.models);
                }
                Some(outcome.picture)
            }
            Ok(Err(exc)) => {
                self.trouble = Some(exc);
                self.running = None;
                self.started = None;
                None
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(250));
                None
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.running = None;
                self.started = None;
                None
            }
        }
    }

    /// Draws the panel. Returns the pixels to put in the picture, when an edit
    /// has come back.
    pub fn panel(&mut self, ui: &mut Ui, source: &RgbaImage) -> Option<RgbaImage> {
        let ctx = ui.ctx().clone();
        if !self.seen {
            self.opened(&ctx);
        }
        let arrived = self.collect(&ctx);

        crate::widgets::panel_title(ui, crate::words::w().ai_title);

        ui.horizontal(|ui| {
            if ui
                .selectable_label(self.provider == ai::OPENROUTER, "OpenRouter")
                .on_hover_text(crate::words::w().hint_provider)
                .clicked()
            {
                self.switch_to(ai::OPENROUTER, &ctx);
            }
            if ui
                .selectable_label(self.provider == ai::OPENAI, "OpenAI")
                .on_hover_text(crate::words::w().hint_provider)
                .clicked()
            {
                self.switch_to(ai::OPENAI, &ctx);
            }
        });
        ui.add_space(6.0);

        self.key_box(ui, &ctx);
        if self.asking_key {
            return arrived;
        }

        self.model_list(ui, &ctx);

        crate::widgets::section(ui, crate::words::w().what_to_change);
        ui.add(
            egui::TextEdit::multiline(&mut self.prompt)
                .desired_rows(3)
                
                .hint_text(crate::words::w().prompt_hint),
        );
        ui.add_space(6.0);

        crate::widgets::section(ui, crate::words::w().answer_size);
        ui.horizontal_wrapped(|ui| {
            for size in [ai::Size::Original, ai::Size::K1, ai::Size::K2, ai::Size::K4] {
                ui.selectable_value(&mut self.size, size, size.label())
                    .on_hover_text(crate::words::w().hint_answer_size);
            }
        });
        ui.add_space(crate::skin::S1);
        crate::widgets::caption(
            ui,
            match self.size {
                ai::Size::Original => crate::words::w().stays_same_size,
                _ => crate::words::w().comes_back_bigger,
            },
        );
        ui.add_space(crate::widgets::FOOT);

        let ready = self.picked().is_some() && self.running.is_none();
        if crate::widgets::primary(ui, crate::words::w().apply, ready && !self.prompt.trim().is_empty())
            .on_hover_text(crate::words::w().hint_ai_apply)
            .clicked()
        {
            self.start(Job::Edit, source, &ctx);
        }
        ui.add_space(crate::skin::S2);
        if crate::widgets::secondary(ui, crate::words::w().enlarge, ready)
            .on_hover_text(
                crate::words::w().enlarge_hint,
            )
            .clicked()
        {
            self.start(Job::Upscale, source, &ctx);
        }

        if let Some(started) = self.started {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(format!("{:.0} s", started.elapsed().as_secs_f32()));
            });
            crate::widgets::caption(ui, crate::words::w().model_working);
        }
        if let Some(said) = &self.reported {
            ui.add_space(crate::skin::S2);
            let said = said.clone();
            crate::widgets::caption(ui, &said);
        }
        if let Some(exc) = &self.trouble {
            ui.add_space(6.0);
            ui.colored_label(crate::skin::tokens(&ctx).danger, exc);
        }

        arrived
    }

    fn key_box(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        if !self.asking_key {
            // The caption on its own line and the buttons under it. Side by
            // side they made a row that cannot wrap, and a row that cannot wrap
            // in a fixed panel widens the panel: measured, this one pushed the
            // AI panel 75 px past its own frame and took "Rimuovi" with it.
            crate::widgets::caption(ui, crate::words::w().key_stored);
            ui.add_space(crate::skin::S1);
            ui.horizontal(|ui| {
                if ui.small_button(crate::words::w().change).on_hover_text(crate::words::w().hint_change_key).clicked() {
                    self.asking_key = true;
                }
                if ui.small_button(crate::words::w().forget).on_hover_text(crate::words::w().hint_forget_key).clicked() {
                    let _ = crate::secrets::forget_key(self.provider);
                    self.catalogue = None;
                    self.chosen = None;
                    self.stored_key = false;
                    self.asking_key = true;
                }
            });
            ui.add_space(crate::skin::S2);
            return;
        }

        ui.label(crate::words::fill(
            crate::words::w().key_for,
            &[if self.provider == ai::OPENAI { "OpenAI" } else { "OpenRouter" }],
        ));
        ui.add(
            egui::TextEdit::singleline(&mut self.typing_key)
                .password(true)
                
                .hint_text("sk-..."),
        );
        // Said here rather than in a readme nobody opens: this is the moment
        // somebody hands a program a thing that costs them money.
        crate::widgets::caption(ui, crate::words::w().key_stays_here);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.typing_key.trim().is_empty(), egui::Button::new(crate::words::w().save))
                .on_hover_text(crate::words::w().hint_save_key)
                .clicked()
            {
                match crate::secrets::save_key(self.provider, self.typing_key.trim()) {
                    Ok(()) => {
                        // Out of memory the moment it is on disk encrypted.
                        self.typing_key.clear();
                        self.stored_key = true;
                        self.asking_key = false;
                        self.trouble = None;
                        self.load(ctx);
                    }
                    Err(exc) => self.trouble = Some(exc),
                }
            }
            if self.stored_key
                && ui.button(crate::words::w().cancel).on_hover_text(crate::words::w().hint_leave_as_is).clicked()
            {
                self.typing_key.clear();
                self.asking_key = false;
            }
        });
        if let Some(exc) = &self.trouble {
            ui.add_space(6.0);
            ui.colored_label(crate::skin::tokens(&ctx).danger, exc);
        }
        ui.add_space(6.0);
    }

    fn model_list(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        if self.loading.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(crate::words::w().reading_catalogue);
            });
            return;
        }
        let Some(catalogue) = &self.catalogue else {
            if ui
                .button(crate::words::w().read_catalogue)
                .on_hover_text(crate::words::w().hint_read_catalogue)
                .clicked()
            {
                self.load(ctx);
            }
            return;
        };
        if catalogue.offline {
            crate::widgets::caption(ui, crate::words::w().catalogue_unreachable);
        }

        let mut chosen = self.chosen.clone();
        egui::ScrollArea::vertical().max_height(260.0).id_salt("modelli").show(ui, |ui| {
            let mut tier = None;
            for model in &catalogue.models {
                if tier != Some(model.tier) {
                    if tier.is_some() {
                        ui.add_space(4.0);
                    }
                    crate::widgets::caption(ui, model.tier.label());
                    tier = Some(model.tier);
                }
                if row(ui, model, chosen.as_deref() == Some(&model.id)) {
                    chosen = Some(model.id.clone());
                }
            }

            if !catalogue.others.is_empty() {
                ui.add_space(6.0);
                crate::widgets::caption(ui, Tier::Unweighed.label());
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        
                        .hint_text(crate::words::w().search_other_models),
                );
                let needle = self.search.trim().to_lowercase();
                if !needle.is_empty() {
                    let mut shown = 0;
                    for model in &catalogue.others {
                        if !model.id.to_lowercase().contains(&needle)
                            && !model.name.to_lowercase().contains(&needle)
                        {
                            continue;
                        }
                        if row(ui, model, chosen.as_deref() == Some(&model.id)) {
                            chosen = Some(model.id.clone());
                        }
                        shown += 1;
                        if shown >= 20 {
                            crate::widgets::caption(ui, "...");
                            break;
                        }
                    }
                    if shown == 0 {
                        crate::widgets::caption(ui, crate::words::w().no_model_by_that_name);
                    }
                }
            }
        });
        self.chosen = chosen;

        // Read out before the row is drawn: the closure below would otherwise
        // hold the catalogue borrowed while asking for a refresh that replaces it.
        let read_on = catalogue.refreshed_at.clone();
        let idle = self.refreshing.is_none();
        let mut refresh_now = false;
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            // The date is the one piece of information that keeps every other
            // one honest.
            match &read_on {
                Some(at) => crate::widgets::caption(
                    ui,
                    &crate::words::fill(crate::words::w().board_read_on, &[at]),
                ),
                None => crate::widgets::caption(
                    ui,
                    crate::words::w().no_board_for_openai,
                ),
            }
            if read_on.is_some() {
                refresh_now = ui
                    .add_enabled(idle, egui::Button::new(crate::words::w().refresh).small())
                    .on_hover_text(crate::words::w().refresh_hint)
                    .clicked();
            }
        });
        if refresh_now {
            self.start_refresh(ctx);
        }
        if self.refreshing.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                crate::widgets::caption(ui, crate::words::w().refreshing);
            });
        }
        if let Some(said) = self.refreshed.clone() {
            crate::widgets::caption(ui, &said);
        }
    }

    fn start_refresh(&mut self, ctx: &egui::Context) {
        let Some(key) = crate::secrets::load_key(self.provider) else { return };
        let (send, receive) = std::sync::mpsc::channel();
        self.refreshing = Some(receive);
        self.refreshed = None;
        self.trouble = None;
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = send.send(models::refresh(&key));
            ctx.request_repaint();
        });
    }
}

/// What one model's row says, as text.
///
/// Separate from the drawing so that a test can read it. It used to be checked
/// through a copy of this arithmetic written inside the test itself, which is a
/// test that passes when the copy is right and the program is wrong.
fn row_text(model: &Model) -> (String, String) {
    let w = crate::words::w();
    let mut first = model.name.clone();
    if !model.vendor.is_empty() {
        first.push_str(&format!("  ·  {}", model.vendor));
    }
    // A model with no comparable price says so rather than going quietly blank:
    // the ones without a price are also the expensive ones.
    match model.usd {
        Some(usd) => first.push_str(&format!("  ·  ${:.3}", usd)),
        None if model.tier != Tier::Unweighed => {
            first.push_str("  ·  ");
            first.push_str(w.no_comparable_price);
        }
        None => {}
    }
    if let Some(seconds) = model.seconds {
        first.push_str(&format!("  ·  {:.0} s", seconds));
    }

    let second = match model.rank {
        Some(rank) => crate::words::fill(w.rank_position, &[&rank.to_string()]),
        None => model.id.clone(),
    };
    (first, second)
}

/// One model, two lines: what decides the choice, then where it sits.
fn row(ui: &mut Ui, model: &Model, picked: bool) -> bool {
    let (first, second) = row_text(model);
    ui.selectable_label(picked, format!("{}\n{}", first, second))
        .on_hover_text(&model.id)
        .clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weighed(usd: Option<f64>, seconds: Option<f64>, rank: Option<u32>) -> Model {
        Model {
            id: "vendor/model".into(),
            name: "Un Modello".into(),
            vendor: "Vendor".into(),
            tier: Tier::Top,
            elo: Some(1250),
            rank,
            usd,
            seconds,
            aspect_ratios: Vec::new(),
            resolutions: Vec::new(),
        }
    }

    /// The program's own row, not a copy of it.
    fn line(model: &Model) -> String {
        row_text(model).0
    }

    #[test]
    fn a_missing_price_is_said_rather_than_left_blank() {
        let priced = line(&weighed(Some(0.048), Some(22.0), Some(5)));
        assert!(priced.contains("$0.048"), "{}", priced);
        assert!(priced.contains("22 s"), "{}", priced);

        // In both languages, because a row that only says this in one of them
        // is a row that goes blank in the other.
        for lang in [crate::words::Lang::It, crate::words::Lang::En] {
            crate::words::speak(lang);
            let unpriced = line(&weighed(None, Some(9.0), Some(4)));
            assert!(
                unpriced.contains(crate::words::w().no_comparable_price),
                "{:?}: {}",
                lang,
                unpriced
            );
            // And it must not read as free.
            assert!(!unpriced.contains("$0"), "{:?}: {}", lang, unpriced);
        }
    }

    #[test]
    fn an_unweighed_model_carries_no_invented_columns() {
        let mut bare = weighed(None, None, None);
        bare.tier = Tier::Unweighed;
        bare.vendor = String::new();
        let text = line(&bare);
        // Nothing about price or time: nobody weighed it, and a blank is honest.
        assert_eq!(text, "Un Modello");
    }

    #[test]
    fn a_new_studio_asks_for_nothing_it_has_not_got() {
        let studio = Studio::default();
        assert!(!studio.busy());
        assert_eq!(studio.provider, ai::OPENROUTER);
        assert!(studio.chosen.is_none());
        // The size starts where an edit belongs, not where an upscale does.
        assert_eq!(studio.size, ai::Size::Original);
        // And it claims nothing about a key it has not looked for: the panel
        // once said "key saved" on a machine that had none.
        assert!(!studio.seen);
        assert!(!studio.stored_key);
    }
}
