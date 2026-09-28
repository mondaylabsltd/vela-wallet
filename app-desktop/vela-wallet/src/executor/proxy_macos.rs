//! The Mac's own answer to "how does a request to this URL get out" (spec 082
//! RD2, ruling 3).
//!
//! The wallet's traffic follows the system proxy the way WebKit does, because
//! it asks the same resolver WebKit asks: `CFNetworkCopyProxiesForURL` over
//! `CFNetworkCopySystemProxySettings`. That one call applies the exception
//! list, "exclude simple hostnames" and the per-scheme proxies, and when the
//! setting is a PAC file it names the file, which is then run by CFNetwork
//! itself (`CFNetworkExecuteProxyAutoConfigurationURL/Script`) — never by a
//! second JavaScript engine that could disagree with the browser's.
//!
//! ## One thread for all of it
//!
//! A PAC run is asynchronous: it reports through a run-loop source, so it
//! needs a run loop to run on. The hidapi work taught this shell that touching
//! `CFRunLoop` from a gpui dispatch worker crashes the app, so every call in
//! this file — the settings read, the per-URL lookup and the PAC run — happens
//! on one thread of its own, `vela-pac`, which owns its run loop and runs it
//! only in a private mode, only while a PAC answer is awaited, for at most
//! [`PAC_BUDGET`]. Callers on any thread send a job and wait for the answer.
//!
//! ## What is remembered
//!
//! The system settings for [`SETTINGS_TTL`] (a proxy switched on in System
//! Settings is honoured within seconds); a PAC answer per scheme and host for
//! [`PAC_TTL`], a PAC failure for [`PAC_FAILURE_TTL`]. Nothing about which
//! route worked: that is decided per request, in `proxy.rs`.
//!
//! This module only reads system settings. It never calls `networksetup` or
//! writes SCPreferences.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_foundation_sys::array::{CFArrayGetCount, CFArrayGetValueAtIndex, CFArrayRef};
use core_foundation_sys::base::{
    CFEqual, CFGetTypeID, CFIndex, CFRelease, CFTypeRef, kCFAllocatorDefault,
};
use core_foundation_sys::dictionary::{CFDictionaryGetValue, CFDictionaryRef};
use core_foundation_sys::error::{CFErrorGetCode, CFErrorRef};
use core_foundation_sys::number::{CFNumberGetTypeID, CFNumberGetValue, kCFNumberSInt64Type};
use core_foundation_sys::runloop::{
    CFRunLoopAddSource, CFRunLoopGetCurrent, CFRunLoopRemoveSource, CFRunLoopRunInMode,
    CFRunLoopSourceInvalidate, CFRunLoopSourceRef, CFRunLoopStop,
};
use core_foundation_sys::string::{CFStringGetTypeID, CFStringRef};
use core_foundation_sys::url::{CFURLCreateWithString, CFURLGetString, CFURLGetTypeID, CFURLRef};

/// How long a PAC run may take before it counts as failed.
pub const PAC_BUDGET: Duration = Duration::from_secs(5);
/// How long the system settings are trusted before they are read again.
const SETTINGS_TTL: Duration = Duration::from_secs(5);
/// How long a PAC answer stands for one scheme and host.
const PAC_TTL: Duration = Duration::from_secs(300);
/// How long a PAC failure stands before the file is run again.
const PAC_FAILURE_TTL: Duration = Duration::from_secs(30);

/// One way out, as CFNetwork names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// `kCFProxyTypeNone`: no proxy — only when the system says so.
    Direct,
    /// `kCFProxyTypeHTTP` / `kCFProxyTypeHTTPS`: an HTTP proxy, reached with
    /// CONNECT.
    Http { host: String, port: u16 },
    /// `kCFProxyTypeSOCKS`.
    Socks { host: String, port: u16 },
}

/// Why no route could be named: the PAC file could not be fetched or run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PacFailed {
    /// The PAC file's host (never its path, which can carry a token), or
    /// "script" for an inline one.
    pub pac: String,
    pub reason: String,
}

#[allow(
    non_upper_case_globals,
    reason = "CFNetwork's own names, so a grep for them finds them"
)]
#[link(name = "CFNetwork", kind = "framework")]
unsafe extern "C" {
    fn CFNetworkCopySystemProxySettings() -> CFDictionaryRef;
    fn CFNetworkCopyProxiesForURL(url: CFURLRef, proxy_settings: CFDictionaryRef) -> CFArrayRef;
    fn CFNetworkExecuteProxyAutoConfigurationURL(
        proxy_auto_config_url: CFURLRef,
        target_url: CFURLRef,
        callback: PacCallback,
        client_context: *mut StreamClientContext,
    ) -> CFRunLoopSourceRef;
    fn CFNetworkExecuteProxyAutoConfigurationScript(
        proxy_auto_configuration_script: CFStringRef,
        target_url: CFURLRef,
        callback: PacCallback,
        client_context: *mut StreamClientContext,
    ) -> CFRunLoopSourceRef;

    static kCFProxyTypeKey: CFStringRef;
    static kCFProxyHostNameKey: CFStringRef;
    static kCFProxyPortNumberKey: CFStringRef;
    static kCFProxyAutoConfigurationURLKey: CFStringRef;
    static kCFProxyAutoConfigurationJavaScriptKey: CFStringRef;
    static kCFProxyTypeNone: CFStringRef;
    static kCFProxyTypeHTTP: CFStringRef;
    static kCFProxyTypeHTTPS: CFStringRef;
    static kCFProxyTypeSOCKS: CFStringRef;
    static kCFProxyTypeAutoConfigurationURL: CFStringRef;
    static kCFProxyTypeAutoConfigurationJavaScript: CFStringRef;
}

/// `CFProxyAutoConfigurationResultCallback`.
type PacCallback = extern "C" fn(client: *mut c_void, proxy_list: CFArrayRef, error: CFErrorRef);

/// `CFStreamClientContext`, with the three callbacks nullable as Apple
/// documents them (the `-sys` crate declares them non-null).
#[repr(C)]
struct StreamClientContext {
    version: CFIndex,
    info: *mut c_void,
    retain: Option<extern "C" fn(*const c_void) -> *const c_void>,
    release: Option<extern "C" fn(*const c_void)>,
    copy_description: Option<extern "C" fn(*const c_void) -> CFStringRef>,
}

/// An owned CF object pointer (created or copied, so released on drop).
struct Owned(CFTypeRef);

impl Owned {
    fn new(ptr: CFTypeRef) -> Option<Self> {
        (!ptr.is_null()).then_some(Self(ptr))
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: created or copied by this file, released exactly once.
        unsafe { CFRelease(self.0) };
    }
}

// SAFETY: an immutable CF object (the settings dictionary) may be used from
// any thread; this one is only ever used on `vela-pac` anyway.
unsafe impl Send for Owned {}

/// A proxy settings dictionary, as `CFNetworkCopyProxiesForURL` takes one —
/// the system's, or one a test builds.
pub struct Settings(Owned);

impl Settings {
    /// The system's settings, fresh.
    fn system() -> Option<Self> {
        // SAFETY: a Copy function; the result is owned and released on drop.
        Owned::new(unsafe { CFNetworkCopySystemProxySettings() }.cast()).map(Self)
    }

    /// Settings from a dictionary the caller built (the tests' seam).
    #[cfg(test)]
    pub fn from_dictionary(
        dictionary: &core_foundation::dictionary::CFDictionary<
            core_foundation::base::CFType,
            core_foundation::base::CFType,
        >,
    ) -> Self {
        // SAFETY: retained here, released on drop.
        let raw = unsafe { core_foundation_sys::base::CFRetain(dictionary.as_CFTypeRef()) };
        Self(Owned(raw))
    }

    fn as_dictionary(&self) -> CFDictionaryRef {
        self.0.0.cast()
    }
}

fn cf_url(url: &str) -> Option<Owned> {
    let text = CFString::new(url);
    // SAFETY: a Create function over a live CFString; owned on return.
    Owned::new(
        unsafe {
            CFURLCreateWithString(
                kCFAllocatorDefault,
                text.as_concrete_TypeRef(),
                std::ptr::null(),
            )
        }
        .cast(),
    )
}

/// A CFString (not owned here) as a Rust string.
fn string_of(value: CFTypeRef) -> Option<String> {
    // SAFETY: type-checked before it is wrapped; the get rule retains.
    unsafe {
        if value.is_null() || CFGetTypeID(value) != CFStringGetTypeID() {
            return None;
        }
        Some(CFString::wrap_under_get_rule(value.cast()).to_string())
    }
}

fn number_of(value: CFTypeRef) -> Option<i64> {
    // SAFETY: type-checked; the value is written into a local.
    unsafe {
        if value.is_null() || CFGetTypeID(value) != CFNumberGetTypeID() {
            return None;
        }
        let mut out: i64 = 0;
        CFNumberGetValue(
            value.cast(),
            kCFNumberSInt64Type,
            std::ptr::from_mut(&mut out).cast(),
        )
        .then_some(out)
    }
}

/// A CFURL or a CFString, as text.
fn url_text_of(value: CFTypeRef) -> Option<String> {
    // SAFETY: type-checked; CFURLGetString follows the get rule.
    unsafe {
        if !value.is_null() && CFGetTypeID(value) == CFURLGetTypeID() {
            return string_of(CFURLGetString(value.cast()).cast());
        }
    }
    string_of(value)
}

fn get(dictionary: CFDictionaryRef, key: CFStringRef) -> CFTypeRef {
    // SAFETY: a live dictionary and one of CFNetwork's constant keys.
    unsafe { CFDictionaryGetValue(dictionary, key.cast()) }
}

fn same(a: CFTypeRef, b: CFStringRef) -> bool {
    // SAFETY: both live; CFEqual tolerates any two CF objects.
    !a.is_null() && unsafe { CFEqual(a, b.cast()) } != 0
}

/// One entry of a proxy list, or what it asks for next.
enum Raw {
    Entry(Entry),
    PacUrl(String),
    PacScript(String),
    /// A type this wallet does not speak (FTP, …): skipped.
    Other,
}

fn raw_of(entry: CFDictionaryRef) -> Raw {
    // SAFETY: the keys are CFNetwork's constants, read only.
    let (kind, host, port) = unsafe {
        (
            get(entry, kCFProxyTypeKey),
            get(entry, kCFProxyHostNameKey),
            get(entry, kCFProxyPortNumberKey),
        )
    };
    let endpoint = || {
        let host = string_of(host)?;
        let port = u16::try_from(number_of(port)?).ok()?;
        (!host.is_empty() && port != 0).then_some((host, port))
    };
    // SAFETY: constant keys and type names, read only.
    unsafe {
        if same(kind, kCFProxyTypeNone) {
            return Raw::Entry(Entry::Direct);
        }
        if same(kind, kCFProxyTypeHTTP) || same(kind, kCFProxyTypeHTTPS) {
            return endpoint().map_or(Raw::Other, |(host, port)| {
                Raw::Entry(Entry::Http { host, port })
            });
        }
        if same(kind, kCFProxyTypeSOCKS) {
            return endpoint().map_or(Raw::Other, |(host, port)| {
                Raw::Entry(Entry::Socks { host, port })
            });
        }
        if same(kind, kCFProxyTypeAutoConfigurationURL) {
            return url_text_of(get(entry, kCFProxyAutoConfigurationURLKey))
                .map_or(Raw::Other, Raw::PacUrl);
        }
        if same(kind, kCFProxyTypeAutoConfigurationJavaScript) {
            return string_of(get(entry, kCFProxyAutoConfigurationJavaScriptKey))
                .map_or(Raw::Other, Raw::PacScript);
        }
    }
    Raw::Other
}

fn raws_of(list: CFArrayRef) -> Vec<Raw> {
    if list.is_null() {
        return Vec::new();
    }
    // SAFETY: a live array of dictionaries, read by index.
    let count = unsafe { CFArrayGetCount(list) };
    (0..count)
        .map(|i| {
            // SAFETY: `i` is in range.
            let entry = unsafe { CFArrayGetValueAtIndex(list, i) };
            raw_of(entry.cast())
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The PAC run, on this thread's own run loop
// ---------------------------------------------------------------------------

/// What the callback leaves for the loop that waits on it. Cells: the
/// callback writes through a shared pointer while the loop reads.
struct PacState {
    done: std::cell::Cell<bool>,
    answer: std::cell::RefCell<Option<Result<Vec<Entry>, String>>>,
}

extern "C" fn pac_answered(client: *mut c_void, list: CFArrayRef, error: CFErrorRef) {
    // SAFETY: `client` is the `PacState` on the stack of `run_pac`, alive
    // until the source is invalidated there, on this same thread.
    let state = unsafe { &*client.cast::<PacState>() };
    state.done.set(true);
    *state.answer.borrow_mut() = Some(if error.is_null() {
        Ok(raws_of(list)
            .into_iter()
            .filter_map(|raw| match raw {
                Raw::Entry(entry) => Some(entry),
                // A PAC answer names proxies, never another PAC file.
                _ => None,
            })
            .collect())
    } else {
        // SAFETY: a live CFError for the length of the callback.
        Err(format!("CFNetwork error {}", unsafe {
            CFErrorGetCode(error)
        }))
    });
    // SAFETY: stops the run loop of this thread, which is running it.
    unsafe { CFRunLoopStop(CFRunLoopGetCurrent()) };
}

/// Run one PAC file (or script) for `target`, on THIS thread's run loop, in a
/// private mode, for at most [`PAC_BUDGET`].
fn run_pac(pac: &Raw, target: &str) -> Result<Vec<Entry>, String> {
    let target = cf_url(target).ok_or_else(|| "not a URL".to_owned())?;
    let state = PacState {
        done: std::cell::Cell::new(false),
        answer: std::cell::RefCell::new(None),
    };
    let mut context = StreamClientContext {
        version: 0,
        info: std::ptr::from_ref(&state).cast_mut().cast(),
        retain: None,
        release: None,
        copy_description: None,
    };
    // Held until the run is over: CFNetwork reads them while it runs.
    let (_pac_url, _script, source) = match pac {
        Raw::PacUrl(url) => {
            let pac_url = cf_url(url).ok_or_else(|| "the PAC address is not a URL".to_owned())?;
            // SAFETY: live URLs and a context that outlives the source.
            let source = unsafe {
                CFNetworkExecuteProxyAutoConfigurationURL(
                    pac_url.0.cast(),
                    target.0.cast(),
                    pac_answered,
                    &raw mut context,
                )
            };
            (Some(pac_url), None, source)
        }
        Raw::PacScript(script) => {
            let script = CFString::new(script);
            // SAFETY: as above.
            let source = unsafe {
                CFNetworkExecuteProxyAutoConfigurationScript(
                    script.as_concrete_TypeRef(),
                    target.0.cast(),
                    pac_answered,
                    &raw mut context,
                )
            };
            (None, Some(script), source)
        }
        _ => return Err("not a PAC entry".to_owned()),
    };
    if source.is_null() {
        return Err("CFNetwork would not run it".to_owned());
    }
    let mode = CFString::new("app.getvela.wallet.pac");
    let deadline = Instant::now() + PAC_BUDGET;
    // SAFETY: this thread's run loop, a live source, a private mode; the
    // source is removed and invalidated before `state` goes out of scope, so
    // the callback can never run after it.
    unsafe {
        let run_loop = CFRunLoopGetCurrent();
        CFRunLoopAddSource(run_loop, source, mode.as_concrete_TypeRef());
        while !state.done.get() {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            CFRunLoopRunInMode(mode.as_concrete_TypeRef(), left.as_secs_f64(), 1);
        }
        CFRunLoopRemoveSource(run_loop, source, mode.as_concrete_TypeRef());
        CFRunLoopSourceInvalidate(source);
        CFRelease(source.cast());
    }
    state
        .answer
        .into_inner()
        .unwrap_or_else(|| Err(format!("no answer in {} s", PAC_BUDGET.as_secs())))
}

/// The routes for `target` under `settings`: the system's list, with every
/// PAC entry replaced by what the PAC file says for this URL. A PAC entry
/// that cannot be run is a failure — never a silent DIRECT (ruling 3, RD9).
fn resolve(
    settings: &Settings,
    target: &str,
    pac_cache: &mut HashMap<String, (Instant, Result<Vec<Entry>, PacFailed>)>,
) -> Result<Vec<Entry>, PacFailed> {
    let Some(url) = cf_url(target) else {
        return Ok(vec![Entry::Direct]);
    };
    // SAFETY: live URL and dictionary; the list is a Copy, owned below.
    let list = unsafe { CFNetworkCopyProxiesForURL(url.0.cast(), settings.as_dictionary()) };
    let _owned = Owned::new(list.cast());
    let mut out = Vec::new();
    for raw in raws_of(list) {
        match raw {
            Raw::Entry(entry) => out.push(entry),
            Raw::Other => {}
            pac @ (Raw::PacUrl(_) | Raw::PacScript(_)) => {
                let key = pac_key(target);
                let fresh = pac_cache.get(&key).filter(|(at, answer)| {
                    at.elapsed()
                        < if answer.is_ok() {
                            PAC_TTL
                        } else {
                            PAC_FAILURE_TTL
                        }
                });
                let answer = match fresh {
                    Some((_, answer)) => answer.clone(),
                    None => {
                        let name = match &pac {
                            Raw::PacUrl(url) => crate::diag::host_of(url),
                            _ => "script".to_owned(),
                        };
                        let answer =
                            run_pac(&pac, target).map_err(|reason| PacFailed { pac: name, reason });
                        pac_cache.insert(key, (Instant::now(), answer.clone()));
                        answer
                    }
                };
                out.extend(answer?);
            }
        }
    }
    // One route once: CFNetwork lists DIRECT after a PAC entry even when the
    // PAC answer already ends in DIRECT.
    let mut seen = Vec::with_capacity(out.len());
    out.retain(|entry| {
        let fresh = !seen.contains(entry);
        if fresh {
            seen.push(entry.clone());
        }
        fresh
    });
    Ok(out)
}

/// A PAC answer is per scheme and host (the path never changes what a
/// sensible PAC file says, and caching by it would run the file per call).
fn pac_key(url: &str) -> String {
    let scheme = url.split_once("://").map_or("", |(scheme, _)| scheme);
    format!("{scheme}://{}", crate::diag::host_of(url))
}

// ---------------------------------------------------------------------------
// The `vela-pac` thread
// ---------------------------------------------------------------------------

struct Job {
    url: String,
    reply: Sender<Result<Vec<Entry>, PacFailed>>,
}

fn jobs() -> Option<Sender<Job>> {
    static JOBS: OnceLock<Mutex<Option<Sender<Job>>>> = OnceLock::new();
    let slot = JOBS.get_or_init(|| {
        let (tx, rx) = channel::<Job>();
        let spawned = std::thread::Builder::new()
            .name("vela-pac".to_owned())
            .spawn(move || serve(&rx));
        Mutex::new(spawned.ok().map(|_| tx))
    });
    slot.lock().ok()?.clone()
}

fn serve(rx: &Receiver<Job>) {
    let mut settings: Option<(Instant, Settings)> = None;
    let mut pac_cache = HashMap::new();
    while let Ok(job) = rx.recv() {
        let stale = settings
            .as_ref()
            .is_none_or(|(at, _)| at.elapsed() >= SETTINGS_TTL);
        if stale {
            settings = Settings::system().map(|fresh| (Instant::now(), fresh));
        }
        let answer = match &settings {
            Some((_, settings)) => resolve(settings, &job.url, &mut pac_cache),
            // No settings at all: the system has no proxy to name.
            None => Ok(vec![Entry::Direct]),
        };
        let _ = job.reply.send(answer);
    }
}

/// The system's routes for `url`, in order. Blocks the caller until the
/// `vela-pac` thread answers — at most a PAC run.
pub fn routes_for(url: &str) -> Result<Vec<Entry>, PacFailed> {
    let (reply, answer) = channel();
    let sent = jobs().is_some_and(|tx| {
        tx.send(Job {
            url: url.to_owned(),
            reply,
        })
        .is_ok()
    });
    if !sent {
        return Err(PacFailed {
            pac: "system".to_owned(),
            reason: "the proxy resolver is not running".to_owned(),
        });
    }
    answer
        .recv_timeout(PAC_BUDGET + Duration::from_secs(2))
        .unwrap_or_else(|_| {
            Err(PacFailed {
                pac: "system".to_owned(),
                reason: "the proxy resolver did not answer".to_owned(),
            })
        })
}

/// The routes for `url` under settings a test built, resolved on the
/// `vela-pac` thread like the real ones.
#[cfg(test)]
pub fn routes_under(settings: Settings, url: &str) -> Result<Vec<Entry>, PacFailed> {
    let (reply, answer) = channel();
    let url = url.to_owned();
    // Its own thread with its own run loop, as `vela-pac` has: the test's
    // thread is not one to run a CF run loop on either.
    let worker = std::thread::spawn(move || {
        let _ = reply.send(resolve(&settings, &url, &mut HashMap::new()));
    });
    let out = answer
        .recv_timeout(PAC_BUDGET + Duration::from_secs(2))
        .unwrap_or_else(|_| {
            Err(PacFailed {
                pac: "test".to_owned(),
                reason: "no answer".to_owned(),
            })
        });
    let _ = worker.join();
    out
}

// SAFETY: `Settings` crosses to the worker thread in the test seam only; an
// immutable CF dictionary is safe to use from another thread.
unsafe impl Send for Settings {}

#[cfg(test)]
mod tests {
    use super::*;
    use core_foundation::array::CFArray;
    use core_foundation::base::CFType;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::number::CFNumber;

    fn settings(pairs: &[(&str, CFType)]) -> Settings {
        let pairs: Vec<(CFType, CFType)> = pairs
            .iter()
            .map(|(key, value)| (CFString::new(key).as_CFType(), value.clone()))
            .collect();
        Settings::from_dictionary(&CFDictionary::from_CFType_pairs(&pairs))
    }

    fn n(value: i32) -> CFType {
        CFNumber::from(value).as_CFType()
    }

    fn s(value: &str) -> CFType {
        CFString::new(value).as_CFType()
    }

    /// The HTTPS proxy, its exception list and "exclude simple hostnames",
    /// as CFNetwork applies them — the same answer WebKit gets.
    fn with_exceptions() -> Settings {
        let exceptions: CFArray<CFString> =
            CFArray::from_CFTypes(&[CFString::new("*.local"), CFString::new("internal.example")]);
        settings(&[
            ("HTTPSEnable", n(1)),
            ("HTTPSProxy", s("127.0.0.1")),
            ("HTTPSPort", n(1088)),
            ("HTTPEnable", n(1)),
            ("HTTPProxy", s("127.0.0.1")),
            ("HTTPPort", n(1088)),
            ("ExcludeSimpleHostnames", n(1)),
            ("ExceptionsList", exceptions.as_CFType()),
        ])
    }

    #[test]
    fn a_public_host_goes_through_the_system_proxy() {
        assert_eq!(
            routes_under(with_exceptions(), "https://rpc.gnosischain.com/"),
            Ok(vec![Entry::Http {
                host: "127.0.0.1".to_owned(),
                port: 1088
            }])
        );
    }

    #[test]
    fn exceptions_and_simple_hostnames_go_direct() {
        for url in [
            "https://printer.local/",
            "https://internal.example/x",
            "http://intranet/",
        ] {
            assert_eq!(
                routes_under(with_exceptions(), url),
                Ok(vec![Entry::Direct]),
                "{url}"
            );
        }
    }

    #[test]
    fn no_proxy_configured_is_direct() {
        assert_eq!(
            routes_under(settings(&[("HTTPSEnable", n(0))]), "https://example.com/"),
            Ok(vec![Entry::Direct])
        );
    }

    /// A PAC script runs in CFNetwork's own engine, on this module's thread,
    /// and its answer is the route list — DIRECT only where it says DIRECT.
    #[test]
    fn a_pac_script_names_the_routes() {
        let script = "function FindProxyForURL(url, host) { return 'PROXY 10.0.0.1:3128; SOCKS 10.0.0.2:1080; DIRECT'; }";
        let answer = routes_under(
            settings(&[
                ("ProxyAutoConfigEnable", n(1)),
                ("ProxyAutoConfigJavaScript", s(script)),
            ]),
            "https://example.com/",
        );
        println!("pac script: {answer:?}");
        let routes = answer.unwrap_or_else(|failure| unreachable!("{failure:?}"));
        assert_eq!(
            routes.first(),
            Some(&Entry::Http {
                host: "10.0.0.1".to_owned(),
                port: 3128
            })
        );
        assert!(routes.contains(&Entry::Socks {
            host: "10.0.0.2".to_owned(),
            port: 1080
        }));
        assert_eq!(routes.last(), Some(&Entry::Direct));
        assert_eq!(routes.len(), 3, "one route once: {routes:?}");
    }

    /// A PAC file nobody serves is a failure, not a silent DIRECT.
    #[test]
    fn a_pac_file_that_cannot_be_fetched_is_a_failure() {
        let closed = std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|listener| listener.local_addr())
            .map(|addr| addr.port())
            .unwrap_or(1);
        let answer = routes_under(
            settings(&[
                ("ProxyAutoConfigEnable", n(1)),
                (
                    "ProxyAutoConfigURLString",
                    s(&format!("http://127.0.0.1:{closed}/proxy.pac")),
                ),
            ]),
            "https://example.com/",
        );
        println!("dead pac: {answer:?}");
        let failure = answer.expect_err("no DIRECT behind a PAC that cannot run");
        assert_eq!(failure.pac, format!("127.0.0.1:{closed}"));
    }
}
