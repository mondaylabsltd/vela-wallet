//! The Trusted Signer's answer, handed to the wallet that is already running
//! (spec 076).
//!
//! The signer page answers by navigating to `velawallet://sign-result?…`. macOS
//! delivers that as an Apple Event to the running app. Windows does not: the
//! scheme's registry key (`installer/VelaWallet.iss`) starts
//! `VelaWallet.exe "%1"`, a SECOND process, and the request that answer belongs
//! to lives in the FIRST one's memory — its one-time token was never written
//! anywhere else. Without this file the second process opened a second window
//! and dropped the answer, and the first waited out its five minutes. That is
//! every create and every sign-in, because the person pressed the button in a
//! running wallet.
//!
//! So the first process owns a named pipe, and the second, when it was started
//! with a callback, writes the URL into it and exits before drawing anything.
//! What arrives goes to `trusted_signer::deliver_callback`, exactly as a
//! cold-start argument or a macOS event does: the pipe decides nothing about
//! the answer — whether it is one, and whose, stay the core's.
//!
//! ## Who can reach the pipe
//!
//! - **Not another machine**: `PIPE_REJECT_REMOTE_CLIENTS`.
//! - **Not another account**: a pipe's default DACL gives write access only to
//!   its creator (and SYSTEM and administrators). The name also carries the
//!   Windows session and the user name, so two people signed in on one machine
//!   each have their own pipe.
//! - **Not a squatter**: the first instance is created with
//!   `FILE_FLAG_FIRST_PIPE_INSTANCE`, so a name somebody else already holds is
//!   refused rather than shared; each next instance is created BEFORE the
//!   current one is read, so the name is never left unowned; and the second
//!   process hands its URL only to a server in its own session, and connects
//!   at `SECURITY_IDENTIFICATION` so that server cannot act as it.
//!
//! None of that is what keeps a forged answer out: the scheme itself is not
//! exclusive — any app may register `velawallet` — which is why the callback
//! carries a one-time token and the core verifies the assertion itself.
//!
//! ## What is still open
//!
//! Linux has the same gap (`%u` in the .desktop entry starts a second process)
//! and no relay yet: a Unix socket would do it, and nothing here is
//! Windows-shaped above the platform calls. Two copies of the wallet running
//! at once also still split: only the first owns the pipe, so an answer for a
//! request the second copy opened reaches the first and is dropped there.

#[cfg(windows)]
mod imp {
    use std::fs::{File, OpenOptions};
    use std::io::{Read as _, Write as _};
    use std::os::windows::fs::OpenOptionsExt as _;
    use std::os::windows::io::{AsRawHandle as _, FromRawHandle as _};
    use std::time::{Duration, Instant};

    use windows_sys::Win32::Foundation::{
        BOOL, ERROR_NO_DATA, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, GetLastError, HANDLE, HWND,
        INVALID_HANDLE_VALUE, LPARAM,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_INBOUND, SECURITY_IDENTIFICATION,
    };
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientSessionId,
        GetNamedPipeServerProcessId, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
        PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
    };
    use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AllowSetForegroundWindow, EnumWindows, GW_OWNER, GetWindow, GetWindowThreadProcessId,
        IsIconic, IsWindowVisible, SW_RESTORE, SetForegroundWindow, ShowWindow,
    };

    /// A signed answer is a few kilobytes; anything past this is not one.
    const MAX_URL: u64 = 64 * 1024;
    /// How long the second process keeps trying a pipe that is between
    /// instances. The server makes the next one before reading the current, so
    /// "busy" is a breath, not a state.
    const BUSY_FOR: Duration = Duration::from_secs(2);

    fn session_of(pid: u32) -> Option<u32> {
        let mut session = 0;
        // SAFETY: `session` is a live u32 for the call's duration.
        (unsafe { ProcessIdToSessionId(pid, &mut session) } != 0).then_some(session)
    }

    fn own_session() -> Option<u32> {
        // SAFETY: no arguments, no failure mode.
        session_of(unsafe { GetCurrentProcessId() })
    }

    fn pipe_name() -> String {
        // A session is `u32::MAX` only if Windows cannot say which one this
        // is — never in practice, and still a name nobody else computes.
        let session = own_session().unwrap_or(u32::MAX);
        // A backslash is the one character a pipe name may not carry past its
        // prefix; a Chinese user name is fine as it is (the name goes out UTF-16).
        let user = std::env::var("USERNAME")
            .unwrap_or_default()
            .replace('\\', "_");
        format!(r"\\.\pipe\app.getvela.wallet.sign-result.{session}.{user}")
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    pub fn forward(url: &str) -> bool {
        forward_to(&pipe_name(), url)
    }

    pub(super) fn forward_to(name: &str, url: &str) -> bool {
        let deadline = Instant::now() + BUSY_FOR;
        let mut pipe = loop {
            match OpenOptions::new()
                .write(true)
                .security_qos_flags(SECURITY_IDENTIFICATION)
                .open(name)
            {
                Ok(pipe) => break pipe,
                Err(error)
                    if error.raw_os_error() == Some(ERROR_PIPE_BUSY as i32)
                        && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(20));
                }
                // Not found: no wallet is running, and this process is the
                // cold start `main` already handles.
                Err(_) => return false,
            }
        };
        let mut server = 0;
        // SAFETY: the handle is `pipe`'s and outlives the call.
        let known =
            unsafe { GetNamedPipeServerProcessId(pipe.as_raw_handle() as HANDLE, &mut server) }
                != 0;
        if !known || session_of(server).is_none() || session_of(server) != own_session() {
            eprintln!(
                "[vela-wallet] sign-result relay: the pipe's owner is not this session's wallet"
            );
            return false;
        }
        // The browser started this process for a click, so it may take the
        // foreground; the wallet, which will want it once the answer settles,
        // may not. This passes the right on.
        // SAFETY: plain value argument.
        unsafe { AllowSetForegroundWindow(server) };
        pipe.write_all(url.as_bytes()).is_ok()
    }

    fn create(name: &[u16], first: bool) -> Option<File> {
        let mode = PIPE_ACCESS_INBOUND
            | if first {
                FILE_FLAG_FIRST_PIPE_INSTANCE
            } else {
                0
            };
        // SAFETY: `name` is NUL-terminated UTF-16 and outlives the call; a null
        // security descriptor is the default DACL, which is the point.
        let handle = unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                mode,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                0,
                4096,
                0,
                std::ptr::null(),
            )
        };
        // SAFETY: a valid handle this process now owns, and nothing else does.
        (handle != INVALID_HANDLE_VALUE).then(|| unsafe { File::from_raw_handle(handle as _) })
    }

    pub fn listen() {
        if !listen_on(
            &pipe_name(),
            crate::executor::trusted_signer::deliver_callback,
        ) {
            // Another copy of the wallet holds it — or somebody who should not.
            // Either way this copy answers only its own cold starts.
            eprintln!("[vela-wallet] sign-result relay: the pipe is already owned; not listening");
        }
    }

    /// `deliver` is `trusted_signer::deliver_callback` in the app; a test hands
    /// its own, so it can watch a real pipe without a request pending.
    pub(super) fn listen_on(name: &str, deliver: fn(&str) -> bool) -> bool {
        let name = wide(name);
        let Some(first) = create(&name, true) else {
            return false;
        };
        let spawned = std::thread::Builder::new()
            .name("sign-result relay".into())
            .spawn(move || serve(&name, first, deliver));
        if let Err(error) = spawned {
            eprintln!("[vela-wallet] sign-result relay: {error}");
        }
        true
    }

    fn serve(name: &[u16], mut pipe: File, deliver: fn(&str) -> bool) {
        loop {
            // SAFETY: the handle is `pipe`'s; no OVERLAPPED, so this blocks.
            // `ERROR_NO_DATA` is a client that connected, wrote and closed
            // before this call — the ordinary case for a second process that
            // exits the moment it has written. Its bytes are still in the pipe.
            let connected = unsafe { ConnectNamedPipe(pipe.as_raw_handle() as HANDLE, std::ptr::null_mut()) } != 0
                // SAFETY: no arguments.
                || matches!(unsafe { GetLastError() }, ERROR_PIPE_CONNECTED | ERROR_NO_DATA);
            // The next instance BEFORE this one is read, so the name is never
            // free for somebody else to take between two answers.
            let Some(next) = create(name, false) else {
                eprintln!("[vela-wallet] sign-result relay: no next pipe instance; stopped");
                return;
            };
            let current = std::mem::replace(&mut pipe, next);
            if connected && client_in_own_session(&current) {
                let mut url = String::new();
                if current.take(MAX_URL).read_to_string(&mut url).is_ok() {
                    deliver(url.trim());
                }
            }
        }
    }

    /// Asked of the pipe, not of the client's process id: that process has
    /// usually exited by now, and a dead pid has no session to look up.
    fn client_in_own_session(pipe: &File) -> bool {
        let mut session = 0;
        // SAFETY: the handle is `pipe`'s and outlives the call.
        let known =
            unsafe { GetNamedPipeClientSessionId(pipe.as_raw_handle() as HANDLE, &mut session) }
                != 0;
        known && Some(session) == own_session()
    }

    pub fn bring_to_front() {
        unsafe extern "system" fn each(window: HWND, found: LPARAM) -> BOOL {
            let mut pid = 0;
            // SAFETY: `window` is the one EnumWindows handed us; `found` is the
            // `HWND` slot `bring_to_front` passed, alive for the enumeration.
            unsafe {
                GetWindowThreadProcessId(window, &mut pid);
                if pid == GetCurrentProcessId()
                    && IsWindowVisible(window) != 0
                    && GetWindow(window, GW_OWNER).is_null()
                {
                    *(found as *mut HWND) = window;
                    return 0;
                }
            }
            1
        }
        let mut found: HWND = std::ptr::null_mut();
        // SAFETY: `found` outlives the enumeration, which is synchronous.
        unsafe { EnumWindows(Some(each), &mut found as *mut HWND as LPARAM) };
        if found.is_null() {
            return;
        }
        // SAFETY: a top-level window of this process, found just now.
        unsafe {
            if IsIconic(found) != 0 {
                ShowWindow(found, SW_RESTORE);
            }
            SetForegroundWindow(found);
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn forward(_url: &str) -> bool {
        false
    }

    pub fn listen() {}

    pub fn bring_to_front() {}
}

/// Hand `url` to the wallet already running in this session. `true` when it
/// took it — this process then has nothing left to do and should exit before
/// opening a window. `false` when no wallet is running, and this process is a
/// cold start.
///
/// Always `false` off Windows: macOS never starts a second process for a URL,
/// and Linux has no relay yet (see the file's header).
pub fn forward(url: &str) -> bool {
    imp::forward(url)
}

/// Own the pipe for the life of this process, on a thread of its own. A no-op
/// off Windows.
pub fn listen() {
    imp::listen();
}

/// Bring this app's window in front of the browser that just answered. Called
/// when an answer SETTLED a request, never merely because one arrived: a
/// callback for nobody must not pull the wallet over whatever the person is
/// doing. A no-op off Windows.
pub fn bring_to_front() {
    imp::bring_to_front();
}

#[cfg(all(test, windows))]
mod tests {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use super::imp::{forward_to, listen_on};

    static HEARD: Mutex<Vec<String>> = Mutex::new(Vec::new());

    fn record(url: &str) -> bool {
        HEARD.lock().unwrap().push(url.to_owned());
        true
    }

    fn heard_within(count: usize) -> Vec<String> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let heard = HEARD.lock().unwrap().clone();
            if heard.len() >= count || Instant::now() >= deadline {
                return heard;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Two answers in a row reach the listener, which is a create: the key,
    /// then the member proof, each its own page visit and its own second
    /// process. And a name somebody already holds is refused, not shared.
    #[test]
    fn a_second_process_hands_the_answer_to_the_first() {
        let name = format!(r"\\.\pipe\vela-relay-test-{}", std::process::id());
        assert!(!forward_to(&name, "velawallet://sign-result?t=nobody"));

        assert!(listen_on(&name, record));
        assert!(!listen_on(&name, record), "a held name must not be shared");

        assert!(forward_to(&name, "velawallet://sign-result?t=one"));
        // Trailing whitespace is trimmed, as a shell's echo would leave it.
        assert!(forward_to(&name, "velawallet://sign-result?t=two\n"));
        assert_eq!(
            heard_within(2),
            [
                "velawallet://sign-result?t=one",
                "velawallet://sign-result?t=two"
            ]
        );
    }
}
