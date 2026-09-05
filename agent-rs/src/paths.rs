// Where the agent keeps its few files, and the one-line files it shares with the
// editor.
//
// Every shared file is one line, UTF-8 without BOM, absolute path, no trailing
// newline, written to a temporary name and swapped in one move: the other side
// never reads a half-written file, and never finds no file at all.
//
// What belongs to a running process - the pid, the pending requests - lives in a
// per-session folder, because "Local\" names are per-session while %LOCALAPPDATA%
// is not: with fast user switching, the same user can have two agents. What is a
// choice rather than a process - autostart, the editor's path, the marker that
// the balloon has been shown - stays shared, which is what it should be.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::SystemTime;

use windows_sys::Win32::Storage::FileSystem::{
    MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
};
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows_sys::Win32::System::Threading::GetCurrentProcessId;

use crate::clock;
use crate::wide::wide;

/// Beyond this the log is cut back to its tail: one line per capture adds up.
const LOG_CEILING: u64 = 256 * 1024;

/// A request nobody served by now was left behind by a process that is gone.
const REQUEST_LIFE_SECS: u64 = 60;

pub fn root() -> PathBuf {
    // %LOCALAPPDATA% is set for every interactive logon; the fallback keeps a
    // stray context from writing into the process's working directory.
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Cutaway")
}

pub fn agent_dir() -> PathBuf {
    root().join("agent")
}

pub fn captures_dir() -> PathBuf {
    root().join("captures")
}

pub fn settings() -> PathBuf {
    root().join("settings.json")
}

/// Which Windows session this process belongs to; two of them can run at once.
pub fn session() -> u32 {
    static SESSION: OnceLock<u32> = OnceLock::new();
    *SESSION.get_or_init(|| unsafe {
        let mut id = 0u32;
        if ProcessIdToSessionId(GetCurrentProcessId(), &mut id) != 0 {
            id
        } else {
            0
        }
    })
}

pub fn session_dir() -> PathBuf {
    agent_dir().join(format!("s{}", session()))
}

// Per session: they describe a running process.
pub fn pid_file() -> PathBuf {
    session_dir().join("pid")
}

// Shared: they describe what the person chose.
pub fn autostart_file() -> PathBuf {
    agent_dir().join("autostart")
}

pub fn editor_hint() -> PathBuf {
    agent_dir().join("editor.txt")
}

pub fn introduced() -> PathBuf {
    agent_dir().join("introduced")
}

pub fn log_file() -> PathBuf {
    agent_dir().join("agent.log")
}

/// Writes the line and swaps it in: never a missing file, never a half one.
pub fn write_line(path: &Path, line: &str) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    // A sibling name rather than set_extension, which would eat the extension of
    // a file that has one.
    let temp = PathBuf::from(format!("{}.tmp", path.display()));
    if fs::write(&temp, line.as_bytes()).is_err() {
        append(&format!("write failed: {}", path.display()));
        return;
    }
    // One operation, unlike delete-then-move, which leaves a gap where the file
    // is not there and fails when someone holds it open.
    let ok = unsafe {
        MoveFileExW(
            wide(&temp.to_string_lossy()).as_ptr(),
            wide(&path.to_string_lossy()).as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        append(&format!("could not replace {}", path.display()));
        let _ = fs::remove_file(&temp);
    }
}

/// The single line, or None when the file is missing or unreadable.
pub fn read_line(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

// --- requests from the editor ----------------------------------------------

/// The folders asked for, oldest first; each request file is consumed as it is read.
pub fn take_requests() -> Vec<String> {
    let mut wanted = Vec::new();
    let Ok(entries) = fs::read_dir(session_dir()) else {
        return wanted;
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("request-") && n.ends_with(".txt"))
                .unwrap_or(false)
        })
        .collect();
    files.sort();
    for file in files {
        let stale = fs::metadata(&file)
            .and_then(|m| m.modified())
            .map(|t| {
                SystemTime::now()
                    .duration_since(t)
                    .map(|d| d.as_secs() > REQUEST_LIFE_SECS)
                    .unwrap_or(false)
            })
            .unwrap_or(false);
        let dir = if stale { None } else { read_line(&file) };
        let _ = fs::remove_file(&file);
        if let Some(d) = dir {
            if !d.is_empty() {
                wanted.push(d);
            }
        }
    }
    wanted
}

// --- captures ---------------------------------------------------------------

/// A fresh folder for one capture, named by the moment it was asked for.
pub fn new_capture_dir() -> std::io::Result<PathBuf> {
    let dir = captures_dir().join(format!("{}-s{}", clock::stamp_ms(), session()));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Leftovers from captures that never got consumed: an editor that died, a crash.
/// Only this session's: another one may have a handoff in flight.
pub fn sweep_captures() {
    let mine = format!("-s{}", session());
    let Ok(entries) = fs::read_dir(captures_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if name.ends_with(&mine) {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

// --- log --------------------------------------------------------------------

pub fn append(line: &str) {
    use std::io::Write;
    let _ = fs::create_dir_all(agent_dir());
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(log_file()) {
        let _ = write!(file, "{} {}\r\n", clock::stamp_log(), line);
    }
}

/// Called at start: a line per capture, kept for good, would grow without end.
pub fn trim_log() {
    let path = log_file();
    let Ok(meta) = fs::metadata(&path) else { return };
    if meta.len() <= LOG_CEILING {
        return;
    }
    let Ok(all) = fs::read_to_string(&path) else { return };
    let lines: Vec<&str> = all.lines().collect();
    // Half the ceiling back, so this does not happen again on every start.
    let tail = lines[lines.len() / 2..].join("\r\n");
    let _ = fs::write(&path, format!("{}\r\n", tail));
}
