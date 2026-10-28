/**
 * The Ethereum backup check (spec 062 §5a) — the REAL core, fed the REAL
 * bytes the Gnosis registry answered for wallet 0x88cC…6894
 * (`__fixtures__/registry-backup.gnosis.json`). What is pinned here is the
 * web/core seam: which chain each question goes to, that a bare `0x` reaches
 * the core as an answer, and that silence is never a verdict.
 */
import '$lib/i18n/wasm-init.server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import gnosis from './__fixtures__/registry-backup.gnosis.json';

vi.mock('$lib/core/client', async (original) => ({
	...(await original<typeof import('$lib/core/client')>()),
	loadCore: async () => {}
}));
vi.mock('$lib/services/endpoints', () => ({ getPasskeyIndexURL: () => 'https://index.test' }));
const fetched: string[] = [];
vi.mock('$lib/services/net', () => ({
	NET_TIMEOUTS: { keyIndexRead: 1 },
	fetchWithTimeout: vi.fn(async (url: string) => {
		fetched.push(url);
		return { ok: false, status: 500, text: async () => '' };
	})
}));

const REGISTRY = '0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9';
const SELECTOR = {
	version: '0xffa1ad74',
	groups: '0xcc7aae8e',
	unit: '0x1655bfbe',
	mirrored: '0xa451c8ce',
	payload: '0x19178efe'
};
const word = (n: number) => n.toString(16).padStart(64, '0');
/** `(false, 0, Unit{})` — what a registry that lacks the group answers. */
const NOT_MIRRORED =
	'0x' + word(0) + word(0) + word(0x60) + word(0xc0) + word(0xe0) + word(0x100) + word(0).repeat(6);

/** What each chain says. `null` = the pool reports an error. */
let ethereum: { version: string | null; mirrored: string | null };
let silentOnGnosis: string | null;
/** What Gnosis stores as unit 10's payload — the real bytes unless a test says otherwise. */
let payloadOnGnosis: string;
/** ABI `bytes` of length zero: what a unit from before registry V13 stored. */
const EMPTY_BYTES = '0x' + word(0x20) + word(0);

/** The rows the core says to draw (`registry_backup::BackupState::row`). */
const TITLE = 'settingsModals.backup.title';
/**
 * The paragraph under the Keys block (`registry_backup::EXPLAIN_KEY`): named
 * for every state a copy can still be made or checked in — and for none that
 * can never be copied (PR 3 note 6).
 */
const EXPLAIN = 'settingsModals.backup.explain';
const ROW = {
	backedUp: {
		title_key: TITLE,
		subtitle_key: 'settingsModals.backup.backedUp',
		tone: 'positive',
		action: 'none',
		explain_key: EXPLAIN
	},
	notBackedUp: {
		title_key: TITLE,
		subtitle_key: 'settingsModals.backup.notBackedUp',
		tone: 'neutral',
		action: 'copy',
		explain_key: EXPLAIN
	},
	couldNotCheck: {
		title_key: TITLE,
		subtitle_key: 'settingsModals.backup.couldNotCheck',
		tone: 'neutral',
		action: 'retry',
		explain_key: EXPLAIN
	},
	// No `explain_key`: the core omits it, and no paragraph is drawn.
	cannotCopy: {
		title_key: TITLE,
		subtitle_key: 'settingsModals.backup.cannotCopy',
		tone: 'neutral',
		action: 'none'
	}
} as const;
const asked: string[] = [];

/** The browser's `localStorage`, in memory. */
const stored = new Map<string, string>();
vi.stubGlobal('localStorage', {
	getItem: (key: string) => stored.get(key) ?? null,
	setItem: (key: string, value: string) => void stored.set(key, value)
});

vi.mock('$lib/services/rpc-pool', () => ({
	poolRpcCall: vi.fn(
		async (_method: string, params: [{ to: string; data: string }], chainId: number) => {
			const { to, data } = params[0];
			expect(to).toBe(REGISTRY);
			const selector = data.slice(0, 10);
			asked.push(`${chainId}:${selector}`);
			const answer = (result: string | null) =>
				result === null
					? { jsonrpc: '2.0', id: 1, error: { code: -32000, message: 'down' } }
					: { jsonrpc: '2.0', id: 1, result };
			if (chainId === 1) {
				if (selector === SELECTOR.version) return answer(ethereum.version);
				if (selector === SELECTOR.mirrored) return answer(ethereum.mirrored);
			}
			if (chainId === 100 && selector !== silentOnGnosis) {
				if (selector === SELECTOR.groups) return answer(gnosis.groups);
				if (selector === SELECTOR.payload) return answer(payloadOnGnosis);
				if (selector === SELECTOR.unit)
					return answer(data.endsWith(word(12)) ? gnosis.unit12 : gnosis.unit10);
			}
			return answer(null);
		}
	)
}));

import { CHECKING_ROW, checkEthereumBackup, lastBackupRow } from './registry-backup';
import { sealStorage, unsealStorage } from './storage';

beforeEach(() => {
	ethereum = { version: gnosis.version, mirrored: NOT_MIRRORED };
	silentOnGnosis = null;
	payloadOnGnosis = gnosis.payload10;
	asked.length = 0;
	fetched.length = 0;
});

describe('checkEthereumBackup', () => {
	it('today: the registry is not on Ethereum — unavailable, and nothing else is asked', async () => {
		ethereum.version = '0x';
		expect(await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey)).toEqual({
			state: 'unavailable',
			call: null,
			unitId: null,
			// Nothing to draw: the core sends no row.
			row: null
		});
		expect(asked).toEqual([`1:${SELECTOR.version}`]);
	});

	it('not backed up: five questions, the right chain for each, and the REAL Gnosis bytes offered', async () => {
		const check = await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey);
		expect(asked).toEqual([
			`1:${SELECTOR.version}`,
			`100:${SELECTOR.groups}`,
			`100:${SELECTOR.unit}`, // unit 12 — the same key's OTHER wallet
			`100:${SELECTOR.unit}`, // unit 10 — this one
			`1:${SELECTOR.mirrored}`,
			`100:${SELECTOR.payload}`
		]);
		expect(check.state).toBe('not_backed_up');
		// The core's row: "not copied yet" is a state, not a warning, and a tap copies.
		expect(check.row).toEqual(ROW.notBackedUp);
		expect(check.unitId).toBe(10);
		expect(check.call).toMatchObject({ chain_id: 1, to: REGISTRY, value: '0' });
		// register(...) — 4,324 bytes, exactly what Gnosis holds for this wallet.
		expect(check.call?.data.slice(0, 10)).toBe('0xcd438f9b');
		expect((check.call!.data.length - 2) / 2).toBe(4324);
		expect(gnosis.payload10).toContain(check.call!.data.slice(2));
	});

	it('backed up: Ethereum holds the group, and the payload is never fetched', async () => {
		ethereum.mirrored = gnosis.mirroredTrue;
		const check = await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey);
		expect(check).toEqual({ state: 'backed_up', call: null, unitId: 10, row: ROW.backedUp });
		expect(asked).not.toContain(`100:${SELECTOR.payload}`);
	});

	it('another wallet of the same key is not this one: not registered', async () => {
		const other = '0x' + '11'.repeat(20);
		expect((await checkEthereumBackup(other, gnosis.foundingPublicKey)).state).toBe(
			'not_registered'
		);
	});

	it('silence on either chain is could-not-check — never "backed up", never a button', async () => {
		ethereum.version = null;
		expect((await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey)).state).toBe(
			'could_not_check'
		);
		ethereum.version = gnosis.version;
		for (const selector of [SELECTOR.groups, SELECTOR.unit, SELECTOR.payload]) {
			silentOnGnosis = selector;
			const check = await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey);
			expect(check, selector).toEqual({
				state: 'could_not_check',
				call: null,
				unitId: null,
				// The core's own row for silence: a tap asks again.
				row: ROW.couldNotCheck
			});
		}
		ethereum.mirrored = null;
		silentOnGnosis = null;
		expect((await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey)).state).toBe(
			'could_not_check'
		);
	});

	// A unit registered before registry V13 stored no payload. Gnosis ANSWERS
	// — with nothing — so asking again gets the same answer: the core calls it
	// `not_copyable`, a calm end with nothing to tap. It used to fall into
	// "could not check", and the row offered a retry for ever.
	it('an older wallet whose record stored no payload can never be copied — and is not asked again', async () => {
		payloadOnGnosis = EMPTY_BYTES;
		const check = await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey);
		expect(check).toEqual({ state: 'not_copyable', call: null, unitId: 10, row: ROW.cannotCopy });
	});

	// The one case the core never answers: the walk itself throws, so there is
	// no step to read a row from. The shell's own row for it is the core's
	// could-not-check row, word for word — the silence above is the core's.
	it('a walk that throws is could-not-check, drawn exactly as the core draws silence', async () => {
		const thrown = await checkEthereumBackup('not-an-address', 'not-a-key');
		expect(thrown.state).toBe('could_not_check');
		ethereum.version = null;
		const silent = await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey);
		expect(silent.state).toBe('could_not_check');
		expect(thrown.row).toEqual(silent.row);
		expect(thrown.row).toEqual(ROW.couldNotCheck);
	});

	it('while the walk runs the row is the core’s "Checking…": neutral, nothing to tap — and the explanation stays', () => {
		expect(CHECKING_ROW).toEqual({
			title_key: TITLE,
			subtitle_key: 'componentsUi.funding.checking',
			tone: 'neutral',
			action: 'none',
			// Nothing is known yet that would take the paragraph away.
			explain_key: EXPLAIN
		});
	});

	// PR 3 note 6: the paragraph describes making the copy. The core names it
	// for every state but the one that can never be copied — read here off the
	// REAL core's answers, so the web's mirror of the row cannot drift.
	it('the core names the explanation for every drawn state except "can never be copied"', async () => {
		const notCopied = await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey);
		expect(notCopied.state).toBe('not_backed_up');
		expect(notCopied.row?.explain_key).toBe(EXPLAIN);

		payloadOnGnosis = EMPTY_BYTES;
		const never = await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey);
		expect(never.state).toBe('not_copyable');
		expect(never.row).not.toBeNull();
		expect(never.row).not.toHaveProperty('explain_key');
	});

	it('server-free: the index service is never contacted', async () => {
		await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey);
		expect(fetched).toEqual([]);
	});
});

/**
 * perf-8: the row the Keys block stands on while the chain is asked again —
 * the core's last row for this account, never a verdict the chain did not
 * give, and never "could not check".
 */
describe('the last row drawn, per account', () => {
	it('keeps the core’s row for each verdict — and "nothing drawn" is remembered too', async () => {
		await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey);
		expect(lastBackupRow(gnosis.address, gnosis.foundingPublicKey)).toEqual(ROW.notBackedUp);

		ethereum.mirrored = gnosis.mirroredTrue;
		await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey);
		expect(lastBackupRow(gnosis.address.toUpperCase(), gnosis.foundingPublicKey)).toEqual(
			ROW.backedUp
		);

		ethereum.version = '0x';
		await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey);
		expect(lastBackupRow(gnosis.address, gnosis.foundingPublicKey)).toBeNull();
		const persisted = JSON.parse(stored.get('vela.backupCheck') ?? '{}') as Record<string, unknown>;
		expect(Object.values(persisted)).toContain(null);
	});

	it('"could not check" is a moment: the last verdict stays', async () => {
		ethereum.mirrored = gnosis.mirroredTrue;
		await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey);
		ethereum.version = null;
		expect((await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey)).state).toBe(
			'could_not_check'
		);
		expect(lastBackupRow(gnosis.address, gnosis.foundingPublicKey)).toEqual(ROW.backedUp);
	});

	it('a rehearsal on another chain is not the account’s answer; another account knows nothing', async () => {
		ethereum.mirrored = gnosis.mirroredTrue;
		await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey);
		await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey, { targetChainId: 8453 });
		expect(lastBackupRow(gnosis.address, gnosis.foundingPublicKey)).toEqual(ROW.backedUp);
		expect(lastBackupRow('0x' + '22'.repeat(20), gnosis.foundingPublicKey)).toBeUndefined();
	});

	it('nothing is written behind an erase’s sweep', async () => {
		stored.clear();
		sealStorage();
		try {
			ethereum.mirrored = gnosis.mirroredTrue;
			await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey);
			expect(stored.has('vela.backupCheck')).toBe(false);
		} finally {
			unsealStorage();
		}
	});

	it('a fresh page reads them back at once, believing only rows in the core’s shape', async () => {
		ethereum.mirrored = gnosis.mirroredTrue;
		await checkEthereumBackup(gnosis.address, gnosis.foundingPublicKey);
		const saved = JSON.parse(stored.get('vela.backupCheck')!) as Record<string, unknown>;
		stored.set(
			'vela.backupCheck',
			JSON.stringify({ ...saved, 'x#y': { title_key: 1 }, 'z#w': null })
		);
		vi.resetModules();
		const fresh = await import('./registry-backup');
		expect(fresh.lastBackupRow(gnosis.address, gnosis.foundingPublicKey)).toEqual(ROW.backedUp);
		expect(fresh.lastBackupRow('x', 'y')).toBeUndefined();
		expect(fresh.lastBackupRow('z', 'w')).toBeNull();
	});
});
