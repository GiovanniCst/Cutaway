// What the window says, in the two languages it speaks.
//
// A struct with one field per string rather than a map with keys, for one
// reason worth more than the extra typing: a translation that is missing is a
// compilation error, not a blank that somebody finds on screen a month later.
// It also costs nothing at run time - no lookup, no allocation, no hashing.
//
// The language is read once from the settings, which are the same settings the
// 1.6 build writes: a person who chose English there opens an English window
// here without choosing again.
//
// Where a sentence carries values, the placeholders stay inside the sentence
// and `fill` puts them in. Splitting a sentence into fragments to be
// concatenated would fix the word order to Italian's, and the order is exactly
// the part that differs.

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    It,
    En,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::It => "it",
            Lang::En => "en",
        }
    }

    pub fn from_code(code: &str) -> Option<Lang> {
        match code {
            "it" => Some(Lang::It),
            "en" => Some(Lang::En),
            _ => None,
        }
    }

    /// What it calls itself, which is how a language is always listed.
    pub fn name(self) -> &'static str {
        match self {
            Lang::It => "Italiano",
            Lang::En => "English",
        }
    }
}

static CHOSEN: AtomicU8 = AtomicU8::new(0);

pub fn speak(lang: Lang) {
    CHOSEN.store(if lang == Lang::It { 1 } else { 0 }, Ordering::Relaxed);
}

pub fn chosen() -> Lang {
    if CHOSEN.load(Ordering::Relaxed) == 1 {
        Lang::It
    } else {
        Lang::En
    }
}

/// The words, in whichever language is being spoken.
pub fn w() -> &'static Words {
    match chosen() {
        Lang::It => &IT,
        Lang::En => &EN,
    }
}

/// One of something, or several: the plural form with the number in it, or
/// the singular form which spells the one out.
///
/// Italian and English agree on this rule and many languages do not, which is
/// why it is a function rather than an `if` at each call site: the day a third
/// language arrives, there is one place to argue with.
pub fn count(how_many: usize, one: &str, many: &str) -> String {
    if how_many == 1 {
        one.to_string()
    } else {
        fill(many, &[&how_many.to_string()])
    }
}

/// Puts values into a sentence, one per `{}`, in the order they appear.
///
/// `format!` cannot be used: it wants its template at compile time, and the
/// whole point here is that the template is chosen at run time.
pub fn fill(template: &str, values: &[&str]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    for value in values {
        match rest.find("{}") {
            Some(at) => {
                out.push_str(&rest[..at]);
                out.push_str(value);
                rest = &rest[at + 2..];
            }
            // More values than places for them. Better to lose the extra than
            // to lose the sentence.
            None => break,
        }
    }
    out.push_str(rest);
    out
}

pub struct Words {
    // --- the toolbar ---------------------------------------------------------
    pub open: &'static str,
    pub paste: &'static str,
    pub copy: &'static str,
    pub email: &'static str,
    pub email_hint: &'static str,
    /// The toolbar capture, which is not the rail one: this takes a piece out
    /// of the screen, that cuts down the picture already open. They were both
    /// called Ritaglia for a moment, which is one name too few.
    pub cut_from_screen: &'static str,
    pub cut_from_screen_hint: &'static str,
    pub no_agent: &'static str,
    pub long_capture: &'static str,
    pub long_capture_hint: &'static str,
    pub long_capture_title: &'static str,
    pub long_capture_body: &'static str,
    pub long_capture_escape: &'static str,
    pub long_capture_go: &'static str,
    pub long_capture_cancelled: &'static str,
    pub long_capture_capped: &'static str,
    pub long_capture_aim: &'static str,
    pub long_capture_starting: &'static str,
    pub long_capture_working: &'static str,
    pub long_capture_done: &'static str,
    pub long_capture_stop: &'static str,
    pub undo: &'static str,
    pub zoom_in: &'static str,
    pub zoom_out: &'static str,
    pub actual_size: &'static str,

    // --- the rail ------------------------------------------------------------
    pub crop: &'static str,
    pub adjust: &'static str,
    pub cutout: &'static str,
    pub ai: &'static str,
    pub resize: &'static str,
    pub markup: &'static str,
    /// The rail entry and the panel title. Called OCR because that is what it
    /// is, and because "Testo" was also the name of an annotation tool: one
    /// word for two things is one word too few.
    pub ocr: &'static str,
    pub ocr_hint: &'static str,
    pub ocr_read: &'static str,
    pub ocr_nothing_yet: &'static str,
    pub text: &'static str,
    pub print: &'static str,
    pub save: &'static str,
    pub save_as: &'static str,

    // --- what every panel has ------------------------------------------------
    pub apply: &'static str,
    pub cancel: &'static str,
    pub close: &'static str,
    pub reset: &'static str,

    // --- crop ----------------------------------------------------------------
    pub whole_picture: &'static str,
    pub proportions: &'static str,
    pub from_size: &'static str,

    // --- adjust --------------------------------------------------------------
    pub brightness: &'static str,
    pub contrast: &'static str,
    pub gamma: &'static str,
    pub saturation: &'static str,
    pub monochrome: &'static str,
    pub recompute: &'static str,

    // --- background ----------------------------------------------------------
    pub dropper: &'static str,
    pub dropper_hint: &'static str,
    pub tolerance: &'static str,
    pub softness: &'static str,
    pub softness_hint: &'static str,

    // --- resize --------------------------------------------------------------
    pub keep_proportions: &'static str,

    // --- markup --------------------------------------------------------------
    pub tool: &'static str,
    pub colour: &'static str,
    /// In the same order as `skin::PALETTE`.
    pub colour_names: [&'static str; 8],
    pub rect: &'static str,
    pub ellipse: &'static str,
    pub arrow: &'static str,
    pub line: &'static str,
    pub number: &'static str,
    pub highlight: &'static str,
    pub eraser: &'static str,
    pub stroke: &'static str,
    pub body_size: &'static str,
    pub numbering: &'static str,
    pub next_number: &'static str,
    pub from_one: &'static str,
    pub numbering_hint: &'static str,
    pub delete: &'static str,
    pub clear_all: &'static str,
    pub marks_count: &'static str,
    pub one_mark: &'static str,
    pub apply_marks_hint: &'static str,

    // --- text (OCR) ----------------------------------------------------------
    pub text_from_picture: &'static str,
    pub reading_text: &'static str,
    pub lines_count: &'static str,
    pub one_line: &'static str,
    pub already_copied: &'static str,
    pub clipboard_was_busy: &'static str,
    pub copy_again: &'static str,
    pub no_text_found: &'static str,
    pub could_not_read: &'static str,

    // --- print ---------------------------------------------------------------
    pub sheet: &'static str,
    pub portrait: &'static str,
    pub landscape: &'static str,
    pub margin: &'static str,
    pub print_hint: &'static str,
    pub prepare: &'static str,

    // --- save ----------------------------------------------------------------
    pub quality: &'static str,
    pub transparency_to_white: &'static str,

    // --- the AI panel --------------------------------------------------------
    pub ai_title: &'static str,
    pub key_for: &'static str,
    pub key_stored: &'static str,
    pub key_stays_here: &'static str,
    pub change: &'static str,
    pub forget: &'static str,
    pub read_catalogue: &'static str,
    pub reading_catalogue: &'static str,
    pub catalogue_unreachable: &'static str,
    pub search_other_models: &'static str,
    pub no_model_by_that_name: &'static str,
    pub board_read_on: &'static str,
    pub no_board_for_openai: &'static str,
    pub refresh: &'static str,
    pub refresh_hint: &'static str,
    pub refreshing: &'static str,
    pub refreshed: &'static str,
    pub what_to_change: &'static str,
    pub prompt_hint: &'static str,
    pub answer_size: &'static str,
    pub original_size: &'static str,
    pub stays_same_size: &'static str,
    pub comes_back_bigger: &'static str,
    pub enlarge: &'static str,
    pub enlarge_hint: &'static str,
    pub model_working: &'static str,
    pub answered_at: &'static str,
    pub size_refused: &'static str,
    pub tier_top: &'static str,
    pub tier_value: &'static str,
    pub tier_unweighed: &'static str,
    pub no_comparable_price: &'static str,
    pub rank_position: &'static str,

    // --- the status bar ------------------------------------------------------
    pub nothing_open: &'static str,
    pub how_strong: &'static str,
    pub highlighter_names: [&'static str; 5],
    pub theme_light: &'static str,
    pub theme_dark: &'static str,
    pub theme_system: &'static str,
    pub hint_theme: &'static str,
    pub about_title: &'static str,
    pub about_tagline: &'static str,
    pub about_summary: &'static str,
    pub about_created_by: &'static str,
    pub about_author_site: &'static str,
    pub about_project: &'static str,
    pub about_licence_before: &'static str,
    pub about_licence_after: &'static str,
    // A word for every button, so nothing is a mystery on hover.
    pub hint_open: &'static str,
    pub hint_paste: &'static str,
    pub hint_copy: &'static str,
    pub hint_undo: &'static str,
    pub hint_wordmark: &'static str,
    pub hint_rail_crop: &'static str,
    pub hint_rail_resize: &'static str,
    pub hint_rail_ai: &'static str,
    pub hint_rail_cutout: &'static str,
    pub hint_rail_markup: &'static str,
    pub hint_rail_adjust: &'static str,
    pub hint_rail_print: &'static str,
    pub hint_rail_save: &'static str,
    pub hint_ratio: &'static str,
    pub hint_whole: &'static str,
    pub hint_crop_apply: &'static str,
    pub hint_leave_as_is: &'static str,
    pub hint_copy_again: &'static str,
    pub hint_close_panel: &'static str,
    pub hint_paper: &'static str,
    pub hint_portrait: &'static str,
    pub hint_landscape: &'static str,
    pub hint_prepare: &'static str,
    pub hint_format: &'static str,
    pub hint_save_as: &'static str,
    pub hint_cutout_apply: &'static str,
    pub hint_reset: &'static str,
    pub hint_proportional: &'static str,
    pub hint_percent: &'static str,
    pub hint_resize_apply: &'static str,
    pub hint_monochrome: &'static str,
    pub hint_kind: &'static str,
    pub hint_from_one: &'static str,
    pub hint_delete: &'static str,
    pub hint_clear_all: &'static str,
    pub hint_provider: &'static str,
    pub hint_answer_size: &'static str,
    pub hint_ai_apply: &'static str,
    pub hint_change_key: &'static str,
    pub hint_forget_key: &'static str,
    pub hint_save_key: &'static str,
    pub hint_read_catalogue: &'static str,
    pub hint_long_go: &'static str,
    pub drop_here: &'static str,
    pub release_to_open: &'static str,
    pub or_else: &'static str,
    pub to_pick_one: &'static str,
    pub to_paste_one: &'static str,
    pub to_cut_one: &'static str,
    /// What the screenshot key is called on this keyboard. An Italian keyboard
    /// has `Stamp` written on it and no key called `PrtSc`, and a shortcut
    /// spelled with a name the key does not have is worse than no shortcut.
    pub print_screen_key: &'static str,
    pub opened_in: &'static str,
    pub no_picture: &'static str,
    pub dismiss: &'static str,

    // --- what can go wrong ---------------------------------------------------
    pub clipboard_busy: &'static str,
    pub clipboard_has_no_picture: &'static str,
    pub no_window_under_pointer: &'static str,
    pub could_not_photograph: &'static str,
    pub no_mail_program: &'static str,
    pub mail_answered: &'static str,
    pub no_ocr_engine: &'static str,
    pub windows_error: &'static str,
    pub could_not_save: &'static str,
    pub could_not_read_text: &'static str,
    pub could_not_attach: &'static str,
    pub invalid_text: &'static str,
    pub could_not_print: &'static str,
    pub clipboard_name: &'static str,
    /// The colophon on the composition the program opens with. Two lines: what
    /// it is, then which one it is.
    pub mondrian_after: &'static str,
    pub mondrian_id: &'static str,
    pub sample_name: &'static str,
    pub pictures: &'static str,
    pub long_capture_name: &'static str,
    pub no_key_stored: &'static str,
    pub write_what_to_change: &'static str,
    pub provider_returned_nothing: &'static str,
    pub picture_unreadable: &'static str,
    pub key_not_accepted: &'static str,
    pub not_enough_credit: &'static str,
    pub too_many_requests: &'static str,
    pub provider_answered: &'static str,
    pub could_not_reach_provider: &'static str,
    pub could_not_encrypt: &'static str,
    pub nothing_reachable: &'static str,
    pub could_not_read_board: &'static str,
    pub could_not_read_catalogue: &'static str,
    pub unknown_provider: &'static str,
    pub undecodable: &'static str,
}

pub const IT: Words = Words {
    open: "Apri",
    paste: "Incolla",
    copy: "Copia",
    email: "Email",
    email_hint: "Apre il tuo programma di posta col ritaglio allegato",
    cut_from_screen: "Screenshot",
    cut_from_screen_hint: "Congela lo schermo e ne ritaglia un pezzo. Anche da qualunque altro programma, con Ctrl+Stamp",
    no_agent: "Non trovo CutawayAgent.exe accanto al programma",
    long_capture: "Cattura lunga",
    long_capture_hint:
        "Nasconde questa finestra, poi fotografa scorrendo cio che sta sotto il cursore",
    long_capture_title: "Cattura lunga",
    long_capture_body: "Cutaway passa in secondo piano. Clicca la finestra da catturare: la porto davanti io e la cattura parte da sola.",
    long_capture_escape: "Premi Esc per annullare \u{2014} adesso, oppure mentre la cattura \u{00e8} in corso.",
    long_capture_go: "Ho capito, vai",
    long_capture_cancelled: "Cattura lunga annullata",
    long_capture_capped: "Fermata a {} px: la pagina continuava",
    long_capture_aim: "Clicca la finestra da catturare",
    long_capture_starting: "Comincio fra {} s",
    long_capture_working: "Scorro e ricucio",
    long_capture_done: "{} schermate ricucite, {} px di altezza",
    long_capture_stop: "Lascia stare",
    undo: "Annulla",
    zoom_in: "Ingrandisci la vista",
    zoom_out: "Rimpicciolisci la vista",
    actual_size: "Torna a dimensione reale",

    crop: "Ritaglia",
    adjust: "Regola",
    cutout: "Scontorna",
    ai: "Modifica AI",
    resize: "Ridimensiona",
    markup: "Annota",
    ocr: "OCR",
    ocr_hint: "Riconosce automaticamente il testo nell'immagine",
    ocr_read: "Rileva testo",
    ocr_nothing_yet: "Premi Rileva testo: leggo l'immagine col motore di Windows e copio quello che trovo",
    text: "Testo",
    print: "Stampa",
    save: "Salva",
    save_as: "Salva come",

    apply: "Applica",
    cancel: "Annulla",
    close: "Chiudi",
    reset: "Azzera",

    whole_picture: "Tutta",
    proportions: "Proporzioni",
    from_size: "da {} \u{00d7} {}",

    brightness: "Luminosità",
    contrast: "Contrasto",
    gamma: "Gamma",
    saturation: "Saturazione",
    monochrome: "Bianco e nero",
    recompute: "ricalcolo: {} ms",

    dropper: "Contagocce",
    dropper_hint: "Poi clicca il colore da togliere",
    tolerance: "Tolleranza",
    softness: "Sfumatura",
    softness_hint: "Senza, i bordi antialiasati restano orlati",

    keep_proportions: "Mantieni le proporzioni",

    tool: "Strumento",
    colour: "Colore",
    colour_names: ["Rosso", "Arancio", "Giallo", "Verde", "Blu", "Viola", "Nero", "Bianco"],
    rect: "Rettangolo",
    ellipse: "Ellisse",
    arrow: "Freccia",
    line: "Linea",
    number: "Numero",
    highlight: "Evidenziatore",
    eraser: "Gomma",
    stroke: "Spessore",
    body_size: "Corpo",
    numbering: "Numeratore",
    next_number: "Il prossimo sarà",
    from_one: "Riparti da 1",
    numbering_hint: "Ogni clic sull'immagine posa un numero, e il contatore avanza da solo",
    delete: "Elimina",
    clear_all: "Togli tutto",
    marks_count: "{} segni",
    one_mark: "1 segno",
    apply_marks_hint: "Scrive i segni nei pixel: dopo non si spostano più",

    text_from_picture: "Letto dall'immagine col motore di Windows, e già copiato",
    reading_text: "Leggo l'immagine",
    lines_count: "{} righe",
    one_line: "1 riga",
    already_copied: "Già negli appunti",
    clipboard_was_busy: "Gli appunti erano occupati",
    copy_again: "Copia di nuovo",
    no_text_found: "Nessun testo trovato in questa immagine",
    could_not_read: "Non sono riuscito a leggerla",

    sheet: "Foglio",
    portrait: "In piedi",
    landscape: "Sdraiato",
    margin: "Margine",
    print_hint: "Si apre nel tuo lettore PDF, che conosce già le stampanti",
    prepare: "Prepara",

    quality: "Qualità",
    transparency_to_white: "La trasparenza va su bianco",

    ai_title: "Intelligenza artificiale",
    key_for: "Chiave {}",
    key_stored: "Chiave salvata e cifrata su questo account",
    key_stays_here: "Resta su questo computer, cifrata col tuo account Windows.",
    change: "Cambia",
    forget: "Rimuovi",
    read_catalogue: "Leggi il catalogo",
    reading_catalogue: "Leggo il catalogo",
    catalogue_unreachable: "Catalogo non raggiungibile: l'elenco qui sotto non è verificato.",
    search_other_models: "cerca fra gli altri modelli",
    no_model_by_that_name: "Nessun modello con questo nome",
    board_read_on: "Classifica LMArena, letta il {}",
    no_board_for_openai: "Nessuna classifica copre l'API diretta di OpenAI",
    refresh: "Aggiorna",
    refresh_hint: "Rilegge le classifiche e ricalcola l'elenco",
    refreshing: "Rileggo le classifiche",
    refreshed: "{} modelli in classifica, {} raggiungibili: elenco aggiornato al {}",
    what_to_change: "Che cosa vuoi cambiare",
    prompt_hint: "togli lo sfondo e mettine uno bianco",
    answer_size: "Dimensione della risposta",
    original_size: "Dimensione originale",
    stays_same_size: "L'immagine torna delle dimensioni che aveva",
    comes_back_bigger: "L'immagine torna più grande di com'è partita",
    enlarge: "Ingrandisci",
    enlarge_hint: "Ridisegna l'immagine più grande: inventa dettaglio che nel file non c'era",
    model_working: "Il modello sta lavorando. La finestra resta viva.",
    answered_at: "risposta {}\u{00d7}{}",
    size_refused: "il modello ha rifiutato la dimensione richiesta",
    tier_top: "Migliori",
    tier_value: "Convenienti",
    tier_unweighed: "Altri",
    no_comparable_price: "prezzo non confrontabile",
    rank_position: "{}\u{00ba} in classifica",

    nothing_open: "Apri un'immagine per cominciare \u{2014} gli strumenti si svegliano con lei",
    how_strong: "Intensit\u{00e0}",
    highlighter_names: ["Giallo", "Verde", "Azzurro", "Rosa", "Arancio"],
    theme_light: "Tema chiaro",
    theme_dark: "Tema scuro",
    theme_system: "Segui Windows",
    hint_theme: "Tema della finestra: passa a {}",
    about_title: "Informazioni su Cutaway",
    about_tagline: "Tutto quello che serve a un'immagine prima di mandarla.",
    about_summary: "Converti, ritaglia, scontorna, annota e regola, poi salva, stampa o manda \u{2028}quello che ne esce. C'\u{00e8} anche l'AI, su una chiave che porti tu \u{2014} che \u{00e8} anche dove arriva il conto.",
    about_created_by: "Creato da {}",
    about_author_site: "costantini.pw",
    about_project: "Il progetto su GitHub",
    about_licence_before: "Distribuito sotto ",
    about_licence_after: ". Puoi usarlo, modificarlo e ridistribuirlo, purch\u{00e9} resti l'attribuzione originale.",
    hint_open: "Apre un'immagine dal disco",
    hint_paste: "Incolla l'immagine che sta negli appunti",
    hint_copy: "Copia l'immagine negli appunti",
    hint_undo: "Torna indietro di un passo",
    hint_wordmark: "Chi ha fatto Cutaway, e con che licenza",
    hint_rail_crop: "Taglia via i bordi dell'immagine",
    hint_rail_resize: "Cambia le dimensioni in pixel",
    hint_rail_ai: "Chiede a un modello di modificare o ingrandire l'immagine",
    hint_rail_cutout: "Rende trasparente lo sfondo",
    hint_rail_markup: "Disegna sopra l'immagine",
    hint_rail_adjust: "Luminosita, contrasto, gamma, saturazione",
    hint_rail_print: "Prepara un PDF da stampare",
    hint_rail_save: "Scrive l'immagine su un file",
    hint_ratio: "Porta il ritaglio a questa proporzione",
    hint_whole: "Riporta il ritaglio attorno a tutta l'immagine",
    hint_crop_apply: "Taglia via tutto quello che sta fuori dal riquadro",
    hint_leave_as_is: "Lascia l'immagine com'è",
    hint_copy_again: "Rimette il testo negli appunti",
    hint_close_panel: "Chiude il pannello",
    hint_paper: "Formato del foglio",
    hint_portrait: "Foglio in verticale",
    hint_landscape: "Foglio in orizzontale",
    hint_prepare: "Scrive un PDF e lo apre nel tuo lettore",
    hint_format: "In che formato scrivere il file",
    hint_save_as: "Scegli dove scrivere il file",
    hint_cutout_apply: "Rende trasparenti i pixel di questo colore",
    hint_reset: "Riporta i valori a com'erano",
    hint_proportional: "Cambiando un lato cambia anche l'altro",
    hint_percent: "Porta l'immagine a questa frazione della sua misura",
    hint_resize_apply: "Ricampiona l'immagine alla misura scelta",
    hint_monochrome: "Toglie il colore",
    hint_kind: "Che segno disegnare",
    hint_from_one: "Riporta il contatore a 1",
    hint_delete: "Toglie il segno selezionato",
    hint_clear_all: "Toglie tutti i segni",
    hint_provider: "Quale servizio usare",
    hint_answer_size: "Quanto grande deve tornare l'immagine",
    hint_ai_apply: "Manda l'immagine al modello con la tua richiesta",
    hint_change_key: "Sostituisci la chiave salvata",
    hint_forget_key: "Cancella la chiave da questo computer",
    hint_save_key: "Cifra la chiave e la salva su questo account Windows",
    hint_read_catalogue: "Chiede al provider quali modelli puoi usare",
    hint_long_go: "Nasconde Cutaway e aspetta che tu clicchi una finestra",
    drop_here: "Trascina qui un'immagine",
    release_to_open: "Lascia per aprire",
    or_else: "oppure",
    to_pick_one: "per sceglierne una",
    to_paste_one: "per incollarne una dagli appunti",
    to_cut_one: "per ritagliare un pezzo di schermo",
    print_screen_key: "Stamp",
    opened_in: "aperto in {} ms",
    no_picture: "nessuna immagine",
    dismiss: "Chiudi l'avviso",

    clipboard_busy: "Gli appunti sono occupati da un altro programma",
    clipboard_has_no_picture: "Negli appunti non c'è un'immagine",
    no_window_under_pointer: "Nessuna finestra sotto il cursore",
    could_not_photograph: "Non sono riuscito a fotografare lo schermo",
    no_mail_program: "Non c'è un programma di posta che Windows sappia aprire da qui",
    mail_answered: "il programma di posta ha risposto {}",
    no_ocr_engine: "Nessun motore OCR per le lingue installate su questo Windows",
    windows_error: "errore di Windows 0x{}",
    could_not_save: "Non sono riuscito a salvare: {}",
    could_not_read_text: "Non sono riuscito a leggere il testo: {}",
    could_not_attach: "Non sono riuscito a preparare l'allegato: {}",
    invalid_text: "testo non valido",
    could_not_print: "Non sono riuscito a stampare: {}",
    clipboard_name: "Appunti.png",
    mondrian_after: "Ispirata all'opera di Piet Mondrian, generata proceduralmente",
    mondrian_id: "{}  ·  Cutaway {}  ·  g.j.c.",
    sample_name: "Composizione.png",
    pictures: "Immagini",
    long_capture_name: "Cattura lunga {}.png",
    no_key_stored: "Non c'è nessuna chiave salvata per questo provider.",
    write_what_to_change: "Scrivi che cosa vuoi cambiare.",
    provider_returned_nothing: "Il provider non ha restituito nessuna immagine.",
    picture_unreadable: "L'immagine ricevuta è illeggibile.",
    key_not_accepted: "La chiave non è stata accettata.",
    not_enough_credit: "Il credito dell'account non basta per questa richiesta.",
    too_many_requests: "Troppe richieste: riprova fra poco.",
    provider_answered: "Il provider ha risposto {}.",
    could_not_reach_provider: "Non sono riuscito a raggiungere il provider: {}",
    could_not_encrypt: "Windows non ha potuto cifrare la chiave",
    nothing_reachable: "Nessun modello in classifica è raggiungibile con questa chiave.",
    could_not_read_board: "Non sono riuscito a leggere la classifica: {}",
    could_not_read_catalogue: "Non sono riuscito a leggere il catalogo: {}",
    unknown_provider: "Provider sconosciuto: {}",
    undecodable: "Immagine non decodificabile: {}",
};

pub const EN: Words = Words {
    open: "Open",
    paste: "Paste",
    copy: "Copy",
    email: "Email",
    email_hint: "Opens your mail program with the piece attached",
    cut_from_screen: "Screenshot",
    cut_from_screen_hint: "Freezes the screen and cuts a piece out of it. From any other program too, with Ctrl+PrtSc",
    no_agent: "I cannot find CutawayAgent.exe beside the program",
    long_capture: "Long capture",
    long_capture_hint:
        "Hides this window, then photographs whatever is under the pointer as it scrolls",
    long_capture_title: "Long capture",
    long_capture_body: "Cutaway steps into the background. Click the window to capture: I bring it to the front and the capture starts on its own.",
    long_capture_escape: "Press Esc to cancel \u{2014} now, or while the capture is running.",
    long_capture_go: "Got it, go",
    long_capture_cancelled: "Long capture cancelled",
    long_capture_capped: "Stopped at {} px: the page kept going",
    long_capture_aim: "Click the window to capture",
    long_capture_starting: "Starting in {} s",
    long_capture_working: "Scrolling and stitching",
    long_capture_done: "{} frames stitched, {} px tall",
    long_capture_stop: "Never mind",
    undo: "Undo",
    zoom_in: "Zoom in",
    zoom_out: "Zoom out",
    actual_size: "Back to life size",

    crop: "Crop",
    adjust: "Adjust",
    cutout: "Cutout",
    ai: "AI edit",
    resize: "Resize",
    markup: "Markup",
    ocr: "OCR",
    ocr_hint: "Recognises the text in the picture on its own",
    ocr_read: "Read the text",
    ocr_nothing_yet: "Press Read the text: I read the picture with the Windows engine and copy whatever is there",
    text: "Text",
    print: "Print",
    save: "Save",
    save_as: "Save as",

    apply: "Apply",
    cancel: "Cancel",
    close: "Close",
    reset: "Reset",

    whole_picture: "Whole",
    proportions: "Proportions",
    from_size: "from {} \u{00d7} {}",

    brightness: "Brightness",
    contrast: "Contrast",
    gamma: "Gamma",
    saturation: "Saturation",
    monochrome: "Black and white",
    recompute: "recompute: {} ms",

    dropper: "Dropper",
    dropper_hint: "Then click the colour to take out",
    tolerance: "Tolerance",
    softness: "Softness",
    softness_hint: "Without it, antialiased edges keep a fringe",

    keep_proportions: "Keep proportions",

    tool: "Tool",
    colour: "Colour",
    colour_names: ["Red", "Orange", "Yellow", "Green", "Blue", "Purple", "Black", "White"],
    rect: "Rectangle",
    ellipse: "Ellipse",
    arrow: "Arrow",
    line: "Line",
    number: "Number",
    highlight: "Highlighter",
    eraser: "Eraser",
    stroke: "Width",
    body_size: "Size",
    numbering: "Numbering",
    next_number: "The next one is",
    from_one: "Restart at 1",
    numbering_hint: "Each click on the picture drops a number, and the counter steps on by itself",
    delete: "Delete",
    clear_all: "Clear all",
    marks_count: "{} marks",
    one_mark: "1 mark",
    apply_marks_hint: "Writes the marks into the pixels: after this they do not move",

    text_from_picture: "Read from the picture by the Windows engine, and already copied",
    reading_text: "Reading the picture",
    lines_count: "{} lines",
    one_line: "1 line",
    already_copied: "Already on the clipboard",
    clipboard_was_busy: "The clipboard was busy",
    copy_again: "Copy again",
    no_text_found: "No text found in this picture",
    could_not_read: "I could not read it",

    sheet: "Sheet",
    portrait: "Portrait",
    landscape: "Landscape",
    margin: "Margin",
    print_hint: "It opens in your PDF reader, which already knows your printers",
    prepare: "Prepare",

    quality: "Quality",
    transparency_to_white: "Transparency becomes white",

    ai_title: "Artificial intelligence",
    key_for: "{} key",
    key_stored: "Key saved and encrypted for this account",
    key_stays_here: "It stays on this computer, encrypted with your Windows account.",
    change: "Change",
    forget: "Remove",
    read_catalogue: "Read the catalogue",
    reading_catalogue: "Reading the catalogue",
    catalogue_unreachable: "The catalogue is unreachable: the list below is unverified.",
    search_other_models: "search the other models",
    no_model_by_that_name: "No model by that name",
    board_read_on: "LMArena board, read on {}",
    no_board_for_openai: "No board covers OpenAI's direct API",
    refresh: "Refresh",
    refresh_hint: "Reads the boards again and recomputes the list",
    refreshing: "Reading the boards again",
    refreshed: "{} models on the board, {} reachable: list updated on {}",
    what_to_change: "What should change",
    prompt_hint: "remove the background and make it white",
    answer_size: "Size of the answer",
    original_size: "Original size",
    stays_same_size: "The picture comes back the size it went in",
    comes_back_bigger: "The picture comes back larger than it went in",
    enlarge: "Enlarge",
    enlarge_hint: "Redraws the picture larger: it invents detail that was never in the file",
    model_working: "The model is working. The window stays alive.",
    answered_at: "answered {}\u{00d7}{}",
    size_refused: "the model refused the size that was asked for",
    tier_top: "Best",
    tier_value: "Value",
    tier_unweighed: "Others",
    no_comparable_price: "no comparable price",
    rank_position: "ranked {}",

    nothing_open: "Open a picture to start \u{2014} the tools wake up with it",
    how_strong: "Strength",
    highlighter_names: ["Yellow", "Green", "Blue", "Pink", "Orange"],
    theme_light: "Light theme",
    theme_dark: "Dark theme",
    theme_system: "Follow Windows",
    hint_theme: "Window theme: switch to {}",
    about_title: "About Cutaway",
    about_tagline: "Everything a picture needs before you send it.",
    about_summary: "Convert, crop, key out a background, annotate and adjust, then save, print or send what comes out. There is AI in here too, on a key you bring yourself \u{2014} which is also where the bill lands.",
    about_created_by: "Created by {}",
    about_author_site: "costantini.pw",
    about_project: "The project on GitHub",
    about_licence_before: "Licensed under the ",
    about_licence_after: ". You may use, modify and redistribute it, provided the original attribution is kept.",
    hint_open: "Opens a picture from the disk",
    hint_paste: "Pastes whatever picture is on the clipboard",
    hint_copy: "Copies the picture to the clipboard",
    hint_undo: "Steps back one operation",
    hint_wordmark: "Who made Cutaway, and under what licence",
    hint_rail_crop: "Cuts the edges off the picture",
    hint_rail_resize: "Changes the size in pixels",
    hint_rail_ai: "Asks a model to change or enlarge the picture",
    hint_rail_cutout: "Makes the background transparent",
    hint_rail_markup: "Draws on top of the picture",
    hint_rail_adjust: "Brightness, contrast, gamma, saturation",
    hint_rail_print: "Prepares a PDF to print",
    hint_rail_save: "Writes the picture to a file",
    hint_ratio: "Sets the crop to this ratio",
    hint_whole: "Puts the crop back around the whole picture",
    hint_crop_apply: "Cuts away everything outside the box",
    hint_leave_as_is: "Leaves the picture as it is",
    hint_copy_again: "Puts the text back on the clipboard",
    hint_close_panel: "Closes the panel",
    hint_paper: "Sheet size",
    hint_portrait: "Sheet upright",
    hint_landscape: "Sheet on its side",
    hint_prepare: "Writes a PDF and opens it in your reader",
    hint_format: "Which format to write the file in",
    hint_save_as: "Choose where to write the file",
    hint_cutout_apply: "Makes the pixels of this colour transparent",
    hint_reset: "Puts the values back where they were",
    hint_proportional: "Changing one side changes the other",
    hint_percent: "Takes the picture to this fraction of its size",
    hint_resize_apply: "Resamples the picture to the chosen size",
    hint_monochrome: "Takes the colour out",
    hint_kind: "Which mark to draw",
    hint_from_one: "Puts the counter back to 1",
    hint_delete: "Removes the selected mark",
    hint_clear_all: "Removes every mark",
    hint_provider: "Which service to use",
    hint_answer_size: "How large the picture should come back",
    hint_ai_apply: "Sends the picture to the model with your request",
    hint_change_key: "Replace the stored key",
    hint_forget_key: "Deletes the key from this computer",
    hint_save_key: "Encrypts the key and stores it for this Windows account",
    hint_read_catalogue: "Asks the provider which models you can use",
    hint_long_go: "Hides Cutaway and waits for you to click a window",
    drop_here: "Drop a picture here",
    release_to_open: "Release to open",
    or_else: "or",
    to_pick_one: "to pick one",
    to_paste_one: "to paste one from the clipboard",
    to_cut_one: "to cut a piece out of the screen",
    print_screen_key: "PrtSc",
    opened_in: "opened in {} ms",
    no_picture: "no picture",
    dismiss: "Dismiss",

    clipboard_busy: "The clipboard is held by another program",
    clipboard_has_no_picture: "There is no picture on the clipboard",
    no_window_under_pointer: "No window under the pointer",
    could_not_photograph: "I could not photograph the screen",
    no_mail_program: "There is no mail program Windows can open from here",
    mail_answered: "the mail program answered {}",
    no_ocr_engine: "No OCR engine for the languages installed on this Windows",
    windows_error: "Windows error 0x{}",
    could_not_save: "I could not save it: {}",
    could_not_read_text: "I could not read the text: {}",
    could_not_attach: "I could not prepare the attachment: {}",
    invalid_text: "invalid text",
    could_not_print: "I could not print it: {}",
    clipboard_name: "Clipboard.png",
    mondrian_after: "After the work of Piet Mondrian, procedurally generated",
    mondrian_id: "{}  ·  Cutaway {}  ·  g.j.c.",
    sample_name: "Composition.png",
    pictures: "Pictures",
    long_capture_name: "Long capture {}.png",
    no_key_stored: "There is no key stored for this provider.",
    write_what_to_change: "Write what should change.",
    provider_returned_nothing: "The provider returned no picture.",
    picture_unreadable: "The picture that came back is unreadable.",
    key_not_accepted: "The key was not accepted.",
    not_enough_credit: "The account has not enough credit for this request.",
    too_many_requests: "Too many requests: try again shortly.",
    provider_answered: "The provider answered {}.",
    could_not_reach_provider: "I could not reach the provider: {}",
    could_not_encrypt: "Windows could not encrypt the key",
    nothing_reachable: "Nothing on the board is reachable with this key.",
    could_not_read_board: "I could not read the board: {}",
    could_not_read_catalogue: "I could not read the catalogue: {}",
    unknown_provider: "Unknown provider: {}",
    undecodable: "Undecodable picture: {}",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_go_where_the_sentence_puts_them() {
        assert_eq!(fill("da {} \u{00d7} {}", &["800", "600"]), "da 800 \u{00d7} 600");
        assert_eq!(fill("{} righe", &["12"]), "12 righe");
        // The order belongs to the sentence, not to the caller: this is the
        // whole reason the placeholders stay inside it.
        assert_eq!(fill("{} of {}", &["a", "b"]), "a of b");
    }

    #[test]
    fn a_sentence_survives_the_wrong_number_of_values() {
        // Fewer values than places: what is left stands rather than panicking.
        assert_eq!(fill("{} e {}", &["uno"]), "uno e {}");
        // More values than places: the extra is dropped, the sentence is not.
        assert_eq!(fill("solo {}", &["uno", "due"]), "solo uno");
        assert_eq!(fill("niente", &[]), "niente");
    }

    #[test]
    fn every_sentence_has_the_same_places_in_both_languages() {
        // A translation that loses a placeholder loses a number the person
        // needed; one that gains a place shows "{}" on screen. Both are caught
        // here rather than by somebody reading the window.
        let pairs: &[(&str, &str, &str)] = &[
            ("from_size", IT.from_size, EN.from_size),
            ("recompute", IT.recompute, EN.recompute),
            ("marks_count", IT.marks_count, EN.marks_count),
            ("lines_count", IT.lines_count, EN.lines_count),
            ("key_for", IT.key_for, EN.key_for),
            ("board_read_on", IT.board_read_on, EN.board_read_on),
            ("refreshed", IT.refreshed, EN.refreshed),
            ("answered_at", IT.answered_at, EN.answered_at),
            ("rank_position", IT.rank_position, EN.rank_position),
            ("opened_in", IT.opened_in, EN.opened_in),
            ("hint_theme", IT.hint_theme, EN.hint_theme),
            ("mail_answered", IT.mail_answered, EN.mail_answered),
            ("windows_error", IT.windows_error, EN.windows_error),
            ("could_not_save", IT.could_not_save, EN.could_not_save),
            ("provider_answered", IT.provider_answered, EN.provider_answered),
            (
                "could_not_reach_provider",
                IT.could_not_reach_provider,
                EN.could_not_reach_provider,
            ),
            ("could_not_read_board", IT.could_not_read_board, EN.could_not_read_board),
            (
                "could_not_read_catalogue",
                IT.could_not_read_catalogue,
                EN.could_not_read_catalogue,
            ),
            ("unknown_provider", IT.unknown_provider, EN.unknown_provider),
            ("undecodable", IT.undecodable, EN.undecodable),
        ];
        for (name, it, en) in pairs {
            assert_eq!(
                it.matches("{}").count(),
                en.matches("{}").count(),
                "{}: \"{}\" contro \"{}\"",
                name,
                it,
                en
            );
        }
    }

    #[test]
    fn the_palette_is_named_the_whole_way_through() {
        // The names and the colours are two lists kept in step by hand, and a
        // list that falls out of step names one colour after another.
        assert_eq!(IT.colour_names.len(), crate::skin::PALETTE.len());
        assert_eq!(EN.colour_names.len(), crate::skin::PALETTE.len());
        assert!(IT.colour_names.iter().all(|name| !name.is_empty()));
        assert!(EN.colour_names.iter().all(|name| !name.is_empty()));
    }

    #[test]
    fn one_of_something_is_not_plural() {
        for words in [&IT, &EN] {
            let one = count(1, words.one_line, words.lines_count);
            assert!(!one.contains("{}"), "{}", one);
            assert!(one.starts_with('1'), "{}", one);
            let several = count(12, words.one_line, words.lines_count);
            assert!(several.starts_with("12"), "{}", several);
        }
        // Nought is plural in both: "0 righe", "0 lines".
        assert_eq!(count(0, IT.one_line, IT.lines_count), "0 righe");
    }

    /// Every hint has to say something, in both languages, and not be the same
    /// string in both - a copied line is a translation nobody wrote.
    #[test]
    fn every_hint_is_written_in_both_languages() {
        let pairs: &[(&str, &str, &str)] = &[
            ("hint_open", IT.hint_open, EN.hint_open),
            ("hint_paste", IT.hint_paste, EN.hint_paste),
            ("hint_copy", IT.hint_copy, EN.hint_copy),
            ("hint_undo", IT.hint_undo, EN.hint_undo),
            ("hint_wordmark", IT.hint_wordmark, EN.hint_wordmark),
            ("hint_rail_crop", IT.hint_rail_crop, EN.hint_rail_crop),
            ("hint_rail_resize", IT.hint_rail_resize, EN.hint_rail_resize),
            ("hint_rail_ai", IT.hint_rail_ai, EN.hint_rail_ai),
            ("hint_rail_cutout", IT.hint_rail_cutout, EN.hint_rail_cutout),
            ("hint_rail_markup", IT.hint_rail_markup, EN.hint_rail_markup),
            ("hint_rail_adjust", IT.hint_rail_adjust, EN.hint_rail_adjust),
            ("hint_rail_print", IT.hint_rail_print, EN.hint_rail_print),
            ("hint_rail_save", IT.hint_rail_save, EN.hint_rail_save),
            ("hint_ratio", IT.hint_ratio, EN.hint_ratio),
            ("hint_whole", IT.hint_whole, EN.hint_whole),
            ("hint_crop_apply", IT.hint_crop_apply, EN.hint_crop_apply),
            ("hint_leave_as_is", IT.hint_leave_as_is, EN.hint_leave_as_is),
            ("hint_copy_again", IT.hint_copy_again, EN.hint_copy_again),
            ("hint_close_panel", IT.hint_close_panel, EN.hint_close_panel),
            ("hint_paper", IT.hint_paper, EN.hint_paper),
            ("hint_portrait", IT.hint_portrait, EN.hint_portrait),
            ("hint_landscape", IT.hint_landscape, EN.hint_landscape),
            ("hint_prepare", IT.hint_prepare, EN.hint_prepare),
            ("hint_format", IT.hint_format, EN.hint_format),
            ("hint_save_as", IT.hint_save_as, EN.hint_save_as),
            ("hint_cutout_apply", IT.hint_cutout_apply, EN.hint_cutout_apply),
            ("hint_reset", IT.hint_reset, EN.hint_reset),
            ("hint_proportional", IT.hint_proportional, EN.hint_proportional),
            ("hint_percent", IT.hint_percent, EN.hint_percent),
            ("hint_resize_apply", IT.hint_resize_apply, EN.hint_resize_apply),
            ("hint_monochrome", IT.hint_monochrome, EN.hint_monochrome),
            ("hint_kind", IT.hint_kind, EN.hint_kind),
            ("hint_from_one", IT.hint_from_one, EN.hint_from_one),
            ("hint_delete", IT.hint_delete, EN.hint_delete),
            ("hint_clear_all", IT.hint_clear_all, EN.hint_clear_all),
            ("hint_provider", IT.hint_provider, EN.hint_provider),
            ("hint_answer_size", IT.hint_answer_size, EN.hint_answer_size),
            ("hint_ai_apply", IT.hint_ai_apply, EN.hint_ai_apply),
            ("hint_change_key", IT.hint_change_key, EN.hint_change_key),
            ("hint_forget_key", IT.hint_forget_key, EN.hint_forget_key),
            ("hint_save_key", IT.hint_save_key, EN.hint_save_key),
            ("hint_read_catalogue", IT.hint_read_catalogue, EN.hint_read_catalogue),
            ("hint_long_go", IT.hint_long_go, EN.hint_long_go),
            ("ocr_hint", IT.ocr_hint, EN.ocr_hint),
            ("email_hint", IT.email_hint, EN.email_hint),
            ("dropper_hint", IT.dropper_hint, EN.dropper_hint),
            ("softness_hint", IT.softness_hint, EN.softness_hint),
        ];
        for (name, it, en) in pairs {
            assert!(it.len() > 8, "{}: italiano troppo corto", name);
            assert!(en.len() > 8, "{}: inglese troppo corto", name);
            assert_ne!(it, en, "{}: la stessa riga in due lingue", name);
            // A hint is a sentence, not a repetition of the button.
            assert!(!it.ends_with('.') || it.len() > 20, "{}", name);
        }
    }

    #[test]
    fn a_language_is_remembered_by_its_code() {
        assert_eq!(Lang::from_code("it"), Some(Lang::It));
        assert_eq!(Lang::from_code("en"), Some(Lang::En));
        assert_eq!(Lang::from_code("de"), None);
        assert_eq!(Lang::It.code(), "it");
    }
}
