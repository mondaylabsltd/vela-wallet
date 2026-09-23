// A mock of the Clear Signer tunnel, faithful to contracts/tunnel.md §1, for the
// page's tests. The real tunnel is Rust (vela-tunnel, in Docker and as a Worker);
// this one exists so the page can be exercised against the same room rules
// without it.
//
//   GET /v1/rooms/{room}?role=requester|signer   WebSocket
//   GET /healthz                                  200 "ok"
//
// · `room` is 22 base64url characters; anything else is an HTTP 400 before the
//   upgrade. A bad role is accepted and closed with 4400.
// · A room holds at most one requester and one signer; a second connection in
//   a taken role is accepted and closed at once with 4409 "role taken".
// · When both roles are present each gets {"v":1,"relay":"joined"}; when one
//   leaves the other gets {"v":1,"relay":"left"} and the room waits.
// · No buffering: a frame sent while the peer is absent is dropped.
// · Every frame (text or binary) reaches the other end byte-for-byte, in order.
// · A frame over 256 KiB → 1009. A room lives 10 minutes → 4408 "expired";
//   an end silent for 120 s → 4408. An empty room is forgotten at once.
// · The tunnel pings every 30 s.
//
// `tap` records every forwarded frame, so a test can check the tunnel only ever
// saw what it should (the hellos in the clear, everything else sealed).
//
//   node samples/mock-tunnel.mjs [port]     — run it on its own
import { createServer } from 'node:http';
import { fileURLToPath } from 'node:url';
import { accept } from './ws-lite.mjs';

const ROOM = /^[A-Za-z0-9_-]{22}$/;

export const LIMITS = {
  maxFrame: 256 * 1024,
  roomLifetimeMs: 10 * 60 * 1000,
  idleMs: 120 * 1000,
  pingMs: 30 * 1000,
};

export function startTunnel(options = {}) {
  const limits = { ...LIMITS, ...(options.limits || {}) };
  const rooms = new Map();
  const tap = [];
  const joined = JSON.stringify({ v: 1, relay: 'joined' });
  const left = JSON.stringify({ v: 1, relay: 'left' });

  function other(role) {
    return role === 'requester' ? 'signer' : 'requester';
  }

  function forget(id) {
    const room = rooms.get(id);
    if (!room) return;
    clearTimeout(room.lifetime);
    rooms.delete(id);
  }

  const server = createServer((req, res) => {
    const url = new URL(req.url, 'http://tunnel');
    if (req.method === 'GET' && url.pathname === '/healthz') {
      res.writeHead(200, { 'content-type': 'text/plain' }).end('ok');
      return;
    }
    res.writeHead(404).end();
  });

  server.on('upgrade', (req, socket, head) => {
    const url = new URL(req.url, 'http://tunnel');
    const match = /^\/v1\/rooms\/([^/]+)$/.exec(url.pathname);
    if (!match || !ROOM.test(match[1])) {
      socket.end('HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n');
      return;
    }
    const id = match[1];
    const role = url.searchParams.get('role');
    const conn = accept(req, socket, head, { maxMessage: limits.maxFrame });
    if (!conn) return;
    if (role !== 'requester' && role !== 'signer') {
      conn.close(4400, 'bad request');
      return;
    }

    let room = rooms.get(id);
    if (!room) {
      room = { requester: null, signer: null, lifetime: null };
      room.lifetime = setTimeout(() => {
        for (const r of ['requester', 'signer']) if (room[r]) room[r].close(4408, 'expired');
        forget(id);
      }, limits.roomLifetimeMs);
      rooms.set(id, room);
    }
    if (room[role]) {
      // The incumbent is untouched.
      conn.close(4409, 'role taken');
      return;
    }
    room[role] = conn;

    let idle = null;
    const touch = () => {
      clearTimeout(idle);
      idle = setTimeout(() => conn.close(4408, 'idle'), limits.idleMs);
    };
    touch();
    const pinger = setInterval(() => conn.ping(), limits.pingMs);

    conn.on('frame', touch);
    conn.on('message', (data, isBinary) => {
      tap.push({ room: id, from: role, isBinary, data: isBinary ? Buffer.from(data) : data });
      const peer = room[other(role)];
      // No buffering: a frame for an absent peer is dropped.
      if (peer) peer.send(isBinary ? data : String(data));
    });
    conn.on('close', () => {
      clearTimeout(idle);
      clearInterval(pinger);
      if (room[role] !== conn) return;
      room[role] = null;
      const peer = room[other(role)];
      if (peer) peer.send(left);
      if (!room.requester && !room.signer) forget(id);
    });

    if (room.requester && room.signer) {
      room.requester.send(joined);
      room.signer.send(joined);
    }
  });

  return new Promise((resolve) => {
    server.listen(options.port || 0, options.host || '127.0.0.1', () => {
      const port = server.address().port;
      resolve({
        port,
        url: `ws://127.0.0.1:${port}`,
        rooms,
        tap,
        close: () => new Promise((done) => {
          for (const room of rooms.values()) {
            clearTimeout(room.lifetime);
            for (const r of ['requester', 'signer']) if (room[r]) room[r].socket.destroy();
          }
          rooms.clear();
          server.close(() => done());
          server.closeAllConnections?.();
        }),
      });
    });
  });
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const tunnel = await startTunnel({ port: Number(process.argv[2]) || 8787 });
  console.log(`mock tunnel on ${tunnel.url}  (GET /healthz, /v1/rooms/{room}?role=…)`);
}
