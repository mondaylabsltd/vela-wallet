//! The CTAP 2.3 BLE-only data channel on macOS: an L2CAP connection-oriented
//! channel to the authenticator, over CoreBluetooth. No tunnel server, no relay,
//! no internet — which is the whole point: it works where the WebSocket tunnel
//! (Google's `cable.ua5v.com`) is unreachable.
//!
//! btleplug has no L2CAP, so this talks to CoreBluetooth directly through
//! `objc2`. The flow mirrors the founder's proven demos
//! (`transport/ble/CableConn.kt` initiator side): the scan already found the
//! peripheral and read the PSM from its advert suffix; here we connect that
//! peripheral, open the L2CAP channel to the PSM, and present its input/output
//! streams as a [`CablePort`] with the demo's 4-byte big-endian length framing.
//!
//! ## Threading — why a dedicated thread, not the ceremony's
//!
//! The ceremony runs on one of gpui's background-executor threads, which are
//! **libdispatch worker threads**. Pumping a nested `CFRunLoop`
//! (`runMode:beforeDate:`) on such a thread corrupts its run loop: the next task
//! libdispatch schedules onto that same worker — `CheckPasskeySupport`, whose
//! `hidapi::HidApi::new()` calls `IOHIDManagerScheduleWithRunLoop` — then traps
//! in `CFRunLoopAddSource` (`__CFCheckCFInfoPACSignature`). caBLE looked fine;
//! the app died a step later signing in with a USB key.
//!
//! So every CoreBluetooth object and every run-loop pump lives on a PRIVATE
//! `std::thread` this port spawns and owns. Its run loop is created, pumped, and
//! torn down entirely there, never touching a shared worker. The ceremony thread
//! only sends `Write`/`Read` commands down a channel and blocks on the reply —
//! the objc types (all `!Send`) never cross the boundary.

#![allow(unexpected_cfgs)]

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use std::ptr::NonNull;

use objc2::{ClassType, DeclaredClass, declare_class, msg_send_id, mutability};
use objc2_core_bluetooth::{
    CBCentralManager, CBCentralManagerDelegate, CBL2CAPChannel, CBManagerState, CBPeripheral,
    CBPeripheralDelegate,
};
use objc2_foundation::{
    NSArray, NSDefaultRunLoopMode, NSInputStream, NSObject, NSObjectProtocol, NSOutputStream,
    NSRunLoop, NSStreamStatus, NSString, NSUUID,
};

use vela_core::cable::conn::CablePort;
use vela_core::ctap::hid_cable::PortError;

use super::HybridError;

/// How long to wait for each async CoreBluetooth step (power-on, connect, open).
const STEP_TIMEOUT: Duration = Duration::from_secs(20);

/// How long a single frame's read/write may take before the peer is called dead.
const IO_TIMEOUT: Duration = Duration::from_secs(130);

/// The most a single L2CAP frame may claim, so a corrupt length prefix cannot
/// ask us to allocate the world. caBLE messages are small.
const MAX_FRAME: usize = 1 << 20;

/// What the delegate hands back to the connecting thread.
#[derive(Default)]
struct Shared {
    powered_on: bool,
    connected: bool,
    channel: Option<Retained<CBL2CAPChannel>>,
    failed: Option<String>,
}

struct Signal {
    state: Mutex<Shared>,
    cond: Condvar,
}

impl Signal {
    // The `Arc` is the delegate's ivar type (objc2's `declare_class!` holds it);
    // `Signal` is `!Send` only through the `Retained` channel it carries, which
    // the connecting thread takes once it is open — the Mutex/Condvar pair is
    // the only thing shared.
    #[allow(clippy::arc_with_non_send_sync)]
    fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(Shared::default()),
            cond: Condvar::new(),
        })
    }
}

declare_class!(
    /// The CoreBluetooth delegate: forwards the three async milestones (state,
    /// connect, channel open) into the [`Signal`] the connecting thread waits on.
    struct Delegate;

    unsafe impl ClassType for Delegate {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "VelaCableL2capDelegate";
    }

    impl DeclaredClass for Delegate {
        type Ivars = Arc<Signal>;
    }

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl CBCentralManagerDelegate for Delegate {
        #[method(centralManagerDidUpdateState:)]
        fn did_update_state(&self, central: &CBCentralManager) {
            let powered = unsafe { central.state() } == CBManagerState::PoweredOn;
            let signal = self.ivars();
            if let Ok(mut state) = signal.state.lock() {
                if powered {
                    state.powered_on = true;
                } else {
                    state.failed = Some("Bluetooth is not powered on".to_owned());
                }
            }
            signal.cond.notify_all();
        }

        #[method(centralManager:didConnectPeripheral:)]
        fn did_connect(&self, _central: &CBCentralManager, _peripheral: &CBPeripheral) {
            let signal = self.ivars();
            if let Ok(mut state) = signal.state.lock() {
                state.connected = true;
            }
            signal.cond.notify_all();
        }

        #[method(centralManager:didFailToConnectPeripheral:error:)]
        fn did_fail_connect(
            &self,
            _central: &CBCentralManager,
            _peripheral: &CBPeripheral,
            _error: Option<&AnyObject>,
        ) {
            let signal = self.ivars();
            if let Ok(mut state) = signal.state.lock() {
                state.failed = Some("could not connect to the phone over Bluetooth".to_owned());
            }
            signal.cond.notify_all();
        }
    }

    unsafe impl CBPeripheralDelegate for Delegate {
        #[method(peripheral:didOpenL2CAPChannel:error:)]
        fn did_open_channel(
            &self,
            _peripheral: &CBPeripheral,
            channel: Option<&CBL2CAPChannel>,
            error: Option<&AnyObject>,
        ) {
            let signal = self.ivars();
            if let Ok(mut state) = signal.state.lock() {
                if error.is_some() || channel.is_none() {
                    state.failed = Some("the phone refused the L2CAP channel".to_owned());
                } else if let Some(channel) = channel {
                    state.channel = Some(channel.retain());
                }
            }
            signal.cond.notify_all();
        }
    }
);

impl Delegate {
    fn new(signal: Arc<Signal>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(signal);
        unsafe { msg_send_id![super(this), init] }
    }
}

/// A command from the ceremony thread to the private Bluetooth thread.
enum Cmd {
    /// Write these bytes in full.
    Write(Vec<u8>),
    /// Read exactly this many bytes.
    Read(usize),
    /// Tear the connection down and end the thread.
    Shutdown,
}

/// The private thread's reply to a [`Cmd`].
enum Reply {
    Wrote(Result<(), PortError>),
    Bytes(Result<Vec<u8>, PortError>),
}

/// One L2CAP CoC to a caBLE authenticator, as the core's [`CablePort`]. Framing
/// is a 4-byte big-endian length prefix + payload, matching the demos.
///
/// This handle lives on the ceremony thread; the CoreBluetooth objects live on
/// the private thread behind `cmd_tx`/`reply_rx`.
pub struct L2capCablePort {
    cmd_tx: Sender<Cmd>,
    reply_rx: Receiver<Reply>,
    worker: Option<JoinHandle<()>>,
}

impl L2capCablePort {
    /// Connect the L2CAP CoC to `psm` on the peripheral named by `uuid` (the
    /// string btleplug reported for the matched advert). Spawns the private
    /// Bluetooth thread and blocks until it has the channel open or has failed.
    pub fn connect(uuid: &str, psm: u16) -> Result<Self, HybridError> {
        let (cmd_tx, cmd_rx) = channel::<Cmd>();
        let (reply_tx, reply_rx) = channel::<Reply>();
        let (ready_tx, ready_rx) = channel::<Result<(), HybridError>>();
        let uuid = uuid.to_owned();

        let worker = std::thread::Builder::new()
            .name("vela-l2cap".to_owned())
            .spawn(move || worker_main(&uuid, psm, &ready_tx, &cmd_rx, &reply_tx))
            .map_err(|error| {
                HybridError::Bluetooth(format!("could not start the Bluetooth thread: {error}"))
            })?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                cmd_tx,
                reply_rx,
                worker: Some(worker),
            }),
            Ok(Err(error)) => {
                let _ = worker.join();
                Err(error)
            }
            Err(_) => {
                let _ = worker.join();
                Err(HybridError::Bluetooth(
                    "the Bluetooth thread stopped before connecting".to_owned(),
                ))
            }
        }
    }

    /// Send one command and wait for its reply, mapping a dead thread to an I/O
    /// error rather than a panic.
    fn request(&mut self, cmd: Cmd) -> Result<Reply, PortError> {
        self.cmd_tx
            .send(cmd)
            .map_err(|_| PortError::Io("the L2CAP thread has gone".to_owned()))?;
        self.reply_rx
            .recv()
            .map_err(|_| PortError::Io("the L2CAP thread has gone".to_owned()))
    }
}

impl CablePort for L2capCablePort {
    fn write_frame(&mut self, frame: &[u8]) -> Result<(), PortError> {
        #[allow(clippy::cast_possible_truncation)]
        let len = frame.len() as u32;
        let mut framed = Vec::with_capacity(frame.len() + 4);
        framed.extend_from_slice(&len.to_be_bytes());
        framed.extend_from_slice(frame);
        match self.request(Cmd::Write(framed))? {
            Reply::Wrote(result) => result,
            Reply::Bytes(_) => Err(PortError::Io("L2CAP reply out of order".to_owned())),
        }
    }

    fn read_frame(&mut self) -> Result<Vec<u8>, PortError> {
        let header = match self.request(Cmd::Read(4))? {
            Reply::Bytes(result) => result?,
            Reply::Wrote(_) => return Err(PortError::Io("L2CAP reply out of order".to_owned())),
        };
        let Ok(header) = <[u8; 4]>::try_from(header.as_slice()) else {
            return Err(PortError::Io("short L2CAP length prefix".to_owned()));
        };
        let len = u32::from_be_bytes(header) as usize;
        if len > MAX_FRAME {
            return Err(PortError::Io(format!("L2CAP frame too large: {len}")));
        }
        match self.request(Cmd::Read(len))? {
            Reply::Bytes(result) => result,
            Reply::Wrote(_) => Err(PortError::Io("L2CAP reply out of order".to_owned())),
        }
    }

    fn channel(&self) -> &str {
        "L2CAP"
    }
}

impl Drop for L2capCablePort {
    fn drop(&mut self) {
        // Ask the private thread to close the streams and end; it tears its run
        // loop down cleanly on its own stack. Joining keeps the objc teardown
        // from racing the next ceremony.
        let _ = self.cmd_tx.send(Cmd::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

// ---------------------------------------------------------------------------
// The private Bluetooth thread
// ---------------------------------------------------------------------------

/// The CoreBluetooth objects, owned entirely by the private thread.
struct Conn {
    // Held so CoreBluetooth keeps the connection alive for the connection's life.
    _manager: Retained<CBCentralManager>,
    _delegate: Retained<Delegate>,
    _channel: Retained<CBL2CAPChannel>,
    input: Retained<NSInputStream>,
    output: Retained<NSOutputStream>,
}

/// The private thread's entry point: connect, announce the result, then serve
/// read/write commands until asked to shut down. Every objc call — and every
/// run-loop pump — happens here, on a thread we own.
fn worker_main(
    uuid: &str,
    psm: u16,
    ready_tx: &Sender<Result<(), HybridError>>,
    cmd_rx: &Receiver<Cmd>,
    reply_tx: &Sender<Reply>,
) {
    let mut conn = match Conn::connect(uuid, psm) {
        Ok(conn) => conn,
        Err(error) => {
            let _ = ready_tx.send(Err(error));
            return;
        }
    };
    if ready_tx.send(Ok(())).is_err() {
        return;
    }

    while let Ok(cmd) = cmd_rx.recv() {
        let reply = match cmd {
            Cmd::Write(bytes) => Reply::Wrote(conn.write_all(&bytes)),
            Cmd::Read(len) => Reply::Bytes(conn.read_exact(len)),
            Cmd::Shutdown => break,
        };
        if reply_tx.send(reply).is_err() {
            break;
        }
    }
    // `conn` drops here, on this thread: the streams are unscheduled from THIS
    // run loop and closed, and the run loop dies with the thread.
}

impl Conn {
    fn connect(uuid: &str, psm: u16) -> Result<Self, HybridError> {
        let signal = Signal::new();
        let delegate = Delegate::new(Arc::clone(&signal));
        let delegate_proto = ProtocolObject::from_ref(&*delegate);

        let manager: Retained<CBCentralManager> = unsafe {
            let alloc = CBCentralManager::alloc();
            msg_send_id![alloc, initWithDelegate: delegate_proto, queue: std::ptr::null::<AnyObject>()]
        };

        pump_until(&signal, |s| match &s.failed {
            Some(detail) => Err(HybridError::Bluetooth(detail.clone())),
            None => Ok(s.powered_on),
        })?;

        // Retrieve the peripheral CoreBluetooth already discovered during the
        // scan, by the identifier btleplug reported.
        let ns_uuid = NSUUID::from_string(&NSString::from_str(uuid))
            .ok_or_else(|| HybridError::Bluetooth(format!("not a peripheral UUID: {uuid}")))?;
        let ids = NSArray::from_slice(&[&*ns_uuid]);
        let peripherals: Retained<NSArray<CBPeripheral>> =
            unsafe { msg_send_id![&manager, retrievePeripheralsWithIdentifiers: &*ids] };
        let peripheral = peripherals
            .first()
            .ok_or_else(|| HybridError::Bluetooth("the phone is no longer in range".to_owned()))?;
        let peripheral: Retained<CBPeripheral> = peripheral.retain();
        unsafe { peripheral.setDelegate(Some(delegate_proto)) };

        unsafe { manager.connectPeripheral_options(&peripheral, None) };
        pump_until(&signal, |s| match &s.failed {
            Some(detail) => Err(HybridError::Bluetooth(detail.clone())),
            None => Ok(s.connected),
        })?;

        unsafe { peripheral.openL2CAPChannel(psm) };
        pump_until(&signal, |s| match &s.failed {
            Some(detail) => Err(HybridError::Bluetooth(detail.clone())),
            None => Ok(s.channel.is_some()),
        })?;

        let channel = {
            let guard = signal
                .state
                .lock()
                .map_err(|_| HybridError::Bluetooth("Bluetooth lock poisoned".to_owned()))?;
            guard
                .channel
                .clone()
                .ok_or_else(|| HybridError::Bluetooth("no L2CAP channel".to_owned()))?
        };

        let input = unsafe { channel.inputStream() }
            .ok_or_else(|| HybridError::Bluetooth("L2CAP channel has no input".to_owned()))?;
        let output = unsafe { channel.outputStream() }
            .ok_or_else(|| HybridError::Bluetooth("L2CAP channel has no output".to_owned()))?;

        // Schedule the streams on THIS thread's run loop, then open them; the
        // read/write helpers pump this same loop so bytes actually move.
        let run_loop = unsafe { NSRunLoop::currentRunLoop() };
        unsafe {
            input.scheduleInRunLoop_forMode(&run_loop, NSDefaultRunLoopMode);
            output.scheduleInRunLoop_forMode(&run_loop, NSDefaultRunLoopMode);
            input.open();
            output.open();
        }

        Ok(Self {
            _manager: manager,
            _delegate: delegate,
            _channel: channel,
            input,
            output,
        })
    }

    /// Write exactly `buf`, pumping the run loop until the stream accepts space.
    fn write_all(&mut self, buf: &[u8]) -> Result<(), PortError> {
        let mut sent = 0;
        let deadline = Instant::now() + IO_TIMEOUT;
        while sent < buf.len() {
            if Instant::now() >= deadline {
                return Err(PortError::TimedOut);
            }
            if unsafe { self.output.hasSpaceAvailable() } {
                let Some(ptr) = NonNull::new(buf[sent..].as_ptr().cast_mut()) else {
                    return Ok(());
                };
                let n = unsafe { self.output.write_maxLength(ptr, buf.len() - sent) };
                if n > 0 {
                    #[allow(clippy::cast_sign_loss)]
                    {
                        sent += n as usize;
                    }
                    continue;
                }
                if n < 0 {
                    return Err(PortError::Io("L2CAP write failed".to_owned()));
                }
            }
            pump_run_loop();
        }
        Ok(())
    }

    /// Read exactly `len` bytes, pumping the run loop until they arrive.
    fn read_exact(&mut self, len: usize) -> Result<Vec<u8>, PortError> {
        let mut buf = vec![0u8; len];
        let mut got = 0;
        let deadline = Instant::now() + IO_TIMEOUT;
        while got < buf.len() {
            if Instant::now() >= deadline {
                return Err(PortError::TimedOut);
            }
            if unsafe { self.input.hasBytesAvailable() } {
                let Some(ptr) = NonNull::new(buf[got..].as_mut_ptr()) else {
                    return Ok(buf);
                };
                let n = unsafe { self.input.read_maxLength(ptr, buf.len() - got) };
                if n > 0 {
                    #[allow(clippy::cast_sign_loss)]
                    {
                        got += n as usize;
                    }
                    continue;
                }
                if n == 0 {
                    return Err(PortError::Io("L2CAP channel closed".to_owned()));
                }
                return Err(PortError::Io("L2CAP read failed".to_owned()));
            }
            let status = unsafe { self.input.streamStatus() };
            if status == NSStreamStatus::Closed || status == NSStreamStatus::Error {
                return Err(PortError::Io("L2CAP channel closed".to_owned()));
            }
            pump_run_loop();
        }
        Ok(buf)
    }
}

impl Drop for Conn {
    fn drop(&mut self) {
        // Runs on the private thread that scheduled them, so currentRunLoop is
        // the loop the streams sit on. Unschedule before closing so no dangling
        // source is left behind.
        let run_loop = unsafe { NSRunLoop::currentRunLoop() };
        unsafe {
            self.input
                .removeFromRunLoop_forMode(&run_loop, NSDefaultRunLoopMode);
            self.output
                .removeFromRunLoop_forMode(&run_loop, NSDefaultRunLoopMode);
            self.input.close();
            self.output.close();
        }
    }
}

/// Run the current run loop for a short slice so CoreBluetooth delegate and
/// stream events are delivered. Only ever called on the private thread.
fn pump_run_loop() {
    let run_loop = unsafe { NSRunLoop::currentRunLoop() };
    let until = unsafe { objc2_foundation::NSDate::dateWithTimeIntervalSinceNow(0.02) };
    unsafe {
        run_loop.runMode_beforeDate(NSDefaultRunLoopMode, &until);
    }
}

/// Pump the run loop until `ready` returns `Ok(true)` or times out. Only ever
/// called on the private thread.
fn pump_until<F>(signal: &Arc<Signal>, mut ready: F) -> Result<(), HybridError>
where
    F: FnMut(&Shared) -> Result<bool, HybridError>,
{
    let deadline = Instant::now() + STEP_TIMEOUT;
    loop {
        {
            let guard = signal
                .state
                .lock()
                .map_err(|_| HybridError::Bluetooth("Bluetooth lock poisoned".to_owned()))?;
            if ready(&guard)? {
                return Ok(());
            }
        }
        if Instant::now() >= deadline {
            return Err(HybridError::Bluetooth(
                "Bluetooth step timed out".to_owned(),
            ));
        }
        pump_run_loop();
    }
}
