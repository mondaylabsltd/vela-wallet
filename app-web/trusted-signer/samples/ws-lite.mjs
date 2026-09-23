// The server half of RFC 6455, in plain Node, for the samples: the mock tunnel
// and the mock wallets listen with it. Zero dependencies, like the page.
//
//   server.on('upgrade', (req, socket, head) => {
//     const conn = accept(req, socket, head);        // null if not a WebSocket request
//     conn.on('message', (data, isBinary) => …);     // whole messages
//     conn.on('close', (code, reason) => …);
//     conn.send('text') / conn.send(bytes) / conn.ping() / conn.close(4409, 'role taken');
//   });
//
// Only what the samples need: no extensions, no subprotocols. Fragmented
// messages are joined; pings are answered; a message over `maxMessage` bytes
// is closed with 1009, as the tunnel contract says.
import { createHash } from 'node:crypto';
import { EventEmitter } from 'node:events';

const GUID = '258EAFA5-E914-47DA-95CA-C5AB0DC85B11';

export class Conn extends EventEmitter {
  constructor(socket, options = {}) {
    super();
    this.socket = socket;
    this.maxMessage = options.maxMessage || 256 * 1024;
    this.buffer = Buffer.alloc(0);
    this.parts = [];
    this.partsBinary = false;
    this.closed = false;
    this.closeSent = false;
    socket.on('data', (chunk) => this.feed(chunk));
    socket.on('close', () => this.gone(1006, ''));
    socket.on('error', () => this.gone(1006, ''));
  }

  feed(chunk) {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    for (;;) {
      if (this.buffer.length < 2) return;
      const b0 = this.buffer[0];
      const b1 = this.buffer[1];
      const fin = (b0 & 0x80) !== 0;
      const opcode = b0 & 0x0f;
      const masked = (b1 & 0x80) !== 0;
      let length = b1 & 0x7f;
      let offset = 2;
      if (length === 126) {
        if (this.buffer.length < 4) return;
        length = this.buffer.readUInt16BE(2);
        offset = 4;
      } else if (length === 127) {
        if (this.buffer.length < 10) return;
        const big = this.buffer.readBigUInt64BE(2);
        if (big > BigInt(this.maxMessage) + 16n) { this.close(1009, 'too big'); return; }
        length = Number(big);
        offset = 10;
      }
      if (length > this.maxMessage) { this.close(1009, 'too big'); return; }
      if (!masked) { this.close(1002, 'unmasked client frame'); return; }
      if (this.buffer.length < offset + 4 + length) return;
      const mask = this.buffer.subarray(offset, offset + 4);
      const payload = Buffer.alloc(length);
      for (let i = 0; i < length; i++) payload[i] = this.buffer[offset + 4 + i] ^ mask[i % 4];
      this.buffer = this.buffer.subarray(offset + 4 + length);
      this.frame(fin, opcode, payload);
      if (this.closed) return;
    }
  }

  frame(fin, opcode, payload) {
    this.emit('frame', opcode);
    if (opcode === 0x8) {
      const code = payload.length >= 2 ? payload.readUInt16BE(0) : 1005;
      const reason = payload.length > 2 ? payload.subarray(2).toString('utf8') : '';
      if (!this.closeSent) this.write(0x8, payload.subarray(0, 2));
      this.closeSent = true;
      this.socket.end();
      this.gone(code, reason);
      return;
    }
    if (opcode === 0x9) { this.write(0xa, payload); return; }
    if (opcode === 0xa) return;
    if (opcode === 0x1 || opcode === 0x2) {
      this.parts = [payload];
      this.partsBinary = opcode === 0x2;
    } else if (opcode === 0x0) {
      this.parts.push(payload);
    } else {
      this.close(1002, 'unknown opcode');
      return;
    }
    const size = this.parts.reduce((n, p) => n + p.length, 0);
    if (size > this.maxMessage) { this.close(1009, 'too big'); return; }
    if (!fin) return;
    const whole = Buffer.concat(this.parts);
    this.parts = [];
    this.emit('message', this.partsBinary ? whole : whole.toString('utf8'), this.partsBinary);
  }

  write(opcode, payload) {
    if (this.socket.destroyed) return;
    const length = payload.length;
    let head;
    if (length < 126) {
      head = Buffer.from([0x80 | opcode, length]);
    } else if (length < 65536) {
      head = Buffer.alloc(4);
      head[0] = 0x80 | opcode; head[1] = 126; head.writeUInt16BE(length, 2);
    } else {
      head = Buffer.alloc(10);
      head[0] = 0x80 | opcode; head[1] = 127; head.writeBigUInt64BE(BigInt(length), 2);
    }
    this.socket.write(Buffer.concat([head, payload]));
  }

  /** A string goes as a text frame, anything else as a binary frame. */
  send(data) {
    if (this.closed || this.closeSent) return;
    if (typeof data === 'string') this.write(0x1, Buffer.from(data, 'utf8'));
    else this.write(0x2, Buffer.from(data));
  }

  ping() {
    if (!this.closed && !this.closeSent) this.write(0x9, Buffer.alloc(0));
  }

  close(code = 1000, reason = '') {
    if (this.closed) return;
    if (!this.closeSent) {
      const body = Buffer.alloc(2 + Buffer.byteLength(reason));
      body.writeUInt16BE(code, 0);
      body.write(reason, 2);
      this.write(0x8, body);
      this.closeSent = true;
    }
    // Give the peer a moment to answer the close, then drop the socket.
    setTimeout(() => { this.socket.destroy(); this.gone(code, reason); }, 50);
  }

  gone(code, reason) {
    if (this.closed) return;
    this.closed = true;
    this.emit('close', code, reason);
  }
}

/** Answer an HTTP upgrade. Returns a Conn, or null (and a 400) if it is not a WebSocket request. */
export function accept(req, socket, head, options = {}) {
  const key = req.headers['sec-websocket-key'];
  if (!key || String(req.headers.upgrade || '').toLowerCase() !== 'websocket') {
    socket.end('HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n');
    return null;
  }
  const acceptKey = createHash('sha1').update(key + GUID).digest('base64');
  socket.write(
    'HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n' +
    `Sec-WebSocket-Accept: ${acceptKey}\r\n\r\n`,
  );
  socket.setNoDelay(true);
  const conn = new Conn(socket, options);
  if (head && head.length) conn.feed(head);
  return conn;
}
