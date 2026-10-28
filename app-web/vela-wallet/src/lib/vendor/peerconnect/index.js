// PeerConnect for JavaScript: browsers, Node.js (22+), Deno and Bun.
//
// The protocol runs in WebAssembly, compiled from the same Rust state
// machine as the native implementation; this file only supplies the
// WebSocket and the timers.

import { initSync, Peer, Room } from "./dist/peerconnect.js";
import module from "./dist/module.js";

initSync({ module: Uint8Array.from(atob(module), (c) => c.charCodeAt(0)) });

export { Room };

/**
 * Connects to a room and keeps it connected until left or closed. A room
 * that is closing carries on ending. `options.token` is for a relay that
 * requires one: a string, or a function returning one (or a promise of
 * one), called before the first connection and again whenever the relay
 * refuses the token.
 */
export function connect(room, options = {}) {
  return new Connection(typeof room === "string" ? Room.parse(room) : room, options);
}

const encoder = new TextEncoder();

export class Connection extends EventTarget {
  /** "offline" | "waiting" | "connected" | "closed" */
  state = "offline";
  /** Why the connection closed: "left" | "closed" | "closed-by-peer". */
  reason = undefined;

  #peer;
  #socket = null;
  #timer = undefined;
  #epoch = performance.now();
  #getToken;
  /** Whether `#getToken` is being asked. */
  #fetching = false;

  constructor(room, { token } = {}) {
    super();
    if (typeof token === "function") {
      this.#getToken = token;
      // Offline until the first token arrives: the relay would refuse us.
      this.#peer = Peer.awaitingToken(room, this.#now());
      queueMicrotask(() => this.#fetchToken());
    } else {
      this.#peer = new Peer(room, this.#now(), token ?? undefined);
      // Let the caller attach listeners before anything happens.
      queueMicrotask(() => this.#pump());
    }
  }

  /** Replaces the relay token, used from the next connection on (at once if the relay refused the last one). */
  setToken(token) {
    this.#peer.setToken(this.#now(), token || undefined);
    this.#pump();
  }

  async #fetchToken() {
    if (this.#fetching) return;
    this.#fetching = true;
    let token;
    try {
      token = await this.#getToken();
    } catch (error) {
      this.dispatchEvent(new CustomEvent("tokenerror", { detail: error }));
    } finally {
      this.#fetching = false;
    }
    if (this.state === "closed") return;
    if (typeof token === "string" && token) this.setToken(token);
    else setTimeout(() => this.state !== "closed" && this.#fetchToken(), 30_000);
  }

  /** Bytes of messages waiting to be acknowledged by the peer. */
  get queued() {
    return this.#peer.queued;
  }

  /** The room as it stands now; save `room.toString()` after "paired" and after `close()`. */
  get room() {
    return this.#peer.room();
  }

  /**
   * Queues a message (bytes, or a string as UTF-8) and returns its id. It is
   * delivered as soon as the peer is reachable; "delivered" events report
   * progress. Throws if it is over 16 MiB, if 64 MiB are already waiting for
   * acknowledgement, or if the room is closed or closing.
   */
  send(data) {
    const bytes =
      typeof data === "string" ? encoder.encode(data) : data instanceof Uint8Array ? data : new Uint8Array(data);
    const id = this.#peer.send(bytes);
    this.#pump();
    return id;
  }

  /** Disconnects, keeping the room for later. Unacknowledged messages are dropped. */
  leave() {
    this.#peer.leave();
    this.#pump();
  }

  /**
   * Ends the room for both peers. The connection carries on until the peer
   * has been told, then its state becomes "closed" with reason "closed".
   * The room now says it is closing: save it, so that connecting with it
   * later finishes the job if this does not.
   */
  close() {
    this.#peer.close();
    this.#pump();
  }

  #now() {
    return performance.now() - this.#epoch;
  }

  #pump() {
    for (let out; (out = this.#peer.pollOutput()); ) {
      switch (out.type) {
        case "connect":
          this.#open(out.url);
          break;
        case "send":
          this.#socket?.send(out.data);
          break;
        case "ping":
          this.#socket?.send("ping");
          break;
        case "disconnect":
          this.#disconnect();
          break;
        case "state":
          this.state = out.state;
          this.reason = out.reason;
          this.dispatchEvent(new Event("state"));
          break;
        case "message":
          this.dispatchEvent(new MessageEvent("message", { data: out.data }));
          break;
        case "delivered":
          this.dispatchEvent(new CustomEvent("delivered", { detail: out.id }));
          break;
        case "paired":
        case "replaced":
          this.dispatchEvent(new Event(out.type));
          break;
        case "unauthorized":
          this.dispatchEvent(new Event("unauthorized"));
          if (this.#getToken) this.#fetchToken();
          break;
      }
    }
    clearTimeout(this.#timer);
    const at = this.#peer.pollTimeout();
    if (at !== undefined) {
      this.#timer = setTimeout(() => {
        this.#peer.handleTimeout(this.#now());
        this.#pump();
      }, Math.max(0, at - this.#now()));
    }
  }

  #open(url) {
    const socket = new WebSocket(url);
    socket.binaryType = "arraybuffer";
    this.#socket = socket;
    const live = (handler) => (event) => {
      if (this.#socket !== socket) return;
      handler(event);
      this.#pump();
    };
    socket.onopen = live(() => this.#peer.handleOpen(this.#now()));
    socket.onmessage = live((event) =>
      typeof event.data === "string"
        ? this.#peer.handleText(this.#now(), event.data)
        : this.#peer.handleBinary(this.#now(), new Uint8Array(event.data)),
    );
    socket.onclose = live((event) => {
      this.#socket = null;
      this.#peer.handleClose(this.#now(), event.code);
    });
  }

  #disconnect() {
    const socket = this.#socket;
    this.#socket = null;
    if (socket) {
      socket.onopen = socket.onmessage = socket.onclose = null;
      socket.close(1000);
    }
  }
}
