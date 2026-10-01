/**
 * The life of one extension request, rule by rule (spec 082 RB1–RB11,
 * data-model §4).
 *
 * `extension/lib/request-life.js` is what the service worker performs: which
 * request a panel shows, whether a claim is live, what a restarted worker
 * keeps, what a browser event ends, where a request is shown and what the page
 * is told. Each rule is a pure function, so each branch of the data model's
 * tables is one case here.
 */
import { describe, expect, it } from 'vitest';
import {
	affectedBy,
	callerOwns,
	claimVerdict,
	deadlineOf,
	maybeSentHash,
	newRecord,
	nextForWindow,
	recoveryPlan,
	settlement,
	surfaceAfterOpen,
	surfaceAnswer,
	withClaim
} from '../../../extension/lib/request-life.js';
import {
	ERR,
	NOT_CONFIRMED_MESSAGE,
	READ_DROPPED_MESSAGE,
	REQUEST_TTL_MS,
	SETTLE,
	droppedChannelAnswer
} from '../../../extension/lib/protocol.js';

const NOW = 1_800_000_000_000;

type Rec = ReturnType<typeof newRecord> & {
	claimedAt?: number;
	phase?: string;
	opHash?: string;
	chainId?: number;
};

function record(over: Partial<Rec> = {}): Rec {
	return {
		v: 1,
		rid: '7:page:1',
		id: 'page:1',
		method: 'personal_sign',
		params: [],
		origin: 'https://a.example',
		tabId: 7,
		windowId: 3,
		documentId: 'doc-a',
		sentAt: NOW - 1_000,
		at: NOW - 1_000,
		surface: 'panel',
		surfaceWindowId: 3,
		state: 'shown',
		...over
	} as Rec;
}

const PANEL = { kind: 'panel', windowId: 3 } as const;

describe('a new record', () => {
	it('keys it by tab and page id, and takes the browser’s facts', () => {
		const r = newRecord({
			request: { id: 'u:1', method: 'personal_sign', params: ['0x00'], sentAt: NOW - 500 },
			sender: { origin: 'https://a.example', documentId: 'doc-a', tab: { id: 7, windowId: 3 } },
			now: NOW,
			surface: 'panel',
			surfaceWindowId: 3
		});
		expect(r.rid).toBe('7:u:1');
		expect(r.documentId).toBe('doc-a');
		expect(r.windowId).toBe(3);
		expect(r.state).toBe('created');
		expect(r.at).toBe(NOW - 500);
	});

	it('never lets a page buy time with a send time in the future', () => {
		const r = newRecord({
			request: { id: 'u:1', method: 'personal_sign', params: [], sentAt: NOW + 60_000 },
			sender: { origin: 'https://a.example', documentId: 'd', tab: { id: 1, windowId: 1 } },
			now: NOW,
			surface: 'panel',
			surfaceWindowId: 1
		});
		expect(r.at).toBe(NOW);
	});
});

describe('one queue per window (RB7)', () => {
	it('serves the oldest request of the window, whichever tab asked', () => {
		const a = record({ rid: '7:a', tabId: 7, at: NOW - 2_000 });
		const b = record({ rid: '9:b', tabId: 9, at: NOW - 1_000 });
		expect(nextForWindow([b, a], 3)?.rid).toBe('7:a');
	});

	it('does not serve another window’s requests, nor a dedicated window’s', () => {
		const other = record({ rid: '1:x', surfaceWindowId: 4 });
		const windowed = record({ rid: '1:y', surface: 'window', surfaceWindowId: 99 });
		expect(nextForWindow([other, windowed], 3)).toBeNull();
	});
});

describe('the claim verdict (RB5)', () => {
	const base = { now: NOW, ttlMs: REQUEST_TTL_MS, docAttached: true, caller: PANEL };

	it('is live for approve and sign on a fresh request of this surface', () => {
		expect(claimVerdict(record(), { ...base, phase: 'approve' })).toEqual({ live: true });
		expect(claimVerdict(record(), { ...base, phase: 'sign' })).toEqual({ live: true });
	});

	it('is not live when the record is gone', () => {
		expect(claimVerdict(null, { ...base, phase: 'sign' })).toEqual({ live: false, cause: 'gone' });
	});

	it('is not live for another surface', () => {
		expect(
			claimVerdict(record(), { ...base, caller: { kind: 'panel', windowId: 4 }, phase: 'sign' })
		).toEqual({ live: false, cause: 'wrong_surface' });
		expect(
			claimVerdict(record(), {
				...base,
				caller: { kind: 'window', rid: '7:page:1' },
				phase: 'sign'
			})
		).toEqual({ live: false, cause: 'wrong_surface' });
	});

	it('is not live when the page is gone', () => {
		expect(claimVerdict(record(), { ...base, docAttached: false, phase: 'sign' })).toEqual({
			live: false,
			cause: 'page_left'
		});
	});

	it('refuses approve and sign at the 5-minute limit (RB11)', () => {
		const old = record({ at: NOW - REQUEST_TTL_MS });
		expect(claimVerdict(old, { ...base, phase: 'sign' })).toEqual({
			live: false,
			cause: 'expired'
		});
		const nearly = record({ at: NOW - REQUEST_TTL_MS + 1 });
		expect(claimVerdict(nearly, { ...base, phase: 'approve' })).toEqual({ live: true });
	});

	it('needs an earlier claim for submit, and ignores the age there', () => {
		expect(claimVerdict(record(), { ...base, phase: 'submit' })).toEqual({
			live: false,
			cause: 'not_claimed'
		});
		const claimedLongAgo = record({ state: 'claimed', at: NOW - 2 * REQUEST_TTL_MS });
		expect(claimVerdict(claimedLongAgo, { ...base, phase: 'submit' })).toEqual({ live: true });
	});

	it('knows a dedicated window owns only its own request', () => {
		const windowed = record({ surface: 'window', surfaceWindowId: 42 });
		expect(callerOwns(windowed, { kind: 'window', rid: '7:page:1' })).toBe(true);
		expect(callerOwns(windowed, { kind: 'window', rid: '7:other' })).toBe(false);
		expect(callerOwns(windowed, PANEL)).toBe(false);
	});
});

describe('recovery at worker start (RB4)', () => {
	const facts = {
		now: NOW,
		ttlMs: REQUEST_TTL_MS,
		panelWindows: new Set([3]),
		openWindows: new Set([3, 42])
	};

	it('settles a request past its limit as expired', () => {
		const old = record({ at: NOW - REQUEST_TTL_MS - 1 });
		expect(recoveryPlan([old], facts)).toEqual([
			{ rid: old.rid, action: 'settle', cause: 'expired' }
		]);
	});

	it('counts a claimed request from its claim', () => {
		const claimed = record({ state: 'claimed', at: NOW - REQUEST_TTL_MS - 10, claimedAt: NOW - 1 });
		expect(deadlineOf(claimed)).toBe(NOW - 1 + REQUEST_TTL_MS);
		expect(recoveryPlan([claimed], facts)).toEqual([{ rid: claimed.rid, action: 'probe' }]);
	});

	it('settles a panel request whose window has no side panel any more', () => {
		const r = record({ surfaceWindowId: 5 });
		expect(recoveryPlan([r], facts)).toEqual([
			{ rid: r.rid, action: 'settle', cause: 'surface_closed' }
		]);
	});

	it('settles a window request whose window is gone', () => {
		const gone = record({ rid: '1:g', surface: 'window', surfaceWindowId: 77 });
		const unknown = record({ rid: '1:u', surface: 'window', surfaceWindowId: undefined });
		expect(recoveryPlan([gone, unknown], facts)).toEqual([
			{ rid: '1:g', action: 'settle', cause: 'surface_closed' },
			{ rid: '1:u', action: 'settle', cause: 'surface_closed' }
		]);
	});

	it('probes the page for everything else, and settles nothing it could not check', () => {
		const panel = record({ rid: '1:p' });
		const windowed = record({ rid: '1:w', surface: 'window', surfaceWindowId: 42 });
		expect(recoveryPlan([panel, windowed], facts)).toEqual([
			{ rid: '1:p', action: 'probe' },
			{ rid: '1:w', action: 'probe' }
		]);
		const blind = { ...facts, panelWindows: null, openWindows: null };
		expect(recoveryPlan([record({ surfaceWindowId: 5 })], blind)).toEqual([
			{ rid: '7:page:1', action: 'probe' }
		]);
	});
});

describe('what a browser event ends (data-model §4)', () => {
	const a = record({ rid: '7:a', documentId: 'doc-a', tabId: 7 });
	const b = record({ rid: '9:b', documentId: 'doc-b', tabId: 9 });
	const w = record({ rid: '9:w', surface: 'window', surfaceWindowId: 42, tabId: 9 });

	it('a page port closing ends that document’s requests: page_left', () => {
		expect(affectedBy([a, b], { type: 'doc_closed', documentId: 'doc-a' })).toEqual([
			{ rid: '7:a', cause: 'page_left' }
		]);
	});

	it('a tab removed or replaced ends its requests: page_left', () => {
		expect(affectedBy([a, b, w], { type: 'tab_removed', tabId: 9 })).toEqual([
			{ rid: '9:b', cause: 'page_left' },
			{ rid: '9:w', cause: 'page_left' }
		]);
		expect(affectedBy([a], { type: 'tab_replaced', tabId: 7 })).toEqual([
			{ rid: '7:a', cause: 'page_left' }
		]);
	});

	it('the panel closing ends every request its window owed: surface_closed (RB10)', () => {
		expect(affectedBy([a, b, w], { type: 'surface_closed', caller: PANEL })).toEqual([
			{ rid: '7:a', cause: 'surface_closed' },
			{ rid: '9:b', cause: 'surface_closed' }
		]);
	});

	it('a window closing ends its dedicated request and its panel’s', () => {
		expect(affectedBy([a, w], { type: 'window_removed', windowId: 42 })).toEqual([
			{ rid: '9:w', cause: 'surface_closed' }
		]);
		expect(affectedBy([a, w], { type: 'window_removed', windowId: 3 })).toEqual([
			{ rid: '7:a', cause: 'surface_closed' }
		]);
	});
});

/**
 * RJ2 (G35, P0): once the panel's claim for SUBMIT carries the operation
 * hash, the bytes may already be on their way. The page must then hear that
 * hash — ruling 1 + RA2, "may have been sent" — never 4900, which a dApp reads
 * as "not sent" and pays again. Before the hash exists (nothing signed or
 * sent), 4900 stays right.
 */
describe('a claimed submit that may have been sent (RJ2, G35)', () => {
	const OP = `0x${'ab'.repeat(32)}`;
	const submitting = (over: Partial<Rec> = {}) =>
		record({
			rid: '7:tx',
			method: 'eth_sendTransaction',
			state: 'claimed',
			claimedAt: NOW - 500,
			phase: 'submit',
			opHash: OP,
			chainId: 100,
			...over
		} as Partial<Rec>);

	it('a live submit claim keeps the operation hash and its chain on the record', () => {
		const signed = withClaim(record(), { phase: 'sign', now: NOW });
		expect(signed).toMatchObject({ state: 'claimed', claimedAt: NOW, phase: 'sign' });
		expect(signed).not.toHaveProperty('opHash');
		const sent = withClaim(signed, { phase: 'submit', now: NOW + 1, opHash: OP, chainId: 100 });
		expect(sent).toMatchObject({ phase: 'submit', opHash: OP, chainId: 100, claimedAt: NOW + 1 });
		expect(maybeSentHash(sent)).toBe(OP);
	});

	it('keeps no hash that is not one, and no chain that is not one', () => {
		const sent = withClaim(record({ state: 'claimed' }), {
			phase: 'submit',
			now: NOW,
			opHash: '0x1234',
			chainId: -1
		});
		expect(sent).not.toHaveProperty('opHash');
		expect(sent).not.toHaveProperty('chainId');
		expect(maybeSentHash(sent)).toBeNull();
	});

	it('names the hash only for a claimed submit that carried one', () => {
		expect(maybeSentHash(submitting())).toBe(OP);
		expect(maybeSentHash(submitting({ phase: 'sign' } as Partial<Rec>))).toBeNull();
		expect(maybeSentHash(submitting({ opHash: undefined } as Partial<Rec>))).toBeNull();
		expect(maybeSentHash(submitting({ state: 'shown' }))).toBeNull();
		expect(maybeSentHash(null)).toBeNull();
	});

	// 083, owner ruling 2026-10-01: never 4900 ("not sent"), never the op
	// hash as if it were a transaction.
	const NOT_CONFIRMED = {
		error: { code: -32603, message: `${NOT_CONFIRMED_MESSAGE} (user operation ${OP})` }
	};

	it('the panel closing answers the claimed submit "not confirmed yet", and settles the rest', () => {
		const owed = record({ rid: '7:sign', state: 'claimed', phase: 'sign' } as Partial<Rec>);
		expect(affectedBy([submitting(), owed], { type: 'surface_closed', caller: PANEL })).toEqual([
			{ rid: '7:tx', cause: 'surface_closed', answer: NOT_CONFIRMED, opHash: OP },
			{ rid: '7:sign', cause: 'surface_closed' }
		]);
	});

	it('a window closing does the same', () => {
		expect(affectedBy([submitting()], { type: 'window_removed', windowId: 3 })).toEqual([
			{ rid: '7:tx', cause: 'surface_closed', answer: NOT_CONFIRMED, opHash: OP }
		]);
	});

	it('the page leaving is still page_left: nobody is there to hear a hash', () => {
		expect(affectedBy([submitting()], { type: 'doc_closed', documentId: 'doc-a' })).toEqual([
			{ rid: '7:tx', cause: 'page_left' }
		]);
	});

	it('a restarted worker answers it "not confirmed yet" when its panel is gone or its time is up', () => {
		const facts = {
			now: NOW,
			ttlMs: REQUEST_TTL_MS,
			panelWindows: new Set<number>(),
			openWindows: new Set([3])
		};
		const signOnly = record({ rid: '7:sign', state: 'claimed', phase: 'sign' } as Partial<Rec>);
		expect(recoveryPlan([submitting(), signOnly], facts)).toEqual([
			{ rid: '7:tx', action: 'settle', cause: 'surface_closed', answer: NOT_CONFIRMED, opHash: OP },
			{ rid: '7:sign', action: 'settle', cause: 'surface_closed' }
		]);
		const late = submitting({ claimedAt: NOW - REQUEST_TTL_MS - 1 });
		expect(recoveryPlan([late], { ...facts, panelWindows: new Set([3]) })).toEqual([
			{ rid: '7:tx', action: 'settle', cause: 'expired', answer: NOT_CONFIRMED, opHash: OP }
		]);
	});

	it('the surface’s own close settlement (4900) becomes "not confirmed yet"; a real answer goes as given', () => {
		const closing = {
			error: { code: 4900, message: 'The browser closed before the request finished' }
		};
		expect(surfaceAnswer(submitting(), closing)).toEqual({
			payload: NOT_CONFIRMED,
			maybeSent: OP
		});
		// A batch's id IS the op hash (EIP-5792): a batch still gets it.
		const batch = submitting({ method: 'wallet_sendCalls' } as Partial<Rec>);
		expect(surfaceAnswer(batch, closing)).toEqual({ payload: { result: OP }, maybeSent: OP });
		// Before the hash exists nothing was sent: 4900 is right.
		const signOnly = record({ state: 'claimed', phase: 'sign' } as Partial<Rec>);
		expect(surfaceAnswer(signOnly, closing)).toEqual({ payload: closing, maybeSent: null });
		// RJ3's refusal, a 4001 and a result are the surface's answer, not a loss.
		const refused = { error: { code: -32603, message: 'refused' } };
		expect(surfaceAnswer(submitting(), refused)).toEqual({ payload: refused, maybeSent: null });
		const rejected = { error: { code: 4001, message: 'rejected' } };
		expect(surfaceAnswer(submitting(), rejected)).toEqual({ payload: rejected, maybeSent: null });
		const ok = { result: `0x${'cd'.repeat(32)}` };
		expect(surfaceAnswer(submitting(), ok)).toEqual({ payload: ok, maybeSent: null });
		expect(SETTLE.surface_closed.code).toBe(4900);
	});
});

describe('where a request is shown (RB8)', () => {
	it('stays in the panel once it opened', () => {
		expect(surfaceAfterOpen({ opened: 'ok', panelOpen: false, preference: 'panel' })).toBe('panel');
	});

	it('stays in a panel already open when the open failed or hung (EX2)', () => {
		expect(surfaceAfterOpen({ opened: 'failed', panelOpen: true, preference: 'panel' })).toBe(
			'panel'
		);
		expect(surfaceAfterOpen({ opened: 'timeout', panelOpen: true, preference: 'panel' })).toBe(
			'panel'
		);
	});

	it('falls back to a window when there is no panel to show it in', () => {
		expect(surfaceAfterOpen({ opened: 'failed', panelOpen: false, preference: 'panel' })).toBe(
			'window'
		);
	});

	it('lets a saved window preference win', () => {
		expect(surfaceAfterOpen({ opened: 'ok', panelOpen: true, preference: 'window' })).toBe(
			'window'
		);
	});
});

describe('what the page is told (RB6)', () => {
	it('is 4900 with plain words for every cause, never 4001', () => {
		for (const cause of Object.keys(SETTLE)) {
			const error = settlement(cause);
			expect(error.code, cause).toBe(4900);
			expect(error.message, cause).toBe(SETTLE[cause as keyof typeof SETTLE].message);
		}
		expect(settlement('page_left').message).toBe('The page navigated away');
		expect(settlement('nonsense').message).toBe(SETTLE.surface_closed.message);
	});
});

describe('a dropped channel (data-model §4, G19)', () => {
	it('retries a read once, then answers plain words', () => {
		expect(droppedChannelAnswer('read', 'eth_call', 0)).toEqual({ retry: true });
		expect(droppedChannelAnswer('read', 'eth_call', 1)).toEqual({
			error: { code: ERR.INTERNAL, message: READ_DROPPED_MESSAGE }
		});
	});

	it('never retries a read that sends', () => {
		for (const method of ['eth_sendRawTransaction', 'eth_sendUserOperation']) {
			expect(droppedChannelAnswer('read', method, 0)).toEqual({
				error: { code: 4900, message: SETTLE.restarted.message }
			});
		}
	});

	it('retries a sign or connect once, then answers restarted', () => {
		expect(droppedChannelAnswer('sign', 'personal_sign', 0)).toEqual({ retry: true });
		expect(droppedChannelAnswer('connect', 'eth_requestAccounts', 1)).toEqual({
			error: { code: 4900, message: SETTLE.restarted.message }
		});
	});

	it('never carries Chrome’s own words', () => {
		for (const bucket of ['read', 'sign', 'connect']) {
			for (const attempt of [0, 1, 2]) {
				const answer = droppedChannelAnswer(bucket, 'eth_call', attempt);
				if ('error' in answer) expect(answer.error?.message).not.toMatch(/channel|port|closed/i);
			}
		}
	});
});
