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
//! ## Linux
//!
//! The same gap (`Exec=vela-wallet %u` in the .desktop entry starts a second
//! process), the same shape: the running wallet owns a Unix socket, and a
//! second process writes the URL into it and exits.
//!
//! - **Not another account**: the socket lives in `$XDG_RUNTIME_DIR`, which
//!   the relay only uses when it is this user's and closed to everyone else
//!   (`0700`); both ends also check the other's uid (`SO_PEERCRED`). In a
//!   Flatpak it is `$XDG_RUNTIME_DIR/app/$FLATPAK_ID`, the one directory every
//!   instance of the app shares — the sandbox's own runtime dir is private to
//!   each.
//! - **One owner**: whoever holds an exclusive `flock` on the lock file beside
//!   the socket owns it, for its whole life; the kernel drops the lock with the
//!   process. So a socket a crashed wallet left behind is replaced by the next
//!   one to start, and one a live wallet holds is never taken from it.
//!
//! What it does not do is bring the wallet in front: [`bring_to_front`] is
//! called off the UI thread, and a Wayland compositor would refuse a window
//! that raised itself without an activation token anyway.
//!
//! ## What is still open
//!
//! Two copies of the wallet running at once still split: only the first owns
//! the pipe (or socket), so an answer for a request the second copy opened
//! reaches the first and is dropped there.

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

#[cfg(target_os = "linux")]
mod imp {
    use std::fs::{File, OpenOptions};
    use std::io::{Read as _, Write as _};
    use std::os::fd::AsRawFd as _;
    use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    /// A signed answer is a few kilobytes; anything past this is not one.
    const MAX_URL: u64 = 64 * 1024;
    /// A client that connects and says nothing must not stall the next answer.
    const READ_FOR: Duration = Duration::from_secs(2);

    const SOCKET: &str = "app.getvela.wallet.sign-result.sock";
    const LOCK: &str = "app.getvela.wallet.sign-result.lock";

    fn own_uid() -> u32 {
        // SAFETY: no arguments, no failure mode.
        unsafe { libc::getuid() }
    }

    /// The uid at the other end of a connected Unix socket.
    fn peer_uid(socket: &UnixStream) -> Option<u32> {
        let mut cred = libc::ucred {
            pid: 0,
            uid: 0,
            gid: 0,
        };
        let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        // SAFETY: `cred` and `len` are live for the call and sized for
        // SO_PEERCRED; the fd is `socket`'s.
        let ok = unsafe {
            libc::getsockopt(
                socket.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&raw mut cred).cast(),
                &mut len,
            )
        } == 0;
        ok.then_some(cred.uid)
    }

    /// `$XDG_RUNTIME_DIR` (or the Flatpak app's shared corner of it), and only
    /// if it is this user's and nobody else's.
    fn runtime_dir() -> Option<PathBuf> {
        let base = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?);
        let dir = match std::env::var_os("FLATPAK_ID") {
            Some(id) => base.join("app").join(id),
            None => base,
        };
        private_dir(&dir).then_some(dir)
    }

    pub(super) fn private_dir(dir: &Path) -> bool {
        std::fs::metadata(dir)
            .is_ok_and(|meta| meta.is_dir() && meta.uid() == own_uid() && meta.mode() & 0o077 == 0)
    }

    pub fn forward(url: &str) -> bool {
        runtime_dir().is_some_and(|dir| forward_to(&dir, url))
    }

    pub(super) fn forward_to(dir: &Path, url: &str) -> bool {
        // Not found, or refused: no wallet is running (a crashed one may have
        // left its socket behind), and this process is the cold start `main`
        // already handles.
        let Ok(mut socket) = UnixStream::connect(dir.join(SOCKET)) else {
            return false;
        };
        if peer_uid(&socket) != Some(own_uid()) {
            eprintln!(
                "[vela-wallet] sign-result relay: the socket's owner is not this user's wallet"
            );
            return false;
        }
        socket.write_all(url.as_bytes()).is_ok()
    }

    pub fn listen() {
        let Some(dir) = runtime_dir() else {
            eprintln!(
                "[vela-wallet] sign-result relay: no private $XDG_RUNTIME_DIR; not listening"
            );
            return;
        };
        if !listen_on(&dir, crate::executor::trusted_signer::deliver_callback) {
            // Another copy of the wallet holds it. This copy answers only its
            // own cold starts.
            eprintln!(
                "[vela-wallet] sign-result relay: the socket is already owned; not listening"
            );
        }
    }

    /// `deliver` is `trusted_signer::deliver_callback` in the app; a test hands
    /// its own, so it can watch a real socket without a request pending.
    pub(super) fn listen_on(dir: &Path, deliver: fn(&str) -> bool) -> bool {
        let Ok(lock) = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .mode(0o600)
            .open(dir.join(LOCK))
        else {
            return false;
        };
        // SAFETY: the fd is `lock`'s. Per open file description, so a second
        // `listen_on` in this same process is refused too.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return false;
        }
        // Holding the lock, whatever socket is there is a dead wallet's.
        let path = dir.join(SOCKET);
        let _ = std::fs::remove_file(&path);
        let listener = match UnixListener::bind(&path) {
            Ok(listener) => listener,
            Err(error) => {
                eprintln!("[vela-wallet] sign-result relay: {error}");
                return false;
            }
        };
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        let spawned = std::thread::Builder::new()
            .name("sign-result relay".into())
            .spawn(move || serve(&listener, lock, deliver));
        if let Err(error) = spawned {
            eprintln!("[vela-wallet] sign-result relay: {error}");
        }
        true
    }

    /// `_lock` is held for as long as this serves, which is the process's life.
    fn serve(listener: &UnixListener, _lock: File, deliver: fn(&str) -> bool) {
        for socket in listener.incoming() {
            let Ok(socket) = socket else { continue };
            if peer_uid(&socket) != Some(own_uid()) {
                continue;
            }
            let _ = socket.set_read_timeout(Some(READ_FOR));
            let mut url = String::new();
            if socket.take(MAX_URL).read_to_string(&mut url).is_ok() {
                deliver(url.trim());
            }
        }
    }

    pub fn bring_to_front() {}
}

#[cfg(not(any(windows, target_os = "linux")))]
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
/// Always `false` on macOS, which never starts a second process for a URL.
pub fn forward(url: &str) -> bool {
    imp::forward(url)
}

/// Own the pipe (Windows) or socket (Linux) for the life of this process, on a
/// thread of its own. A no-op on macOS.
pub fn listen() {
    imp::listen();
}

/// Bring this app's window in front of the browser that just answered. Called
/// when an answer SETTLED a request, never merely because one arrived: a
/// callback for nobody must not pull the wallet over whatever the person is
/// doing. A no-op off Windows (see the file's header for Linux).
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

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use super::imp::{forward_to, listen_on, private_dir};

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

    fn private_temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vela-relay-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        dir
    }

    /// Two answers in a row reach the listener, which is a create: the key,
    /// then the member proof, each its own page visit and its own second
    /// process. A socket a crashed wallet left behind is no wallet, and is
    /// replaced; and a live owner is refused, not shared.
    #[test]
    fn a_second_process_hands_the_answer_to_the_first() {
        let dir = private_temp_dir("handoff");
        assert!(!forward_to(&dir, "velawallet://sign-result?t=nobody"));

        // What a crashed wallet leaves: a socket file nobody listens on.
        drop(std::os::unix::net::UnixListener::bind(
            dir.join("app.getvela.wallet.sign-result.sock"),
        ));
        assert!(!forward_to(&dir, "velawallet://sign-result?t=stale"));

        assert!(listen_on(&dir, record));
        assert!(!listen_on(&dir, record), "a held socket must not be shared");

        assert!(forward_to(&dir, "velawallet://sign-result?t=one"));
        // Trailing whitespace is trimmed, as a shell's echo would leave it.
        assert!(forward_to(&dir, "velawallet://sign-result?t=two\n"));
        assert_eq!(
            heard_within(2),
            [
                "velawallet://sign-result?t=one",
                "velawallet://sign-result?t=two"
            ]
        );
    }

    /// A runtime directory other people can reach is not one to listen in.
    #[test]
    fn only_a_directory_closed_to_everyone_else_is_used() {
        let dir = private_temp_dir("perms");
        assert!(private_dir(&dir));
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o750)).unwrap();
        assert!(!private_dir(&dir));
        assert!(!private_dir(&dir.join("missing")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
