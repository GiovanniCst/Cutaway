// Where the API key lives.
//
// The key belongs to the person, not to this program: it is theirs, it costs
// them money, and it must never leave this machine except to the provider it
// belongs to. So it is encrypted with DPAPI before it touches the disk.
//
// DPAPI ties the ciphertext to this Windows account. A file copied to another
// machine, or read by another user on this one, decrypts to nothing - which is
// the property that matters, because a key sitting in plain text in a JSON file
// under the profile is one careless backup away from being somebody else's.

use std::path::PathBuf;

use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB,
};

/// Extra entropy mixed into the encryption, so a blob lifted from another
/// program cannot be decrypted here.
///
/// Its value only has to stay the same, and it is the same one the 1.6 build
/// uses: a key stored there is read here without the person typing it again,
/// and a key stored here stays readable there. Changing this string makes every
/// key already stored impossible to decrypt, on both sides.
const ENTROPY: &[u8] = b"tiny-graphics/api-keys/v1";

/// No prompt, ever. This runs while a panel is being drawn.
const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;

fn blob(bytes: &[u8]) -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB { cbData: bytes.len() as u32, pbData: bytes.as_ptr() as *mut u8 }
}

/// Where the store sits: beside everything else this program keeps.
pub fn store_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("Cutaway").join("keys.json")
}

fn encrypt(plaintext: &str) -> Option<String> {
    unsafe {
        let mut input = blob(plaintext.as_bytes());
        let mut entropy = blob(ENTROPY);
        let mut output: CRYPT_INTEGER_BLOB = std::mem::zeroed();
        let ok = CryptProtectData(
            &mut input,
            std::ptr::null(),
            &mut entropy,
            std::ptr::null_mut(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        );
        if ok == 0 {
            return None;
        }
        let bytes =
            std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData as _);
        Some(base64(&bytes))
    }
}

fn decrypt(encoded: &str) -> Option<String> {
    let bytes = unbase64(encoded)?;
    unsafe {
        let mut input = blob(&bytes);
        let mut entropy = blob(ENTROPY);
        let mut output: CRYPT_INTEGER_BLOB = std::mem::zeroed();
        let ok = CryptUnprotectData(
            &mut input,
            std::ptr::null_mut(),
            &mut entropy,
            std::ptr::null_mut(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        );
        if ok == 0 {
            // Written by another account, or on another machine. Not an error
            // worth reporting: it means there is no key here for this person.
            return None;
        }
        let plain =
            std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData as _);
        String::from_utf8(plain).ok()
    }
}

/// The store: provider to ciphertext, the same file the 1.6 build reads and
/// writes. A real parser rather than a line matcher, because this file belongs
/// to somebody and losing a key to a mis-parse is not a small failure.
///
/// A store that cannot be read counts as no keys: it is not worth crashing over
/// and it is what the other build does.
fn read_store() -> serde_json::Map<String, serde_json::Value> {
    match crate::settings::read_json(&store_path()) {
        Some(serde_json::Value::Object(map)) => {
            map.into_iter().filter(|(_, value)| value.is_string()).collect()
        }
        _ => serde_json::Map::new(),
    }
}

fn write_store(entries: &serde_json::Map<String, serde_json::Value>) -> Result<(), String> {
    let path = store_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|exc| exc.to_string())?;
    }
    let text = serde_json::to_string_pretty(&serde_json::Value::Object(entries.clone()))
        .map_err(|exc| exc.to_string())?;
    std::fs::write(&path, text).map_err(|exc| exc.to_string())
}

pub fn save_key(provider: &str, key: &str) -> Result<(), String> {
    let encrypted = encrypt(key.trim()).ok_or(crate::words::w().could_not_encrypt)?;
    let mut entries = read_store();
    entries.insert(provider.to_string(), serde_json::Value::String(encrypted));
    write_store(&entries)
}

pub fn load_key(provider: &str) -> Option<String> {
    decrypt(read_store().get(provider)?.as_str()?)
}

pub fn forget_key(provider: &str) -> Result<(), String> {
    let mut entries = read_store();
    entries.remove(provider);
    write_store(&entries)
}

pub fn has_key(provider: &str) -> bool {
    load_key(provider).is_some()
}

// --- base64, because the store is text -------------------------------------

const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { ALPHABET[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { ALPHABET[n as usize & 63] as char } else { '=' });
    }
    out
}

fn unbase64(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for ch in text.bytes() {
        if ch == b'=' || ch == b'\n' || ch == b'\r' {
            continue;
        }
        let value = ALPHABET.iter().position(|c| *c == ch)? as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_comes_back_the_way_it_went_in() {
        let secret = "sk-test-0123456789-abcdefghijklmnop";
        let hidden = encrypt(secret).expect("DPAPI ha cifrato");
        // And it is not sitting there in plain sight.
        assert!(!hidden.contains("sk-test"));
        assert_eq!(decrypt(&hidden).as_deref(), Some(secret));
    }

    #[test]
    fn rubbish_decrypts_to_nothing_rather_than_panicking() {
        assert_eq!(decrypt("bm9uIHVuIGJsb2IgRFBBUEk="), None);
        assert_eq!(decrypt("!!!"), None);
    }

    #[test]
    fn base64_round_trips_every_length() {
        for length in 0..40 {
            let bytes: Vec<u8> = (0..length).map(|i| (i * 7 % 251) as u8).collect();
            let text = base64(&bytes);
            assert_eq!(unbase64(&text).as_deref(), Some(bytes.as_slice()), "lunghezza {}", length);
        }
    }
}
