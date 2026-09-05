// UTF-16, null terminated: what every W function on this side wants.

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Copies a string into a fixed-size UTF-16 array, truncating to leave the
/// terminator in place. The notification icon structures are full of these.
pub fn fill(target: &mut [u16], text: &str) {
    let source: Vec<u16> = text.encode_utf16().collect();
    let room = target.len() - 1;
    let n = source.len().min(room);
    target[..n].copy_from_slice(&source[..n]);
    target[n] = 0;
}
