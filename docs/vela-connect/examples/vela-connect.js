/**
 * Vela Connect v1 — a reference client, in one file, with no dependencies.
 *
 * This is NOT a package. It is the protocol (../PROTOCOL.md) written out once,
 * so you can read how each rule looks in code and copy what you need into your
 * own app. Everything here is plain browser JavaScript.
 *
 *   const vela = new VelaPopup();
 *   button.onclick = async () => {
 *     if (!vela.open()) return redirectToVela({ method: 'eth_requestAccounts' }); // popup blocked
 *     const [account] = await vela.request('eth_requestAccounts');
 *   };
 */

/** The Vela web wallet. Messages come from, and go to, this origin only. */
export const VELA_WALLET = 'https://wallet.getvela.app';

/** An EIP-1193 error, as Vela answers it (PROTOCOL.md §6). */
export class VelaError extends Error {
	constructor(code, message, data) {
		super(message);
		this.name = 'VelaError';
		this.code = code;
		if (data !== undefined) this.data = data;
	}
}

/** Methods whose unanswered request may still have happened (§6). */
const SENDING = new Set(['eth_sendTransaction', 'wallet_sendCalls']);

// ---------------------------------------------------------------------------
// Popup mode (§3)
// ---------------------------------------------------------------------------

export class VelaPopup {
	/**
	 * @param {{ wallet?: string, readyTimeoutMs?: number }} [options]
	 *   `wallet`: the wallet's origin (your local Vela while developing).
	 */
	constructor({ wallet = VELA_WALLET, readyTimeoutMs = 30_000 } = {}) {
		this.origin = new URL(wallet).origin;
		this.readyTimeoutMs = readyTimeoutMs;
		/** @type {Window | null} */
		this.window = null;
		this.ready = false;
		this.pending = new Map(); // id → { method, params, resolve, reject }
		this.nextId = 1;
		this.onMessage = this.onMessage.bind(this);
	}

	/**
	 * Open Vela. Call it SYNCHRONOUSLY in a click handler — no `await` before
	 * it — or the browser blocks the window (§3.1). `false`: it was blocked;
	 * use redirect mode instead.
	 */
	open() {
		if (this.window && !this.window.closed) {
			this.window.focus();
			return true;
		}
		const opened = window.open(`${this.origin}/dapp`, 'vela-connect', 'popup,width=420,height=720');
		if (!opened) return false;
		this.window = opened;
		this.ready = false;
		window.addEventListener('message', this.onMessage);
		// No `ready` in time: Vela did not load (§3.2).
		this.readyTimer = setTimeout(() => {
			if (!this.ready) this.end(new VelaError(4900, 'Vela did not load'));
		}, this.readyTimeoutMs);
		// The person may close the window at any time (§3.5).
		this.closedTimer = setInterval(() => {
			if (this.window?.closed) this.end();
		}, 500);
		return true;
	}

	/**
	 * Ask Vela — an EIP-1193 request. Open the window first (in the same
	 * click); requests sent before Vela is ready wait for it.
	 */
	request(method, params = []) {
		if (!this.window || this.window.closed) {
			return Promise.reject(new VelaError(4900, 'Vela is not open — call open() in a click first'));
		}
		const id = `r${this.nextId++}`;
		return new Promise((resolve, reject) => {
			this.pending.set(id, { method, params, resolve, reject });
			if (this.ready) this.post(id);
		});
	}

	/** Close Vela's window (when your flow is done, §3.5). */
	close() {
		this.window?.close();
		this.end();
	}

	post(id) {
		const { method, params } = this.pending.get(id);
		// Never '*': the request goes to Vela's origin only.
		this.window.postMessage({ vela: 1, type: 'request', id, method, params }, this.origin);
	}

	onMessage(event) {
		// Only Vela's window, from Vela's origin.
		if (event.source !== this.window || event.origin !== this.origin) return;
		const message = event.data;
		if (!message || message.vela !== 1) return;
		if (message.type === 'ready') {
			// First ready, or ready again after the person created a wallet:
			// send everything not answered yet (§3.2). Vela never answers an id twice.
			this.ready = true;
			clearTimeout(this.readyTimer);
			for (const id of this.pending.keys()) this.post(id);
			return;
		}
		if (message.type === 'response') {
			const waiting = this.pending.get(message.id);
			if (!waiting) return;
			this.pending.delete(message.id);
			if (message.error) {
				const { code, message: text, data } = message.error;
				waiting.reject(new VelaError(code, text, data));
			} else {
				waiting.resolve(message.result);
			}
		}
	}

	/**
	 * The window is gone. What it did not answer is UNKNOWN for a payment —
	 * it may have been sent, so check before asking again — and declined for
	 * everything else (§6).
	 */
	end(reason) {
		clearTimeout(this.readyTimer);
		clearInterval(this.closedTimer);
		window.removeEventListener('message', this.onMessage);
		for (const { method, reject } of this.pending.values()) {
			reject(
				reason ??
					(SENDING.has(method)
						? new VelaError(4900, 'Vela closed before answering — the payment may have been sent; check before retrying')
						: new VelaError(4001, 'Vela closed before answering'))
			);
		}
		this.pending.clear();
		this.window = null;
		this.ready = false;
	}
}

// ---------------------------------------------------------------------------
// Redirect mode (§4) — when a popup cannot open
// ---------------------------------------------------------------------------

const STATE_KEY = 'vela-connect.state';

function base64url(json) {
	const bytes = new TextEncoder().encode(JSON.stringify(json));
	let binary = '';
	for (const byte of bytes) binary += String.fromCharCode(byte);
	return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

function fromBase64url(text) {
	const padded = text.replace(/-/g, '+').replace(/_/g, '/');
	const binary = atob(padded + '='.repeat((4 - (padded.length % 4)) % 4));
	return JSON.parse(new TextDecoder().decode(Uint8Array.from(binary, (c) => c.charCodeAt(0))));
}

/**
 * Leave for Vela with ONE request; the answer comes back to `returnUrl`
 * (default: this page). Read it there with `takeVelaAnswer()`.
 */
export function redirectToVela({ method, params = [], returnUrl = location.href, wallet = VELA_WALLET }) {
	const random = crypto.getRandomValues(new Uint8Array(24));
	const state = btoa(String.fromCharCode(...random)).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
	sessionStorage.setItem(STATE_KEY, state);
	const back = new URL(returnUrl);
	back.hash = '';
	const fragment = new URLSearchParams({
		request: base64url({ method, params }),
		return: back.toString(),
		state
	});
	location.assign(`${new URL(wallet).origin}/dapp#${fragment}`);
}

/**
 * The answer Vela brought back in this page's fragment: `{ result }`, or
 * throws a `VelaError`; `null` when there is none. An answer whose `state`
 * is not the one this page kept is thrown away — you did not ask for it (§4.3).
 */
export function takeVelaAnswer() {
	if (!location.hash.startsWith('#vela=')) return null;
	const encoded = location.hash.slice('#vela='.length);
	history.replaceState(history.state, '', location.pathname + location.search);
	const expected = sessionStorage.getItem(STATE_KEY);
	sessionStorage.removeItem(STATE_KEY);
	let answer;
	try {
		answer = fromBase64url(encoded);
	} catch {
		return null;
	}
	if (!expected || answer?.vela !== 1 || answer.state !== expected) return null;
	if (answer.error) throw new VelaError(answer.error.code, answer.error.message, answer.error.data);
	return { result: answer.result };
}
