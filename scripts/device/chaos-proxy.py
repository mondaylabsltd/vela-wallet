#!/usr/bin/env python3
"""A fault-injecting HTTP/CONNECT proxy for device passes (specs 079, 082).

Only the app under test is pointed at it (owner ruling 6): that app's page, its
RPC pool, the relay and the signing page go through it, so a fault can be aimed
at one host and repeated while nothing else on the device notices. The rules
and every row: specs/082-dapp-browser-mac-ext-ios/quickstart.md §0.

Run:   CHAOS_UPSTREAM=127.0.0.1:1088 python3 scripts/device/chaos-proxy.py chaos.log
       (CHAOS_UPSTREAM: an HTTP proxy the Mac itself needs to reach the internet;
        unset = connect directly.  CHAOS_BIND=0.0.0.0 to serve a phone on the LAN.)

Point ONE app at it, never a device:
Desktop:   a dev-fixtures build launched with VELA_DEV_PROXY=127.0.0.1:8899 sends
           its page AND wallet traffic here and nothing else on the Mac.
iPhone:    a Debug build launched with "VELA_DEV_PROXY":"<mac-lan-ip>:8899" in the
           devicectl -e JSON (per-app ProxyConfiguration), with CHAOS_BIND=0.0.0.0.
Extension: Chrome for Testing in its own --user-data-dir, started with
           --proxy-server=http://127.0.0.1:8899 (never the owner's Chrome profile).
Android:   no fault rows; the only switch that exists is device-wide (082 RH3).
Never (each one moves every app's traffic, ruling 6): the macOS system proxy or
network location, the iPhone's Settings > Wi-Fi > Configure Proxy, Shadowrocket
toggles, airplane mode, or `adb shell settings put global http_proxy`.

Control: curl 'http://127.0.0.1:8899/__chaos?mode=drop&match=vela-relay'
         (CHAOS_PORT=<n> runs another instance on its own port: one per client under test.)
  mode      pass | latency | throttle | drop | blackhole | reset_mid | mute | stall
            (mute: the request reaches the host, its reply never comes back;
             stall: CONNECT is answered 200 and the upstream is never opened)
  latency   ms added before the upstream connect (latency / throttle)
  bps       bytes/second each direction (throttle)
  drop      probability 0..1 that a matching connection is refused (drop)
  match     regex on host; only matching hosts get the fault ('' = all)
  GET /__chaos with no query prints the config. Switching a fault on also cuts
  the live tunnels it matches — a real drop does not spare open connections.
Every connection is logged: time, host:port, verdict.
"""
import asyncio, json, os, random, re, sys, time, urllib.parse

CFG = {"mode": "pass", "latency": 0, "bps": 0, "drop": 1.0, "match": ""}
UPSTREAM = os.environ.get("CHAOS_UPSTREAM", "")
BIND = os.environ.get("CHAOS_BIND", "127.0.0.1")
# One proxy per client under test, so a fault set for one never reaches another (spec 082).
PORT = int(os.environ.get("CHAOS_PORT", "8899"))
LIVE = set()  # (host, client_writer, upstream_writer)
LOG = open(sys.argv[1] if len(sys.argv) > 1 else "chaos.log", "a", buffering=1)


def log(*a):
    LOG.write(time.strftime("%H:%M:%S ") + " ".join(str(x) for x in a) + "\n")


def applies(host):
    return CFG["mode"] != "pass" and (not CFG["match"] or re.search(CFG["match"], host))


class Mute:
    """`mute`: the request goes through, the answer never comes back — the lost
    reply of a relay that accepted an operation (spec 082 W1). Reads the TLS
    records the client sends; once the client's first application-data record
    after the handshake has gone upstream, every byte from upstream is dropped.
    TLS 1.3 sends the client's Finished as application data (0x17) right after
    its ChangeCipherSpec, so that one record is skipped; TLS 1.2 sends it as
    handshake (0x16). Plain http: the request is the first client write."""

    def __init__(self, tls):
        self.tls, self.buf, self.after_ccs, self.skip_finished, self.on = tls, b"", False, False, False

    def client_sent(self, chunk):
        if self.on:
            return
        if not self.tls:
            self.on = True
            return
        self.buf += chunk
        while len(self.buf) >= 5:
            kind, size = self.buf[0], int.from_bytes(self.buf[3:5], "big")
            if len(self.buf) < 5 + size:
                break
            self.buf = self.buf[5 + size:]
            if kind == 0x14:
                self.after_ccs = True
            elif self.after_ccs and kind == 0x17 and not self.skip_finished:
                self.skip_finished = True  # TLS 1.3 Finished
            elif self.after_ccs and kind == 0x16:
                self.skip_finished = True  # TLS 1.2 Finished
            elif self.after_ccs and kind == 0x17:
                self.on = True
                return


async def pipe(r, w, bps, mute=None, upstream=False):
    try:
        while True:
            chunk = await r.read(16384 if not bps else max(1, min(16384, bps // 10)))
            if not chunk:
                break
            if mute is not None:
                if upstream:
                    mute.client_sent(chunk)
                elif mute.on:
                    continue
            w.write(chunk)
            await w.drain()
            if bps:
                await asyncio.sleep(len(chunk) / bps)
    except Exception:
        pass
    finally:
        try:
            w.close()
        except Exception:
            pass


async def swallow(r):
    """Read and drop whatever the client sends until it gives up."""
    while await r.read(16384):
        pass


async def handle(cr, cw):
    try:
        head = await cr.readuntil(b"\r\n\r\n")
    except Exception:
        cw.close()
        return
    line = head.split(b"\r\n", 1)[0].decode("latin1")
    method, target, _ = (line.split(" ") + ["", ""])[:3]
    if target.startswith("/__chaos"):
        q = urllib.parse.parse_qs(urllib.parse.urlparse(target).query)
        for k, v in q.items():
            CFG[k] = type(CFG.get(k, ""))(v[0]) if k in CFG else v[0]
        log("CONFIG", json.dumps(CFG))
        cut = [c for c in list(LIVE) if applies(c[0])]
        for h, a, b in cut:
            for w in (a, b):
                try: w.close()
                except Exception: pass
            LIVE.discard((h, a, b))
        if cut: log("CUT", len(cut), "live tunnels:", ",".join(sorted({c[0] for c in cut})))
        body = json.dumps(CFG).encode()
        cw.write(b"HTTP/1.1 200 OK\r\nContent-Length: %d\r\n\r\n" % len(body) + body)
        await cw.drain()
        cw.close()
        return
    if method == "CONNECT":
        host, _, port = target.rpartition(":")
        port = int(port or 443)
        rest = b""
    else:
        u = urllib.parse.urlparse(target)
        host, port = u.hostname or "", u.port or 80
        path = (u.path or "/") + (("?" + u.query) if u.query else "")
        rest = head.replace(target.encode(), path.encode(), 1)
    fault = applies(host)
    verdict = CFG["mode"] if fault else "pass"
    if fault and CFG["mode"] == "drop" and random.random() < CFG["drop"]:
        log("DROP", f"{host}:{port}")
        cw.close()
        return
    if fault and CFG["mode"] == "blackhole":
        log("HOLE", f"{host}:{port}")
        await asyncio.sleep(600)
        cw.close()
        return
    if fault and CFG["mode"] == "stall":
        # The tunnel is "established" and nothing ever comes through it: what a
        # TUN-mode proxy client with a dead node does (spec 082 W24). Unlike
        # blackhole, the client sees its CONNECT succeed.
        log("STALL", f"{host}:{port}")
        if method == "CONNECT":
            cw.write(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            await cw.drain()
        entry = (host, cw, cw)
        LIVE.add(entry)
        try:
            await asyncio.wait_for(swallow(cr), 600)
        except Exception:
            pass
        finally:
            LIVE.discard(entry)
            cw.close()
        return
    if fault and CFG["latency"]:
        await asyncio.sleep(CFG["latency"] / 1000)
    t0 = time.time()
    try:
        if UPSTREAM:
            # Through the proxy this machine itself needs to reach the internet.
            up_host, _, up_port = UPSTREAM.rpartition(":")
            ur, uw = await asyncio.wait_for(asyncio.open_connection(up_host, int(up_port)), 20)
            uw.write(f"CONNECT {host}:{port} HTTP/1.1\r\nHost: {host}:{port}\r\n\r\n".encode())
            await uw.drain()
            resp = await asyncio.wait_for(ur.readuntil(b"\r\n\r\n"), 20)
            if b" 200" not in resp.split(b"\r\n", 1)[0]:
                raise ConnectionError(resp.split(b"\r\n", 1)[0].decode("latin1"))
        else:
            ur, uw = await asyncio.wait_for(asyncio.open_connection(host, port), 20)
    except Exception as e:
        log("FAIL", f"{host}:{port}", type(e).__name__)
        cw.write(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n")
        cw.close()
        return
    log(verdict.upper(), f"{host}:{port}", f"{int((time.time()-t0)*1000)}ms")
    if method == "CONNECT":
        cw.write(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        await cw.drain()
    else:
        uw.write(rest)
        await uw.drain()
    bps = CFG["bps"] if fault and CFG["mode"] == "throttle" else 0
    if fault and CFG["mode"] == "reset_mid":
        async def cut():
            await asyncio.sleep(1.5)
            log("RESET", f"{host}:{port}")
            cw.close(); uw.close()
        asyncio.create_task(cut())
    mute = Mute(method == "CONNECT") if fault and CFG["mode"] == "mute" else None
    if mute is not None and method != "CONNECT":
        mute.on = True  # plain http: `rest` above already carried the request
    entry = (host, cw, uw)
    LIVE.add(entry)
    try:
        await asyncio.gather(pipe(cr, uw, bps, mute, upstream=True), pipe(ur, cw, bps, mute))
    finally:
        LIVE.discard(entry)


async def main():
    srv = await asyncio.start_server(handle, BIND, PORT)
    log("listening", BIND, PORT, "upstream", UPSTREAM or "direct")
    async with srv:
        await srv.serve_forever()


asyncio.run(main())
