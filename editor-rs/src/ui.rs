// The window: a toolbar, a stage, and a line at the bottom that says what is on
// it.
//
// The stage draws the picture through a texture with a transform of its own -
// wheel to zoom, middle button to pan - which is the same arrangement the
// WebView2 build had, minus the layer of CSS that made a transform necessary in
// the first place.

use std::sync::mpsc::Receiver;

use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};

use crate::capture::{self, Delivery};
use crate::crop::{Grip, Selection};
use crate::picture::Picture;
use crate::Started;

#[derive(PartialEq, Clone, Copy)]
enum Tool {
    None,
    Text,
    Save,
    Print,
    Crop,
    Cutout,
    Resize,
    Adjust,
    Markup,
    Ai,
}

pub struct Editor {
    picture: Option<Picture>,
    /// A capture on its way, when this editor was started for one.
    incoming: Option<Receiver<Delivery>>,
    /// Still hidden, waiting for the piece that justifies showing the window.
    hidden: bool,
    zoom: f32,
    pan: Vec2,
    /// What went wrong, if anything did, said where it happened rather than in a
    /// dialog that has to be dismissed before the program can be used.
    trouble: Option<String>,
    /// How long the window took to be usable, kept for the status line: this
    /// build exists because of that number.
    opened_in_ms: Option<u128>,
    /// When the window appeared, so the boast about how fast it did can stop
    /// being on the screen after a few seconds. It is a measurement, not a
    /// status: worth seeing once.
    opened_at: std::time::Instant,
    /// Under CUTAWAY_FPS: how many frames were drawn in the last second.
    ///
    /// It exists to keep one promise honest. Animations are only affordable
    /// because egui asks for a repaint while an interpolation is in flight and
    /// stops when it lands, so a still window has to draw *nothing* - and the
    /// only way to know is to count. Silence is the answer being looked for:
    /// nothing is printed because no frame is drawn.
    counting_frames: bool,
    frames: (u32, std::time::Instant),
    /// Which panel is open on the left, if any.
    tool: Tool,
    /// The blade's rectangle, in fractions of the picture, and what is being
    /// held onto while it is dragged.
    selection: Selection,
    holding: Grip,
    /// The marks put on the picture, and everything about putting them there.
    markup: crate::markup::Markup,
    /// The AI panel: the key, the model list, and whatever call is in flight.
    studio: crate::studio::Studio,
    /// What the resize panel is set to, and whether it keeps the proportions.
    wanted: (u32, u32),
    proportional: bool,
    /// What the background remover is set to, and whether the dropper is armed.
    keying: crate::cutout::Keying,
    picking_colour: bool,
    /// What the save panel is set to, and how big the file came out when it was
    /// last encoded with those settings.
    format: crate::save::Format,
    quality: u8,
    weighed: Option<(crate::save::Format, u8, usize)>,
    /// What the print panel is set to.
    /// What the OCR read, and whether it reached the clipboard.
    /// What the OCR made of the picture: None until it has been asked, then
    /// either the reading or the reason there is none.
    read_text: Option<Result<(String, usize, bool), String>>,
    /// A scrolling capture in flight, and how it is going.
    scrolling: Option<
        std::sync::mpsc::Receiver<
            Result<Option<(image::RgbaImage, usize, crate::scroll::Stop)>, String>,
        >,
    >,
    /// A long capture asked for but not yet started: the window is still up,
    /// counting down, so there is somewhere to read what is about to happen and
    /// somewhere to change your mind. It used to vanish for three silent
    /// seconds and come back with a picture, which is hard to tell from a
    /// program that has crashed and recovered.
    aiming: bool,
    /// True while this window put itself away to photograph the screen behind
    /// it. Distinct from `hidden`, which means the window was never shown at
    /// all: one has to come back, the other has to appear.
    stood_aside: bool,
    /// Something that went right, said briefly. Not `trouble`: an outcome is
    /// not a fault, and putting the two in one slot teaches a person to read
    /// the red strip as bad news even when it is not.
    notice: Option<(String, std::time::Instant)>,
    paper: crate::print::Paper,
    landscape: bool,
    margin_mm: f32,
    /// How long the last full recompute of the adjustments took. Shown because
    /// it is the measurement that decides whether a slider can drive the whole
    /// picture rather than a downscaled stand-in.
    adjust_ms: u128,
    /// Where to write a picture of this window, when asked through CUTAWAY_SHOT.
    ///
    /// Not a feature: a way to check the window from outside without reading the
    /// screen. PrintWindow returns a blank rectangle for a window drawn in
    /// OpenGL, and reading the desktop needs a desktop that is awake - neither
    /// is true for a test running unattended. The frame buffer knows what it
    /// drew.
    shot_to: Option<std::path::PathBuf>,
    shot_asked: bool,
    /// Whether the credits are showing.
    crediting: bool,
    /// Where the last file dialog was pointed. Kept for the run rather than
    /// written down: the settings file is shared with the 1.6 build, whose own
    /// writer drops any key it does not know, so a folder stored there would be
    /// erased the next time somebody changed a setting over there. A run is
    /// where this actually matters anyway - saving three crops in a row.
    last_folder: Option<std::path::PathBuf>,
}

impl Editor {
    pub fn new(cc: &eframe::CreationContext<'_>, started: Started) -> Editor {
        let mut editor = Editor {
            picture: None,
            incoming: None,
            hidden: started.capture_from.is_some(),
            zoom: 1.0,
            pan: Vec2::ZERO,
            trouble: None,
            // CUTAWAY_TOOL goes with CUTAWAY_SHOT: a panel cannot be clicked
            // open by a test that has no hands, and a picture of the window is
            // worth little if it only ever shows the window with nothing out.
            tool: match std::env::var("CUTAWAY_TOOL").as_deref() {
                Ok("crop") => Tool::Crop,
                Ok("adjust") => Tool::Adjust,
                Ok("markup") => Tool::Markup,
                Ok("resize") => Tool::Resize,
                Ok("cutout") => Tool::Cutout,
                Ok("save") => Tool::Save,
                Ok("print") => Tool::Print,
                Ok("text") => Tool::Text,
                Ok("ai") => Tool::Ai,
                _ => Tool::None,
            },
            selection: Selection::whole(),
            holding: Grip::None,
            markup: crate::markup::Markup::default(),
            studio: crate::studio::Studio::default(),
            wanted: (0, 0),
            proportional: true,
            keying: crate::cutout::Keying::default(),
            picking_colour: false,
            format: crate::save::Format::Png,
            quality: 90,
            weighed: None,
            read_text: None,
            scrolling: None,
            aiming: false,
            stood_aside: false,
            notice: None,
            paper: crate::print::Paper::A4,
            landscape: false,
            margin_mm: 10.0,
            adjust_ms: 0,
            opened_in_ms: None,
            opened_at: std::time::Instant::now(),
            // Read once. Asking the environment on every frame would be a
            // cost paid by every user to measure a thing nobody is measuring.
            counting_frames: std::env::var_os("CUTAWAY_FPS").is_some(),
            frames: (0, std::time::Instant::now()),
            shot_to: std::env::var_os("CUTAWAY_SHOT").map(std::path::PathBuf::from),
            shot_asked: false,
            crediting: false,
            last_folder: None,
        };
        // Colours, type and spacing, all from one place - and both themes, so
        // the window follows whichever one Windows is set to. What used to be
        // here was a single grey assigned to `panel_fill`, which turned out to
        // be the toolbar, the panels, the status bar and the ground under the
        // photograph at once: measured, chrome against stage came to 1.00.
        let chosen = crate::settings::read();
        crate::words::speak(chosen.language);
        let skin_ms = crate::skin::install(&cc.egui_ctx, chosen.theme);
        if skin_ms > 5 {
            // On the way to the first frame, and the whole argument for a native
            // window is a number in the tens of milliseconds. Reading three font
            // files is the only part of this that touches the disk.
            eprintln!("skin: {} ms", skin_ms);
        }

        // The half that listens for the key, brought up if it is not already.
        // The installer starts it; a zip cannot, so until this was here the
        // portable was unzipped, run, and the shortcut the program is named for
        // did nothing at all. Skipped when the agent itself started this window:
        // it is plainly already running.
        if started.agent_pid.is_none() {
            capture::wake_agent();
        }

        if let Some(folder) = &started.capture_from {
            editor.incoming = Some(capture::wait_in(folder, started.agent_pid));
        } else if let Some(path) = &started.open {
            match Picture::open(path) {
                Ok(picture) => editor.picture = Some(picture),
                Err(exc) => editor.trouble = Some(format!("{}: {}", path.display(), exc)),
            }
        } else {
            // Nothing was asked for, so the program brings something of its
            // own: a composition drawn on the spot, different every time. A
            // window that opens empty asks somebody to go and find a picture
            // before it will do anything at all, and until they do, every tool
            // in the rail is grey and none of them can be tried.
            editor.picture = Some(crate::picture::Picture::adopt(
                crate::mondrian::opening(),
                crate::words::w().sample_name.to_string(),
            ));
        }
        // The credits are not a Tool, so they get their own word in the same
        // hook: a window that only a click can open is a window a check with no
        // hands can never look at.
        editor.crediting = matches!(std::env::var("CUTAWAY_TOOL").as_deref(), Ok("about"));
        editor.aiming = matches!(std::env::var("CUTAWAY_TOOL").as_deref(), Ok("lunga"));
        // Whatever opened a panel, it gets the same setting-up. CUTAWAY_TOOL
        // sets `tool` directly and used to skip this, so the text panel opened
        // saying it could not read the picture when nobody had asked it to -
        // the same shape of defect the AI panel had, where a flag only the
        // click path set made the window claim a key it had never looked for.
        if editor.tool != Tool::None {
            editor.entered(&cc.egui_ctx);
        }
        editor.opened_in_ms = Some(started.clock.elapsed().as_millis());
        if editor.shot_to.is_some() {
            // Said out loud only under the test hook, so a run that is being
            // measured reports the number instead of drawing it in a corner
            // somebody then has to read off a screenshot.
            eprintln!("aperto in {} ms", editor.opened_in_ms.unwrap_or(0));
        }
        editor
    }

    /// Takes delivery of a capture, if one has arrived since the last frame.
    fn collect(&mut self, ctx: &egui::Context) {
        let Some(waiting) = &self.incoming else { return };
        match waiting.try_recv() {
            Ok(Delivery::Piece(pixels, name)) => {
                self.adopt(Picture::adopt(pixels, name));
                self.incoming = None;
                if self.hidden {
                    self.hidden = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                self.come_back(ctx);
            }
            Ok(Delivery::Nothing) => {
                // Cancelled, or the agent went away. A window that was never
                // shown has nothing to show and leaving is the point; one that
                // stood aside has to come back, or the person is left looking at
                // a program that vanished when they pressed Escape.
                if self.hidden {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                self.come_back(ctx);
                self.incoming = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                // Still drawing the rectangle. Ask to be woken rather than
                // spinning: a hidden window has nothing to redraw.
                ctx.request_repaint_after(std::time::Duration::from_millis(30));
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => self.incoming = None,
        }
    }

    fn open_dialog(&mut self) {
        let mut dialog = rfd::FileDialog::new().add_filter(
            crate::words::w().pictures,
            &["png", "jpg", "jpeg", "webp", "tif", "tiff", "bmp", "gif"],
        );
        if let Some(folder) = &self.last_folder {
            dialog = dialog.set_directory(folder);
        }
        if let Some(path) = dialog.pick_file() {
            self.open_path(&path);
        }
    }

    /// Opens one file, from wherever it came: the dialog, or dropped on the
    /// window.
    fn open_path(&mut self, path: &std::path::Path) {
        match Picture::open(path) {
            Ok(picture) => {
                self.remember_folder(path);
                self.adopt(picture);
            }
            Err(exc) => self.trouble = Some(format!("{}: {}", path.display(), exc)),
        }
    }

    fn remember_folder(&mut self, path: &std::path::Path) {
        if let Some(folder) = path.parent() {
            self.last_folder = Some(folder.to_path_buf());
        }
    }

    /// A picture dragged onto the window.
    ///
    /// The first one that opens wins: dropping a folder full of them and
    /// getting the last is not what anybody meant, and this program holds one
    /// picture at a time.
    fn take_dropped(&mut self, ctx: &egui::Context) {
        let dropped: Vec<std::path::PathBuf> = ctx.input(|i| {
            i.raw.dropped_files.iter().filter_map(|file| file.path.clone()).collect()
        });
        if let Some(path) = dropped.into_iter().next() {
            self.open_path(&path);
        }
    }

    /// Says the window will take it, while something is being dragged over it.
    ///
    /// Without this the drop works and nothing suggests that it would, which is
    /// the same as not having it: nobody tries.
    fn show_drop_target(&self, ctx: &egui::Context) {
        let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
        if !hovering {
            return;
        }
        let tokens = crate::skin::tokens(ctx);
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("trascinato"),
        ));
        let screen = ctx.screen_rect();
        painter.rect_filled(screen, 0.0, tokens.accent.gamma_multiply(0.10));
        painter.rect_stroke(
            screen.shrink(crate::skin::S3),
            8.0,
            egui::Stroke::new(2.0, tokens.accent),
        );
        painter.text(
            screen.center(),
            egui::Align2::CENTER_CENTER,
            crate::words::w().drop_here,
            egui::TextStyle::resolve(&egui::TextStyle::Heading, &ctx.style()),
            tokens.accent,
        );
    }

    /// Takes a new picture as the one being worked on.
    ///
    /// The view starts over with it: a picture of a different size at the old
    /// zoom and pan is somewhere off screen, which reads as nothing having
    /// happened.
    fn adopt(&mut self, picture: crate::picture::Picture) {
        self.picture = Some(picture);
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
        self.selection = Selection::whole();
        self.trouble = None;
    }

    /// What a mode needs doing when it is entered or left.
    ///
    /// One place, because the rail entry is now the only way in and there is no
    /// reason for each of them to remember its own housekeeping - which is how
    /// the colour dropper used to stay armed after its panel had closed.
    fn entered(&mut self, ctx: &egui::Context) {
        // Left the background remover: the preview it was showing is not a
        // decision and does not survive.
        if self.tool != Tool::Cutout {
            self.picking_colour = false;
            if let Some(picture) = self.picture.as_mut() {
                picture.forget_preview();
            }
        }
        match self.tool {
            // A fresh blade starts around the whole picture rather than
            // wherever it was left last time.
            Tool::Crop => self.selection = Selection::whole(),
            Tool::Save => self.weighed = None,
            Tool::Cutout => self.picking_colour = true,
            Tool::Resize => {
                if let Some(picture) = &self.picture {
                    self.wanted = (picture.width(), picture.height());
                }
            }
            Tool::Ai => self.studio.opened(ctx),
            _ => {}
        }
    }

    /// Brings the window back after it stood aside for a capture.
    fn come_back(&mut self, ctx: &egui::Context) {
        if !self.stood_aside {
            return;
        }
        self.stood_aside = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    /// Cuts a rectangle out of the screen, from inside the window.
    ///
    /// The status bar has been telling people to cut a piece out of the screen
    /// since the first day, and the window offered no button and no shortcut
    /// for it - only the resident agent's global hotkey, which somebody who has
    /// this window in front of them has no reason to know about. The README has
    /// promised Ctrl+Shift+S since the 1.6 build.
    ///
    /// The window goes away first: it would otherwise be part of the screen
    /// being frozen.
    fn cut_from_screen(&mut self, ctx: &egui::Context) {
        if self.incoming.is_some() {
            return;
        }
        let Some(agent) = crate::capture::find_agent() else {
            self.trouble = Some(crate::words::w().no_agent.to_string());
            return;
        };
        let folder = crate::capture::fresh_folder();
        if std::fs::create_dir_all(&folder).is_err() {
            return;
        }
        // Away first, and the agent started only once it has gone. The two used
        // to happen in the same frame: `Minimized` is a request Windows gets
        // round to, the agent freezes the screen the moment it starts, and the
        // frozen screen still had this window standing in the middle of it.
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        self.stood_aside = true;
        self.incoming = Some(crate::capture::wait_in(&folder, None));

        let waiting = folder.clone();
        std::thread::spawn(move || {
            // Long enough for the window to be gone, short enough not to feel
            // like a wait. The scrolling capture uses the same figure for the
            // same reason.
            std::thread::sleep(std::time::Duration::from_millis(400));
            if std::process::Command::new(agent).arg("--once").arg(&waiting).spawn().is_err() {
                // The watcher is already waiting on this folder, and taking it
                // away is how it is told there will be nothing.
                let _ = std::fs::remove_dir_all(&waiting);
            }
        });
    }

    /// Starts a scrolling capture of whatever is under the pointer.
    ///
    /// The window goes away first and comes back with the result: it would
    /// otherwise photograph itself, and the thing being captured is behind it.
    fn start_scrolling(&mut self, ctx: &egui::Context) {
        let (send, receive) = std::sync::mpsc::channel();
        self.scrolling = Some(receive);
        self.notice = None;
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        std::thread::spawn(move || {
            // A moment for the window to actually be gone: it stands in front
            // of the thing being photographed until Windows has finished
            // putting it away.
            std::thread::sleep(std::time::Duration::from_millis(400));
            // Then the person picks the window by clicking it, with a minute to
            // do it in and Escape to give up.
            let Some(at) =
                crate::scroll::wait_for_click(std::time::Duration::from_secs(60))
            else {
                let _ = send.send(Ok(None));
                return;
            };
            let outcome = match crate::scroll::window_at(at.x, at.y) {
                Some(target) => crate::scroll::capture(&target, &crate::scroll::Limits::default())
                    .map(Some),
                None => Err(crate::words::w().no_window_under_pointer.to_string()),
            };
            let _ = send.send(outcome);
        });
    }

    /// Says what is about to happen, and waits to be told to go.
    ///
    /// This used to be a thin strip at the bottom with a countdown in it, and
    /// it was neither read nor understood: a countdown makes somebody hurry,
    /// and the thing they are hurrying to do - put the pointer on the right
    /// window - is the one thing the capture cannot recover from getting wrong.
    /// Now it is a window in the middle that says the whole arrangement, and
    /// nothing starts until the person says so.
    fn take_aim(&mut self, ctx: &egui::Context) {
        if !self.aiming {
            return;
        }
        let tokens = crate::skin::tokens(ctx);
        let w = crate::words::w();
        let mut go = false;
        let mut stop = false;
        egui::Window::new(w.long_capture_title)
            .collapsible(false)
            .resizable(false)
            .default_width(420.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.add_space(crate::skin::S2);
                ui.label(egui::RichText::new(w.long_capture_body).color(tokens.ink));
                ui.add_space(crate::skin::S3);
                crate::widgets::caption(ui, w.long_capture_escape);
                ui.add_space(crate::widgets::FOOT);
                go = crate::widgets::primary(ui, w.long_capture_go, true)
                    .on_hover_text(w.hint_long_go)
                    .clicked();
                ui.add_space(crate::skin::S2);
                stop = crate::widgets::secondary(ui, w.cancel, true)
                    .on_hover_text(w.hint_leave_as_is)
                    .clicked();
            });
        if stop {
            self.aiming = false;
        }
        if go {
            self.aiming = false;
            self.start_scrolling(ctx);
        }
    }

    /// Takes delivery of a scrolling capture, when one finishes.
    fn collect_scrolling(&mut self, ctx: &egui::Context) {
        let Some(waiting) = &self.scrolling else { return };
        match waiting.try_recv() {
            Ok(outcome) => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                let w = crate::words::w();
                match outcome {
                    Ok(Some((pixels, frames, why))) => {
                        let tall = pixels.height();
                        let name = crate::words::fill(
                            w.long_capture_name,
                            &[&crate::clock::stamp_file()],
                        );
                        self.adopt(crate::picture::Picture::adopt(pixels, name));
                        // Said, because one frame and forty produce the same
                        // kind of picture and only the number tells them apart:
                        // a page that would not scroll looks exactly like a
                        // page that scrolled once. And when it stopped for a
                        // reason of its own, that reason is the thing to say.
                        let said = match why {
                            crate::scroll::Stop::TooTall => {
                                crate::words::fill(w.long_capture_capped, &[&tall.to_string()])
                            }
                            crate::scroll::Stop::Cancelled => {
                                w.long_capture_cancelled.to_string()
                            }
                            crate::scroll::Stop::Ended => crate::words::fill(
                                w.long_capture_done,
                                &[&frames.to_string(), &tall.to_string()],
                            ),
                        };
                        self.notice = Some((said, std::time::Instant::now()));
                    }
                    // Given up before it began.
                    Ok(None) => {
                        self.notice =
                            Some((w.long_capture_cancelled.to_string(), std::time::Instant::now()));
                    }
                    Err(exc) => self.trouble = Some(exc),
                }
                self.scrolling = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => self.scrolling = None,
        }
    }

    /// Writes the picture to a temporary file and hands it to the mail client.
    ///
    /// Nothing is sent from here: MAPI opens the client with the message ready
    /// and the person presses send. The address is theirs to type, in their own
    /// client, where their address book is.
    fn mail_now(&mut self) {
        let Some(picture) = &self.picture else { return };
        let folder = std::env::temp_dir();
        let name = format!("Cutaway {}.png", crate::clock::stamp_file());
        let path = folder.join(&name);
        let written = crate::save::encode(&picture.pixels, crate::save::Format::Png, 100)
            .and_then(|bytes| crate::save::write(&path, &bytes));
        if let Err(exc) = written {
            self.trouble = Some(crate::words::fill(crate::words::w().could_not_attach, &[&exc.to_string()]));
            return;
        }
        match crate::mail::compose(&path, "", "") {
            Ok(()) => self.trouble = None,
            Err(exc) => self.trouble = Some(exc),
        }
        // MAPI copies the attachment before it returns, so the file has done its
        // job by now; leaving it in the temporary folder would leave a picture
        // of somebody's screen lying about.
        let _ = std::fs::remove_file(&path);
    }

    /// Reads the text out of the picture and puts it straight on the clipboard.
    ///
    /// Copied without being asked, because that is what reading a screenshot is
    /// for: the text is wanted somewhere else, and a step between here and there
    /// is a step that serves nothing.
    fn read_now(&mut self) {
        let Some(picture) = &self.picture else { return };
        match crate::ocr::read(&picture.pixels) {
            Ok(reading) => {
                let copied = if reading.text.trim().is_empty() {
                    false
                } else {
                    crate::clip::put_text(&reading.text)
                };
                self.read_text = Some(Ok((reading.text, reading.lines, copied)));
                self.trouble = None;
            }
            Err(exc) => {
                // In the panel, not in the red strip: this is the answer to a
                // question that was asked, and the place to read an answer is
                // where the question was put.
                self.read_text = Some(Err(exc.to_string()));
            }
        }
    }

    /// Asks where to put it, and writes exactly what was weighed.
    fn save_now(&mut self) {
        let Some(picture) = &self.picture else { return };
        // The suggested name is the source with a suffix: no path through this
        // program overwrites the file it opened.
        let stem = picture.name.rsplit_once('.').map(|(s, _)| s).unwrap_or(&picture.name);
        let mut dialog = rfd::FileDialog::new()
            .set_file_name(format!("{}-edited.{}", stem, self.format.extension()))
            .add_filter(self.format.label(), &[self.format.extension()]);
        if let Some(folder) = &self.last_folder {
            dialog = dialog.set_directory(folder);
        }
        let Some(path) = dialog.save_file() else { return };
        // The format follows what the panel says, not the suffix typed into the
        // dialog: the size shown was measured for this one.
        match crate::save::encode(&picture.pixels, self.format, self.quality)
            .and_then(|bytes| crate::save::write(&path, &bytes))
        {
            Ok(()) => {
                self.remember_folder(&path);
                self.trouble = None;
                self.tool = Tool::None;
            }
            Err(exc) => self.trouble = Some(crate::words::fill(crate::words::w().could_not_save, &[&exc.to_string()])),
        }
    }
}

impl Editor {
    /// Writes the frame buffer out when CUTAWAY_SHOT asked for it, then leaves.
    fn keep_shot(&mut self, ctx: &egui::Context) {
        let Some(target) = self.shot_to.clone() else { return };
        if !self.shot_asked {
            // One frame of settling first, so the picture is in the texture and
            // the panels have been laid out at their real size.
            self.shot_asked = true;
            ctx.request_repaint_after(std::time::Duration::from_millis(400));
            return;
        }
        let taken = ctx.input(|i| {
            i.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        match taken {
            Some(image) => {
                let pixels: Vec<u8> =
                    image.pixels.iter().flat_map(|p| p.to_array()).collect();
                let saved = image::RgbaImage::from_raw(
                    image.width() as u32,
                    image.height() as u32,
                    pixels,
                )
                .map(|buffer| buffer.save(&target));
                let _ = saved;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            None => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
        }
    }
}

impl Editor {
    /// The keys, read before anything is drawn.
    ///
    /// The same ones the WebView2 build answers to, because they are the ones
    /// already in people's hands - and because a program that moves to a new
    /// toolkit and quietly changes its shortcuts has moved the furniture in the
    /// dark.
    fn keys(&mut self, ctx: &egui::Context) {
        let (open, save, copy, paste, escape) = ctx.input(|i| {
            (
                i.modifiers.command && i.key_pressed(egui::Key::O),
                // Not with shift: that is the screen-capture shortcut, and
                // without this exclusion pressing it both opened the save panel
                // and went off to photograph the screen.
                i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::S),
                i.modifiers.command && i.key_pressed(egui::Key::C),
                i.modifiers.command && i.key_pressed(egui::Key::V),
                i.key_pressed(egui::Key::Escape),
            )
        });
        if open {
            self.open_dialog();
        }
        if save && self.picture.is_some() {
            self.tool = Tool::Save;
            self.entered(ctx);
        }
        if copy {
            if let Some(picture) = &self.picture {
                crate::clip::put(&picture.pixels);
            }
        }
        if paste {
            if let Some(pixels) = crate::clip::take() {
                self.adopt(crate::picture::Picture::adopt(pixels, crate::words::w().clipboard_name.into()));
            }
        }
        if ctx.input(|i| {
            i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::S)
        }) {
            self.cut_from_screen(ctx);
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::P))
            && self.picture.is_some()
        {
            self.tool = Tool::Print;
            self.entered(ctx);
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z)) {
            if let Some(picture) = self.picture.as_mut() {
                picture.undo();
            }
        }
        if escape && self.tool != Tool::None {
            // Escape puts the tool down rather than closing the window: losing
            // an open picture to a stray keypress is not a thing to risk.
            self.tool = Tool::None;
            self.entered(ctx);
        }
    }
}

impl eframe::App for Editor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.counting_frames {
            self.frames.0 += 1;
            if self.frames.1.elapsed().as_secs_f32() >= 1.0 {
                eprintln!("{} frame nell'ultimo secondo", self.frames.0);
                self.frames = (0, std::time::Instant::now());
            }
        }
        self.collect(ctx);
        self.collect_scrolling(ctx);
        // Drawn last of the bottom strips so it sits above the status bar, and
        // before the panels so a countdown cannot be hidden behind one.
        self.take_aim(ctx);
        crate::about::window(ctx, &mut self.crediting);
        self.take_dropped(ctx);
        self.show_drop_target(ctx);
        self.keys(ctx);
        // Nothing to draw and nothing to show: the window is still hidden behind
        // the agent's overlay, waiting for a rectangle to be released.
        if self.hidden {
            return;
        }

        // The toolbar carries what is done *to* the program - open a file, put
        // something on the clipboard, undo - and nothing that is a mode. Seven
        // entries where there were fourteen.
        egui::TopBottomPanel::top("strumenti")
            .frame(crate::skin::chrome_frame(ctx, true))
            .show(ctx, |ui| {
                let has = self.picture.is_some();
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = crate::skin::S2;
                    if crate::widgets::wordmark(ui) {
                        self.crediting = true;
                    }
                    crate::widgets::divider(ui);
                    if ui.button(crate::words::w().open).on_hover_text(crate::words::w().hint_open).clicked() {
                        self.open_dialog();
                    }
                    if ui.button(crate::words::w().paste).on_hover_text(crate::words::w().hint_paste).clicked() {
                        match crate::clip::take() {
                            Some(pixels) => {
                                self.adopt(
                                    crate::picture::Picture::adopt(pixels, crate::words::w().clipboard_name.into()),
                                );
                            }
                            None => {
                                self.trouble = Some(crate::words::w().clipboard_has_no_picture.into())
                            }
                        }
                    }
                    if ui
                        .add_enabled(has, egui::Button::new(crate::words::w().copy))
                        .on_hover_text(crate::words::w().hint_copy)
                        .clicked()
                    {
                        if let Some(picture) = &self.picture {
                            if !crate::clip::put(&picture.pixels) {
                                self.trouble =
                                    Some(crate::words::w().clipboard_busy.into());
                            }
                        }
                    }
                    if ui
                        .add_enabled(has && crate::mail::available(), egui::Button::new(crate::words::w().email))
                        .on_hover_text(crate::words::w().email_hint)
                        .clicked()
                    {
                        self.mail_now();
                    }

                    crate::widgets::divider(ui);

                    if ui
                        .button(crate::words::w().cut_from_screen)
                        .on_hover_text(crate::words::w().cut_from_screen_hint)
                        .clicked()
                    {
                        self.cut_from_screen(ctx);
                    }
                    if ui
                        .add_enabled(
                            self.scrolling.is_none() && !self.aiming,
                            egui::Button::new(crate::words::w().long_capture),
                        )
                        .on_hover_text(crate::words::w().long_capture_hint)
                        .clicked()
                    {
                        self.aiming = true;
                    }

                    // The view and the undo sit at the far end: they are about
                    // looking, not about doing.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let can_undo =
                            self.picture.as_ref().map(|p| p.can_undo()).unwrap_or(false);
                        if ui
                            .add_enabled(can_undo, egui::Button::new(crate::words::w().undo))
                            .on_hover_text(crate::words::w().hint_undo)
                            .clicked()
                        {
                            if let Some(picture) = self.picture.as_mut() {
                                picture.undo();
                            }
                        }
                        crate::widgets::divider(ui);
                        if ui
                            .add_enabled(has, egui::Button::new("+"))
                            .on_hover_text(crate::words::w().zoom_in)
                            .clicked()
                        {
                            self.zoom = (self.zoom * 1.25).min(8.0);
                        }
                        // The number is the control: clicking it goes back to
                        // life size, which is what somebody reaching for it
                        // wants nine times out of ten.
                        let percent = egui::RichText::new(format!("{:>4.0}%", self.zoom * 100.0))
                            .text_style(crate::skin::numeric_style());
                        if ui
                            .add_enabled(has, egui::Button::new(percent))
                            .on_hover_text(crate::words::w().actual_size)
                            .clicked()
                        {
                            self.zoom = 1.0;
                            self.pan = Vec2::ZERO;
                        }
                        if ui
                            .add_enabled(has, egui::Button::new("\u{2212}"))
                            .on_hover_text(crate::words::w().zoom_out)
                            .clicked()
                        {
                            self.zoom = (self.zoom / 1.25).max(0.05);
                        }
                    });
                });
            });

        // The rail is the enum made visible: one entry per Tool, always there,
        // and the panel it opens appears immediately beside it. The button and
        // its consequence used to be eight hundred logical pixels apart.
        egui::SidePanel::left("modi")
            .frame(
                egui::Frame::none()
                    .fill(crate::skin::tokens(ctx).chrome)
                    .inner_margin(egui::Margin {
                        top: crate::skin::S3,
                        ..Default::default()
                    }),
            )
            .resizable(false)
            .exact_width(crate::widgets::RAIL_WIDTH)
            .show(ctx, |ui| {
                let has = self.picture.is_some();
                ui.spacing_mut().item_spacing.y = crate::skin::S1;
                // The order a photo is worked, not the order the enum is
                // written in: cut it down, correct it, take things out, make it
                // bigger, mark it, read it, put it on paper, keep it.
                let w = crate::words::w();
                for (tool, icon, label, hint) in [
                    (Tool::Crop, crate::skin::Icon::Crop, w.crop, w.hint_rail_crop),
                    (Tool::Resize, crate::skin::Icon::Resize, w.resize, w.hint_rail_resize),
                    (Tool::Ai, crate::skin::Icon::Ai, w.ai, w.hint_rail_ai),
                    (Tool::Cutout, crate::skin::Icon::Cutout, w.cutout, w.hint_rail_cutout),
                    (Tool::Markup, crate::skin::Icon::Markup, w.markup, w.hint_rail_markup),
                    (Tool::Adjust, crate::skin::Icon::Adjust, w.adjust, w.hint_rail_adjust),
                    (Tool::Text, crate::skin::Icon::Text, w.ocr, w.ocr_hint),
                    (Tool::Print, crate::skin::Icon::Print, w.print, w.hint_rail_print),
                    (Tool::Save, crate::skin::Icon::Save, w.save, w.hint_rail_save),
                ] {
                    let active = self.tool == tool;
                    if crate::widgets::rail_entry(ui, icon, label, active, has)
                        .on_hover_text(hint)
                        .clicked()
                    {
                        self.tool = if active { Tool::None } else { tool };
                        self.entered(ctx);
                    }
                }
            });

        if self.tool == Tool::Crop && self.picture.is_some() {
            let mut apply = false;
            let mut cancel = false;
            let mut shut = false;
            egui::SidePanel::left("ritaglia")
                .frame(crate::skin::panel_frame(ctx))
                .resizable(false)
                .exact_width(crate::widgets::PANEL_WIDTH).show(ctx, |ui| {
                let picture = self.picture.as_ref().expect("checked above");
                if crate::widgets::panel_title(ui, crate::words::w().crop) {
                    shut = true;
                }
                let (was_w, was_h) = (picture.width(), picture.height());
                let (_, _, width, height) = self.selection.in_pixels(was_w, was_h);
                crate::widgets::number(ui, &format!("{} \u{00d7} {} px", width, height));
                crate::widgets::caption(
                    ui,
                    &crate::words::fill(
                        crate::words::w().from_size,
                        &[&was_w.to_string(), &was_h.to_string()],
                    ),
                );

                // The shapes a screenshot is usually asked to be. The 1.6 build
                // had these and the rewrite lost them, which is the difference
                // between cropping to 16:9 and cropping to nearly 16:9 by hand.
                crate::widgets::section(ui, crate::words::w().proportions);
                ui.horizontal_wrapped(|ui| {
                    for (name, ratio) in crate::crop::RATIOS {
                        if ui.button(*name).on_hover_text(crate::words::w().hint_ratio).clicked() {
                            self.selection = self.selection.at_ratio(*ratio, was_w, was_h);
                        }
                    }
                    if ui.button(crate::words::w().whole_picture).on_hover_text(crate::words::w().hint_whole).clicked() {
                        self.selection = Selection::whole();
                    }
                });
                ui.add_space(crate::widgets::FOOT);
                apply = crate::widgets::primary(ui, crate::words::w().apply, !self.selection.empty())
                    .on_hover_text(crate::words::w().hint_crop_apply)
                    .clicked();
                ui.add_space(crate::skin::S2);
                cancel = crate::widgets::secondary(ui, crate::words::w().cancel, true)
                    .on_hover_text(crate::words::w().hint_leave_as_is)
                    .clicked();
            });
            if shut {
                self.tool = Tool::None;
                self.entered(ctx);
            }
            if apply {
                if let Some(picture) = self.picture.as_mut() {
                    picture.cut_to(self.selection);
                }
                self.selection = Selection::whole();
                self.tool = Tool::None;
            }
            if cancel {
                self.selection = Selection::whole();
                self.tool = Tool::None;
            }
        }

        if self.tool == Tool::Text {
            let mut shut = false;
            let mut read = false;
            egui::SidePanel::left("testo")
                .frame(crate::skin::panel_frame(ctx))
                .resizable(false)
                .exact_width(crate::widgets::WIDE_PANEL_WIDTH).show(ctx, |ui| {
                if crate::widgets::panel_title(ui, crate::words::w().ocr) {
                    shut = true;
                }
                match &self.read_text {
                    Some(Ok((text, lines, copied))) if !text.trim().is_empty() => {
                        // Where this came from, said out loud. The panel used to
                        // open with a wall of text and no account of where it
                        // had been got, which reads as the program having found
                        // it somewhere rather than having read the picture.
                        crate::widgets::caption(
                            ui,
                            if *copied {
                                crate::words::w().text_from_picture
                            } else {
                                crate::words::w().clipboard_was_busy
                            },
                        );
                        ui.add_space(crate::skin::S1);
                        crate::widgets::number(
                            ui,
                            &crate::words::count(
                                *lines,
                                crate::words::w().one_line,
                                crate::words::w().lines_count,
                            ),
                        );
                        ui.add_space(crate::skin::S2);
                        egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                            // Selectable rather than editable: this is what was
                            // read, and a text box would invite correcting it
                            // here instead of in whatever it is pasted into.
                            ui.add(egui::TextEdit::multiline(&mut text.as_str()));
                        });
                        ui.add_space(crate::skin::S2);
                        if ui.button(crate::words::w().copy_again).on_hover_text(crate::words::w().hint_copy_again).clicked() {
                            let again = crate::clip::put_text(text);
                            self.read_text = Some(Ok((text.clone(), *lines, again)));
                        }
                    }
                    Some(Ok(_)) => {
                        ui.label(crate::words::w().no_text_found);
                    }
                    Some(Err(why)) => {
                        ui.label(crate::words::w().could_not_read);
                        ui.add_space(crate::skin::S1);
                        crate::widgets::caption(ui, why);
                    }
                    None => {
                        crate::widgets::caption(ui, crate::words::w().ocr_nothing_yet);
                    }
                }
                ui.add_space(crate::widgets::FOOT);
                // Asked for, not sprung. It used to run the moment the panel
                // opened, so a wall of text appeared with no account of where it
                // had come from - and the one thing it did, putting that text on
                // the clipboard, happened without anybody asking for it.
                read = crate::widgets::primary(ui, crate::words::w().ocr_read, true)
                    .on_hover_text(crate::words::w().ocr_hint)
                    .clicked();
                ui.add_space(crate::skin::S2);
                if crate::widgets::secondary(ui, crate::words::w().close, true)
                    .on_hover_text(crate::words::w().hint_close_panel)
                    .clicked() {
                    shut = true;
                }
            });
            if read {
                self.read_now();
            }
            if shut {
                self.tool = Tool::None;
                self.entered(ctx);
            }
        }

        if self.tool == Tool::Print && self.picture.is_some() {
            let mut send = false;
            let mut shut = false;
            egui::SidePanel::left("stampa")
                .frame(crate::skin::panel_frame(ctx))
                .resizable(false)
                .exact_width(crate::widgets::PANEL_WIDTH).show(ctx, |ui| {
                if crate::widgets::panel_title(ui, crate::words::w().print) {
                    shut = true;
                }
                ui.label(crate::words::w().sheet);
                ui.horizontal_wrapped(|ui| {
                    for paper in crate::print::Paper::ALL {
                        if ui
                            .selectable_label(self.paper == *paper, paper.label())
                            .on_hover_text(crate::words::w().hint_paper)
                            .clicked()
                        {
                            self.paper = *paper;
                        }
                    }
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(!self.landscape, crate::words::w().portrait)
                        .on_hover_text(crate::words::w().hint_portrait)
                        .clicked()
                    {
                        self.landscape = false;
                    }
                    if ui
                        .selectable_label(self.landscape, crate::words::w().landscape)
                        .on_hover_text(crate::words::w().hint_landscape)
                        .clicked()
                    {
                        self.landscape = true;
                    }
                });
                ui.add_space(6.0);
                let shown = format!("{:.0} mm", self.margin_mm);
                crate::widgets::slider_row(ui, crate::words::w().margin, &mut self.margin_mm, 0.0..=50.0, &shown);
                ui.add_space(8.0);
                crate::widgets::caption(ui, crate::words::w().print_hint);
                ui.add_space(crate::widgets::FOOT);
                send = crate::widgets::primary(ui, crate::words::w().prepare, true)
                    .on_hover_text(crate::words::w().hint_prepare)
                    .clicked();
                ui.add_space(crate::skin::S2);
                if crate::widgets::secondary(ui, crate::words::w().cancel, true)
                    .on_hover_text(crate::words::w().hint_leave_as_is)
                    .clicked() {
                    self.tool = Tool::None;
                }
            });
            if shut {
                self.tool = Tool::None;
                self.entered(ctx);
            }
            if send {
                if let Some(picture) = &self.picture {
                    let sheet = crate::print::compose(
                        &picture.pixels,
                        self.paper,
                        self.landscape,
                        self.margin_mm,
                    );
                    match crate::print::write_pdf(&sheet, None)
                        .and_then(|path| crate::print::open_externally(&path).map(|_| path))
                    {
                        Ok(_) => {
                            self.trouble = None;
                            self.tool = Tool::None;
                        }
                        Err(exc) => {
                            self.trouble = Some(crate::words::fill(crate::words::w().could_not_print, &[&exc.to_string()]))
                        }
                    }
                }
            }
        }

        if self.tool == Tool::Save && self.picture.is_some() {
            let mut write = false;
            let mut shut = false;
            egui::SidePanel::left("salva")
                .frame(crate::skin::panel_frame(ctx))
                .resizable(false)
                .exact_width(crate::widgets::PANEL_WIDTH).show(ctx, |ui| {
                if crate::widgets::panel_title(ui, crate::words::w().save_as) {
                    shut = true;
                }
                for format in crate::save::Format::ALL {
                    if ui
                        .selectable_label(self.format == *format, format.label())
                        .on_hover_text(crate::words::w().hint_format)
                        .clicked()
                    {
                        self.format = *format;
                        self.weighed = None;
                    }
                }
                if self.format.lossy() {
                    ui.add_space(crate::skin::S2);
                    let shown = format!("{}", self.quality);
                    if crate::widgets::slider_row(ui, crate::words::w().quality, &mut self.quality, 1..=100, &shown).changed()
                    {
                        self.weighed = None;
                    }
                }
                if !self.format.keeps_transparency() {
                    ui.add_space(4.0);
                    crate::widgets::caption(ui, crate::words::w().transparency_to_white);
                }

                // Encoded to be weighed, not estimated: the number shown is the
                // number that will be on disk.
                if self.weighed.map(|(f, q, _)| (f, q)) != Some((self.format, self.quality)) {
                    if let Some(picture) = &self.picture {
                        if let Ok(bytes) =
                            crate::save::encode(&picture.pixels, self.format, self.quality)
                        {
                            self.weighed = Some((self.format, self.quality, bytes.len()));
                        }
                    }
                }
                ui.add_space(8.0);
                if let Some((_, _, size)) = self.weighed {
                    ui.label(crate::save::readable(size));
                }
                ui.add_space(crate::widgets::FOOT);
                write = crate::widgets::primary(ui, crate::words::w().save, true)
                    .on_hover_text(crate::words::w().hint_save_as)
                    .clicked();
                ui.add_space(crate::skin::S2);
                if crate::widgets::secondary(ui, crate::words::w().cancel, true)
                    .on_hover_text(crate::words::w().hint_leave_as_is)
                    .clicked() {
                    self.tool = Tool::None;
                }
            });
            if shut {
                self.tool = Tool::None;
                self.entered(ctx);
            }
            if write {
                self.save_now();
            }
        }

        if self.tool == Tool::Cutout && self.picture.is_some() {
            let mut apply = false;
            let mut cancel = false;
            let mut restate = false;
            let mut shut = false;
            egui::SidePanel::left("sfondo")
                .frame(crate::skin::panel_frame(ctx))
                .resizable(false)
                .exact_width(crate::widgets::PANEL_WIDTH).show(ctx, |ui| {
                if crate::widgets::panel_title(ui, crate::words::w().cutout) {
                    shut = true;
                }
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(24.0), Sense::hover());
                    ui.painter().rect_filled(
                        rect,
                        3.0,
                        Color32::from_rgb(
                            self.keying.colour[0],
                            self.keying.colour[1],
                            self.keying.colour[2],
                        ),
                    );
                    if ui
                        .selectable_label(self.picking_colour, crate::words::w().dropper)
                        .on_hover_text(crate::words::w().dropper_hint)
                        .clicked()
                    {
                        self.picking_colour = !self.picking_colour;
                    }
                });
                ui.add_space(crate::skin::S2);
                let shown = format!("{:.2}", self.keying.tolerance);
                if crate::widgets::slider_row(ui, crate::words::w().tolerance, &mut self.keying.tolerance, 0.0..=1.0, &shown)
                    .changed()
                {
                    restate = true;
                }
                let shown = format!("{:.2}", self.keying.softness);
                if crate::widgets::slider_row(ui, crate::words::w().softness, &mut self.keying.softness, 0.0..=0.5, &shown)
                    .on_hover_text(crate::words::w().softness_hint)
                    .changed()
                {
                    restate = true;
                }
                ui.add_space(crate::widgets::FOOT);
                apply = crate::widgets::primary(ui, crate::words::w().apply, true)
                    .on_hover_text(crate::words::w().hint_cutout_apply)
                    .clicked();
                ui.add_space(crate::skin::S2);
                cancel = crate::widgets::secondary(ui, crate::words::w().reset, true)
                    .on_hover_text(crate::words::w().hint_reset)
                    .clicked();
            });
            if shut {
                self.tool = Tool::None;
                self.entered(ctx);
            }
            if restate {
                if let Some(picture) = self.picture.as_mut() {
                    let how = self.keying;
                    picture.preview_cutout(&how);
                }
            }
            if apply {
                if let Some(picture) = self.picture.as_mut() {
                    let how = self.keying;
                    picture.apply_cutout(&how);
                }
                self.tool = Tool::None;
                self.picking_colour = false;
            }
            if cancel {
                // Puts the keying back where it started and stays: inside a
                // tool, cancelling is undoing what was set up, not leaving.
                // Closing on it took the panel away from somebody who only
                // wanted to start over, and the way out is the cross.
                self.keying = crate::cutout::Keying::default();
                self.picking_colour = true;
                if let Some(picture) = self.picture.as_mut() {
                    picture.forget_preview();
                }
            }
        }

        if self.tool == Tool::Resize && self.picture.is_some() {
            let mut apply = false;
            let mut shut = false;
            egui::SidePanel::left("ridimensiona")
                .frame(crate::skin::panel_frame(ctx))
                .resizable(false)
                .exact_width(crate::widgets::PANEL_WIDTH).show(ctx, |ui| {
                let picture = self.picture.as_ref().expect("checked above");
                let (was_w, was_h) = (picture.width(), picture.height());
                let ratio = was_w as f32 / was_h.max(1) as f32;
                if crate::widgets::panel_title(ui, crate::words::w().resize) {
                    shut = true;
                }
                crate::widgets::caption(
                    ui,
                    &crate::words::fill(
                        crate::words::w().from_size,
                        &[&was_w.to_string(), &was_h.to_string()],
                    ),
                );
                ui.add_space(6.0);

                let mut width = self.wanted.0;
                let mut height = self.wanted.1;
                let touched_w = ui
                    .add(egui::DragValue::new(&mut width).range(1..=20000).prefix("larghezza "))
                    .changed();
                let touched_h = ui
                    .add(egui::DragValue::new(&mut height).range(1..=20000).prefix("altezza "))
                    .changed();
                ui.checkbox(&mut self.proportional, crate::words::w().keep_proportions)
                    .on_hover_text(crate::words::w().hint_proportional);
                if self.proportional {
                    // Whichever was touched leads, so typing a width does not
                    // fight with the height being corrected under the cursor.
                    if touched_w {
                        height = ((width as f32 / ratio).round() as u32).max(1);
                    } else if touched_h {
                        width = ((height as f32 * ratio).round() as u32).max(1);
                    }
                }
                self.wanted = (width, height);

                ui.add_space(crate::skin::S1);
                ui.horizontal(|ui| {
                    for part in [0.25_f32, 0.5, 0.75] {
                        if ui
                            .button(format!("{}%", (part * 100.0) as u32))
                            .on_hover_text(crate::words::w().hint_percent)
                            .clicked()
                        {
                            self.wanted = (
                                ((was_w as f32 * part).round() as u32).max(1),
                                ((was_h as f32 * part).round() as u32).max(1),
                            );
                        }
                    }
                });
                ui.add_space(crate::widgets::FOOT);
                apply = crate::widgets::primary(ui, crate::words::w().apply, self.wanted != (was_w, was_h))
                    .on_hover_text(crate::words::w().hint_resize_apply)
                    .clicked();
                ui.add_space(crate::skin::S2);
                if crate::widgets::secondary(ui, crate::words::w().cancel, true)
                    .on_hover_text(crate::words::w().hint_leave_as_is)
                    .clicked() {
                    self.tool = Tool::None;
                }
            });
            if shut {
                self.tool = Tool::None;
                self.entered(ctx);
            }
            if apply {
                if let Some(picture) = self.picture.as_mut() {
                    picture.resize_to(self.wanted.0, self.wanted.1);
                }
                self.tool = Tool::None;
            }
        }

        if self.tool == Tool::Markup && self.picture.is_some() {
            let mut apply = false;
            let mut shut = false;
            egui::SidePanel::left("annota")
                .frame(crate::skin::panel_frame(ctx))
                .resizable(false)
                .exact_width(crate::widgets::PANEL_WIDTH).show(ctx, |ui| {
                shut = self.markup.panel(ui);
                ui.add_space(crate::widgets::FOOT);
                apply = crate::widgets::primary(ui, crate::words::w().apply, !self.markup.shapes.is_empty())
                    .on_hover_text(crate::words::w().apply_marks_hint)
                    .clicked();
            });
            if apply {
                if let Some(picture) = self.picture.as_mut() {
                    picture.stamp(&self.markup.shapes);
                }
                self.markup.clear();
                self.tool = Tool::None;
            }
            if shut {
                self.tool = Tool::None;
                self.entered(ctx);
            }
        }

        if self.tool == Tool::Adjust && self.picture.is_some() {
            let mut shut = false;
            egui::SidePanel::left("regola")
                .frame(crate::skin::panel_frame(ctx))
                .resizable(false)
                .exact_width(crate::widgets::PANEL_WIDTH).show(ctx, |ui| {
                let picture = self.picture.as_mut().expect("checked above");
                if crate::widgets::panel_title(ui, crate::words::w().adjust) {
                    shut = true;
                }
                let before = picture.adjustments;
                let mut how = picture.adjustments;
                for (label, value, range) in [
                    (crate::words::w().brightness, &mut how.brightness, 0.0..=2.0),
                    (crate::words::w().contrast, &mut how.contrast, 0.0..=2.0),
                    (crate::words::w().gamma, &mut how.gamma, 0.1..=3.0),
                ] {
                    let shown = format!("{:.2}", *value);
                    crate::widgets::slider_row(ui, label, value, range, &shown);
                }
                let shown = format!("{:.2}", how.saturation);
                ui.add_enabled_ui(!how.monochrome, |ui| {
                    crate::widgets::slider_row(ui, crate::words::w().saturation, &mut how.saturation, 0.0..=2.0, &shown);
                });
                ui.checkbox(&mut how.monochrome, crate::words::w().monochrome)
                    .on_hover_text(crate::words::w().hint_monochrome);
                ui.add_space(crate::widgets::FOOT);
                if crate::widgets::secondary(ui, crate::words::w().reset, how != crate::adjust::Adjustments::default())
                    .on_hover_text(crate::words::w().hint_reset)
                    .clicked()
                {
                    how = crate::adjust::Adjustments::default();
                }
                if how != before {
                    picture.adjustments = how;
                    // The whole picture, at full resolution, on every change:
                    // the cost is measured and shown below rather than assumed,
                    // and if it ever stops being small the status line says so.
                    self.adjust_ms = picture.adjust_cost_ms();
                }
                ui.add_space(6.0);
                crate::widgets::caption(ui, &crate::words::fill(crate::words::w().recompute, &[&self.adjust_ms.to_string()]));
            });
            if shut {
                self.tool = Tool::None;
                self.entered(ctx);
            }
        }

        if self.tool == Tool::Ai && self.picture.is_some() {
            let mut arrived = None;
            egui::SidePanel::left("ai")
                .frame(crate::skin::panel_frame(ctx))
                .resizable(false)
                .exact_width(crate::widgets::WIDE_PANEL_WIDTH).show(ctx, |ui| {
                // Two disjoint fields of self: the panel needs the pixels to
                // send and its own state to draw, and neither is the other.
                let source = &self.picture.as_ref().expect("checked above").pixels;
                arrived = self.studio.panel(ui, source);
                ui.add_space(crate::widgets::FOOT);
                if crate::widgets::secondary(ui, crate::words::w().close, !self.studio.busy())
                    .on_hover_text(crate::words::w().hint_close_panel)
                    .clicked() {
                    self.tool = Tool::None;
                }
            });
            if let Some(pixels) = arrived {
                if let Some(picture) = self.picture.as_mut() {
                    picture.replace_with(pixels);
                }
                // A model answers at its own size, so the view has to start over.
                self.zoom = 1.0;
                self.pan = Vec2::ZERO;
                self.selection = Selection::whole();
            }
        }

        egui::TopBottomPanel::bottom("stato")
            .frame(crate::skin::chrome_frame(ctx, false))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    match &self.picture {
                        Some(picture) => {
                            ui.label(picture.name.clone());
                            crate::widgets::number(
                                ui,
                                &format!(
                                    "·  {} × {} px  ·  {:.0}%",
                                    picture.width(),
                                    picture.height(),
                                    self.zoom * 100.0
                                ),
                            );
                        }
                        None => {
                            ui.label(crate::words::w().nothing_open);
                        }
                    }
                    // Both of these belong at the right edge, so they share one
                    // layout: two right-to-left blocks each start from the edge
                    // again, and the second draws on top of the first. Measured
                    // on the window, "opened in 103 ms" and "EN" were printed
                    // over each other.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Light, dark, or whatever Windows is doing. It lived in
                        // the WebView2 build's status bar from the beginning and
                        // was simply not carried over: the setting was read at
                        // startup and honoured, and there was no way to change
                        // it from inside the program.
                        //
                        // One button rather than three, cycling, because the
                        // whole bar is one line high and the language next to it
                        // is a cycling button too.
                        let theme = crate::settings::read().theme;
                        let (figure, next) = match theme {
                            crate::settings::Theme::System => {
                                (crate::skin::Icon::Follow, crate::settings::Theme::Light)
                            }
                            crate::settings::Theme::Light => {
                                (crate::skin::Icon::Sun, crate::settings::Theme::Dark)
                            }
                            crate::settings::Theme::Dark => {
                                (crate::skin::Icon::Moon, crate::settings::Theme::System)
                            }
                        };
                        let named = |which: crate::settings::Theme| match which {
                            crate::settings::Theme::System => crate::words::w().theme_system,
                            crate::settings::Theme::Light => crate::words::w().theme_light,
                            crate::settings::Theme::Dark => crate::words::w().theme_dark,
                        };
                        let (spot, click) =
                            ui.allocate_exact_size(egui::Vec2::splat(18.0), Sense::click());
                        let lit = if click.hovered() {
                            crate::skin::tokens(ctx).ink
                        } else {
                            crate::skin::tokens(ctx).ink_faint
                        };
                        crate::skin::icon(ui.painter(), spot, figure, lit);
                        if click
                            .on_hover_text(crate::words::fill(
                                crate::words::w().hint_theme,
                                &[&named(next).to_lowercase()],
                            ))
                            .clicked()
                        {
                            crate::skin::wear(ctx, next);
                            let mut kept = crate::settings::read();
                            kept.theme = next;
                            let _ = crate::settings::write(kept);
                        }

                        // The way to change language, where a person looks for
                        // it when the program is in the wrong one. It takes
                        // effect on the next frame: nothing has to be restarted.
                        let now = crate::words::chosen();
                        let other = match now {
                            crate::words::Lang::It => crate::words::Lang::En,
                            crate::words::Lang::En => crate::words::Lang::It,
                        };
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(now.code().to_uppercase())
                                        .text_style(crate::skin::caption_style())
                                        .color(crate::skin::tokens(ctx).ink_faint),
                                )
                                .fill(egui::Color32::TRANSPARENT),
                            )
                            .on_hover_text(other.name())
                            .clicked()
                        {
                            crate::words::speak(other);
                            let mut kept = crate::settings::read();
                            kept.language = other;
                            // A preference that is not written down is not a
                            // preference, it is this run's mood.
                            let _ = crate::settings::write(kept);
                        }

                        // Something that went right, said for a moment and then
                        // gone. It takes the same slot as the opening figure,
                        // and while it is there the figure waits: two things
                        // fading out of one corner at once is one too many.
                        let mut said = false;
                        if let Some((notice, when)) = self.notice.clone() {
                            let young = when.elapsed().as_secs_f32() < 8.0;
                            if young {
                                ctx.request_repaint_after(std::time::Duration::from_secs(8));
                            }
                            let alpha = ui.ctx().animate_bool_with_time_and_easing(
                                egui::Id::new("esito"),
                                young,
                                0.4,
                                egui::emath::easing::quadratic_out,
                            );
                            if alpha > 0.0 {
                                crate::widgets::fading_caption(ui, &notice, alpha);
                                said = true;
                            } else {
                                self.notice = None;
                            }
                        }

                        // How long the window took, and then not any more. It is
                        // a measurement worth seeing once, not a thing to keep
                        // saying.
                        if let (Some(ms), false) = (self.opened_in_ms, said) {
                            let young = self.opened_at.elapsed().as_secs_f32() < 4.0;
                            if young {
                                ctx.request_repaint_after(std::time::Duration::from_secs(4));
                            }
                            let alpha = ui.ctx().animate_bool_with_time_and_easing(
                                egui::Id::new("vanto"),
                                young,
                                0.4,
                                egui::emath::easing::quadratic_out,
                            );
                            if alpha > 0.0 {
                                crate::widgets::fading_caption(
                                    ui,
                                    &crate::words::fill(
                                        crate::words::w().opened_in,
                                        &[&ms.to_string()],
                                    ),
                                    alpha,
                                );
                            }
                        }
                    });
                });
            });

        // An error gets its own strip rather than the status bar's one slot.
        // Sharing it meant that whenever something went wrong the window also
        // stopped saying which picture was open.
        if let Some(said) = self.trouble.clone() {
            let tokens = crate::skin::tokens(ctx);
            egui::TopBottomPanel::bottom("errore")
                .frame(
                    egui::Frame::none()
                        .fill(tokens.danger.gamma_multiply(0.16))
                        .inner_margin(egui::Margin::symmetric(crate::skin::S4, crate::skin::S2)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.colored_label(tokens.danger, &said);
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui
                                    .add(
                                        egui::Button::new("\u{00D7}")
                                            .fill(egui::Color32::TRANSPARENT),
                                    )
                                    .on_hover_text(crate::words::w().dismiss)
                                    .clicked()
                                {
                                    self.trouble = None;
                                }
                            },
                        );
                    });
                });
        }

        self.keep_shot(ctx);

        // Its own frame, not the panel fill: the ground under a photograph is a
        // photometric reference and does not belong to a theme. This is the one
        // line that separates the chrome from the stage.
        let mut asked_open = false;
        let stage = egui::Frame::none().fill(crate::skin::STAGE);
        egui::CentralPanel::default().frame(stage).show(ctx, |ui| {
            if self.picture.is_none() {
                let dragging = ctx.input(|i| !i.raw.hovered_files.is_empty());
                if let crate::empty::Wanted::Open = crate::empty::stage(ui, dragging) {
                    asked_open = true;
                }
                return;
            }
            let picture = self.picture.as_mut().expect("checked above");
            let room = ui.available_rect_before_wrap();
            // Clicks as well as drags. With Sense::drag() alone egui never
            // reports a click at all, so every click on the picture was
            // silently dropped: the colour dropper picked nothing, and text and
            // number marks could not be put down. Nothing about the panels said
            // so, because the panels were right - only the stage was deaf.
            let (response, painter) =
                ui.allocate_painter(room.size(), Sense::click_and_drag());

            // Zoom on the wheel, around the pointer rather than the centre: the
            // thing under the cursor is the thing being looked at.
            if response.hovered() {
                let scroll = ui.input(|i| i.raw_scroll_delta.y);
                if scroll != 0.0 {
                    let before = self.zoom;
                    self.zoom = (self.zoom * (scroll * 0.0012).exp()).clamp(0.1, 16.0);
                    if let Some(at) = response.hover_pos() {
                        let middle = room.center() + self.pan;
                        self.pan += (at - middle) * (1.0 - self.zoom / before);
                    }
                }
            }
            // Dragging with any button pans, which on a picture is what dragging
            // means until there is a tool that says otherwise.
            // While a tool is out, dragging belongs to the tool rather than to
            // the view: panning is still there on the middle button.
            let panning = self.tool == Tool::None
                || ui.input(|i| i.pointer.middle_down());
            if response.dragged() && panning {
                self.pan += response.drag_delta();
            }

            let size = Vec2::new(picture.width() as f32, picture.height() as f32);
            // The fit is decided before the texture is asked for, because the
            // filter depends on how large the picture ends up being drawn.
            // Fit on first sight, so a large photograph is not opened at 100%
            // with only its top-left corner visible.
            let fit = (room.width() / size.x).min(room.height() / size.y).min(1.0);
            let shown = size * fit * self.zoom;
            let middle = room.center() + self.pan;
            let where_at = Rect::from_center_size(middle, shown);

            // Under the picture, in order: the shadow that lifts it off the
            // ground, then the chequer where the picture has holes in it.
            crate::widgets::picture_shadow(&painter, where_at);
            if picture.see_through() {
                crate::widgets::chequer(&painter, where_at);
            }
            let texture = picture.texture(ctx, fit * self.zoom > 1.0);
            painter.image(
                texture.id(),
                where_at,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                // White is not a colour choice here: it is the neutral tint,
                // which is to say the picture is drawn as it is.
                Color32::WHITE,
            );
            crate::widgets::picture_edge(&painter, where_at);
            crate::widgets::picture_marks(&painter, where_at, self.tool == Tool::Crop);

            if self.tool == Tool::Cutout && self.picking_colour {
                ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
                if response.clicked() {
                    if let Some(at) = response.interact_pointer_pos() {
                        let x = (at.x - where_at.left()) / where_at.width().max(1.0);
                        let y = (at.y - where_at.top()) / where_at.height().max(1.0);
                        // Read from the original rather than from what is on
                        // screen: the preview already has holes in it, and
                        // sampling one would pick the colour of nothing.
                        self.keying.colour = picture.sample_original(x, y);
                        self.picking_colour = false;
                        let how = self.keying;
                        picture.preview_cutout(&how);
                    }
                }
            }

            if self.tool == Tool::Markup {
                let size = (picture.width(), picture.height());
                self.markup.on_stage(&painter, &response, where_at, size);
            }

            if self.tool == Tool::Crop {
                let box_on_screen = self.selection.on_screen(where_at);

                // Everything outside the blade, dimmed: four rectangles rather
                // than one with a hole, because a hole is not a rectangle.
                let veil = crate::skin::VEIL;
                let whole = where_at;
                for outside in [
                    Rect::from_min_max(whole.left_top(), Pos2::new(whole.right(), box_on_screen.top())),
                    Rect::from_min_max(Pos2::new(whole.left(), box_on_screen.bottom()), whole.right_bottom()),
                    Rect::from_min_max(Pos2::new(whole.left(), box_on_screen.top()), box_on_screen.left_bottom()),
                    Rect::from_min_max(box_on_screen.right_top(), Pos2::new(whole.right(), box_on_screen.bottom())),
                ] {
                    if outside.is_positive() {
                        painter.rect_filled(outside, 0.0, veil);
                    }
                }

                let mark = crate::skin::MARK;
                painter.rect_stroke(box_on_screen, 0.0, Stroke::new(1.5_f32, mark));
                // The rule of thirds, which is what the guides are for.
                for step in 1..3 {
                    let part = step as f32 / 3.0;
                    let x = box_on_screen.left() + box_on_screen.width() * part;
                    let y = box_on_screen.top() + box_on_screen.height() * part;
                    let faint = Stroke::new(1.0_f32, crate::skin::GUIDE);
                    painter.line_segment(
                        [Pos2::new(x, box_on_screen.top()), Pos2::new(x, box_on_screen.bottom())],
                        faint,
                    );
                    painter.line_segment(
                        [Pos2::new(box_on_screen.left(), y), Pos2::new(box_on_screen.right(), y)],
                        faint,
                    );
                }
                // Corners heavier than the sides, the way the agent's overlay
                // draws them: the eye reads a frame without an outline being
                // painted over the photograph.
                let arm = 16.0_f32.min(box_on_screen.width() / 3.0).min(box_on_screen.height() / 3.0);
                let heavy = Stroke::new(3.0_f32, mark);
                for (corner, dx, dy) in [
                    (box_on_screen.left_top(), 1.0, 1.0),
                    (box_on_screen.right_top(), -1.0, 1.0),
                    (box_on_screen.left_bottom(), 1.0, -1.0),
                    (box_on_screen.right_bottom(), -1.0, -1.0),
                ] {
                    painter.line_segment([corner, corner + Vec2::new(arm * dx, 0.0)], heavy);
                    painter.line_segment([corner, corner + Vec2::new(0.0, arm * dy)], heavy);
                }

                if let Some(pointer) = response.hover_pos() {
                    let over = Grip::at(pointer, box_on_screen);
                    if over != Grip::None {
                        ctx.set_cursor_icon(over.cursor());
                    }
                    if response.drag_started() {
                        self.holding = over;
                    }
                }
                if response.dragged() && self.holding != Grip::None {
                    // The drag arrives in points on screen; the selection lives
                    // in fractions, so it is converted rather than mixed.
                    let by = Vec2::new(
                        response.drag_delta().x / where_at.width().max(1.0),
                        response.drag_delta().y / where_at.height().max(1.0),
                    );
                    self.selection = self.holding.drag(self.selection, by);
                }
                if response.drag_stopped() {
                    self.holding = Grip::None;
                }
            }
        });

        // Outside the panel, because the dialog it opens is modal and blocks
        // the thread: opening it from inside the closure would hold the stage
        // borrowed while Windows waits for somebody to pick a file.
        if asked_open {
            self.open_dialog();
        }
    }
}
