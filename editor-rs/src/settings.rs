// What the program remembers between runs, which is not much.
//
// The same file the 1.6 build reads and writes, in the same folder and the same
// shape: somebody who set this program to English there opens an English window
// here without being asked again. Two settings, each with a fixed set of values
// so a bad file cannot put the program into a state it has no code for.
//
// In the clear, unlike the API keys next door: there is nothing here worth
// encrypting, and a preference nobody can read is a preference nobody can fix.

use std::path::PathBuf;

use crate::words::Lang;

/// Which way the window is painted, when the choice is not to follow Windows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    System,
    Light,
    Dark,
}

impl Theme {
    fn code(self) -> &'static str {
        match self {
            Theme::System => "system",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }

    fn from_code(code: &str) -> Option<Theme> {
        match code {
            "system" => Some(Theme::System),
            "light" => Some(Theme::Light),
            "dark" => Some(Theme::Dark),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Settings {
    pub theme: Theme,
    pub language: Lang,
}

/// The display language Windows is set to, if this program speaks it.
///
/// The display language, not the locale: the locale decides how a date is
/// written, this decides what the buttons say, and a machine can have one of
/// each.
pub fn windows_language() -> Lang {
    /// The primary language identifier for Italian.
    const ITALIAN: u16 = 0x10;
    let id = unsafe {
        windows_sys::Win32::Globalization::GetUserDefaultUILanguage()
    };
    if id & 0x3FF == ITALIAN {
        Lang::It
    } else {
        Lang::En
    }
}

impl Default for Settings {
    /// What each setting is before anybody has chosen.
    ///
    /// The language starts from the machine rather than from a constant: an
    /// Italian Windows should open an Italian window without being asked, and
    /// anything else gets the language the program is written in.
    fn default() -> Settings {
        Settings { theme: Theme::System, language: windows_language() }
    }
}

pub fn store_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("Cutaway").join("settings.json")
}

/// Everything, with the defaults filled in for whatever is missing.
///
/// A file that cannot be read means nobody has chosen anything yet. It is a
/// preference, not a document: failing to start over it would be absurd.
/// Reads one of the store files, whatever wrote it.
///
/// The whatever is the point. These four files are shared with the WebView2
/// build and sit in a folder people open: a settings.json saved from Notepad
/// comes back with a byte order mark in front of it, and serde stops at the
/// first character. What follows is worse than an error, because there is no
/// error: the file parses as nothing, every setting falls back to its default,
/// and the theme and language a person chose are quietly forgotten. Found by
/// taking screenshots - a script wrote the file from Windows PowerShell, whose
/// UTF8 has a mark, and the light theme never appeared.
pub fn read_json(path: &std::path::Path) -> Option<serde_json::Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

pub fn read() -> Settings {
    let mut settings = Settings::default();
    let Some(stored) = read_json(&store_path()) else {
        return settings;
    };
    if let Some(theme) = stored["theme"].as_str().and_then(Theme::from_code) {
        settings.theme = theme;
    }
    if let Some(language) = stored["language"].as_str().and_then(Lang::from_code) {
        settings.language = language;
    }
    settings
}

/// Stores the lot, keeping any other key the file already had.
///
/// The other build may know settings this one does not, and dropping them
/// because this program has no field for them would be a program deciding what
/// another program is allowed to remember.
pub fn write(settings: Settings) -> Result<(), String> {
    let mut stored = match read_json(&store_path())
    {
        Some(serde_json::Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    };
    stored.insert("theme".into(), settings.theme.code().into());
    stored.insert("language".into(), settings.language.code().into());

    let path = store_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|exc| exc.to_string())?;
    }
    let text = serde_json::to_string_pretty(&serde_json::Value::Object(stored))
        .map_err(|exc| exc.to_string())?;
    std::fs::write(&path, text).map_err(|exc| exc.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_survives_the_round_trip() {
        for theme in [Theme::System, Theme::Light, Theme::Dark] {
            assert_eq!(Theme::from_code(theme.code()), Some(theme));
        }
        for language in [Lang::It, Lang::En] {
            assert_eq!(Lang::from_code(language.code()), Some(language));
        }
    }

    #[test]
    fn a_byte_order_mark_does_not_erase_the_settings() {
        // It did. serde stops at the first character, the file parses as
        // nothing, and every setting silently falls back to its default - so a
        // settings.json saved from Notepad forgets the theme and the language
        // the person chose, with nothing on screen to say why.
        let folder = std::env::temp_dir().join(format!("cutaway-bom-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let file = folder.join("settings.json");
        let body = r#"{"language": "en", "theme": "light"}"#;
        for text in [body.to_string(), format!("\u{feff}{}", body)] {
            std::fs::write(&file, &text).unwrap();
            let read = read_json(&file).expect("il file non si e' letto");
            assert_eq!(read["theme"].as_str(), Some("light"));
            assert_eq!(read["language"].as_str(), Some("en"));
        }
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_value_this_program_does_not_know_is_ignored() {
        // The other build might one day write a theme this one has no code for.
        // Falling back to the default is right; panicking is not.
        assert_eq!(Theme::from_code("solarized"), None);
        assert_eq!(Lang::from_code(""), None);
    }

    #[test]
    fn the_machine_decides_before_anybody_has_chosen() {
        // Whatever this Windows is set to, it has to be one of the two.
        let language = windows_language();
        assert!(matches!(language, Lang::It | Lang::En));
        assert_eq!(Settings::default().theme, Theme::System);
    }
}
