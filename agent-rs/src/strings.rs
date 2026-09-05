// The few sentences the agent says, in the language the editor uses.
//
// The editor's choice lives in settings.json; when nobody has chosen, Windows'
// display language decides, the same rule the editor follows. Read once per
// start: the agent is restarted by the editor whenever it matters.

use std::sync::OnceLock;

use windows_sys::Win32::Globalization::GetUserDefaultUILanguage;

use crate::paths;

pub fn italian() -> bool {
    static ITALIAN: OnceLock<bool> = OnceLock::new();
    *ITALIAN.get_or_init(detect)
}

fn detect() -> bool {
    if let Some(json) = paths::read_line(&paths::settings()) {
        // Small enough not to want a JSON parser: the file is written by this
        // project and holds a handful of flat keys.
        if let Some(found) = language_in(&json) {
            return found == "it";
        }
    }
    // The primary language id is the low ten bits; 0x10 is Italian.
    (unsafe { GetUserDefaultUILanguage() } & 0x3FF) == 0x10
}

fn language_in(json: &str) -> Option<String> {
    let at = json.find("\"language\"")?;
    let rest = &json[at + "\"language\"".len()..];
    let colon = rest.find(':')?;
    let after = &rest[colon + 1..];
    let open = after.find('"')?;
    let tail = &after[open + 1..];
    let close = tail.find('"')?;
    Some(tail[..close].to_string())
}

/// The English sentence, or its Italian counterpart when that is the language.
pub fn t(english: &str) -> String {
    if !italian() {
        return english.to_string();
    }
    for (en, it) in TABLE {
        if *en == english {
            return it.to_string();
        }
    }
    english.to_string()
}

/// `t` with `{0}` and `{1}` filled in, the two-argument case being all this needs.
pub fn f1(english: &str, a: &str) -> String {
    t(english).replace("{0}", a)
}

pub fn f2(english: &str, a: &str, b: &str) -> String {
    t(english).replace("{0}", a).replace("{1}", b)
}

pub fn ctrl() -> &'static str {
    if italian() { "Ctrl+Stamp" } else { "Ctrl+PrtSc" }
}

pub fn altgr() -> &'static str {
    if italian() { "AltGr+Stamp" } else { "AltGr+PrtSc" }
}

const TABLE: &[(&str, &str)] = &[
    ("Cut a piece of the screen", "Ritaglia lo schermo"),
    ("About Cutaway", "Informazioni su Cutaway"),
    ("Keeps the Cutaway shortcut working.", "Tiene attiva la scorciatoia di Cutaway."),
    (
        "Press {0} anywhere to freeze the screen and cut a piece out of it. The piece goes to the clipboard and opens in Cutaway.",
        "Premi {0} da qualsiasi programma per congelare lo schermo e ritagliarne un pezzo. Il pezzo va negli appunti e si apre in Cutaway.",
    ),
    ("Version {0}", "Versione {0}"),
    ("Created by {0}", "Creato da {0}"),
    ("Open Cutaway", "Apri Cutaway"),
    ("Where Cutaway is...", "Dove si trova Cutaway..."),
    ("Where Cutaway is", "Dove si trova Cutaway"),
    ("Programs", "Programmi"),
    ("Cutaway will be opened from {0}.", "Cutaway verr\u{00e0} aperto da {0}."),
    (
        "The folder you unpacked is still where you put it: delete {0} when you like.",
        "La cartella che hai scompattato \u{e8} ancora dove l'hai messa: cancella {0} quando vuoi.",
    ),
    ("Source code", "Il codice sorgente"),
    ("The author's site", "Il sito dell'autore"),
    ("Licensed under the {0}", "Distribuito con licenza {0}"),
    ("Start with Windows", "Avvia con Windows"),
    ("Quit", "Esci"),
    ("Cutaway is here", "Cutaway \u{e8} qui"),
    (
        "Press Ctrl+PrtSc or AltGr+PrtSc to cut a piece of the screen. Quit from this icon's menu.",
        "Premi Ctrl+Stamp (PrtSc) o AltGr+Stamp per ritagliare lo schermo. Esci dal menu di questa icona.",
    ),
    ("Shortcut taken", "Scorciatoia occupata"),
    (
        "{0} is already used by another program; {1} still works.",
        "{0} \u{e8} gi\u{e0} usata da un altro programma; {1} funziona lo stesso.",
    ),
    (
        "Both shortcuts are already used by other programs.",
        "Entrambe le scorciatoie sono gi\u{e0} usate da altri programmi.",
    ),
    ("Cutaway not found", "Cutaway non trovato"),
    (
        "The editor is not where the agent expected it. Start Cutaway once and try again.",
        "L'editor non \u{e8} dove l'agente se lo aspettava. Avvia Cutaway una volta e riprova.",
    ),
    (
        "The editor closed before showing the capture.",
        "L'editor si \u{e8} chiuso prima di mostrare il ritaglio.",
    ),
    ("The editor did not open the capture.", "L'editor non ha aperto il ritaglio."),
    (
        "The screen could not be read just now. Try again.",
        "Non sono riuscito a leggere lo schermo. Riprova.",
    ),
    (
        "Drag a rectangle over what you want",
        "Traccia un rettangolo su quello che ti serve",
    ),
    (
        "it opens in Cutaway when you let go \u{b7}",
        "si apre in Cutaway quando lasci \u{b7}",
    ),
    ("to leave the screen alone", "per lasciare stare lo schermo"),
    ("{0}: taken by another program", "{0}: occupata da un altro programma"),
    ("Cutaway\u{2026}", "Cutaway\u{2026}"),
    (
        "There is nothing on the screen to cut: it came back black.",
        "Non c'\u{e8} niente da ritagliare sullo schermo: \u{e8} tornato tutto nero.",
    ),
    (
        "Something went wrong; the shortcut may have stopped working.",
        "Qualcosa \u{e8} andato storto; la scorciatoia potrebbe non funzionare pi\u{f9}.",
    ),
    ("Remove Cutaway from this computer", "Rimuovi Cutaway da questo computer"),
    (
        "This removes the shortcut, the background agent and the folder Cutaway keeps in your user profile. The picture files you saved are not touched.",
        "Toglie la scorciatoia, l'agente in secondo piano e la cartella che Cutaway tiene nel tuo profilo. Le immagini che hai salvato non si toccano.",
    ),
    (
        "Cutaway has been removed from this computer.",
        "Cutaway \u{e8} stato rimosso da questo computer.",
    ),
    (
        "The last capture is still on its way to the editor. Try again in a moment.",
        "Il ritaglio di prima sta ancora arrivando all'editor. Riprova fra un attimo.",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Every sentence this program shows, and its Italian.
    ///
    /// The table is looked up by the English text itself, so a sentence that is
    /// not in it falls through and appears in English - silently, on an Italian
    /// machine, with nothing failing. That is exactly what happened to the whole
    /// About dialog: five sentences, none of them listed, and it took a
    /// screenshot to notice. These are the ones that have to be there.
    #[test]
    fn the_sentences_the_program_shows_are_all_translated() {
        let shown = [
            "About Cutaway",
            "Keeps the Cutaway shortcut working.",
            "Press {0} anywhere to freeze the screen and cut a piece out of it. The piece goes to the clipboard and opens in Cutaway.",
            "Version {0}",
            "Created by {0}",
            "Source code",
            "The author's site",
            "Licensed under the {0}",
            "Where Cutaway is...",
            "Where Cutaway is",
            "Programs",
            "Cutaway will be opened from {0}.",
            "Cut a piece of the screen",
            "Open Cutaway",
            "Start with Windows",
            "Quit",
            "Remove Cutaway from this computer",
        ];
        for english in shown {
            let found = TABLE.iter().find(|(en, _)| *en == english);
            assert!(found.is_some(), "senza traduzione: {}", english);
            let (_, italian) = found.unwrap();
            assert_ne!(*italian, english, "traduzione uguale all'inglese: {}", english);
        }
    }

    /// A sentence with a place in it has to keep it in both languages.
    #[test]
    fn a_translation_never_drops_a_placeholder() {
        for (english, italian) in TABLE {
            for place in ["{0}", "{1}"] {
                assert_eq!(
                    english.contains(place),
                    italian.contains(place),
                    "{}: {} sta solo in una delle due",
                    english,
                    place
                );
            }
        }
    }

    /// Nothing is listed twice: the first entry wins and the second is dead.
    #[test]
    fn nothing_is_in_the_table_twice() {
        let mut seen: Vec<&str> = TABLE.iter().map(|(en, _)| *en).collect();
        seen.sort_unstable();
        let mut unique = seen.clone();
        unique.dedup();
        assert_eq!(seen.len(), unique.len(), "una frase compare due volte");
    }
}
