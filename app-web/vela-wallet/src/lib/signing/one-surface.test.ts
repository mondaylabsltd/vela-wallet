/**
 * One signing surface, as a rule the build enforces (spec 077).
 *
 * Three defects the owner reported were one defect wearing three faces, and
 * each of them was invisible to every existing test because each was an absent
 * prop or a URL in a three-line file. So they are pinned here, by reading the
 * source the way `core-table.test.ts` reads the worker's routing table.
 *
 * - **B-7** — "设置里的保存公钥到以太坊主网…和转账签名的体验差别很大". The
 *   landing was wired into the request WINDOW, and the backup's sheet is on the
 *   settings page, so the backup got nothing. A surface that mounts the sheet
 *   and forgets the receipt is the whole bug; that is what the first case here
 *   makes impossible to ship again.
 * - **B-2** — "侧边栏的钱包 UI 都在". The panel's doorway sent it to the request
 *   page, which replaced the wallet rather than sitting over it.
 * - **"签名管线应该统一一下吧"** — the answer lifecycle must live in ONE
 *   component. Two copies is two places for a request to go unanswered.
 */
import { readFileSync, readdirSync } from 'node:fs';
import { join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';

const ROOT = join(import.meta.dirname, '../../..');
const SRC = join(ROOT, 'src');

function walk(dir: string, out: string[] = []): string[] {
	for (const entry of readdirSync(dir, { withFileTypes: true })) {
		const full = join(dir, entry.name);
		if (entry.isDirectory()) walk(full, out);
		else out.push(full);
	}
	return out;
}

const svelteFiles = walk(SRC).filter((f) => f.endsWith('.svelte'));
const read = (f: string) => readFileSync(f, 'utf8');
const rel = (f: string) => relative(ROOT, f);

/**
 * The file with its comments removed.
 *
 * The wallet page EXPLAINS in a comment why the wiring lives in
 * `<SigningHost>`; without this the checks below would read that sentence as a
 * mount and fail on prose.
 */
const code = (f: string) =>
	read(f)
		.replace(/<!--[\s\S]*?-->/g, '')
		.replace(/\/\*[\s\S]*?\*\//g, '')
		.replace(/^\s*\/\/.*$/gm, '');

describe('every surface that mounts the signing sheet can show a landing', () => {
	it('passes `receipt` wherever `SigningHost` is mounted', () => {
		// The receipt is a PROP, so a surface can mount the sheet and silently
		// have no landing — which is exactly what the Ethereum backup did. The
		// sheet owns the landing (spec 077 FR-002); this is the rule that every
		// surface asks for it.
		const offenders: string[] = [];
		for (const file of svelteFiles) {
			const text = code(file);
			// The host's own definition, not a mount of it.
			if (file.endsWith('SigningHost.svelte')) continue;
			for (const tag of text.match(/<SigningHost[\s\S]*?\/>/g) ?? []) {
				if (!/receipt=\{/.test(tag)) offenders.push(rel(file));
			}
		}
		expect(offenders).toEqual([]);
	});

	it('mounts the sheet at all — at least the three surfaces that sign', () => {
		// A guard on the guard: if the regex above stopped matching anything, the
		// first case would pass by finding nothing.
		const mounts = svelteFiles.filter(
			(f) => !f.endsWith('SigningHost.svelte') && /<SigningHost/.test(code(f))
		);
		expect(mounts.length).toBeGreaterThanOrEqual(3);
	});
});

describe('the side panel is the wallet', () => {
	const panel = readFileSync(join(ROOT, 'extension/panel.js'), 'utf8');

	it('opens the wallet page, marked as the panel', () => {
		expect(panel).toMatch(/walletPage\(/);
		expect(panel).toMatch(/\?panel/);
	});

	it('does NOT open the request page, which would replace the wallet', () => {
		// The owner's B-2 in one line of code. `request.html` in the panel is the
		// wallet being gone.
		expect(panel).not.toMatch(/requestPage/);
	});
});

describe('the panel hears about a request that arrives while it is open', () => {
	it('watches the same storage keys the worker writes', async () => {
		// The panel stays open now, so `sidePanel.open` on a second request only
		// SHOWS it — no reload, no fresh mount. The page learns from the worker's
		// own record of pending requests, and this pins the two spellings of that
		// key together.
		const worker = await import('../../../extension/lib/protocol.js');
		const app = read(join(SRC, 'lib/dapp/transport.ts'));
		const declared = /const REQUEST_PREFIX = '([^']+)'/.exec(app)?.[1];
		expect(declared).toBe(worker.REQUEST_PREFIX);
	});
});

describe('one answer path, not one per surface', () => {
	const routes = walk(join(SRC, 'routes')).filter((f) => f.endsWith('.svelte'));

	it('no route page answers a dApp request itself', () => {
		// `DappRequestHost` owns the lifecycle. A page that calls `answerRequest`
		// or registers its own transport is a second copy of the one thing that
		// must happen exactly once.
		const offenders = routes.filter((f) => /answerRequest\(|registerTransport\(/.test(code(f)));
		expect(offenders.map(rel)).toEqual([]);
	});

	it('the host does own it', () => {
		const host = read(join(SRC, 'lib/dapp/DappRequestHost.svelte'));
		expect(host).toMatch(/answerRequest\(/);
		expect(host).toMatch(/registerTransport\(/);
		// And it settles a teardown with the CORE's code, never an invented one.
		expect(host).toMatch(/popupCloseSettlement\(/);
	});
});
