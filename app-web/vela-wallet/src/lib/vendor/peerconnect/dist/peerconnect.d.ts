/* tslint:disable */
/* eslint-disable */

/**
 * The peer state machine (see `peerconnect::Peer`). Times are milliseconds
 * on a monotonic clock.
 */
export class Peer {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * A peer for a relay that requires a token not fetched yet: it stays
     * offline until `setToken`.
     */
    static awaitingToken(room: Room, now: number): Peer;
    close(): void;
    handleBinary(now: number, frame: Uint8Array): void;
    handleClose(now: number, code?: number | null): void;
    handleOpen(now: number): void;
    handleText(now: number, text: string): void;
    handleTimeout(now: number): void;
    leave(): void;
    /**
     * `token`: for a relay that requires one (SPEC §4.1).
     */
    constructor(room: Room, now: number, token?: string | null);
    /**
     * The next output as `{ type, ... }`, or `undefined`:
     * `connect {url}`, `send {data}`, `ping`, `disconnect`,
     * `state {state, reason?}`, `message {data}`, `delivered {id}`,
     * `paired`, `replaced`, `unauthorized`.
     */
    pollOutput(): any;
    pollTimeout(): number | undefined;
    room(): Room;
    /**
     * Queues a message and returns its id; throws if it is over 16 MiB,
     * if too much is unacknowledged, or if the room is closed or closing.
     */
    send(data: Uint8Array): number;
    /**
     * Replaces the relay token; after `unauthorized`, connects at once.
     */
    setToken(now: number, token?: string | null): void;
    /**
     * Bytes of messages waiting to be acknowledged by the peer.
     */
    readonly queued: number;
}

/**
 * Room credentials (see `peerconnect::Room`).
 */
export class Room {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Creates a room on a relay (`ws://` or `wss://` URL); you are the inviter.
     */
    static create(relay: string): Room;
    /**
     * Accepts an invite; you are the joiner. Save the room before connecting:
     * it holds the identity the inviter will pin.
     */
    static join(invite: string): Room;
    /**
     * Restores a room saved with `toString()`.
     */
    static parse(record: string): Room;
    /**
     * The room record, to persist (it is a secret).
     */
    toString(): string;
    /**
     * Whether the room is being ended.
     */
    readonly closing: boolean;
    readonly id: string;
    /**
     * The invite to hand to the other peer, until the inviter has paired.
     */
    readonly invite: string | undefined;
    /**
     * Whether pairing is complete (the room has moved to its pair ID).
     */
    readonly paired: boolean;
    readonly peerKey: Uint8Array | undefined;
    readonly publicKey: Uint8Array;
    readonly relay: string;
    /**
     * `"inviter"` or `"joiner"`.
     */
    readonly role: string;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_peer_free: (a: number, b: number) => void;
    readonly __wbg_room_free: (a: number, b: number) => void;
    readonly peer_awaitingToken: (a: number, b: number) => number;
    readonly peer_close: (a: number) => void;
    readonly peer_handleBinary: (a: number, b: number, c: number, d: number) => void;
    readonly peer_handleClose: (a: number, b: number, c: number) => void;
    readonly peer_handleOpen: (a: number, b: number) => void;
    readonly peer_handleText: (a: number, b: number, c: number, d: number) => void;
    readonly peer_handleTimeout: (a: number, b: number) => void;
    readonly peer_leave: (a: number) => void;
    readonly peer_new: (a: number, b: number, c: number, d: number) => number;
    readonly peer_pollOutput: (a: number) => any;
    readonly peer_pollTimeout: (a: number) => [number, number];
    readonly peer_queued: (a: number) => number;
    readonly peer_room: (a: number) => number;
    readonly peer_send: (a: number, b: number, c: number) => [number, number, number];
    readonly peer_setToken: (a: number, b: number, c: number, d: number) => void;
    readonly room_closing: (a: number) => number;
    readonly room_create: (a: number, b: number) => [number, number, number];
    readonly room_id: (a: number) => [number, number];
    readonly room_invite: (a: number) => [number, number];
    readonly room_join: (a: number, b: number) => [number, number, number];
    readonly room_paired: (a: number) => number;
    readonly room_parse: (a: number, b: number) => [number, number, number];
    readonly room_peerKey: (a: number) => [number, number];
    readonly room_publicKey: (a: number) => [number, number];
    readonly room_relay: (a: number) => [number, number];
    readonly room_role: (a: number) => [number, number];
    readonly room_toString: (a: number) => [number, number];
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
