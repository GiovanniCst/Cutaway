// Handing a picture to the mail client, through Simple MAPI.
//
// Nothing is ever sent from here, and that is a property of the code rather
// than a promise: MAPI_DIALOG makes MAPISendMailW open the client with the
// message ready and hand control to the person. Without that flag the same call
// would send by itself - so the flag is the whole safety, and there is no code
// path in this program that omits it.
//
// Simple MAPI is a legacy interface. The classic Outlook exposes it; the newer
// one does not, and neither does every webmail-only setup - so whether a client
// is registered at all is checked before the button offers to use it.

use std::ffi::CString;
use std::path::Path;

use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};

/// Opens the client with the message ready, rather than sending anything.
const MAPI_DIALOG: u32 = 0x0000_0008;
const MAPI_LOGON_UI: u32 = 0x0000_0001;
/// What comes back when the person closes the window without sending. Not an
/// error: it is the answer "no", and telling them something went wrong would be
/// a lie.
const MAPI_E_USER_ABORT: u32 = 1;

#[repr(C)]
struct MapiFileDesc {
    reserved: u32,
    flags: u32,
    position: u32,
    path: *const i8,
    name: *const i8,
    file_type: *const i8,
}

#[repr(C)]
struct MapiMessage {
    reserved: u32,
    subject: *const i8,
    note_text: *const i8,
    message_type: *const i8,
    date_received: *const i8,
    conversation_id: *const i8,
    flags: u32,
    originator: *const i8,
    recipient_count: u32,
    recipients: *const i8,
    file_count: u32,
    files: *const MapiFileDesc,
}

type MapiSendMail = unsafe extern "system" fn(
    session: usize,
    parent: usize,
    message: *const MapiMessage,
    flags: u32,
    reserved: u32,
) -> u32;

/// Whether a mail client that speaks Simple MAPI is registered here.
pub fn available() -> bool {
    load().is_some()
}

fn load() -> Option<MapiSendMail> {
    unsafe {
        let name = CString::new("MAPI32.DLL").ok()?;
        let library = LoadLibraryA(name.as_ptr() as *const u8);
        if library.is_null() {
            return None;
        }
        // The ANSI entry point on purpose: MAPISendMailW exists but is not
        // implemented by every client that registers the DLL, and one that
        // answers the export while doing nothing is worse than one that is
        // absent.
        let symbol = CString::new("MAPISendMail").ok()?;
        let address = GetProcAddress(library, symbol.as_ptr() as *const u8)?;
        Some(std::mem::transmute::<_, MapiSendMail>(address))
    }
}

/// Opens the mail client with the picture attached.
///
/// The file has to exist when this is called and may be deleted afterwards:
/// MAPI copies the attachment before returning.
pub fn compose(attachment: &Path, subject: &str, body: &str) -> Result<(), String> {
    let Some(send) = load() else {
        return Err(crate::words::w().no_mail_program.into());
    };

    // Ansi strings, kept alive for the whole call: the structure holds pointers
    // into them and nothing copies them until MAPI does.
    let path = CString::new(attachment.to_string_lossy().as_ref())
        .map_err(|_| crate::words::w().invalid_text.to_string())?;
    let name = CString::new(
        attachment.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
    )
    .map_err(|_| crate::words::w().invalid_text.to_string())?;
    let subject = CString::new(subject).map_err(|_| crate::words::w().invalid_text.to_string())?;
    let body = CString::new(body).map_err(|_| crate::words::w().invalid_text.to_string())?;

    let file = MapiFileDesc {
        reserved: 0,
        flags: 0,
        // -1 as an unsigned: the attachment goes at the end rather than being
        // placed inside the text at a character position.
        position: u32::MAX,
        path: path.as_ptr(),
        name: name.as_ptr(),
        file_type: std::ptr::null(),
    };
    let message = MapiMessage {
        reserved: 0,
        subject: subject.as_ptr(),
        note_text: body.as_ptr(),
        message_type: std::ptr::null(),
        date_received: std::ptr::null(),
        conversation_id: std::ptr::null(),
        flags: 0,
        originator: std::ptr::null(),
        // Nobody to: the address is the person's to type, in their own client,
        // where their address book is.
        recipient_count: 0,
        recipients: std::ptr::null(),
        file_count: 1,
        files: &file,
    };

    let outcome = unsafe { send(0, 0, &message, MAPI_DIALOG | MAPI_LOGON_UI, 0) };
    match outcome {
        0 | MAPI_E_USER_ABORT => Ok(()),
        other => Err(crate::words::fill(crate::words::w().mail_answered, &[&other.to_string()])),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The structure's shape is a contract with a DLL written in C: if the
    /// fields drift, MAPI reads a pointer where a count should be and the
    /// failure is a crash inside somebody else's code.
    #[test]
    fn the_structures_are_the_size_mapi_expects() {
        if std::mem::size_of::<*const i8>() != 8 {
            return; // the numbers below are the 64-bit ones
        }
        // MapiFileDesc in C: three ULONGs, then three pointers. The compiler
        // pads the twelve bytes of counters out to sixteen so the first pointer
        // is aligned - 16 + 24.
        assert_eq!(std::mem::size_of::<MapiFileDesc>(), 40);
        // 96, and worth saying how that is known: the same declarations were put
        // through Python's ctypes, which lays structures out by the C rules, and
        // both sides were asked. Counting the padding by hand gave 88 - one pad
        // forgotten between the recipient count and the file count - and that
        // wrong number would have failed against a structure that was correct.
        assert_eq!(std::mem::size_of::<MapiMessage>(), 96);
        // And the fields land where C would put them, which is what actually
        // matters: a count read as a pointer crashes inside somebody else's DLL.
        let message = MapiMessage {
            reserved: 0,
            subject: std::ptr::null(),
            note_text: std::ptr::null(),
            message_type: std::ptr::null(),
            date_received: std::ptr::null(),
            conversation_id: std::ptr::null(),
            flags: 0,
            originator: std::ptr::null(),
            recipient_count: 0,
            recipients: std::ptr::null(),
            file_count: 1,
            files: std::ptr::null(),
        };
        let base = &message as *const MapiMessage as usize;
        assert_eq!(&message.subject as *const _ as usize - base, 8);
        assert_eq!(&message.file_count as *const _ as usize - base, 80);
        assert_eq!(&message.files as *const _ as usize - base, 88);
    }

    #[test]
    fn a_missing_client_is_reported_rather_than_assumed() {
        // Whatever this machine has, asking must not panic and must give an
        // answer either way.
        let _ = available();
    }
}
