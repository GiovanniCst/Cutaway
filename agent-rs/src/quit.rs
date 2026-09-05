// Stopping the resident agent from outside: the installer before it replaces the
// file, the uninstaller, the portable editor before it updates the copy.
//
// A named event, and the caller waits for the process itself to be gone. Not for
// the mutex: it is released as this process unwinds, so it comes free while the
// image of the exe is still mapped - and "the file can be replaced now" is
// exactly what the caller needs to know.
//
// The pid alone does not identify anything: Windows reuses pids. It is written
// with the process's start time, and only a process that matches both is the
// agent.

use std::thread;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, WAIT_OBJECT_0};
use windows_sys::Win32::System::Threading::{
    CreateEventW, CreateMutexW, GetCurrentProcess, GetCurrentProcessId, GetProcessTimes,
    OpenEventW, OpenProcess, SetEvent, WaitForSingleObject, EVENT_MODIFY_STATE, INFINITE,
    PROCESS_QUERY_LIMITED_INFORMATION,
};

/// The standard right to wait on a handle. Spelled out because the crate files
/// it under file access rights, where a process handle has no business looking.
const SYNCHRONIZE: u32 = 0x0010_0000;

use crate::clock;
use crate::paths;
use crate::wide::wide;

pub const MUTEX_NAME: &str = "Local\\Cutaway.Agent";
pub const QUIT_EVENT: &str = "Local\\Cutaway.Agent.Quit";
pub const CAPTURE_EVENT: &str = "Local\\Cutaway.Agent.Capture";

/// A handle that closes itself. Every Win32 handle in this program travels in one.
pub struct Owned(pub HANDLE);

impl Drop for Owned {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CloseHandle(self.0) };
        }
    }
}

unsafe impl Send for Owned {}

/// Takes the single-instance mutex. None when an agent is already resident.
pub fn claim_single_instance() -> Option<Owned> {
    unsafe {
        let handle = CreateMutexW(std::ptr::null(), 1, wide(MUTEX_NAME).as_ptr());
        if handle.is_null() {
            return None;
        }
        // 183 is ERROR_ALREADY_EXISTS: the mutex was somebody else's first.
        if windows_sys::Win32::Foundation::GetLastError() == 183 {
            CloseHandle(handle);
            return None;
        }
        Some(Owned(handle))
    }
}

/// Writes down who we are, so the editor and --quit can find this exact process.
pub fn record_self() {
    let pid = unsafe { GetCurrentProcessId() };
    match start_ticks_of(unsafe { GetCurrentProcess() }) {
        Some(ticks) => paths::write_line(&paths::pid_file(), &format!("{} {}", pid, ticks)),
        None => paths::append("could not read own start time"),
    }
}

/// The process's start time in .NET ticks, which is what the pid file records.
fn start_ticks_of(process: HANDLE) -> Option<i64> {
    unsafe {
        let mut created: FILETIME = std::mem::zeroed();
        let mut exited: FILETIME = std::mem::zeroed();
        let mut kernel: FILETIME = std::mem::zeroed();
        let mut user: FILETIME = std::mem::zeroed();
        if GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) == 0 {
            return None;
        }
        Some(clock::filetime_to_net_ticks(created))
    }
}

/// The recorded agent if it is still that same process, otherwise None.
fn recorded() -> Option<Owned> {
    let line = paths::read_line(&paths::pid_file())?;
    let (pid, ticks) = line.split_once(' ')?;
    let pid: u32 = pid.parse().ok()?;
    let ticks: i64 = ticks.parse().ok()?;
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, 0, pid);
        if handle.is_null() {
            return None; // nothing runs under that pid, or no right to look
        }
        let owned = Owned(handle);
        match start_ticks_of(owned.0) {
            // The same number belonging to another process: the pid was reused.
            Some(found) if found == ticks => Some(owned),
            _ => None,
        }
    }
}

/// Creates the quit event and watches it on its own thread.
pub fn watch<F: FnOnce() + Send + 'static>(on_quit: F) -> Owned {
    let handle = unsafe {
        // Manual reset: everyone waiting on it should see it, and it is set once.
        CreateEventW(std::ptr::null(), 1, 0, wide(QUIT_EVENT).as_ptr())
    };
    let watched = Owned(handle);
    let raw = handle as usize;
    thread::spawn(move || unsafe {
        WaitForSingleObject(raw as HANDLE, INFINITE);
        on_quit();
    });
    watched
}

/// Creates the capture event and watches it, calling back on every signal.
pub fn watch_captures<F: Fn() + Send + 'static>(on_capture: F) -> Owned {
    let handle = unsafe {
        // Auto reset: each signal stands for one request to serve.
        CreateEventW(std::ptr::null(), 0, 0, wide(CAPTURE_EVENT).as_ptr())
    };
    let watched = Owned(handle);
    let raw = handle as usize;
    thread::spawn(move || loop {
        unsafe { WaitForSingleObject(raw as HANDLE, INFINITE) };
        on_capture();
    });
    watched
}

/// Asks a running agent to leave and waits until it has. True when none is left.
pub fn request(timeout_ms: u64) -> bool {
    unsafe {
        let signal = OpenEventW(EVENT_MODIFY_STATE, 0, wide(QUIT_EVENT).as_ptr());
        if signal.is_null() {
            return true; // nobody is listening: nothing to stop
        }
        let signal = Owned(signal);
        // The handle is taken before the event is set: afterwards the process may
        // already be gone, and there would be nothing left to wait on.
        let agent = recorded();
        SetEvent(signal.0);
        if let Some(agent) = agent {
            return WaitForSingleObject(agent.0, timeout_ms as u32) == WAIT_OBJECT_0;
        }
    }
    wait_for_mutex(timeout_ms)
}

/// Fallback when there is no pid to watch: the mutex says the message got
/// through, which is less than the process being gone, and is all there is.
fn wait_for_mutex(timeout_ms: u64) -> bool {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    while Instant::now() < deadline {
        if claim_single_instance().is_some() {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    false
}
